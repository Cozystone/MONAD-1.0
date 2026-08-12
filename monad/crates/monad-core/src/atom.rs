//! 인지 원자 (Cognitive Atom) — MONAD의 유일한 자료형.
//!
//! PRD v0.2 §4.1. 지각·개념·관계·상태·목표·스키마·행동이 전부 이 타입 하나다.
//!
//! ```text
//! Atom { id: Sbv, value: Option<Val>, evidence: u32, t: u64 }
//! ```
//!
//! **왜 value 필드가 따로 있는가**: 정체성·바인딩·연상은 SBV가 잘 하지만,
//! 위치 0.392, 속도 7.82 같은 연속량을 억지로 이진화하면 정밀도와 효율을
//! 동시에 잃는다(v0.2 비평 반영). 그래서 연속량은 별도 필드가 담고,
//! **대수 연산은 오직 `id` 위에서만** 정의된다 — 기질은 여전히 하나다.

use crate::sbv::Sbv;

/// 연속값 슬롯. 저차원 물리량(위치·속도·각도·확률)을 담는다.
/// 고정 길이 배열로 힙 할당을 피한다 — 저사양 상시 구동이 전제.
pub const VAL_DIM: usize = 8;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Val {
    pub v: [f32; VAL_DIM],
    /// 실제 사용 중인 차원 수.
    pub used: u8,
}

impl Val {
    pub fn new(slice: &[f32]) -> Val {
        let mut v = [0.0f32; VAL_DIM];
        let n = slice.len().min(VAL_DIM);
        v[..n].copy_from_slice(&slice[..n]);
        Val { v, used: n as u8 }
    }

    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        &self.v[..self.used as usize]
    }

    /// 유클리드 거리(사용 차원만).
    #[inline]
    pub fn dist(&self, other: &Val) -> f32 {
        let n = (self.used.min(other.used)) as usize;
        let mut s = 0.0f32;
        for i in 0..n {
            let d = self.v[i] - other.v[i];
            s += d * d;
        }
        s.sqrt()
    }

    /// 증거 가중 이동 평균으로 갱신(온라인 학습: 에폭도 학습률도 없다).
    /// n은 갱신 이전까지의 관측 수.
    #[inline]
    pub fn absorb(&mut self, other: &Val, n: u32) {
        let w = 1.0 / (n as f32 + 1.0);
        let used = self.used.max(other.used) as usize;
        for i in 0..used {
            self.v[i] += w * (other.v[i] - self.v[i]);
        }
        self.used = used as u8;
    }
}

/// 인지 원자.
#[derive(Clone, Copy, Debug)]
pub struct Atom {
    /// 정체성. 모든 대수 연산의 피연산자.
    pub id: Sbv,
    /// 선택적 연속량.
    pub value: Option<Val>,
    /// 이 원자를 지지하는 관측 수. 신뢰도·망각·BMR의 근거.
    pub evidence: u32,
    /// 마지막 갱신 시점(논리 시계 틱).
    pub t: u64,
}

impl Atom {
    pub fn new(id: Sbv) -> Atom {
        Atom { id, value: None, evidence: 1, t: 0 }
    }

    pub fn with_value(id: Sbv, value: Val) -> Atom {
        Atom { id, value: Some(value), evidence: 1, t: 0 }
    }

    pub fn from_symbol(name: &str) -> Atom {
        Atom::new(Sbv::from_symbol(name))
    }

    /// 신뢰도 — NARS식 증거 기반 (w/(w+k), k=1).
    /// 증거가 쌓일수록 1에 수렴하되 결코 1이 되지 않는다(AIKR: 확실성은 없다).
    #[inline]
    pub fn confidence(&self) -> f32 {
        self.evidence as f32 / (self.evidence as f32 + 1.0)
    }

    /// 같은 원자에 대한 새 관측을 흡수한다. 1-shot 갱신.
    pub fn observe(&mut self, value: Option<Val>, t: u64) {
        if let Some(nv) = value {
            match &mut self.value {
                Some(v) => v.absorb(&nv, self.evidence),
                None => self.value = Some(nv),
            }
        }
        self.evidence = self.evidence.saturating_add(1);
        self.t = t;
    }

    /// id 유사도. value는 관여하지 않는다 — 대수는 id 위에서만.
    #[inline]
    pub fn sim(&self, other: &Atom) -> f32 {
        self.id.sim(&other.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn value_does_not_affect_id_algebra() {
        // v0.2 DoD: value 필드 유무가 id 대수에 영향이 없어야 한다.
        let mut r = Rng::new(11);
        for _ in 0..200 {
            let ida = Sbv::random(&mut r);
            let idb = Sbv::random(&mut r);
            let bare = Atom::new(ida);
            let valued = Atom::with_value(ida, Val::new(&[1.5, -2.0, 42.0]));
            assert_eq!(bare.id, valued.id);
            assert_eq!(bare.id.bind(&idb), valued.id.bind(&idb));
            let other = Atom::with_value(idb, Val::new(&[0.1]));
            assert_eq!(bare.sim(&other), valued.sim(&other));
        }
    }

    #[test]
    fn absorb_converges_to_mean() {
        let mut a = Atom::with_value(Sbv::from_symbol("x"), Val::new(&[0.0]));
        for _ in 0..1000 {
            a.observe(Some(Val::new(&[10.0])), 0);
        }
        let got = a.value.unwrap().as_slice()[0];
        assert!((got - 10.0).abs() < 0.1, "수렴 실패: {got}");
    }

    #[test]
    fn confidence_monotone_below_one() {
        let mut a = Atom::from_symbol("y");
        let mut prev = a.confidence();
        for _ in 0..100 {
            a.observe(None, 0);
            let c = a.confidence();
            assert!(c > prev && c < 1.0);
            prev = c;
        }
    }

    #[test]
    fn val_dist() {
        let a = Val::new(&[3.0, 4.0]);
        let b = Val::new(&[0.0, 0.0]);
        assert!((a.dist(&b) - 5.0).abs() < 1e-5);
    }
}
