//! M2-R — **객체 델타 규칙**: 표현 경쟁의 승자 위에 세운 두 번째 규칙 계층.
//!
//! # 왜 이 표현인가 (시도 165의 계량이 정했다 — 사람이 고르지 않았다)
//!
//! 홀드아웃 바뀐 셀의 기술 가능성: 셀 국소 재작성 **17.4%** vs 객체 델타
//! (단색 4-연결) **72.9%**, 전량 기술 가능 과제 64/100. ARC 과제 간에 공유되는
//! 것은 리터럴 국소 패턴이 아니라 "가장 작은 것을 지워라" 같은 **객체 수준의
//! 추상 관계**다 — 그것만이 팔레트·배치가 다른 과제로 전이될 수 있다.
//!
//! # 규칙의 문법 (내용은 전부 경험이 채운다)
//!
//! `ORULE(OPROPS(p0..p11), OACT(kind, param))` — 조건은 객체의 성질 벡터,
//! 결과는 행동(재색/삭제). 무엇이 변수인지는 LGG가, 채택은 MDL이 정한다.
//! `param`이 성질 자리와 **변수를 공유**하면 "다수색이 된다" 같은 팔레트 독립
//! 규칙이 된다(시도 164에서 세운 바인딩 일관 의미론을 그대로 쓴다).
//!
//! 성질 12종은 전부 동결 기질의 객체 분해에서 기계적으로 나온다(해석 없음):
//! own color · size rank · unique-max/min · border touch · area log2 ·
//! object count · majority/rarest color · largest/smallest obj color ·
//! shape twin 유무 · color frequency rank.
//!
//! # v1의 정직한 범위
//!
//! 재색·삭제만 다룬다. 이동(param=(dx,dy))·출현(원본 객체 없음)은 다음 단계 —
//! 기술률 72.9% 중 재색·삭제가 차지하는 몫만 노린다.

use crate::grid::{components_bg, Grid, Obj};
use monad_core::abstraction::{generalize, Library, Provenance, Term};

const F_ORULE: u32 = 910;
const F_OPROPS: u32 = 911;
const F_OACT: u32 = 912;
/// 행동 종류.
const ACT_RECOLOR: u64 = 1;
const ACT_DELETE: u64 = 2;
const NPROPS: usize = 12;

/// 승자 표현(시도 165): 단색 4-연결, 배경 0.
fn decompose(g: &Grid) -> Vec<Obj> {
    components_bg(g, false, 0)
}

fn obj_color(o: &Obj) -> u8 {
    o.mask
        .iter()
        .zip(o.colors.iter())
        .find(|(m, _)| **m)
        .map(|(_, &c)| c)
        .unwrap_or(0)
}

fn shape_key(o: &Obj) -> (usize, usize, &Vec<bool>) {
    (o.w, o.h, &o.mask)
}

fn log2_bucket(v: usize) -> u64 {
    (usize::BITS - v.max(1).leading_zeros() - 1) as u64
}

/// 한 격자의 객체 성질 벡터들(객체별 12칸). 전부 분해에서 기계적으로 나온다.
pub fn object_props(g: &Grid, objs: &[Obj]) -> Vec<[u64; NPROPS]> {
    let n = objs.len();
    // 크기 순위
    let mut areas: Vec<usize> = objs.iter().map(|o| o.area).collect();
    areas.sort_unstable_by(|a, b| b.cmp(a));
    let max_a = areas.first().copied().unwrap_or(0);
    let min_a = areas.last().copied().unwrap_or(0);
    let max_unique = areas.iter().filter(|&&a| a == max_a).count() == 1;
    let min_unique = areas.iter().filter(|&&a| a == min_a).count() == 1;
    // 격자 다수색·희소색(배경 제외)
    let mut freq = [0usize; 10];
    for &c in &g.cells {
        if c != 0 && c <= 9 {
            freq[c as usize] += 1;
        }
    }
    let majority = (1..10).filter(|&c| freq[c] > 0).max_by_key(|&c| freq[c]).unwrap_or(0) as u64;
    let rarest = (1..10).filter(|&c| freq[c] > 0).min_by_key(|&c| freq[c]).unwrap_or(0) as u64;
    // 최대·최소 객체의 색
    let largest_c = objs.iter().max_by_key(|o| o.area).map(obj_color).unwrap_or(0) as u64;
    let smallest_c = objs.iter().min_by_key(|o| o.area).map(obj_color).unwrap_or(0) as u64;
    // 객체 색 빈도(객체 수 기준)
    let mut cfreq = [0usize; 10];
    for o in objs {
        cfreq[obj_color(o) as usize] += 1;
    }
    let cmax = cfreq.iter().copied().max().unwrap_or(0);
    let cmin = cfreq.iter().copied().filter(|&v| v > 0).min().unwrap_or(0);

    objs.iter()
        .map(|o| {
            let c = obj_color(o);
            let rank = if o.area == max_a {
                0
            } else if o.area == min_a {
                2
            } else {
                1
            };
            let twin = objs
                .iter()
                .filter(|p| !std::ptr::eq(*p, o))
                .any(|p| shape_key(p) == shape_key(o));
            let cf = cfreq[c as usize];
            let cfrank = if cf == cmax {
                0
            } else if cf == cmin {
                2
            } else {
                1
            };
            [
                c as u64,
                rank,
                (max_unique && o.area == max_a) as u64,
                (min_unique && o.area == min_a) as u64,
                (o.x0 == 0 || o.y0 == 0 || o.x0 + o.w == g.w || o.y0 + o.h == g.h) as u64,
                log2_bucket(o.area),
                (n.min(9)) as u64,
                majority,
                rarest,
                largest_c,
                smallest_c,
                (twin as u64) * 3 + cfrank, // twin(0/1)×3 + 색빈도순위(0..2) — 한 칸 절약
            ]
        })
        .collect()
}

fn rule_term(props: &[u64; NPROPS], kind: u64, param: u64) -> Term {
    Term::App(
        F_ORULE,
        vec![
            Term::App(F_OPROPS, props.iter().map(|&v| Term::Const(v)).collect()),
            Term::App(F_OACT, vec![Term::Const(kind), Term::Const(param)]),
        ],
    )
}

fn split_orule(t: &Term) -> Option<(&Vec<Term>, &Term, &Term)> {
    match t {
        Term::App(f, args) if *f == F_ORULE && args.len() == 2 => {
            match (&args[0], &args[1]) {
                (Term::App(pf, props), Term::App(af, act))
                    if *pf == F_OPROPS && props.len() == NPROPS && *af == F_OACT
                        && act.len() == 2 =>
                {
                    Some((props, &act[0], &act[1]))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// 입력↔출력 객체를 짝짓고 각 입력 객체의 실제 델타를 정한다.
/// None = 이 표현으로 완전 기술 불가(이동·출현·부분 변형 포함 과제).
///
/// 반환: 객체별 (stay=None | recolor(newc)=Some(c) | delete=Some(10)).
pub fn actual_deltas(i: &Grid, o: &Grid) -> Option<Vec<Option<u64>>> {
    if i.w != o.w || i.h != o.h {
        return None;
    }
    let oi = decompose(i);
    let oo = decompose(o);
    let mut used_o = vec![false; oo.len()];
    let mut deltas: Vec<Option<u64>> = vec![None; oi.len()];
    // 유지(완전 동일)
    for (a, ia) in oi.iter().enumerate() {
        for (b, ob) in oo.iter().enumerate() {
            if !used_o[b]
                && ia.x0 == ob.x0
                && ia.y0 == ob.y0
                && shape_key(ia) == shape_key(ob)
                && obj_color(ia) == obj_color(ob)
            {
                used_o[b] = true;
                deltas[a] = None;
                break;
            }
        }
    }
    let mut matched = vec![false; oi.len()];
    for (a, ia) in oi.iter().enumerate() {
        if deltas[a].is_none() {
            // 유지로 이미 짝지어졌는지 구별: 다시 검사
            let stayed = oo.iter().enumerate().any(|(b, ob)| {
                used_o[b]
                    && ia.x0 == ob.x0
                    && ia.y0 == ob.y0
                    && shape_key(ia) == shape_key(ob)
                    && obj_color(ia) == obj_color(ob)
            });
            if stayed {
                matched[a] = true;
            }
        }
    }
    // 재색(위치·모양 동일, 색만 다름)
    for (a, ia) in oi.iter().enumerate() {
        if matched[a] {
            continue;
        }
        for (b, ob) in oo.iter().enumerate() {
            if !used_o[b]
                && ia.x0 == ob.x0
                && ia.y0 == ob.y0
                && shape_key(ia) == shape_key(ob)
            {
                used_o[b] = true;
                matched[a] = true;
                deltas[a] = Some(obj_color(ob) as u64);
                break;
            }
        }
    }
    // 삭제(출력에서 그 자리가 전부 배경)
    for (a, ia) in oi.iter().enumerate() {
        if matched[a] {
            continue;
        }
        let all_bg = (0..ia.h)
            .flat_map(|dy| (0..ia.w).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| ia.mask[dy * ia.w + dx])
            .all(|(dx, dy)| o.get(ia.x0 + dx, ia.y0 + dy) == 0);
        if all_bg {
            matched[a] = true;
            deltas[a] = Some(10); // 10 = 삭제 표지(색 0..9와 구별)
        }
    }
    // 짝 없는 입력 객체(이동·부분 변형) 또는 짝 없는 출력 객체(출현·이동)가 남으면
    // 이 표현으로 완전 기술 불가 — v1은 정직하게 포기한다.
    if matched.iter().any(|m| !m) || used_o.iter().any(|u| !u) {
        return None;
    }
    Some(deltas)
}

/// 훈련쌍에서 객체 델타 경험을 뽑는다(재색·삭제만, 기계적).
pub fn extract_obj_rules(train: &[(Grid, Grid)]) -> Vec<Term> {
    let mut out = Vec::new();
    for (i, o) in train {
        let Some(deltas) = actual_deltas(i, o) else { continue };
        let objs = decompose(i);
        let props = object_props(i, &objs);
        for (a, d) in deltas.iter().enumerate() {
            match d {
                Some(10) => out.push(rule_term(&props[a], ACT_DELETE, 0)),
                Some(c) => out.push(rule_term(&props[a], ACT_RECOLOR, *c)),
                None => {}
            }
        }
    }
    out
}

/// 수면: 델타 경험을 일반화한다 — 행동별 그룹 + **전역 이웃쌍**(param이 성질
/// 자리와 변수를 공유해 팔레트 독립 규칙이 되는 경로). 채택은 MDL.
pub fn sleep_obj_abstract(rules: &[Term], lib: &mut Library) -> (usize, usize) {
    let (mut tried, mut added) = (0usize, 0usize);
    let mut ins = |terms: &[Term], tried: &mut usize, added: &mut usize| {
        *tried += 1;
        if let Some(a) = generalize(terms) {
            if lib.insert(&a, Provenance::MonadDerived) {
                *added += 1;
            }
        }
    };
    for w in rules.windows(2) {
        ins(w, &mut tried, &mut added);
    }
    // 같은 행동끼리 더 넓게 접기(3개 창 — 그룹 전체는 과일반화라 이웃 3개까지만)
    for w in rules.windows(3) {
        ins(w, &mut tried, &mut added);
    }
    (tried, added)
}

/// 바인딩 일관 발화(시도 164의 의미론을 객체 성질에 적용).
/// 반환: (행동 종류, 매개값).
fn orule_fire(
    props_cond: &[Term],
    kind: &Term,
    param: &Term,
    props: &[u64; NPROPS],
) -> Option<(u64, u64)> {
    let mut bind: Vec<(u32, u64)> = Vec::new();
    for (t, &v) in props_cond.iter().zip(props.iter()) {
        match t {
            Term::Const(c) if *c != v => return None,
            Term::Const(_) => {}
            Term::Var(i) => match bind.iter().find(|(b, _)| b == i) {
                Some((_, prev)) if *prev != v => return None,
                Some(_) => {}
                None => bind.push((*i, v)),
            },
            Term::App(_, _) => return None,
        }
    }
    // 행동 종류는 실행 가능해야 하므로 상수만
    let k = match kind {
        Term::Const(k) => *k,
        _ => return None,
    };
    let p = match param {
        Term::Const(p) => *p,
        Term::Var(v) => bind.iter().find(|(b, _)| b == v).map(|(_, val)| *val)?,
        Term::App(_, _) => return None,
    };
    Some((k, p))
}

/// **증거 기반 선택**: 이 과제의 모든 객체(유지 포함)에 대해 모순 없이 발화하는
/// 규칙만 채택한다. 유지 객체에서 변경 행동이 발화하면 모순이다.
pub fn select_obj_consistent(
    lib: &Library,
    train: &[(Grid, Grid)],
) -> Vec<(Vec<Term>, Term, Term)> {
    // 훈련쌍별 (성질, 실제 델타) — 하나라도 기술 불가면 빈 손
    let mut sites: Vec<([u64; NPROPS], Option<u64>)> = Vec::new();
    for (i, o) in train {
        let Some(deltas) = actual_deltas(i, o) else { return Vec::new() };
        let objs = decompose(i);
        let props = object_props(i, &objs);
        for (p, d) in props.into_iter().zip(deltas) {
            sites.push((p, d));
        }
    }
    let mut kept = Vec::new();
    for e in lib.by_prior() {
        let Some((cond, kind, param)) = split_orule(&lib.entries[e].schema) else { continue };
        let mut consistent = true;
        let mut useful = false;
        for (props, actual) in &sites {
            let Some((k, p)) = orule_fire(cond, kind, param, props) else { continue };
            let ok = match actual {
                None => k == ACT_RECOLOR && p == props[0], // 자기 색 재색 = 유지와 동치
                Some(10) => k == ACT_DELETE,
                Some(c) => k == ACT_RECOLOR && p == *c,
            };
            if !ok {
                consistent = false;
                break;
            }
            if actual.is_some() {
                useful = true;
            }
        }
        if consistent && useful {
            kept.push((cond.clone(), kind.clone(), param.clone()));
        }
    }
    kept
}

/// 선택 규칙 적용: 발화한 객체에 행동을 실행(재색/삭제), 나머지는 유지.
pub fn apply_obj_rules(rules: &[(Vec<Term>, Term, Term)], g: &Grid) -> Grid {
    let objs = decompose(g);
    let props = object_props(g, &objs);
    let mut out = g.clone();
    for (o, p) in objs.iter().zip(props.iter()) {
        for (cond, kind, param) in rules {
            let Some((k, val)) = orule_fire(cond, kind, param, p) else { continue };
            let paint = match k {
                ACT_DELETE => 0u8,
                ACT_RECOLOR if val <= 9 => val as u8,
                _ => continue,
            };
            for dy in 0..o.h {
                for dx in 0..o.w {
                    if o.mask[dy * o.w + dx] {
                        out.set(o.x0 + dx, o.y0 + dy, paint);
                    }
                }
            }
            break;
        }
    }
    out
}

/// 선택 규칙이 훈련쌍을 완전히 재현하는가(전이 게이트).
pub fn obj_rules_reproduce(rules: &[(Vec<Term>, Term, Term)], train: &[(Grid, Grid)]) -> bool {
    !rules.is_empty()
        && train
            .iter()
            .all(|(i, o)| i.w == o.w && i.h == o.h && &apply_obj_rules(rules, i) == o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(g: &mut Grid, x0: usize, y0: usize, w: usize, h: usize, c: u8) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                g.set(x, y, c);
            }
        }
    }

    /// 추출은 바뀐 객체만, 재색·삭제로 분류한다(해석 없음).
    #[test]
    fn extraction_classifies_recolor_and_delete() {
        let mut i = Grid::new(8, 8);
        place(&mut i, 0, 0, 3, 3, 4); // 큰 것
        place(&mut i, 5, 5, 1, 1, 7); // 작은 것
        let mut o = i.clone();
        place(&mut o, 0, 0, 3, 3, 2); // 재색
        place(&mut o, 5, 5, 1, 1, 0); // 삭제
        let r = extract_obj_rules(&[(i, o)]);
        assert_eq!(r.len(), 2, "재색 1 + 삭제 1이어야 한다");
        let kinds: Vec<u64> = r
            .iter()
            .filter_map(|t| split_orule(t).map(|(_, k, _)| match k {
                Term::Const(v) => *v,
                _ => 99,
            }))
            .collect();
        assert!(kinds.contains(&ACT_RECOLOR) && kinds.contains(&ACT_DELETE));
    }

    /// **과제 간 전이**: "가장 작은 객체를 지워라"를 배치·색이 다른 두 과제에서
    /// 경험 → 본 적 없는 세 번째 과제의 훈련쌍을 재현하고 시험까지 푼다.
    /// 리터럴이 아니라 성질(최소 크기)이 조건이 됐다는 뜻이다.
    #[test]
    fn delete_smallest_transfers_across_tasks() {
        let mk = |big: (usize, usize, u8), small: (usize, usize, u8)| {
            let mut i = Grid::new(9, 9);
            place(&mut i, big.0, big.1, 3, 3, big.2);
            place(&mut i, small.0, small.1, 1, 1, small.2);
            let mut o = i.clone();
            place(&mut o, small.0, small.1, 1, 1, 0);
            (i, o)
        };
        // 과제 A·B: 다른 배치·다른 색
        let mut rules = extract_obj_rules(&[mk((0, 0, 3), (7, 7, 5))]);
        rules.extend(extract_obj_rules(&[mk((5, 2, 6), (1, 6, 8))]));
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        assert!(!lib.entries.is_empty(), "수면이 규칙을 만들지 못했다");

        // 과제 C: 또 다른 배치·색 — 본 적 없는 조합
        // (경험한 작은 객체들과 같은 성질 부류: 비테두리 — 성질이 조건이 됐으므로
        //  테두리 접촉 여부가 다르면 발화하지 않는 것이 올바른 동작이다)
        let (ci, co) = mk((2, 4, 9), (7, 1, 1));
        let train = [(ci.clone(), co.clone())];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty(), "일관 규칙을 못 골랐다");
        assert!(obj_rules_reproduce(&sel, &train), "훈련 재현 실패");
        // 시험쌍(같은 규칙, 또 다른 배치)
        let (ti, to) = mk((3, 3, 2), (1, 7, 4));
        assert_eq!(apply_obj_rules(&sel, &ti), to, "시험 실패");
    }

    /// **팔레트 독립 재색**: "전부 다수색이 된다"를 서로 다른 팔레트의 두 과제에서
    /// 경험 → LGG가 param과 다수색 성질 자리를 같은 변수로 접는다 → 세 번째
    /// 팔레트에서 재현. 상수 param으로는 원리상 불가능한 전이다.
    #[test]
    fn recolor_to_majority_is_palette_independent() {
        let mk = |maj: u8, minor: u8| {
            let mut i = Grid::new(9, 5);
            place(&mut i, 0, 0, 4, 4, maj); // 다수색 덩어리
            place(&mut i, 6, 1, 1, 1, minor);
            place(&mut i, 6, 3, 1, 1, minor);
            let mut o = i.clone();
            place(&mut o, 6, 1, 1, 1, maj);
            place(&mut o, 6, 3, 1, 1, maj);
            (i, o)
        };
        let mut rules = extract_obj_rules(&[mk(3, 5)]);
        rules.extend(extract_obj_rules(&[mk(6, 2)]));
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        // param이 변수인 규칙이 실제로 생겼는가
        let has_var_param = lib.entries.iter().any(|e| {
            split_orule(&e.schema)
                .map(|(_, _, p)| matches!(p, Term::Var(_)))
                .unwrap_or(false)
        });
        assert!(has_var_param, "param 변수 규칙이 수면에서 나오지 않았다");

        let (ci, co) = mk(9, 1); // 본 적 없는 팔레트
        let train = [(ci, co)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty() && obj_rules_reproduce(&sel, &train), "팔레트 독립 전이 실패");
    }

    /// 맞지 않는 과제에서는 게이트가 막는다(거짓 양성 방지).
    #[test]
    fn gate_rejects_unrelated_task() {
        let mut i = Grid::new(6, 6);
        place(&mut i, 0, 0, 2, 2, 3);
        place(&mut i, 4, 4, 1, 1, 5);
        let mut o = i.clone();
        place(&mut o, 4, 4, 1, 1, 0);
        let rules = {
            let mut r = extract_obj_rules(&[(i.clone(), o.clone())]);
            r.extend(r.clone());
            r
        };
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        // 전혀 다른 변환: 큰 것을 재색(작은 것 유지)
        let mut o2 = i.clone();
        place(&mut o2, 0, 0, 2, 2, 7);
        let train = [(i, o2)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!obj_rules_reproduce(&sel, &train), "맞지 않는데 재현을 통과시켰다");
    }
}
