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

/// 중간 격자의 상한(셀 수). ARC 최대는 30×30=900이므로 확대 중간 단계까지
/// 넉넉하다. 재구체화가 확대 연산에 큰 값을 넣으면 크기가 곱으로 폭발하므로
/// (실측: 656GB 할당 시도) **적용 전에** 예측 크기로 막는다.
pub const MAX_CELLS: usize = 60_000;

/// 자원 가드가 붙은 연쇄 적용. 상한을 넘으면 적용하지 않고 None.
pub fn safe_chain(g: &Grid, ops: &[GridOp]) -> Option<Grid> {
    let mut cur = g.clone();
    for op in ops {
        // 크기를 키우는 연산만 배율이 1을 넘는다(나머지는 보존하거나 줄인다)
        let (mx, my) = match op {
            GridOp::Scale(k) => (*k as usize, *k as usize),
            GridOp::Tile(a, b) => (*a as usize, *b as usize),
            GridOp::TileMirror4 => (2, 2),
            GridOp::Fractal(_) | GridOp::FractalRecolor => (cur.w, cur.h),
            _ => (1, 1),
        };
        let cells = cur
            .w
            .saturating_mul(mx)
            .saturating_mul(cur.h.saturating_mul(my));
        if cells == 0 || cells > MAX_CELLS {
            return None;
        }
        cur = apply_grid_op(&cur, *op);
        if cur.w == 0 || cur.h == 0 || cur.w * cur.h > MAX_CELLS {
            return None;
        }
    }
    Some(cur)
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

/// 색을 격자에 **쓰는** 인자는 0..=9여야 한다. 재구체화가 크기 같은 값을 색
/// 자리에 넣으면 격자에 10 이상의 셀이 생겨 이후 연산이 무너진다(실측 패닉).
/// 디코더가 그 자리에서 막는다 — 동결 솔버는 손대지 않는다.
fn color_ok(op: &GridOp) -> bool {
    use GridOp::*;
    let c9 = |c: u8| c <= 9;
    match op {
        FillEnclosed(c) | RemoveColor(c) | SymFillPatchColor(c) | PeriodicFill(c)
        | PeriodicPatch(c) | ConnectPairsColor(c) | MarkIntersections(c) => c9(*c),
        MarkLines(_, c) | SymFillColor(_, c) => c9(*c),
        PaletteMap(m) => m.iter().all(|&c| c9(c)),
        RecolorBy(_, m) => m.iter().all(|&c| c9(c)),
        _ => true,
    }
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
    let op = match tag {
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
    };
    if color_ok(&op) {
        Some(op)
    } else {
        None
    }
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
                    && train
                        .iter()
                        .all(|(i, o)| safe_chain(i, &ops).as_ref() == Some(o));
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

/// 한 스키마의 모든 대입을 훑어, 훈련쌍에 적용한 **중간 결과**를 돌려준다.
/// (합성의 1단계 — 잔차를 만들어 다음 스키마에 넘긴다.)
fn instantiations(
    lib: &Library,
    ix: usize,
    train: &[(Grid, Grid)],
    palette: &[u8],
    dims: &[u8],
    cap: usize,
) -> Vec<(Vec<GridOp>, Vec<Grid>)> {
    let schema = lib.entries[ix].schema.clone();
    let vars = schema.vars();
    if vars.len() > 2 {
        return Vec::new();
    }
    let cand: Vec<Vec<Term>> = vars
        .iter()
        .map(|&v| candidates(lib, ix, v, palette, dims))
        .collect();
    let mut out = Vec::new();
    let mut idx = vec![0usize; vars.len()];
    loop {
        if out.len() >= cap {
            break;
        }
        let mut b: HashMap<u32, Term> = HashMap::new();
        for (vi, &v) in vars.iter().enumerate() {
            b.insert(v, cand[vi][idx[vi]].clone());
        }
        if let Some(ops) = term_to_chain(&schema.substitute(&b)) {
            if !ops.is_empty() {
                let mids: Option<Vec<Grid>> =
                    train.iter().map(|(i, _)| safe_chain(i, &ops)).collect();
                // 아무것도 바꾸지 않는 대입은 합성에 무의미(자원 초과분도 제외)
                if let Some(mids) = mids {
                    if mids.iter().zip(train.iter()).any(|(m, (i, _))| m != i) {
                        out.push((ops, mids));
                    }
                }
            }
        }
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
    out
}

/// **스키마 합성**(자기학습 루프의 5단계): 스키마 하나로 안 닫히면 둘을 잇는다.
///
/// 기저 솔버의 연쇄 탐색은 **탐욕적**(각 단계에서 첫 적합만 채택)이라 조합을
/// 놓친다. 여기서는 A의 각 대입이 만든 **잔차**(중간 격자 → 정답)에 대해 B를
/// 다시 찾는다 — 기저가 구조적으로 못 훑는 공간이다.
pub fn reinstantiate_compose(
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
        .map(|(i, o)| {
            vec![i.w.min(255) as u8, i.h.min(255) as u8, o.w.min(255) as u8, o.h.min(255) as u8]
        })
        .unwrap_or_default();

    let order = lib.by_prior();
    for &ia in &order {
        if rep.probes >= budget {
            break;
        }
        let firsts = instantiations(lib, ia, train, &palette, &dims, 24);
        for (ops_a, mids) in firsts {
            if rep.probes >= budget {
                break;
            }
            // 잔차: (중간, 정답) — 여기에 두 번째 스키마를 찾는다
            let residual: Vec<(Grid, Grid)> = mids
                .iter()
                .cloned()
                .zip(train.iter().map(|(_, o)| o.clone()))
                .collect();
            for &ib in &order {
                if rep.probes >= budget {
                    break;
                }
                rep.tries += 1;
                let seconds = instantiations(lib, ib, &residual, &palette, &dims, 24);
                for (ops_b, outs) in seconds {
                    rep.probes += 1;
                    if outs.iter().zip(train.iter()).all(|(g, (_, o))| g == o) {
                        rep.hits += 1;
                        lib.entries[ia].wins = lib.entries[ia].wins.saturating_add(1);
                        lib.entries[ib].wins = lib.entries[ib].wins.saturating_add(1);
                        rep.novel += 1; // 합성은 정의상 경험에 없던 구성
                        let mut ops = ops_a.clone();
                        ops.extend(ops_b);
                        return (Some(ops), rep);
                    }
                }
            }
        }
    }
    (None, rep)
}

/// 잔차 = 훈련쌍 전체의 셀 불일치 비율(크기가 다르면 완전 불일치로 본다).
/// 교사는 이 값이 0인 경우만 성공으로 세고 나머지를 버린다 — 우리는 남긴다.
pub fn residual(train: &[(Grid, Grid)], ops: &[GridOp]) -> f64 {
    let mut bad = 0usize;
    let mut total = 0usize;
    for (i, o) in train {
        let g = match safe_chain(i, ops) {
            Some(g) => g,
            None => return 1.0,
        };
        total += o.cells.len();
        if g.w != o.w || g.h != o.h {
            bad += o.cells.len();
        } else {
            bad += g.cells.iter().zip(o.cells.iter()).filter(|(a, b)| a != b).count();
        }
    }
    if total == 0 {
        1.0
    } else {
        bad as f64 / total as f64
    }
}

/// **부분 진전 경험**: 문제를 닫지는 못했지만 잔차를 줄인 스키마 대입.
///
/// 교사(동결 솔버)는 정확 재현만 성공으로 치고 나머지를 폐기한다. 그래서 교사의
/// 성공만 기록하면 학생의 가설 공간은 교사의 탐색 공간을 넘지 못한다(시도 150의
/// 인수분해). 여기서는 **실패했지만 진전한 시도**를 남긴다 — 이것이 교사가
/// 탐욕적 첫-적합 때문에 영영 조합해 보지 않는 부분해들의 원료다.
///
/// 반환: (연산열, 원래 잔차, 줄어든 잔차, **효과 프로파일**) — 개선이 없으면 None.
///
/// 효과 프로파일이 함께 나오는 이유: 잔차 비율 하나로는 "많이 고치고 조금
/// 망가뜨림"과 "조용히 고침"이 구별되지 않는다. 수면이 **부분 목표 스키마**
/// ("이 도구는 이런 잔차를 없앤다")를 만들려면 그 구분이 필요하다.
pub fn probe_partial(
    lib: &Library,
    train: &[(Grid, Grid)],
    budget: u32,
) -> Option<(Vec<GridOp>, f64, f64, EffectProfile)> {
    let base = residual(train, &[]);
    if base <= 0.0 {
        return None;
    }
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
        .map(|(i, o)| {
            vec![i.w.min(255) as u8, i.h.min(255) as u8, o.w.min(255) as u8, o.h.min(255) as u8]
        })
        .unwrap_or_default();

    let mut best: Option<(Vec<GridOp>, f64)> = None;
    let mut used = 0u32;
    for ix in lib.by_prior() {
        if used >= budget {
            break;
        }
        for (ops, _) in instantiations(lib, ix, train, &palette, &dims, 16) {
            used += 1;
            if used >= budget {
                break;
            }
            let r = residual(train, &ops);
            // 유의미한 진전만(잡음 방지: 5% 이상 감소)
            if r < base * 0.95 && best.as_ref().map(|(_, br)| r < *br).unwrap_or(true) {
                best = Some((ops, r));
            }
        }
    }
    best.map(|(ops, r)| {
        let before: Vec<Grid> = train.iter().map(|(i, _)| i.clone()).collect();
        let after: Vec<Grid> = train
            .iter()
            .map(|(i, _)| safe_chain(i, &ops).unwrap_or_else(|| i.clone()))
            .collect();
        let prof = effect_profile(train, &before, &after);
        (ops, base, r, prof)
    })
}

/// **유도 합성**: 부분 진전이 확인된 연산열을 1단계로 고정하고 마무리만 찾는다.
///
/// 맹목 합성(모든 스키마 × 모든 스키마)과 달리, 1단계 후보가 "잔차를 줄인다"는
/// 증거로 이미 걸러져 있다. 이것이 **경험이 탐색 공간을 줄이는** 지점이다.
pub fn compose_guided(
    lib: &mut Library,
    train: &[(Grid, Grid)],
    seeds: &[Vec<GridOp>],
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
        .map(|(i, o)| {
            vec![i.w.min(255) as u8, i.h.min(255) as u8, o.w.min(255) as u8, o.h.min(255) as u8]
        })
        .unwrap_or_default();

    let order = lib.by_prior();
    for seed in seeds {
        if rep.probes >= budget {
            break;
        }
        // 1단계를 적용한 잔차 상태
        let mids: Option<Vec<Grid>> = train.iter().map(|(i, _)| safe_chain(i, seed)).collect();
        let Some(mids) = mids else { continue };
        let residual_pairs: Vec<(Grid, Grid)> = mids
            .into_iter()
            .zip(train.iter().map(|(_, o)| o.clone()))
            .collect();
        for &ib in &order {
            if rep.probes >= budget {
                break;
            }
            rep.tries += 1;
            for (ops_b, outs) in instantiations(lib, ib, &residual_pairs, &palette, &dims, 24) {
                rep.probes += 1;
                if outs.iter().zip(train.iter()).all(|(g, (_, o))| g == o) {
                    rep.hits += 1;
                    rep.novel += 1;
                    lib.entries[ib].wins = lib.entries[ib].wins.saturating_add(1);
                    let mut ops = seed.clone();
                    ops.extend(ops_b);
                    return (Some(ops), rep);
                }
            }
        }
    }
    (None, rep)
}

/// 중간 상태의 정규형 지문 — **같은 결과를 내는 서로 다른 연산열은 같은 상태**다.
/// 이것이 "동일 효과 스키마의 정규화"이자 실패 조합 메모이제이션의 키가 된다.
fn state_key(mids: &[Grid]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for g in mids {
        h = h.wrapping_mul(0x100000001b3) ^ g.w as u64;
        h = h.wrapping_mul(0x100000001b3) ^ g.h as u64;
        for &c in &g.cells {
            h = h.wrapping_mul(0x100000001b3) ^ c as u64;
        }
    }
    h
}

/// 잔차(불일치 비율)를 중간 상태에서 직접 잰다.
fn residual_mids(mids: &[Grid], train: &[(Grid, Grid)]) -> f64 {
    let mut bad = 0usize;
    let mut total = 0usize;
    for (g, (_, o)) in mids.iter().zip(train.iter()) {
        total += o.cells.len();
        if g.w != o.w || g.h != o.h {
            bad += o.cells.len();
        } else {
            bad += g.cells.iter().zip(o.cells.iter()).filter(|(a, b)| a != b).count();
        }
    }
    if total == 0 {
        1.0
    } else {
        bad as f64 / total as f64
    }
}

/// 한 연산이 잔차에 **무엇을 했는가** — 고친 셀과 망가뜨린 셀을 나눠 본다.
///
/// 잔차 비율 하나로는 "70칸 고치고 15칸 망가뜨림"과 "55칸 조용히 고침"이 같아
/// 보인다. 둘은 전혀 다른 도구다. 이 구분이 있어야 수면이 **부분 목표 스키마**
/// ("이 도구는 이런 종류의 잔차를 없앤다")를 만들 수 있다.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EffectProfile {
    /// 틀렸던 셀이 맞게 된 수.
    pub corrected: usize,
    /// 맞았던 셀이 틀리게 된 수(부작용).
    pub damaged: usize,
    /// 적용 전 틀린 셀 수.
    pub before_wrong: usize,
    /// 적용 후 틀린 셀 수.
    pub after_wrong: usize,
}

impl EffectProfile {
    pub fn net(&self) -> i64 {
        self.corrected as i64 - self.damaged as i64
    }
    /// 부작용 없이 고치는 도구인가(정밀도) — 부분 목표 스키마의 핵심 성질.
    pub fn precision(&self) -> f64 {
        let t = self.corrected + self.damaged;
        if t == 0 {
            0.0
        } else {
            self.corrected as f64 / t as f64
        }
    }
}

/// 적용 전/후 상태를 정답과 대조해 효과를 분해한다.
pub fn effect_profile(
    train: &[(Grid, Grid)],
    before: &[Grid],
    after: &[Grid],
) -> EffectProfile {
    let mut p = EffectProfile::default();
    for ((b, a), (_, o)) in before.iter().zip(after.iter()).zip(train.iter()) {
        for y in 0..o.h {
            for x in 0..o.w {
                let want = o.get(x, y);
                let had = if b.w == o.w && b.h == o.h { Some(b.get(x, y)) } else { None };
                let now = if a.w == o.w && a.h == o.h { Some(a.get(x, y)) } else { None };
                let was_wrong = had != Some(want);
                let is_wrong = now != Some(want);
                if was_wrong {
                    p.before_wrong += 1;
                }
                if is_wrong {
                    p.after_wrong += 1;
                }
                match (was_wrong, is_wrong) {
                    (true, false) => p.corrected += 1,
                    (false, true) => p.damaged += 1,
                    _ => {}
                }
            }
        }
    }
    p
}

/// 탐색이 멈춘 이유 — **네 가지 병목을 분리**하는 진단의 핵심.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StopReason {
    /// 정답에 도달(현 라이브러리로 표현 가능하며 찾았다).
    Solved,
    /// 확장할 가지가 소진 — 이 깊이에서 **단조 진전 경로로는 도달 불가**.
    /// (일시적 악화를 거치는 경로는 배제되므로 "표현 불가"의 증거이지 증명은 아니다.)
    FrontierExhausted,
    /// 예산 소진 — 도달 가능성은 미정(탐색 병목 가능).
    BudgetExhausted,
}

/// 탐색의 진단 보고 — 무엇이 막혔는지 말한다.
#[derive(Clone, Debug)]
pub struct SearchReport {
    pub stop: StopReason,
    /// 도달한 최선 잔차(0이면 해결).
    pub best_residual: f64,
    /// 최선 상태까지의 연산열.
    pub best_ops: Vec<GridOp>,
    pub reuse: ReuseReport,
    /// 방문한 서로 다른 상태 수(정규형 기준).
    pub states: usize,
}

/// **일반화된 스키마 합성** — anytime 최선 우선 탐색.
///
/// 고정 깊이 2가 아니라, 예산이 허락하는 만큼 깊어진다. 규율:
///
/// - **잔차가 실제로 줄 때만** 확장한다(진전 없는 가지는 죽는다)
/// - **정규형 메모이제이션**: 같은 중간 상태에 도달하는 다른 경로는 한 번만 본다
///   (동일 효과 스키마가 자동으로 하나로 접힌다)
/// - **학습된 사전분포** 순으로 스키마를 시도한다(유망한 것 먼저)
/// - 도메인·자원 가드는 항상 켜져 있다(`safe_chain`)
///
/// 목표는 깊이를 늘리는 것이 아니라 **경험이 탐색을 줄이는 것**이다 — 라이브러리가
/// 좋아질수록 같은 예산에서 더 깊이 간다.
pub fn compose_anytime(
    lib: &mut Library,
    train: &[(Grid, Grid)],
    budget: u32,
    max_depth: usize,
) -> (Option<Vec<GridOp>>, ReuseReport) {
    let r = search_report(lib, train, budget, max_depth);
    let ops = if r.stop == StopReason::Solved { Some(r.best_ops) } else { None };
    (ops, r.reuse)
}

/// 위와 같은 탐색이되 **왜 멈췄는지**까지 보고한다(병목 진단용).
pub fn search_report(
    lib: &mut Library,
    train: &[(Grid, Grid)],
    budget: u32,
    max_depth: usize,
) -> SearchReport {
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
        .map(|(i, o)| {
            vec![i.w.min(255) as u8, i.h.min(255) as u8, o.w.min(255) as u8, o.h.min(255) as u8]
        })
        .unwrap_or_default();

    let start: Vec<Grid> = train.iter().map(|(i, _)| i.clone()).collect();
    let r0 = residual_mids(&start, train);
    if r0 <= 0.0 {
        return SearchReport {
            stop: StopReason::Solved,
            best_residual: 0.0,
            best_ops: Vec::new(),
            reuse: rep,
            states: 1,
        };
    }
    // 최선 상태 추적(해결 못 해도 어디까지 갔는지 보고한다)
    let mut best: (f64, Vec<GridOp>) = (r0, Vec::new());
    let mut budget_hit = false;
    // 최선 우선 변경자: (잔차, 연산열, 중간 상태)
    let mut frontier: Vec<(f64, Vec<GridOp>, Vec<Grid>)> = vec![(r0, Vec::new(), start.clone())];
    let mut seen: std::collections::HashSet<u64> = Default::default();
    seen.insert(state_key(&start));
    let order = lib.by_prior();

    while let Some(pos) = frontier
        .iter()
        .enumerate()
        .min_by(|a, b| a.1 .0.partial_cmp(&b.1 .0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
    {
        if rep.probes >= budget {
            budget_hit = true;
            break;
        }
        let (cur_r, cur_ops, cur_mids) = frontier.swap_remove(pos);
        if cur_ops.len() >= max_depth {
            continue;
        }
        // 현 잔차 상태를 새 훈련 문제로 보고, 라이브러리를 다시 적용한다
        let sub: Vec<(Grid, Grid)> = cur_mids
            .iter()
            .cloned()
            .zip(train.iter().map(|(_, o)| o.clone()))
            .collect();
        for &ix in &order {
            if rep.probes >= budget {
                budget_hit = true;
                break;
            }
            rep.tries += 1;
            for (ops, mids) in instantiations(lib, ix, &sub, &palette, &dims, 12) {
                rep.probes += 1;
                let key = state_key(&mids);
                if !seen.insert(key) {
                    continue; // 이미 본 상태(정규형 중복·실패 조합)
                }
                let r = residual_mids(&mids, train);
                let mut next_ops = cur_ops.clone();
                next_ops.extend(ops);
                if r <= 0.0 {
                    rep.hits += 1;
                    rep.novel += 1;
                    lib.entries[ix].wins = lib.entries[ix].wins.saturating_add(1);
                    let states = seen.len();
                    return SearchReport {
                        stop: StopReason::Solved,
                        best_residual: 0.0,
                        best_ops: next_ops,
                        reuse: rep,
                        states,
                    };
                }
                if r < best.0 {
                    best = (r, next_ops.clone());
                }
                // **진전이 있을 때만** 확장한다
                if r < cur_r - 1e-9 {
                    frontier.push((r, next_ops, mids));
                }
            }
        }
    }
    SearchReport {
        stop: if budget_hit {
            StopReason::BudgetExhausted
        } else {
            StopReason::FrontierExhausted
        },
        best_residual: best.0,
        best_ops: best.1,
        states: seen.len(),
        reuse: rep,
    }
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

    /// **합성 시험**: 경험에 각각 따로 있던 두 스키마를 이어 붙여, 어느 하나로도
    /// 못 푸는 문제를 푼다(자기학습 루프 5단계).
    #[test]
    fn composition_solves_what_single_schemas_cannot() {
        // 경험: 회전 계열과 색 제거 계열이 따로 있었다
        let exp = vec![
            ("a".into(), chain_to_term(&[GridOp::RemoveColor(1)])),
            ("b".into(), chain_to_term(&[GridOp::RemoveColor(2)])),
            ("c".into(), chain_to_term(&[GridOp::Scale(2)])),
            ("d".into(), chain_to_term(&[GridOp::Scale(3)])),
        ];
        let mut lib = Library::new();
        sleep_abstract(&exp, &mut lib);

        // 새 문제: 색 3을 지우고 2배 확대 — 어느 한 스키마로도 못 닫는다
        let mk = |cells: &[(usize, usize, u8)]| {
            let mut g = Grid::new(3, 3);
            for &(x, y, c) in cells {
                g.set(x, y, c);
            }
            g
        };
        let prog = [GridOp::RemoveColor(3), GridOp::Scale(2)];
        let i1 = mk(&[(0, 0, 5), (1, 1, 3), (2, 2, 5)]);
        let i2 = mk(&[(0, 2, 4), (1, 0, 3)]);
        let out = |g: &Grid| {
            let mut x = g.clone();
            for op in &prog {
                x = apply_grid_op(&x, *op);
            }
            x
        };
        let train = vec![(i1.clone(), out(&i1)), (i2.clone(), out(&i2))];

        let (single, _) = reinstantiate(&mut lib, &train, 20_000);
        assert!(single.is_none(), "단일 스키마로 풀리면 합성 시험이 무의미하다");

        let (composed, rep) = reinstantiate_compose(&mut lib, &train, 200_000);
        let ops = composed.expect("합성으로도 못 풀었다");
        let t = mk(&[(2, 0, 3), (0, 1, 7)]);
        let mut got = t.clone();
        for op in &ops {
            got = apply_grid_op(&got, *op);
        }
        assert_eq!(got, out(&t), "합성 프로그램이 시험에서 틀렸다");
        assert!(rep.hits >= 1 && rep.novel >= 1);
    }

    /// **부분 진전 경험 + 유도 합성**: 교사가 버리는 정보(닫지 못했지만 진전한
    /// 시도)가 탐색 공간을 실제로 줄인다 — 맹목 합성 대비 검사 횟수 비교.
    #[test]
    fn partial_progress_guides_composition_and_reduces_search() {
        let exp = vec![
            ("a".into(), chain_to_term(&[GridOp::RemoveColor(1)])),
            ("b".into(), chain_to_term(&[GridOp::RemoveColor(2)])),
            ("c".into(), chain_to_term(&[GridOp::Scale(2)])),
            ("d".into(), chain_to_term(&[GridOp::Scale(3)])),
        ];
        let mut lib = Library::new();
        sleep_abstract(&exp, &mut lib);

        let mk = |cells: &[(usize, usize, u8)]| {
            let mut g = Grid::new(3, 3);
            for &(x, y, c) in cells {
                g.set(x, y, c);
            }
            g
        };
        let prog = [GridOp::RemoveColor(3), GridOp::Scale(2)];
        let out = |g: &Grid| {
            let mut x = g.clone();
            for op in &prog {
                x = apply_grid_op(&x, *op);
            }
            x
        };
        let i1 = mk(&[(0, 0, 5), (1, 1, 3), (2, 2, 5)]);
        let i2 = mk(&[(0, 2, 4), (1, 0, 3)]);
        let train = vec![(i1.clone(), out(&i1)), (i2.clone(), out(&i2))];

        // 부분 진전: 잔차를 줄이는 대입이 발견된다(닫지는 못한다)
        let (seed, base, after, prof) = probe_partial(&lib, &train, 10_000).expect("진전 없음");
        assert!(after < base, "잔차가 줄지 않았다 {base} → {after}");
        // 효과 프로파일: 무엇을 고쳤고 무엇을 망가뜨렸는지가 분해돼 있어야 한다
        assert!(prof.corrected > 0, "고친 셀이 없는데 진전이라고 보고했다");
        assert!(prof.net() > 0, "순이득이 없다: 고침 {} 망침 {}", prof.corrected, prof.damaged);
        assert!(prof.precision() > 0.0 && prof.precision() <= 1.0);
        assert!(residual(&train, &seed) > 0.0, "이것은 이미 완전해다 — 부분해가 아니다");

        // 유도 합성이 그 씨앗에서 마무리를 찾는다
        let (guided, grep) = compose_guided(&mut lib, &train, &[seed], 100_000);
        let ops = guided.expect("유도 합성 실패");
        let t = mk(&[(2, 0, 3), (0, 1, 7)]);
        assert_eq!(safe_chain(&t, &ops).unwrap(), out(&t), "시험에서 틀렸다");

        // 탐색 감소: 맹목 합성보다 적은 검사로 도달
        let mut lib2 = Library::new();
        sleep_abstract(&exp, &mut lib2);
        let (blind, brep) = reinstantiate_compose(&mut lib2, &train, 200_000);
        assert!(blind.is_some(), "맹목 합성도 풀 수 있어야 비교가 성립한다");
        assert!(
            grep.probes < brep.probes,
            "경험이 탐색을 줄이지 못했다: 유도 {} vs 맹목 {}",
            grep.probes,
            brep.probes
        );
    }

    /// **일반화 합성(anytime)**: 3단계 프로그램을 잔차 유도 최선우선으로 찾는다.
    /// 고정 깊이 2로는 못 닫는 문제를 예산만 늘려 닫는다 — 그리고 정규형
    /// 메모이제이션이 같은 상태의 재방문을 막는다.
    #[test]
    fn anytime_composition_reaches_depth_three() {
        let exp = vec![
            ("a".into(), chain_to_term(&[GridOp::RemoveColor(1)])),
            ("b".into(), chain_to_term(&[GridOp::RemoveColor(2)])),
            ("c".into(), chain_to_term(&[GridOp::Scale(2)])),
            ("d".into(), chain_to_term(&[GridOp::Scale(3)])),
            ("e".into(), chain_to_term(&[GridOp::MirrorHGrid])),
            ("f".into(), chain_to_term(&[GridOp::MirrorVGrid])),
        ];
        let mut lib = Library::new();
        sleep_abstract(&exp, &mut lib);

        let mk = |cells: &[(usize, usize, u8)]| {
            let mut g = Grid::new(3, 3);
            for &(x, y, c) in cells {
                g.set(x, y, c);
            }
            g
        };
        // 3단계: 색 지우기 → 거울 → 확대
        let prog = [GridOp::RemoveColor(3), GridOp::MirrorHGrid, GridOp::Scale(2)];
        let out = |g: &Grid| safe_chain(g, &prog).unwrap();
        let i1 = mk(&[(0, 0, 5), (1, 1, 3), (2, 2, 4)]);
        let i2 = mk(&[(0, 2, 6), (1, 0, 3), (2, 1, 7)]);
        let train = vec![(i1.clone(), out(&i1)), (i2.clone(), out(&i2))];

        // 깊이 2로는 못 닫는다
        let (d2, _) = compose_anytime(&mut lib, &train, 300_000, 2);
        assert!(d2.is_none(), "깊이 2로 풀리면 깊이 시험이 무의미하다");

        // 예산·깊이를 늘리면 닫는다(anytime 계약)
        let (d3, rep) = compose_anytime(&mut lib, &train, 800_000, 3);
        let ops = d3.expect("깊이 3 탐색 실패");
        let t = mk(&[(2, 0, 3), (0, 1, 8), (1, 2, 9)]);
        assert_eq!(safe_chain(&t, &ops).unwrap(), out(&t), "시험에서 틀렸다");
        assert!(rep.hits >= 1 && rep.novel >= 1);
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
