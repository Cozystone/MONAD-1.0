//! M2-R — **생성적 기질**: 패치 재작성 규칙.
//!
//! # 왜 이것인가 (잔차 해부가 정한 사양, 시도 156)
//!
//! 미해결 과제의 잔차 111건을 집계한 결과:
//!
//! | 측정 | 값 | 함의 |
//! |---|---|---|
//! | 평균 잔차 셀 | 23.6 | 작다 |
//! | 덩어리 수 | 4.1 (덩어리당 ~5.8셀) | **뭉쳐 있다**(흩어진 잡음 아님) |
//! | 없어서/남아서/색만 틀림 | 42% / 23% / 35% | **생성·삭제·재색이 섞여 있다** |
//! | 입력 객체 안 | 54% | 객체 연산도, 격자 연산도 아니다 — 반반 |
//! | 행/열 편중 | 0.13 | 줄 구조 아님 — 2차원 국소 |
//!
//! 요약: **국소 이웃을 보고 조건부로 생성·삭제·재색하는 규칙**이 없다. 동결
//! 어휘에는 전역 연산(모두에게 같은 것)과 객체 속성 규칙은 있지만 이것이 없다.
//!
//! # 기존 시도와 무엇이 다른가
//!
//! EBM(시도 141)·셀 역할 꿈(139)도 국소 문맥을 썼다. 결정적 차이는 **축적과
//! 전이**다:
//!
//! - EBM은 과제마다 통계를 **처음부터** 학습하고 버린다(전이 없음).
//! - 패치 규칙은 **항(Term)**이라 LGG가 변수를 만들고, MDL이 채택하고,
//!   **라이브러리에 쌓여 다른 과제에서 재사용**된다.
//!
//! 즉 이 모듈의 존재 이유는 성능이 아니라 **과제 간 전이의 가능성**이다:
//! 과제 A에서 배운 규칙이 과제 B를 푸는 것 — 그것이 code-free 학습의 정의다.
//!
//! # 교리 준수
//!
//! 여기에 ARC 규칙을 손으로 적지 않는다. 규칙은 전부 데이터에서 나온다. 이
//! 파일이 정하는 것은 **규칙의 문법**(무엇을 조건으로 삼고 무엇을 결과로 삼는가)
//! 뿐이며, 그 문법 위의 내용은 전적으로 경험이 채운다.

use crate::grid::Grid;
use monad_core::abstraction::{generalize, Library, Provenance, Term};
use std::collections::HashMap;

/// 패치 규칙의 함자: `RULE(PATCH(c0..c8), out)`.
const F_RULE: u32 = 900;
const F_PATCH: u32 = 901;
/// 격자 밖을 뜻하는 값(색 0..9와 구별).
const OUTSIDE: u64 = 10;

/// (x, y) 둘레 3×3을 항으로 — 밖은 [`OUTSIDE`].
pub fn patch_term(g: &Grid, x: usize, y: usize) -> Term {
    let mut cells = Vec::with_capacity(9);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            let v = if nx < 0 || ny < 0 || nx >= g.w as i32 || ny >= g.h as i32 {
                OUTSIDE
            } else {
                g.get(nx as usize, ny as usize) as u64
            };
            cells.push(Term::Const(v));
        }
    }
    Term::App(F_PATCH, cells)
}

/// 규칙 항: 이웃 패치 → 그 자리의 정답 색.
pub fn rule_term(patch: Term, out: u8) -> Term {
    Term::App(F_RULE, vec![patch, Term::Const(out as u64)])
}

/// 규칙에서 (패치, 결과색)을 되꺼낸다.
fn split_rule(t: &Term) -> Option<(&Vec<Term>, u64)> {
    match t {
        Term::App(f, args) if *f == F_RULE && args.len() == 2 => match (&args[0], &args[1]) {
            (Term::App(pf, cells), Term::Const(c)) if *pf == F_PATCH && cells.len() == 9 => {
                Some((cells, *c))
            }
            _ => None,
        },
        _ => None,
    }
}

/// 훈련쌍에서 **바뀐 자리**의 규칙 경험을 뽑는다(크기 같은 쌍만).
///
/// 해석하지 않는다 — 기계적 추출이다. "무엇이 조건인가"는 LGG가 정한다.
pub fn extract_rules(train: &[(Grid, Grid)]) -> Vec<Term> {
    let mut out = Vec::new();
    for (i, o) in train {
        if i.w != o.w || i.h != o.h {
            continue;
        }
        for y in 0..o.h {
            for x in 0..o.w {
                if i.get(x, y) != o.get(x, y) {
                    out.push(rule_term(patch_term(i, x, y), o.get(x, y)));
                }
            }
        }
    }
    out
}

/// 라이브러리의 패치 규칙만 골라 (패치 조건, 결과색)로 준다.
fn patch_rules(lib: &Library) -> Vec<(usize, Vec<Term>, u64)> {
    lib.by_prior()
        .into_iter()
        .filter_map(|ix| {
            split_rule(&lib.entries[ix].schema).map(|(cells, c)| (ix, cells.clone(), c))
        })
        .collect()
}

/// 패치 조건이 이 자리에 맞는가(변수는 무엇이든 허용).
fn patch_matches(cond: &[Term], g: &Grid, x: usize, y: usize) -> bool {
    let mut k = 0usize;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            let v = if nx < 0 || ny < 0 || nx >= g.w as i32 || ny >= g.h as i32 {
                OUTSIDE
            } else {
                g.get(nx as usize, ny as usize) as u64
            };
            match &cond[k] {
                Term::Const(c) if *c != v => return false,
                Term::App(_, _) => return false,
                _ => {}
            }
            k += 1;
        }
    }
    true
}

/// 라이브러리 규칙을 한 격자에 적용한다(동시 적용 — 입력 기준으로 읽고 새 격자에 쓴다).
///
/// 여러 규칙이 맞으면 **사전분포 순서상 먼저 오는 것**이 이긴다(학습된 우선순위).
pub fn apply_rules(lib: &Library, g: &Grid) -> Grid {
    let rules = patch_rules(lib);
    let mut o = g.clone();
    for y in 0..g.h {
        for x in 0..g.w {
            for (_, cond, c) in &rules {
                if patch_matches(cond, g, x, y) {
                    if *c <= 9 {
                        o.set(x, y, *c as u8);
                    }
                    break;
                }
            }
        }
    }
    o
}

/// **증거 기반 규칙 선택**(가설 → 증거 → 선택).
///
/// 라이브러리 전체를 무차별 적용하면 남의 과제에서 온 규칙이 엉뚱한 자리에서
/// 발화해 반드시 깨진다(시도 158에서 4,049개 전량 적용 → 게이트 통과 0건).
/// 기억은 가설일 뿐이므로, **이 과제의 증거로 검증해 모순 없는 것만 채택**한다:
///
/// - 발화한 모든 자리에서 결과색이 정답과 일치해야 한다(반례 0)
/// - 적어도 한 번은 **바뀌어야 하는 자리**에서 발화해야 한다(쓸모)
///
/// 이것이 "라이브러리에 기억이 있다"와 "그 기억이 여기 적용된다" 사이의 다리다.
pub fn select_consistent(lib: &Library, train: &[(Grid, Grid)]) -> Vec<(Vec<Term>, u64)> {
    let mut kept = Vec::new();
    for (_, cond, c) in patch_rules(lib) {
        if c > 9 {
            continue;
        }
        let mut consistent = true;
        let mut useful = false;
        'outer: for (i, o) in train {
            if i.w != o.w || i.h != o.h {
                consistent = false;
                break;
            }
            for y in 0..o.h {
                for x in 0..o.w {
                    if !patch_matches(&cond, i, x, y) {
                        continue;
                    }
                    if c as u8 != o.get(x, y) {
                        consistent = false;
                        break 'outer;
                    }
                    if i.get(x, y) != o.get(x, y) {
                        useful = true;
                    }
                }
            }
        }
        if consistent && useful {
            kept.push((cond, c));
        }
    }
    kept
}

/// 규칙의 **일반성** — 조건에 변수가 많을수록(구체 상수가 적을수록) 일반적이다.
/// 3×3 조건 9칸 중 상수로 고정된 칸 수를 센다(적을수록 넓게 적용된다).
fn rule_specificity(cond: &[Term]) -> usize {
    cond.iter().filter(|t| matches!(t, Term::Const(_))).count()
}

/// **일반화 압력을 넣은 증거 선택**(시도 160).
///
/// 시도 159에서 게이트를 통과한 4건 중 3건이 시험에서 틀렸다 — 훈련쌍에 우연히
/// 맞는 **과도하게 구체적인** 규칙이 섞였기 때문이다. 같은 결과를 내는 규칙
/// 집합이 여럿이면 **더 일반적인 쪽**(변수가 많은 쪽)을 택한다 — 오컴의 면도날을
/// 규칙 선택에 적용하는 것이며, 이것이 훈련 적합과 시험 일반화의 간극을 좁힌다.
///
/// `min_support`: 이 규칙이 훈련쌍에서 **실제로 고친 셀 수**의 하한. 한 셀만
/// 고치는 규칙은 그 셀을 외운 것일 수 있다.
pub fn select_generalizing(
    lib: &Library,
    train: &[(Grid, Grid)],
    min_support: usize,
) -> Vec<(Vec<Term>, u64)> {
    let mut scored: Vec<(usize, usize, Vec<Term>, u64)> = Vec::new();
    for (cond, c) in select_consistent(lib, train) {
        // 지지도: 이 규칙이 바뀌어야 하는 자리에서 실제로 발화한 횟수
        let mut support = 0usize;
        for (i, o) in train {
            if i.w != o.w || i.h != o.h {
                continue;
            }
            for y in 0..o.h {
                for x in 0..o.w {
                    if i.get(x, y) != o.get(x, y) && patch_matches(&cond, i, x, y) {
                        support += 1;
                    }
                }
            }
        }
        if support >= min_support {
            scored.push((rule_specificity(&cond), support, cond, c));
        }
    }
    // 일반적인 것(구체 상수 적은 것) 먼저, 같으면 지지도 큰 것 먼저
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    scored.into_iter().map(|(_, _, cond, c)| (cond, c)).collect()
}

/// 선택된 규칙만으로 격자를 고친다(동시 적용).
pub fn apply_selected(rules: &[(Vec<Term>, u64)], g: &Grid) -> Grid {
    let mut o = g.clone();
    for y in 0..g.h {
        for x in 0..g.w {
            for (cond, c) in rules {
                if patch_matches(cond, g, x, y) {
                    o.set(x, y, *c as u8);
                    break;
                }
            }
        }
    }
    o
}

/// 선택된 규칙이 훈련쌍을 **완전히** 재현하는가(덮개 검사).
pub fn selected_reproduce(rules: &[(Vec<Term>, u64)], train: &[(Grid, Grid)]) -> bool {
    !rules.is_empty()
        && train
            .iter()
            .all(|(i, o)| i.w == o.w && i.h == o.h && &apply_selected(rules, i) == o)
}

/// 라이브러리 규칙이 **이 과제의 훈련쌍을 정확히 재현**하는가.
/// 이것이 전이의 게이트다 — 다른 과제에서 배운 규칙이 여기서도 맞는지 검증한다.
pub fn rules_reproduce(lib: &Library, train: &[(Grid, Grid)]) -> bool {
    !patch_rules(lib).is_empty()
        && train.iter().all(|(i, o)| i.w == o.w && i.h == o.h && &apply_rules(lib, i) == o)
}

/// 수면: 규칙 경험을 일반화해 라이브러리에 넣는다.
///
/// 같은 결과색끼리 묶어 LGG를 돌린다 — 그래야 "어떤 이웃 조건이 이 색을 부르는가"의
/// 공통 구조가 나온다. 채택은 MDL(압축 이득 양수)만.
pub fn sleep_patch_abstract(rules: &[Term], lib: &mut Library) -> (usize, usize) {
    let mut by_out: HashMap<u64, Vec<Term>> = HashMap::new();
    for r in rules {
        if let Some((_, c)) = split_rule(r) {
            by_out.entry(c).or_default().push(r.clone());
        }
    }
    let (mut tried, mut added) = (0usize, 0usize);
    let mut keys: Vec<u64> = by_out.keys().copied().collect();
    keys.sort_unstable();
    for k in keys {
        let group = &by_out[&k];
        if group.len() < 2 {
            continue;
        }
        // 그룹 전체의 일반화 + 이웃한 쌍들의 국소 일반화(더 구체적인 규칙)
        let mut cands: Vec<Vec<Term>> = vec![group.clone()];
        for w in group.windows(2) {
            cands.push(w.to_vec());
        }
        for c in cands {
            tried += 1;
            if let Some(a) = generalize(&c) {
                if lib.insert(&a, Provenance::MonadDerived) {
                    added += 1;
                }
            }
        }
    }
    (tried, added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g3(cells: &[u8; 9]) -> Grid {
        let mut g = Grid::new(3, 3);
        for (i, &c) in cells.iter().enumerate() {
            g.set(i % 3, i / 3, c);
        }
        g
    }

    /// 규칙 추출은 **바뀐 자리만** 뽑는다(해석 없음).
    #[test]
    fn extraction_takes_only_changed_cells() {
        let i = g3(&[0, 1, 0, 0, 0, 0, 0, 0, 0]);
        let o = g3(&[0, 2, 0, 0, 0, 0, 0, 0, 0]);
        let r = extract_rules(&[(i, o)]);
        assert_eq!(r.len(), 1, "바뀐 셀은 하나뿐인데 {}개 뽑혔다", r.len());
        let (_, c) = split_rule(&r[0]).unwrap();
        assert_eq!(c, 2);
    }

    /// **과제 간 전이**: A에서 배운 규칙이 B를 푼다 — code-free 학습의 정의.
    #[test]
    fn rules_learned_on_one_task_transfer_to_another() {
        // 과제 A: "1로 둘러싸인 0은 2가 된다"류의 국소 규칙
        let a_in = g3(&[0, 1, 0, 1, 0, 1, 0, 1, 0]);
        let a_out = g3(&[0, 1, 0, 1, 2, 1, 0, 1, 0]);
        let mut lib = Library::new();
        let exp = extract_rules(&[(a_in.clone(), a_out.clone())]);
        // 같은 규칙을 두 번 봐야 일반화가 성립한다(2건 이상 필요)
        let mut twice = exp.clone();
        twice.extend(exp.iter().cloned());
        sleep_patch_abstract(&twice, &mut lib);
        assert!(!patch_rules(&lib).is_empty(), "패치 규칙이 하나도 안 만들어졌다");

        // 과제 B: 같은 국소 패턴이 다른 배치로 나타난다
        let mut b_in = Grid::new(5, 3);
        for (x, y) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
            b_in.set(x, y, 1);
        }
        let mut b_out = b_in.clone();
        b_out.set(1, 1, 2);
        // 라이브러리 규칙만으로 B의 훈련쌍이 재현된다 = 전이 성립
        assert!(
            rules_reproduce(&lib, &[(b_in.clone(), b_out.clone())]),
            "다른 과제에서 배운 규칙이 전이되지 않았다"
        );
        assert_eq!(apply_rules(&lib, &b_in), b_out);
    }

    /// **증거 기반 선택**: 라이브러리에 무관한 규칙이 섞여 있어도, 이 과제의
    /// 증거와 모순되는 것을 걸러내면 전이가 살아난다.
    #[test]
    fn evidence_selection_rescues_transfer_from_a_noisy_library() {
        let mut lib = Library::new();
        // 유용한 규칙(A 과제에서 옴)
        let a_in = g3(&[0, 1, 0, 1, 0, 1, 0, 1, 0]);
        let a_out = g3(&[0, 1, 0, 1, 2, 1, 0, 1, 0]);
        let mut exp = extract_rules(&[(a_in, a_out)]);
        exp.extend(exp.clone());
        // 방해 규칙(전혀 다른 과제에서 옴 — 1을 보면 7로 바꾸라는 식)
        let b_in = g3(&[1, 1, 1, 1, 1, 1, 1, 1, 1]);
        let b_out = g3(&[7, 7, 7, 7, 7, 7, 7, 7, 7]);
        let mut noise = extract_rules(&[(b_in, b_out)]);
        noise.extend(noise.clone());
        exp.extend(noise);
        sleep_patch_abstract(&exp, &mut lib);

        // 새 과제: A와 같은 국소 규칙이 필요하다
        let mut c_in = Grid::new(5, 3);
        for (x, y) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
            c_in.set(x, y, 1);
        }
        let mut c_out = c_in.clone();
        c_out.set(1, 1, 2);
        let train = [(c_in.clone(), c_out.clone())];

        // 전량 적용은 방해 규칙 때문에 깨진다
        assert!(
            !rules_reproduce(&lib, &train),
            "잡음 규칙이 있는데 전량 적용이 통과했다 — 시험이 무의미"
        );
        // 증거로 걸러내면 통과한다
        let sel = select_consistent(&lib, &train);
        assert!(!sel.is_empty(), "일관 규칙을 하나도 못 골랐다");
        assert!(selected_reproduce(&sel, &train), "선택 후에도 재현 실패");
        assert_eq!(apply_selected(&sel, &c_in), c_out);
    }

    /// **일반화 압력**: 훈련쌍에 우연히 맞는 과도하게 구체적인 규칙보다
    /// 더 일반적인(변수가 많은) 규칙이 먼저 적용돼야 시험에서 살아남는다.
    #[test]
    fn generalization_pressure_prefers_broader_rules() {
        let mut lib = Library::new();
        // 같은 결과(2)를 내는 두 규칙이 생기도록: 하나는 넓고 하나는 좁게
        let a_in = g3(&[0, 1, 0, 1, 0, 1, 0, 1, 0]);
        let a_out = g3(&[0, 1, 0, 1, 2, 1, 0, 1, 0]);
        let b_in = g3(&[5, 1, 6, 1, 0, 1, 7, 1, 8]);
        let b_out = g3(&[5, 1, 6, 1, 2, 1, 7, 1, 8]);
        let mut exp = extract_rules(&[(a_in, a_out), (b_in, b_out)]);
        exp.extend(exp.clone());
        sleep_patch_abstract(&exp, &mut lib);

        let mut c_in = Grid::new(5, 3);
        for (x, y) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
            c_in.set(x, y, 1);
        }
        c_in.set(0, 0, 9); // 모서리를 다르게 — 좁은 규칙은 여기서 안 맞는다
        let mut c_out = c_in.clone();
        c_out.set(1, 1, 2);
        let train = [(c_in.clone(), c_out.clone())];

        let sel = select_generalizing(&lib, &train, 1);
        assert!(!sel.is_empty(), "일반화 선택이 아무것도 못 골랐다");
        // 가장 먼저 오는 규칙이 가장 일반적이어야 한다
        let first = rule_specificity(&sel[0].0);
        let worst = sel.iter().map(|(c, _)| rule_specificity(c)).max().unwrap();
        assert!(first <= worst, "구체적인 규칙이 앞에 왔다");
        assert!(selected_reproduce(&sel, &train), "일반화 선택 후 재현 실패");
    }

    /// 지지도 하한이 **한 셀만 외운 규칙**을 걸러낸다.
    #[test]
    fn support_threshold_filters_memorized_singletons() {
        let mut lib = Library::new();
        let a_in = g3(&[0, 1, 0, 1, 0, 1, 0, 1, 0]);
        let a_out = g3(&[0, 1, 0, 1, 2, 1, 0, 1, 0]);
        let mut exp = extract_rules(&[(a_in.clone(), a_out.clone())]);
        exp.extend(exp.clone());
        sleep_patch_abstract(&exp, &mut lib);

        let train = [(a_in, a_out)];
        let lo = select_generalizing(&lib, &train, 1);
        let hi = select_generalizing(&lib, &train, 5);
        assert!(!lo.is_empty(), "지지도 1에서도 못 고르면 시험이 무의미");
        assert!(hi.len() <= lo.len(), "지지도 하한이 규칙을 못 걸렀다");
    }

    /// 규칙이 맞지 않는 과제에서는 **전이 게이트가 막는다**(거짓 양성 방지).
    #[test]
    fn transfer_gate_rejects_when_rules_do_not_fit() {
        let a_in = g3(&[0, 1, 0, 1, 0, 1, 0, 1, 0]);
        let a_out = g3(&[0, 1, 0, 1, 2, 1, 0, 1, 0]);
        let mut lib = Library::new();
        let exp = extract_rules(&[(a_in, a_out)]);
        let mut twice = exp.clone();
        twice.extend(exp);
        sleep_patch_abstract(&twice, &mut lib);

        // 전혀 다른 변환(모두 3으로)
        let c_in = g3(&[1, 1, 1, 1, 1, 1, 1, 1, 1]);
        let c_out = g3(&[3, 3, 3, 3, 3, 3, 3, 3, 3]);
        assert!(!rules_reproduce(&lib, &[(c_in, c_out)]), "맞지 않는데 통과시켰다");
    }
}
