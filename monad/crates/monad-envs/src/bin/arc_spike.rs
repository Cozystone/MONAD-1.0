//! W2-0 — ARC 스파이크: 격자 변환의 스키마 귀납 (합성 10과제군).
//!
//! 솔버 본체는 `monad_envs::arc_solve`(공용 모듈 — 시도 62에서 10/10 검증).
//! 이 하네스는 합성 과제 생성·채점·유리상자 덤프만 담당한다.
//!
//! 실행: `cargo run --release --bin arc-spike`

use monad_envs::arc_solve::{apply, learn, CLASS_NAMES, S_COLOR, S_COPY_K, S_GAP, S_LARGEST};
use monad_envs::grid::{make_tasks, Grid};

fn main() {
    println!("=========================================================================");
    println!("W2-0 — ARC 스파이크: 객체 변환 이벤트 → 스키마 귀납(MDL) → 시험 적용");
    println!("=========================================================================");
    println!("합성 10과제군 · 훈련 3쌍 · 시험 1쌍 · 채점 = 격자 정확 일치\n");

    let mut pass = 0;
    let mut total = 0;
    let mut results = Vec::new();
    for seed in [7u64] {
        for task in make_tasks(seed) {
            total += 1;
            let libs = learn(&task.train);
            let pred = apply(&task.test_in, &libs);
            let ok = pred == task.test_out;
            if ok {
                pass += 1;
            } else if std::env::var("MONAD_ARC_DEBUG").is_ok() {
                let show = |g: &Grid, tag: &str| {
                    println!("    {tag}:");
                    for y in 0..g.h {
                        let row: String =
                            (0..g.w).map(|x| char::from(b'0' + g.get(x, y))).collect();
                        println!("      {row}");
                    }
                };
                println!("  [디버그] {} 실패:", task.name);
                println!(
                    "    copies {}규칙/기본{:?} · dx {}/{:?} · class {}/{:?}",
                    libs.copies.schemas.len(),
                    libs.copies.default_effect,
                    libs.dx.schemas.len(),
                    libs.dx.default_effect,
                    libs.class.schemas.len(),
                    libs.class.default_effect
                );
                for s in &libs.copies.schemas {
                    println!(
                        "    copies 규칙: {}",
                        s.describe(&|sl| format!("s{sl}"), &|e| format!("{e}"))
                    );
                }
                show(&task.test_in, "입력");
                show(&task.test_out, "정답");
                show(&pred, "예측");
            }
            let head = libs
                .class
                .schemas
                .iter()
                .map(|s| {
                    s.describe(
                        &|slot| match slot {
                            S_COLOR => "색".to_string(),
                            S_LARGEST => "최대객체".to_string(),
                            S_COPY_K => "사본k".to_string(),
                            S_GAP => "접지".to_string(),
                            _ => format!("s{slot}"),
                        },
                        &|e| CLASS_NAMES.get(e as usize).unwrap_or(&"?").to_string(),
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            results.push((task.name, ok, head));
        }
    }
    for (name, ok, head) in &results {
        println!("  {:>16}: {} · 규칙 {}", name, if *ok { "✅" } else { "❌" }, head);
    }
    let bar = total - 2; // 합격선: 전체 − 2
    println!(
        "\n▶ W2-0 스파이크: {}/{} {}",
        pass,
        total,
        if pass >= bar { "✅ 통과" } else { "❌ 미달" }
    );
    std::process::exit(if pass >= bar { 0 } else { 1 });
}
