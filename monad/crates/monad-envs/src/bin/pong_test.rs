//! Pong급 온라인 학습 — M0 엑싯 게이트 2번.
//!
//! Catch-Pong: 공이 위에서 떨어지며 좌우 벽에 반사되고, 바닥의 패들로 받는다.
//! 사전학습 0, 보상 셰이핑 0, 데모 0 — 오직 상호작용 스트림만으로 배운다.
//!
//! **DoD: 누적 5,000 스텝 안에 최근 50 에피소드 잡기율 ≥ 80% 도달.**
//! (무작위 기준선 ≈ 패들 폭/너비 수준)
//!
//! 지각은 자기중심(egocentric) 상대 좌표 — "공이 나에게서 어디에 있는가".
//! 보상 배선은 선호 하나뿐: 잡은 순간의 지각을 선호한다(prefer_percept).
//! 그러면 EFE 계획이 그 지각으로 가는 행동을 고른다. 정책 학습이 따로 없다.
//!
//! 실행: `cargo run --release --bin pong-test` (+ `--random` 기준선)

use monad_core::encode::Obs;
use monad_core::rng::Rng;
use monad_core::wake::{Agent, Config};

const W: i32 = 8;
const H: i32 = 6;
const N_ACTIONS: u16 = 3; // 0=stay 1=left 2=right

struct Catch {
    ball_x: i32,
    ball_y: i32,
    dx: i32,
    pad_x: i32, // 패들 중심(폭 3: pad_x-1..=pad_x+1)
}

impl Catch {
    fn new(r: &mut Rng) -> Self {
        Catch {
            ball_x: r.below(W as u32) as i32,
            ball_y: 0,
            dx: if r.below(2) == 0 { -1 } else { 1 },
            pad_x: (W / 2).min(W - 2).max(1),
        }
    }
    fn reset_ball(&mut self, r: &mut Rng) {
        self.ball_x = r.below(W as u32) as i32;
        self.ball_y = 0;
        self.dx = if r.below(2) == 0 { -1 } else { 1 };
    }
    /// 행동 적용 + 공 낙하. 반환: (에피소드 종료, 잡았는가)
    fn step(&mut self, a: u16) -> (bool, bool) {
        match a {
            1 => self.pad_x = (self.pad_x - 1).max(1),
            2 => self.pad_x = (self.pad_x + 1).min(W - 2),
            _ => {}
        }
        self.ball_x += self.dx;
        if self.ball_x < 0 {
            self.ball_x = 1;
            self.dx = 1;
        } else if self.ball_x >= W {
            self.ball_x = W - 2;
            self.dx = -1;
        }
        self.ball_y += 1;
        if self.ball_y >= H {
            let caught = (self.ball_x - self.pad_x).abs() <= 1;
            (true, caught)
        } else {
            (false, false)
        }
    }
    /// 자기중심 관측: 공의 상대 위치 + 낙하 진행 + 공 진행 방향 (+ 종료 이벤트).
    fn obs(&self, event: u32) -> Obs {
        let rel = (self.ball_x - self.pad_x).clamp(-4, 4) + 4; // 0..=8
        Obs::new()
            .cat(0, rel as u32)
            .cat(1, self.ball_y as u32)
            .cat(2, ((self.dx + 1) / 2) as u32)
            .cat(3, event) // 0=진행 1=잡음 2=놓침
    }
}

fn run(random_only: bool, seed: u64, budget: usize) -> (Option<usize>, f32, usize, usize) {
    let mut r = Rng::new(seed);
    let mut env = Catch::new(&mut r);
    let mut agent = Agent::with_config(Config::default());
    for (i, n) in ["rel_x", "ball_y", "ball_dx", "event"].iter().enumerate() {
        agent.encoder.declare(i as u16, n);
    }

    let mut results: Vec<bool> = Vec::new();
    let mut steps = 0usize;
    let mut reached_at: Option<usize> = None;
    agent.perceive(&env.obs(0), 0);

    while steps < budget {
        // 정책: ε는 앞 1/3 구간에서 1.0→0.02로 줄이고, **유능선 도달 후에는 0**
        // (순수 활용 — 배운 뒤에도 계속 주사위를 던지면 정상상태가 그만큼 샌다).
        // explore()도 에이전트의 자체 기능(안 해본 행동 우선)이지 외부 지식이 아니다.
        let eps = if random_only {
            2.0
        } else if reached_at.is_some() {
            0.0
        } else {
            (1.0 - steps as f32 / (budget as f32 / 3.0)).max(0.02)
        };
        // 주기적 수면 — 각성이 근사한 지도를 꿈(EM)이 다듬는다.
        // (정상상태 침하 처방 2종 실측 기각 기록: ① switch_cost 0.2 — 무효(75.4≈75.8)
        //  ② consume 증분 응고 — 역회귀(도달 3/5): pong의 작은 세계에서는 전면
        //  재건의 전역 재추론이 더 깨끗하다. 원복. 잔여 원인 후보는 재건 직후
        //  재정착 과도기의 평균 혼입 — 측정 창 분리 실험으로 이월.)
        if !random_only && steps > 0 && steps % 1200 == 0 {
            monad_core::dream::dream(&mut agent, monad_core::dream::DreamConfig::default());
            agent.reset_episode();
            agent.perceive(&env.obs(0), 0);
        }
        let a = if (r.next_f64() as f32) < eps {
            agent.explore(N_ACTIONS, &mut r)
        } else {
            agent.plan(N_ACTIONS).unwrap_or_else(|| agent.explore(N_ACTIONS, &mut r))
        };
        let (done, caught) = env.step(a);
        steps += 1;
        if done {
            let ev = if caught { 1 } else { 2 };
            let s = agent.perceive(&env.obs(ev), a);
            // 보상 배선의 전부: 잡은 순간의 지각을 선호한다.
            if caught {
                agent.prefer_percept(s.percept, 8.0);
            }
            results.push(caught);
            if reached_at.is_none() && results.len() >= 50 {
                let last50 = &results[results.len() - 50..];
                let rate = last50.iter().filter(|&&c| c).count() as f32 / 50.0;
                if rate >= 0.80 && !random_only {
                    reached_at = Some(steps);
                }
            }
            env.reset_ball(&mut r);
            agent.reset_episode();
            agent.perceive(&env.obs(0), 0);
        } else {
            agent.perceive(&env.obs(0), a);
        }
    }

    let last100 = if results.len() >= 100 {
        &results[results.len() - 100..]
    } else {
        &results[..]
    };
    let final_rate = last100.iter().filter(|&&c| c).count() as f32 / last100.len().max(1) as f32;
    (reached_at, final_rate, agent.graph.n_nodes(), results.len())
}

/// 동일 하네스 기준선: 표 형태 Q-러닝 (같은 관측, 같은 행동, 같은 예산).
///
/// 문헌 수치 대조가 아니라 **같은 벤치마크에서의 직접 비교**를 위해 둔다.
/// (state, action) 표 + ε-탐욕, 표준적인 설정(α=0.2, γ=0.95).
fn run_qlearn(seed: u64, budget: usize) -> (Option<usize>, f32, usize) {
    let mut r = Rng::new(seed);
    let mut env = Catch::new(&mut r);
    let mut q: std::collections::HashMap<(i32, i32, i32, i32, u16), f32> =
        std::collections::HashMap::new();
    let key = |e: &Catch, a: u16| (e.ball_x - e.pad_x, e.ball_y, e.dx, e.pad_x, a);
    let mut results: Vec<bool> = Vec::new();
    let mut steps = 0usize;
    let mut reached_at: Option<usize> = None;
    let (alpha, gamma) = (0.2f32, 0.95f32);

    while steps < budget {
        let eps = (1.0 - steps as f32 / (budget as f32 / 3.0)).max(0.02);
        let s0 = (env.ball_x - env.pad_x, env.ball_y, env.dx, env.pad_x);
        let a = if (r.next_f64() as f32) < eps {
            r.below(N_ACTIONS as u32) as u16
        } else {
            (0..N_ACTIONS)
                .max_by(|&x, &y| {
                    let qx = q.get(&(s0.0, s0.1, s0.2, s0.3, x)).copied().unwrap_or(0.0);
                    let qy = q.get(&(s0.0, s0.1, s0.2, s0.3, y)).copied().unwrap_or(0.0);
                    qx.partial_cmp(&qy).unwrap()
                })
                .unwrap()
        };
        let (done, caught) = env.step(a);
        steps += 1;
        let reward = if done {
            if caught {
                1.0
            } else {
                -1.0
            }
        } else {
            0.0
        };
        let max_next = if done {
            0.0
        } else {
            (0..N_ACTIONS)
                .map(|x| q.get(&key(&env, x)).copied().unwrap_or(0.0))
                .fold(f32::MIN, f32::max)
        };
        let e = q.entry((s0.0, s0.1, s0.2, s0.3, a)).or_insert(0.0);
        *e += alpha * (reward + gamma * max_next - *e);
        if done {
            results.push(caught);
            if reached_at.is_none() && results.len() >= 50 {
                let rate = results[results.len() - 50..].iter().filter(|&&c| c).count() as f32
                    / 50.0;
                if rate >= 0.80 {
                    reached_at = Some(steps);
                }
            }
            env.reset_ball(&mut r);
        }
    }
    let last100 = if results.len() >= 100 {
        &results[results.len() - 100..]
    } else {
        &results[..]
    };
    let final_rate = last100.iter().filter(|&&c| c).count() as f32 / last100.len().max(1) as f32;
    (reached_at, final_rate, q.len())
}

fn main() {
    let random_only = std::env::args().any(|a| a == "--random");
    if std::env::args().any(|a| a == "--qlearn") {
        println!("== 동일 하네스 기준선: 표 형태 Q-러닝 ==");
        println!("{:>6} {:>12} {:>14} {:>10}", "시드", "도달 스텝", "최종 잡기율", "Q표 크기");
        println!("{}", "-".repeat(48));
        for seed in [1u64, 2, 3, 4, 5] {
            let (at, rate, tbl) = run_qlearn(seed, 5000);
            println!(
                "{:>6} {:>12} {:>13.1}% {:>10}",
                seed,
                at.map(|s| s.to_string()).unwrap_or_else(|| "미도달".into()),
                rate * 100.0,
                tbl
            );
        }
        return;
    }
    println!("=========================================================================");
    println!(
        "Catch-Pong 온라인 학습 — M0 게이트 2 {}",
        if random_only { "(무작위 기준선)" } else { "" }
    );
    println!("=========================================================================");
    println!("격자 {W}x{H}, 패들 폭 3, 행동 3종. 사전학습·보상셰이핑·데모 없음.");
    println!("DoD: 5,000 스텝 내 최근 50 에피소드 잡기율 ≥ 80%\n");

    println!("{:>6} {:>12} {:>14} {:>10} {:>10}", "시드", "도달 스텝", "최종 잡기율", "상태 수", "에피소드");
    println!("{}", "-".repeat(58));
    let mut reached = 0;
    let mut worst: usize = 0;
    for seed in [1u64, 2, 3, 4, 5] {
        let (at, rate, nodes, eps) = run(random_only, seed, 5000);
        if let Some(s) = at {
            reached += 1;
            worst = worst.max(s);
        }
        println!(
            "{:>6} {:>12} {:>13.1}% {:>10} {:>10}",
            seed,
            at.map(|s| s.to_string()).unwrap_or_else(|| "미도달".into()),
            rate * 100.0,
            nodes,
            eps
        );
    }
    println!("{}", "-".repeat(58));
    if random_only {
        println!("무작위 기준선 — 위 잡기율이 우연 수준이다.");
    } else {
        let pass = reached == 5;
        println!(
            "▶ M0 게이트 2: {} (5시드 중 {}개 도달{})",
            if pass { "✅ 통과" } else { "❌ 미통과" },
            reached,
            if reached > 0 { format!(", 최악 {worst} 스텝") } else { String::new() }
        );
        std::process::exit(if pass { 0 } else { 1 });
    }
}
