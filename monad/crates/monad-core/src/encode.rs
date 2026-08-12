//! B1 — 지각 인코더 (Perception Boundary).
//!
//! 관측을 인지 원자로 바꾼다. MONAD에서 유일하게 "신경망스러울 수 있는" 경계이며,
//! v1에서는 **학습 없는 고정 인코더**로 시작한다(PRD §4.6).
//!
//! # 구조
//!
//! 관측은 (역할, 값)의 집합이다. 각 항을 묶어 중첩한 것이 지각 코드다.
//!
//! ```text
//! code = bundle[ bind(역할ᵢ, 값ᵢ) ]
//! ```
//!
//! - **범주형**: 값 → 심볼에서 결정론적으로 생성한 벡터
//! - **연속형**: 값 → 두 앵커 사이의 보간 벡터(가까운 값은 가까운 벡터). 동시에
//!   원래 수치는 인지 원자의 `value` 필드에 그대로 보존한다 — 억지 이진화 금지(v0.2)
//!
//! # 이벤트 구동
//!
//! 매 틱 전체를 다시 계산하지 않는다. 바뀐 역할만 중첩 누산기에서 빼고 더한다.
//! "바뀐 것만 계산한다"는 §7 원리 7의 물리적 구현.
//!
//! # 왜 이래도 되는가
//!
//! A1에서 **온전한 블록 16개면 10만 개 중 원본을 식별**함을 측정했다. 인코더가
//! 거칠어도 상태 재인식은 무너지지 않는다.

use crate::atom::{Val, VAL_DIM};
use crate::sbv::{Bundler, Sbv, NBLOCKS};
use std::collections::HashMap;

/// 관측 항의 값.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Feature {
    /// 범주형(심볼 번호).
    Cat(u32),
    /// 연속형 실수.
    Num(f32),
}

/// 한 틱의 관측: (역할, 값)의 목록.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Obs {
    pub slots: Vec<(u16, Feature)>,
}

impl Obs {
    pub fn new() -> Self {
        Obs { slots: Vec::new() }
    }
    pub fn cat(mut self, role: u16, v: u32) -> Self {
        self.slots.push((role, Feature::Cat(v)));
        self
    }
    pub fn num(mut self, role: u16, v: f32) -> Self {
        self.slots.push((role, Feature::Num(v)));
        self
    }
    pub fn get(&self, role: u16) -> Option<Feature> {
        self.slots.iter().find(|(r, _)| *r == role).map(|(_, f)| *f)
    }
}

/// 역할 이름 ↔ 번호. 인코더와 스키마 추출(C2)이 공유하는 어휘.
#[derive(Default)]
pub struct Vocab {
    names: Vec<String>,
    by_name: HashMap<String, u16>,
}

impl Vocab {
    pub fn new() -> Self {
        Vocab::default()
    }
    pub fn intern(&mut self, name: &str) -> u16 {
        if let Some(&i) = self.by_name.get(name) {
            return i;
        }
        let i = self.names.len() as u16;
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), i);
        i
    }
    pub fn name(&self, id: u16) -> &str {
        &self.names[id as usize]
    }
    pub fn len(&self) -> usize {
        self.names.len()
    }
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// 연속량 범위 지정. 인코더가 값을 [0,1]로 정규화할 때 쓴다.
#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub lo: f32,
    pub hi: f32,
}

pub struct Encoder {
    role_vec: Vec<Sbv>,
    role_name: Vec<String>,
    /// 연속형 역할의 앵커 쌍 (lo 벡터, hi 벡터)와 범위.
    num_anchor: HashMap<u16, (Sbv, Sbv, Range)>,
    cat_cache: HashMap<(u16, u32), Sbv>,
    /// 직전 관측 — 이벤트 구동 갱신용.
    prev: Obs,
    bundler: Bundler,
    /// 이번 인코딩에서 실제로 다시 계산한 항의 수(계측용).
    pub last_recomputed: usize,
}

impl Encoder {
    pub fn new() -> Self {
        Encoder {
            role_vec: Vec::new(),
            role_name: Vec::new(),
            num_anchor: HashMap::new(),
            cat_cache: HashMap::new(),
            prev: Obs::new(),
            bundler: Bundler::new(),
            last_recomputed: 0,
        }
    }

    fn ensure_role(&mut self, role: u16, name_hint: &str) {
        while self.role_vec.len() <= role as usize {
            let i = self.role_vec.len();
            let nm = if i == role as usize && !name_hint.is_empty() {
                name_hint.to_string()
            } else {
                format!("role{i}")
            };
            self.role_vec.push(Sbv::from_symbol(&format!("ROLE:{nm}")));
            self.role_name.push(nm);
        }
    }

    /// 연속형 역할의 범위를 등록한다. 등록하지 않으면 [0,1]로 가정.
    pub fn set_range(&mut self, role: u16, name: &str, lo: f32, hi: f32) {
        self.ensure_role(role, name);
        self.num_anchor.insert(
            role,
            (
                Sbv::from_symbol(&format!("LVL:{name}:lo")),
                Sbv::from_symbol(&format!("LVL:{name}:hi")),
                Range { lo, hi },
            ),
        );
    }

    pub fn declare(&mut self, role: u16, name: &str) {
        self.ensure_role(role, name);
    }

    /// 연속값 → 앵커 사이의 보간 벡터.
    ///
    /// 앞쪽 k개 블록은 hi에서, 나머지는 lo에서 가져온다. 두 값의 거리는
    /// |k₁−k₂|에 비례하므로 **가까운 값이 가까운 벡터**가 된다 — 수치의 위상을
    /// 하이퍼벡터 공간으로 옮기는 최소 장치.
    fn level_vec(&self, role: u16, v: f32) -> Sbv {
        let (lo_v, hi_v, r) = match self.num_anchor.get(&role) {
            Some(x) => x,
            None => return Sbv::from_seed(v.to_bits() as u64),
        };
        let t = ((v - r.lo) / (r.hi - r.lo).max(1e-9)).clamp(0.0, 1.0);
        let k = (t * NBLOCKS as f32).round() as usize;
        let mut out = *lo_v;
        out.idx[..k].copy_from_slice(&hi_v.idx[..k]);
        out
    }

    /// 범주값의 벡터. 이름에서 결정론적으로 생성되므로 캐시는 순전히 속도용이다.
    fn cat_vec(&self, role: u16, c: u32) -> Sbv {
        if let Some(v) = self.cat_cache.get(&(role, c)) {
            return *v;
        }
        Sbv::from_symbol(&format!("VAL:{}:{}", self.role_name[role as usize], c))
    }

    fn term(&mut self, role: u16, f: Feature) -> Sbv {
        let rv = self.role_vec[role as usize];
        match f {
            Feature::Cat(c) => {
                let v = self.cat_vec(role, c);
                self.cat_cache.entry((role, c)).or_insert(v);
                rv.bind(&v)
            }
            Feature::Num(x) => rv.bind(&self.level_vec(role, x)),
        }
    }

    /// 관측을 코드와 연속량으로 인코딩한다.
    ///
    /// 직전 관측과 비교해 **바뀐 항만** 다시 계산한다.
    pub fn encode(&mut self, obs: &Obs) -> (Sbv, Option<Val>) {
        for (r, _) in &obs.slots {
            self.ensure_role(*r, "");
        }
        self.last_recomputed = 0;

        // 사라진 항 제거
        let prev = std::mem::take(&mut self.prev);
        for (r, f) in &prev.slots {
            if obs.get(*r) != Some(*f) {
                let t = self.term(*r, *f);
                self.bundler.remove_weighted(&t, 1);
                self.last_recomputed += 1;
            }
        }
        // 새로 생긴/바뀐 항 추가
        for (r, f) in &obs.slots {
            if prev.get(*r) != Some(*f) {
                let t = self.term(*r, *f);
                self.bundler.add_weighted(&t, 1);
                self.last_recomputed += 1;
            }
        }
        self.prev = obs.clone();

        // 연속량은 원래 정밀도 그대로 보존한다
        let mut vals: Vec<f32> = Vec::new();
        for (_, f) in &obs.slots {
            if let Feature::Num(x) = f {
                if vals.len() < VAL_DIM {
                    vals.push(*x);
                }
            }
        }
        let val = if vals.is_empty() { None } else { Some(Val::new(&vals)) };
        (self.bundler.finalize(), val)
    }

    /// 누산기를 비운다(에피소드 경계).
    pub fn reset(&mut self) {
        self.bundler = Bundler::new();
        self.prev = Obs::new();
    }

    /// 코드에서 특정 역할의 값을 되묻는다(유리상자: 무엇을 보고 있는지 검사).
    /// 후보 값들 중 가장 그럴듯한 것을 돌려준다.
    pub fn probe_cat(&self, code: &Sbv, role: u16, candidates: &[u32]) -> Option<(u32, f32)> {
        let rv = self.role_vec.get(role as usize)?;
        let q = code.unbind(rv);
        let mut best: Option<(u32, f32)> = None;
        for &c in candidates {
            let s = q.sim(&self.cat_vec(role, c));
            if best.is_none() || s > best.unwrap().1 {
                best = Some((c, s));
            }
        }
        best
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Encoder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc() -> (Encoder, Vocab) {
        let mut v = Vocab::new();
        let mut e = Encoder::new();
        let color = v.intern("color");
        let shape = v.intern("shape");
        let x = v.intern("x");
        e.declare(color, "color");
        e.declare(shape, "shape");
        e.set_range(x, "x", 0.0, 100.0);
        (e, v)
    }

    #[test]
    fn same_scene_gives_same_code() {
        // B1 DoD: 동일 장면 반복 시 자기거리 0
        let (mut e, mut v) = enc();
        let (c, s) = (v.intern("color"), v.intern("shape"));
        let o = Obs::new().cat(c, 3).cat(s, 1);
        let (a, _) = e.encode(&o);
        e.reset();
        let (b, _) = e.encode(&o);
        assert_eq!(a, b);
        assert_eq!(a.dist(&b), 0);
    }

    #[test]
    fn different_scenes_are_far() {
        let (mut e, mut v) = enc();
        let (c, s) = (v.intern("color"), v.intern("shape"));
        let (a, _) = e.encode(&Obs::new().cat(c, 3).cat(s, 1));
        e.reset();
        let (b, _) = e.encode(&Obs::new().cat(c, 5).cat(s, 4));
        // 두 항이 모두 다르면 코드도 확실히 달라야 한다
        assert!(a.sim(&b) < 0.3, "분리 실패 sim={}", a.sim(&b));
    }

    #[test]
    fn one_changed_slot_moves_code_partially() {
        // 한 항만 바뀌면 코드도 부분적으로만 바뀐다(구조가 보존된다)
        let (mut e, mut v) = enc();
        let (c, s) = (v.intern("color"), v.intern("shape"));
        let (a, _) = e.encode(&Obs::new().cat(c, 3).cat(s, 1));
        e.reset();
        let (b, _) = e.encode(&Obs::new().cat(c, 3).cat(s, 2));
        e.reset();
        let (d, _) = e.encode(&Obs::new().cat(c, 7).cat(s, 9));
        assert!(
            a.sim(&b) > a.sim(&d),
            "한 항 변화({:.3})가 두 항 변화({:.3})보다 가까워야",
            a.sim(&b),
            a.sim(&d)
        );
    }

    #[test]
    fn numeric_locality() {
        // 가까운 수치는 가까운 벡터
        let (mut e, mut v) = enc();
        let x = v.intern("x");
        let (a, _) = e.encode(&Obs::new().num(x, 10.0));
        e.reset();
        let (b, _) = e.encode(&Obs::new().num(x, 12.0));
        e.reset();
        let (c, _) = e.encode(&Obs::new().num(x, 90.0));
        assert!(a.sim(&b) > 0.9, "가까운 값 sim={}", a.sim(&b));
        assert!(a.sim(&c) < 0.3, "먼 값 sim={}", a.sim(&c));
    }

    #[test]
    fn value_field_preserves_precision() {
        let (mut e, mut v) = enc();
        let x = v.intern("x");
        let (_, val) = e.encode(&Obs::new().num(x, 3.14159));
        assert!((val.unwrap().as_slice()[0] - 3.14159).abs() < 1e-5);
    }

    #[test]
    fn event_driven_only_recomputes_changes() {
        let (mut e, mut v) = enc();
        let (c, s) = (v.intern("color"), v.intern("shape"));
        e.encode(&Obs::new().cat(c, 1).cat(s, 1));
        assert_eq!(e.last_recomputed, 2, "첫 인코딩은 두 항 모두");
        e.encode(&Obs::new().cat(c, 1).cat(s, 1));
        assert_eq!(e.last_recomputed, 0, "변화 없으면 재계산 없음");
        e.encode(&Obs::new().cat(c, 1).cat(s, 2));
        assert_eq!(e.last_recomputed, 2, "한 항 변화 = 제거1 + 추가1");
    }

    #[test]
    fn probe_recovers_role_value() {
        // 유리상자: 코드에서 '무엇을 보고 있었는지' 되물을 수 있어야 한다
        let (mut e, mut v) = enc();
        let (c, s) = (v.intern("color"), v.intern("shape"));
        let (code, _) = e.encode(&Obs::new().cat(c, 3).cat(s, 1));
        let got = e.probe_cat(&code, c, &[0, 1, 2, 3, 4, 5]).unwrap();
        assert_eq!(got.0, 3, "색 역할을 되물으면 3이 나와야 (신뢰도 {:.3})", got.1);
    }
}
