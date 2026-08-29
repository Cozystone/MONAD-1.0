//! M2-R — **답 색 규칙**: "어느 색이 답인가"를 배우는 계층(시도 207).
//!
//! # 왜 이 계층인가 (계량이 정했다)
//!
//! 미해결 크기 변환 97건 중 **출력이 단색인 과제가 9건**이고, 그중 **4건이 기존
//! 성질로 그대로 설명된다**(다수색 2 · 희소색 1 · 최소 객체색 1). 답이 이미 내
//! 성질 벡터의 슬롯에 들어 있다는 뜻이다.
//!
//! 두 가지가 이 계층을 값지게 한다:
//!
//! 1. **게이트가 쌍당 결정 하나**다 — 세션 내내 ③를 막아온 과제당 100% 덮개
//!    요구가 없다(선택 계층과 같은 이유).
//! 2. **구성상 팔레트 독립**이다 — 규칙이 "슬롯 j의 색"이지 "색 3"이 아니므로,
//!    팔레트가 다른 과제에서도 그대로 성립한다. 세션 내내 전이를 막은 리터럴
//!    의존이 원리적으로 없다.
//!
//! 규칙 공간은 "어느 슬롯인가 × 출력 크기를 어떻게 정하는가"로 극히 작다.
//! 그래서 경험 한둘로도 배울 수 있고, 그것이 이 계층의 존재 이유다.

use crate::grid::{components_bg, Grid, Obj};
use monad_core::abstraction::{Library, Provenance, Term};

const F_ANSRULE: u32 = 940;

/// 격자 수준 색 성질 — 전부 동결 기질의 분해에서 기계적으로 나온다.
pub const NCOLORSLOTS: usize = 6;
pub const SLOT_NAMES: [&str; NCOLORSLOTS] = [
    "majority",      // 최빈색(배경 제외)
    "rarest",        // 최소빈도색
    "largest_obj",   // 최대 객체의 색
    "smallest_obj",  // 최소 객체의 색
    "unique_shape",  // 모양이 유일한 객체의 색
    "most_objects",  // 객체 수가 가장 많은 색
];

fn obj_color(o: &Obj) -> u8 {
    o.mask
        .iter()
        .zip(o.colors.iter())
        .find(|(m, _)| **m)
        .map(|(_, &c)| c)
        .unwrap_or(0)
}

/// 격자에서 색 성질 슬롯들을 뽑는다.
pub fn color_slots(g: &Grid) -> [u8; NCOLORSLOTS] {
    let mut freq = [0usize; 10];
    for &c in &g.cells {
        if c != 0 && c <= 9 {
            freq[c as usize] += 1;
        }
    }
    let majority = (1..10).filter(|&c| freq[c] > 0).max_by_key(|&c| freq[c]).unwrap_or(0) as u8;
    let rarest = (1..10).filter(|&c| freq[c] > 0).min_by_key(|&c| freq[c]).unwrap_or(0) as u8;
    let objs = components_bg(g, false, 0);
    let largest = objs.iter().max_by_key(|b| b.area).map(obj_color).unwrap_or(0);
    let smallest = objs.iter().min_by_key(|b| b.area).map(obj_color).unwrap_or(0);
    // 모양이 유일한 객체(같은 모양이 하나도 없는 것)
    let unique_shape = objs
        .iter()
        .find(|a| {
            !objs
                .iter()
                .any(|b| !std::ptr::eq(*a, b) && b.w == a.w && b.h == a.h && b.mask == a.mask)
        })
        .map(obj_color)
        .unwrap_or(0);
    // 객체 수가 가장 많은 색
    let mut cnt = [0usize; 10];
    for o in &objs {
        cnt[obj_color(o) as usize] += 1;
    }
    let most_objects = (1..10).max_by_key(|&c| cnt[c]).unwrap_or(0) as u8;
    [majority, rarest, largest, smallest, unique_shape, most_objects]
}

/// 출력 크기를 어떻게 정하는가.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dims {
    /// 모든 쌍에서 같은 고정 크기.
    Fixed(usize, usize),
    /// 입력과 같은 크기.
    SameAsInput,
}

/// 규칙: `ANSRULE(slot, mode, w, h)` — 슬롯 j의 색으로 지정 크기 격자를 채운다.
fn build(slot: usize, dims: Dims) -> Term {
    let (mode, w, h) = match dims {
        Dims::Fixed(w, h) => (0u64, w as u64, h as u64),
        Dims::SameAsInput => (1u64, 0, 0),
    };
    Term::App(
        F_ANSRULE,
        vec![
            Term::Const(slot as u64),
            Term::Const(mode),
            Term::Const(w),
            Term::Const(h),
        ],
    )
}

fn split(t: &Term) -> Option<(usize, Dims)> {
    let Term::App(f, a) = t else { return None };
    if *f != F_ANSRULE || a.len() != 4 {
        return None;
    }
    let (Term::Const(slot), Term::Const(mode), Term::Const(w), Term::Const(h)) =
        (&a[0], &a[1], &a[2], &a[3])
    else {
        return None;
    };
    let dims = if *mode == 0 {
        Dims::Fixed(*w as usize, *h as usize)
    } else {
        Dims::SameAsInput
    };
    (*slot < NCOLORSLOTS as u64).then_some((*slot as usize, dims))
}

/// 규칙을 적용해 답 격자를 만든다.
pub fn apply_ans_rule(slot: usize, dims: Dims, g: &Grid) -> Option<Grid> {
    let c = color_slots(g).get(slot).copied()?;
    let (w, h) = match dims {
        Dims::Fixed(w, h) => (w, h),
        Dims::SameAsInput => (g.w, g.h),
    };
    if w == 0 || h == 0 {
        return None;
    }
    let mut out = Grid::new(w, h);
    for y in 0..h {
        for x in 0..w {
            out.set(x, y, c);
        }
    }
    Some(out)
}

/// 훈련쌍이 **모두 단색 출력**이면 그 색과 크기 규칙을 찾는다.
///
/// 규칙 공간이 작아 전수 검사한다(슬롯 6 × 크기 2). 모든 쌍을 정확히 재현하는
/// 것만 채택 — 다른 계층과 같은 게이트다.
pub fn learn_ans_rules(train: &[(Grid, Grid)]) -> Vec<(usize, Dims)> {
    if train.is_empty() {
        return Vec::new();
    }
    // 출력이 전부 단색인가
    for (_, o) in train {
        let f = o.cells.first().copied().unwrap_or(0);
        if o.cells.is_empty() || o.cells.iter().any(|&c| c != f) {
            return Vec::new();
        }
    }
    let (w0, h0) = (train[0].1.w, train[0].1.h);
    let fixed_ok = train.iter().all(|(_, o)| o.w == w0 && o.h == h0);
    let same_ok = train.iter().all(|(i, o)| o.w == i.w && o.h == i.h);
    let mut out = Vec::new();
    for slot in 0..NCOLORSLOTS {
        for dims in [Dims::Fixed(w0, h0), Dims::SameAsInput] {
            if dims == Dims::Fixed(w0, h0) && !fixed_ok {
                continue;
            }
            if dims == Dims::SameAsInput && !same_ok {
                continue;
            }
            if train
                .iter()
                .all(|(i, o)| apply_ans_rule(slot, dims, i).as_ref() == Some(o))
            {
                out.push((slot, dims));
            }
        }
    }
    out
}

/// 수면: 배운 규칙을 라이브러리에 넣는다(출처 태그 포함).
pub fn sleep_ans(per_task: &[Vec<(usize, Dims)>], lib: &mut Library) -> usize {
    let mut added = 0usize;
    for rules in per_task {
        for &(slot, dims) in rules {
            let schema = build(slot, dims);
            let abs = monad_core::abstraction::Abstraction {
                schema: schema.clone(),
                instances: vec![Default::default()],
                gain: 1,
            };
            if lib.insert(&abs, Provenance::MonadDerived) {
                added += 1;
            }
        }
    }
    added
}

/// **증거 기반 선택**: 이 과제의 모든 훈련쌍을 정확히 재현하는 라이브러리 규칙.
pub fn select_ans_consistent(lib: &Library, train: &[(Grid, Grid)]) -> Vec<(usize, Dims)> {
    lib.by_prior()
        .into_iter()
        .filter_map(|ix| split(&lib.entries[ix].schema))
        .filter(|&(slot, dims)| {
            // 고정 크기 규칙은 이 과제의 출력 크기와 맞아야 한다
            let dims = match dims {
                Dims::Fixed(_, _) => match train.first() {
                    Some((_, o)) => Dims::Fixed(o.w, o.h),
                    None => return false,
                },
                d => d,
            };
            !train.is_empty()
                && train
                    .iter()
                    .all(|(i, o)| apply_ans_rule(slot, dims, i).as_ref() == Some(o))
        })
        .map(|(slot, dims)| {
            let dims = match dims {
                Dims::Fixed(_, _) => {
                    let o = &train[0].1;
                    Dims::Fixed(o.w, o.h)
                }
                d => d,
            };
            (slot, dims)
        })
        .collect()
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

    /// **구성상 팔레트 독립**: "다수색이 답"을 한 팔레트에서 배우면 어떤 팔레트에도
    /// 그대로 적용된다 — 규칙이 "슬롯 j"이지 "색 3"이 아니기 때문이다.
    /// 세션 내내 전이를 막은 리터럴 의존이 이 계층에는 원리적으로 없다.
    #[test]
    fn answer_color_rule_is_palette_independent_by_construction() {
        let mk = |major: u8, minor: u8| {
            let mut i = Grid::new(8, 8);
            place(&mut i, 0, 0, 4, 4, major); // 16칸 — 다수
            place(&mut i, 6, 6, 1, 1, minor); // 1칸
            let mut o = Grid::new(1, 1);
            o.set(0, 0, major);
            (i, o)
        };
        let learned = learn_ans_rules(&[mk(3, 5)]);
        assert!(!learned.is_empty(), "규칙을 못 배웠다");
        let mut lib = Library::new();
        assert!(sleep_ans(&[learned], &mut lib) > 0);

        // 전혀 다른 팔레트에서 그대로 성립한다
        for (a, b) in [(6u8, 2u8), (9, 1), (7, 4)] {
            let train = [mk(a, b)];
            let sel = select_ans_consistent(&lib, &train);
            assert!(!sel.is_empty(), "팔레트 ({a},{b})에서 일관 규칙 없음");
            let (slot, dims) = sel[0];
            assert_eq!(
                apply_ans_rule(slot, dims, &train[0].0).as_ref(),
                Some(&train[0].1),
                "팔레트 ({a},{b}) 적용 실패"
            );
        }
    }

    /// 단색 출력이 아니면 이 계층은 손대지 않는다(범위 규율).
    #[test]
    fn non_monochrome_output_is_out_of_scope() {
        let mut i = Grid::new(4, 4);
        place(&mut i, 0, 0, 2, 2, 3);
        let mut o = Grid::new(2, 2);
        o.set(0, 0, 3);
        o.set(1, 1, 5); // 두 가지 색
        assert!(learn_ans_rules(&[(i, o)]).is_empty());
    }
}
