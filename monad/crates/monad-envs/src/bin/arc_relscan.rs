//! M2-R 진단 — **일차 관계의 판별력 계량**(집계 전용, 시도 185).
//!
//! 여덟 번의 독립 개입이 모두 부재+필터 ≈ 147을 남겼다(시도 184). 남은 가설은
//! 하나다: 필요한 정보가 **특정 객체 쌍 사이의 관계**라서 속성 벡터에 담기지
//! 않는다는 것. 그러나 그것도 가설이므로 **만들기 전에 잰다**.
//!
//! 기준: 모호쌍(속성 벡터가 같은데 행동이 다른 두 객체)에 대해
//!
//! > **어떤 객체 O가 존재해 관계 R(self,O)의 값이 두 객체에서 다른가**
//!
//! 를 관계별로 센다. 존재 양화가 핵심이다 — 속성 벡터는 집계값만 담을 수 있고
//! "어느 특정 객체와의 관계"를 담지 못한다. 그것이 실제 판별 정보인지 여기서
//! 확정한다.
//!
//! 실행: `arc-relscan`

use monad_envs::arc_data::load_dir;
use monad_envs::arc_objrule::{actual_deltas, object_props};
use monad_envs::grid::{components_bg, Grid, Obj};

fn obj_color(o: &Obj) -> u8 {
    o.mask
        .iter()
        .zip(o.colors.iter())
        .find(|(m, _)| **m)
        .map(|(_, &c)| c)
        .unwrap_or(0)
}

fn shape_eq(a: &Obj, b: &Obj) -> bool {
    a.w == b.w && a.h == b.h && a.mask == b.mask
}

fn inside(a: &Obj, b: &Obj) -> bool {
    // a의 bbox가 b의 bbox 안에 완전히 들어간다
    a.x0 >= b.x0 && a.y0 >= b.y0 && a.x0 + a.w <= b.x0 + b.w && a.y0 + a.h <= b.y0 + b.h
}

fn adjacent(a: &Obj, b: &Obj) -> bool {
    a.x0 <= b.x0 + b.w && b.x0 <= a.x0 + a.w && a.y0 <= b.y0 + b.h && b.y0 <= a.y0 + a.h
}

fn row_overlap(a: &Obj, b: &Obj) -> bool {
    a.y0 < b.y0 + b.h && b.y0 < a.y0 + a.h
}

fn col_overlap(a: &Obj, b: &Obj) -> bool {
    a.x0 < b.x0 + b.w && b.x0 < a.x0 + a.w
}

/// 관계 목록: (이름, 두 객체 사이의 참/거짓).
fn relations() -> Vec<(&'static str, fn(&Obj, &Obj) -> bool)> {
    vec![
        ("same_color_as", |a: &Obj, b: &Obj| obj_color(a) == obj_color(b)),
        ("same_shape_as", |a: &Obj, b: &Obj| shape_eq(a, b)),
        ("inside_of", |a: &Obj, b: &Obj| inside(a, b) && !shape_eq(a, b)),
        ("contains", |a: &Obj, b: &Obj| inside(b, a) && !shape_eq(a, b)),
        ("adjacent_to", adjacent as fn(&Obj, &Obj) -> bool),
        ("same_row_as", row_overlap as fn(&Obj, &Obj) -> bool),
        ("same_col_as", col_overlap as fn(&Obj, &Obj) -> bool),
        ("bigger_than", |a: &Obj, b: &Obj| a.area > b.area),
        ("same_size_as", |a: &Obj, b: &Obj| a.area == b.area),
        ("left_of", |a: &Obj, b: &Obj| a.x0 + a.w <= b.x0),
        ("above", |a: &Obj, b: &Obj| a.y0 + a.h <= b.y0),
    ]
}

/// 이 객체가 관계 R로 맺어지는 상대들의 **성질 서명 집합**.
/// 존재 양화의 근사 — "어떤 O가 있어 R(self,O)이고 O가 이러이러하다".
fn rel_signature(
    objs: &[Obj],
    ix: usize,
    rel: fn(&Obj, &Obj) -> bool,
    props: &[[u64; 14]],
) -> Vec<u64> {
    let mut sig: Vec<u64> = objs
        .iter()
        .enumerate()
        .filter(|(j, o)| *j != ix && rel(&objs[ix], o))
        // 상대의 성질 벡터를 하나의 해시로 요약(어떤 종류의 상대인가)
        .map(|(j, _)| {
            let mut h = 0xcbf29ce484222325u64;
            for &v in &props[j] {
                h = h.wrapping_mul(0x100000001b3) ^ v;
            }
            h
        })
        .collect();
    sig.sort_unstable();
    sig.dedup();
    sig
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
    println!("M2-R 일차 관계 판별력 — 모호쌍을 특정 객체와의 관계가 가르는가 (집계 전용)");
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

    let rels = relations();
    let mut split: Vec<usize> = vec![0; rels.len()];
    let mut any_split = 0usize;
    let mut total_pairs = 0usize;

    for task in &holdout {
        let train: Vec<(Grid, Grid)> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let deltas: Option<Vec<_>> = train.iter().map(|(i, o)| actual_deltas(i, o)).collect();
        let Some(deltas) = deltas else { continue };

        // 지점: (성질, 행동, 격자 색인, 객체 색인)
        let mut sites: Vec<([u64; 14], Option<u64>, usize, usize)> = Vec::new();
        let mut grids: Vec<(Vec<Obj>, Vec<[u64; 14]>)> = Vec::new();
        for (gi, ((i, _), d)) in train.iter().zip(deltas.iter()).enumerate() {
            let objs = components_bg(i, false, 0);
            let props = object_props(i, &objs);
            for (ix, (p, dv)) in props.iter().zip(d.iter()).enumerate() {
                sites.push((*p, *dv, gi, ix));
            }
            grids.push((objs, props));
        }
        // 모호쌍
        for a in 0..sites.len() {
            for b in a + 1..sites.len() {
                if sites[a].0 != sites[b].0 || sites[a].1 == sites[b].1 {
                    continue;
                }
                total_pairs += 1;
                let (ga, ia) = (sites[a].2, sites[a].3);
                let (gb, ib) = (sites[b].2, sites[b].3);
                let mut split_here = false;
                for (k, (_, r)) in rels.iter().enumerate() {
                    let sa = rel_signature(&grids[ga].0, ia, *r, &grids[ga].1);
                    let sb = rel_signature(&grids[gb].0, ib, *r, &grids[gb].1);
                    if sa != sb {
                        split[k] += 1;
                        split_here = true;
                    }
                }
                if split_here {
                    any_split += 1;
                }
            }
        }
    }

    if total_pairs == 0 {
        println!("모호쌍이 없다.");
        return;
    }
    println!("홀드아웃 {}건 · **모호쌍 {}개**\n", holdout.len(), total_pairs);
    let mut ranked: Vec<(usize, &'static str)> =
        split.iter().copied().zip(rels.iter().map(|(n, _)| *n)).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    println!("관계별 **모호쌍 분리율**(존재 양화 — 어떤 객체 O가 있어 R(self,O)이 다르다):");
    for (c, n) in &ranked {
        let pct = 100.0 * *c as f64 / total_pairs as f64;
        let bar: String = "█".repeat((pct / 4.0).round() as usize);
        println!("  {n:<16} {pct:5.1}%  {bar}");
    }
    println!(
        "\n  ★ **하나라도 가르는 모호쌍**: {}/{} = **{:.1}%**",
        any_split,
        total_pairs,
        100.0 * any_split as f64 / total_pairs as f64
    );
    println!("\n▶ 판정 기준: 이 값이 높으면 일차 관계 학습기를 만들 근거가 있다.");
    println!("  낮으면 천장이 더 깊다는 뜻이므로 그대로 기록한다(만들지 않는다).");
}
