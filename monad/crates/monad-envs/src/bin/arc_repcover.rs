//! M2-R 진단 — **표현 후보별 기술 가능성 상한**(집계 전용).
//!
//! 8단계(표현 복수 가설)의 측정 우선 진입점. 기계를 만들기 전에, 각 표현 후보가
//! 홀드아웃의 변화를 **원리상 몇 %나 기술할 수 있는지**부터 잰다 — 셀 국소
//! 재작성은 이미 ~17%로 측정됐다(시도 162~164). 여기서는 **객체 수준 델타**
//! (유지/재색/이동/삭제/출현)로 기술되는 비율을 분해 4종에 대해 잰다:
//!
//! - 4-연결 단색 · 8-연결 단색 · 4-연결 복합색 · 8-연결 복합색
//!
//! 분해기는 전부 동결 기질이 이미 가진 것(`grid.rs`)이다 — 새 어휘를 만들지
//! 않는다. 어느 표현이 좋은지는 이 계량이 말하고, 실제 선택은 이후
//! `representation.rs`의 공통 점수가 한다(사람이 고르지 않는다).
//!
//! 과제 내용은 출력하지 않는다(봉인 규율). 실행: `arc-repcover`

use monad_envs::arc_data::load_dir;
use monad_envs::grid::{components_bg, components_multi, Grid, Obj};

/// 객체의 모양 지문: (w, h, 마스크) — 위치·색 무관.
fn shape_key(o: &Obj) -> (usize, usize, Vec<bool>) {
    (o.w, o.h, o.mask.clone())
}

/// 객체의 색 패턴(마스크 자리만).
fn color_key(o: &Obj) -> Vec<u8> {
    o.mask
        .iter()
        .zip(o.colors.iter())
        .filter(|(m, _)| **m)
        .map(|(_, &c)| c)
        .collect()
}

/// 한 훈련쌍에서, 이 분해로 객체 델타(유지/재색/이동/삭제/출현)가 기술하는
/// 바뀐 셀 수를 센다. 반환: (기술된 바뀐 셀, 전체 바뀐 셀).
fn delta_cover(i: &Grid, o: &Grid, decomp: &dyn Fn(&Grid) -> Vec<Obj>) -> (usize, usize) {
    let total: usize = (0..o.h)
        .flat_map(|y| (0..o.w).map(move |x| (x, y)))
        .filter(|&(x, y)| i.get(x, y) != o.get(x, y))
        .count();
    if total == 0 {
        return (0, 0);
    }
    let oi = decomp(i);
    let oo = decomp(o);
    let mut used_o = vec![false; oo.len()];
    let mut covered = vec![vec![false; o.w]; o.h];
    let mut mark = |obj: &Obj, cov: &mut Vec<Vec<bool>>| {
        for dy in 0..obj.h {
            for dx in 0..obj.w {
                if obj.mask[dy * obj.w + dx] {
                    cov[obj.y0 + dy][obj.x0 + dx] = true;
                }
            }
        }
    };

    // 1) 유지(동일 위치·모양·색): 델타 없음 — 소거만
    let mut used_i = vec![false; oi.len()];
    for (a, ia) in oi.iter().enumerate() {
        for (b, ob) in oo.iter().enumerate() {
            if used_o[b] {
                continue;
            }
            if ia.x0 == ob.x0
                && ia.y0 == ob.y0
                && shape_key(ia) == shape_key(ob)
                && color_key(ia) == color_key(ob)
            {
                used_i[a] = true;
                used_o[b] = true;
                break;
            }
        }
    }
    // 2) 재색(동일 위치·모양, 색만 다름): 그 마스크의 바뀐 셀을 기술
    for (a, ia) in oi.iter().enumerate() {
        if used_i[a] {
            continue;
        }
        for (b, ob) in oo.iter().enumerate() {
            if used_o[b] {
                continue;
            }
            if ia.x0 == ob.x0 && ia.y0 == ob.y0 && shape_key(ia) == shape_key(ob) {
                used_i[a] = true;
                used_o[b] = true;
                mark(ob, &mut covered);
                break;
            }
        }
    }
    // 3) 이동(모양·색 동일, 위치 다름): 옛 자리(배경화)와 새 자리를 기술
    for (a, ia) in oi.iter().enumerate() {
        if used_i[a] {
            continue;
        }
        for (b, ob) in oo.iter().enumerate() {
            if used_o[b] {
                continue;
            }
            if shape_key(ia) == shape_key(ob) && color_key(ia) == color_key(ob) {
                used_i[a] = true;
                used_o[b] = true;
                mark(ia, &mut covered);
                mark(ob, &mut covered);
                break;
            }
        }
    }
    // 4) 삭제: 짝 없는 입력 객체의 자리가 출력에서 전부 배경이면 기술
    for (a, ia) in oi.iter().enumerate() {
        if used_i[a] {
            continue;
        }
        let all_bg = (0..ia.h)
            .flat_map(|dy| (0..ia.w).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| ia.mask[dy * ia.w + dx])
            .all(|(dx, dy)| o.get(ia.x0 + dx, ia.y0 + dy) == 0);
        if all_bg {
            mark(ia, &mut covered);
        }
    }
    // 5) 출현: 짝 없는 출력 객체의 자리가 입력에서 전부 배경이면 기술
    for (b, ob) in oo.iter().enumerate() {
        if used_o[b] {
            continue;
        }
        let was_bg = (0..ob.h)
            .flat_map(|dy| (0..ob.w).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| ob.mask[dy * ob.w + dx])
            .all(|(dx, dy)| i.get(ob.x0 + dx, ob.y0 + dy) == 0);
        if was_bg {
            mark(ob, &mut covered);
        }
    }

    let described = (0..o.h)
        .flat_map(|y| (0..o.w).map(move |x| (x, y)))
        .filter(|&(x, y)| i.get(x, y) != o.get(x, y) && covered[y][x])
        .count();
    (described, total)
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let src_path = std::env::var("MONAD_ARC_PATCHSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-patchsource.txt".into()
    });
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());

    println!("=========================================================================");
    println!("M2-R 표현 후보별 기술 가능성 — 객체 델타(유지/재색/이동/삭제/출현), 집계 전용");
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

    let reps: Vec<(&str, Box<dyn Fn(&Grid) -> Vec<Obj>>)> = vec![
        ("단색 4-연결", Box::new(|g: &Grid| components_bg(g, false, 0))),
        ("단색 8-연결", Box::new(|g: &Grid| components_bg(g, true, 0))),
        ("복합색 4-연결", Box::new(|g: &Grid| components_multi(g, false, 0))),
        ("복합색 8-연결", Box::new(|g: &Grid| components_multi(g, true, 0))),
    ];

    println!("홀드아웃 {}건 · 동일 크기 쌍만\n", holdout.len());
    println!("  (비교 기준: 셀 국소 재작성 표현의 깨끗한 덮개 = **17.4%**, 시도 164)\n");

    for (name, decomp) in &reps {
        let mut described = 0usize;
        let mut total = 0usize;
        let mut n_tasks = 0usize;
        let mut full = 0usize;
        for task in &holdout {
            let mut t_desc = 0usize;
            let mut t_total = 0usize;
            let mut usable = false;
            for p in &task.train {
                if p.input.w != p.output.w || p.input.h != p.output.h {
                    continue;
                }
                let (d, t) = delta_cover(&p.input, &p.output, decomp.as_ref());
                if t > 0 {
                    usable = true;
                    t_desc += d;
                    t_total += t;
                }
            }
            if usable {
                n_tasks += 1;
                described += t_desc;
                total += t_total;
                if t_desc == t_total {
                    full += 1;
                }
            }
        }
        println!(
            "  {name:14} 기술률 {:.1}% ({described}/{total}) · 전체 기술 과제 {}건/{n_tasks}",
            100.0 * described as f64 / total.max(1) as f64,
            full
        );
    }
    println!("\n▶ 이 표가 표현 경쟁의 출발 순위다 — 선택은 representation.rs 점수가 한다.");
}
