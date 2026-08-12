//! A2 — 연상 메모리 (Associative Store).
//!
//! `cleanup` 연산의 물리적 구현: 잡음 섞인 질의 벡터로 저장된 원자 중 가장
//! 가까운 것을 찾는다. MONAD에서 이것은 "이 지각이 아는 무엇인가?"라는 질문이며,
//! 정착(B2)·계획(B4)·스키마 매칭(C3)이 전부 이 한 연산 위에 선다.
//!
//! # 왜 선형 스캔이 안 되는가
//!
//! 100만 원자를 매 틱 20회 스캔하면 CPU 예산을 전부 먹는다. 그러나 A1에서
//! **온전한 블록 16개면 10만 개 중 원본을 식별할 수 있음**을 측정했다. 즉 전체를
//! 볼 필요가 없다.
//!
//! # 밴딩 색인
//!
//! 블록을 4개씩 묶어 밴드를 만들고, 밴드 값을 키로 역색인한다. 두 벡터가 한 밴드에서
//! 충돌하려면 그 4블록이 **모두** 같아야 한다.
//!
//! - 일치율 p인 진짜 이웃: 밴드 하나가 걸릴 확률 p⁴, 밴드 B개 중 하나라도 걸릴 확률
//!   1−(1−p⁴)^B. p=0.8(블록 20% 손상), B=16 → 99.97%
//! - 무작위 벡터: p≈1/128 → p⁴ ≈ 3.7e-9. 100만 개를 넣어도 오검출은 사실상 0
//!
//! 후보를 좁힌 뒤에만 정확한 해밍 거리를 계산하므로 **정확도 손실 없이** 빨라진다.

use crate::sbv::{Sbv, NBLOCKS};
use std::collections::HashMap;

/// 한 밴드를 이루는 블록 수. 크면 후보가 줄고(빠름) 잡음 내성이 떨어진다.
pub const BAND_BLOCKS: usize = 4;
/// 기본 밴드 수. 16 × 4 = 블록 64개를 색인에 사용한다.
pub const DEFAULT_BANDS: usize = 16;

/// 조회 결과 한 건.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub id: u32,
    pub dist: u32,
}

pub struct Store {
    ids: Vec<u32>,
    vecs: Vec<Sbv>,
    bands: usize,
    /// 밴드별 역색인: 밴드 키 → 원자 슬롯 번호 목록.
    index: Vec<HashMap<u32, Vec<u32>>>,
    /// 외부 id → 슬롯 번호.
    by_id: HashMap<u32, u32>,
}

impl Store {
    pub fn new() -> Self {
        Store::with_bands(DEFAULT_BANDS)
    }

    pub fn with_bands(bands: usize) -> Self {
        assert!(bands * BAND_BLOCKS <= NBLOCKS, "밴드가 블록 수를 넘을 수 없다");
        Store {
            ids: Vec::new(),
            vecs: Vec::new(),
            bands,
            index: (0..bands).map(|_| HashMap::new()).collect(),
            by_id: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    #[inline]
    fn band_key(v: &Sbv, band: usize) -> u32 {
        let o = band * BAND_BLOCKS;
        (v.idx[o] as u32)
            | ((v.idx[o + 1] as u32) << 8)
            | ((v.idx[o + 2] as u32) << 16)
            | ((v.idx[o + 3] as u32) << 24)
    }

    /// 원자를 넣는다. 같은 id를 다시 넣으면 벡터를 교체한다.
    pub fn insert(&mut self, id: u32, v: Sbv) {
        if let Some(&slot) = self.by_id.get(&id) {
            let old = self.vecs[slot as usize];
            if old == v {
                return;
            }
            self.unindex(slot, &old);
            self.vecs[slot as usize] = v;
            self.index_slot(slot, &v);
            return;
        }
        let slot = self.ids.len() as u32;
        self.ids.push(id);
        self.vecs.push(v);
        self.by_id.insert(id, slot);
        self.index_slot(slot, &v);
    }

    fn index_slot(&mut self, slot: u32, v: &Sbv) {
        for b in 0..self.bands {
            self.index[b].entry(Self::band_key(v, b)).or_default().push(slot);
        }
    }

    fn unindex(&mut self, slot: u32, v: &Sbv) {
        for b in 0..self.bands {
            if let Some(list) = self.index[b].get_mut(&Self::band_key(v, b)) {
                if let Some(p) = list.iter().position(|&s| s == slot) {
                    list.swap_remove(p);
                }
            }
        }
    }

    pub fn get(&self, id: u32) -> Option<&Sbv> {
        self.by_id.get(&id).map(|&s| &self.vecs[s as usize])
    }

    /// 가장 가까운 k개. 밴딩으로 후보를 좁힌 뒤 정확 거리로 순위를 매긴다.
    pub fn query(&self, q: &Sbv, k: usize) -> Vec<Hit> {
        let mut cands: Vec<u32> = Vec::with_capacity(64);
        let mut seen = vec![false; self.ids.len()];
        for b in 0..self.bands {
            if let Some(list) = self.index[b].get(&Self::band_key(q, b)) {
                for &slot in list {
                    let s = slot as usize;
                    if !seen[s] {
                        seen[s] = true;
                        cands.push(slot);
                    }
                }
            }
        }
        self.rank(q, &cands, k)
    }

    /// 밴딩이 아무것도 못 찾았을 때를 위한 정확 조회(느림). 검증·소규모용.
    pub fn query_exact(&self, q: &Sbv, k: usize) -> Vec<Hit> {
        let all: Vec<u32> = (0..self.ids.len() as u32).collect();
        self.rank(q, &all, k)
    }

    fn rank(&self, q: &Sbv, cands: &[u32], k: usize) -> Vec<Hit> {
        let mut hits: Vec<Hit> = cands
            .iter()
            .map(|&slot| Hit {
                id: self.ids[slot as usize],
                dist: q.dist(&self.vecs[slot as usize]),
            })
            .collect();
        hits.sort_unstable_by_key(|h| (h.dist, h.id));
        hits.truncate(k);
        hits
    }

    /// 거리 임계 안의 최근접 하나. `settle`이 "이건 아는 상태인가?"를 묻는 형태.
    pub fn nearest_within(&self, q: &Sbv, max_dist: u32) -> Option<Hit> {
        self.query(q, 1).into_iter().find(|h| h.dist <= max_dist)
    }

    /// 색인 통계(리포트/유리상자용).
    pub fn index_stats(&self) -> (usize, usize, f32) {
        let mut buckets = 0usize;
        let mut entries = 0usize;
        for b in &self.index {
            buckets += b.len();
            entries += b.values().map(|v| v.len()).sum::<usize>();
        }
        let avg = if buckets == 0 { 0.0 } else { entries as f32 / buckets as f32 };
        (buckets, entries, avg)
    }
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn corrupt(v: &Sbv, blocks: usize, r: &mut Rng) -> Sbv {
        let mut q = *v;
        let mut perm: [u8; NBLOCKS] = std::array::from_fn(|i| i as u8);
        for i in 0..blocks.min(NBLOCKS) {
            let j = i + r.below((NBLOCKS - i) as u32) as usize;
            perm.swap(i, j);
            let b = perm[i] as usize;
            q.idx[b] = (q.idx[b] + 1 + r.below(127) as u8) & 127;
        }
        q
    }

    #[test]
    fn exact_lookup() {
        let mut r = Rng::new(1);
        let mut s = Store::new();
        let vs: Vec<Sbv> = (0..2000).map(|_| Sbv::random(&mut r)).collect();
        for (i, v) in vs.iter().enumerate() {
            s.insert(i as u32, *v);
        }
        for (i, v) in vs.iter().enumerate() {
            let h = s.query(v, 1);
            assert_eq!(h[0].id, i as u32);
            assert_eq!(h[0].dist, 0);
        }
    }

    #[test]
    fn noisy_recall_at_20pct() {
        // A2 DoD: 20% 잡음에서 리콜@8 ≥ 99%
        let mut r = Rng::new(2);
        let n = 20_000usize;
        let mut s = Store::new();
        let vs: Vec<Sbv> = (0..n).map(|_| Sbv::random(&mut r)).collect();
        for (i, v) in vs.iter().enumerate() {
            s.insert(i as u32, *v);
        }
        let trials = 500;
        let mut hit = 0;
        for _ in 0..trials {
            let t = r.below(n as u32) as usize;
            let q = corrupt(&vs[t], 26, &mut r); // 26/128 ≈ 20%
            if s.query(&q, 8).iter().any(|h| h.id == t as u32) {
                hit += 1;
            }
        }
        let recall = hit as f64 / trials as f64;
        assert!(recall >= 0.99, "리콜@8 = {recall:.4}");
    }

    #[test]
    fn banding_agrees_with_exact_when_found() {
        let mut r = Rng::new(3);
        let mut s = Store::new();
        let vs: Vec<Sbv> = (0..3000).map(|_| Sbv::random(&mut r)).collect();
        for (i, v) in vs.iter().enumerate() {
            s.insert(i as u32, *v);
        }
        for _ in 0..200 {
            let t = r.below(3000) as usize;
            let q = corrupt(&vs[t], 20, &mut r);
            let fast = s.query(&q, 1);
            let slow = s.query_exact(&q, 1);
            if !fast.is_empty() {
                // 밴딩이 후보를 찾았다면 그 최근접은 정확 조회와 같아야 한다
                assert_eq!(fast[0], slow[0]);
            }
        }
    }

    #[test]
    fn replace_updates_index() {
        let mut r = Rng::new(4);
        let mut s = Store::new();
        let a = Sbv::random(&mut r);
        let b = Sbv::random(&mut r);
        s.insert(7, a);
        assert_eq!(s.query(&a, 1)[0].id, 7);
        s.insert(7, b);
        assert_eq!(s.len(), 1);
        assert_eq!(s.query(&b, 1)[0].dist, 0);
        // 옛 벡터로는 더 이상 7을 0거리로 찾을 수 없어야 한다
        let old = s.query(&a, 1);
        assert!(old.is_empty() || old[0].dist > 0);
    }
}
