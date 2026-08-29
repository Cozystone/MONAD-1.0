//! M2-R — **관계 규칙 수면**(GEN3): 존재 양화가 있는 규칙을 경험에서 만든다.
//!
//! 객체 계층(GEN2)과 같은 규율: 기계적 추출 → 반례 기반 조건 탈락(과제 내 판정)
//! → MONAD_DERIVED 라이브러리 + **출처 과제 목록**(provenance holdout).
//!
//! 실행: `cargo run --release --bin arc-relsleep`

use monad_core::abstraction::{Library, Provenance};
use monad_envs::arc_data::load_dir;
use monad_envs::arc_relrule::{sleep_rel_drop, task_rsites, RSite};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_RELLIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-rellib.tsv".into());
    let src_path = std::env::var("MONAD_ARC_RELSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-relsource.txt".into()
    });
    let take: usize = std::env::var("MONAD_ARC_REL_TAKE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    println!("=========================================================================");
    println!("M2-R 관계 규칙 수면(GEN3) — 존재 양화 규칙을 경험에서 만든다");
    println!("=========================================================================");

    let tasks = load_dir(std::path::Path::new(&dir));
    let mut lib = Library::load(&lib_path).unwrap_or_default();
    let before = lib.entries.len();

    let mut per_task: Vec<Vec<RSite>> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    let mut total_sites = 0usize;
    for task in tasks.iter().take(take) {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let st = task_rsites(&train);
        // 바뀐 객체가 하나라도 있어야 규칙의 씨앗이 된다
        if st.iter().any(|s| s.delta.is_some()) {
            sources.push(task.name.clone());
            total_sites += st.len();
            per_task.push(st);
        }
    }
    let _ = std::fs::write(&src_path, sources.join("\n"));
    println!(
        "관계 지점 수집: {}개 (과제 {}개에서, 앞 {}개 대상) · 출처 기록 완료",
        total_sites,
        sources.len(),
        take
    );
    if per_task.is_empty() {
        println!("일반화할 것이 없다.");
        return;
    }

    let t0 = std::time::Instant::now();
    let (tried, added) = sleep_rel_drop(&per_task, &mut lib);
    let _ = lib.save(&lib_path);
    println!(
        "반례 기반 탈락(존재 양화): 씨앗 {tried}개 → **새 규칙 {added}개** \
         (라이브러리 {} → {}) · {:.1}초",
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
