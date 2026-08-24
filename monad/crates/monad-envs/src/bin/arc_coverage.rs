//! M2-R 진단 — **홀드아웃 전이 실패의 분해**(집계 전용).
//!
//! 시도 161에서 출처 분리 후 게이트 통과가 0이 됐다. 이유는 둘 중 하나다:
//!
//! | 원인 | 서명 | 처방 |
//! |---|---|---|
//! | **덮지 못함**(경험 부족·문법 한계) | 바뀐 셀 중 규칙이 없는 비율이 높다 | 경험 확대 / 문법 확장 |
//! | **덮지만 오발화**(선택 실패) | 덮개는 높은데 오발화가 있다 | 선택 규율 강화 |
//!
//! 추측 없이 이 둘을 가른다. 과제 내용은 출력하지 않는다(봉인 규율).
//!
//! 실행: `cargo run --release --bin arc-coverage`

use monad_core::abstraction::Library;
use monad_envs::arc_data::load_dir;
use monad_envs::arc_patch::{coverage_report, CoverageStats};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_PATCHLIB").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-patchlib.tsv".into()
    });
    let src_path = std::env::var("MONAD_ARC_PATCHSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-patchsource.txt".into()
    });
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());

    println!("=========================================================================");
    println!("M2-R 전이 실패 분해 — 덮지 못하는가, 덮지만 오발화하는가 (집계 전용)");
    println!("=========================================================================");

    let lib = Library::load(&lib_path).unwrap_or_default();
    let sources: Vec<String> = std::fs::read_to_string(&src_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let solved: Vec<String> = std::fs::read_to_string(&solved_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let tasks = load_dir(std::path::Path::new(&dir));
    // 홀드아웃 = 규칙 출처가 아니고, 동결 솔버도 못 푼 과제
    let holdout: Vec<_> = tasks
        .into_iter()
        .filter(|t| !sources.contains(&t.name) && !solved.contains(&t.name))
        .collect();

    println!("규칙 {}개 · 출처 과제 {}개 제외 · 홀드아웃 {}건\n",
        lib.entries.len(), sources.len(), holdout.len());

    let mut n = 0usize;
    let mut agg = CoverageStats::default();
    let mut full_cover = 0usize;
    let mut any_cover = 0usize;
    let mut misfire_only = 0usize;
    let mut clean_full = 0usize;

    for task in &holdout {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        if train.iter().any(|(i, o)| i.w != o.w || i.h != o.h) {
            continue;
        }
        let s = coverage_report(&lib, &train);
        if s.changed == 0 {
            continue;
        }
        n += 1;
        agg.changed += s.changed;
        agg.covered += s.covered;
        agg.clean_covered += s.clean_covered;
        agg.misfires += s.misfires;
        agg.unchanged += s.unchanged;
        if s.clean_covered == s.changed {
            clean_full += 1;
        }
        if s.covered == s.changed {
            full_cover += 1;
            if s.misfires > 0 {
                misfire_only += 1;
            }
        }
        if s.covered > 0 {
            any_cover += 1;
        }
    }

    if n == 0 {
        println!("분석할 홀드아웃 과제가 없다.");
        return;
    }
    let f = n as f64;
    println!("표본 {n}건(홀드아웃·동일 크기)\n");
    println!(
        "  바뀌어야 하는 셀 중 **규칙이 있는 비율(덮개)**: {:.1}%",
        100.0 * agg.covered as f64 / agg.changed.max(1) as f64
    );
    println!(
        "  덮개 100% 과제: {}건 ({:.0}%) · 그중 오발화로 막힌 것: {}건",
        full_cover,
        100.0 * full_cover as f64 / f,
        misfire_only
    );
    println!(
        "  일부라도 덮인 과제: {}건 ({:.0}%)",
        any_cover,
        100.0 * any_cover as f64 / f
    );
    println!(
        "  **오발화**(안 바뀌어야 하는 셀에서 규칙이 다른 색을 주장): 평균 {:.1}셀/과제",
        agg.misfires as f64 / f
    );
    let clean_rate = agg.clean_covered as f64 / agg.changed.max(1) as f64;
    println!(
        "\n  ★ **깨끗한 덮개**(오발화 없는 맞는 규칙이 있는 셀): {:.1}%  · 전 셀이 깨끗한 과제 {}건",
        100.0 * clean_rate,
        clean_full
    );
    println!("\n▶ 판정:");
    if clean_rate < 0.5 {
        println!(
            "  **문법의 표현력 한계**(깨끗한 덮개 {:.0}%) — 3×3 이웃으로는 '어디에 적용할지'를\n  \
             가릴 수 없다. 맞는 규칙은 있으나 같은 조건이 다른 자리에서도 성립해 오발화한다.\n  \
             처방: 조건의 표현력 확장(더 넓은 문맥·객체/영역 수준 술어) — 선택 튜닝으로는 못 연다.",
            100.0 * clean_rate
        );
        return;
    }
    let cover_rate = agg.covered as f64 / agg.changed.max(1) as f64;
    if cover_rate < 0.5 {
        println!("  **덮지 못한다**(덮개 {:.0}%) — 경험 부족 또는 3×3 패치 문법의 표현력 한계.",
            100.0 * cover_rate);
        println!("  처방: 경험 확대(전 과제 학습) 또는 문법 확장(더 큰 문맥·객체 수준 조건).");
    } else if misfire_only > 0 {
        println!("  **덮지만 오발화한다** — 선택 규율을 강화하면 열린다.");
    } else {
        println!("  덮개는 있으나 완전 덮개 과제가 드물다 — 경험 확대가 1차 처방.");
    }
}
