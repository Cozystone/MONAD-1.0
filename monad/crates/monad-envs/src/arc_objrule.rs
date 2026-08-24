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
use std::collections::HashMap;

const F_ORULE: u32 = 910;
const F_OPROPS: u32 = 911;
const F_OACT: u32 = 912;
/// 행동 종류.
const ACT_RECOLOR: u64 = 1;
const ACT_DELETE: u64 = 2;
/// v2(시도 169): 이동을 1급 델타로. param = 인코딩된 (dx,dy).
const ACT_MOVE: u64 = 3;
/// 시도 170~171의 계량 판정: 18종 확장(x/y 순위·모양 클래스·구멍·색 유일·비율)은
/// 모호쌍을 428→164로 줄였지만 **홀드아웃 전이를 2→0으로 죽였다** — 성질 수에
/// 비례해 쌍 LGG에서 우연히 상수로 굳는 슬롯이 늘기 때문(과잉 구체화). 스키마
/// 정련 1라운드로도 회복 불가. **④를 실증한 12종을 유지**하고, 확장은 일반화
/// 사다리가 여러 라운드로 강해진 뒤 재시도한다(측정으로 기각, 추측 아님).
pub const NPROPS: usize = 12;

/// 이동 벡터 인코딩(격자 ≤30이므로 ±30이면 충분). 델타 표기와 규칙 param 공용.
const MOVE_BASE: u64 = 1000;
fn encode_move(dx: i64, dy: i64) -> u64 {
    MOVE_BASE + ((dx + 30) as u64) * 61 + ((dy + 30) as u64)
}
fn decode_move(v: u64) -> Option<(i64, i64)> {
    if v < MOVE_BASE {
        return None;
    }
    let r = v - MOVE_BASE;
    let dx = (r / 61) as i64 - 30;
    let dy = (r % 61) as i64 - 30;
    (dx.abs() <= 30 && dy.abs() <= 30).then_some((dx, dy))
}

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

/// 입력↔출력 객체를 짝짓는 공통 코어. 반환: (객체별 델타, 완전 기술 여부).
///
/// 델타: stay=None | recolor(newc)=Some(c) | delete=Some(10). `complete`가 거짓이면
/// 짝 없는 객체(이동·출현·부분 변형)가 남아 있다 — **확정된 재색·삭제 델타는
/// 그래도 유효하다**(부분 추출의 근거).
fn match_deltas(i: &Grid, o: &Grid) -> (Vec<Option<u64>>, Vec<bool>, bool) {
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
        // **이동 매칭**(v2, 시도 169): 같은 모양·색의 짝 없는 출력 객체가 다른
        // 자리에 있으면 이동이다 — v1에서는 삭제 오분류만 막았지만(가드), 이제
        // 1급 델타로 승격한다. 델타 = 인코딩된 (dx,dy). 후보가 여럿이면 가장
        // 가까운 것(맨해튼 거리)을 짝으로 — 결정론적.
        let mv = oo
            .iter()
            .enumerate()
            .filter(|(b, ob)| {
                !used_o[*b] && shape_key(ia) == shape_key(ob) && obj_color(ia) == obj_color(ob)
            })
            .min_by_key(|(_, ob)| {
                (ob.x0 as i64 - ia.x0 as i64).abs() + (ob.y0 as i64 - ia.y0 as i64).abs()
            })
            .map(|(b, ob)| (b, ob.x0 as i64 - ia.x0 as i64, ob.y0 as i64 - ia.y0 as i64));
        if let Some((b, dx, dy)) = mv {
            used_o[b] = true;
            matched[a] = true;
            deltas[a] = Some(encode_move(dx, dy));
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
    let complete = matched.iter().all(|m| *m) && used_o.iter().all(|u| *u);
    (deltas, matched, complete)
}

/// 완전 기술 판정(선택·게이트용): 이동·출현이 섞이면 None — 전이 판정의
/// 엄격함은 유지한다.
pub fn actual_deltas(i: &Grid, o: &Grid) -> Option<Vec<Option<u64>>> {
    if i.w != o.w || i.h != o.h {
        return None;
    }
    let (deltas, _, complete) = match_deltas(i, o);
    if complete {
        Some(deltas)
    } else {
        None
    }
}

/// 훈련쌍에서 객체 델타 경험을 뽑는다(재색·삭제만, 기계적).
///
/// **부분 추출**(시도 167): 이동·출현이 섞인 쌍이라도 짝이 확정된 재색·삭제
/// 델타는 경험으로 남긴다. 완전 기술 요구는 추출이 아니라 **선택·게이트**의
/// 몫이다 — 교사가 버리던 정보를 회수하자던 원칙(시도 151)을 추출기가 다시
/// 어기고 있었다(200과제 중 10개만 경험 생산의 원인).
pub fn extract_obj_rules(train: &[(Grid, Grid)]) -> Vec<Term> {
    let mut out = Vec::new();
    for (i, o) in train {
        if i.w != o.w || i.h != o.h {
            continue;
        }
        let (deltas, matched, _complete) = match_deltas(i, o);
        let objs = decompose(i);
        let props = object_props(i, &objs);
        for (a, d) in deltas.iter().enumerate() {
            if !matched[a] {
                continue; // 짝 미확정(이동 후보 등) — 델타를 단정하지 않는다
            }
            match d {
                Some(10) => out.push(rule_term(&props[a], ACT_DELETE, 0)),
                Some(v) if *v >= MOVE_BASE => {
                    out.push(rule_term(&props[a], ACT_MOVE, *v))
                }
                Some(c) => out.push(rule_term(&props[a], ACT_RECOLOR, *c)),
                None => {}
            }
        }
    }
    out
}

/// 수면: 델타 경험을 일반화한다 — 이웃쌍 + 3창(과제 내 구조). 채택은 MDL.
pub fn sleep_obj_abstract(rules: &[Term], lib: &mut Library) -> (usize, usize) {
    let (mut tried, mut added) = (0usize, 0usize);
    for w in rules.windows(2) {
        tried += 1;
        if let Some(a) = generalize(w) {
            if lib.insert(&a, Provenance::MonadDerived) {
                added += 1;
            }
        }
    }
    // 같은 행동끼리 더 넓게 접기(3개 창 — 그룹 전체는 과일반화라 이웃 3개까지만)
    for w in rules.windows(3) {
        tried += 1;
        if let Some(a) = generalize(w) {
            if lib.insert(&a, Provenance::MonadDerived) {
                added += 1;
            }
        }
    }
    (tried, added)
}

/// **과제 간 수면**(시도 168): 서로 다른 과제의 경험끼리 접는다 — 전이 규칙의
/// 원천은 이것이다. 이웃쌍(`windows`)은 대부분 같은 과제 안의 쌍이라, 과제 간
/// 공통 구조(팔레트가 달라도 성립하는 조건)가 거의 생성되지 않고 있었다.
///
/// 같은 행동 종류끼리만 쌍을 만든다(종류가 다르면 LGG가 행동을 변수로 만들어
/// 실행 불가 규칙이 된다). 채택은 여전히 MDL + 중복 병합.
pub fn sleep_obj_cross(groups: &[Vec<Term>], lib: &mut Library) -> (usize, usize) {
    let kind_of = |t: &Term| -> u64 {
        split_orule(t)
            .and_then(|(_, k, _)| match k {
                Term::Const(v) => Some(*v),
                _ => None,
            })
            .unwrap_or(0)
    };
    let (mut tried, mut added) = (0usize, 0usize);
    for gi in 0..groups.len() {
        for gj in gi + 1..groups.len() {
            for a in &groups[gi] {
                let ka = kind_of(a);
                for b in &groups[gj] {
                    if ka != kind_of(b) {
                        continue;
                    }
                    tried += 1;
                    if let Some(abs) = generalize(&[a.clone(), b.clone()]) {
                        if lib.insert(&abs, Provenance::MonadDerived) {
                            added += 1;
                        }
                    }
                }
            }
        }
    }
    (tried, added)
}

/// **스키마 정련**(시도 171): 라이브러리의 규칙들끼리 한 번 더 접는다.
///
/// 성질을 18종으로 늘리자(시도 170) 모호쌍은 428→164로 줄었지만 v2가 풀던
/// 홀드아웃 2건이 0으로 후퇴했다 — 쌍 LGG에서 **우연히 상수로 굳는 슬롯**이
/// 성질 수에 비례해 늘기 때문이다(일반화-특이성 트레이드오프의 실측). 처방은
/// 성질 축소가 아니라 **일반화 사다리의 다음 칸**: 같은 행동의 스키마끼리
/// 다시 LGG를 돌리면, 세 과제 이상에 공통인 조건만 상수로 남는다. 어느 수준이
/// 옳은지는 미리 정하지 않는다 — 여러 수준이 라이브러리에 공존하고, 과제의
/// 증거(select)가 고른다. 채택은 여전히 MDL.
pub fn sleep_obj_refine(lib: &mut Library) -> (usize, usize) {
    let mut by_kind: HashMap<u64, Vec<Term>> = HashMap::new();
    for e in &lib.entries {
        if let Some((_, Term::Const(k), _)) = split_orule(&e.schema) {
            by_kind.entry(*k).or_default().push(e.schema.clone());
        }
    }
    let (mut tried, mut added) = (0usize, 0usize);
    let mut kinds: Vec<u64> = by_kind.keys().copied().collect();
    kinds.sort_unstable();
    for k in kinds {
        let mut group = by_kind.remove(&k).unwrap();
        // 결정론적 순서(문자열 표기) — 인접쌍이 재현 가능해야 한다
        group.sort_by_key(|t| format!("{t}"));
        for w in group.windows(2) {
            tried += 1;
            if let Some(a) = generalize(w) {
                if lib.insert(&a, Provenance::MonadDerived) {
                    added += 1;
                }
            }
        }
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
/// 과제의 모든 (성질, 실제 델타) 지점. 하나라도 기술 불가면 빈 벡터 —
/// 선택·진단이 공유하는 표준 좌표계다.
pub fn task_props(train: &[(Grid, Grid)]) -> Vec<([u64; NPROPS], Option<u64>)> {
    let mut sites: Vec<([u64; NPROPS], Option<u64>)> = Vec::new();
    for (i, o) in train {
        let Some(deltas) = actual_deltas(i, o) else { return Vec::new() };
        let objs = decompose(i);
        let props = object_props(i, &objs);
        for (p, d) in props.into_iter().zip(deltas) {
            sites.push((p, d));
        }
    }
    sites
}

/// 이 규칙이 이 성질 지점에서 발화하는가(진단용 — 시도 170).
pub fn rule_covers(rule: &(Vec<Term>, Term, Term), props: &[u64; NPROPS]) -> bool {
    orule_fire(&rule.0, &rule.1, &rule.2, props).is_some()
}

pub fn select_obj_consistent(
    lib: &Library,
    train: &[(Grid, Grid)],
) -> Vec<(Vec<Term>, Term, Term)> {
    // 훈련쌍별 (성질, 실제 델타) — 하나라도 기술 불가면 빈 손
    let sites = task_props(train);
    if sites.is_empty() {
        return Vec::new();
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
                Some(v) if *v >= MOVE_BASE => k == ACT_MOVE && p == *v,
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
    // 1패스: 각 객체의 행동을 확정(첫 발화 규칙, 사전분포 순서)
    let mut acts: Vec<Option<(u64, u64)>> = vec![None; objs.len()];
    for (ix, p) in props.iter().enumerate() {
        for (cond, kind, param) in rules {
            if let Some((k, val)) = orule_fire(cond, kind, param, p) {
                acts[ix] = Some((k, val));
                break;
            }
        }
    }
    let mut out = g.clone();
    // 2패스-지우기: 삭제·이동의 옛 자리를 먼저 비운다(이동이 서로의 옛 자리로
    // 들어가도 안전하도록 그리기와 분리)
    for (o, a) in objs.iter().zip(acts.iter()) {
        let clear = matches!(a, Some((k, _)) if *k == ACT_DELETE || *k == ACT_MOVE);
        if !clear {
            continue;
        }
        for dy in 0..o.h {
            for dx in 0..o.w {
                if o.mask[dy * o.w + dx] {
                    out.set(o.x0 + dx, o.y0 + dy, 0);
                }
            }
        }
    }
    // 2패스-그리기: 재색과 이동의 새 자리
    for (o, a) in objs.iter().zip(acts.iter()) {
        match a {
            Some((k, val)) if *k == ACT_RECOLOR && *val <= 9 => {
                for dy in 0..o.h {
                    for dx in 0..o.w {
                        if o.mask[dy * o.w + dx] {
                            out.set(o.x0 + dx, o.y0 + dy, *val as u8);
                        }
                    }
                }
            }
            Some((k, val)) if *k == ACT_MOVE => {
                let Some((mdx, mdy)) = decode_move(*val) else { continue };
                let c = obj_color(o);
                for dy in 0..o.h {
                    for dx in 0..o.w {
                        if !o.mask[dy * o.w + dx] {
                            continue;
                        }
                        let nx = o.x0 as i64 + dx as i64 + mdx;
                        let ny = o.y0 as i64 + dy as i64 + mdy;
                        if nx >= 0 && ny >= 0 && (nx as usize) < g.w && (ny as usize) < g.h {
                            out.set(nx as usize, ny as usize, c);
                        }
                    }
                }
            }
            _ => {}
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

    /// **이동 1급 델타**(시도 169): 이동이 섞인 쌍이 완전 기술되고, 이동은
    /// 삭제로 오분류되지 않으며(옛 자리 배경화만으로는 삭제가 아니다), 재색과
    /// 이동이 각각 올바른 행동·매개로 추출된다.
    #[test]
    fn move_is_first_class_and_never_misread_as_delete() {
        let mut i = Grid::new(9, 9);
        place(&mut i, 0, 0, 2, 2, 3); // 재색될 것
        place(&mut i, 6, 6, 2, 2, 5); // 이동할 것
        let mut o = Grid::new(9, 9);
        place(&mut o, 0, 0, 2, 2, 7); // 재색됨
        place(&mut o, 2, 6, 2, 2, 5); // 왼쪽으로 4칸 이동
        // v2: 이동 포함 쌍도 완전 기술된다
        let deltas = actual_deltas(&i, &o).expect("이동 1급인데 완전 기술 실패");
        assert!(deltas.iter().any(|d| matches!(d, Some(v) if *v >= MOVE_BASE)));
        let r = extract_obj_rules(&[(i, o)]);
        assert_eq!(r.len(), 2, "재색 1 + 이동 1이어야 한다: {}건", r.len());
        let mut kinds: Vec<u64> = r
            .iter()
            .filter_map(|t| split_orule(t).and_then(|(_, k, _)| match k {
                Term::Const(v) => Some(*v),
                _ => None,
            }))
            .collect();
        kinds.sort_unstable();
        assert_eq!(kinds, vec![ACT_RECOLOR, ACT_MOVE]);
        // 이동 매개가 정확히 (-4, 0)인가
        let mv = r
            .iter()
            .find_map(|t| split_orule(t).and_then(|(_, k, p)| {
                (k == &Term::Const(ACT_MOVE)).then_some(p.clone())
            }))
            .unwrap();
        assert_eq!(mv, Term::Const(encode_move(-4, 0)));
    }

    /// **이동 전이**: "오른쪽으로 2칸"을 색·배치가 다른 두 과제에서 경험 →
    /// 본 적 없는 세 번째 과제를 재현하고 시험까지 푼다. 이동 벡터는 상수로
    /// 공유되고 나머지 성질은 LGG가 변수로 접는다.
    #[test]
    fn move_rule_transfers_across_tasks() {
        let mk = |x: usize, y: usize, c: u8| {
            let mut i = Grid::new(9, 9);
            place(&mut i, x, y, 2, 2, c);
            let mut o = Grid::new(9, 9);
            place(&mut o, x + 2, y, 2, 2, c);
            (i, o)
        };
        let mut rules = extract_obj_rules(&[mk(1, 1, 3)]);
        rules.extend(extract_obj_rules(&[mk(2, 4, 5)]));
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        // (경험과 같은 성질 부류: 비테두리 — x0=0이면 border-touch 성질이 달라
        //  발화하지 않는 것이 올바른 동작이다)
        let (ci, co) = mk(1, 3, 7);
        let train = [(ci, co)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty(), "이동 일관 규칙을 못 골랐다");
        assert!(obj_rules_reproduce(&sel, &train), "이동 재현 실패");
        let (ti, to) = mk(3, 2, 9);
        assert_eq!(apply_obj_rules(&sel, &ti), to, "이동 시험 실패");
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
        // (경험한 작은 객체들과 같은 성질 부류: 비테두리·아래쪽 — 성질이 조건이
        //  됐으므로 그 부류를 벗어나면 발화하지 않는 것이 올바른 동작이다)
        let (ci, co) = mk((2, 4, 9), (7, 7, 1));
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
