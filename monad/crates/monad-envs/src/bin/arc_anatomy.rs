//! M2-R 진단 — **잔차 해부(집계 전용)**.
//!
//! oracle 진단(시도 154)은 "조합으로 도달 불가"를, 잔차 EBM(시도 155)은 "국소
//! 문맥으로도 불가"를 말했다. 그러면 남은 잔차는 **어떤 종류의 구조**인가?
//!
//! 이 진단기는 과제를 사람에게 보여주지 않는다. 미해결 과제에서 부분 진전 후
//! 남는 잔차의 **집계 통계만** 낸다 — 그것이 필요한 생성적 기질의 최소 사양을
//! 정한다. 추측 대신 계량.
//!
//! 재는 것:
//! - 잔차 셀이 **뭉쳐 있나 흩어져 있나**(연결 성분 수 / 셀 수)
//! - **추가인가 삭제인가**(정답에 있는데 없음 vs 없는데 있음)
//! - **색 다양성**(잔차가 한 색인가 여러 색인가)
//! - **객체 경계 정렬**(잔차가 입력 객체의 bbox와 겹치나)
//! - **행/열 정렬**(한 줄에 몰려 있나)
//!
//! 실행: `cargo run --release --bin arc-anatomy`

use monad_core::abstraction::Library;
use monad_envs::arc_data::load_dir;
use monad_envs::arc_experience::{probe_partial, safe_chain};
use monad_envs::grid::{components, Grid};

/// 잔차 마스크의 연결 성분 수(4-이웃).
fn blobs(mask: &[bool], w: usize, h: usize) -> usize {
    let mut seen = vec![false; w * h];
    let mut n = 0;
    for s in 0..w * h {
        if !mask[s] || seen[s] {
            continue;
        }
        n += 1;
        let mut st = vec![s];
        seen[s] = true;
        while let Some(i) = st.pop() {
            let (x, y) = (i % w, i / w);
            let nb = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in nb {
                if nx < w && ny < h {
                    let j = ny * w + nx;
                    if mask[j] && !seen[j] {
                        seen[j] = true;
                        st.push(j);
                    }
                }
            }
        }
    }
    n
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_LIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-library.tsv".into());
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());

    println!("=========================================================================");
    println!("M2-R 잔차 해부 — 남은 것은 어떤 종류의 구조인가 (집계 전용)");
    println!("=========================================================================");

    let lib = Library::load(&lib_path).unwrap_or_default();
    let solved: Vec<String> = std::fs::read_to_string(&solved_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let tasks = load_dir(std::path::Path::new(&dir));
    let unsolved: Vec<_> = tasks.into_iter().filter(|t| !solved.contains(&t.name)).collect();

    let mut n = 0usize;
    let (mut sum_cells, mut sum_blobs) = (0f64, 0f64);
    let (mut missing, mut extra, mut wrongcolor) = (0f64, 0f64, 0f64);
    let mut sum_colors = 0f64;
    let mut aligned_obj = 0f64;
    let mut line_aligned = 0f64;

    for task in &unsolved {
        let train: Vec<(Grid, Grid)> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let Some((seed, _, _, _)) = probe_partial(&lib, &train, 6_000) else { continue };
        // 첫 훈련쌍의 잔차만 본다(집계 목적 — 과제별 내용은 출력하지 않는다)
        let (gi, go) = &train[0];
        let Some(mid) = safe_chain(gi, &seed) else { continue };
        if mid.w != go.w || mid.h != go.h {
            continue;
        }
        let (w, h) = (go.w, go.h);
        let mut mask = vec![false; w * h];
        let (mut miss, mut ext, mut wc) = (0usize, 0usize, 0usize);
        let mut colors: Vec<u8> = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let want = go.get(x, y);
                let got = mid.get(x, y);
                if want == got {
                    continue;
                }
                mask[y * w + x] = true;
                if !colors.contains(&want) {
                    colors.push(want);
                }
                match (got == 0, want == 0) {
                    (true, false) => miss += 1,  // 있어야 하는데 비어 있음
                    (false, true) => ext += 1,   // 없어야 하는데 차 있음
                    _ => wc += 1,                // 둘 다 유색인데 색이 다름
                }
            }
        }
        let cells = miss + ext + wc;
        if cells == 0 {
            continue;
        }
        n += 1;
        sum_cells += cells as f64;
        sum_blobs += blobs(&mask, w, h) as f64;
        missing += miss as f64 / cells as f64;
        extra += ext as f64 / cells as f64;
        wrongcolor += wc as f64 / cells as f64;
        sum_colors += colors.len() as f64;

        // 입력 객체 bbox와의 정렬
        let objs = components(gi);
        let inside = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| mask[y * w + x])
            .filter(|&(x, y)| {
                objs.iter().any(|o| {
                    x >= o.x0 && x < o.x0 + o.w && y >= o.y0 && y < o.y0 + o.h
                })
            })
            .count();
        aligned_obj += inside as f64 / cells as f64;

        // 행/열 집중도: 잔차가 걸친 행 수·열 수가 적을수록 줄 구조
        let rows: usize = (0..h).filter(|&y| (0..w).any(|x| mask[y * w + x])).count();
        let cols: usize = (0..w).filter(|&x| (0..h).any(|y| mask[y * w + x])).count();
        line_aligned += 1.0 - (rows.min(cols) as f64 / rows.max(cols).max(1) as f64);
    }

    if n == 0 {
        println!("부분 진전이 있는 미해결 과제가 없다 — 해부할 잔차가 없음.");
        return;
    }
    let f = n as f64;
    println!("표본 {n}건(부분 진전 뒤 남은 잔차, 첫 훈련쌍 기준)\n");
    println!("  평균 잔차 셀 수      {:.1}", sum_cells / f);
    println!("  평균 덩어리 수       {:.1}  (셀당 {:.2} — 1에 가까울수록 흩어짐)",
        sum_blobs / f, sum_blobs / sum_cells.max(1.0));
    println!("  구성:  없어서 틀림 {:.0}% · 남아서 틀림 {:.0}% · 색만 틀림 {:.0}%",
        100.0 * missing / f, 100.0 * extra / f, 100.0 * wrongcolor / f);
    println!("  잔차 색 종류         {:.1}", sum_colors / f);
    println!("  입력 객체 안에 위치  {:.0}%  (밖이면 새 위치에 생성해야 하는 것)",
        100.0 * aligned_obj / f);
    println!("  행/열 편중도         {:.2}  (1에 가까울수록 한 줄 구조)", line_aligned / f);
    println!("\n▶ 이 통계가 생성적 기질의 최소 사양을 정한다(추측 아님).");
}
