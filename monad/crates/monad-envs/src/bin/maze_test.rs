//! B3 DoD: 별칭 미로 재현.
//!
//! 검사 항목
//!  1. **클론 순도** — 학습된 각 클론이 하나의 참 위치에 대응하는가.
//!     (관측이 같아도 문맥으로 위치를 구분해냈다는 뜻)
//!  2. **위치 식별률** — 학습 후 걸으면서 자기 위치를 맞히는가.
//!  3. **길찾기** — 목표 칸으로 계획해 실제로 도달하는가(최단경로 대비).
//!  4. **어블레이션** — 회고적 클론 분화를 끄면 무너지는가.
//!
//! 실행: `cargo run --release --bin maze-test`

use monad_core::encode::Obs;
use monad_core::rng::Rng;
use monad_core::sleep::SleepConfig;
use monad_core::wake::{Agent, Config};
use monad_envs::maze::{Maze, N_ACTIONS};
use std::collections::HashMap;

const ROLE: u16 = 0;

struct Result_ {
    nodes_wake: usize,
    nodes: usize,
    purity: f32,
    frag: f32,
    id_rate: f32,
    plan_ok: f32,
    plan_ratio: f32,
}

fn run(
    w: i32,
    h: i32,
    syms: u32,
    steps: usize,
    seed: u64,
    cfg: Config,
    split: bool,
    do_sleep: bool,
) -> Result_ {
    run_inner(w, h, syms, steps, seed, cfg, split, do_sleep, false)
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    w: i32,
    h: i32,
    syms: u32,
    steps: usize,
    seed: u64,
    cfg: Config,
    split: bool,
    do_sleep: bool,
    debug: bool,
) -> Result_ {
    let mut maze = Maze::new(w, h, syms, seed);
    let mut r = Rng::new(seed ^ 0x5eed);
    let mut agent = Agent::with_config(Config {
        // 분화를 끄려면 지각당 클론을 1개로 제한한다(어블레이션)
        max_clones: if split { cfg.max_clones } else { 1 },
        ..cfg
    });
    agent.encoder.declare(ROLE, "cell");

    // --- 각성: 무작위 걷기로 세계를 겪는다 ---
    agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
    for _ in 0..steps {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        agent.perceive(&Obs::new().cat(ROLE, o), a);
    }
    let nodes_wake = agent.graph.n_nodes();

    // --- 수면: 꿈(EM 전역 추론)으로 지도를 다시 세우고, 구별 불가 상태를 합친다.
    //     한 밤에 꿈을 두 번 꾼다: 첫 꿈이 세운 지도 위에서 잠깐 걷고(재정착 경험),
    //     그 경험까지 넣어 다시 꾸면 남은 중복 클론이 마저 정리된다.
    if do_sleep {
        let sc = SleepConfig { n_actions: N_ACTIONS, min_shared_actions: 2, ..Default::default() };
        for _round in 0..2 {
            monad_core::dream::dream(&mut agent, monad_core::dream::DreamConfig::default());
            for _ in 0..4 {
                let rep = agent.sleep(sc);
                if rep.nodes_after >= rep.nodes_before {
                    break;
                }
            }
            agent.reset_episode();
            agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
            for _ in 0..1000 {
                let a = r.below(N_ACTIONS as u32) as u16;
                let o = maze.step(a);
                agent.perceive(&Obs::new().cat(ROLE, o), a);
            }
        }
    }

    // --- 평가 1·2: 클론↔참위치 대응표 ---
    let mut table: HashMap<(u32, usize), u32> = HashMap::new();
    let eval_steps = 4000usize;
    let mut correct = 0usize;
    let mut cell_of_clone: HashMap<u32, usize> = HashMap::new();

    // 1차 통과: 대응표 작성
    for _ in 0..eval_steps {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        let s = agent.perceive(&Obs::new().cat(ROLE, o), a);
        *table.entry((s.state, maze.cell())).or_insert(0) += 1;
    }
    // 각 클론의 대표 위치
    let mut per_clone: HashMap<u32, Vec<(usize, u32)>> = HashMap::new();
    for (&(c, cell), &n) in &table {
        per_clone.entry(c).or_default().push((cell, n));
    }
    let mut pure_sum = 0f64;
    let mut tot = 0f64;
    for (c, v) in &per_clone {
        let total: u32 = v.iter().map(|x| x.1).sum();
        let best = v.iter().max_by_key(|x| x.1).unwrap();
        cell_of_clone.insert(*c, best.0);
        pure_sum += best.1 as f64;
        tot += total as f64;
    }
    let purity = (pure_sum / tot.max(1.0)) as f32;

    // 위치당 클론 수(파편화)
    let mut clones_per_cell: HashMap<usize, std::collections::HashSet<u32>> = HashMap::new();
    for (&c, &cell) in &cell_of_clone {
        clones_per_cell.entry(cell).or_default().insert(c);
    }
    let frag = clones_per_cell.values().map(|s| s.len()).sum::<usize>() as f32
        / clones_per_cell.len().max(1) as f32;

    // 2차 통과: 위치 식별률
    for _ in 0..eval_steps {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        let s = agent.perceive(&Obs::new().cat(ROLE, o), a);
        if cell_of_clone.get(&s.state) == Some(&maze.cell()) {
            correct += 1;
        }
    }
    let id_rate = correct as f32 / eval_steps as f32;

    // --- 평가 3: 길찾기 ---
    let trials = 40usize;
    let mut ok = 0usize;
    let mut ratio_sum = 0f32;
    for t in 0..trials {
        let goal = (t * 7 + 3) % maze.n_cells();
        // 목표 칸에 대응하는 클론들을 선호로 설정
        agent.clear_preferences();
        let mut any = false;
        for (&c, &cell) in &cell_of_clone {
            if cell == goal {
                agent.prefer_state(c, 8.0);
                any = true;
            }
        }
        if !any {
            continue;
        }
        // 임의 위치에서 출발해 **확신이 설 때까지** 둘러본다(자기 정위).
        // 믿음 분포를 들고 다니는 이유가 이것이다: "지금 어디인지 모른다"를
        // 시스템이 스스로 안다(belief_entropy). 확신 없이 출발하면 계획은 무의미하다.
        let start = (t * 13 + 5) % maze.n_cells();
        maze.set_cell(start);
        agent.reset_episode();
        agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
        for _ in 0..16 {
            if agent.belief_entropy() < 0.3 && agent.state.is_some() {
                break;
            }
            let a = r.below(N_ACTIONS as u32) as u16;
            let o = maze.step(a);
            agent.perceive(&Obs::new().cat(ROLE, o), a);
        }
        let from = maze.cell();
        let sp = maze.shortest(from, goal).max(1);
        let budget = (sp * 4).max(12) as usize;
        let loc_at_start = cell_of_clone.get(&agent.state.unwrap_or(u32::MAX)).copied();
        let mut reached = 0usize;
        let mut visited: Vec<usize> = vec![from];
        for step in 0..budget {
            if maze.cell() == goal {
                reached = step;
                break;
            }
            let a = match agent.plan(N_ACTIONS) {
                Some(a) => a,
                None => r.below(N_ACTIONS as u32) as u16,
            };
            let o = maze.step(a);
            agent.perceive(&Obs::new().cat(ROLE, o), a);
            visited.push(maze.cell());
            if maze.cell() == goal {
                reached = step + 1;
                break;
            }
        }
        if maze.cell() == goal {
            ok += 1;
            ratio_sum += reached.max(1) as f32 / sp as f32;
        } else if debug {
            let believed = cell_of_clone.get(&agent.state.unwrap_or(u32::MAX)).copied();
            println!(
                "실패 trial{t}: 출발{from}(정위:{loc_at_start:?}) 목표{goal} sp={sp} 예산={budget} 끝{}(믿음:{believed:?} H={:.2}) 경로={:?}",
                maze.cell(),
                agent.belief_entropy(),
                &visited[..visited.len().min(20)]
            );
        }
    }
    agent.clear_preferences();

    Result_ {
        nodes_wake,
        nodes: agent.graph.n_nodes(),
        purity,
        frag,
        id_rate,
        plan_ok: ok as f32 / trials as f32,
        plan_ratio: if ok > 0 { ratio_sum / ok as f32 } else { 0.0 },
    }
}

fn main() {
    if std::env::args().any(|a| a == "--debug-nav") {
        println!("== 길찾기 실패 진단 (6x6, 기호 6종, 시드 1) ==");
        run_inner(6, 6, 6, 20_000, 1, Config::default(), true, true, true);
        return;
    }
    println!("=========================================================================");
    println!("B3 DoD — 별칭 미로 (Aliasing Maze) 재현");
    println!("=========================================================================");
    println!("관측 기호가 칸 수보다 훨씬 적어 '많은 칸이 똑같이 보이는' 방.");
    println!("관측만으로는 위치를 알 수 없고 문맥으로만 알 수 있다.\n");

    let cfg = Config::default();

    println!("-- 규모별 (무작위 걷기 20,000걸음, 시드 1) --");
    println!(
        "{:>8} {:>5} {:>6} {:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>7}",
        "방크기", "기호", "별칭도", "참칸수", "각성후", "수면후", "순도", "위치식별", "길찾기", "경로비"
    );
    println!("{}", "-".repeat(84));
    for (w, h, s) in [(4i32, 4i32, 4u32), (5, 5, 5), (6, 6, 6), (8, 8, 8), (6, 6, 3)] {
        let r = run(w, h, s, 20_000, 1, cfg, true, true);
        let maze = Maze::new(w, h, s, 1);
        println!(
            "{:>8} {:>5} {:>6.1} {:>7} {:>7} {:>7} {:>7.3} {:>7.1}% {:>7.1}% {:>7.2}",
            format!("{w}x{h}"),
            s,
            maze.aliasing(),
            maze.n_cells(),
            r.nodes_wake,
            r.nodes,
            r.purity,
            r.id_rate * 100.0,
            r.plan_ok * 100.0,
            r.plan_ratio
        );
    }

    println!("\n-- 시드 안정성 (6x6=36칸, 기호 6종) --");
    println!("{:>6} {:>8} {:>8} {:>7} {:>8} {:>8}", "시드", "각성후", "수면후", "순도", "위치식별", "길찾기");
    println!("{}", "-".repeat(50));
    let mut id_sum = 0f32;
    let mut plan_sum = 0f32;
    let seeds = [1u64, 2, 3, 4, 5];
    for &sd in &seeds {
        let r = run(6, 6, 6, 20_000, sd, cfg, true, true);
        id_sum += r.id_rate;
        plan_sum += r.plan_ok;
        println!(
            "{:>6} {:>8} {:>8} {:>7.3} {:>7.1}% {:>7.1}%",
            sd, r.nodes_wake, r.nodes, r.purity, r.id_rate * 100.0, r.plan_ok * 100.0
        );
    }
    let n = seeds.len() as f32;
    println!("{}", "-".repeat(50));
    println!("평균 위치식별 {:.1}%, 길찾기 {:.1}%", id_sum / n * 100.0, plan_sum / n * 100.0);

    println!("\n-- 어블레이션 (6x6, 기호 6종) --");
    println!("{:>22} {:>8} {:>7} {:>8} {:>8}", "구성", "노드수", "순도", "위치식별", "길찾기");
    println!("{}", "-".repeat(58));
    for (name, split, slp) in [
        ("전체", true, true),
        ("− 수면 압축", true, false),
        ("− 클론 분화", false, true),
    ] {
        let r = run(6, 6, 6, 20_000, 1, cfg, split, slp);
        println!(
            "{:>22} {:>8} {:>7.3} {:>7.1}% {:>7.1}%",
            name, r.nodes, r.purity, r.id_rate * 100.0, r.plan_ok * 100.0
        );
    }

    println!("\n-- 학습 곡선: 몇 걸음이면 지도가 서는가 (6x6, 기호 6종) --");
    println!("{:>10} {:>8} {:>8} {:>8} {:>8}", "걸음", "각성후", "수면후", "위치식별", "길찾기");
    println!("{}", "-".repeat(46));
    for steps in [500usize, 1000, 2000, 5000, 10_000, 20_000] {
        let r = run(6, 6, 6, steps, 1, cfg, true, true);
        println!(
            "{:>10} {:>8} {:>8} {:>7.1}% {:>7.1}%",
            steps, r.nodes_wake, r.nodes, r.id_rate * 100.0, r.plan_ok * 100.0
        );
    }

    println!("\n판정: 위치식별 ≥ 90% 이고 길찾기 ≥ 90% 이면 B3 DoD 통과.");
}
