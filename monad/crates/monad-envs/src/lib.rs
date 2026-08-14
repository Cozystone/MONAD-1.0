//! MONAD 시험 환경.
//!
//! - [`maze`] — 별칭 미로: B3(클론 성장)의 DoD
//! - [`bounce`] — Bounce Test: M0의 관문(D1)

pub mod arc_data;
pub mod arc_solve;
pub mod bounce;
pub mod arc_dream;
pub mod grid;
pub mod maze;

pub use bounce::{Body, BounceWorld, Tick};
pub use maze::Maze;
