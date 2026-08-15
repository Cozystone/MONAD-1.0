//! # MONAD core — 단일 기질 인지 런타임
//!
//! MONAD는 문장을 생성하지 않는다. 세계를 배운다.
//!
//! 이 크레이트는 PRD v0.2의 "Five Ones"를 코드로 구현한다:
//!
//! | # | 단 하나의 | 모듈 |
//! |---|---|---|
//! | 1 | 자료형 — 인지 원자 | [`atom`] (+ [`sbv`]) |
//! | 2 | 자료구조 — 클론 세계 그래프 | `graph` (A3) |
//! | 3 | 목적함수 — 자유에너지 F / G | `wake` (B2·B4) |
//! | 4 | 학습 규칙 — 1-shot 쓰기 + 수면 압축 | `wake` (B3) · `sleep` (C1·C2) |
//! | 5 | 루프 — wake 틱 + sleep 패스 | `runtime` |
//!
//! ## 설계 불변식
//!
//! - **코어 루프에 경사하강·백프롭·리플레이 버퍼가 없다.** 학습은 카운트 갱신이다.
//! - **외부 의존성이 없다.** 저사양·오프라인 상시 구동이 제품 전제다.
//! - **모든 내부 상태는 사람이 읽을 수 있다.**(유리상자 의무)

#![deny(rust_2018_idioms)]

pub mod abstraction;
pub mod atom;
pub mod dream;
pub mod encode;
pub mod graph;
pub mod rng;
pub mod sbv;
pub mod schema;
pub mod sleep;
pub mod store;
pub mod wake;

pub use abstraction::{generalize, lgg, Abstraction, Library, Provenance, Term};
pub use atom::{Atom, Val};
pub use dream::{dream, DreamConfig, DreamReport};
pub use encode::{Encoder, Feature, Obs, Vocab};
pub use graph::{Node, Succ, WorldGraph, PERCEPT_TOL};
pub use rng::Rng;
pub use sbv::{bundle, Bundler, Sbv, DIM, NBLOCKS};
pub use schema::{induce, Constraint, Event, InduceConfig, Schema, SchemaLib};
pub use sleep::{SleepConfig, SleepReport};
pub use store::{Hit, Store};
pub use wake::{Agent, Config, Settled, Stats};

/// 빌드 시 감지된 SIMD 경로 이름(리포트용).
pub fn simd_backend() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            return "avx2";
        }
        return "scalar(x86_64)";
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        "scalar"
    }
}
