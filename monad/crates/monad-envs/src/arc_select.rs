//! M2-R — **선택 규칙**: "무엇이 답인가"를 배우는 계층(시도 206).
//!
//! # 왜 이 계층인가 (계량이 정했다)
//!
//! 미해결 크기 변환 97건을 해부하니 **출력이 입력 어느 객체의 bbox 잘라내기와
//! 정확히 같은** 과제가 7건(전 훈련쌍 성립)이었다. 작지만 이 계층에는 다른
//! 계층에 없는 성질이 있다:
//!
//! > **게이트가 "모든 객체를 덮어라"가 아니라 "어느 객체가 답인가"다.**
//!
//! 세션 내내 ③를 막아온 것은 과제당 100% 덮개 요구였다(바뀐 객체 하나만 못 덮어도
//! 실패). 선택은 **쌍당 결정 하나**이므로 그 전부-아니면-전무 구조를 우회한다.
//!
//! 그리고 새 표현이 아니다 — 기존 객체 성질 어휘(`object_props`)를 **새로운
//! 질문**에 재사용한다. 지금까지는 "각 객체에 무슨 일이 일어나는가"만 배웠다.
//!
//! # 규칙과 학습
//!
//! `SELRULE(OPROPS(p0..p13))` — 이 성질을 가진 객체가 답이다. 학습은 다른
//! 계층과 같은 규율: 정답 객체의 구체 성질에서 출발해 슬롯을 떨어뜨리되,
//! **과제 안에 반례가 없을 때만**(선택되지 않은 객체에서 발화하면 반례).

use crate::arc_objrule::{object_props, NPROPS};
use crate::grid::{components_bg, Grid, Obj};
use monad_core::abstraction::{generalize, Library, Provenance, Term};

const F_SELRULE: u32 = 930;
const F_SELPROPS: u32 = 931;

/// 한 훈련쌍의 선택 관측: 후보들의 성질과 **어느 것이 답인가**.
pub struct SelSite {
    pub cands: Vec<[u64; NPROPS]>,
    pub answer: usize,
}

/// 출력이 어느 객체의 bbox 잘라내기와 정확히 같은가 — 그 객체가 답이다.
fn answer_index(i: &Grid, o: &Grid, objs: &[Obj]) -> Option<usize> {
    objs.iter().position(|b| {
        b.w == o.w
            && b.h == o.h
            && (0..o.h).all(|y| (0..o.w).all(|x| i.get(b.x0 + x, b.y0 + y) == o.get(x, y)))
    })
}

/// 훈련쌍들에서 선택 관측을 뽑는다. **모든 쌍에서 답을 찾을 수 있어야** 한다 —
/// 하나라도 못 찾으면 이 과제는 선택 문제가 아니다(정직한 범위 제한).
pub fn task_sel_sites(train: &[(Grid, Grid)]) -> Vec<SelSite> {
    let mut out = Vec::new();
    for (i, o) in train {
        let objs = components_bg(i, false, 0);
        if objs.len() < 2 {
            return Vec::new(); // 후보가 하나뿐이면 고를 것이 없다
        }
        let Some(ans) = answer_index(i, o, &objs) else { return Vec::new() };
        out.push(SelSite { cands: object_props(i, &objs), answer: ans });
    }
    out
}

fn build(cond: Vec<Term>) -> Term {
    Term::App(F_SELRULE, vec![Term::App(F_SELPROPS, cond)])
}

fn split_sel(t: &Term) -> Option<&Vec<Term>> {
    match t {
        Term::App(f, args) if *f == F_SELRULE && args.len() == 1 => match &args[0] {
            Term::App(pf, c) if *pf == F_SELPROPS && c.len() == NPROPS => Some(c),
            _ => None,
        },
        _ => None,
    }
}

/// 이 조건이 이 성질 벡터에 맞는가(변수는 무엇이든 허용, 같은 변수는 같은 값).
fn sel_fire(cond: &[Term], props: &[u64; NPROPS]) -> bool {
    let mut bind: Vec<(u32, u64)> = Vec::new();
    for (t, &v) in cond.iter().zip(props.iter()) {
        match t {
            Term::Const(c) => {
                if *c != v {
                    return false;
                }
            }
            Term::Var(i) => match bind.iter().find(|(b, _)| b == i) {
                Some((_, prev)) if *prev != v => return false,
                Some(_) => {}
                None => bind.push((*i, v)),
            },
            Term::App(_, _) => return false,
        }
    }
    true
}

/// **거짓 양성**: 답이 아닌 후보에서 발화한다. 학습 중에는 이것만 금지한다 —
/// 슬롯을 떨어뜨리면 발화는 **늘기만** 하므로(단조성), "모든 답에서 발화"를
/// 처음부터 요구하면 구체 씨앗이 즉시 기각된다(시도 206에서 규칙 0개의 원인:
/// 한 쌍의 구체 벡터는 다른 쌍의 답과 절대 일치하지 않는다).
fn has_false_positive(cond: &[Term], sites: &[SelSite]) -> bool {
    sites.iter().any(|s| {
        s.cands
            .iter()
            .enumerate()
            .any(|(ix, p)| ix != s.answer && sel_fire(cond, p))
    })
}

/// **완전한 규칙**: 거짓 양성이 없고, 모든 쌍의 답에서 발화한다.
fn is_complete(cond: &[Term], sites: &[SelSite]) -> bool {
    !has_false_positive(cond, sites) && sites.iter().all(|s| sel_fire(cond, &s.cands[s.answer]))
}

/// 선택·게이트용 판정(완전성 요구).
fn has_counterexample(cond: &[Term], sites: &[SelSite]) -> bool {
    !is_complete(cond, sites)
}

/// **수면**: 정답 객체의 성질에서 출발해 무관한 슬롯을 떨어뜨린다(반례 검사).
pub fn sleep_sel_drop(per_task: &[(String, Vec<SelSite>)], lib: &mut Library) -> (usize, usize) {
    let (mut tried, mut added) = (0usize, 0usize);
    for (task_name, sites) in per_task {
        lib.minting = vec![task_name.clone()];
        for seed in sites {
            tried += 1;
            let mut cond: Vec<Term> = seed.cands[seed.answer]
                .iter()
                .map(|&v| Term::Const(v))
                .collect();
            // 씨앗이 자기 자리에서부터 거짓 양성이면(같은 성질의 다른 후보가
            // 있으면) 이 자리는 성질로 구분 불가 — 건너뛴다.
            if has_false_positive(&cond, sites) {
                continue;
            }
            // 단조성을 이용해 **거짓 양성이 없는 한** 최대한 떨어뜨린다
            for j in 0..NPROPS {
                let mut trial = cond.clone();
                trial[j] = Term::Var(j as u32);
                if !has_false_positive(&trial, sites) {
                    cond = trial;
                }
            }
            // 그 결과가 **모든 쌍의 답을 덮을 때만** 채택한다
            if !is_complete(&cond, sites) {
                continue;
            }
            let schema = build(cond);
            let concrete = build(
                seed.cands[seed.answer]
                    .iter()
                    .map(|&v| Term::Const(v))
                    .collect(),
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
    (tried, added)
}

/// **증거 기반 선택**: 이 과제의 모든 쌍에서 **정확히 답만** 고르는 규칙.
pub fn select_sel_consistent(lib: &Library, train: &[(Grid, Grid)]) -> Vec<Vec<Term>> {
    let sites = task_sel_sites(train);
    if sites.is_empty() {
        return Vec::new();
    }
    lib.by_prior()
        .into_iter()
        .filter_map(|ix| split_sel(&lib.entries[ix].schema).cloned())
        .filter(|cond| !has_counterexample(cond, &sites))
        .collect()
}

/// 규칙으로 답을 고르고 그 부분을 잘라낸다. 정확히 하나를 고를 때만 답한다 —
/// 둘 이상 고르면 모호하므로 답하지 않는다(거짓 양성 방지).
pub fn apply_sel_rules(rules: &[Vec<Term>], g: &Grid) -> Option<Grid> {
    let objs = components_bg(g, false, 0);
    if objs.len() < 2 {
        return None;
    }
    let props = object_props(g, &objs);
    for cond in rules {
        let hits: Vec<usize> = (0..objs.len()).filter(|&ix| sel_fire(cond, &props[ix])).collect();
        if hits.len() != 1 {
            continue;
        }
        let b = &objs[hits[0]];
        let mut out = Grid::new(b.w, b.h);
        for y in 0..b.h {
            for x in 0..b.w {
                out.set(x, y, g.get(b.x0 + x, b.y0 + y));
            }
        }
        return Some(out);
    }
    None
}

/// 전이 게이트: 훈련쌍 전부에서 정확히 답을 낸다.
pub fn sel_rules_reproduce(rules: &[Vec<Term>], train: &[(Grid, Grid)]) -> bool {
    !rules.is_empty()
        && train
            .iter()
            .all(|(i, o)| apply_sel_rules(rules, i).as_ref() == Some(o))
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

    /// **선택 규칙의 전이**: "가장 큰 것이 답"을 색·배치가 다른 두 과제에서 배우고
    /// 본 적 없는 세 번째에서 답한다. 게이트가 쌍당 결정 하나라 덮개 요구가 없다.
    #[test]
    fn selection_rule_transfers_across_tasks() {
        let mk = |bx: usize, by: usize, bc: u8, sc: u8| {
            let mut i = Grid::new(12, 12);
            place(&mut i, bx, by, 3, 3, bc); // 답(최대)
            place(&mut i, 9, 9, 1, 1, sc);
            place(&mut i, 0, 9, 2, 1, sc);
            let mut o = Grid::new(3, 3);
            place(&mut o, 0, 0, 3, 3, bc);
            (i, o)
        };
        let per_task: Vec<(String, Vec<SelSite>)> = vec![
            ("exp0".into(), task_sel_sites(&[mk(1, 1, 3, 5)])),
            ("exp1".into(), task_sel_sites(&[mk(5, 2, 6, 8)])),
        ];
        assert!(per_task.iter().all(|(_, s)| !s.is_empty()), "선택 관측 추출 실패");
        let mut lib = Library::new();
        let (tried, added) = sleep_sel_drop(&per_task, &mut lib);
        assert!(tried > 0 && added > 0, "선택 규칙이 만들어지지 않았다");

        // 본 적 없는 색·배치
        let train = [mk(2, 6, 9, 1)];
        let sel = select_sel_consistent(&lib, &train);
        assert!(!sel.is_empty(), "일관 선택 규칙을 못 골랐다");
        assert!(sel_rules_reproduce(&sel, &train), "선택 재현 실패");
        let (ti, to) = mk(7, 1, 2, 4);
        assert_eq!(apply_sel_rules(&sel, &ti).as_ref(), Some(&to), "시험 실패");
    }

    /// 모호하면 답하지 않는다(둘 이상이 조건을 만족하면 침묵).
    #[test]
    fn ambiguous_selection_stays_silent() {
        let mut g = Grid::new(10, 10);
        place(&mut g, 0, 0, 2, 2, 3);
        place(&mut g, 5, 5, 2, 2, 3); // 같은 크기·색 — 구별 불가
        let all_var: Vec<Term> = (0..NPROPS).map(|j| Term::Var(j as u32)).collect();
        assert!(apply_sel_rules(&[all_var], &g).is_none(), "모호한데 답했다");
    }
}
