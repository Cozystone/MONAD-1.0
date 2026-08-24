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
use monad_envs::arc_objrule::{extract_obj_rules, sleep_obj_abstract};

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
    let mut sources: Vec<String> = Vec::new();
    for task in tasks.iter().take(take) {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let r = extract_obj_rules(&train);
        if !r.is_empty() {
            sources.push(task.name.clone());
            rules.extend(r);
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
    let (tried, added) = sleep_obj_abstract(&rules, &mut lib);
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
