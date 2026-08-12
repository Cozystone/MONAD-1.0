//! Bounce Test 환경 — M0의 관문 (개발계획 D1).
//!
//! 2D 상자 안에서 물체가 움직이고 벽에 부딪힌다. 시험하는 것은 단 하나:
//! **"경험을 저장하는 시스템이 경험을 이해하는 시스템이 되는가."**
//!
//! 반사 규칙의 진짜 인과는 {접촉, 벽면}뿐이다. 모양·색·크기·속도·벽 무늬는
//! 전부 무관하다. 관찰(Phase 1)에서 그 구조를 스스로 추려내고, 처음 보는
//! 물체(Phase 2)·바뀐 외관(Phase 3)·바뀐 규칙(Phase 4, M1)에 전이해야 한다.

use monad_core::rng::Rng;

pub const SHAPE_NAMES: [&str; 4] = ["circle", "square", "triangle", "star"];
pub const COLOR_NAMES: [&str; 6] = ["red", "blue", "green", "yellow", "white", "purple"];
pub const TEX_NAMES: [&str; 4] = ["brick", "steel", "wood", "glassy"];
pub const SIDE_NAMES: [&str; 5] = ["none", "left", "right", "floor", "ceiling"];
pub const EFFECT_NAMES: [&str; 3] = ["none", "reverse_vx", "reverse_vy"];

/// 물체 외양 — 인과와 무관해야 하는 것들.
#[derive(Clone, Copy, Debug)]
pub struct Body {
    pub shape: u32,
    pub color: u32,
    pub size: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Tick {
    /// 접촉했는가(이번 틱에 벽 충돌).
    pub contact: bool,
    /// 충돌한 벽면(0=none, 1=left, 2=right, 3=floor, 4=ceiling).
    pub side: u32,
    /// 충돌 직전 속도.
    pub v_before: (f32, f32),
    /// 이번 틱 이후 속도.
    pub v_after: (f32, f32),
    pub pos: (f32, f32),
    /// 결과 분류(정답 라벨이 아니라 물리 관측 — vx 부호가 뒤집혔는가 등).
    pub effect: u32,
}

pub struct BounceWorld {
    pub body: Body,
    pub tex: u32,
    pub gravity: f32,
    pub pos: (f32, f32),
    pub vel: (f32, f32),
    dt: f32,
}

impl BounceWorld {
    pub fn new(body: Body, tex: u32, rng: &mut Rng) -> Self {
        let ang = rng.next_f64() as f32 * std::f32::consts::TAU;
        let speed = 0.6 + rng.next_f64() as f32 * 1.4;
        BounceWorld {
            body,
            tex,
            gravity: 0.0,
            pos: (
                0.25 + rng.next_f64() as f32 * 0.5,
                0.25 + rng.next_f64() as f32 * 0.5,
            ),
            vel: (ang.cos() * speed, ang.sin() * speed),
            dt: 0.02,
        }
    }

    /// 한 틱 진행. 물리는 단순하다: 강체 벽은 수직 성분을 뒤집는다.
    pub fn step(&mut self) -> Tick {
        let v_before = self.vel;
        self.vel.1 -= self.gravity * self.dt;
        let mut p = (self.pos.0 + self.vel.0 * self.dt, self.pos.1 + self.vel.1 * self.dt);
        let mut contact = false;
        let mut side = 0u32;
        let mut effect = 0u32;
        let r = self.body.size * 0.02;

        if p.0 - r < 0.0 {
            p.0 = r + (r - p.0);
            self.vel.0 = -self.vel.0;
            contact = true;
            side = 1;
            effect = 1;
        } else if p.0 + r > 1.0 {
            p.0 = (1.0 - r) - (p.0 + r - 1.0);
            self.vel.0 = -self.vel.0;
            contact = true;
            side = 2;
            effect = 1;
        }
        if p.1 - r < 0.0 {
            p.1 = r + (r - p.1);
            self.vel.1 = -self.vel.1;
            contact = true;
            side = 3;
            effect = 2;
        } else if p.1 + r > 1.0 {
            p.1 = (1.0 - r) - (p.1 + r - 1.0);
            self.vel.1 = -self.vel.1;
            contact = true;
            side = 4;
            effect = 2;
        }
        self.pos = p;
        Tick {
            contact,
            side,
            v_before,
            v_after: self.vel,
            pos: p,
            effect,
        }
    }

    /// 다음 틱에 어느 벽과 접촉할지(접촉 직전 예측 질의용).
    pub fn peek_contact(&self) -> (bool, u32) {
        let r = self.body.size * 0.02;
        let vy = self.vel.1 - self.gravity * self.dt;
        let p = (self.pos.0 + self.vel.0 * self.dt, self.pos.1 + vy * self.dt);
        if p.0 - r < 0.0 {
            (true, 1)
        } else if p.0 + r > 1.0 {
            (true, 2)
        } else if p.1 - r < 0.0 {
            (true, 3)
        } else if p.1 + r > 1.0 {
            (true, 4)
        } else {
            (false, 0)
        }
    }
}
