//! W2-E — 에너지 최소화 출력 탐색 (시도 140 사양의 구현, PRD 지정 대안).
//!
//! 가설: 남은 꼬리는 "하나의 명료한 프로그램"이 아니라 **약한 제약 여러 개의
//! 중첩이 만드는 에너지 최저점**으로 도달된다. 이산 프로그램 탐색이 아니라
//! 연속 완화(ICM) — 자유에너지 최소화 교리의 ARC 번역.
//!
//! E = w1·E1(문맥→색 일치) + w2·E2(출력 이웃쌍 일관) + w3·E3(대칭 축 불일치)
//! 채택 게이트: 같은 추론을 훈련 입력에 돌려 훈련 출력 정확 재현일 때만.

use crate::grid::{components, stamp, Grid};
use std::collections::HashMap;

const ICM_ITERS: usize = 12;

/// 입력 3×3 문맥 키(경계=10).
fn ctx_key(g: &Grid, x: usize, y: usize) -> [u8; 9] {
    let mut k = [10u8; 9];
    let mut i = 0;
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if g.in_bounds(nx, ny) {
                k[i] = g.get(nx as usize, ny as usize);
            }
            i += 1;
        }
    }
    k
}

pub struct Ebm {
    /// 문맥 → 출력색 카운트
    ctx: HashMap<[u8; 9], [f32; 10]>,
    /// 출력 이웃(수평/수직) 색쌍 카운트
    pair: [[f32; 10]; 10],
    /// 훈련 출력의 대칭성(수평/수직 축이 일관 대칭이었는가)
    sym_h: bool,
    sym_v: bool,
    /// E4 객체 보존: 훈련에서 일관된 (입력 객체 수 → 출력 객체 수) 사상(불일관이면 None)
    obj_delta: Option<i32>,
    w: [f32; 3],
}

fn is_sym_h(g: &Grid) -> bool {
    (0..g.h).all(|y| (0..g.w).all(|x| g.get(x, y) == g.get(g.w - 1 - x, y)))
}
fn is_sym_v(g: &Grid) -> bool {
    (0..g.h).all(|y| (0..g.w).all(|x| g.get(x, y) == g.get(x, g.h - 1 - y)))
}

/// 3×3 패치의 8정이면군 변형(등가류 문맥 증강용).
fn dihedral9(k: &[u8; 9]) -> Vec<[u8; 9]> {
    let idx = |x: usize, y: usize| y * 3 + x;
    let mut out = Vec::with_capacity(8);
    for t in 0..8u8 {
        let mut v = [0u8; 9];
        for y in 0..3 {
            for x in 0..3 {
                let (mut nx, mut ny) = (x, y);
                if t & 1 != 0 {
                    nx = 2 - nx; // 수평 반전
                }
                if t & 2 != 0 {
                    ny = 2 - ny; // 수직 반전
                }
                if t & 4 != 0 {
                    std::mem::swap(&mut nx, &mut ny); // 전치
                }
                v[idx(nx, ny)] = k[idx(x, y)];
            }
        }
        out.push(v);
    }
    out
}

impl Ebm {
    pub fn learn(train: &[(Grid, Grid)]) -> Ebm {
        Ebm::learn_w(train, [1.0, 0.3, 0.5], false)
    }

    pub fn learn_w(train: &[(Grid, Grid)], w: [f32; 3], augment: bool) -> Ebm {
        let mut ctx: HashMap<[u8; 9], [f32; 10]> = HashMap::new();
        let mut pair = [[0.1f32; 10]; 10];
        for (i, o) in train {
            for y in 0..i.h {
                for x in 0..i.w {
                    let k = ctx_key(i, x, y);
                    let c = o.get(x, y) as usize;
                    let e = ctx.entry(k).or_insert([0.1; 10]);
                    e[c] += 1.0;
                    // 등가류 증강: 문맥의 기하 변형에도 같은 출력색(약한 가중)
                    if augment {
                        for v in dihedral9(&k) {
                            let e = ctx.entry(v).or_insert([0.1; 10]);
                            e[c] += 0.25;
                        }
                    }
                    if x + 1 < o.w {
                        pair[o.get(x, y) as usize][o.get(x + 1, y) as usize] += 1.0;
                    }
                    if y + 1 < o.h {
                        pair[o.get(x, y) as usize][o.get(x, y + 1) as usize] += 1.0;
                    }
                }
            }
        }
        let sym_h = train.iter().all(|(_, o)| is_sym_h(o));
        let sym_v = train.iter().all(|(_, o)| is_sym_v(o));
        // E4: (출력 객체 수 − 입력 객체 수)가 전 훈련쌍에서 일관되면 기대 델타
        let deltas: Vec<i32> = train
            .iter()
            .map(|(i, o)| components(o).len() as i32 - components(i).len() as i32)
            .collect();
        let obj_delta = if deltas.windows(2).all(|w| w[0] == w[1]) {
            deltas.first().copied()
        } else {
            None
        };
        Ebm { ctx, pair, sym_h, sym_v, obj_delta, w }
    }

    /// 전체 에너지(객체 항 포함) — 객체 제안 패스의 수용 판정에 쓴다.
    fn total_energy(&self, input: &Grid, out: &Grid) -> f32 {
        let (w, h) = (out.w, out.h);
        let mut e = 0.0f32;
        for y in 0..h {
            for x in 0..w {
                let k = ctx_key(input, x, y);
                let c = out.get(x, y) as usize;
                e += self.w[0] * self.e1(&k, c);
                if x + 1 < w {
                    e += self.w[1] * self.pair_e(c, out.get(x + 1, y) as usize);
                }
                if y + 1 < h {
                    e += self.w[1] * self.pair_e(c, out.get(x, y + 1) as usize);
                }
                if self.sym_h && out.get(w - 1 - x, y) as usize != c {
                    e += self.w[2];
                }
                if self.sym_v && out.get(x, h - 1 - y) as usize != c {
                    e += self.w[2];
                }
            }
        }
        // E4: 객체 수 기대 이탈(훈련이 일관 사상일 때만, 셀 스케일 벌점)
        if let Some(d) = self.obj_delta {
            let expect = components(input).len() as i32 + d;
            let got = components(out).len() as i32;
            e += 3.0 * (expect - got).abs() as f32;
        }
        e
    }

    /// 객체 제안 패스: 출력의 각 객체에 {±1 이동(4방향), 팔레트 재색, 삭제}를
    /// 제안하고 전체 에너지가 줄면 수용. 셀 ICM과 교대(2단 완화).
    fn object_pass(&self, input: &Grid, out: &mut Grid) -> bool {
        let mut changed = false;
        let palette: Vec<u8> = {
            let mut p: Vec<u8> = self
                .ctx
                .values()
                .flat_map(|c| (0..10).filter(move |&i| c[i] > 0.5).map(|i| i as u8))
                .collect();
            p.sort_unstable();
            p.dedup();
            p
        };
        let base_e = self.total_energy(input, out);
        let mut cur_e = base_e;
        for obj in components(&out.clone()) {
            // 후보 격자 생성: 객체 지우고 변형해 되찍기
            let mut erased = out.clone();
            for dy in 0..obj.h {
                for dx in 0..obj.w {
                    if obj.mask[dy * obj.w + dx] {
                        erased.set(obj.x0 + dx, obj.y0 + dy, 0);
                    }
                }
            }
            let mut cands: Vec<Grid> = Vec::new();
            cands.push(erased.clone()); // 삭제
            for (sx, sy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (obj.x0 as i32 + sx, obj.y0 as i32 + sy);
                if nx >= 0
                    && ny >= 0
                    && nx as usize + obj.w <= out.w
                    && ny as usize + obj.h <= out.h
                {
                    let mut g = erased.clone();
                    stamp(&mut g, &obj, nx as usize, ny as usize, obj.color);
                    cands.push(g);
                }
            }
            for &c in &palette {
                if c != obj.color && c != 0 {
                    let mut g = erased.clone();
                    stamp(&mut g, &obj, obj.x0, obj.y0, c);
                    cands.push(g);
                }
            }
            for cand in cands {
                let e = self.total_energy(input, &cand);
                if e + 1e-6 < cur_e {
                    *out = cand;
                    cur_e = e;
                    changed = true;
                    break; // 이 객체에 대해 첫 개선 수용(결정론)
                }
            }
        }
        changed
    }

    fn e1(&self, k: &[u8; 9], c: usize) -> f32 {
        match self.ctx.get(k) {
            Some(cnt) => {
                let s: f32 = cnt.iter().sum();
                -(cnt[c] / s).ln()
            }
            None => 2.3, // 미지 문맥 — 균등 수준 벌점
        }
    }

    fn pair_e(&self, a: usize, b: usize) -> f32 {
        let s: f32 = self.pair[a].iter().sum();
        -(self.pair[a][b] / s).ln() * 0.2
    }

    /// ICM 추론(결정론): 초기 = 셀별 E1 최빈, 이후 조건부 최소화 반복.
    pub fn infer(&self, input: &Grid) -> Grid {
        let (w, h) = (input.w, input.h);
        let mut out = Grid::new(w, h);
        let keys: Vec<[u8; 9]> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| ctx_key(input, x, y))
            .collect();
        for y in 0..h {
            for x in 0..w {
                let k = &keys[y * w + x];
                let mut bc = 0usize;
                let mut bv = f32::INFINITY;
                for c in 0..10 {
                    let v = self.e1(k, c);
                    if v < bv {
                        bv = v;
                        bc = c;
                    }
                }
                out.set(x, y, bc as u8);
            }
        }
        for _ in 0..ICM_ITERS {
            let mut changed = false;
            for y in 0..h {
                for x in 0..w {
                    let k = &keys[y * w + x];
                    let cur = out.get(x, y) as usize;
                    let mut bc = cur;
                    let mut bv = f32::INFINITY;
                    for c in 0..10 {
                        let mut e = self.w[0] * self.e1(k, c);
                        // E2: 4이웃 쌍
                        if x > 0 {
                            e += self.w[1] * self.pair_e(out.get(x - 1, y) as usize, c);
                        }
                        if x + 1 < w {
                            e += self.w[1] * self.pair_e(c, out.get(x + 1, y) as usize);
                        }
                        if y > 0 {
                            e += self.w[1] * self.pair_e(out.get(x, y - 1) as usize, c);
                        }
                        if y + 1 < h {
                            e += self.w[1] * self.pair_e(c, out.get(x, y + 1) as usize);
                        }
                        // E3: 대칭 축(훈련 출력이 일관 대칭이었을 때만)
                        if self.sym_h {
                            let m = out.get(w - 1 - x, y) as usize;
                            if m != c {
                                e += self.w[2];
                            }
                        }
                        if self.sym_v {
                            let m = out.get(x, h - 1 - y) as usize;
                            if m != c {
                                e += self.w[2];
                            }
                        }
                        if e < bv {
                            bv = e;
                            bc = c;
                        }
                    }
                    if bc != cur {
                        out.set(x, y, bc as u8);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        out
    }

    /// í ìì êµ­ì ìëì§(E1+E2+E3) â ICMÂ·ë´ê¸ì§ ê³µì©.
    fn local_e(&self, keys: &[[u8; 9]], out: &Grid, x: usize, y: usize, c: usize) -> f32 {
        let (w, h) = (out.w, out.h);
        let k = &keys[y * w + x];
        let mut e = self.w[0] * self.e1(k, c);
        if x > 0 {
            e += self.w[1] * self.pair_e(out.get(x - 1, y) as usize, c);
        }
        if x + 1 < w {
            e += self.w[1] * self.pair_e(c, out.get(x + 1, y) as usize);
        }
        if y > 0 {
            e += self.w[1] * self.pair_e(out.get(x, y - 1) as usize, c);
        }
        if y + 1 < h {
            e += self.w[1] * self.pair_e(c, out.get(x, y + 1) as usize);
        }
        if self.sym_h && out.get(w - 1 - x, y) as usize != c {
            e += self.w[2];
        }
        if self.sym_v && out.get(x, h - 1 - y) as usize != c {
            e += self.w[2];
        }
        e
    }

    /// ë´ê¸ì§ v2(ìë 144): ICM êµ­ì ìµì ì  íì¶ â ë©í¸ë¡í´ë¦¬ì¤+ê¸°í ëê°,
    /// ìë ê³ ì  ê²°ì ë¡ . ì´ê¸°ê° = ICM ê²°ê³¼, ë§ë¬´ë¦¬ = ì¡°ê±´ë¶ ìµìí ìë ´.
    pub fn anneal(&self, input: &Grid, seed: u64) -> Grid {
        use monad_core::rng::Rng;
        let (w, h) = (input.w, input.h);
        let keys: Vec<[u8; 9]> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| ctx_key(input, x, y))
            .collect();
        let mut out = self.infer(input);
        let mut rng = Rng::new(seed);
        let mut acc = 0f32;
        let mut best_acc = 0f32;
        let mut best = out.clone();
        let mut t = 1.5f32;
        for _ in 0..24 {
            for y in 0..h {
                for x in 0..w {
                    let cur = out.get(x, y) as usize;
                    let c = rng.below(10) as usize;
                    if c == cur {
                        continue;
                    }
                    let de = self.local_e(&keys, &out, x, y, c)
                        - self.local_e(&keys, &out, x, y, cur);
                    if de <= 0.0 || rng.next_f64() < ((-de / t) as f64).exp() {
                        out.set(x, y, c as u8);
                        acc += de;
                        if acc < best_acc {
                            best_acc = acc;
                            best = out.clone();
                        }
                    }
                }
            }
            t *= 0.82;
        }
        // 마무리: 최저 상태에서 조건부 최소화로 수렴
        let mut o = best;
        for _ in 0..ICM_ITERS {
            let mut changed = false;
            for y in 0..h {
                for x in 0..w {
                    let cur = o.get(x, y) as usize;
                    let mut bc = cur;
                    let mut bv = f32::INFINITY;
                    for c in 0..10 {
                        let e = self.local_e(&keys, &o, x, y, c);
                        if e < bv {
                            bv = e;
                            bc = c;
                        }
                    }
                    if bc != cur {
                        o.set(x, y, bc as u8);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        o
    }
}

/// 학습 → 훈련 정확 재현 게이트 → 시험 추론.
/// 확장(시도 142): 가중치 소형 그리드 × 등가류 증강을 순서대로 시험 —
/// 훈련 정확 게이트를 처음 통과하는 구성을 채택(오컴: 단순 구성 우선).
pub fn ebm_solve(train: &[(Grid, Grid)], test_in: &Grid) -> Option<Grid> {
    if train.iter().any(|(i, o)| i.w != o.w || i.h != o.h) {
        return None;
    }
    let configs: [([f32; 3], bool); 5] = [
        ([1.0, 0.3, 0.5], false), // v1 기본
        ([1.0, 0.0, 0.0], false), // 문맥 단독
        ([1.0, 0.6, 0.5], false), // 이웃 강화
        ([1.0, 0.3, 1.5], false), // 대칭 강화
        ([1.0, 0.3, 0.5], true),  // 등가류 증강
    ];
    for (w, aug) in configs {
        let ebm = Ebm::learn_w(train, w, aug);
        if train.iter().all(|(i, o)| &ebm.infer(i) == o) {
            return Some(ebm.infer(test_in));
        }
        // 2단 교대 완화(시도 143): 셀 ICM → 객체 제안 패스 → (변화 시) 반복.
        let relax = |g: &Grid| -> Grid {
            let mut out = ebm.infer(g);
            for _ in 0..3 {
                if !ebm.object_pass(g, &mut out) {
                    break;
                }
            }
            out
        };
        if train.iter().all(|(i, o)| &relax(i) == o) {
            return Some(relax(test_in));
        }
    }
    // ë´ê¸ì§ v2(ìë 144): ê¸°ë³¸ êµ¬ì± íì  Â· ì â¤400 ë¹ì© ë³´í¸ Â· ìë ê³ ì .
    if test_in.w * test_in.h <= 400
        && train.iter().all(|(i, _)| i.w * i.h <= 400)
    {
        let ebm = Ebm::learn_w(train, [1.0, 0.3, 0.5], false);
        if train
            .iter()
            .enumerate()
            .all(|(k, (i, o))| &ebm.anneal(i, 0xEB77 ^ k as u64) == o)
        {
            return Some(ebm.anneal(test_in, 0xEB77 ^ 99));
        }
    }
    None
}
