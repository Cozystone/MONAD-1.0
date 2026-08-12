//! M1 본게이트 — 10환경 연속 학습.
//!
//! 미로 10개를 순서대로 배운다. 전부 **같은 기호 어휘**(0..6)를 쓰므로 지각이
//! 전부 겹친다 — 가중치 시스템이라면 나중 학습이 앞 학습을 덮어쓰는 최악의
//! 간섭 조건이다. MONAD의 주장: 구조는 덮어쓰지 않고 자라므로 망각이 없다.
//!
//! DoD (개발계획 M1): 유지율 ≥ 90% (재학습 없이), 환경당 습득 ≤ 10⁴ 스텝.
//!
//! 실행: `cargo run --release --bin continual-test`

use monad_core::encode::Obs;
use monad_core::rng::Rng;
use monad_core::sleep::SleepConfig;
use monad_core::wake::{Agent, Config};
use monad_envs::maze::{Maze, N_ACTIONS};
use std::collections::HashMap;

const ROLE: u16 = 0;
const N_ENVS: usize = 10;
const LEARN_STEPS: usize = 9_000; // 환경당 습득 예산 (DoD ≤10⁴)

fn consolidate(agent: &mut Agent) {
    // 수면 = 응고화: 해마 버퍼(미소화 에피소드)만 꿈꾸고 피질(기존 그래프)에 병합.
    // 전 생애 재건 꿈은 10환경에서 실측 실패(3시간·습득 붕괴) — LAB-NOTEBOOK 참조.
    // 용의자 1 원격측정: 에피소드 조각화(수·평균 길이)
    let n_eps = agent.episodes.iter().filter(|e| e.len() >= 2).count();
    let mean_len = if n_eps > 0 {
        agent.episodes.iter().map(|e| e.len()).sum::<usize>() / n_eps
    } else {
        0
    };
    let rep = monad_core::dream::dream(
        agent,
        monad_core::dream::DreamConfig {
            max_clones: 16,
            consume: true,
            // 용의자 2 검증: 고정 시드가 유사 입력에서 같은 나쁜 분지로 수렴하는
            // 것을 막기 위해 밤마다 변주한다(결정론은 tick 경유로 유지).
            seed: 0xD2EA ^ agent.graph.tick,
            ..Default::default()
        },
    );
    if std::env::var("MONAD_TELEM").is_ok() {
        println!(
            "    [꿈] 에피소드 {}개(평균 {}걸음) · 스텝 {} · 정렬흡수 {} · 신규 {} · 노드 {}→{} · ll {:.3}",
            n_eps, mean_len, rep.steps_used, rep.aligned, rep.created, rep.nodes_before,
            rep.nodes_after, rep.final_ll
        );
        // 지도별 피질 벡터 — blob이 자라는 밤을 특정하는 프로브
        let mc: Vec<u32> = agent.map_cortical.iter().copied().take(14).collect();
        println!("    [피질] {:?} (n_maps {})", mc, agent.n_maps);
    }
    let sc = SleepConfig { n_actions: N_ACTIONS, min_shared_actions: 2, ..Default::default() };
    for _ in 0..4 {
        let rep = agent.sleep(sc);
        if rep.nodes_after >= rep.nodes_before {
            break;
        }
    }
}

fn learn(agent: &mut Agent, maze: &mut Maze, steps: usize, r: &mut Rng) {
    let diag = std::env::var("MONAD_DIAG").is_ok();
    agent.reset_episode();
    agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
    for i in 0..steps {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        agent.perceive(&Obs::new().cat(ROLE, o), a);
        // 주기 응고는 MONAD_PERIODIC=1일 때만 — 오라클 실험이 주기 응고 루프
        // 자체가 품질 병목임을 보였다(완벽한 지도 정답에도 습득 53.7%).
        if std::env::var("MONAD_PERIODIC").is_ok() && i > 0 && i % 1500 == 0 {
            consolidate(agent);
            agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
        }
        if diag && (i == 100 || i == 1400 || i == 2000 || i == 6000) {
            let (bad, scores) = agent.map_diag();
            println!(
                "    [diag t={i}] 불량률 {:.2} · 활성 {} · 생존율 {:?}",
                bad,
                agent.active_map,
                scores.iter().map(|s| (s * 100.0).round() / 100.0).collect::<Vec<_>>()
            );
        }
    }
    consolidate(agent);
}

/// 위치식별 측정: 대응표 작성(1500) → 채점(1500). 꿈 없음.
/// 온라인 시스템은 측정 중에도 배우므로(끌 수 없다 — 설계상 항상 켜짐),
/// 습득·복귀 측정을 **같은 길이**로 맞춰 공정 비교한다.
fn measure(agent: &mut Agent, maze: &mut Maze, r: &mut Rng) -> f32 {
    let diag = std::env::var("MONAD_DIAG").is_ok();
    agent.reset_episode();
    agent.perceive(&Obs::new().cat(ROLE, maze.observe()), 0);
    // 워밍업: 응고·은퇴 직후에는 문맥 색인이 비어 재정착에 수백 걸음이 든다.
    // 그 과도기를 채점에 넣으면 지도 품질이 아니라 재정착 속도를 재게 된다(가설 i).
    for i in 0..400 {
        let a = r.below(N_ACTIONS as u32) as u16;
        let o = maze.step(a);
        agent.perceive(&Obs::new().cat(ROLE, o), a);
        if diag && (i == 50 || i == 200 || i == 399) {
            let (bad, scores) = agent.map_diag();
            println!(
                "        [측정診 t={i}] 불량률 {:.2} · 활성 {} · 생존율 {:?}",
                bad,
                agent.active_map,
                scores.iter().map(|s| (s * 100.0).round() / 100.0).collect::<Vec<_>>()
            );
        }
    }
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
    // 동률은 셀 번호로 고정 — HashMap 순회 순서가 argmax에 새면 실행마다 ±수 히트.
    let map: HashMap<u32, usize> = per_clone
        .iter()
        .map(|(&c, v)| {
            (c, v.iter().max_by_key(|x| (x.1, std::cmp::Reverse(x.0))).unwrap().0)
        })
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
    println!("M1 본게이트 — {N_ENVS}개 환경 연속 학습 (전 환경 같은 기호 어휘 = 최대 간섭)");
    println!("=========================================================================");
    println!("환경: 6x6 미로 x{N_ENVS}, 기호 6종 공유. 환경당 학습 {LEARN_STEPS}걸음 + 수면.");
    println!("DoD: 유지율 ≥90% (재학습 없음) · 습득 ≤10⁴ 걸음\n");

    // MONAD_MAPS=1이면 지도 추론을 켠다(기본: 정렬 병합만으로 시험).
    let maps_on = std::env::var("MONAD_MAPS").is_ok();
    println!("지도 추론: {}\n", if maps_on { "ON" } else { "OFF (정렬 병합 단독)" });
    let mut agent = Agent::with_config(Config { map_inference: maps_on, ..Config::default() });
    agent.encoder.declare(ROLE, "cell");
    let seed: u64 = std::env::var("MONAD_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(2026);
    println!("시드: {seed}");
    let mut r = Rng::new(seed);

    // MONAD_REVERSE=1: 환경 순서를 뒤집는다 — 붕괴가 생애 위치를 따르는지(기제 결함)
    // 특정 미로를 따르는지(고유 난이도) 가르는 인수분해 실험.
    let mut order: Vec<u64> = (0..N_ENVS as u64).map(|i| 11 + i).collect();
    if std::env::var("MONAD_REVERSE").is_ok() {
        order.reverse();
        println!("환경 순서 반전: {order:?}");
    }
    let mut mazes: Vec<Maze> = order.iter().map(|&s| Maze::new(6, 6, 6, s)).collect();
    let mut acquired = vec![0f32; N_ENVS];

    // MONAD_ORACLE=1: 환경 경계에서 지도 정답을 알려준다(감지기 배제 인수분해 실험).
    let oracle = std::env::var("MONAD_ORACLE").is_ok();
    if oracle {
        println!("오라클 모드: 지도 정답 외부 지정(감지기 배제)\n");
    }
    let t0 = std::time::Instant::now();
    for i in 0..N_ENVS {
        if oracle {
            agent.oracle_set_map(i as u32);
        }
        learn(&mut agent, &mut mazes[i], LEARN_STEPS, &mut r);
        acquired[i] = measure(&mut agent, &mut mazes[i], &mut r);
        let mut per_map: HashMap<u32, usize> = HashMap::new();
        for &m in &agent.node_map {
            *per_map.entry(m).or_insert(0) += 1;
        }
        let mut pm: Vec<(u32, usize)> = per_map.into_iter().collect();
        pm.sort_unstable();
        println!(
            "환경 {:>2}: 습득 {:>5.1}% · 노드 {:>4} · 지도 {}개(활성 {}) · 전환 {} · 지도별 {:?} · {:.0}초",
            i + 1,
            acquired[i] * 100.0,
            agent.graph.n_nodes(),
            agent.n_maps,
            agent.active_map,
            agent.stats.map_switches,
            &pm[..pm.len().min(12)],
            t0.elapsed().as_secs_f32()
        );
    }

    // 막차 응고: 마지막 환경의 트래픽에도 밤을 준다. 새 환경 노출이 없으므로
    // "재학습 없음" 조건과 무관한, 이미 겪은 경험의 정리다("자기 전에 정리").
    consolidate(&mut agent);

    println!("\n-- 전 환경 복귀 측정 (재학습·꿈 없음) --");
    println!("{:>6} {:>10} {:>10} {:>8}", "환경", "습득 시", "복귀 시", "유지율");
    println!("{}", "-".repeat(40));
    // MONAD_ORACLE_RETURN=1: 학습은 감지기, 복귀만 오라클 — 잔여 유지 갭이
    // "복귀 재인식"에 있는지 "젊은 지도의 피질 품질"에 있는지 가르는 인수분해.
    let oracle_return = oracle || std::env::var("MONAD_ORACLE_RETURN").is_ok();
    let telem = std::env::var("MONAD_TELEM").is_ok();
    let mut ret_sum = 0f32;
    let mut min_ret = f32::MAX;
    // MONAD_RETURN_REVERSE=1: 복귀 순서만 뒤집는다 — 복귀 열화가 "복귀 위치 누적"
    // (순서를 따라감)인지 "지도 나이"(환경 번호를 따라감)인지 가르는 인수분해.
    let ret_order: Vec<usize> = if std::env::var("MONAD_RETURN_REVERSE").is_ok() {
        (0..N_ENVS).rev().collect()
    } else {
        (0..N_ENVS).collect()
    };
    for i in ret_order {
        if oracle_return {
            agent.oracle_set_map(i as u32);
        }
        let n0 = agent.graph.n_nodes();
        let back = measure(&mut agent, &mut mazes[i], &mut r);
        // (복귀 사이의 밤은 기각 — 실측 악화 80.0/74.3/84.2: 스크래치 누적은 구속
        //  조건이 아니고, 복귀 에피소드 재분류가 지도 내 중복 사본을 만든다. 시도 56.)
        let ret = back / acquired[i].max(1e-6);
        ret_sum += ret;
        min_ret = min_ret.min(ret);
        println!(
            "{:>6} {:>9.1}% {:>9.1}% {:>7.1}%",
            i + 1,
            acquired[i] * 100.0,
            back * 100.0,
            ret * 100.0
        );
        if telem {
            // 스크래치 재성장량(놀람의 총량)과 활성 지도의 실제 크기 — 깨끗한 지도가
            // 복귀에서 무너지는 잔여 수수께끼의 프로브.
            let active_sz =
                agent.node_map.iter().filter(|&&m| m == agent.active_map).count();
            println!(
                "        [복귀 {}] 활성 {} · 지도 {}개 · 전환누계 {} · p_new {:.2} · 신규스크래치 {} · 활성지도크기 {}",
                i + 1,
                agent.active_map,
                agent.n_maps,
                agent.stats.map_switches,
                agent.p_new,
                agent.graph.n_nodes() - n0,
                active_sz
            );
        }
    }
    println!("{}", "-".repeat(40));
    let mean_ret = ret_sum / N_ENVS as f32;
    let mean_acq = acquired.iter().sum::<f32>() / N_ENVS as f32;
    println!(
        "평균 습득 {:.1}% · 평균 유지율 {:.1}% · 최저 유지율 {:.1}% · 최종 노드 {}",
        mean_acq * 100.0,
        mean_ret * 100.0,
        min_ret * 100.0,
        agent.graph.n_nodes()
    );
    println!("메모리 추정 {:.1}MB · 총 소요 {:.0}초",
        agent.graph.memory_estimate() as f64 / 1e6, t0.elapsed().as_secs_f32());

    let pass = mean_ret >= 0.90 && mean_acq >= 0.90;
    println!(
        "\n▶ M1 본게이트: {} (평균 유지 {:.1}% / 평균 습득 {:.1}%)",
        if pass { "✅ 통과" } else { "❌ 미통과" },
        mean_ret * 100.0,
        mean_acq * 100.0
    );
    std::process::exit(if pass { 0 } else { 1 });
}
