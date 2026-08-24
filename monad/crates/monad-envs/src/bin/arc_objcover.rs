//! M2-R 진단 — **객체 규칙 전이 실패의 분해**(집계 전용).
//!
//! Day 11에서 객체 규칙 게이트 통과가 0이었다. 원인 후보를 가른다:
//!
//! | 병목 | 서명 | 처방 |
//! |---|---|---|
//! | **행동 어휘**(재색·삭제만) | 시도 가능 과제 자체가 거의 없다 | 이동·출현 확장 |
//! | **경험 폭**(출처 10과제) | 시도 가능한데 일관 규칙이 없다 | 경험 확대 |
//! | **성질 어휘** | 일관 규칙은 있는데 덮개가 안 된다 | 성질 확장 |
//! | **선택** | 덮는데 재현이 안 된다 | 선택 규율 |
//!
//! 실행: `arc-objcover`

use monad_core::abstraction::Library;
use monad_envs::arc_data::load_dir;
use monad_envs::arc_objrule::{
    actual_deltas, apply_obj_rules, obj_rules_reproduce, rule_covers, select_obj_consistent,
    task_props,
};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_OBJLIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objlib.tsv".into());
    let src_path = std::env::var("MONAD_ARC_OBJSRC").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objsource.txt".into()
    });
    let solved_path = std::env::var("MONAD_ARC_SOLVED")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into());

    println!("=========================================================================");
    println!("M2-R 객체 규칙 전이 실패 분해 — 행동 어휘/경험 폭/성질 어휘/선택 (집계 전용)");
    println!("=========================================================================");

    let lib = Library::load(&lib_path).unwrap_or_default();
    let sources: Vec<String> = std::fs::read_to_string(&src_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    let solved: Vec<String> = std::fs::read_to_string(&solved_path)
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    // 고정 홀드아웃(성장 곡선용): 지정 인덱스 이후만 평가 — 경험 풀이 자라도
    // 시험 집합이 불변이어야 "경험 ↑ → 미접촉 해결 ↑" 곡선이 성립한다.
    let holdout_from: usize = std::env::var("MONAD_ARC_HOLDOUT_FROM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let tasks = load_dir(std::path::Path::new(&dir));
    let holdout: Vec<_> = tasks
        .into_iter()
        .enumerate()
        .filter(|(ix, t)| {
            *ix >= holdout_from && !sources.contains(&t.name) && !solved.contains(&t.name)
        })
        .map(|(_, t)| t)
        .collect();

    let mut n_attemptable = 0usize; // 모든 훈련쌍이 재색·삭제·유지로 완전 기술
    let mut n_sel_nonempty = 0usize;
    let mut n_reproduce = 0usize;
    let mut n_test_ok = 0usize;
    let mut changed_objs = 0usize; // 시도 가능 과제의 바뀐 객체 수
    let mut sel_lens = 0usize;
    // 병목 해부(시도 170): 성질 어휘의 정보 부족 vs 경험 부족을 가른다
    let mut ambiguous_pairs = 0usize; // 성질 동일·행동 상이 객체쌍(성질로 원리상 구분 불가)
    let mut n_ambiguous_tasks = 0usize;
    let mut uncovered_changed = 0usize; // 바뀐 객체 중 어떤 일관 규칙도 발화하지 않음

    for task in &holdout {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let deltas: Option<Vec<_>> =
            train.iter().map(|(i, o)| actual_deltas(i, o)).collect();
        let Some(deltas) = deltas else { continue };
        n_attemptable += 1;
        changed_objs += deltas
            .iter()
            .flat_map(|v| v.iter())
            .filter(|d| d.is_some())
            .count();

        // 성질 동일·행동 상이 쌍 = 현재 성질 12종으로는 원리상 구분 불가한 지점
        let sites = task_props(&train);
        let mut amb_here = 0usize;
        for a in 0..sites.len() {
            for b in a + 1..sites.len() {
                if sites[a].0 == sites[b].0 && sites[a].1 != sites[b].1 {
                    amb_here += 1;
                }
            }
        }
        ambiguous_pairs += amb_here;
        if amb_here > 0 {
            n_ambiguous_tasks += 1;
        }

        let sel = select_obj_consistent(&lib, &train);
        // 바뀐 객체 중 일관 규칙이 하나도 발화하지 않는 것(경험/성질의 구멍)
        uncovered_changed += sites
            .iter()
            .filter(|(p, d)| d.is_some() && !sel.iter().any(|r| rule_covers(r, p)))
            .count();
        if sel.is_empty() {
            continue;
        }
        n_sel_nonempty += 1;
        sel_lens += sel.len();
        if !obj_rules_reproduce(&sel, &train) {
            continue;
        }
        n_reproduce += 1;
        if task
            .test
            .iter()
            .all(|p| apply_obj_rules(&sel, &p.input) == p.output)
        {
            n_test_ok += 1;
        }
    }

    println!("규칙 {}개 · 출처 {}과제 제외 · 홀드아웃 {}건\n", lib.entries.len(), sources.len(), holdout.len());
    println!("  ① 시도 가능(재색·삭제·유지로 완전 기술): {}건 · 바뀐 객체 총 {}개",
        n_attemptable, changed_objs);
    println!("  ② 그중 일관 규칙 존재: {}건 (평균 선택 {}개)",
        n_sel_nonempty,
        if n_sel_nonempty > 0 { sel_lens / n_sel_nonempty } else { 0 });
    println!("  ③ 훈련 재현 통과: {}건", n_reproduce);
    println!("  ④ 시험까지 정확: {}건", n_test_ok);
    println!(
        "\n  🔬 병목 해부: 성질 동일·행동 상이 쌍 {}개({}과제) — 성질 어휘로 원리상 구분 불가",
        ambiguous_pairs, n_ambiguous_tasks
    );
    println!(
        "     일관 규칙 미발화 바뀐 객체 {}개/{} — 경험·성질의 구멍",
        uncovered_changed, changed_objs
    );

    println!("\n▶ 판정:");
    if n_attemptable < 5 {
        println!("  **행동 어휘 병목** — 재색·삭제만으로 완전 기술되는 홀드아웃 과제가 {}건뿐.", n_attemptable);
        println!("  처방: 델타 어휘 확장(이동·출현) 또는 부분 적용(전량 기술 요구 완화).");
    } else if n_sel_nonempty * 2 < n_attemptable {
        println!("  **경험/성질 병목** — 시도 가능 {}건 중 일관 규칙이 있는 것은 {}건.",
            n_attemptable, n_sel_nonempty);
        println!("  처방: 출처 경험 확대(현재 10과제) 또는 성질 어휘 확장.");
    } else if n_reproduce * 2 < n_sel_nonempty {
        println!("  **덮개 병목** — 일관 규칙은 있으나 훈련 재현까지 가는 것이 적다.");
    } else {
        println!("  **일반화 병목** — 재현은 되는데 시험에서 틀린다(과적합).");
    }
}
