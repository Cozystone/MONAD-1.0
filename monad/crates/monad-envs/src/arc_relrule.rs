//! M2-R — **일차 관계 규칙**: 존재 양화가 있는 세 번째 규칙 계층(GEN3).
//!
//! # 왜 이것인가 (시도 187~191의 계량이 정했다)
//!
//! 객체 속성 벡터 표현의 상한이 계량으로 확정됐다: 홀드아웃 바뀐 객체 212개 중
//! **122개(58%)가 원리상 차단** — 같은 과제에 성질 벡터가 동일한데 행동이 다른
//! 객체가 있어 **어떤 속성 규칙으로도** 구별할 수 없다. 열두 번의 개입이 모두
//! 최종 지표 106에 머문 이유다.
//!
//! `arc-relscan`이 그 다음을 지목했다: 모호쌍의 **87.1%**를 일차 관계가 가르고,
//! 상위는 전부 기하 관계다(above 57.2% · adjacent 48.6% · left_of 45.1% ·
//! same_row 31.8%). 의미 관계(같은 색·포함)는 0%였다.
//!
//! # 왜 고정폭 슬롯으로는 안 되는가 (시도 185에서 실증)
//!
//! 관계를 슬롯에 **집계해 넣으면**(예: "내 위 객체 중 최상위 역할") 보존 법칙에
//! 걸린다 — 판별력이 오른 만큼 덮개가 내려 총합이 그대로다. 필요한 것은 집계가
//! 아니라 **존재 양화**다:
//!
//! ```text
//! ∃Y ≠ X.  R(X, Y) ∧ target_cond(Y)     ← Y의 성질이 조건이 된다
//! ```
//!
//! 그리고 결정적으로, **self와 target의 슬롯이 변수를 공유**할 수 있다
//! ("내 색과 같은 색의 객체가 내 위에 있다"). 이것이 속성 벡터가 원리상 담지
//! 못하는 정보이며, 122개를 여는 유일한 형태다.
//!
//! # 교리 준수
//!
//! 관계 4종은 계량이 고른 것이고(사람이 아니라), 전부 동결 기질의 객체 분해에서
//! 기계적으로 나온다. 규칙의 *내용*은 전부 경험이 채우며, 채택은 반례 검사와
//! MDL이 한다. 기존 객체 계층(GEN2, 검증된 n=1)은 건드리지 않는다.

use crate::arc_objrule::{object_props, NPROPS};
use crate::grid::{components_bg, Grid, Obj};
use monad_core::abstraction::{generalize, Library, Provenance, Term};

const F_RELRULE: u32 = 920;
const F_SELF: u32 = 921;
const F_TARGET: u32 = 922;
const F_RACT: u32 = 923;

const ACT_RECOLOR: u64 = 1;
const ACT_DELETE: u64 = 2;
const MOVE_BASE: u64 = 1000;

/// 계량이 고른 관계 4종(분리율 순). 전부 기하 관계다.
pub const NRELS: usize = 4;
const REL_NAMES: [&str; NRELS] = ["above", "adjacent", "left_of", "same_row"];

fn rel_holds(r: usize, me: &Obj, other: &Obj) -> bool {
    match r {
        // above: 상대가 내 **바로 위**(열이 겹쳐야 한다). 열 겹침을 빼면
        // "위쪽 어딘가"가 되어 거의 모든 객체 쌍이 성립하고, 판별력이 사라진다
        // (시도 192에서 실측: 규칙이 만들어져도 전이 과제에서 발화 0회).
        0 => {
            other.y0 + other.h <= me.y0
                && me.x0 < other.x0 + other.w
                && other.x0 < me.x0 + me.w
        }
        1 => {
            me.x0 <= other.x0 + other.w
                && other.x0 <= me.x0 + me.w
                && me.y0 <= other.y0 + other.h
                && other.y0 <= me.y0 + me.h
        }
        // left_of: 상대가 내 **바로 왼쪽**(행이 겹쳐야 한다). 같은 이유.
        2 => {
            other.x0 + other.w <= me.x0
                && me.y0 < other.y0 + other.h
                && other.y0 < me.y0 + me.h
        }
        _ => me.y0 < other.y0 + other.h && other.y0 < me.y0 + me.h, // same_row
    }
}

/// 한 관측 지점: 자기 성질 · 실제 행동 · **관계별 상대들의 성질**(양화의 재료).
#[derive(Clone, Debug)]
pub struct RSite {
    pub props: [u64; NPROPS],
    pub delta: Option<u64>,
    pub copies: Vec<u64>,
    /// `targets[r]` = 관계 r로 맺어지는 상대들의 성질 벡터.
    pub targets: [Vec<[u64; NPROPS]>; NRELS],
}

fn build_targets(objs: &[Obj], props: &[[u64; NPROPS]], ix: usize) -> [Vec<[u64; NPROPS]>; NRELS] {
    let mut out: [Vec<[u64; NPROPS]>; NRELS] = Default::default();
    for (r, slot) in out.iter_mut().enumerate() {
        for (j, o) in objs.iter().enumerate() {
            if j != ix && rel_holds(r, &objs[ix], o) {
                slot.push(props[j]);
            }
        }
        // **정규화**(시도 201): 존재 양화는 상대의 **집합**만 보므로 순서는 의미가
        // 없다. 정렬·중복 제거하지 않으면 같은 집합을 다른 순서로 가진 두 객체가
        // "구별 가능"으로 잘못 집계되어 **천장이 부풀려진다**(원리상 차단 21의 신뢰성).
        slot.sort_unstable();
        slot.dedup();
    }
    out
}

/// 격자 하나의 지점들(행동 없이 — 적용 시에 쓴다).
pub fn grid_sites(g: &Grid) -> Vec<RSite> {
    let objs = components_bg(g, false, 0);
    let props = object_props(g, &objs);
    (0..objs.len())
        .map(|ix| RSite {
            props: props[ix],
            delta: None,
            copies: Vec::new(),
            targets: build_targets(&objs, &props, ix),
        })
        .collect()
}

/// 훈련쌍의 지점들(행동 포함). 객체 계층의 델타 매칭을 그대로 쓴다.
pub fn task_rsites(train: &[(Grid, Grid)]) -> Vec<RSite> {
    let mut out = Vec::new();
    for (i, o) in train {
        let Some((deltas, matched, copies)) = crate::arc_objrule::pair_deltas(i, o) else {
            continue;
        };
        let objs = components_bg(i, false, 0);
        let props = object_props(i, &objs);
        for ix in 0..objs.len() {
            // 델타 미확정 객체는 씨앗도 반례도 될 수 없다(시도 186의 규율)
            if !matched[ix] && copies[ix].is_empty() {
                continue;
            }
            out.push(RSite {
                props: props[ix],
                delta: deltas[ix],
                copies: copies[ix].clone(),
                targets: build_targets(&objs, &props, ix),
            });
        }
    }
    out
}

fn split_relrule(t: &Term) -> Option<(&Vec<Term>, u64, &Vec<Term>, &Term, &Term)> {
    let Term::App(f, args) = t else { return None };
    if *f != F_RELRULE || args.len() != 4 {
        return None;
    }
    let (Term::App(sf, sc), Term::Const(r), Term::App(tf, tc), Term::App(af, act)) =
        (&args[0], &args[1], &args[2], &args[3])
    else {
        return None;
    };
    if *sf != F_SELF || *tf != F_TARGET || *af != F_RACT || sc.len() != NPROPS
        || tc.len() != NPROPS || act.len() != 2
    {
        return None;
    }
    Some((sc, *r, tc, &act[0], &act[1]))
}

/// 한 슬롯 판정 + 변수 바인딩(self와 target이 **변수를 공유**한다).
fn slot_ok(t: &Term, v: u64, bind: &mut Vec<(u32, u64)>) -> bool {
    match t {
        Term::Const(c) => *c == v,
        Term::Var(i) => match bind.iter().find(|(b, _)| b == i) {
            Some((_, prev)) => *prev == v,
            None => {
                bind.push((*i, v));
                true
            }
        },
        Term::App(_, _) => false,
    }
}

/// **존재 양화 발화**: self가 맞고, 관계 r로 맺어진 상대 중 target 조건을
/// 만족하는 것이 **하나라도 있으면** 발화한다.
fn relrule_fire(
    self_cond: &[Term],
    rel: u64,
    target_cond: &[Term],
    kind: &Term,
    param: &Term,
    site: &RSite,
) -> Option<(u64, u64)> {
    let r = rel as usize;
    if r >= NRELS {
        return None;
    }
    let mut base: Vec<(u32, u64)> = Vec::new();
    for (t, &v) in self_cond.iter().zip(site.props.iter()) {
        if !slot_ok(t, v, &mut base) {
            return None;
        }
    }
    // ∃Y: 상대 하나라도 target 조건을 만족하면 된다(바인딩은 상대마다 새로 시도)
    for tp in &site.targets[r] {
        let mut bind = base.clone();
        if target_cond
            .iter()
            .zip(tp.iter())
            .all(|(t, &v)| slot_ok(t, v, &mut bind))
        {
            let k = match kind {
                Term::Const(k) => *k,
                _ => return None,
            };
            let p = match param {
                Term::Const(p) => *p,
                Term::Var(v) => bind.iter().find(|(b, _)| b == v).map(|(_, x)| *x)?,
                Term::App(_, _) => return None,
            };
            return Some((k, p));
        }
    }
    None
}

fn action_ok(k: u64, p: u64, site: &RSite) -> bool {
    match site.delta {
        None => k == ACT_RECOLOR && p == site.props[0],
        Some(10) => k == ACT_DELETE,
        Some(v) if v >= MOVE_BASE => k == 3 && p == v, // ACT_MOVE
        Some(c) => k == ACT_RECOLOR && p == c,
    }
}

fn has_counterexample(
    self_cond: &[Term],
    rel: u64,
    target_cond: &[Term],
    kind: &Term,
    param: &Term,
    sites: &[RSite],
) -> bool {
    sites.iter().any(|s| {
        relrule_fire(self_cond, rel, target_cond, kind, param, s)
            .map(|(k, p)| !action_ok(k, p, s))
            .unwrap_or(false)
    })
}

fn build(self_cond: Vec<Term>, rel: u64, target_cond: Vec<Term>, kind: u64, param: Term) -> Term {
    Term::App(
        F_RELRULE,
        vec![
            Term::App(F_SELF, self_cond),
            Term::Const(rel),
            Term::App(F_TARGET, target_cond),
            Term::App(F_RACT, vec![Term::Const(kind), param]),
        ],
    )
}

/// **수면**: 관계 규칙을 반례 기반 조건 탈락으로 만든다(과제 내 판정).
///
/// 씨앗: (바뀐 객체 X, 관계 r, X와 r로 맺어진 상대 Y). self와 target 조건을
/// 구체값에서 출발해 슬롯을 떨어뜨리고, **self↔target 슬롯 등식**을 시도한다 —
/// 그 등식이 속성 벡터가 담지 못하는 관계 정보다.
pub fn sleep_rel_drop(per_task: &[(String, Vec<RSite>)], lib: &mut Library) -> (usize, usize) {
    let (mut tried, mut added) = (0usize, 0usize);
    // **유지도 증거다**(시도 216). 지금까지 `delta == None`인 객체(=바뀌지 않은
    // 객체)는 씨앗에서 통째로 빠졌다("규칙으로 쓸 것이 없다"). 시도 215가 그
    // 대가를 드러냈다: 근접 실패에서 옳은 행동은 **"이 객체를 건드리지 않는 것"**
    // 인데, 부작위를 말하는 규칙이 없으니 훈련과 모순 없는 다른 규칙이 그 객체를
    // 덮어써도 반증할 길이 없었다.
    //
    // 새 연산은 필요 없다 — `action_ok`는 이미 `delta == None`을 **"자기 색으로
    // 재색"**으로 취급한다. 즉 유지는 이 규칙 형태 안에서 이미 표현 가능했고,
    // 수면이 씨앗으로 삼지 않았을 뿐이다. 버려지던 증거를 줍는 것이다.
    //
    // 그러면 유지 규칙과 변경 규칙이 같은 객체에서 갈릴 수 있고, 그때는
    // 시도 215의 기권 규율(`MONAD_ARC_ABSTAIN`)이 "그대로 두라"로 판정한다.
    let keep = std::env::var("MONAD_ARC_KEEP").is_ok();
    for (task_name, sites) in per_task {
        lib.minting = vec![task_name.clone()];
        for seed in sites {
            let mut acts: Vec<(u64, u64)> = Vec::new();
            match seed.delta {
                Some(10) => acts.push((ACT_DELETE, 0)),
                Some(v) if v >= MOVE_BASE => acts.push((3, v)),
                Some(c) => acts.push((ACT_RECOLOR, c)),
                None => {
                    if keep {
                        acts.push((ACT_RECOLOR, seed.props[0]));
                    }
                }
            }
            if acts.is_empty() {
                continue;
            }
            for r in 0..NRELS {
                // 관계별 상대를 **여럿** 씨앗으로 쓴다(시도 193).
                // 첫 상대만 쓰면 판별에 필요한 상대가 첫 번째가 아닐 때 그 규칙이
                // 아예 만들어지지 않는다 — 폭발 방지가 표현력을 깎고 있었다.
                // 서로 다른 성질 벡터만 취해 상한까지(결정론적 순서 유지).
                let mut seen_t: Vec<[u64; NPROPS]> = Vec::new();
                for &tp in seed.targets[r].iter() {
                    if seen_t.len() >= 4 {
                        break;
                    }
                    if !seen_t.contains(&tp) {
                        seen_t.push(tp);
                    }
                }
                for tprops in seen_t {
                for &(kind, param) in &acts {
                    // **두 일반화 순서를 모두 만든다**(시도 184의 교훈).
                    // 먼저 떨어뜨리면 self·target 슬롯이 각각 자유 변수가 되어
                    // 등식을 맺을 상수가 남지 않는다 — 그런데 그 등식이야말로
                    // 속성 벡터가 담지 못하는 관계 정보다. 순서를 고르지 않고
                    // 두 가설을 다 남긴 뒤 **과제의 증거가 고르게 한다**.
                    // 세 변형: ①탈락만 ②등식 우선 ③**self 통째 변수**(시도 200).
                    //
                    // 합집합 덮개 계량(74/212, GEN3 단독 9)이 드러낸 것: GEN3 규칙이
                    // self 조건 14슬롯에 관계를 **덧붙인** 형태라 속성 규칙의 특수화가
                    // 되어, 속성이 이미 통하는 자리에서만 발화한다. 관계만이 가를 수
                    // 있는 101개(122−21)에 닿으려면 **관계가 판별을 떠맡아야** 하고,
                    // 그러려면 self가 일반적이어야 한다. 슬롯 단위 탈락으로는 거기
                    // 도달하기 어렵다(중간 단계마다 반례에 걸린다) — target을 통째로
                    // 비운 "맨몸 존재"의 거울상을 명시적으로 시도한다.
                    for variant in 0..3 {
                        let equations_first = variant == 1;
                        tried += 1;
                        let mut sc: Vec<Term> =
                            seed.props.iter().map(|&v| Term::Const(v)).collect();
                        let mut tc: Vec<Term> = tprops.iter().map(|&v| Term::Const(v)).collect();
                        let out = Term::Const(param);
                        let equate = |sc: &mut Vec<Term>, tc: &mut Vec<Term>| {
                            let mut nv = 800u32;
                            for a in 0..NPROPS {
                                for b in 0..NPROPS {
                                    if seed.props[a] != tprops[b] {
                                        continue;
                                    }
                                    if !matches!(sc[a], Term::Const(_))
                                        || !matches!(tc[b], Term::Const(_))
                                    {
                                        continue;
                                    }
                                    let (mut s2, mut t2) = (sc.clone(), tc.clone());
                                    s2[a] = Term::Var(nv);
                                    t2[b] = Term::Var(nv);
                                    if !has_counterexample(
                                        &s2, r as u64, &t2, &Term::Const(kind), &out, sites,
                                    ) {
                                        *sc = s2;
                                        *tc = t2;
                                        nv += 1;
                                    }
                                }
                            }
                        };
                        if variant == 2 {
                            // self를 통째로 자유 변수로: 관계와 target이 전부 판별한다
                            let bare_self: Vec<Term> =
                                (0..NPROPS).map(|j| Term::Var(j as u32)).collect();
                            if !has_counterexample(
                                &bare_self, r as u64, &tc, &Term::Const(kind), &out, sites,
                            ) {
                                sc = bare_self;
                            }
                        }
                        if equations_first {
                            equate(&mut sc, &mut tc);
                        } else if variant == 0 {
                            // **맨몸 존재**(시도 195): target을 통째로 자유 변수로
                            // 두면 규칙은 "그런 상대가 **있기만 하면**"이 된다
                            // ("바로 위에 뭔가 있는 객체를 지운다"). 관계 규칙에서
                            // 가장 일반적인 형태인데, 슬롯을 하나씩 떨어뜨리는
                            // 순서로는 도달하기 어렵다 — 존재 양화 탓에 발화가
                            // 잦아 중간 단계에서 반례에 걸리기 때문이다.
                            // 통째로 시도해 반례가 없으면 채택한다.
                            let bare: Vec<Term> =
                                (0..NPROPS).map(|j| Term::Var(100 + j as u32)).collect();
                            if !has_counterexample(
                                &sc, r as u64, &bare, &Term::Const(kind), &out, sites,
                            ) {
                                tc = bare;
                            }
                        }
                        for j in 0..NPROPS {
                            if matches!(sc[j], Term::Var(_)) {
                                continue;
                            }
                            let mut trial = sc.clone();
                            trial[j] = Term::Var(j as u32);
                            if !has_counterexample(
                                &trial, r as u64, &tc, &Term::Const(kind), &out, sites,
                            ) {
                                sc = trial;
                            }
                        }
                        for j in 0..NPROPS {
                            if matches!(tc[j], Term::Var(_)) {
                                continue;
                            }
                            let mut trial = tc.clone();
                            trial[j] = Term::Var(100 + j as u32);
                            if !has_counterexample(
                                &sc, r as u64, &trial, &Term::Const(kind), &out, sites,
                            ) {
                                tc = trial;
                            }
                        }
                        if !equations_first {
                            equate(&mut sc, &mut tc);
                        }
                        let schema = build(sc, r as u64, tc, kind, out);
                        let concrete = build(
                            seed.props.iter().map(|&v| Term::Const(v)).collect(),
                            r as u64,
                            tprops.iter().map(|&v| Term::Const(v)).collect(),
                            kind,
                            Term::Const(param),
                        );
                        let Some(bind) = schema.matches(&concrete) else { continue };
                        if let Some(a) = generalize(&[schema.clone(), concrete]) {
                            let abs = monad_core::abstraction::Abstraction {
                                schema,
                                instances: vec![bind],
                                gain: a.gain.max(1),
                            };
                            if lib.insert(&abs, Provenance::MonadDerived) {
                                added += 1;
                            }
                        }
                    }
                }
                }
            }
        }
    }
    (tried, added)
}

type RelRule = (Vec<Term>, u64, Vec<Term>, Term, Term);

/// **증거 기반 선택**: 이 과제의 모든 지점에서 모순 없이 발화하는 규칙만.
pub fn select_rel_consistent(lib: &Library, train: &[(Grid, Grid)]) -> Vec<RelRule> {
    let sites = task_rsites(train);
    if sites.is_empty() {
        return Vec::new();
    }
    let mut kept = Vec::new();
    for e in lib.by_prior() {
        let Some((sc, r, tc, kind, param)) = split_relrule(&lib.entries[e].schema) else {
            continue;
        };
        let mut consistent = true;
        let mut useful = false;
        for s in &sites {
            let Some((k, p)) = relrule_fire(sc, r, tc, kind, param, s) else { continue };
            if !action_ok(k, p, s) {
                consistent = false;
                break;
            }
            if s.delta.is_some() {
                useful = true;
            }
        }
        if consistent && useful {
            kept.push((sc.clone(), r, tc.clone(), kind.clone(), param.clone()));
        }
    }
    kept
}

/// 선택 규칙 적용(첫 발화 승, 나머지는 유지).
/// 이 과제의 **훈련쌍에서 한 번도 바뀌지 않은** 객체들의 성질 벡터.
///
/// 시도 216에서 다른 과제의 유지 규칙을 배워 봤지만 근접 실패의 그 객체를 덮는
/// 규칙은 없었다. 그런데 **이 과제의 훈련쌍 자체가** 직접적인 증거를 갖고 있다 —
/// "이런 성질의 객체는 그대로 둔다". 훈련쌍은 주어진 것이므로 이것을 쓰는 것은
/// 정답을 엿보는 것이 아니다(시험 출력은 건드리지 않는다).
///
/// 어느 훈련쌍에서든 **한 번이라도 바뀌었다면** 그 벡터는 보호하지 않는다 —
/// 증거가 갈리는 자리에서 침묵을 강제하면 고쳐야 할 것도 못 고친다.
pub fn keep_props_from_train(train: &[(Grid, Grid)]) -> Vec<[u64; NPROPS]> {
    let sites = task_rsites(train);
    let mut changed: Vec<[u64; NPROPS]> = Vec::new();
    let mut kept: Vec<[u64; NPROPS]> = Vec::new();
    for s in &sites {
        if s.delta.is_some() || !s.copies.is_empty() {
            changed.push(s.props);
        } else {
            kept.push(s.props);
        }
    }
    kept.retain(|p| !changed.contains(p));
    kept.sort_unstable();
    kept.dedup();
    kept
}

/// 이 과제의 훈련쌍에 **나타난 적 있는** 객체 성질 벡터 전부(바뀐 것·안 바뀐 것 모두).
///
/// 시도 217은 "훈련에서 **늘 그대로**였던" 벡터를 보호했다. 그런데 시도 215~217의
/// 진단이 가리킨 객체는 훈련에 **아예 나타난 적이 없는** 종류였다 — 보호 목록에
/// 없으니 그대로 통과해 망가졌다. 필요한 것은 그 여집합이다:
/// **아는 종류에만 손대고, 모르는 종류에는 침묵한다.**
pub fn attested_props_from_train(train: &[(Grid, Grid)]) -> Vec<[u64; NPROPS]> {
    let mut v: Vec<[u64; NPROPS]> = task_rsites(train).iter().map(|s| s.props).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// **외삽 금지**: 훈련에서 본 적 없는 성질의 객체에는 어떤 규칙도 적용하지 않는다.
///
/// 게이트(훈련 정확 재현)에는 영향이 없다 — 훈련 객체는 정의상 전부 attested다.
/// 이 보호막도 **시험에서만** 작동한다.
pub fn apply_rel_rules_attested(
    rules: &[RelRule],
    g: &Grid,
    attested: &[[u64; NPROPS]],
) -> Grid {
    let objs = components_bg(g, false, 0);
    let sites = grid_sites(g);
    let mut out = g.clone();
    for (o, s) in objs.iter().zip(sites.iter()) {
        if attested.binary_search(&s.props).is_err() {
            continue; // 모르는 종류 — 침묵한다
        }
        for (sc, r, tc, kind, param) in rules {
            let Some((k, val)) = relrule_fire(sc, *r, tc, kind, param, s) else { continue };
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

/// 유지 증거로 보호하며 적용한다. 훈련에서 늘 그대로였던 성질의 객체는
/// 어떤 규칙이 발화해도 건드리지 않는다.
///
/// 게이트(훈련 정확 재현)에는 영향이 없다: 보호되는 벡터는 훈련에서 바뀐 적이
/// 없고, 선택된 규칙은 훈련과 모순되지 않으므로 그 자리에서 이미 아무 일도
/// 하지 않았다. 즉 이 보호막은 **시험에서만** 작동한다.
pub fn apply_rel_rules_keepguard(
    rules: &[RelRule],
    g: &Grid,
    keep: &[[u64; NPROPS]],
) -> Grid {
    let objs = components_bg(g, false, 0);
    let sites = grid_sites(g);
    let mut out = g.clone();
    for (o, s) in objs.iter().zip(sites.iter()) {
        if keep.binary_search(&s.props).is_ok() {
            continue;
        }
        for (sc, r, tc, kind, param) in rules {
            let Some((k, val)) = relrule_fire(sc, *r, tc, kind, param, s) else { continue };
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

pub fn apply_rel_rules(rules: &[RelRule], g: &Grid) -> Grid {
    let objs = components_bg(g, false, 0);
    let sites = grid_sites(g);
    let mut out = g.clone();
    // **모호하면 답하지 않는다**(시도 215). 기본 정책은 "먼저 맞는 규칙이 이긴다"
    // 인데, 그러면 서로 다른 행동을 말하는 규칙들이 있어도 순서가 임의로 승자를
    // 정한다. 시도 214의 계량이 이 자리를 지목했다: 근접 실패의 잔여는
    // **망침 4 · 미수정 0**이었다 — 고쳐야 할 곳은 전부 고쳤고, 건드리지 말았어야
    // 할 곳 4칸만 건드렸다. 그 4칸을 건드리지 않으면 정확 일치다.
    //
    // 이 규율은 이 저장소에 이미 있다(`arc_select::apply_sel_rules`는 정확히 하나가
    // 발화할 때만 답한다). 관계 계층에만 빠져 있었다.
    let abstain = std::env::var("MONAD_ARC_ABSTAIN").is_ok();
    for (o, s) in objs.iter().zip(sites.iter()) {
        if abstain {
            // 발화한 규칙들이 **서로 다른 행동**을 말하면 이 객체는 건드리지 않는다.
            let mut acts = rules
                .iter()
                .filter_map(|(sc, r, tc, kind, param)| {
                    relrule_fire(sc, *r, tc, kind, param, s)
                });
            if let Some(first) = acts.next() {
                if acts.any(|a| a != first) {
                    continue;
                }
            }
        }
        for (sc, r, tc, kind, param) in rules {
            let Some((k, val)) = relrule_fire(sc, *r, tc, kind, param, s) else { continue };
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

/// 전이 게이트: 훈련쌍을 완전히 재현하는가.
pub fn rel_rules_reproduce(rules: &[RelRule], train: &[(Grid, Grid)]) -> bool {
    !rules.is_empty()
        && train
            .iter()
            .all(|(i, o)| i.w == o.w && i.h == o.h && &apply_rel_rules(rules, i) == o)
}

/// 이 규칙이 이 지점에서 발화하는가(진단용).
pub fn rel_rule_covers(rule: &RelRule, site: &RSite) -> bool {
    relrule_fire(&rule.0, rule.1, &rule.2, &rule.3, &rule.4, site).is_some()
}

/// **필터 이전**에 이 지점에서 정답 행동을 주장하는 규칙이 있는가(진단용).
/// GEN2에서 이해를 열었던 원인 분해를 GEN3에도 적용한다:
/// 부재(경험 구멍) vs 필터 탈락(판별력 부족)을 가른다.
pub fn rel_raw_correct_exists(lib: &Library, site: &RSite) -> bool {
    lib.entries.iter().any(|e| {
        split_relrule(&e.schema)
            .and_then(|(sc, r, tc, k, p)| relrule_fire(sc, r, tc, k, p, site))
            .map(|(k, p)| action_ok(k, p, site))
            .unwrap_or(false)
    })
}

/// 이 규칙의 발화 결과(계층 결합용).
pub fn rel_rule_action(rule: &RelRule, site: &RSite) -> Option<(u64, u64)> {
    relrule_fire(&rule.0, rule.1, &rule.2, &rule.3, &rule.4, site)
}

/// **두 계층의 합집합 적용**(시도 196).
///
/// GEN2(속성)와 GEN3(관계)는 **서로 다른 객체를 덮는다** — 전자는 성질로
/// 결정되는 자리를, 후자는 관계로 결정되는 자리를. 각각 30~76%씩 덮지만
/// 과제의 게이트는 바뀐 객체가 **전부** 덮여야 열린다. 두 계층을 합치면
/// 그 전부가 채워질 수 있다.
///
/// 순서: 속성 규칙 먼저(더 구체적·검증된 계층), 없으면 관계 규칙.
pub fn apply_combined(
    obj_rules: &[(Vec<Term>, Term, Term)],
    rel_rules: &[RelRule],
    g: &Grid,
) -> Grid {
    let objs = components_bg(g, false, 0);
    let props = object_props(g, &objs);
    let sites = grid_sites(g);
    let mut out = g.clone();
    // 결합 경로도 같은 규율을 따른다(시도 215) — 속성·관계 양쪽에서 발화한
    // 행동이 하나로 모이지 않으면 건드리지 않는다.
    let abstain = std::env::var("MONAD_ARC_ABSTAIN").is_ok();
    for (ix, o) in objs.iter().enumerate() {
        if abstain {
            let mut acts = obj_rules
                .iter()
                .filter_map(|r| crate::arc_objrule::obj_rule_action(r, &props[ix]))
                .chain(rel_rules.iter().filter_map(|r| rel_rule_action(r, &sites[ix])));
            if let Some(first) = acts.next() {
                if acts.any(|a| a != first) {
                    continue;
                }
            }
        }
        let act = obj_rules
            .iter()
            .find_map(|r| crate::arc_objrule::obj_rule_action(r, &props[ix]))
            .or_else(|| {
                rel_rules
                    .iter()
                    .find_map(|r| rel_rule_action(r, &sites[ix]))
            });
        let Some((k, val)) = act else { continue };
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
    }
    out
}

/// 결합 게이트: 두 계층을 합쳐 훈련쌍을 완전히 재현하는가.
pub fn combined_reproduce(
    obj_rules: &[(Vec<Term>, Term, Term)],
    rel_rules: &[RelRule],
    train: &[(Grid, Grid)],
) -> bool {
    (!obj_rules.is_empty() || !rel_rules.is_empty())
        && train.iter().all(|(i, o)| {
            i.w == o.w && i.h == o.h && &apply_combined(obj_rules, rel_rules, i) == o
        })
}

/// 관계 이름(보고용).
pub fn rel_name(r: usize) -> &'static str {
    REL_NAMES.get(r).copied().unwrap_or("?")
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

    /// **존재 양화가 속성 벡터로 불가능한 구별을 해낸다**(GEN3의 존재 이유).
    ///
    /// 규칙: "**같은 색 객체가 자기 위에 있는** 객체를 지운다."
    /// 두 후보 객체는 크기·색·테두리·개수 등 **모든 자기 성질이 동일**하고,
    /// 오직 "위에 같은 색 짝이 있는가"만 다르다 — 속성 벡터로는 원리상 구별 불가
    /// (시도 187이 측정한 122개가 정확히 이런 자리들이다).
    #[test]
    fn existential_quantification_separates_what_attributes_cannot() {
        // 위쪽에 색 A 짝이 있는 A(삭제) / 짝이 없는 A(유지) — 자기 성질은 동일
        let mk = |a: u8| {
            let mut i = Grid::new(12, 12);
            place(&mut i, 1, 0, 2, 2, a); // 짝(위)
            place(&mut i, 1, 5, 2, 2, a); // 대상: 위에 짝 있음 → 삭제
            place(&mut i, 8, 5, 2, 2, a); // 대상: 위에 짝 없음 → 유지
            let mut o = i.clone();
            place(&mut o, 1, 5, 2, 2, 0);
            (i, o)
        };
        let per_task: Vec<(String, Vec<RSite>)> = vec![("exp0".into(), task_rsites(&[mk(3)]))];
        assert!(!per_task[0].1.is_empty());
        // 속성 벡터로는 두 대상이 구별되지 않음을 먼저 확인(전제의 검증)
        let ps = crate::arc_objrule::task_props_partial(&[mk(3)]);
        let ambiguous = ps.iter().enumerate().any(|(a, sa)| {
            ps.iter()
                .enumerate()
                .any(|(b, sb)| b != a && sb.props == sa.props && sb.delta != sa.delta)
        });
        assert!(ambiguous, "시험 전제 실패: 속성 벡터가 이미 구별한다");

        let mut lib = Library::new();
        let (tried, added) = sleep_rel_drop(&per_task, &mut lib);
        assert!(tried > 0 && added > 0, "관계 규칙이 만들어지지 않았다");

        // 같은 구조·다른 팔레트로 전이
        let train = [mk(6)];
        let sel = select_rel_consistent(&lib, &train);
        if sel.is_empty() {
            // 진단: 학습된 규칙이 전이 과제에서 어떻게 실패하는가
            let sites = task_rsites(&train);
            let mut fired = 0;
            let mut wrong = 0;
            for e in &lib.entries {
                if let Some((sc, r, tc, k, pm)) = split_relrule(&e.schema) {
                    for st in &sites {
                        if let Some((kk, pp)) = relrule_fire(sc, r, tc, k, pm, st) {
                            fired += 1;
                            if !action_ok(kk, pp, st) {
                                wrong += 1;
                            }
                        }
                    }
                }
            }
            let learn_sites = task_rsites(&[mk(3)]);
            let tgt: Vec<usize> = learn_sites.iter().map(|s| s.targets[0].len()).collect();
            let tgt2: Vec<usize> = sites.iter().map(|s| s.targets[0].len()).collect();
            let schemas: Vec<String> =
                lib.entries.iter().map(|e| format!("{}", e.schema)).collect();
            panic!(
                "일관 0 — 규칙 {}개 지점 {}개 발화 {} 오발화 {}
학습 above타깃수 {:?} 전이 above타깃수 {:?}
스키마: {:?}",
                lib.entries.len(), sites.len(), fired, wrong, tgt, tgt2, schemas
            );
        }
        assert!(
            rel_rules_reproduce(&sel, &train),
            "존재 양화 규칙이 재현에 실패했다"
        );
        let (ti, to) = mk(9);
        assert_eq!(apply_rel_rules(&sel, &ti), to, "시험 팔레트에서 실패");
    }

    /// 맞지 않는 과제에서는 게이트가 막는다(거짓 양성 방지).
    #[test]
    fn rel_gate_rejects_unrelated_task() {
        let mut i = Grid::new(10, 10);
        place(&mut i, 1, 0, 2, 2, 3);
        place(&mut i, 1, 5, 2, 2, 3);
        let mut o = i.clone();
        place(&mut o, 1, 5, 2, 2, 0);
        let per_task = vec![("exp0".to_string(), task_rsites(&[(i.clone(), o)]))];
        let mut lib = Library::new();
        sleep_rel_drop(&per_task, &mut lib);
        // 전혀 다른 변환: 전부 7로 재색
        let mut o2 = i.clone();
        place(&mut o2, 1, 0, 2, 2, 7);
        place(&mut o2, 1, 5, 2, 2, 7);
        let train = [(i, o2)];
        let sel = select_rel_consistent(&lib, &train);
        assert!(!rel_rules_reproduce(&sel, &train), "맞지 않는데 통과시켰다");
    }
}
