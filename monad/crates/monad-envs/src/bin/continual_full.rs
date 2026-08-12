//! M1 본게이트 — 가설 (c): 전 생애 풀-리빌드 EM 경로.
//!
//! 명시적 "지도" 변수 없이, 클론-HMM EM에 환경 문맥까지 맡긴다: 같은 지각이라도
//! 다른 환경에서 다르게 행동하면 EM이 다른 클론으로 갈라낸다(A→B→A 유지율 100%가
//! 이 노선의 소규모 증거). 1차 시도의 붕괴 원인 두 가지를 수리한 재도전이다:
//! ① EM 초기화의 clone_ix 필터가 후기 환경 클론을 버리던 버그 → 최근성 순위 배정
//! ② K=128(K²=16384)의 비용 폭발 → K=64로 절반(필요량 = 환경10 × 칸6/지각 = 60)
//!
//! 실행: `cargo run --release --bin continual-full`

use monad_core::encode::Obs;
use monad_core::rng::Rng;
use monad_core::sleep::SleepConfig;
use monad_core::wake::{Agent, Config};
use monad_envs::maze::{Maze, N_ACTIONS};
use std::collections::HashMap;

const ROLE: u16 = 0;
const N_ENVS: usize = 10;
const LEARN_STEPS: usize = 9_000;

fn consolidate(agent: &mut Agent) {
    monad_core::dream::dream(
        agent,
        monad_core::dream::DreamConfig { max_clones: 64, consume: false, ..Default::default() },
    );
    // bisim 병합은 여기서 뺀다: 그룹 제한 없는 와일드카드 병합이 서로 다른
    // 환경의 칸을 합쳐(2개 행동만 일치해도) 환경 수에 비례해 교차 오염을 만든다
    // — 환경4부터의 열화 원인으로 실측 특정. EM이 이미 최소 구조를 찾는다.
    let _ = SleepConfig::default();
}

fn learn(agent: &mut Agent, maze: &mut Maze, steps: usize, r: &mut Rng) {
    agent.reset_episode();
    agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
    for _ in 0..steps {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        agent.perceive(&Obs::new().cat(ROLE, o), a);
    }
    consolidate(agent);
}

fn measure(agent: &mut Agent, maze: &mut Maze, r: &mut Rng) -> f32 {
    agent.reset_episode();
    agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
    let mut table: HashMap<(u32, usize), u32> = HashMap::new();
    for _ in 0..1500 {
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
    let trials = 1500;
    for _ in 0..trials {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        let s = agent.perceive(&Obs::new().cat(ROLE, o), a);
        if map.get(&s.state) == Some(&maze.cell()) {
            hit += 1;
        }
    }
    hit as f32 / trials as f32
}

fn main() {
    println!("=========================================================================");
    println!("M1 본게이트 · 풀-리빌드 EM 경로 — {N_ENVS}개 환경, 같은 기호 어휘");
    println!("=========================================================================");
    println!("DoD: 유지율 ≥90% (재학습 없음) · 습득 ≤10⁴ 걸음\n");

    let mut agent = Agent::with_config(Config::default()); // 지도 추론 불필요 — EM이 맡는다
    agent.encoder.declare(ROLE, "cell");
    let mut r = Rng::new(2026);

    let mut mazes: Vec<Maze> = (0..N_ENVS).map(|i| Maze::new(6, 6, 6, 11 + i as u64)).collect();
    let mut acquired = vec![0f32; N_ENVS];

    let t0 = std::time::Instant::now();
    for i in 0..N_ENVS {
        learn(&mut agent, &mut mazes[i], LEARN_STEPS, &mut r);
        acquired[i] = measure(&mut agent, &mut mazes[i], &mut r);
        println!(
            "환경 {:>2}: 습득 {:>5.1}% · 노드 {:>4} · 경과 {:>4.0}초",
            i + 1,
            acquired[i] * 100.0,
            agent.graph.n_nodes(),
            t0.elapsed().as_secs_f32()
        );
    }

    println!("\n-- 전 환경 복귀 측정 (재학습·꿈 없음) --");
    println!("{:>6} {:>10} {:>10} {:>8}", "환경", "습득 시", "복귀 시", "유지율");
    println!("{}", "-".repeat(40));
    let mut ret_sum = 0f32;
    let mut min_ret = f32::MAX;
    for i in 0..N_ENVS {
        let back = measure(&mut agent, &mut mazes[i], &mut r);
        let ret = (back / acquired[i].max(1e-6)).min(1.5);
        ret_sum += ret;
        min_ret = min_ret.min(ret);
        println!(
            "{:>6} {:>9.1}% {:>9.1}% {:>7.1}%",
            i + 1,
            acquired[i] * 100.0,
            back * 100.0,
            ret * 100.0
        );
    }
    println!("{}", "-".repeat(40));
    let mean_ret = ret_sum / N_ENVS as f32;
    let mean_acq = acquired.iter().sum::<f32>() / N_ENVS as f32;
    println!(
        "평균 습득 {:.1}% · 평균 유지율 {:.1}% · 최저 유지율 {:.1}% · 노드 {} · {:.0}초",
        mean_acq * 100.0,
        mean_ret * 100.0,
        min_ret * 100.0,
        agent.graph.n_nodes(),
        t0.elapsed().as_secs_f32()
    );

    let pass = mean_ret >= 0.90 && mean_acq >= 0.90;
    println!(
        "\n▶ M1 본게이트(풀-리빌드): {} (평균 유지 {:.1}% / 평균 습득 {:.1}%)",
        if pass { "✅ 통과" } else { "❌ 미통과" },
        mean_ret * 100.0,
        mean_acq * 100.0
    );
    std::process::exit(if pass { 0 } else { 1 });
}
