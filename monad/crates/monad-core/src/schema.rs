//! C2 — 스키마 추출 (구조 추상화).
//!
//! 수면 패스의 두 번째 일: 사건들에서 반복 구조를 찾아 **어떤 슬롯을 변수화할지**
//! 결정하고, 사람이 읽을 수 있는 규칙(스키마)으로 만든다. C2-S 스파이크에서
//! 8/8 규칙 세계·시드 5개 40/40으로 검증된 알고리즘(MDL 압축 이득 + 전방탐색
//! 빔서치 + 예외 정제)의 Rust 이식이다.
//!
//! 판정 원리는 하나다: **스키마는 결과를 기술하는 비트를 줄일 때만 채택한다.**
//! - 과일반화(전부 변수화) → 덮은 집합이 불순해져 결과 기술 비용 상승 → 기각
//! - 과특수화(전부 고정) → 스키마 자체 비용만 늘고 이득 없음 → 기각
//!
//! 스키마는 그대로 SBV로 이식 가능하다:
//! `schema_id = bundle[ bind(role_slot, value_atom) ]` — 변수화된 슬롯은 넣지 않는다.
//! A1 용량 곡선(K=16, 98.7%)이 스키마당 제약 수 상한을 준다(현재 최대 4로 여유).

use std::collections::HashMap;

const LAMBDA: f64 = 0.5; // KT 평활

/// 사건: 슬롯 값들 + 결과.
#[derive(Clone, Debug)]
pub struct Event {
    /// 슬롯 값(범주형은 그대로, 수치형은 별도 배열).
    pub cats: Vec<(u16, u32)>,
    pub nums: Vec<(u16, f32)>,
    pub effect: u32,
}

impl Event {
    pub fn cat(&self, slot: u16) -> Option<u32> {
        self.cats.iter().find(|(s, _)| *s == slot).map(|(_, v)| *v)
    }
    pub fn num(&self, slot: u16) -> Option<f32> {
        self.nums.iter().find(|(s, _)| *s == slot).map(|(_, v)| *v)
    }
}

/// 제약: 스키마의 조건 하나.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constraint {
    /// 슬롯이 특정 값.
    Eq(u16, u32),
    /// 두 슬롯이 서로 같음(관계적 추상화).
    EqSlot(u16, u16),
    /// 수치 슬롯 ≥ 임계.
    Ge(u16, f32),
    /// 수치 슬롯 < 임계.
    Lt(u16, f32),
}

impl Constraint {
    pub fn matches(&self, ev: &Event) -> bool {
        match *self {
            Constraint::Eq(s, v) => ev.cat(s) == Some(v),
            Constraint::EqSlot(a, b) => {
                ev.cat(a).is_some() && ev.cat(a) == ev.cat(b)
            }
            Constraint::Ge(s, t) => ev.num(s).map(|x| x >= t).unwrap_or(false),
            Constraint::Lt(s, t) => ev.num(s).map(|x| x < t).unwrap_or(false),
        }
    }
    pub fn slots(&self) -> Vec<u16> {
        match *self {
            Constraint::Eq(s, _) | Constraint::Ge(s, _) | Constraint::Lt(s, _) => vec![s],
            Constraint::EqSlot(a, b) => vec![a, b],
        }
    }
}

/// 스키마: 제약 집합 → 결과. 증거·반례 카운트 포함(유리상자 덤프의 원천).
#[derive(Clone, Debug)]
pub struct Schema {
    pub constraints: Vec<Constraint>,
    pub effect: u32,
    pub evidence: u32,
    pub counterexamples: u32,
    pub gain: f64,
}

impl Schema {
    pub fn matches(&self, ev: &Event) -> bool {
        self.constraints.iter().all(|c| c.matches(ev))
    }
    /// NARS식 신뢰도.
    pub fn confidence(&self) -> f32 {
        let w = self.evidence as f32;
        let n = self.counterexamples as f32;
        (w - n).max(0.0) / (w + 1.0)
    }
    pub fn slots(&self) -> Vec<u16> {
        let mut v: Vec<u16> = self.constraints.iter().flat_map(|c| c.slots()).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
    /// 사람이 읽는 형식(D1 관문의 필수 산출물).
    pub fn describe(&self, slot_name: &dyn Fn(u16) -> String, effect_name: &dyn Fn(u32) -> String) -> String {
        let cond = if self.constraints.is_empty() {
            "(항상)".to_string()
        } else {
            self.constraints
                .iter()
                .map(|c| match *c {
                    Constraint::Eq(s, v) => format!("{}={}", slot_name(s), v),
                    Constraint::EqSlot(a, b) => format!("{}=={}", slot_name(a), slot_name(b)),
                    Constraint::Ge(s, t) => format!("{}≥{:.2}", slot_name(s), t),
                    Constraint::Lt(s, t) => format!("{}<{:.2}", slot_name(s), t),
                })
                .collect::<Vec<_>>()
                .join(" ∧ ")
        };
        format!(
            "IF {} THEN {}   [evidence={} counterexamples={} confidence={:.2}]",
            cond,
            effect_name(self.effect),
            self.evidence,
            self.counterexamples,
            self.confidence()
        )
    }
}

/// 스키마 라이브러리: 구체적인 것(제약 많은 것)이 먼저 적용된다 — 예외가 일반을 이긴다.
#[derive(Clone, Debug, Default)]
pub struct SchemaLib {
    pub schemas: Vec<Schema>,
    pub default_effect: Option<u32>,
}

impl SchemaLib {
    pub fn predict(&self, ev: &Event) -> Option<u32> {
        for s in &self.schemas {
            if s.matches(ev) {
                return Some(s.effect);
            }
        }
        self.default_effect
    }
    pub fn is_empty(&self) -> bool {
        self.schemas.is_empty()
    }
}

// ------------------------------------------------------------------ 데이터셋

/// 64비트 워드 비트셋 — 사건 수천 개 규모에 충분.
#[derive(Clone, PartialEq)]
struct Mask {
    w: Vec<u64>,
    n: usize,
}
impl Mask {
    fn zeros(n: usize) -> Self {
        Mask { w: vec![0; (n + 63) / 64], n }
    }
    fn ones(n: usize) -> Self {
        let mut m = Mask::zeros(n);
        for i in 0..n {
            m.set(i);
        }
        m
    }
    #[inline]
    fn set(&mut self, i: usize) {
        self.w[i / 64] |= 1 << (i % 64);
    }
    #[inline]
    fn get(&self, i: usize) -> bool {
        (self.w[i / 64] >> (i % 64)) & 1 == 1
    }
    fn count(&self) -> u32 {
        self.w.iter().map(|x| x.count_ones()).sum()
    }
    fn and(&self, o: &Mask) -> Mask {
        Mask { w: self.w.iter().zip(&o.w).map(|(a, b)| a & b).collect(), n: self.n }
    }
    fn and_not(&self, o: &Mask) -> Mask {
        Mask { w: self.w.iter().zip(&o.w).map(|(a, b)| a & !b).collect(), n: self.n }
    }
    fn is_zero(&self) -> bool {
        self.w.iter().all(|&x| x == 0)
    }
}

struct Ds<'a> {
    events: &'a [Event],
    effects: Vec<u32>,
    effect_mask: Vec<Mask>,
    cand: Vec<Constraint>,
    cand_mask: Vec<Mask>,
    vocab_bits: f64,
    all: Mask,
}

impl<'a> Ds<'a> {
    fn new(events: &'a [Event], max_thresholds: usize) -> Self {
        let n = events.len();
        let mut effects: Vec<u32> = events.iter().map(|e| e.effect).collect();
        effects.sort_unstable();
        effects.dedup();
        let effect_mask: Vec<Mask> = effects
            .iter()
            .map(|&eff| {
                let mut m = Mask::zeros(n);
                for (i, e) in events.iter().enumerate() {
                    if e.effect == eff {
                        m.set(i);
                    }
                }
                m
            })
            .collect();

        let mut cand: Vec<Constraint> = Vec::new();
        let mut cand_mask: Vec<Mask> = Vec::new();
        let all = Mask::ones(n);
        let mut push = |c: Constraint, m: Mask, cand: &mut Vec<Constraint>, cm: &mut Vec<Mask>| {
            if !m.is_zero() && m != Mask::ones(n) {
                cand.push(c);
                cm.push(m);
            }
        };

        // 범주 슬롯 수집
        let mut cat_slots: Vec<u16> = Vec::new();
        let mut cat_domain: HashMap<u16, Vec<u32>> = HashMap::new();
        for e in events {
            for &(s, v) in &e.cats {
                if !cat_slots.contains(&s) {
                    cat_slots.push(s);
                }
                let d = cat_domain.entry(s).or_default();
                if !d.contains(&v) {
                    d.push(v);
                }
            }
        }
        cat_slots.sort_unstable();

        // 1) 값 고정
        for &s in &cat_slots {
            for &v in &cat_domain[&s] {
                let mut m = Mask::zeros(n);
                for (i, e) in events.iter().enumerate() {
                    if e.cat(s) == Some(v) {
                        m.set(i);
                    }
                }
                push(Constraint::Eq(s, v), m, &mut cand, &mut cand_mask);
            }
        }
        // 2) 관계(같은 도메인 크기의 슬롯 쌍만 — 근사)
        for i in 0..cat_slots.len() {
            for j in (i + 1)..cat_slots.len() {
                let (a, b) = (cat_slots[i], cat_slots[j]);
                let (da, db) = (&cat_domain[&a], &cat_domain[&b]);
                if !da.iter().any(|v| db.contains(v)) {
                    continue;
                }
                let mut m = Mask::zeros(n);
                for (k, e) in events.iter().enumerate() {
                    if e.cat(a).is_some() && e.cat(a) == e.cat(b) {
                        m.set(k);
                    }
                }
                push(Constraint::EqSlot(a, b), m, &mut cand, &mut cand_mask);
            }
        }
        // 3) 수치 임계(결과가 바뀌는 경계의 중점만)
        let mut num_slots: Vec<u16> = Vec::new();
        for e in events {
            for &(s, _) in &e.nums {
                if !num_slots.contains(&s) {
                    num_slots.push(s);
                }
            }
        }
        for &s in &num_slots {
            let mut pairs: Vec<(f32, u32)> = events
                .iter()
                .filter_map(|e| e.num(s).map(|x| (x, e.effect)))
                .collect();
            pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            let mut thr: Vec<f32> = Vec::new();
            for k in 1..pairs.len() {
                if pairs[k].1 != pairs[k - 1].1 && pairs[k].0 != pairs[k - 1].0 {
                    thr.push((pairs[k].0 + pairs[k - 1].0) / 2.0);
                }
            }
            thr.dedup();
            if thr.len() > max_thresholds {
                let step = thr.len() as f32 / max_thresholds as f32;
                thr = (0..max_thresholds).map(|i| thr[(i as f32 * step) as usize]).collect();
            }
            for t in thr {
                let mut mge = Mask::zeros(n);
                for (i, e) in events.iter().enumerate() {
                    if e.num(s).map(|x| x >= t).unwrap_or(false) {
                        mge.set(i);
                    }
                }
                let mlt = all.and_not(&mge);
                push(Constraint::Ge(s, t), mge, &mut cand, &mut cand_mask);
                push(Constraint::Lt(s, t), mlt, &mut cand, &mut cand_mask);
            }
        }

        let vocab_bits = ((cand.len() + 1) as f64).log2();
        Ds { events, effects, effect_mask, cand, cand_mask, vocab_bits, all }
    }

    /// 마스크가 가리키는 사건들의 결과 기술 비용(비트).
    fn code_len(&self, m: &Mask) -> f64 {
        let n = m.count() as f64;
        if n == 0.0 {
            return 0.0;
        }
        let k = self.effects.len() as f64;
        let denom = n + LAMBDA * k;
        let mut total = 0.0;
        for em in &self.effect_mask {
            let c = m.and(em).count() as f64;
            if c > 0.0 {
                total -= c * ((c + LAMBDA) / denom).log2();
            }
        }
        total
    }

    fn gain(&self, active: &Mask, covered: &Mask, k_constraints: usize) -> f64 {
        if covered.is_zero() {
            return f64::NEG_INFINITY;
        }
        let rest = active.and_not(covered);
        let after = self.code_len(covered)
            + self.code_len(&rest)
            + (k_constraints as f64 + 1.0) * self.vocab_bits;
        self.code_len(active) - after
    }

    fn majority(&self, m: &Mask) -> (u32, f64, u32) {
        let mut best = (self.effects[0], 0u32);
        for (i, em) in self.effect_mask.iter().enumerate() {
            let c = m.and(em).count();
            if c > best.1 {
                best = (self.effects[i], c);
            }
        }
        let n = m.count();
        (best.0, if n > 0 { best.1 as f64 / n as f64 } else { 0.0 }, n)
    }
}

// ------------------------------------------------------------------ 탐색

#[derive(Clone)]
struct Pat {
    cons: Vec<usize>, // 후보 색인
    mask: Mask,
    gain: f64,
}

fn conflicts(ds: &Ds<'_>, cons: &[usize], c: usize) -> bool {
    if cons.contains(&c) {
        return true;
    }
    // 같은 슬롯의 값 고정 중복 방지
    if let Constraint::Eq(s, _) = ds.cand[c] {
        for &o in cons {
            for os in ds.cand[o].slots() {
                if os == s {
                    return true;
                }
            }
        }
    }
    false
}

fn beam_search(ds: &Ds<'_>, active: &Mask, beam_width: usize, max_depth: usize) -> Option<Pat> {
    let mut best: Option<Pat> = None;
    let mut beam: Vec<Pat> = vec![Pat { cons: Vec::new(), mask: active.clone(), gain: 0.0 }];

    for _ in 0..max_depth {
        let mut scored: Vec<(f64, Pat)> = Vec::new();
        for base in &beam {
            for c in 0..ds.cand.len() {
                if conflicts(ds, &base.cons, c) {
                    continue;
                }
                let m = base.mask.and(&ds.cand_mask[c]);
                if m.is_zero() || m == base.mask {
                    continue;
                }
                let mut cons = base.cons.clone();
                cons.push(c);
                cons.sort_unstable();
                let g = ds.gain(active, &m, cons.len());
                let pat = Pat { cons, mask: m, gain: g };
                if best.as_ref().map(|b| g > b.gain).unwrap_or(true) {
                    best = Some(pat.clone());
                }
                // 낙관적(한 수 앞) 점수 — 논리곱 발견의 열쇠
                let mut score = g;
                for c2 in 0..ds.cand.len() {
                    if conflicts(ds, &pat.cons, c2) {
                        continue;
                    }
                    let m2 = pat.mask.and(&ds.cand_mask[c2]);
                    if m2.is_zero() || m2 == pat.mask {
                        continue;
                    }
                    let g2 = ds.gain(active, &m2, pat.cons.len() + 1);
                    if g2 > score {
                        score = g2;
                    }
                }
                scored.push((score, pat));
            }
        }
        if scored.is_empty() {
            break;
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.truncate(beam_width);
        beam = scored.into_iter().map(|(_, p)| p).collect();
    }
    best
}

#[derive(Clone, Copy, Debug)]
pub struct InduceConfig {
    pub beam_width: usize,
    pub max_depth: usize,
    pub min_support: u32,
    pub max_rules: usize,
    pub exception_depth: usize,
    pub max_thresholds: usize,
}

impl Default for InduceConfig {
    fn default() -> Self {
        InduceConfig {
            beam_width: 8,
            max_depth: 4,
            min_support: 6,
            max_rules: 8,
            exception_depth: 2,
            max_thresholds: 16,
        }
    }
}

/// 순차 피복 + 예외 정제 — C2-S 스파이크에서 검증된 그 절차.
pub fn induce(events: &[Event], cfg: InduceConfig) -> SchemaLib {
    if events.is_empty() {
        return SchemaLib::default();
    }
    let ds = Ds::new(events, cfg.max_thresholds);
    let mut active = ds.all.clone();
    let mut out: Vec<Schema> = Vec::new();

    for _ in 0..cfg.max_rules {
        if active.count() < cfg.min_support {
            break;
        }
        let p = match beam_search(&ds, &active, cfg.beam_width, cfg.max_depth) {
            Some(p) if p.gain > 0.0 && !p.cons.is_empty() => p,
            _ => break,
        };
        let (eff, purity, support) = ds.majority(&p.mask);
        let errs = p.mask.count() - p.mask.and(&ds.effect_mask[ds.effects.iter().position(|&e| e == eff).unwrap()]).count();
        out.push(Schema {
            constraints: p.cons.iter().map(|&i| ds.cand[i]).collect(),
            effect: eff,
            evidence: support - errs,
            counterexamples: errs,
            gain: p.gain,
        });

        // 예외 정제: 덮은 범위 안에서 체계적으로 틀리는 부분을 더 구체적인 스키마로
        let mut parent_mask = p.mask.clone();
        let mut parent_cons = p.cons.clone();
        let mut parent_eff = eff;
        for _ in 0..cfg.exception_depth {
            let perr = {
                let ei = ds.effects.iter().position(|&e| e == parent_eff).unwrap();
                parent_mask.and_not(&ds.effect_mask[ei])
            };
            if perr.count() < cfg.min_support {
                break;
            }
            let sub = match beam_search(&ds, &parent_mask, cfg.beam_width, cfg.max_depth) {
                Some(s) if s.gain > 0.0 && !s.cons.is_empty() => s,
                _ => break,
            };
            let (seff, spurity, ssup) = ds.majority(&sub.mask);
            if seff == parent_eff || spurity < 0.5 {
                break;
            }
            let mut cons = parent_cons.clone();
            for c in &sub.cons {
                if !cons.contains(c) {
                    cons.push(*c);
                }
            }
            cons.sort_unstable();
            let serrs = ssup
                - sub
                    .mask
                    .and(&ds.effect_mask[ds.effects.iter().position(|&e| e == seff).unwrap()])
                    .count();
            out.push(Schema {
                constraints: cons.iter().map(|&i| ds.cand[i]).collect(),
                effect: seff,
                evidence: ssup - serrs,
                counterexamples: serrs,
                gain: sub.gain,
            });
            parent_mask = sub.mask;
            parent_cons = cons;
            parent_eff = seff;
            let _ = spurity;
        }

        active = active.and_not(&p.mask);
        let _ = purity;
    }

    // 구체적인 것 먼저
    out.sort_by_key(|s| std::cmp::Reverse(s.constraints.len()));
    let default_effect = if active.is_zero() {
        events.first().map(|e| e.effect)
    } else {
        Some(ds.majority(&active).0)
    };
    SchemaLib { schemas: out, default_effect }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    /// R1 동형: 단일 조건 + 무관 슬롯 — 무관한 것을 전부 변수화하는가.
    #[test]
    fn generalizes_over_irrelevant_slots() {
        let mut r = Rng::new(1);
        let mut evs = Vec::new();
        for _ in 0..400 {
            let rigid = r.below(2);
            evs.push(Event {
                cats: vec![
                    (0, r.below(6)),  // color — 무관
                    (1, r.below(5)),  // shape — 무관
                    (2, rigid),       // rigid — 원인
                ],
                nums: vec![(3, r.next_f64() as f32 * 10.0)], // size — 무관
                effect: if rigid == 1 { 1 } else { 0 },
            });
        }
        let lib = induce(&evs, InduceConfig::default());
        assert!(!lib.is_empty());
        // 발견 슬롯이 정확히 {2}여야 한다
        let mut slots: Vec<u16> = lib.schemas.iter().flat_map(|s| s.slots()).collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots, vec![2], "무관 슬롯이 남았다: {slots:?}");
        // 전이 정확도
        let mut correct = 0;
        for _ in 0..200 {
            let rigid = r.below(2);
            let ev = Event {
                cats: vec![(0, r.below(6)), (1, r.below(5)), (2, rigid)],
                nums: vec![(3, r.next_f64() as f32 * 10.0)],
                effect: if rigid == 1 { 1 } else { 0 },
            };
            if lib.predict(&ev) == Some(ev.effect) {
                correct += 1;
            }
        }
        assert!(correct >= 198, "전이 정확도 {correct}/200");
    }

    /// R7 동형: 일반 규칙 + 예외.
    #[test]
    fn finds_exceptions() {
        let mut r = Rng::new(2);
        let mut evs = Vec::new();
        for _ in 0..600 {
            let moving = r.below(2);
            let rigid = r.below(2);
            let mat = r.below(5); // 4 = glass
            let effect = if moving == 1 && rigid == 1 {
                if mat == 4 {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            evs.push(Event {
                cats: vec![(0, moving), (1, rigid), (2, mat), (3, r.below(6))],
                nums: vec![],
                effect,
            });
        }
        let lib = induce(&evs, InduceConfig::default());
        let mut correct = 0;
        for _ in 0..300 {
            let moving = r.below(2);
            let rigid = r.below(2);
            let mat = r.below(5);
            let effect = if moving == 1 && rigid == 1 {
                if mat == 4 {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            let ev = Event {
                cats: vec![(0, moving), (1, rigid), (2, mat), (3, r.below(6))],
                nums: vec![],
                effect,
            };
            if lib.predict(&ev) == Some(effect) {
                correct += 1;
            }
        }
        assert!(correct >= 294, "예외 포함 정확도 {correct}/300");
        // 유리 예외 스키마가 실제로 존재해야 한다
        let has_exception = lib
            .schemas
            .iter()
            .any(|s| s.effect == 2 && s.constraints.iter().any(|c| matches!(c, Constraint::Eq(2, 4))));
        assert!(has_exception, "유리 예외 스키마 없음");
    }

    /// R5 동형: 수치 임계.
    #[test]
    fn finds_numeric_threshold() {
        let mut r = Rng::new(3);
        let mut evs = Vec::new();
        for _ in 0..400 {
            let speed = r.next_f64() as f32 * 12.0;
            evs.push(Event {
                cats: vec![(0, r.below(6))],
                nums: vec![(1, speed)],
                effect: if speed >= 7.0 { 1 } else { 0 },
            });
        }
        let lib = induce(&evs, InduceConfig::default());
        let mut correct = 0;
        for _ in 0..200 {
            let speed = r.next_f64() as f32 * 12.0;
            let ev = Event {
                cats: vec![(0, r.below(6))],
                nums: vec![(1, speed)],
                effect: if speed >= 7.0 { 1 } else { 0 },
            };
            if lib.predict(&ev) == Some(ev.effect) {
                correct += 1;
            }
        }
        assert!(correct >= 194, "임계 정확도 {correct}/200");
    }
}
