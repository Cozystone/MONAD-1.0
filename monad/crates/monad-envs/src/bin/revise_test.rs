//! M1 첫 시험 2종.
//!
//! [1] C4 규칙 개정 — Bounce Phase 4의 진짜 버전.
//!     세계의 규칙 자체가 바뀐다: 바닥이 접착제로 변해 reverse_vy → stick.
//!     DoD: 규칙 교체 후 **바닥 충돌 20회 이내**에 바닥 예측 정확도 ≥90% 회복,
//!     그리고 옛 스키마가 반례로 신뢰도를 잃는 과정이 덤프에 보일 것.
//!
//! [2] 연속 학습 유지율 — M1 본게이트의 축소판.
//!     미로 A를 배우고 → 미로 B(같은 기호 어휘!)를 배우고 → A로 복귀.
//!     DoD: 재학습 없이 A 위치식별이 원성능의 90% 이상.
//!
//! 실행: `cargo run --release --bin revise-test`

use monad_core::encode::Obs;
use monad_core::rng::Rng;
use monad_core::schema::{induce, Event, InduceConfig};
use monad_core::sleep::SleepConfig;
use monad_core::wake::{Agent, Config};
use monad_envs::bounce::{Body, BounceWorld};
use monad_envs::maze::{Maze, N_ACTIONS};
use std::collections::HashMap;

const S_CONTACT: u16 = 0;
const S_SIDE: u16 = 1;
const S_SHAPE: u16 = 2;
const S_COLOR: u16 = 3;
const EFFECTS: [&str; 4] = ["none", "reverse_vx", "reverse_vy", "stick"];

fn slot_name(s: u16) -> String {
    ["contact", "wall_side", "shape", "color"][s as usize].to_string()
}
fn effect_name(e: u32) -> String {
    EFFECTS.get(e as usize).unwrap_or(&"?").to_string()
}

/// 접착 바닥 모드가 있는 바운스 틱.
fn step_sticky(w: &mut BounceWorld, sticky: bool) -> (bool, u32, u32) {
    let t = w.step();
    if sticky && t.side == 3 {
        // 바닥 규칙 교체: 반사 대신 위로 약하게 접착 반동(사실상 정지 후 재시동)
        w.vel = (w.vel.0 * 0.2, 0.35); // 물리는 단순화 — 분류가 요점
        return (t.contact, t.side, 3); // effect = stick
    }
    (t.contact, t.side, t.effect)
}

fn ev(w: &BounceWorld, contact: bool, side: u32, effect: u32) -> Event {
    Event {
        cats: vec![
            (S_CONTACT, contact as u32),
            (S_SIDE, side),
            (S_SHAPE, w.body.shape),
            (S_COLOR, w.body.color),
        ],
        nums: vec![],
        effect,
    }
}

/// [1] 규칙 개정 시험.
fn revision_test() -> bool {
    println!("=========================================================================");
    println!("[1] C4 규칙 개정 — 바닥이 접착제로 변한다 (reverse_vy → stick)");
    println!("=========================================================================");
    let mut rng = Rng::new(42);
    let mut w = BounceWorld::new(Body { shape: 0, color: 0, size: 1.0 }, 0, &mut rng);
    let mut events: Vec<Event> = Vec::new();

    // 1기: 정상 물리로 학습
    let mut floor_hits = 0;
    while floor_hits < 30 {
        let (c, s, e) = step_sticky(&mut w, false);
        events.push(ev(&w, c, s, e));
        if s == 3 {
            floor_hits += 1;
        }
    }
    let lib1 = induce(&events, InduceConfig::default());
    println!("\n규칙 교체 전 스키마:");
    for (i, s) in lib1.schemas.iter().enumerate() {
        println!("  #{i}: {}", s.describe(&slot_name, &effect_name));
    }

    // 2기: 규칙 교체. 최근 사건 창(작업 기억)으로 주기 재유도 — 개정의 메커니즘.
    // 옛 지식을 다 버리지 않되(창에 남은 non-floor 규칙 유지), 새 반례가 창을
    // 채우면 스키마가 갈린다.
    println!("\n규칙 교체! 이후 바닥 충돌마다 예측→관측→기록:");
    let window = 400usize;
    let mut post_hits = 0usize;
    let mut recovered_at: Option<usize> = None;
    let mut recent_ok: Vec<bool> = Vec::new();
    let mut lib = lib1;
    while post_hits < 40 {
        let (will, side) = w.peek_contact();
        if will && side == 3 {
            // 바닥 충돌 직전 예측
            let q = Event {
                cats: vec![(S_CONTACT, 1), (S_SIDE, 3), (S_SHAPE, 0), (S_COLOR, 0)],
                nums: vec![],
                effect: u32::MAX,
            };
            let pred = lib.predict(&q);
            let (c, s, e) = step_sticky(&mut w, true);
            events.push(ev(&w, c, s, e));
            post_hits += 1;
            let ok = pred == Some(e);
            recent_ok.push(ok);
            // 수면(재유도): 최근 창으로 스키마를 다시 세운다
            let start = events.len().saturating_sub(window);
            lib = induce(&events[start..], InduceConfig::default());
            let tail: Vec<&bool> = recent_ok.iter().rev().take(5).collect();
            let tail_rate = tail.iter().filter(|&&&b| b).count() as f32 / tail.len().max(1) as f32;
            if recovered_at.is_none() && recent_ok.len() >= 5 && tail_rate >= 0.9 {
                recovered_at = Some(post_hits);
            }
            if post_hits <= 8 || post_hits % 10 == 0 {
                println!(
                    "  바닥충돌 {post_hits:>2}: 예측={:<10} 실제={:<6} {}",
                    pred.map(effect_name).unwrap_or("?".into()),
                    effect_name(e),
                    if ok { "○" } else { "×" }
                );
            }
        } else {
            let (c, s, e) = step_sticky(&mut w, true);
            events.push(ev(&w, c, s, e));
        }
    }

    println!("\n개정 후 스키마:");
    for (i, s) in lib.schemas.iter().enumerate() {
        println!("  #{i}: {}", s.describe(&slot_name, &effect_name));
    }
    // stick이 명시적 스키마로든 기본 결과(잔여 집합의 최대 압축)로든 표현되면 인정 —
    // "접촉 없음→none, 그 외→stick"은 이 퇴화 세계의 올바른 최소 서술이다.
    let has_stick =
        lib.schemas.iter().any(|s| s.effect == 3) || lib.default_effect == Some(3);
    let ok = recovered_at.map(|n| n <= 20).unwrap_or(false) && has_stick;
    println!(
        "\n판정: 회복 시점 = 바닥 충돌 {}회 (DoD ≤20) · stick 스키마 생성 {} → {}",
        recovered_at.map(|n| n.to_string()).unwrap_or("미회복".into()),
        if has_stick { "✅" } else { "❌" },
        if ok { "✅ 통과" } else { "❌ 실패" }
    );
    ok
}

/// [2] 연속 학습 유지율.
fn retention_test() -> bool {
    println!("\n=========================================================================");
    println!("[2] 연속 학습 — 미로 A 학습 → 미로 B 학습 → A 복귀 (재학습 없음)");
    println!("=========================================================================");
    const ROLE: u16 = 0;
    let cfg = Config::default();
    let mut agent = Agent::with_config(cfg);
    agent.encoder.declare(ROLE, "cell");
    let mut r = Rng::new(7);

    let mut learn = |agent: &mut Agent, maze: &mut Maze, steps: usize, r: &mut Rng| {
        agent.reset_episode();
        agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
        for _ in 0..steps {
            let a = r.below(N_ACTIONS as u32) as u16;
            let o = maze.step(a);
            agent.perceive(&Obs::new().cat(ROLE, o), a);
        }
        for _ in 0..2 {
            monad_core::dream::dream(agent, monad_core::dream::DreamConfig::default());
            let sc = SleepConfig { n_actions: N_ACTIONS, min_shared_actions: 2, ..Default::default() };
            for _ in 0..4 {
                let rep = agent.sleep(sc);
                if rep.nodes_after >= rep.nodes_before {
                    break;
                }
            }
            agent.reset_episode();
            agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
            for _ in 0..800 {
                let a = r.below(N_ACTIONS as u32) as u16;
                let o = maze.step(a);
                agent.perceive(&Obs::new().cat(ROLE, o), a);
            }
        }
    };

    // 위치식별 측정(대응표 작성 → 채점) — 학습은 계속되나 구조는 이미 성숙
    let mut measure = |agent: &mut Agent, maze: &mut Maze, r: &mut Rng| -> f32 {
        agent.reset_episode();
        agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
        let mut table: HashMap<(u32, usize), u32> = HashMap::new();
        for _ in 0..3000 {
            let a = r.below(N_ACTIONS as u32) as u16;
            let o = maze.step(a);
            let s = agent.perceive(&Obs::new().cat(ROLE, o), a);
            *table.entry((s.state, maze.cell())).or_insert(0) += 1;
        }
        let mut per_clone: HashMap<u32, Vec<(usize, u32)>> = HashMap::new();
        for (&(c, cell), &n) in &table {
            per_clone.entry(c).or_default().push((cell, n));
        }
        let map: HashMap<u32, usize> = per_clone
            .iter()
            .map(|(&c, v)| (c, v.iter().max_by_key(|x| x.1).unwrap().0))
            .collect();
        let mut hit = 0;
        let trials = 3000;
        for _ in 0..trials {
            let a = r.below(N_ACTIONS as u32) as u16;
            let o = maze.step(a);
            let s = agent.perceive(&Obs::new().cat(ROLE, o), a);
            if map.get(&s.state) == Some(&maze.cell()) {
                hit += 1;
            }
        }
        hit as f32 / trials as f32
    };

    let mut maze_a = Maze::new(6, 6, 6, 11);
    let mut maze_b = Maze::new(6, 6, 6, 99); // 같은 기호 어휘, 다른 배치 — 간섭 유발

    learn(&mut agent, &mut maze_a, 20_000, &mut r);
    let base_a = measure(&mut agent, &mut maze_a, &mut r);
    let nodes_after_a = agent.graph.n_nodes();
    println!("  A 학습 직후:  위치식별 {:.1}% (노드 {})", base_a * 100.0, nodes_after_a);

    learn(&mut agent, &mut maze_b, 20_000, &mut r);
    let base_b = measure(&mut agent, &mut maze_b, &mut r);
    println!("  B 학습 직후:  위치식별 {:.1}% (노드 {})", base_b * 100.0, agent.graph.n_nodes());

    let ret_a = measure(&mut agent, &mut maze_a, &mut r);
    println!("  A 복귀(재학습 없음): 위치식별 {:.1}%", ret_a * 100.0);
    let retention = ret_a / base_a.max(1e-6);
    let ok = retention >= 0.90 && base_b >= 0.90;
    println!(
        "\n판정: 유지율 = {:.1}% (DoD ≥90%) · B 습득 {:.1}% → {}",
        retention * 100.0,
        base_b * 100.0,
        if ok { "✅ 통과" } else { "❌ 실패" }
    );
    ok
}

fn main() {
    let a = revision_test();
    let b = retention_test();
    println!("\n=========================================================================");
    println!("M1 첫 시험 요약: 규칙 개정 {} · 연속 학습 유지 {}",
        if a { "✅" } else { "❌" }, if b { "✅" } else { "❌" });
    std::process::exit(if a && b { 0 } else { 1 });
}
