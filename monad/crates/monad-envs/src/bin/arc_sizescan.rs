//! M2-R 진단 — **크기 변환 과제의 구조**(집계 전용, 시도 205).
//!
//! 약 스무 번의 개입이 모두 ①(시도 가능 22건) 안에서만 움직였다. 그런데 홀드아웃
//! 254건 중 **231건이 애초에 기술 불가**이고 그중 **97건이 크기 불일치**다 —
//! 시도 가능 풀의 **4배**이며 지금까지 손대지 않은 가장 큰 몫이다.
//!
//! 만들기 전에 잰다: 그 97건의 입출력 크기 관계가 **학습 가능한 구조**인가?
//!
//! | 부류 | 의미 |
//! |---|---|
//! | `배수 k` | 출력 = 입력 × k (양변 같은 배수) — 확대/타일 |
//! | `배수 kx,ky` | 양변 다른 배수 |
//! | `약수` | 출력 = 입력 ÷ k — 축소/요약 |
//! | `객체 bbox` | 출력 크기 = 입력 어느 객체의 bbox — 추출 |
//! | `고정` | 모든 쌍의 출력 크기가 같음 — 상수 출력 |
//! | `기타` | 위 어디에도 안 맞음 |
//!
//! 배수·약수·추출이 대부분이면 작은 학습 어휘로 열리고, 기타가 대부분이면
//! 이 슬라이스도 이 표현족 밖이다. 과제 내용은 출력하지 않는다(봉인 규율).
//!
//! 실행: `arc-sizescan`

use monad_envs::arc_data::load_dir;
use monad_envs::grid::{components_bg, Grid};

#[derive(Default)]
struct Counts {
    scale_k: usize,
    scale_kxky: usize,
    divide: usize,
    obj_bbox: usize,
    fixed: usize,
    other: usize,
}

/// 한 훈련쌍의 크기 관계를 분류한다(우선순위 순 — 가장 단순한 설명 먼저).
fn classify_pair(i: &Grid, o: &Grid) -> &'static str {
    if i.w == o.w && i.h == o.h {
        return "same";
    }
    // 배수(양변 같은 k)
    if o.w % i.w == 0 && o.h % i.h == 0 {
        let (kx, ky) = (o.w / i.w, o.h / i.h);
        return if kx == ky { "scale_k" } else { "scale_kxky" };
    }
    // 약수
    if i.w % o.w == 0 && i.h % o.h == 0 {
        return "divide";
    }
    // 출력 크기가 입력 어느 객체의 bbox와 같은가(추출)
    let objs = components_bg(i, false, 0);
    if objs.iter().any(|b| b.w == o.w && b.h == o.h) {
        return "obj_bbox";
    }
    "other"
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());

    println!("=========================================================================");
    println!("M2-R 크기 변환 구조 — 가장 큰 미개척 슬라이스의 계량 (집계 전용)");
    println!("=========================================================================");

    let solved: Vec<String> = std::fs::read_to_string(&solved_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let tasks = load_dir(std::path::Path::new(&dir));

    let mut c = Counts::default();
    let mut n_size = 0usize;
    let mut consistent_task = 0usize; // 모든 훈련쌍이 **같은 부류**인 과제
    let mut same_k_task = 0usize; // 배수가 모든 쌍에서 **같은 값**인 과제
    // **선택 문제인가**(시도 206): 출력이 입력의 어느 부분(객체 bbox 잘라내기)과
    // 정확히 같은가. 그렇다면 이 과제족은 "무엇이 답인가"를 고르는 문제이고,
    // 성질 규칙 기계를 **선택**에 재사용할 수 있다 — 지금까지는 "각 객체에 무슨
    // 일이 일어나는가"만 배웠다.
    let mut selection_task = 0usize;
    let mut selection_pairs = 0usize;
    let mut selection_total = 0usize;

    for task in tasks.iter().filter(|t| !solved.contains(&t.name)) {
        let pairs: Vec<(Grid, Grid)> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        if pairs.iter().all(|(i, o)| i.w == o.w && i.h == o.h) {
            continue; // 크기 변환 과제가 아니다
        }
        n_size += 1;

        // 출력 크기가 모든 쌍에서 동일한가(상수 출력)
        let fixed = pairs
            .windows(2)
            .all(|w| w[0].1.w == w[1].1.w && w[0].1.h == w[1].1.h);

        // 각 쌍에서 출력이 입력의 어느 객체 bbox 잘라내기와 정확히 같은가
        let mut sel_ok = 0usize;
        for (i, o) in &pairs {
            selection_total += 1;
            let objs = components_bg(i, false, 0);
            let hit = objs.iter().any(|b| {
                if b.w != o.w || b.h != o.h {
                    return false;
                }
                (0..o.h).all(|y| (0..o.w).all(|x| i.get(b.x0 + x, b.y0 + y) == o.get(x, y)))
            });
            if hit {
                sel_ok += 1;
                selection_pairs += 1;
            }
        }
        if sel_ok == pairs.len() {
            selection_task += 1;
        }

        let kinds: Vec<&str> = pairs.iter().map(|(i, o)| classify_pair(i, o)).collect();
        let all_same = kinds.windows(2).all(|w| w[0] == w[1]);
        if all_same {
            consistent_task += 1;
        }
        // 대표 부류(첫 쌍 기준, fixed가 더 단순하면 fixed 우선)
        let rep = if fixed && kinds[0] != "scale_k" { "fixed" } else { kinds[0] };
        match rep {
            "scale_k" => {
                c.scale_k += 1;
                // 배수가 모든 쌍에서 같은 값인가
                let ks: Vec<usize> = pairs
                    .iter()
                    .filter(|(i, o)| o.w % i.w == 0)
                    .map(|(i, o)| o.w / i.w)
                    .collect();
                if !ks.is_empty() && ks.windows(2).all(|w| w[0] == w[1]) {
                    same_k_task += 1;
                }
            }
            "scale_kxky" => c.scale_kxky += 1,
            "divide" => c.divide += 1,
            "obj_bbox" => c.obj_bbox += 1,
            "fixed" => c.fixed += 1,
            _ => c.other += 1,
        }
    }

    if n_size == 0 {
        println!("크기 변환 과제가 없다.");
        return;
    }
    let pct = |x: usize| 100.0 * x as f64 / n_size as f64;
    println!("미해결 크기 변환 과제 {n_size}건\n");
    println!("  배수 k(양변 동일)  {:>3}건 ({:.0}%)", c.scale_k, pct(c.scale_k));
    println!("  배수 kx,ky        {:>3}건 ({:.0}%)", c.scale_kxky, pct(c.scale_kxky));
    println!("  약수(축소)        {:>3}건 ({:.0}%)", c.divide, pct(c.divide));
    println!("  객체 bbox(추출)   {:>3}건 ({:.0}%)", c.obj_bbox, pct(c.obj_bbox));
    println!("  고정 크기 출력    {:>3}건 ({:.0}%)", c.fixed, pct(c.fixed));
    println!("  기타              {:>3}건 ({:.0}%)", c.other, pct(c.other));
    println!(
        "\n  훈련쌍 전부가 같은 부류인 과제: {}건 ({:.0}%) — 규칙이 일관된다는 뜻",
        consistent_task,
        pct(consistent_task)
    );
    println!(
        "  배수가 전 쌍에서 같은 값인 과제: {}건 — 상수 배수로 학습 가능",
        same_k_task
    );
    println!(
        "\n  ★ **선택 문제**(출력 = 입력 어느 객체의 bbox 잘라내기): 전 쌍 성립 {}건 ({:.0}%) · 쌍 단위 {}/{}",
        selection_task,
        pct(selection_task),
        selection_pairs,
        selection_total
    );
    println!("     → 성립하면 \"무엇이 답인가\"를 고르는 문제이고, 성질 규칙 기계를 선택에 재사용할 수 있다.");
    println!("\n▶ 배수·약수·추출이 대부분이면 작은 학습 어휘로 열린다.");
    println!("  기타가 대부분이면 이 슬라이스도 이 표현족 밖이다.");
}
