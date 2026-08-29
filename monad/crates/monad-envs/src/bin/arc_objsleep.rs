//! M2-R — **객체 델타 수면**: 표현 경쟁의 승자(단색 4-연결 객체) 위에서
//! 재색·삭제 규칙을 일반화해 축적한다.
//!
//! 패치 수면과 같은 규율: 기계적 추출(해석 없음) → LGG+MDL → MONAD_DERIVED
//! 라이브러리 + **출처 과제 목록**(전이 시험에서 자기 출처를 차단하기 위한
//! provenance holdout — 시도 161의 교훈).
//!
//! 실행: `cargo run --release --bin arc-objsleep`

use monad_core::abstraction::{Library, Provenance};
use monad_envs::arc_data::load_dir;
use monad_envs::arc_objrule::{
    extract_obj_rules, sleep_obj_abstract, sleep_obj_cross, sleep_obj_drop,
    sleep_obj_refine_rounds, task_props_partial, Site,
};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_OBJLIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objlib.tsv".into());
    let src_path = std::env::var("MONAD_ARC_OBJSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objsource.txt".into()
    });
    let take: usize = std::env::var("MONAD_ARC_OBJ_TAKE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    println!("=========================================================================");
    println!("M2-R 객체 델타 수면 — 승자 표현 위에서 재색·삭제 규칙을 스스로 만든다");
    println!("=========================================================================");

    let tasks = load_dir(std::path::Path::new(&dir));
    let mut lib = Library::load(&lib_path).unwrap_or_default();
    let before = lib.entries.len();

    let mut rules = Vec::new();
    let mut groups: Vec<(String, Vec<monad_core::abstraction::Term>)> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    // 반례 집합(시도 182): 경험 과제의 모든 관측 지점 — 조건 탈락의 근거
    let mut per_task: Vec<(String, Vec<Site>)> = Vec::new();
    // 커리큘럼 목록이 있으면 **그 과제들만** 쓴다(능동 선택). 없으면 앞 take개(수동).
    let cur_list: Vec<String> = std::env::var("MONAD_ARC_CUR_LIST")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let selected: Vec<&monad_envs::arc_data::ArcTask> = if cur_list.is_empty() {
        tasks.iter().take(take).collect()
    } else {
        tasks.iter().filter(|t| cur_list.contains(&t.name)).collect()
    };
    for task in selected {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let r = extract_obj_rules(&train);
        if !r.is_empty() {
            sources.push(task.name.clone());
            // 부분 관측 지점(시도 186): 완전 기술되지 않는 과제에서도 짝이
            // 확정된 객체는 참인 델타를 갖는다 — 인가 지점을 늘리는 유일한 축.
            let st = task_props_partial(&train);
            if !st.is_empty() {
                per_task.push((task.name.clone(), st));
            }
            // 규칙마다 낳은 과제를 달고 다닌다 — 출처를 정확히 찍기 위해서다.
            rules.extend(r.iter().cloned().map(|t| (task.name.clone(), t)));
            groups.push((task.name.clone(), r));
        }
    }
    let _ = std::fs::write(&src_path, sources.join("\n"));
    println!(
        "델타 경험 추출: {}건 (과제 {}개에서, 앞 {}개 대상) · 출처 기록 완료",
        rules.len(),
        sources.len(),
        take
    );
    if rules.len() < 2 {
        println!("일반화할 것이 없다.");
        return;
    }

    let t0 = std::time::Instant::now();
    // 두 반일반화는 이제 **자기가 접은 과제만** 출처로 찍는다(함수 안에서).
    // 전체 목록을 찍던 초안은 규칙 하나를 150개 과제 모두에게 가려 버려서
    // 사실상 무용지물로 만들었다 — 엄격한 것과 부정확한 것은 다르다.
    let (tried, added) = sleep_obj_abstract(&rules, &mut lib);
    // 과제 간 수면 — 전이 규칙의 원천(시도 168)
    let (tried_x, added_x) = sleep_obj_cross(&groups, &mut lib);
    println!(
        "과제 간 일반화: 시도 {tried_x}회 → 새 규칙 {added_x}개 (서로 다른 과제의 경험쌍)"
    );
    // 일반화 사다리(시도 173): 고정점까지 오른다 — 라운드마다 더 일반적인 층이
    // 쌓이고, MDL이 스스로 멈춘다. 여러 층이 공존한 채 과제의 증거가 고른다.
    let rounds: usize = std::env::var("MONAD_ARC_REFINE_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(6);
    // **반례 기반 조건 탈락**(시도 182): 경험 전체를 반례 집합으로 삼아 무관한
    // 슬롯을 의도적으로 떨어뜨린다 — LGG(긍정 쌍만 봄)와 상보적이며, 판별력과
    // 덮개를 동시에 얻는 유일한 경로.
    let (tried_d, added_d) = sleep_obj_drop(&per_task, &mut lib);
    println!(
        "반례 기반 탈락(과제 내 판정): 씨앗 {tried_d}개 · 과제 {}개 → 새 규칙 {added_d}개",
        per_task.len()
    );
    let log = sleep_obj_refine_rounds(&mut lib, rounds);
    let total_added: usize = log.iter().map(|(_, a)| a).sum();
    println!(
        "일반화 사다리: {}라운드 → 새 규칙 {}개 (라운드별 {})",
        log.len(),
        total_added,
        log.iter()
            .map(|(_, a)| a.to_string())
            .collect::<Vec<_>>()
            .join("+")
    );
    let _ = lib.save(&lib_path);
    println!(
        "일반화 시도 {tried}회 → **새 규칙 {added}개** (라이브러리 {} → {}) · {:.1}초",
        before,
        lib.entries.len(),
        t0.elapsed().as_secs_f32()
    );
    println!(
        "출처: MONAD_DERIVED {} · 압축률 {:.2}",
        lib.count(Provenance::MonadDerived),
        lib.compression()
    );
    println!("\n▶ 다음 각성이 이 규칙들을 **출처 밖 과제**에 적용한다(엄격 홀드아웃 전이).");
}
