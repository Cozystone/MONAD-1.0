//! 별칭 미로 (Aliasing Maze) — B3의 DoD 환경.
//!
//! 격자 방의 각 칸은 작은 기호 집합에서 하나를 낸다. 기호가 칸 수보다 훨씬 적으므로
//! **많은 칸이 똑같아 보인다**. 관측만으로는 어디 있는지 알 수 없고, 오직 문맥
//! (어디서 어떻게 왔는가)으로만 위치가 결정된다.
//!
//! 이것이 세계 모델의 최소 시험대다. 이 방에서 지도를 만들 수 없으면 어떤 환경에서도
//! 만들 수 없다. CSCG(Nature Communications 2021)가 사용한 설정과 같은 계열이다.

use monad_core::rng::Rng;

pub const N_ACTIONS: u16 = 4;
const DX: [i32; 4] = [0, 0, -1, 1];
const DY: [i32; 4] = [-1, 1, 0, 0];

pub struct Maze {
    pub w: i32,
    pub h: i32,
    /// 칸별 관측 기호.
    pub obs: Vec<u32>,
    pub n_symbols: u32,
    pub x: i32,
    pub y: i32,
}

impl Maze {
    /// 관측 기호가 `n_symbols`종뿐인 열린 방. 기호 수가 적을수록 별칭이 심하다.
    pub fn new(w: i32, h: i32, n_symbols: u32, seed: u64) -> Self {
        let mut r = Rng::new(seed);
        let obs = (0..(w * h)).map(|_| r.below(n_symbols)).collect();
        Maze { w, h, obs, n_symbols, x: 0, y: 0 }
    }

    #[inline]
    pub fn cell(&self) -> usize {
        (self.y * self.w + self.x) as usize
    }

    #[inline]
    pub fn n_cells(&self) -> usize {
        (self.w * self.h) as usize
    }

    #[inline]
    pub fn observe(&self) -> u32 {
        self.obs[self.cell()]
    }

    pub fn set_cell(&mut self, c: usize) {
        self.x = (c as i32) % self.w;
        self.y = (c as i32) / self.w;
    }

    /// 행동을 실행하고 새 관측을 돌려준다. 벽에 막히면 제자리.
    pub fn step(&mut self, a: u16) -> u32 {
        let i = (a as usize) % 4;
        let nx = self.x + DX[i];
        let ny = self.y + DY[i];
        if nx >= 0 && nx < self.w && ny >= 0 && ny < self.h {
            self.x = nx;
            self.y = ny;
        }
        self.observe()
    }

    /// 두 칸 사이 최단 거리(맨해튼 — 열린 방이므로 정확).
    pub fn shortest(&self, a: usize, b: usize) -> i32 {
        let (ax, ay) = ((a as i32) % self.w, (a as i32) / self.w);
        let (bx, by) = ((b as i32) % self.w, (b as i32) / self.w);
        (ax - bx).abs() + (ay - by).abs()
    }

    /// 별칭 정도: 평균적으로 한 기호가 몇 칸을 가리키는가.
    pub fn aliasing(&self) -> f32 {
        let mut count = vec![0u32; self.n_symbols as usize];
        for &o in &self.obs {
            count[o as usize] += 1;
        }
        let used = count.iter().filter(|&&c| c > 0).count().max(1);
        self.n_cells() as f32 / used as f32
    }
}
