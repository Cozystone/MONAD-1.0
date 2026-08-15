//! M2-R — 경험 배선: 동결 솔버 → 경험 → 수면 추상화 → 재구체화.
//!
//! 이 모듈은 **새 풀이 어휘를 만들지 않는다.** 하는 일은 셋뿐이다:
//!
//! 1. 동결 솔버(`HUMAN_DERIVED`)가 푼 프로그램을 **항으로 기계 직렬화**해 경험
//!    저널에 적는다. (해석 없음 — 순수 인코딩)
//! 2. 수면에서 그 경험들을 [`monad_core::abstraction`]에 넘겨 **공통 구조를
//!    스스로 발견**시킨다(anti-unification + MDL). 산출 스키마는 `MONAD_DERIVED`.
//! 3. 다음 실행에서 **미해결 과제**에 그 스키마를 재구체화한다. 변수의 후보값은
//!    ①경험이 본 값 ②현 문제가 제공하는 값(팔레트·크기)에서 기계적으로 만든다.
//!
//! 사람이 과제를 판독하는 자리는 없다. 무엇이 변수인지는 LGG가 정하고, 채택
//! 여부는 MDL이 정하며, 어떤 스키마를 먼저 시도할지는 학습된 사전분포가 정한다.

use crate::arc_solve::{apply_grid_op_pub as apply_grid_op, GridOp};
use crate::grid::Grid;
use monad_core::abstraction::{
    generalize, Library, Provenance, Term,
};
use std::collections::HashMap;

/// 연쇄를 감싸는 함자.
const F_CHAIN: u32 = 0;
/// 연산 함자의 시작 오프셋(1..) — 태그 0은 연쇄 전용.
const F_OP_BASE: u32 = 1;

/// 연산 → 태그. **순서를 바꾸면 기존 저널·라이브러리가 무효가 된다**(추가만 할 것).
fn op_tag(op: &GridOp) -> u32 {
    use GridOp::*;
    let t = match op {
        FillEnclosed(_) => 0,
        SymFillH => 1,
        SymFillV => 2,
        Scale(_) => 3,
        Tile(_, _) => 4,
        TileMirror4 => 5,
        ExtractLargest => 6,
        ExtractUniqueColor => 7,
        ExtractContent => 8,
        ExtractFrameInterior => 9,
        ExtractBy(_) => 10,
        ExtractWindow(_, _, _) => 11,
        RemoveColor(_) => 12,
        PaletteMap(_) => 13,
        PeriodicRepair(_, _) => 14,
        ConnectPairs => 15,
        SymFillAll => 16,
        SymFillBBox => 17,
        RepeatToEdge(_) => 18,
        SymFillPatch => 19,
        SymFillPatchColor(_) => 20,
        PeriodicFill(_) => 21,
        PeriodicPatch(_) => 22,
        PanelSelect(_) => 23,
        PanelSummary(_) => 24,
        PanelPaint(_) => 25,
        Fractal(_) => 26,
        FractalRecolor => 27,
        ObjSymFill => 28,
        ScaleDown(_) => 29,
        SingleCell(_) => 30,
        SolidAnswer(_, _, _) => 31,
        RecolorBy(_, _) => 32,
        MarkLines(_, _) => 33,
        ConnectPairsColor(_) => 34,
        MarkIntersections(_) => 35,
        PeriodicDiagFill(_, _) => 36,
        SymFillColor(_, _) => 37,
        DiagRaysX => 38,
        Rot90 => 39,
        Rot180 => 40,
        Rot270 => 41,
        Transpose => 42,
        MirrorHGrid => 43,
        MirrorVGrid => 44,
    };
    F_OP_BASE + t
}

fn c(v: u8) -> Term {
    Term::Const(v as u64)
}

/// 연산 → 항(인자는 상수 리스트). 해석 없는 기계 인코딩.
pub fn op_to_term(op: &GridOp) -> Term {
    use GridOp::*;
    let args: Vec<Term> = match op {
        FillEnclosed(a) | Scale(a) | ExtractBy(a) | RemoveColor(a) | RepeatToEdge(a)
        | SymFillPatchColor(a) | PeriodicFill(a) | PeriodicPatch(a) | PanelSelect(a)
        | PanelSummary(a) | PanelPaint(a) | ScaleDown(a) | SingleCell(a)
        | ConnectPairsColor(a) | MarkIntersections(a) => vec![c(*a)],
        Tile(a, b) | PeriodicRepair(a, b) | MarkLines(a, b) | PeriodicDiagFill(a, b)
        | SymFillColor(a, b) => vec![c(*a), c(*b)],
        ExtractWindow(a, b, d) | SolidAnswer(a, b, d) => vec![c(*a), c(*b), c(*d)],
        Fractal(b) => vec![c(*b as u8)],
        PaletteMap(m) => m.iter().map(|x| c(*x)).collect(),
        RecolorBy(a, m) => {
            let mut v = vec![c(*a)];
            v.extend(m.iter().map(|x| c(*x)));
            v
        }
        _ => vec![],
    };
    Term::App(op_tag(op), args)
}

/// 항 → 연산(역연산). 인자 개수·범위가 맞지 않으면 None.
pub fn term_to_op(t: &Term) -> Option<GridOp> {
    use GridOp::*;
    let Term::App(f, args) = t else { return None };
    let tag = f.checked_sub(F_OP_BASE)?;
    let a = |i: usize| -> Option<u8> {
        match args.get(i)? {
            Term::Const(v) if *v <= u8::MAX as u64 => Some(*v as u8),
            _ => None,
        }
    };
    let map10 = |off: usize| -> Option<[u8; 10]> {
        let mut m = [0u8; 10];
        for (i, slot) in m.iter_mut().enumerate() {
            *slot = a(off + i)?;
        }
        Some(m)
    };
    Some(match tag {
        0 => FillEnclosed(a(0)?),
        1 => SymFillH,
        2 => SymFillV,
        3 => Scale(a(0)?),
        4 => Tile(a(0)?, a(1)?),
        5 => TileMirror4,
        6 => ExtractLargest,
        7 => ExtractUniqueColor,
        8 => ExtractContent,
        9 => ExtractFrameInterior,
        10 => ExtractBy(a(0)?),
        11 => ExtractWindow(a(0)?, a(1)?, a(2)?),
        12 => RemoveColor(a(0)?),
        13 => PaletteMap(map10(0)?),
        14 => PeriodicRepair(a(0)?, a(1)?),
        15 => ConnectPairs,
        16 => SymFillAll,
        17 => SymFillBBox,
        18 => RepeatToEdge(a(0)?),
        19 => SymFillPatch,
        20 => SymFillPatchColor(a(0)?),
        21 => PeriodicFill(a(0)?),
        22 => PeriodicPatch(a(0)?),
        23 => PanelSelect(a(0)?),
        24 => PanelSummary(a(0)?),
        25 => PanelPaint(a(0)?),
        26 => Fractal(a(0)? != 0),
        27 => FractalRecolor,
        28 => ObjSymFill,
        29 => ScaleDown(a(0)?),
        30 => SingleCell(a(0)?),
        31 => SolidAnswer(a(0)?, a(1)?, a(2)?),
        32 => RecolorBy(a(0)?, map10(1)?),
        33 => MarkLines(a(0)?, a(1)?),
        34 => ConnectPairsColor(a(0)?),
        35 => MarkIntersections(a(0)?),
        36 => PeriodicDiagFill(a(0)?, a(1)?),
        37 => SymFillColor(a(0)?, a(1)?),
        38 => DiagRaysX,
        39 => Rot90,
        40 => Rot180,
        41 => Rot270,
        42 => Transpose,
        43 => MirrorHGrid,
        44 => MirrorVGrid,
        _ => return None,
    })
}

/// 연쇄 ↔ 항.
pub fn chain_to_term(ops: &[GridOp]) -> Term {
    Term::App(F_CHAIN, ops.iter().map(op_to_term).collect())
}

pub fn term_to_chain(t: &Term) -> Option<Vec<GridOp>> {
    let Term::App(F_CHAIN, args) = t else { return None };
    args.iter().map(term_to_op).collect()
}

// ------------------------------------------------------------------ 경험 저널

/// 경험 한 줄: 어떤 과제를 어떤 프로그램으로 풀었는가(항 직렬화).
pub fn append_experience(path: impl AsRef<std::path::Path>, task: &str, program: &Term) {
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            f,
            "{task}\t{}",
            monad_core::abstraction::write_term(program)
        );
    }
}

/// 저널 적재(과제명, 프로그램 항).
pub fn load_experience(path: impl AsRef<std::path::Path>) -> Vec<(String, Term)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let (t, p) = l.split_once('\t')?;
            Some((t.to_string(), monad_core::abstraction::read_term(p)?))
        })
        .collect()
}

// ------------------------------------------------------------------ 수면(추상화)

/// **수면 패스**: 경험들의 공통 구조를 스스로 발견해 라이브러리에 축적한다.
///
/// 사람의 분류 없이 진행한다 — 구조가 같은 것끼리는 LGG가 실질적 일반화를
/// 내놓고, 다른 것끼리는 통째 변수(=구조 없음)가 되어 MDL이 거른다.
/// 반환: (시도한 조합 수, 새로 채택된 스키마 수).
pub fn sleep_abstract(experience: &[(String, Term)], lib: &mut Library) -> (usize, usize) {
    let mut tried = 0usize;
    let mut added = 0usize;
    // 2개 조합: 가장 구체적인 일반화(가장 많은 구조를 남긴다)
    for i in 0..experience.len() {
        for j in (i + 1)..experience.len() {
            tried += 1;
            if let Some(abs) = generalize(&[experience[i].1.clone(), experience[j].1.clone()]) {
                if lib.insert(&abs, Provenance::MonadDerived) {
                    added += 1;
                }
            }
        }
    }
    // 3개 조합: 더 넓은 스키마(변수 늘고 이득 줄지만 재사용 폭이 크다).
    // 비용 보호를 위해 앞쪽 경험으로 제한.
    let n = experience.len().min(24);
    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                tried += 1;
                if let Some(abs) = generalize(&[
                    experience[i].1.clone(),
                    experience[j].1.clone(),
                    experience[k].1.clone(),
                ]) {
                    if lib.insert(&abs, Provenance::MonadDerived) {
                        added += 1;
                    }
                }
            }
        }
    }
    (tried, added)
}

// ------------------------------------------------------------------ 재구체화

/// 변수 후보값: ①경험이 본 값 ②현 문제가 제공하는 값(팔레트·크기·작은 정수).
/// 전부 기계적 — 과제 판독이 아니다.
fn candidates(lib: &Library, ix: usize, var: u32, task_palette: &[u8], dims: &[u8]) -> Vec<Term> {
    let mut out = lib.observed(ix, var);
    for &c0 in task_palette {
        let t = Term::Const(c0 as u64);
        if !out.contains(&t) {
            out.push(t);
        }
    }
    for &d in dims {
        let t = Term::Const(d as u64);
        if !out.contains(&t) {
            out.push(t);
        }
    }
    for v in 0u64..=4 {
        let t = Term::Const(v);
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// 재구체화 결과(회계용).
#[derive(Default, Debug, Clone, Copy)]
pub struct ReuseReport {
    /// 스키마를 시험한 횟수.
    pub tries: u32,
    /// 훈련쌍을 정확히 재현해 채택된 횟수.
    pub hits: u32,
    /// 그중 **경험에 없던 대입**으로 성공한 횟수(신규 재구체화).
    pub novel: u32,
    /// 정답 도달 전에 검사한 후보 수(탐색 비용 — 사전분포 효과 측정).
    pub probes: u32,
}

/// **미해결 과제에 라이브러리 스키마를 재구체화한다.**
///
/// 학습된 사전분포 순서로 스키마를 꺼내, 변수에 후보값을 채워 훈련쌍을 정확히
/// 재현하는 조합을 찾는다. 찾으면 그 프로그램을 시험 입력에 적용한다.
/// 라이브러리의 시도·성공 이력이 갱신된다(다음 실행의 사전분포가 된다).
pub fn reinstantiate(
    lib: &mut Library,
    train: &[(Grid, Grid)],
    budget: u32,
) -> (Option<Vec<GridOp>>, ReuseReport) {
    let mut rep = ReuseReport::default();
    let mut palette: Vec<u8> = Vec::new();
    for (i, o) in train {
        for g in [i, o] {
            for &v in &g.cells {
                if !palette.contains(&v) {
                    palette.push(v);
                }
            }
        }
    }
    palette.sort_unstable();
    let dims: Vec<u8> = train
        .first()
        .map(|(i, o)| vec![i.w.min(255) as u8, i.h.min(255) as u8, o.w.min(255) as u8, o.h.min(255) as u8])
        .unwrap_or_default();

    for ix in lib.by_prior() {
        if rep.probes >= budget {
            break;
        }
        let schema = lib.entries[ix].schema.clone();
        let vars = schema.vars();
        if vars.len() > 3 {
            continue; // 후보 폭발 방지(변수 4개 이상은 이번 예산에서 제외)
        }
        rep.tries += 1;
        lib.entries[ix].tries = lib.entries[ix].tries.saturating_add(1);

        // 변수별 후보 목록의 데카르트 곱을 예산 안에서 훑는다
        let cand: Vec<Vec<Term>> = vars
            .iter()
            .map(|&v| candidates(lib, ix, v, &palette, &dims))
            .collect();
        let mut idx = vec![0usize; vars.len()];
        loop {
            if rep.probes >= budget {
                break;
            }
            rep.probes += 1;
            let mut b: HashMap<u32, Term> = HashMap::new();
            for (vi, &v) in vars.iter().enumerate() {
                b.insert(v, cand[vi][idx[vi]].clone());
            }
            let filled = schema.substitute(&b);
            if let Some(ops) = term_to_chain(&filled) {
                let ok = !ops.is_empty()
                    && train.iter().all(|(i, o)| {
                        let mut g = i.clone();
                        for op in &ops {
                            g = apply_grid_op(&g, *op);
                        }
                        &g == o
                    });
                if ok {
                    rep.hits += 1;
                    lib.entries[ix].wins = lib.entries[ix].wins.saturating_add(1);
                    if lib.is_novel(ix, &b) {
                        rep.novel += 1;
                    }
                    return (Some(ops), rep);
                }
            }
            // 다음 조합
            let mut carry = 0usize;
            while carry < vars.len() {
                idx[carry] += 1;
                if idx[carry] < cand[carry].len() {
                    break;
                }
                idx[carry] = 0;
                carry += 1;
            }
            if carry == vars.len() {
                break;
            }
        }
    }
    (None, rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 인코딩 왕복: 모든 인자 형태(0·1·2·3개, 10색 사상)가 무손실.
    #[test]
    fn program_encoding_round_trips() {
        let progs = vec![
            GridOp::Rot180,
            GridOp::Scale(3),
            GridOp::Tile(2, 3),
            GridOp::ExtractWindow(3, 3, 1),
            GridOp::PaletteMap([0, 2, 1, 3, 4, 5, 6, 7, 8, 9]),
            GridOp::RecolorBy(1, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            GridOp::Fractal(true),
        ];
        for p in &progs {
            let t = op_to_term(p);
            assert_eq!(term_to_op(&t).as_ref(), Some(p), "왕복 실패: {p:?}");
        }
        let chain = chain_to_term(&progs);
        assert_eq!(term_to_chain(&chain).as_ref(), Some(&progs));
    }

    /// 수면이 두 경험에서 **매개변수 자리를 스스로 찾아낸다**(사람 지정 없음).
    #[test]
    fn sleep_discovers_the_parameter_slot() {
        let exp = vec![
            ("a".into(), chain_to_term(&[GridOp::Scale(2)])),
            ("b".into(), chain_to_term(&[GridOp::Scale(3)])),
        ];
        let mut lib = Library::new();
        let (tried, added) = sleep_abstract(&exp, &mut lib);
        assert!(tried > 0);
        assert_eq!(added, 1, "압축하는 스키마가 하나 나와야 한다");
        let e = &lib.entries[0];
        assert_eq!(e.provenance, Provenance::MonadDerived);
        assert_eq!(e.schema.vars().len(), 1, "배율 자리가 변수화돼야 한다");
        assert!(e.gain > 0);
    }

    /// **핵심 시험**: 경험에 없던 값으로 재구체화해 새 문제를 푼다.
    /// (경험은 Scale(2)·Scale(3)뿐, 문제는 4배 — 코드 추가 없이 일반화로 해결)
    #[test]
    fn reinstantiation_solves_with_a_value_never_experienced() {
        let exp = vec![
            ("a".into(), chain_to_term(&[GridOp::Scale(2)])),
            ("b".into(), chain_to_term(&[GridOp::Scale(3)])),
        ];
        let mut lib = Library::new();
        sleep_abstract(&exp, &mut lib);

        // 4배 확대 과제(경험에 없는 배율)
        let mut i1 = Grid::new(2, 2);
        i1.set(0, 0, 1);
        i1.set(1, 1, 2);
        let o1 = apply_grid_op(&i1, GridOp::Scale(4));
        let mut i2 = Grid::new(2, 2);
        i2.set(1, 0, 3);
        let o2 = apply_grid_op(&i2, GridOp::Scale(4));
        let mut t = Grid::new(2, 2);
        t.set(0, 1, 5);
        let expected = apply_grid_op(&t, GridOp::Scale(4));

        let (ops, rep) = reinstantiate(&mut lib, &[(i1, o1), (i2, o2)], 500);
        let got = ops.map(|ops| {
            let mut g = t.clone();
            for op in &ops {
                g = apply_grid_op(&g, *op);
            }
            g
        });
        assert_eq!(got.as_ref(), Some(&expected), "재구체화로 못 풀었다");
        assert_eq!(rep.hits, 1);
        assert_eq!(rep.novel, 1, "경험에 없던 대입이어야 한다");
        assert_eq!(lib.entries[0].wins, 1, "성공 이력이 기록돼야 한다");
    }

    /// 성공 이력이 다음 탐색의 순서를 바꾼다(탐색 감소의 기전).
    #[test]
    fn learned_prior_reorders_search() {
        let mut lib = Library::new();
        let a = generalize(&[
            chain_to_term(&[GridOp::Scale(2)]),
            chain_to_term(&[GridOp::Scale(3)]),
        ])
        .unwrap();
        let b = generalize(&[
            chain_to_term(&[GridOp::RemoveColor(1)]),
            chain_to_term(&[GridOp::RemoveColor(2)]),
        ])
        .unwrap();
        lib.insert(&a, Provenance::MonadDerived);
        lib.insert(&b, Provenance::MonadDerived);
        assert_eq!(lib.by_prior()[0], 0, "동률이면 삽입 순서");
        lib.entries[1].tries = 3;
        lib.entries[1].wins = 3;
        assert_eq!(lib.by_prior()[0], 1, "성공한 스키마가 먼저 시도돼야 한다");
    }
}
