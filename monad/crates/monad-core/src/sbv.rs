//! SBV — 희소 블록 하이퍼벡터 (Sparse Block hyperVector).
//!
//! MONAD 인지 원자의 `id` 필드를 구성하는 대수적 기질.
//!
//! # 표현
//!
//! 개념적으로는 D = 16,384비트 벡터를 128개 블록(블록당 128비트)으로 나누고
//! **블록마다 정확히 1비트만 활성**인 희소 코드다.
//!
//! 구현은 그 활성 비트의 **인덱스만** 저장한다: `[u8; 128]` = 128바이트.
//! 밀집 비트맵(2KB) 대비 16배 작고, 다음 성질을 공짜로 얻는다:
//!
//! - `bind` = 블록별 모듈러 덧셈 → **완전 가역**(부동소수 근사 없음)
//! - `hamming` = 인덱스 배열 비교 → 128바이트 SIMD 4회(2KB popcount 64회 대비 16배 적은 작업)
//! - 원자 100만 개 = 128MB (PRD의 RAM ≤ 4GB 예산 안에 여유)
//!
//! # 거리
//!
//! `dist(a,b)` = 서로 다른 블록의 수(0..=128). 무작위 두 벡터의 기대 거리는
//! 128·(127/128) ≈ 127 — 즉 거의 모든 블록이 다르다. 이 큰 여백이
//! 중첩(bundle)된 벡터에서 구성원을 되찾을 수 있게 하는 근거다.

use crate::rng::{hash64, Rng};
use std::fmt;

/// 블록 수. `dist`의 최대값이자 유사도의 분모.
pub const NBLOCKS: usize = 128;
/// 블록당 상태 수(= 블록 비트 수). 인덱스는 0..BLOCK_STATES.
pub const BLOCK_STATES: usize = 128;
/// 개념적 차원 = 16,384비트.
pub const DIM: usize = NBLOCKS * BLOCK_STATES;
/// 실제 저장 바이트 수.
pub const SBV_BYTES: usize = NBLOCKS;

const MASK: u8 = (BLOCK_STATES - 1) as u8; // 0x7F

/// 희소 블록 하이퍼벡터.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sbv {
    /// 블록별 활성 인덱스. 각 원소는 0..BLOCK_STATES.
    pub idx: [u8; NBLOCKS],
}

impl Sbv {
    /// bind의 항등원(모든 블록 인덱스 0).
    pub const ZERO: Sbv = Sbv { idx: [0u8; NBLOCKS] };

    /// 무작위 벡터.
    pub fn random(rng: &mut Rng) -> Sbv {
        let mut idx = [0u8; NBLOCKS];
        // u64 하나에서 8블록씩 뽑아 호출 수를 줄인다.
        let mut i = 0;
        while i < NBLOCKS {
            let r = rng.next_u64();
            let take = (NBLOCKS - i).min(8);
            for k in 0..take {
                idx[i + k] = ((r >> (8 * k)) as u8) & MASK;
            }
            i += take;
        }
        Sbv { idx }
    }

    /// 심볼 이름에서 결정론적으로 생성. 같은 이름 → 항상 같은 벡터.
    /// 인코더가 "빨강", "벽" 같은 안정적 정체성을 만들 때 쓴다.
    pub fn from_symbol(name: &str) -> Sbv {
        let mut rng = Rng::new(hash64(name.as_bytes()));
        Sbv::random(&mut rng)
    }

    /// 64비트 시드에서 결정론적으로 생성.
    pub fn from_seed(seed: u64) -> Sbv {
        let mut rng = Rng::new(seed);
        Sbv::random(&mut rng)
    }

    /// 바인딩: 블록별 (a + b) mod 128.
    ///
    /// 가환·결합적이며 항등원 `ZERO`를 갖는 아벨군 연산.
    /// 거리 보존(isometry): `dist(a⊗c, b⊗c) == dist(a, b)`.
    #[inline]
    pub fn bind(&self, other: &Sbv) -> Sbv {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                // SAFETY: avx2 지원을 런타임에 확인함.
                return unsafe { simd::bind_avx2(self, other) };
            }
        }
        bind_scalar(self, other)
    }

    /// 언바인딩: bind의 역연산. `a.bind(&b).unbind(&b) == a`.
    #[inline]
    pub fn unbind(&self, other: &Sbv) -> Sbv {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                // SAFETY: avx2 지원을 런타임에 확인함.
                return unsafe { simd::unbind_avx2(self, other) };
            }
        }
        unbind_scalar(self, other)
    }

    /// 치환: 블록 순서를 k칸 회전. 순서(시퀀스) 인코딩용. 가역.
    #[inline]
    pub fn permute(&self, k: usize) -> Sbv {
        let k = k % NBLOCKS;
        let mut out = [0u8; NBLOCKS];
        out[..NBLOCKS - k].copy_from_slice(&self.idx[k..]);
        out[NBLOCKS - k..].copy_from_slice(&self.idx[..k]);
        Sbv { idx: out }
    }

    /// 치환의 역연산.
    #[inline]
    pub fn unpermute(&self, k: usize) -> Sbv {
        self.permute(NBLOCKS - (k % NBLOCKS))
    }

    /// 해밍 거리: 서로 다른 블록의 수 (0..=128).
    #[inline]
    pub fn dist(&self, other: &Sbv) -> u32 {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("avx2") {
                // SAFETY: avx2 지원을 런타임에 확인함.
                return unsafe { simd::dist_avx2(self, other) };
            }
        }
        dist_scalar(self, other)
    }

    /// 유사도 = 1 - dist/128. 무작위 쌍의 기대값 ≈ 0.0078 (1/128).
    #[inline]
    pub fn sim(&self, other: &Sbv) -> f32 {
        1.0 - (self.dist(other) as f32) / (NBLOCKS as f32)
    }

    /// 직렬화(그래프 스냅숏용).
    #[inline]
    pub fn as_bytes(&self) -> &[u8; SBV_BYTES] {
        &self.idx
    }

    #[inline]
    pub fn from_bytes(b: &[u8; SBV_BYTES]) -> Sbv {
        let mut idx = *b;
        // 방어적 정규화: 손상된 스냅숏이 범위를 벗어난 인덱스를 갖지 않도록.
        for v in idx.iter_mut() {
            *v &= MASK;
        }
        Sbv { idx }
    }

    /// 짧은 지문(디버그/덤프용).
    pub fn fingerprint(&self) -> u64 {
        hash64(&self.idx)
    }
}

impl fmt::Debug for Sbv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sbv#{:012x}", self.fingerprint() & 0xffff_ffff_ffff)
    }
}

impl Default for Sbv {
    fn default() -> Self {
        Sbv::ZERO
    }
}

// ---------------------------------------------------------------- 스칼라 경로

#[inline]
fn bind_scalar(a: &Sbv, b: &Sbv) -> Sbv {
    let mut out = [0u8; NBLOCKS];
    for i in 0..NBLOCKS {
        out[i] = a.idx[i].wrapping_add(b.idx[i]) & MASK;
    }
    Sbv { idx: out }
}

#[inline]
fn unbind_scalar(a: &Sbv, b: &Sbv) -> Sbv {
    let mut out = [0u8; NBLOCKS];
    for i in 0..NBLOCKS {
        out[i] = a.idx[i].wrapping_sub(b.idx[i]) & MASK;
    }
    Sbv { idx: out }
}

#[inline]
fn dist_scalar(a: &Sbv, b: &Sbv) -> u32 {
    let mut d = 0u32;
    for i in 0..NBLOCKS {
        d += (a.idx[i] != b.idx[i]) as u32;
    }
    d
}

// ------------------------------------------------------------------ SIMD 경로

#[cfg(target_arch = "x86_64")]
mod simd {
    use super::{Sbv, MASK, NBLOCKS};
    use core::arch::x86_64::*;

    #[target_feature(enable = "avx2")]
    pub unsafe fn bind_avx2(a: &Sbv, b: &Sbv) -> Sbv {
        let mut out = [0u8; NBLOCKS];
        let mask = _mm256_set1_epi8(MASK as i8);
        let mut i = 0;
        while i < NBLOCKS {
            let va = _mm256_loadu_si256(a.idx.as_ptr().add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.idx.as_ptr().add(i) as *const __m256i);
            let s = _mm256_and_si256(_mm256_add_epi8(va, vb), mask);
            _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, s);
            i += 32;
        }
        Sbv { idx: out }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn unbind_avx2(a: &Sbv, b: &Sbv) -> Sbv {
        let mut out = [0u8; NBLOCKS];
        let mask = _mm256_set1_epi8(MASK as i8);
        let mut i = 0;
        while i < NBLOCKS {
            let va = _mm256_loadu_si256(a.idx.as_ptr().add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.idx.as_ptr().add(i) as *const __m256i);
            let s = _mm256_and_si256(_mm256_sub_epi8(va, vb), mask);
            _mm256_storeu_si256(out.as_mut_ptr().add(i) as *mut __m256i, s);
            i += 32;
        }
        Sbv { idx: out }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn dist_avx2(a: &Sbv, b: &Sbv) -> u32 {
        let mut eq = 0u32;
        let mut i = 0;
        while i < NBLOCKS {
            let va = _mm256_loadu_si256(a.idx.as_ptr().add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.idx.as_ptr().add(i) as *const __m256i);
            let c = _mm256_cmpeq_epi8(va, vb);
            eq += (_mm256_movemask_epi8(c) as u32).count_ones();
            i += 32;
        }
        NBLOCKS as u32 - eq
    }
}

// -------------------------------------------------------------------- 번들링

/// 중첩(bundle) 누산기.
///
/// 여러 SBV를 겹쳐 "집합"이나 "평균 개념"을 만든다. 블록마다 상태별 증거를
/// 누적한 뒤 `finalize`에서 argmax로 다시 희소화한다.
///
/// 이 누산 구조가 **1-shot 학습**의 물리적 형태다: 새 경험을 더하는 것은
/// 카운터 증가 한 번이며, 경사하강도 학습률도 없다.
pub struct Bundler {
    counts: Box<[u32; DIM]>,
    n: u32,
    tiebreak: usize,
}

impl Bundler {
    pub fn new() -> Self {
        Bundler {
            counts: Box::new([0u32; DIM]),
            n: 0,
            tiebreak: 0,
        }
    }

    /// 동점 처리 시작 오프셋. 블록별로 회전시켜 낮은 인덱스 편향을 없앤다.
    pub fn with_tiebreak(seed: usize) -> Self {
        let mut b = Bundler::new();
        b.tiebreak = seed;
        b
    }

    #[inline]
    pub fn add(&mut self, s: &Sbv) {
        self.add_weighted(s, 1);
    }

    /// 증거 가중 누적. evidence가 큰 원자가 더 큰 표를 갖는다.
    #[inline]
    pub fn add_weighted(&mut self, s: &Sbv, w: u32) {
        for blk in 0..NBLOCKS {
            self.counts[blk * BLOCK_STATES + s.idx[blk] as usize] += w;
        }
        self.n += 1;
    }

    /// 누적에서 제거(반증·망각용).
    #[inline]
    pub fn remove_weighted(&mut self, s: &Sbv, w: u32) {
        for blk in 0..NBLOCKS {
            let c = &mut self.counts[blk * BLOCK_STATES + s.idx[blk] as usize];
            *c = c.saturating_sub(w);
        }
        self.n = self.n.saturating_sub(1);
    }

    pub fn len(&self) -> u32 {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// 희소화: 블록별 argmax. 동점은 블록마다 다른 지점에서 스캔을 시작해 해소.
    pub fn finalize(&self) -> Sbv {
        let mut idx = [0u8; NBLOCKS];
        for blk in 0..NBLOCKS {
            let base = blk * BLOCK_STATES;
            let start = (blk.wrapping_mul(53).wrapping_add(self.tiebreak)) % BLOCK_STATES;
            let mut best = 0u32;
            let mut best_i = start;
            for off in 0..BLOCK_STATES {
                let i = (start + off) % BLOCK_STATES;
                let c = self.counts[base + i];
                if c > best {
                    best = c;
                    best_i = i;
                }
            }
            idx[blk] = best_i as u8;
        }
        Sbv { idx }
    }

    /// 블록별 최빈 상태의 지지도(0..1). 중첩이 얼마나 "선명한지"의 척도.
    /// 값이 1에 가까우면 구성원들이 일치하고, 1/K에 가까우면 서로 다르다.
    pub fn sharpness(&self) -> f32 {
        if self.n == 0 {
            return 0.0;
        }
        let total: u64 = NBLOCKS as u64 * self.n as u64;
        let mut top: u64 = 0;
        for blk in 0..NBLOCKS {
            let base = blk * BLOCK_STATES;
            let mut best = 0u32;
            for i in 0..BLOCK_STATES {
                let c = self.counts[base + i];
                if c > best {
                    best = c;
                }
            }
            top += best as u64;
        }
        top as f32 / total as f32
    }
}

impl Default for Bundler {
    fn default() -> Self {
        Bundler::new()
    }
}

/// 편의 함수: 슬라이스를 한 번에 중첩.
pub fn bundle(items: &[Sbv]) -> Sbv {
    let mut b = Bundler::new();
    for s in items {
        b.add(s);
    }
    b.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: u64 = 0x4D_4F_4E_41_44; // "MONAD"

    fn rng() -> Rng {
        Rng::new(SEED)
    }

    #[test]
    fn bind_identity() {
        let mut r = Rng::new(1);
        for _ in 0..200 {
            let a = Sbv::random(&mut r);
            assert_eq!(a.bind(&Sbv::ZERO), a);
        }
    }

    #[test]
    fn bind_commutative_associative() {
        let mut r = Rng::new(2);
        for _ in 0..200 {
            let (a, b, c) = (Sbv::random(&mut r), Sbv::random(&mut r), Sbv::random(&mut r));
            assert_eq!(a.bind(&b), b.bind(&a), "가환성");
            assert_eq!(a.bind(&b).bind(&c), a.bind(&b.bind(&c)), "결합성");
        }
    }

    #[test]
    fn unbind_is_exact_inverse() {
        let mut r = Rng::new(3);
        for _ in 0..500 {
            let (a, b) = (Sbv::random(&mut r), Sbv::random(&mut r));
            assert_eq!(a.bind(&b).unbind(&b), a, "언바인딩은 근사가 아니라 정확해야 한다");
        }
    }

    #[test]
    fn bind_preserves_distance() {
        // 등거리성: 바인딩은 구조를 회전시킬 뿐 관계를 왜곡하지 않는다.
        let mut r = Rng::new(4);
        for _ in 0..200 {
            let (a, b, c) = (Sbv::random(&mut r), Sbv::random(&mut r), Sbv::random(&mut r));
            assert_eq!(a.bind(&c).dist(&b.bind(&c)), a.dist(&b));
        }
    }

    #[test]
    fn permute_invertible_and_dissimilar() {
        let mut r = Rng::new(5);
        for _ in 0..100 {
            let a = Sbv::random(&mut r);
            assert_eq!(a.permute(7).unpermute(7), a);
            // 치환된 벡터는 원본과 무관해야(시퀀스 위치 구분 가능)
            assert!(a.permute(7).sim(&a) < 0.2);
        }
    }

    #[test]
    fn random_pairs_are_far() {
        let mut r = Rng::new(6);
        let mut sum = 0u64;
        let n = 2000;
        for _ in 0..n {
            let (a, b) = (Sbv::random(&mut r), Sbv::random(&mut r));
            sum += a.dist(&b) as u64;
        }
        let mean = sum as f64 / n as f64;
        // 기대값 128*(127/128) = 127
        assert!((mean - 127.0).abs() < 1.0, "무작위 쌍 평균 거리 {mean}");
    }

    #[test]
    fn simd_matches_scalar() {
        let mut r = Rng::new(7);
        for _ in 0..500 {
            let (a, b) = (Sbv::random(&mut r), Sbv::random(&mut r));
            assert_eq!(a.bind(&b), bind_scalar(&a, &b));
            assert_eq!(a.unbind(&b), unbind_scalar(&a, &b));
            assert_eq!(a.dist(&b), dist_scalar(&a, &b));
        }
    }

    #[test]
    fn symbol_is_stable() {
        assert_eq!(Sbv::from_symbol("wall"), Sbv::from_symbol("wall"));
        assert!(Sbv::from_symbol("wall").sim(&Sbv::from_symbol("ball")) < 0.2);
    }

    #[test]
    fn bundle_recovers_members() {
        // 중첩된 벡터는 구성원과 무작위 대조군보다 확실히 가까워야 한다.
        let mut r = rng();
        for k in [2usize, 4, 8, 16] {
            let items: Vec<Sbv> = (0..k).map(|_| Sbv::random(&mut r)).collect();
            let b = bundle(&items);
            let member_sim: f32 =
                items.iter().map(|s| b.sim(s)).sum::<f32>() / k as f32;
            let noise_sim: f32 = (0..64)
                .map(|_| b.sim(&Sbv::random(&mut r)))
                .sum::<f32>()
                / 64.0;
            assert!(
                member_sim > noise_sim * 4.0,
                "K={k}: 구성원 유사도 {member_sim:.4} vs 잡음 {noise_sim:.4}"
            );
        }
    }

    #[test]
    fn bundle_tiebreak_is_unbiased() {
        // 동점 처리 시 인덱스 0으로 몰리지 않아야 한다.
        let mut r = Rng::new(9);
        let items: Vec<Sbv> = (0..4).map(|_| Sbv::random(&mut r)).collect();
        let b = bundle(&items);
        let zeros = b.idx.iter().filter(|&&v| v == 0).count();
        assert!(zeros < 20, "인덱스 0 편향 의심: {zeros}/128");
    }

    #[test]
    fn serialization_roundtrip() {
        let mut r = Rng::new(10);
        for _ in 0..100 {
            let a = Sbv::random(&mut r);
            assert_eq!(Sbv::from_bytes(a.as_bytes()), a);
        }
    }

    #[test]
    fn size_is_128_bytes() {
        assert_eq!(std::mem::size_of::<Sbv>(), 128);
    }
}
