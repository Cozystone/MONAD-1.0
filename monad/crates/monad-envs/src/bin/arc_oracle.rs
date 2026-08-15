//! M2-R 진단 — **oracle reachability**: 못 푸는 이유를 네 갈래로 가른다.
//!
//! 미해결 과제에 큰 예산을 주고, 현재 라이브러리로 정답이 **애초에 표현
//! 가능한가**를 묻는다. 답에 따라 다음 연구 방향이 갈린다:
//!
//! | 멈춘 이유 | 해석 | 다음 표적 |
//! |---|---|---|
//! | `Solved` | 표현 가능·발견함 | (예산만 늘리면 됨) |
//! | `BudgetExhausted` | 표현 가능성 미정 — **탐색 병목 가능** | 사전분포·가지치기 |
//! | `FrontierExhausted` | 단조 경로로 **도달 불가** | **표현/개념 발명** |
//!
//! 마지막 줄이 핵심이다. 탐색을 아무리 개선해도 도달 불가라면, 필요한 것은
//! 더 나은 검색이 아니라 **없는 개념을 만드는 일**이다.
//!
//! 정직 고지: `FrontierExhausted`는 "일시적 악화를 거치는 경로"를 배제하므로
//! 도달 불가의 **증거**이지 증명은 아니다. 보고에 그대로 적는다.
//!
//! 이 진단기는 과제 내용을 사람에게 보여주지 않는다 — 집계만 낸다(봉인 규율).
//!
//! 실행: `cargo run --release --bin arc-oracle`

use monad_core::abstraction::Library;
use monad_envs::arc_experience::{search_report, StopReason};
use monad_envs::arc_data::load_dir;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_LIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-library.tsv".into());
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());
    let budget: u32 = std::env::var("MONAD_ARC_ORACLE_BUDGET")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(400_000);
    let depth: usize = std::env::var("MONAD_ARC_ORACLE_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let sample: usize = std::env::var("MONAD_ARC_ORACLE_N")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);

    println!("=========================================================================");
    println!("M2-R oracle reachability — 못 푸는 이유의 네 갈래 분해");
    println!("=========================================================================");

    let mut lib = Library::load(&lib_path).unwrap_or_default();
    let solved: Vec<String> = std::fs::read_to_string(&solved_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let tasks = load_dir(std::path::Path::new(&dir));
    let unsolved: Vec<_> = tasks
        .into_iter()
        .filter(|t| !solved.contains(&t.name))
        .collect();

    println!(
        "라이브러리 {}개 · 미해결 {}건 중 앞 {}건 진단 · 예산 {} · 깊이 {}\n",
        lib.entries.len(),
        unsolved.len(),
        sample.min(unsolved.len()),
        budget,
        depth
    );

    let (mut n_solved, mut n_budget, mut n_frontier) = (0usize, 0usize, 0usize);
    let mut res_budget = Vec::new();
    let mut res_frontier = Vec::new();
    let t0 = std::time::Instant::now();

    for task in unsolved.iter().take(sample) {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let r = search_report(&mut lib, &train, budget, depth);
        match r.stop {
            StopReason::Solved => n_solved += 1,
            StopReason::BudgetExhausted => {
                n_budget += 1;
                res_budget.push(r.best_residual);
            }
            StopReason::FrontierExhausted => {
                n_frontier += 1;
                res_frontier.push(r.best_residual);
            }
        }
    }

    let mean = |v: &[f64]| if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 };
    let n = sample.min(unsolved.len()).max(1);
    println!("진단 결과({:.0}초):", t0.elapsed().as_secs_f32());
    println!(
        "  ✅ 표현 가능·발견        {:>3}건 ({:.0}%) — 예산만 있으면 풀린다",
        n_solved,
        100.0 * n_solved as f64 / n as f64
    );
    println!(
        "  ⏳ 예산 소진(미정)       {:>3}건 ({:.0}%) · 최선 잔차 평균 {:.3} — **탐색 병목 후보**",
        n_budget,
        100.0 * n_budget as f64 / n as f64,
        mean(&res_budget)
    );
    println!(
        "  🚧 단조 경로 소진        {:>3}건 ({:.0}%) · 최선 잔차 평균 {:.3} — **표현/개념 부재**",
        n_frontier,
        100.0 * n_frontier as f64 / n as f64,
        mean(&res_frontier)
    );
    println!(
        "\n▶ 해석: 탐색 개선의 상한 = 예산 소진 {}건. 그 너머({}건)는 개념 발명이 필요.",
        n_budget, n_frontier
    );
    println!("  (단조 경로 소진은 일시적 악화 경로를 배제한 결과 — 도달 불가의 증거이지 증명은 아니다.)");
}
