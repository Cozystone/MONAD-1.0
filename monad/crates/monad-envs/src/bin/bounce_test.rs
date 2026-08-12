//! Bounce Test — M0의 관문 (개발계획 D1, PRD v0.2 §6).
//!
//! ```text
//! Phase 1  관찰      빨간 원의 충돌을 보고 반사 스키마를 스스로 만든다
//! Phase 2  신규 객체  처음 보는 파란 삼각형의 반사를 예측한다      ← 실패 시 M0 킬
//! Phase 3  외관 불변  벽 무늬·색을 바꿔도 예측이 유지된다
//! Phase 4  규칙 개정  중력 추가(M1 범위 — 정보 제공용으로만 실행)
//! ```
//!
//! 통과 판정에는 두 가지가 모두 필요하다:
//!  (a) 예측 정확도 — 충돌 직전에 물은 "다음 속도"가 실제와 일치(부호 기준)
//!  (b) 인과 슬롯 일치 — 스키마가 {접촉, 벽면}만 제약하고 외양은 전부 변수화
//!
//! 내부 스키마는 사람이 읽는 형식으로 덤프한다(유리상자 의무).
//!
//! 실행: `cargo run --release --bin bounce-test`

use monad_core::encode::Obs;
use monad_core::rng::Rng;
use monad_core::schema::{induce, Event, InduceConfig, SchemaLib};
use monad_core::wake::{Agent, Config};
use monad_envs::bounce::{
    Body, BounceWorld, COLOR_NAMES, EFFECT_NAMES, SHAPE_NAMES, SIDE_NAMES, TEX_NAMES,
};

// 슬롯 번호(스키마 학습기와 공유하는 어휘)
const S_CONTACT: u16 = 0;
const S_SIDE: u16 = 1;
const S_SHAPE: u16 = 2;
const S_COLOR: u16 = 3;
const S_TEX: u16 = 4;
const S_SPEED: u16 = 5;
const S_SIZE: u16 = 6;

fn slot_name(s: u16) -> String {
    match s {
        S_CONTACT => "contact".into(),
        S_SIDE => "wall_side".into(),
        S_SHAPE => "shape".into(),
        S_COLOR => "color".into(),
        S_TEX => "wall_texture".into(),
        S_SPEED => "speed".into(),
        S_SIZE => "size".into(),
        _ => format!("slot{s}"),
    }
}
fn effect_name(e: u32) -> String {
    EFFECT_NAMES.get(e as usize).unwrap_or(&"?").to_string()
}

/// 환경 틱 → 스키마 학습기의 사건.
fn tick_event(w: &BounceWorld, contact: bool, side: u32, effect: u32) -> Event {
    let speed = (w.vel.0 * w.vel.0 + w.vel.1 * w.vel.1).sqrt();
    Event {
        cats: vec![
            (S_CONTACT, contact as u32),
            (S_SIDE, side),
            (S_SHAPE, w.body.shape),
            (S_COLOR, w.body.color),
            (S_TEX, w.tex),
        ],
        nums: vec![(S_SPEED, speed), (S_SIZE, w.body.size)],
        effect,
    }
}

/// 환경 틱 → 에이전트 관측(그래프 학습 경로 — 스키마 경로와 병행).
fn tick_obs(w: &BounceWorld, t: &monad_envs::bounce::Tick) -> Obs {
    let bin = |x: f32, n: u32| ((x.clamp(0.0, 0.999) * n as f32) as u32).min(n - 1);
    Obs::new()
        .cat(0, bin(t.pos.0, 8))
        .cat(1, bin(t.pos.1, 8))
        .cat(2, (t.v_after.0 > 0.0) as u32)
        .cat(3, (t.v_after.1 > 0.0) as u32)
        .cat(4, w.body.shape)
        .cat(5, w.body.color)
        .num(6, t.v_after.0)
        .num(7, t.v_after.1)
}

/// 한 세계를 굴리며 사건·관측을 수집한다.
fn observe_world(
    agent: &mut Agent,
    events: &mut Vec<Event>,
    body: Body,
    tex: u32,
    ticks: usize,
    gravity: f32,
    seed: u64,
) -> usize {
    let mut rng = Rng::new(seed);
    let mut w = BounceWorld::new(body, tex, &mut rng);
    w.gravity = gravity;
    agent.reset_episode();
    let mut collisions = 0;
    for _ in 0..ticks {
        let t = w.step();
        agent.perceive(&tick_obs(&w, &t), 0);
        events.push(tick_event(&w, t.contact, t.side, t.effect));
        if t.contact {
            collisions += 1;
        }
    }
    collisions
}

/// 충돌 직전 예측 시험: 다음 틱의 속도 부호를 스키마로 예측한다.
struct PhaseResult {
    asked: usize,
    correct: usize,
}

fn predict_phase(
    lib: &SchemaLib,
    body: Body,
    tex: u32,
    gravity: f32,
    n_collisions: usize,
    seed: u64,
) -> PhaseResult {
    let mut rng = Rng::new(seed);
    let mut w = BounceWorld::new(body, tex, &mut rng);
    w.gravity = gravity;
    let mut asked = 0;
    let mut correct = 0;
    let mut guard = 0usize;
    while asked < n_collisions && guard < 100_000 {
        guard += 1;
        let (will, side) = w.peek_contact();
        if will {
            // 예측 질의: 접촉 직전의 사건 슬롯으로 스키마에 묻는다
            let q = {
                let mut e = tick_event(&w, true, side, 0);
                e.effect = u32::MAX; // 미지 — 라벨은 조회에 쓰이지 않는다
                e
            };
            let pred = lib.predict(&q);
            let v_now = w.vel;
            let t = w.step();
            debug_assert!(t.contact);
            // 예측한 결과 분류 → 속도 부호 예측으로 변환해 실제와 대조
            let pv = match pred {
                Some(1) => (-v_now.0, v_now.1),
                Some(2) => (v_now.0, -v_now.1),
                _ => v_now,
            };
            let sign_ok = (pv.0 > 0.0) == (t.v_after.0 > 0.0)
                && (pv.1 > 0.0) == (t.v_after.1 > 0.0);
            asked += 1;
            if sign_ok {
                correct += 1;
            }
        } else {
            w.step();
        }
    }
    PhaseResult { asked, correct }
}

fn causal_slots_ok(lib: &SchemaLib) -> (bool, Vec<String>) {
    // 반사 스키마(effect != none)는 {contact, wall_side}만 제약해야 한다
    let mut offending = Vec::new();
    let mut ok = true;
    for s in lib.schemas.iter().filter(|s| s.effect != 0) {
        for slot in s.slots() {
            if slot != S_CONTACT && slot != S_SIDE {
                ok = false;
                offending.push(slot_name(slot));
            }
        }
    }
    offending.sort();
    offending.dedup();
    (ok, offending)
}

fn dump(lib: &SchemaLib, title: &str) {
    println!("\n  ── 스키마 라이브러리 덤프: {title} ──");
    if lib.is_empty() {
        println!("  (비어 있음)");
    }
    for (i, s) in lib.schemas.iter().enumerate() {
        println!("  Schema #{i}: {}", s.describe(&slot_name, &effect_name));
    }
    if let Some(d) = lib.default_effect {
        println!("  (그 외) → {}", effect_name(d));
    }
}

fn main() {
    let ablate_schema = std::env::args().any(|a| a == "--no-schema");
    println!("=========================================================================");
    println!("Bounce Test — M0 관문 (경험 → 구조 → 압축 → 추상화 → 전이)");
    println!("=========================================================================");

    let mut agent = Agent::with_config(Config::default());
    for (r, n) in [(0u16, "px"), (1, "py"), (2, "vx_sign"), (3, "vy_sign"), (4, "shape"), (5, "color")] {
        agent.encoder.declare(r, n);
    }
    agent.encoder.set_range(6, "vx", -3.0, 3.0);
    agent.encoder.set_range(7, "vy", -3.0, 3.0);

    let mut events: Vec<Event> = Vec::new();

    // ---------- Phase 1: 관찰 ----------
    // 빨간 원 하나. 5회 이상의 충돌을 보여준다(개발계획 사양). 속도·크기는 자연 변주.
    println!("\n[Phase 1] 관찰 — 빨간 원(circle/red), 벽 무늬 brick");
    let mut collisions = 0;
    for ep in 0..3 {
        collisions += observe_world(
            &mut agent,
            &mut events,
            Body { shape: 0, color: 0, size: 1.0 + ep as f32 * 0.5 },
            0,
            400,
            0.0,
            100 + ep as u64,
        );
    }
    println!("  관측: 틱 {}회, 충돌 {}회", events.len(), collisions);

    // 수면: 사건 기억에서 스키마를 추출한다
    let lib = if ablate_schema {
        SchemaLib::default()
    } else {
        induce(&events, InduceConfig::default())
    };
    dump(&lib, "Phase 1 수면 직후");
    let (slots_ok, offending) = causal_slots_ok(&lib);
    let p1_found = lib.schemas.iter().any(|s| s.effect != 0);
    println!(
        "\n  Phase 1 판정: 반사 스키마 생성 {} · 인과 슬롯 일치(contact/wall_side만) {}{}",
        if p1_found { "✅" } else { "❌" },
        if slots_ok { "✅" } else { "❌" },
        if offending.is_empty() {
            String::new()
        } else {
            format!(" (외양 슬롯 잔존: {offending:?})")
        }
    );

    // ---------- Phase 2: 신규 객체 전이 ----------
    println!("\n[Phase 2] 신규 객체 — 파란 삼각형(triangle/blue): 학습에 없던 모양·색");
    let p2 = predict_phase(&lib, Body { shape: 2, color: 1, size: 0.7 }, 0, 0.0, 40, 777);
    let p2_rate = p2.correct as f32 / p2.asked.max(1) as f32;
    println!("  충돌 직전 속도 예측: {}/{} = {:.1}%", p2.correct, p2.asked, p2_rate * 100.0);

    // ---------- Phase 3: 외관 불변 ----------
    println!("\n[Phase 3] 외관 변화 — 벽 무늬 brick→glassy, 물체 노란 별(star/yellow)");
    let p3 = predict_phase(&lib, Body { shape: 3, color: 3, size: 1.8 }, 3, 0.0, 40, 888);
    let p3_rate = p3.correct as f32 / p3.asked.max(1) as f32;
    println!("  충돌 직전 속도 예측: {}/{} = {:.1}%", p3.correct, p3.asked, p3_rate * 100.0);

    // ---------- Phase 4: 규칙 개정 (M1 범위 — 정보용) ----------
    println!("\n[Phase 4·정보용/M1] 중력 추가 후 스키마 개정");
    let mut ev4 = events.clone();
    for ep in 0..2 {
        observe_world(
            &mut agent,
            &mut ev4,
            Body { shape: 0, color: 0, size: 1.0 },
            0,
            400,
            1.2,
            300 + ep as u64,
        );
    }
    let lib4 = if ablate_schema { SchemaLib::default() } else { induce(&ev4, InduceConfig::default()) };
    let p4 = predict_phase(&lib4, Body { shape: 1, color: 4, size: 1.0 }, 1, 1.2, 40, 999);
    let p4_rate = p4.correct as f32 / p4.asked.max(1) as f32;
    println!("  중력 하 신규 객체 예측: {}/{} = {:.1}%", p4.correct, p4.asked, p4_rate * 100.0);

    // ---------- 그래프 계측(유리상자) ----------
    println!("\n  세계 그래프: 지각 {} · 상태 {} · 간선 {} · 메모리 ~{:.1}MB",
        agent.graph.n_percepts(),
        agent.graph.n_nodes(),
        agent.graph.n_edges(),
        agent.graph.memory_estimate() as f64 / 1e6
    );

    // ---------- 판정 ----------
    println!("\n=========================================================================");
    println!("판정 (M0 관문 = Phase 1~3)");
    println!("=========================================================================");
    let pass1 = p1_found && slots_ok;
    let pass2 = p2_rate >= 0.90;
    let pass3 = p3_rate >= 0.90;
    println!("  Phase 1 (관찰→스키마):        {}", if pass1 { "통과" } else { "실패" });
    println!("  Phase 2 (신규 객체 전이):      {} ({:.1}%)", if pass2 { "통과" } else { "실패 — M0 킬 기준" }, p2_rate * 100.0);
    println!("  Phase 3 (외관 불변):          {} ({:.1}%)", if pass3 { "통과" } else { "실패" }, p3_rate * 100.0);
    println!("  Phase 4 (규칙 개정, M1 참고):  {:.1}%", p4_rate * 100.0);
    if ablate_schema {
        println!("\n  [어블레이션 모드 --no-schema] 스키마 층 제거 시 위 수치가 붕괴하는 것이 정상.");
    }
    let all = pass1 && pass2 && pass3;
    println!("\n  ▶ Bounce Test Phase 1~3: {}", if all { "✅ 전체 통과" } else { "❌ 미통과" });
    std::process::exit(if all { 0 } else { 1 });
}
