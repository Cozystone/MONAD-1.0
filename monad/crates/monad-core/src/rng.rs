//! 결정론적 PRNG. 외부 의존성 없이 재현 가능한 실험을 보장한다.
//!
//! MONAD의 모든 실험은 시드를 명시해 재현 가능해야 한다(개발계획 §실행원칙).

/// xoshiro256++ — 빠르고 통계적 품질이 충분한 결정론적 생성기.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// 시드에서 생성. 동일 시드 → 동일 수열(플랫폼 무관).
    pub fn new(seed: u64) -> Self {
        // SplitMix64로 상태를 채운다.
        let mut z = seed;
        let mut next = || {
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            x ^ (x >> 31)
        };
        Rng { s: [next(), next(), next(), next()] }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[0]
            .wrapping_add(self.s[3])
            .rotate_left(23)
            .wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// 0..n 균등 (n > 0). Lemire의 곱셈 축약.
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        debug_assert!(n > 0);
        ((self.next_u64() as u128 * n as u128) >> 64) as u32
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        // [0,1)
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// 표준정규 근사 (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// 문자열/바이트 → 결정론적 64비트 해시 (FNV-1a 64).
/// 심볼 이름에서 안정적인 SBV를 만들 때 쓴다.
pub fn hash64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproducible() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn below_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.below(128);
            assert!(v < 128);
        }
    }

    #[test]
    fn rough_uniformity() {
        // 128칸 히스토그램이 심하게 편향되지 않는지(카이제곱 대용 간이 검사)
        let mut r = Rng::new(1);
        let mut hist = [0u32; 128];
        let n = 128 * 500;
        for _ in 0..n {
            hist[r.below(128) as usize] += 1;
        }
        let expected = 500.0;
        for &h in hist.iter() {
            let dev = (h as f64 - expected).abs() / expected;
            assert!(dev < 0.30, "bucket deviation too large: {h}");
        }
    }
}
