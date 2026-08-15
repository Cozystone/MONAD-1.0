//! 제약 계층 — 자유로운 생성적 인지에는 타입·도메인·자원의 울타리가 필요하다.
//!
//! # 왜 이 모듈이 생겼는가 (두 사고의 승격)
//!
//! 재구체화가 스키마의 구멍을 자유롭게 채우기 시작하자 두 가지가 터졌다:
//!
//! 1. **잘못된 타입 대입** — 색 슬롯에 격자 크기 14가 들어가 격자에 존재하지
//!    않는 색이 쓰였고, 이후 연산이 10칸 배열을 인덱싱하다 무너졌다.
//! 2. **조합 폭발** — 확대 연산이 중첩돼 656GB 할당을 시도했다.
//!
//! 이것은 버그가 아니라 **구조적 요구사항의 증거**다. 스스로 구조를 만들어
//! 쓰는 시스템은 "무엇이 어디에 들어갈 수 있는가"와 "이 조합이 얼마나 비싼가"를
//! **생성 전에** 알아야 한다. 그 선언을 담는 것이 이 모듈이다.
//!
//! # 교리
//!
//! - **도메인 중립**: 여기에 ARC도, 색도, 격자도 없다. 함자·인자·크기 배율만 있다.
//!   각 도메인이 자기 연산의 사양을 등록하고, 검증은 코어가 한다.
//! - **생성 전 검사**: 적용해 보고 터지는 것이 아니라, 항을 만들 때 막는다.
//! - **비용은 예측한다**: 실행해서 재는 것이 아니라 구조에서 추정한다
//!   (폭발은 실행 한 번으로 프로세스를 죽이므로 사후 측정이 불가능하다).

use crate::abstraction::Term;
use std::collections::HashMap;

/// 값이 놓일 수 있는 자리의 종류.
#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    /// 닫힌 정수 구간 [lo, hi] — "색은 0..=9", "방향은 0..=3".
    Range(i64, i64),
    /// 열거된 허용 값.
    Enum(Vec<u64>),
    /// 제약 없음(구조 인자 등).
    Any,
}

impl Domain {
    pub fn admits(&self, v: u64) -> bool {
        match self {
            Domain::Range(lo, hi) => (v as i64) >= *lo && (v as i64) <= *hi,
            Domain::Enum(vs) => vs.contains(&v),
            Domain::Any => true,
        }
    }

    /// 이 도메인이 허용하는 값들(재구체화 후보 생성기 — 무한이면 None).
    pub fn values(&self) -> Option<Vec<u64>> {
        match self {
            Domain::Range(lo, hi) if *hi >= *lo && (*hi - *lo) < 4096 => {
                Some((*lo..=*hi).map(|v| v as u64).collect())
            }
            Domain::Enum(vs) => Some(vs.clone()),
            _ => None,
        }
    }
}

/// 연산이 결과 크기를 어떻게 바꾸는가 — 자원 폭발의 예측 모형.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Growth {
    /// 크기 유지 또는 축소.
    Preserve,
    /// 인자 i의 값이 양변 배율(정사각 확대).
    Scale(usize),
    /// 인자 i, j가 각각 가로·세로 배율.
    ScaleXY(usize, usize),
    /// 입력 크기 자체가 배율(자기합성 — w·h배).
    Square,
}

/// 자원 비용 추정치(상대 단위).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cost {
    /// 연산 수 규모.
    pub compute: u64,
    /// 중간 표현이 차지할 최대 크기(셀 수 등).
    pub memory: u64,
}

/// 예산 — 이 한도를 넘을 것으로 **예측되면** 애초에 만들지 않는다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Budget {
    pub max_compute: u64,
    pub max_memory: u64,
}

impl Default for Budget {
    fn default() -> Self {
        // 기본값은 넉넉하되 폭발은 잡는 규모(중간 표현 6만 단위).
        Budget { max_compute: 100_000_000, max_memory: 60_000 }
    }
}

/// 한 함자(연산)의 사양.
#[derive(Clone, Debug)]
pub struct OpSpec {
    pub functor: u32,
    /// 사람이 읽는 이름(유리상자 의무).
    pub name: String,
    /// 인자별 도메인(길이 = 항수).
    pub args: Vec<Domain>,
    pub growth: Growth,
    /// 입력 단위당 기본 연산 비용.
    pub unit_cost: u64,
}

/// 위반 사유 — 무엇이 왜 막혔는지 사람이 읽을 수 있어야 한다.
#[derive(Clone, Debug, PartialEq)]
pub enum Violation {
    UnknownFunctor(u32),
    Arity { functor: u32, expected: usize, got: usize },
    OutOfDomain { functor: u32, arg: usize, value: u64 },
    /// 변수가 남아 있어 아직 실행 가능한 항이 아니다.
    Unbound(u32),
    Budget { estimated: Cost, budget: Budget },
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Violation::UnknownFunctor(x) => write!(f, "미등록 함자 f{x}"),
            Violation::Arity { functor, expected, got } => {
                write!(f, "f{functor} 항수 불일치: {expected} 필요, {got} 받음")
            }
            Violation::OutOfDomain { functor, arg, value } => {
                write!(f, "f{functor} 인자 {arg}에 도메인 밖 값 {value}")
            }
            Violation::Unbound(v) => write!(f, "미결정 변수 ?{v}"),
            Violation::Budget { estimated, .. } => write!(
                f,
                "예산 초과(추정 메모리 {} · 연산 {})",
                estimated.memory, estimated.compute
            ),
        }
    }
}

/// 등록된 연산 사양들 + 예산. **재구체화·합성 전에 이 관문을 통과해야 한다.**
#[derive(Clone, Debug, Default)]
pub struct Constraints {
    ops: HashMap<u32, OpSpec>,
    pub budget: Budget,
}

impl Constraints {
    pub fn new(budget: Budget) -> Self {
        Constraints { ops: HashMap::new(), budget }
    }

    pub fn register(&mut self, spec: OpSpec) {
        self.ops.insert(spec.functor, spec);
    }

    pub fn spec(&self, functor: u32) -> Option<&OpSpec> {
        self.ops.get(&functor)
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// 한 변수 자리의 도메인 — 재구체화 후보를 **도메인에서** 뽑게 한다
    /// (아무 값이나 넣고 터지길 기다리지 않는다).
    pub fn arg_domain(&self, functor: u32, arg: usize) -> Option<&Domain> {
        self.ops.get(&functor).and_then(|s| s.args.get(arg))
    }

    /// 항의 타입·도메인 검증. 변수가 남아 있으면 그 자리는 검사만 유보한다
    /// (`require_ground`가 참이면 미결정 변수도 위반).
    pub fn check(&self, t: &Term, require_ground: bool) -> Result<(), Violation> {
        match t {
            Term::Var(v) => {
                if require_ground {
                    Err(Violation::Unbound(*v))
                } else {
                    Ok(())
                }
            }
            Term::Const(_) => Ok(()),
            Term::App(f, args) => {
                let spec = self.ops.get(f).ok_or(Violation::UnknownFunctor(*f))?;
                if spec.args.len() != args.len() {
                    return Err(Violation::Arity {
                        functor: *f,
                        expected: spec.args.len(),
                        got: args.len(),
                    });
                }
                for (i, a) in args.iter().enumerate() {
                    match a {
                        Term::Const(v) if !spec.args[i].admits(*v) => {
                            return Err(Violation::OutOfDomain {
                                functor: *f,
                                arg: i,
                                value: *v,
                            })
                        }
                        _ => self.check(a, require_ground)?,
                    }
                }
                Ok(())
            }
        }
    }

    /// 실행 전 비용 추정. `input`은 입력 표현의 크기(셀 수 등).
    ///
    /// 순차 적용을 가정한다: 항의 인자들을 먼저 처리하고(내포 = 앞 단계),
    /// 그 결과 크기에 이 연산의 배율을 적용한다.
    pub fn estimate(&self, t: &Term, input: u64) -> Cost {
        let mut cost = Cost { compute: 0, memory: input };
        self.estimate_into(t, &mut cost);
        cost
    }

    fn estimate_into(&self, t: &Term, acc: &mut Cost) {
        let Term::App(f, args) = t else { return };
        // 내포된 항(앞 단계)을 먼저
        for a in args {
            self.estimate_into(a, acc);
        }
        let Some(spec) = self.ops.get(f) else { return };
        let cur = acc.memory;
        let arg_val = |i: usize| -> u64 {
            match args.get(i) {
                Some(Term::Const(v)) => *v,
                _ => 1,
            }
        };
        let next = match spec.growth {
            Growth::Preserve => cur,
            Growth::Scale(i) => {
                let k = arg_val(i).max(1);
                cur.saturating_mul(k).saturating_mul(k)
            }
            Growth::ScaleXY(i, j) => cur
                .saturating_mul(arg_val(i).max(1))
                .saturating_mul(arg_val(j).max(1)),
            Growth::Square => cur.saturating_mul(cur),
        };
        acc.memory = next;
        acc.compute = acc
            .compute
            .saturating_add(next.saturating_mul(spec.unit_cost));
    }

    /// 도메인 검증 + 예산 검증을 한 번에. **생성 관문**.
    pub fn admit(&self, t: &Term, input: u64) -> Result<Cost, Violation> {
        self.check(t, true)?;
        let c = self.estimate(t, input);
        if c.memory > self.budget.max_memory || c.compute > self.budget.max_compute {
            return Err(Violation::Budget { estimated: c, budget: self.budget });
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abstraction::Term;

    // 시험용 도메인: f1(색 0..=9), f2(배율), f3(자기합성), f4(항등)
    fn cs() -> Constraints {
        let mut c = Constraints::new(Budget { max_compute: 10_000_000, max_memory: 60_000 });
        c.register(OpSpec {
            functor: 1,
            name: "paint".into(),
            args: vec![Domain::Range(0, 9)],
            growth: Growth::Preserve,
            unit_cost: 1,
        });
        c.register(OpSpec {
            functor: 2,
            name: "scale".into(),
            args: vec![Domain::Range(1, 8), Domain::Any],
            growth: Growth::Scale(0),
            unit_cost: 1,
        });
        c.register(OpSpec {
            functor: 3,
            name: "self_compose".into(),
            args: vec![Domain::Any],
            growth: Growth::Square,
            unit_cost: 1,
        });
        c.register(OpSpec {
            functor: 4,
            name: "id".into(),
            args: vec![],
            growth: Growth::Preserve,
            unit_cost: 1,
        });
        c
    }

    /// 사고 1의 재현과 차단: 색 자리에 크기 값이 들어오면 **생성 단계에서** 막힌다.
    #[test]
    fn wrong_type_binding_is_refused_before_execution() {
        let c = cs();
        let bad = Term::App(1, vec![Term::Const(14)]);
        assert_eq!(
            c.check(&bad, true),
            Err(Violation::OutOfDomain { functor: 1, arg: 0, value: 14 })
        );
        let good = Term::App(1, vec![Term::Const(7)]);
        assert!(c.check(&good, true).is_ok());
    }

    /// 사고 2의 재현과 차단: 확대 중첩의 크기를 **실행 전에 예측**해 거부한다.
    #[test]
    fn resource_explosion_is_predicted_not_experienced() {
        let c = cs();
        // scale(8) 세 번 중첩: 900 → 57,600 → 3.7M → …
        let nested = Term::App(
            2,
            vec![
                Term::Const(8),
                Term::App(2, vec![Term::Const(8), Term::App(4, vec![])]),
            ],
        );
        let err = c.admit(&nested, 900).unwrap_err();
        assert!(matches!(err, Violation::Budget { .. }), "폭발을 못 막았다: {err:?}");

        // 한 번만이면 통과(57,600 < 60,000)
        let ok = Term::App(2, vec![Term::Const(8), Term::App(4, vec![])]);
        assert!(c.admit(&ok, 900).is_ok());
    }

    /// 자기합성(제곱 성장)도 예측된다.
    #[test]
    fn square_growth_is_estimated() {
        let c = cs();
        let t = Term::App(3, vec![Term::App(4, vec![])]);
        assert!(c.admit(&t, 900).is_err(), "900²=810,000은 예산 밖이어야 한다");
        assert!(c.admit(&t, 200).is_ok(), "200²=40,000은 예산 안");
    }

    /// 항수·미등록 함자·미결정 변수도 잡는다.
    #[test]
    fn arity_unknown_and_unbound_are_caught() {
        let c = cs();
        assert_eq!(
            c.check(&Term::App(1, vec![]), true),
            Err(Violation::Arity { functor: 1, expected: 1, got: 0 })
        );
        assert_eq!(
            c.check(&Term::App(99, vec![]), true),
            Err(Violation::UnknownFunctor(99))
        );
        assert_eq!(
            c.check(&Term::App(1, vec![Term::Var(0)]), true),
            Err(Violation::Unbound(0))
        );
        // 미완성 스키마 검사(require_ground=false)에서는 변수를 허용한다
        assert!(c.check(&Term::App(1, vec![Term::Var(0)]), false).is_ok());
    }

    /// 도메인이 **후보 생성기**가 된다 — 아무 값이나 시도하지 않는다.
    #[test]
    fn domain_generates_candidates_for_reinstantiation() {
        let c = cs();
        let d = c.arg_domain(1, 0).unwrap();
        assert_eq!(d.values().unwrap().len(), 10, "색 자리는 10개 값만");
        assert!(!d.admits(14));
        let scale = c.arg_domain(2, 0).unwrap();
        assert_eq!(scale.values().unwrap(), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
