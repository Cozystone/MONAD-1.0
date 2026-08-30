//! GEN4 — **세계모델 핵심**: 예측하고, 틀리고, 그 오차로 자기 앎의 경계를 배운다.
//!
//! # 왜 이것이 GEN3와 다른가
//!
//! GEN3는 `관찰 → 조건 매칭 → 규칙 적용`이었다. 규칙이 훈련과 모순되지 않으면
//! 채택되고, 시험에서 미지의 자리로 **외삽**해도 그것을 막을 장치가 없었다
//! (시도 215~217: 여섯 개입이 같은 4칸을 못 없앴다).
//!
//! GEN4는 `상태 → 예측 → 오차 → 수정`이다. 핵심 차이는 **모르는 것을 안다**는 것:
//! 경험이 갈리거나 없는 자리에서는 **침묵한다.**
//!
//! # 사전등록 조건 1을 어떻게 만족하는가 (자유 파라미터 0개)
//!
//! 침묵 여부를 정하는 **임계 상수가 없다.** 판단은 전부 경험에서 유도된다:
//!
//! - 질의 맥락과 **가장 많은 슬롯이 일치하는** 경험들만 본다(일치 폭 자체가
//!   데이터가 정한다 — 고정된 이웃 반경이 없다).
//! - 그들이 **만장일치**면 그것이 예측이고, **갈리면 모른다**고 답한다.
//! - 한 번이라도 **틀린 적 있는 맥락류**는 그 뒤로 신뢰하지 않는다
//!   (`예측 → 오차 → 수정` 고리).
//!
//! `if confidence < 0.7` 같은 손으로 정한 문턱이 하나도 없다. 이것이
//! 사전등록에서 "실패로 간주"한 수동 휴리스틱과 갈리는 지점이다.
//!
//! # 도메인 중립 (사전등록 조건 4)
//!
//! 이 모듈은 격자·색·객체·ARC를 **모른다.** 맥락은 `u64` 벡터이고 결과는 `u64`
//! 코드다. 그 코드가 무엇을 뜻하는지는 어댑터의 몫이다. 같은 핵심이 ARC(정적)와
//! Bounce(시간축) 양쪽에 붙는다.

use std::collections::HashMap;

/// 한 경험: 맥락과 그때 실제로 일어난 일.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Episode {
    pub ctx: Vec<u64>,
    pub outcome: u64,
}

/// 예측 결과 — **모른다고 답할 수 있다**는 것이 이 타입의 존재 이유다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Belief {
    /// 이 맥락은 안다: 가장 가까운 경험들이 만장일치로 이 결과를 말한다.
    Known {
        outcome: u64,
        /// 근거가 된 경험들이 질의와 일치한 슬롯 수(클수록 가까운 경험).
        agreement: usize,
        /// 근거가 된 경험 수.
        support: usize,
    },
    /// 모른다. 행동하지 않는다.
    Unknown(Ignorance),
}

/// **왜** 모르는가 — 처방이 다르므로 구분해 센다(사전등록 조건 5의 계량 근거).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ignorance {
    /// 비슷한 경험이 아예 없다.
    NoExperience,
    /// 가장 가까운 경험들이 서로 다른 결과를 말한다.
    Conflict,
    /// 이 맥락류에서 예측했다가 틀린 적이 있다.
    Burned,
}

impl Belief {
    pub fn outcome(&self) -> Option<u64> {
        match self {
            Belief::Known { outcome, .. } => Some(*outcome),
            Belief::Unknown(_) => None,
        }
    }
    pub fn is_known(&self) -> bool {
        matches!(self, Belief::Known { .. })
    }
}

/// 예측·오차·수정을 담는 세계모델.
///
/// 경험만 쌓인다 — 코드는 고정이다(사전등록 조건 2).
#[derive(Clone, Debug, Default)]
pub struct WorldModel {
    episodes: Vec<Episode>,
    /// **덴 자리**: 여기서 예측했다가 틀렸다. 맥락과 그때의 일치 폭을 같이 둔다 —
    /// 같은 폭으로 같은 맥락을 다시 만나면 다시 속지 않는다.
    burned: Vec<(Vec<u64>, usize)>,
    /// 회계: 예측한 횟수 / 맞은 횟수 / 침묵한 횟수.
    pub predicted: u64,
    pub correct: u64,
    pub abstained: u64,
}

impl WorldModel {
    pub fn new() -> Self {
        WorldModel::default()
    }

    pub fn len(&self) -> usize {
        self.episodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.episodes.is_empty()
    }

    /// 두 맥락이 몇 슬롯이나 같은가.
    fn agreement(a: &[u64], b: &[u64]) -> usize {
        a.iter().zip(b.iter()).filter(|(x, y)| x == y).count()
    }

    /// 경험을 넣는다. 같은 (맥락, 결과)를 중복 저장하지 않는다.
    pub fn observe(&mut self, ctx: Vec<u64>, outcome: u64) {
        let e = Episode { ctx, outcome };
        if !self.episodes.contains(&e) {
            self.episodes.push(e);
        }
    }

    /// **예측하거나, 모른다고 말한다.**
    ///
    /// 고정된 이웃 반경이나 신뢰도 문턱이 없다는 점이 중요하다. 어디까지를
    /// "가까운 경험"으로 볼지는 **데이터가 정한다** — 질의와 가장 많이 일치하는
    /// 경험들이 곧 근거이고, 그들이 갈리면 그 사실 자체가 무지의 신호다.
    pub fn believe(&self, ctx: &[u64]) -> Belief {
        if self.episodes.is_empty() {
            return Belief::Unknown(Ignorance::NoExperience);
        }
        let best = self
            .episodes
            .iter()
            .map(|e| Self::agreement(&e.ctx, ctx))
            .max()
            .unwrap_or(0);
        if best == 0 {
            return Belief::Unknown(Ignorance::NoExperience);
        }
        // 이 맥락·이 일치 폭에서 이미 덴 적이 있으면 다시 시도하지 않는다.
        if self
            .burned
            .iter()
            .any(|(bc, ba)| *ba == best && Self::agreement(bc, ctx) >= best)
        {
            return Belief::Unknown(Ignorance::Burned);
        }
        let near: Vec<&Episode> = self
            .episodes
            .iter()
            .filter(|e| Self::agreement(&e.ctx, ctx) == best)
            .collect();
        let first = near[0].outcome;
        if near.iter().any(|e| e.outcome != first) {
            return Belief::Unknown(Ignorance::Conflict);
        }
        Belief::Known { outcome: first, agreement: best, support: near.len() }
    }

    /// **예측하고 답을 받는다** — 오차가 있으면 그 자리를 기억한다.
    ///
    /// 이것이 `예측 → 오차 → 가설 수정` 고리의 몸통이다. 틀린 자리를 남겨 두면
    /// 같은 종류의 외삽을 반복하게 된다(GEN3가 정확히 그랬다).
    pub fn learn(&mut self, ctx: Vec<u64>, actual: u64) -> Belief {
        let b = self.believe(&ctx);
        match &b {
            Belief::Known { outcome, agreement, .. } => {
                self.predicted += 1;
                if *outcome == actual {
                    self.correct += 1;
                } else {
                    let key = (ctx.clone(), *agreement);
                    if !self.burned.contains(&key) {
                        self.burned.push(key);
                    }
                }
            }
            Belief::Unknown(_) => self.abstained += 1,
        }
        self.observe(ctx, actual);
        b
    }

    /// 예측한 것 중 맞은 비율(침묵은 분모에 넣지 않는다).
    pub fn precision(&self) -> f64 {
        if self.predicted == 0 {
            0.0
        } else {
            self.correct as f64 / self.predicted as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 경험이 없으면 **모른다고 답한다.** 아무 말이나 하지 않는다.
    #[test]
    fn empty_model_knows_that_it_knows_nothing() {
        let m = WorldModel::new();
        assert_eq!(m.believe(&[1, 2, 3]), Belief::Unknown(Ignorance::NoExperience));
    }

    /// 같은 맥락을 다시 만나면 안다.
    #[test]
    fn recalls_what_it_has_seen() {
        let mut m = WorldModel::new();
        m.observe(vec![1, 2, 3], 7);
        assert_eq!(m.believe(&[1, 2, 3]).outcome(), Some(7));
    }

    /// **일반화**: 무관한 슬롯이 달라도 가장 가까운 경험이 만장일치면 안다.
    /// (경험이 아니라 규칙을 외우는 것이 아니라는 점이 중요하다.)
    #[test]
    fn generalizes_to_nearest_unanimous_experience() {
        let mut m = WorldModel::new();
        m.observe(vec![1, 2, 10], 7);
        m.observe(vec![1, 2, 20], 7);
        // 세 번째 슬롯은 결과와 무관했다 — 새 값에서도 안다고 말해야 한다
        match m.believe(&[1, 2, 30]) {
            Belief::Known { outcome, agreement, support } => {
                assert_eq!(outcome, 7);
                assert_eq!(agreement, 2);
                assert_eq!(support, 2);
            }
            other => panic!("일반화 실패: {other:?}"),
        }
    }

    /// **갈리면 모른다.** 가장 가까운 경험들이 다른 결과를 말하면 침묵한다.
    #[test]
    fn conflicting_experience_yields_ignorance_not_a_guess() {
        let mut m = WorldModel::new();
        m.observe(vec![1, 2, 10], 7);
        m.observe(vec![1, 2, 20], 9);
        assert_eq!(m.believe(&[1, 2, 30]), Belief::Unknown(Ignorance::Conflict));
    }

    /// **가까운 경험이 먼 경험을 이긴다** — 일치 폭이 큰 쪽만 근거가 된다.
    #[test]
    fn closer_experience_overrides_distant_one() {
        let mut m = WorldModel::new();
        m.observe(vec![9, 9, 9], 1); // 멀다
        m.observe(vec![1, 2, 3], 5); // 정확히 일치
        assert_eq!(m.believe(&[1, 2, 3]).outcome(), Some(5));
    }

    /// **오차가 앎을 고친다**: 틀린 자리에서는 그 뒤로 침묵한다.
    /// 이것이 없으면 같은 외삽을 영원히 반복한다(GEN3의 실패 형태).
    #[test]
    fn being_wrong_teaches_it_to_abstain_there() {
        let mut m = WorldModel::new();
        m.observe(vec![1, 2, 10], 7);
        m.observe(vec![1, 2, 20], 7);
        // 새 자리에서 7이라고 예측했는데 실제는 8이었다
        let b = m.learn(vec![1, 2, 30], 8);
        assert_eq!(b.outcome(), Some(7), "예측은 했어야 한다");
        assert_eq!(m.correct, 0);
        // 같은 종류의 자리에서 다시는 그렇게 단정하지 않는다
        assert!(!m.believe(&[1, 2, 40]).is_known(), "덴 자리에서 또 단정했다");
    }

    /// 맞힌 경험은 앎을 무너뜨리지 않는다(과잉 위축 방지).
    #[test]
    fn being_right_does_not_erode_confidence() {
        let mut m = WorldModel::new();
        m.observe(vec![1, 2, 10], 7);
        m.observe(vec![1, 2, 20], 7);
        let b = m.learn(vec![1, 2, 30], 7);
        assert_eq!(b.outcome(), Some(7));
        assert_eq!(m.correct, 1);
        assert!(m.believe(&[1, 2, 40]).is_known(), "맞혔는데 위축됐다");
    }

    /// **자유 파라미터가 없다**는 것의 조작적 확인: 모델은 상수 문턱이 아니라
    /// 경험의 구조로만 판단하므로, 경험을 더 주면 침묵이 앎으로 바뀔 수 있다.
    #[test]
    fn more_experience_can_turn_ignorance_into_knowledge() {
        let mut m = WorldModel::new();
        assert!(!m.believe(&[4, 5, 6]).is_known());
        m.observe(vec![4, 5, 6], 3);
        assert!(m.believe(&[4, 5, 6]).is_known(), "경험을 줬는데도 모른다");
    }
}
