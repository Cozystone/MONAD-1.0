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

/// **셀 문맥 특징**(시도 163) — 3×3 색만으로는 "어디에 적용할지"를 가릴 수 없다는
/// 진단(깨끗한 덮개 17.8%)의 처방.
///
/// 새 개념을 손으로 만들지 않는다. **동결 기질이 이미 가진 객체 분해**(연결 성분)
/// 에서 나오는 사실만 노출한다 — 어느 것이 조건으로 쓸모 있는지는 LGG·MDL이 정한다.
///
/// 특징: ①객체 안인가 ②객체 크기 순위(0=최대·1=중간·2=최소·3=객체 밖)
/// ③bbox 내 위치(0=모서리·1=가장자리·2=내부·3=밖) ④그 객체의 색 종류 수(0~2)
fn cell_features(g: &Grid, objs: &[crate::grid::Obj], x: usize, y: usize) -> [u64; 4] {
    let mut sizes: Vec<usize> = objs.iter().map(|o| o.area).collect();
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    let owner = objs.iter().find(|o| {
        x >= o.x0 && x < o.x0 + o.w && y >= o.y0 && y < o.y0 + o.h && {
            let (dx, dy) = (x - o.x0, y - o.y0);
            o.mask[dy * o.w + dx]
        }
    });
    match owner {
        None => [0, 3, 3, 0],
        Some(o) => {
            let rank = match sizes.iter().position(|&s| s == o.area).unwrap_or(0) {
                0 => 0,
                r if r + 1 == sizes.len() => 2,
                _ => 1,
            };
            let (dx, dy) = (x - o.x0, y - o.y0);
            let edge_x = dx == 0 || dx + 1 == o.w;
            let edge_y = dy == 0 || dy + 1 == o.h;
            let pos = match (edge_x, edge_y) {
                (true, true) => 0,
                (true, false) | (false, true) => 1,
                _ => 2,
            };
            let mut cs: Vec<u8> = o
                .colors
                .iter()
                .enumerate()
                .filter(|(i, _)| o.mask[*i])
                .map(|(_, &c)| c)
                .collect();
            cs.sort_unstable();
            cs.dedup();
            [1, rank as u64, pos as u64, (cs.len().min(3) - 1) as u64]
        }
    }
}

/// (x, y) 둘레 3×3 + **셀 문맥 특징 4종**을 항으로 — 밖은 [`OUTSIDE`].
/// 조건이 13칸이 되며, 그중 무엇이 실제 조건인지는 LGG가 정한다.
pub fn patch_term(g: &Grid, objs: &[crate::grid::Obj], x: usize, y: usize) -> Term {
    let mut cells = Vec::with_capacity(13);
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
    for f in cell_features(g, objs, x, y) {
        cells.push(Term::Const(f));
    }
    Term::App(F_PATCH, cells)
}

/// 규칙 항: 이웃 패치 → 그 자리의 정답 색.
pub fn rule_term(patch: Term, out: u8) -> Term {
    Term::App(F_RULE, vec![patch, Term::Const(out as u64)])
}

/// 규칙에서 (패치 조건, 결과 항)을 되꺼낸다. 결과는 상수 색일 수도, **조건의
/// 어느 자리와 공유된 변수**일 수도 있다("왼쪽 이웃의 색이 된다" 같은 규칙).
fn split_rule(t: &Term) -> Option<(&Vec<Term>, &Term)> {
    match t {
        Term::App(f, args) if *f == F_RULE && args.len() == 2 => match &args[0] {
            Term::App(pf, cells) if *pf == F_PATCH && cells.len() == 13 => {
                Some((cells, &args[1]))
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
        let objs = crate::grid::components(i);
        for y in 0..o.h {
            for x in 0..o.w {
                if i.get(x, y) != o.get(x, y) {
                    out.push(rule_term(patch_term(i, &objs, x, y), o.get(x, y)));
                }
            }
        }
    }
    out
}

/// 라이브러리의 패치 규칙만 골라 (패치 조건, 결과 항)로 준다.
fn patch_rules(lib: &Library) -> Vec<(usize, Vec<Term>, Term)> {
    lib.by_prior()
        .into_iter()
        .filter_map(|ix| {
            split_rule(&lib.entries[ix].schema).map(|(cells, c)| (ix, cells.clone(), c.clone()))
        })
        .collect()
}

/// 한 슬롯의 판정 + 변수 바인딩(선형 탐색 — 슬롯 13개라 HashMap보다 빠르다).
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

/// **바인딩 일관 발화**(시도 164의 교정). 성공하면 결과색을 돌려준다.
///
/// 이전 구현은 변수를 무조건 통과시켰다 — LGG가 발견한 "함께 변하는 자리는 같은
/// 변수"라는 구조를 **적용 단계에서 버리고** 있었던 것이다(그래서 규칙이 아무
/// 데서나 발화해 깨끗한 덮개가 18%에 머물렀다). 스키마의 의미론(subsumption)을
/// 그대로 시행한다: 같은 변수는 같은 값이어야 하고, 결과가 변수면 조건에서 묶인
/// 값이 결과색이 된다 — "이웃의 색이 된다"는 팔레트 독립 규칙이 이때 처음 가능해진다.
fn rule_fire(
    cond: &[Term],
    out: &Term,
    g: &Grid,
    objs: &[crate::grid::Obj],
    x: usize,
    y: usize,
) -> Option<u8> {
    let mut bind: Vec<(u32, u64)> = Vec::new();
    let mut k = 0usize;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            let v = if nx < 0 || ny < 0 || nx >= g.w as i32 || ny >= g.h as i32 {
                OUTSIDE
            } else {
                g.get(nx as usize, ny as usize) as u64
            };
            if !slot_ok(&cond[k], v, &mut bind) {
                return None;
            }
            k += 1;
        }
    }
    for f in cell_features(g, objs, x, y) {
        match cond.get(k) {
            Some(t) if slot_ok(t, f, &mut bind) => {}
            _ => return None,
        }
        k += 1;
    }
    let c = match out {
        Term::Const(c) => *c,
        Term::Var(v) => bind.iter().find(|(b, _)| b == v).map(|(_, val)| *val)?,
        Term::App(_, _) => return None,
    };
    (c <= 9).then_some(c as u8)
}

/// 라이브러리 규칙을 한 격자에 적용한다(동시 적용 — 입력 기준으로 읽고 새 격자에 쓴다).
///
/// 여러 규칙이 맞으면 **사전분포 순서상 먼저 오는 것**이 이긴다(학습된 우선순위).
pub fn apply_rules(lib: &Library, g: &Grid) -> Grid {
    let rules = patch_rules(lib);
    let objs = crate::grid::components(g);
    let mut o = g.clone();
    for y in 0..g.h {
        for x in 0..g.w {
            for (_, cond, out) in &rules {
                if let Some(c) = rule_fire(cond, out, g, &objs, x, y) {
                    o.set(x, y, c);
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
pub fn select_consistent(lib: &Library, train: &[(Grid, Grid)]) -> Vec<(Vec<Term>, Term)> {
    let objs_of: Vec<Vec<crate::grid::Obj>> =
        train.iter().map(|(i, _)| crate::grid::components(i)).collect();
    let mut kept = Vec::new();
    for (_, cond, out) in patch_rules(lib) {
        if matches!(out, Term::Const(c) if c > 9) || matches!(out, Term::App(_, _)) {
            continue;
        }
        let mut consistent = true;
        let mut useful = false;
        'outer: for (pi, (i, o)) in train.iter().enumerate() {
            if i.w != o.w || i.h != o.h {
                consistent = false;
                break;
            }
            for y in 0..o.h {
                for x in 0..o.w {
                    let Some(c) = rule_fire(&cond, &out, i, &objs_of[pi], x, y) else {
                        continue;
                    };
                    if c != o.get(x, y) {
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
            kept.push((cond, out));
        }
    }
    kept
}

/// 전이 실패의 분해에 쓰는 집계량.
#[derive(Clone, Copy, Debug, Default)]
pub struct CoverageStats {
    /// 바뀌어야 하는 셀 수.
    pub changed: usize,
    /// 그중 **맞는 색을 주장하는 규칙이 있는** 셀 수(덮개).
    pub covered: usize,
    /// 안 바뀌어야 하는데 **다른 색을 주장하는 규칙이 발화**한 셀 수(오발화).
    pub misfires: usize,
    /// 안 바뀌어야 하는 셀 수.
    pub unchanged: usize,
    /// **깨끗한 덮개**: 맞는 색을 주장하면서 이 과제에서 **한 번도 오발화하지 않는**
    /// 규칙이 있는 셀 수. 시도 162의 진단 결함 교정 — "맞는 규칙이 있다"와
    /// "쓸 수 있는 맞는 규칙이 있다"는 다르다. 이 값이 낮으면 선택이 아니라
    /// **문법의 표현력**이 병목이다(3×3으로는 적용 위치를 가릴 수 없다).
    pub clean_covered: usize,
}

/// 라이브러리 규칙이 이 과제를 얼마나 **덮는지**와 얼마나 **오발화하는지** 센다.
///
/// 선택·게이트를 거치지 않은 날것의 계량이다 — "왜 게이트를 못 넘는가"의 원인을
/// 덮개 부족과 오발화로 가르기 위한 것.
pub fn coverage_report(lib: &Library, train: &[(Grid, Grid)]) -> CoverageStats {
    let rules = patch_rules(lib);
    let mut s = CoverageStats::default();
    let objs_of: Vec<Vec<crate::grid::Obj>> =
        train.iter().map(|(i, _)| crate::grid::components(i)).collect();
    // 각 규칙이 이 과제에서 오발화하는지 먼저 판정한다(깨끗한 규칙 집합).
    let clean: Vec<bool> = rules
        .iter()
        .map(|(_, cond, out)| {
            train.iter().enumerate().all(|(pi, (i, o))| {
                if i.w != o.w || i.h != o.h {
                    return false;
                }
                (0..o.h).all(|y| {
                    (0..o.w).all(|x| match rule_fire(cond, out, i, &objs_of[pi], x, y) {
                        Some(c) => c == o.get(x, y),
                        None => true,
                    })
                })
            })
        })
        .collect();
    for (pi, (i, o)) in train.iter().enumerate() {
        if i.w != o.w || i.h != o.h {
            continue;
        }
        for y in 0..o.h {
            for x in 0..o.w {
                let want = o.get(x, y);
                let changed = i.get(x, y) != want;
                let mut has_right = false;
                let mut has_wrong = false;
                let mut has_clean_right = false;
                for (k, (_, cond, out)) in rules.iter().enumerate() {
                    if let Some(c) = rule_fire(cond, out, i, &objs_of[pi], x, y) {
                        if c == want {
                            has_right = true;
                            if clean[k] {
                                has_clean_right = true;
                            }
                        } else {
                            has_wrong = true;
                        }
                    }
                }
                if changed {
                    s.changed += 1;
                    if has_right {
                        s.covered += 1;
                    }
                    if has_clean_right {
                        s.clean_covered += 1;
                    }
                } else {
                    s.unchanged += 1;
                    if has_wrong {
                        s.misfires += 1;
                    }
                }
            }
        }
    }
    s
}

/// 규칙의 **일반성** — 조건에 변수가 많을수록(구체 상수가 적을수록) 일반적이다.
/// 3×3 조건 9칸 중 상수로 고정된 칸 수를 센다(적을수록 넓게 적용된다).
fn rule_specificity(cond: &[Term]) -> usize {
    cond.iter().filter(|t| matches!(t, Term::Const(_))).count()
}

/// **충돌 없는 탐욕 덮개 선택**(시도 162 — 분해가 지목한 처방).
///
/// 진단(시도 162): 홀드아웃 99건 전부 **덮개 100%**(바뀌어야 하는 셀마다 맞는
/// 규칙이 이미 있다) 인데 **95건이 오발화로 막혔다**(평균 341셀). 지식은 있고
/// **선택이 문제**였다.
///
/// 두 가지를 고친다:
///
/// 1. **구체적인 규칙이 먼저**(예외가 일반을 이긴다). 일반적인 규칙을 먼저 적용하면
///    넓게 발화해 오발화한다 — [`monad_core::schema::SchemaLib`]가 이미 쓰는 원리를
///    여기에도 적용한다. (시도 160은 반대로 정렬했고, 그래서 무효였다.)
/// 2. **충돌을 세며 하나씩 넣는다**. 바뀌어야 하는 셀을 아직 못 덮었으면, 그 셀을
///    맞게 덮으면서 **새 충돌을 가장 적게 만드는** 규칙을 고른다. 충돌이 0인 것만
///    받는다.
pub fn select_cover(
    lib: &Library,
    train: &[(Grid, Grid)],
    max_rules: usize,
) -> Vec<(Vec<Term>, Term)> {
    let mut cands: Vec<(Vec<Term>, Term)> = patch_rules(lib)
        .into_iter()
        .filter(|(_, _, out)| {
            !matches!(out, Term::Const(c) if *c > 9) && !matches!(out, Term::App(_, _))
        })
        .map(|(_, cond, out)| (cond, out))
        .collect();
    // 구체적인 것 먼저 — 예외가 일반을 이긴다
    cands.sort_by(|a, b| rule_specificity(&b.0).cmp(&rule_specificity(&a.0)));

    let mut chosen: Vec<(Vec<Term>, Term)> = Vec::new();
    for _ in 0..max_rules {
        // 현 선택으로 만들어지는 결과와 남은 결손을 센다
        let mut uncovered: Vec<(usize, usize, usize, u8)> = Vec::new(); // (쌍, x, y, 정답)
        let mut done = true;
        for (pi, (i, o)) in train.iter().enumerate() {
            if i.w != o.w || i.h != o.h {
                return Vec::new();
            }
            let cur = apply_selected(&chosen, i);
            for y in 0..o.h {
                for x in 0..o.w {
                    if cur.get(x, y) != o.get(x, y) {
                        done = false;
                        if i.get(x, y) != o.get(x, y) {
                            uncovered.push((pi, x, y, o.get(x, y)));
                        }
                    }
                }
            }
        }
        if done {
            return chosen;
        }
        let Some(&(pi, ux, uy, want)) = uncovered.first() else {
            // 결손은 있는데 "바뀌어야 하는 셀"이 아니다 = 이미 넣은 규칙이 망쳤다
            return Vec::new();
        };
        // 그 셀을 맞게 덮는 후보 중 **새 충돌이 가장 적은** 것
        let (ti, _to) = &train[pi];
        let objs_ti = crate::grid::components(ti);
        let mut best: Option<(usize, usize)> = None; // (충돌 수, 후보 색인)
        for (k, (cond, out)) in cands.iter().enumerate() {
            if rule_fire(cond, out, ti, &objs_ti, ux, uy) != Some(want) {
                continue;
            }
            let mut trial = chosen.clone();
            trial.insert(0, (cond.clone(), out.clone())); // 구체 규칙을 앞에 — 우선 적용
            let mut conflicts = 0usize;
            for (i, o) in train {
                let g = apply_selected(&trial, i);
                conflicts += (0..o.h)
                    .flat_map(|y| (0..o.w).map(move |x| (x, y)))
                    .filter(|&(x, y)| g.get(x, y) != o.get(x, y))
                    .count();
            }
            if best.map(|(bc, _)| conflicts < bc).unwrap_or(true) {
                best = Some((conflicts, k));
            }
            if conflicts == 0 {
                break;
            }
        }
        match best {
            Some((_, k)) => {
                let (cond, out) = cands.remove(k);
                chosen.insert(0, (cond, out));
            }
            None => return Vec::new(), // 이 셀을 덮을 방법이 없다
        }
    }
    chosen
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
) -> Vec<(Vec<Term>, Term)> {
    let objs_of: Vec<Vec<crate::grid::Obj>> =
        train.iter().map(|(i, _)| crate::grid::components(i)).collect();
    let mut scored: Vec<(usize, usize, Vec<Term>, Term)> = Vec::new();
    for (cond, out) in select_consistent(lib, train) {
        // 지지도: 이 규칙이 바뀌어야 하는 자리에서 실제로 발화한 횟수
        let mut support = 0usize;
        for (pi, (i, o)) in train.iter().enumerate() {
            if i.w != o.w || i.h != o.h {
                continue;
            }
            for y in 0..o.h {
                for x in 0..o.w {
                    if i.get(x, y) != o.get(x, y)
                        && rule_fire(&cond, &out, i, &objs_of[pi], x, y).is_some()
                    {
                        support += 1;
                    }
                }
            }
        }
        if support >= min_support {
            scored.push((rule_specificity(&cond), support, cond, out));
        }
    }
    // 일반적인 것(구체 상수 적은 것) 먼저, 같으면 지지도 큰 것 먼저
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    scored.into_iter().map(|(_, _, cond, out)| (cond, out)).collect()
}

/// 선택된 규칙만으로 격자를 고친다(동시 적용).
pub fn apply_selected(rules: &[(Vec<Term>, Term)], g: &Grid) -> Grid {
    let objs = crate::grid::components(g);
    let mut o = g.clone();
    for y in 0..g.h {
        for x in 0..g.w {
            for (cond, out) in rules {
                if let Some(c) = rule_fire(cond, out, g, &objs, x, y) {
                    o.set(x, y, c);
                    break;
                }
            }
        }
    }
    o
}

/// 선택된 규칙이 훈련쌍을 **완전히** 재현하는가(덮개 검사).
pub fn selected_reproduce(rules: &[(Vec<Term>, Term)], train: &[(Grid, Grid)]) -> bool {
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
        if let Some((_, Term::Const(c))) = split_rule(r) {
            by_out.entry(*c).or_default().push(r.clone());
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
    // **결과색을 가로지르는 일반화** — 같은 색끼리만 접으면 "결과 = 조건의 어느
    // 자리"라는 팔레트 독립 구조가 절대 나올 수 없다. 이웃쌍의 LGG에서 결과가
    // 조건과 함께 변하면 공유 변수가 되고(예: "이웃의 색이 된다"), 그 규칙만이
    // 팔레트가 다른 과제로 전이될 수 있다. 채택은 여전히 MDL.
    for w in rules.windows(2) {
        tried += 1;
        if let Some(a) = generalize(w) {
            if lib.insert(&a, Provenance::MonadDerived) {
                added += 1;
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
        assert_eq!(c, &Term::Const(2));
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

    /// **공유 변수 규칙의 팔레트 독립 전이**(시도 164의 핵심 시험).
    ///
    /// "0 셀이 균일한 색 X에 둘러싸이면 X가 된다"를 팔레트 {3}, {5} 과제에서
    /// 경험 → LGG가 여덟 이웃과 결과를 **같은 변수**로 접는다 → 한 번도 본 적
    /// 없는 팔레트 {7} 과제의 훈련쌍을 재현한다. 상수 결과 규칙으로는 원리상
    /// 불가능한 전이다. 동시에 바인딩 일관성 시험: 이웃이 섞인 격자에서는
    /// 발화하지 않아야 한다(이전 구현은 변수를 무조건 통과시켜 여기서 깨졌다).
    #[test]
    fn shared_variable_rules_transfer_across_palettes() {
        let uniform = |c: u8| {
            let mut g = Grid::new(3, 3);
            for y in 0..3 {
                for x in 0..3 {
                    g.set(x, y, c);
                }
            }
            g.set(1, 1, 0);
            g
        };
        let filled = |c: u8| {
            let mut g = Grid::new(3, 3);
            for y in 0..3 {
                for x in 0..3 {
                    g.set(x, y, c);
                }
            }
            g
        };
        // 팔레트 3, 5에서 경험 수집 → 수면
        let mut rules = extract_rules(&[(uniform(3), filled(3))]);
        rules.extend(extract_rules(&[(uniform(5), filled(5))]));
        let mut lib = Library::new();
        sleep_patch_abstract(&rules, &mut lib);
        // 결과가 변수인 규칙(팔레트 독립)이 실제로 만들어졌는가
        let has_var_out = lib
            .entries
            .iter()
            .any(|e| matches!(&e.schema, Term::App(_, args) if matches!(args.get(1), Some(Term::Var(_)))));
        assert!(has_var_out, "결과 변수 규칙이 수면에서 나오지 않았다");

        // 본 적 없는 팔레트 7 과제로 전이
        let train = [(uniform(7), filled(7))];
        let sel = select_consistent(&lib, &train);
        assert!(!sel.is_empty(), "팔레트 7에서 일관 규칙을 못 골랐다");
        assert!(selected_reproduce(&sel, &train), "팔레트 독립 전이 실패");

        // 바인딩 일관성: 이웃이 섞이면(7과 8) 발화하지 않는다
        let mut mixed = uniform(7);
        mixed.set(0, 0, 8);
        mixed.set(2, 2, 8);
        let out = apply_selected(&sel, &mixed);
        assert_eq!(out.get(1, 1), 0, "이웃이 균일하지 않은데 발화했다 — 바인딩 미검증");
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

    /// **충돌 없는 덮개 선택**: 오발화하는 넓은 규칙이 섞여 있어도, 구체적인 규칙을
    /// 먼저 놓고 충돌을 세며 고르면 훈련쌍을 정확히 재현한다(시도 162의 처방).
    #[test]
    fn cover_selection_beats_misfiring_broad_rules() {
        let mut lib = Library::new();
        // 좁고 정확한 규칙(십자 가운데를 2로)
        let a_in = g3(&[0, 1, 0, 1, 0, 1, 0, 1, 0]);
        let a_out = g3(&[0, 1, 0, 1, 2, 1, 0, 1, 0]);
        let mut exp = extract_rules(&[(a_in, a_out)]);
        exp.extend(exp.clone());
        // 넓게 오발화하는 규칙(0을 보면 무조건 7) — 다른 과제에서 왔다
        let b_in = g3(&[0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let b_out = g3(&[7, 7, 7, 7, 7, 7, 7, 7, 7]);
        let mut noise = extract_rules(&[(b_in, b_out)]);
        noise.extend(noise.clone());
        exp.extend(noise);
        sleep_patch_abstract(&exp, &mut lib);

        let mut c_in = Grid::new(5, 3);
        for (x, y) in [(1, 0), (0, 1), (2, 1), (1, 2)] {
            c_in.set(x, y, 1);
        }
        let mut c_out = c_in.clone();
        c_out.set(1, 1, 2);
        let train = [(c_in.clone(), c_out.clone())];

        // 전량 적용은 넓은 규칙 때문에 깨진다
        assert!(!rules_reproduce(&lib, &train), "잡음 규칙이 없다면 시험이 무의미");
        // 충돌 없는 덮개 선택은 통과한다
        let sel = select_cover(&lib, &train, 16);
        assert!(!sel.is_empty(), "덮개 선택이 아무것도 못 골랐다");
        assert!(selected_reproduce(&sel, &train), "덮개 선택 후에도 재현 실패");
        assert_eq!(apply_selected(&sel, &c_in), c_out);
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
