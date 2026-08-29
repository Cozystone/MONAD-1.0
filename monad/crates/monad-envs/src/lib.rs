//! MONAD 시험 환경.
//!
//! - [`maze`] — 별칭 미로: B3(클론 성장)의 DoD
//! - [`bounce`] — Bounce Test: M0의 관문(D1)

pub mod arc_data;
pub mod arc_dream;
pub mod arc_ebm;
/// M2-R: 동결 솔버를 **경험 공급자**로 배선하는 계층(새 풀이 어휘가 아니다).
pub mod arc_experience;
pub mod arc_objrule;
pub mod arc_patch;
pub mod arc_relrule;
pub mod arc_solve;
pub mod bounce;
pub mod grid;
pub mod maze;

pub use bounce::{Body, BounceWorld, Tick};
pub use maze::Maze;
