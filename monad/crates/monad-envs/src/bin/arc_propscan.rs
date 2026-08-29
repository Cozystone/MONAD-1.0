//! M2-R 진단 — **성질 후보의 정보량 계량**(집계 전용, 시도 181).
//!
//! 시도 170에서 성질 6종을 추측으로 늘렸다가 전이가 죽었다(④ 2→0). 그러므로
//! 이번에는 **만들기 전에 잰다**.
//!
//! 기준은 하나: 현 성질 12종으로 **구분되지 않는데 행동이 다른 객체쌍**
//! (모호쌍)을 후보 성질이 몇 개나 가르는가. 가르지 못하면 그 성질은 정보가
//! 없고, 늘리면 과잉 구체화만 부른다.
//!
//! 후보는 전부 **관계적**이다 — 현 12종은 내재적(색·크기·모양)이거나
//! 전역적(다수색·객체 수)이고, ARC 규칙이 실제로 쓰는 관계(포함·인접·정렬·
//! 최근접·유일성)가 통째로 빠져 있다. 전부 동결 기질의 분해에서 기계적으로
//! 계산되며, 어느 것을 채택할지는 이 계량이 정한다(사람이 고르지 않는다).
//!
//! 실행: `arc-propscan`

use monad_envs::arc_data::load_dir;
use monad_envs::arc_objrule::{actual_deltas, object_props, NPROPS};
use monad_envs::grid::{components_bg, Grid, Obj};

fn obj_color(o: &Obj) -> u8 {
    o.mask
        .iter()
        .zip(o.colors.iter())
        .find(|(m, _)| **m)
        .map(|(_, &c)| c)
        .unwrap_or(0)
}

fn bbox_contains(a: &Obj, b: &Obj) -> bool {
    a.x0 <= b.x0 && a.y0 <= b.y0 && a.x0 + a.w >= b.x0 + b.w && a.y0 + a.h >= b.y0 + b.h
}

fn touches(a: &Obj, b: &Obj) -> bool {
    // bbox 팽창 1칸으로 인접 판정(대각 포함)
    let (ax1, ay1) = (a.x0 + a.w, a.y0 + a.h);
    let (bx1, by1) = (b.x0 + b.w, b.y0 + b.h);
    a.x0 <= bx1 && b.x0 <= ax1 && a.y0 <= by1 && b.y0 <= ay1
}

fn center(o: &Obj) -> (i64, i64) {
    ((2 * o.x0 + o.w) as i64, (2 * o.y0 + o.h) as i64)
}

/// 후보 관계 성질 — 이름과 값 계산기.
fn candidate_props(g: &Grid, objs: &[Obj], ix: usize) -> Vec<(&'static str, u64)> {
    let o = &objs[ix];
    let n = objs.len();
    let others = || objs.iter().enumerate().filter(move |(j, _)| *j != ix).map(|(_, p)| p);

    // 포함 관계
    let contains_other = others().any(|p| bbox_contains(o, p)) as u64;
    let contained_by = others().any(|p| bbox_contains(p, o)) as u64;
    // 인접
    let touch_count = others().filter(|p| touches(o, p)).count().min(4) as u64;
    // 정렬(같은 행/열 띠에 다른 객체가 있는가)
    let row_aligned = others()
        .any(|p| o.y0 < p.y0 + p.h && p.y0 < o.y0 + o.h) as u64;
    let col_aligned = others()
        .any(|p| o.x0 < p.x0 + p.w && p.x0 < o.x0 + o.w) as u64;
    // 모양 유일성 / 색 유일성
    let shape_unique = !others().any(|p| (p.w, p.h, &p.mask) == (o.w, o.h, &o.mask)) as u64;
    let color_unique = !others().any(|p| obj_color(p) == obj_color(o)) as u64;
    // 최근접 거리 순위(0=가장 가까운 이웃을 가짐 … 2=가장 먼)
    let my_nn = others()
        .map(|p| {
            let (cx, cy) = center(o);
            let (px, py) = center(p);
            (cx - px).abs() + (cy - py).abs()
        })
        .min()
        .unwrap_or(0);
    let mut all_nn: Vec<i64> = (0..n)
        .map(|a| {
            objs.iter()
                .enumerate()
                .filter(|(b, _)| *b != a)
                .map(|(_, p)| {
                    let (cx, cy) = center(&objs[a]);
                    let (px, py) = center(p);
                    (cx - px).abs() + (cy - py).abs()
                })
                .min()
                .unwrap_or(0)
        })
        .collect();
    all_nn.sort_unstable();
    let nn_rank = if all_nn.first() == Some(&my_nn) {
        0
    } else if all_nn.last() == Some(&my_nn) {
        2
    } else {
        1
    };
    // 격자 중심 대비 사분면(위치의 거친 부호)
    let (cx, cy) = center(o);
    let quadrant = ((cx > g.w as i64) as u64) * 2 + (cy > g.h as i64) as u64;
    // 자기 색 객체의 개수(같은 색이 몇 개인가)
    let same_color_count = objs
        .iter()
        .filter(|p| obj_color(p) == obj_color(o))
        .count()
        .min(4) as u64;
    // 자기 모양 객체의 개수
    let same_shape_count = objs
        .iter()
        .filter(|p| (p.w, p.h, &p.mask) == (o.w, o.h, &o.mask))
        .count()
        .min(4) as u64;

    vec![
        ("contains_other", contains_other),
        ("contained_by", contained_by),
        ("touch_count", touch_count),
        ("row_aligned", row_aligned),
        ("col_aligned", col_aligned),
        ("shape_unique", shape_unique),
        ("color_unique", color_unique),
        ("nn_rank", nn_rank),
        ("quadrant", quadrant),
        ("same_color_count", same_color_count),
        ("same_shape_count", same_shape_count),
    ]
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let src_path = std::env::var("MONAD_ARC_OBJSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objsource.txt".into()
    });
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());

    println!("=========================================================================");
    println!("M2-R 성질 후보 정보량 — 모호쌍을 가르는가 (집계 전용, 만들기 전에 잰다)");
    println!("=========================================================================");

    let sources: Vec<String> = std::fs::read_to_string(&src_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let solved: Vec<String> = std::fs::read_to_string(&solved_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let tasks = load_dir(std::path::Path::new(&dir));
    let holdout: Vec<_> = tasks
        .into_iter()
        .filter(|t| !sources.contains(&t.name) && !solved.contains(&t.name))
        .collect();

    // 모호쌍 수집: 현 12종 성질이 같은데 행동이 다른 (객체, 객체) 쌍
    let mut names: Vec<&'static str> = Vec::new();
    let mut split_counts: Vec<usize> = Vec::new();
    let mut total_pairs = 0usize;
    let mut tasks_used = 0usize;

    for task in &holdout {
        let train: Vec<(Grid, Grid)> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        // 완전 기술되는 과제에서만(델타가 확정돼야 행동 비교가 의미 있다)
        let deltas: Option<Vec<_>> = train.iter().map(|(i, o)| actual_deltas(i, o)).collect();
        let Some(deltas) = deltas else { continue };
        tasks_used += 1;

        // 지점 수집: (12종 성질, 행동, 후보 성질들)
        let mut sites: Vec<([u64; NPROPS], Option<u64>, Vec<u64>)> = Vec::new();
        for ((i, _), d) in train.iter().zip(deltas.iter()) {
            let objs = components_bg(i, false, 0);
            let props = object_props(i, &objs);
            for (ix, (p, dv)) in props.into_iter().zip(d.iter()).enumerate() {
                let cands = candidate_props(i, &objs, ix);
                if names.is_empty() {
                    names = cands.iter().map(|(n, _)| *n).collect();
                    split_counts = vec![0; names.len()];
                }
                sites.push((p, *dv, cands.into_iter().map(|(_, v)| v).collect()));
            }
        }
        // 모호쌍마다 각 후보가 가르는지 센다
        for a in 0..sites.len() {
            for b in a + 1..sites.len() {
                if sites[a].0 != sites[b].0 || sites[a].1 == sites[b].1 {
                    continue;
                }
                total_pairs += 1;
                for (k, sc) in split_counts.iter_mut().enumerate() {
                    if sites[a].2[k] != sites[b].2[k] {
                        *sc += 1;
                    }
                }
            }
        }
    }

    println!("홀드아웃 {}건 중 완전 기술 {}건 · **모호쌍 {}개**\n", holdout.len(), tasks_used, total_pairs);
    if total_pairs == 0 {
        println!("모호쌍이 없다 — 성질 확장의 근거가 없음.");
        return;
    }
    let mut ranked: Vec<(usize, &'static str)> = split_counts
        .iter()
        .copied()
        .zip(names.iter().copied())
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    println!("후보 성질별 **모호쌍 분리율**(높을수록 정보가 많다):");
    for (c, n) in &ranked {
        let pct = 100.0 * *c as f64 / total_pairs as f64;
        let bar: String = "█".repeat((pct / 4.0).round() as usize);
        println!("  {n:<18} {pct:5.1}%  {bar}");
    }
    println!("\n▶ 채택 기준: 분리율이 높은 소수만. 낮은 것을 넣으면 과잉 구체화로");
    println!("  전이가 죽는다(시도 170에서 실측: 성질 18종 → ④ 2→0).");
}
