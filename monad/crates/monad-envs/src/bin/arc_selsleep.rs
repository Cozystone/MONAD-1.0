//! M2-R — **선택 규칙 수면**: "무엇이 답인가"를 경험에서 배운다(시도 206).
//!
//! 다른 계층과 같은 규율: 기계적 추출 → 반례 기반 조건 탈락(과제 내 판정) →
//! MONAD_DERIVED 라이브러리 + **출처 과제 목록**(provenance holdout).
//!
//! 실행: `cargo run --release --bin arc-selsleep`

use monad_core::abstraction::{Library, Provenance};
use monad_envs::arc_data::load_dir;
use monad_envs::arc_select::{sleep_sel_drop, task_sel_sites, SelSite};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_SELLIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-sellib.tsv".into());
    let src_path = std::env::var("MONAD_ARC_SELSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-selsource.txt".into()
    });
    let take: usize = std::env::var("MONAD_ARC_SEL_TAKE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    println!("=========================================================================");
    println!("M2-R 선택 규칙 수면 — \"무엇이 답인가\"를 경험에서 배운다");
    println!("=========================================================================");

    let tasks = load_dir(std::path::Path::new(&dir));
    let mut lib = Library::load(&lib_path).unwrap_or_default();
    let before = lib.entries.len();

    let mut per_task: Vec<(String, Vec<SelSite>)> = Vec::new();
    let mut sources: Vec<String> = Vec::new();
    for task in tasks.iter().take(take) {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let st = task_sel_sites(&train);
        if !st.is_empty() {
            sources.push(task.name.clone());
            per_task.push((task.name.clone(), st));
        }
    }
    let _ = std::fs::write(&src_path, sources.join("\n"));
    println!(
        "선택 관측 수집: 과제 {}개 (앞 {}개 대상) · 출처 기록 완료",
        sources.len(),
        take
    );
    if per_task.is_empty() {
        println!("선택 문제로 기술되는 과제가 없다.");
        return;
    }

    let t0 = std::time::Instant::now();
    let (tried, added) = sleep_sel_drop(&per_task, &mut lib);
    let _ = lib.save(&lib_path);
    println!(
        "반례 기반 탈락: 씨앗 {tried}개 → **새 규칙 {added}개** (라이브러리 {} → {}) · {:.1}초",
        before,
        lib.entries.len(),
        t0.elapsed().as_secs_f32()
    );
    println!(
        "출처: MONAD_DERIVED {} · 압축률 {:.2}",
        lib.count(Provenance::MonadDerived),
        lib.compression()
    );
    println!("\n▶ 이 계층의 게이트는 **쌍당 결정 하나**다 — 과제당 100% 덮개 요구가 없다.");
}
