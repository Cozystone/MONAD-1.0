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
    actual_deltas, apply_obj_rules, obj_rules_reproduce, raw_correct_rule_exists, rule_covers,
    appearance_stats, describe_failure, select_obj_consistent, select_obj_cover, task_props, DescribeFail,
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
    let mut filtered_out = 0usize;   // 정답 규칙이 있었으나 일관성 필터에 걸림
    let mut no_rule_at_all = 0usize; // 정답 행동 규칙이 애초에 없음
    // 결정 목록 선택(시도 177)과의 나란한 비교
    let (mut dl_nonempty, mut dl_reproduce, mut dl_test_ok) = (0usize, 0usize, 0usize);
    // 결정 목록의 **한계 기여**: 단독 선택이 재현 못한 과제를 구해내는가
    let (mut dl_rescue, mut dl_rescue_test) = (0usize, 0usize);
    // ① 탈락 사유(시도 178): 상한이 왜 17인가
    let (mut f_size, mut f_in, mut f_out, mut f_both) = (0usize, 0usize, 0usize, 0usize);
    // **원리상 상한**(시도 187): 라이브러리가 무엇을 담든, 같은 과제 안에 성질이
    // 같은데 행동이 다른 객체가 있으면 그 객체는 **어떤 성질 패턴으로도** 옳게
    // 덮을 수 없다. 조건 언어 9가지와 인가 지점 3.5배에도 불변이던 147의 정체가
    // 이것인지 잰다 — 그렇다면 상한은 라이브러리가 아니라 표적 과제의 성질이다.
    let mut inprinciple_blocked = 0usize;
    // GEN3 관계 계층의 깔때기와 **관계 표현의 원리상 상한**(시도 194).
    // 관계 서명(상대들의 성질 집합)까지 포함해도 구별되지 않는 객체가 몇 개나
    // 남는가 — 표현 교체의 실효를 속성 상한(122)과 직접 비교하는 유일한 방법.
    let rel_lib = Library::load(
        std::env::var("MONAD_ARC_RELLIB")
            .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-rellib.tsv".into()),
    )
    .unwrap_or_default();
    let (mut rel_sel_nonempty, mut rel_reproduce, mut rel_blocked) = (0usize, 0usize, 0usize);
    let (mut rel_uncovered, mut rel_norule, mut rel_filtered) = (0usize, 0usize, 0usize);
    // 짝 없는 출력의 성질(시도 179): 복제로 기술 가능한가, 진짜 출현인가
    let (mut ap_total, mut ap_sc, mut ap_s, mut ap_novel) = (0usize, 0usize, 0usize, 0usize);
    let mut ap_tasks_copyable = 0usize;

    for task in &holdout {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let deltas: Option<Vec<_>> =
            train.iter().map(|(i, o)| actual_deltas(i, o)).collect();
        let Some(deltas) = deltas else {
            // 첫 실패 쌍의 사유를 집계한다(과제 단위 대표값)
            if let Some(why) = train.iter().find_map(|(i, o)| describe_failure(i, o)) {
                match why {
                    DescribeFail::SizeMismatch => f_size += 1,
                    DescribeFail::UnmatchedInput => f_in += 1,
                    DescribeFail::UnmatchedOutput => f_out += 1,
                    DescribeFail::Both => f_both += 1,
                }
                // 출현·복제 계열이면 그 성질을 센다
                if matches!(why, DescribeFail::UnmatchedOutput | DescribeFail::Both) {
                    let mut t = (0usize, 0usize, 0usize, 0usize);
                    for (i, o) in &train {
                        let a = appearance_stats(i, o);
                        t.0 += a.unmatched_out;
                        t.1 += a.same_shape_color;
                        t.2 += a.same_shape_only;
                        t.3 += a.novel;
                    }
                    ap_total += t.0;
                    ap_sc += t.1;
                    ap_s += t.2;
                    ap_novel += t.3;
                    // 모든 짝 없는 출력이 입력 원본을 가진 과제 = 복제만으로 기술 가능
                    if t.0 > 0 && t.3 == 0 {
                        ap_tasks_copyable += 1;
                    }
                }
            }
            continue;
        };
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
                if sites[a].props == sites[b].props && sites[a].delta != sites[b].delta {
                    amb_here += 1;
                }
            }
        }
        ambiguous_pairs += amb_here;
        if amb_here > 0 {
            n_ambiguous_tasks += 1;
        }

        // 원리상 덮을 수 없는 바뀐 객체(성질 동일·행동 상이가 같은 과제에 존재)
        for (a, sa) in sites.iter().enumerate() {
            if sa.delta.is_none() && sa.copies.is_empty() {
                continue;
            }
            let blocked = sites.iter().enumerate().any(|(b, sb)| {
                b != a && sb.props == sa.props && (sb.delta != sa.delta || sb.copies != sa.copies)
            });
            if blocked {
                inprinciple_blocked += 1;
            }
        }
        // GEN3: 관계 서명까지 포함한 원리상 상한 + 선택/재현 깔때기
        {
            let rs = monad_envs::arc_relrule::task_rsites(&train);
            for (a, sa) in rs.iter().enumerate() {
                if sa.delta.is_none() {
                    continue;
                }
                let blocked = rs.iter().enumerate().any(|(b, sb)| {
                    b != a
                        && sb.props == sa.props
                        && sb.targets == sa.targets
                        && sb.delta != sa.delta
                });
                if blocked {
                    rel_blocked += 1;
                }
            }
            let rsel = monad_envs::arc_relrule::select_rel_consistent(&rel_lib, &train);
            // GEN2에서 이해를 열었던 원인 분해를 GEN3에도 적용한다
            for site in rs.iter().filter(|s| s.delta.is_some()) {
                if rsel
                    .iter()
                    .any(|r| monad_envs::arc_relrule::rel_rule_covers(r, site))
                {
                    continue;
                }
                rel_uncovered += 1;
                if monad_envs::arc_relrule::rel_raw_correct_exists(&rel_lib, site) {
                    rel_filtered += 1;
                } else {
                    rel_norule += 1;
                }
            }
            if !rsel.is_empty() {
                rel_sel_nonempty += 1;
                if monad_envs::arc_relrule::rel_rules_reproduce(&rsel, &train) {
                    rel_reproduce += 1;
                }
            }
        }
        let sel = select_obj_consistent(&lib, &train);
        // 바뀐 객체 중 일관 규칙이 하나도 발화하지 않는 것(경험/성질의 구멍)
        for site in sites.iter().filter(|s| s.delta.is_some()) {
            if sel.iter().any(|r| rule_covers(r, &site.props)) {
                continue;
            }
            uncovered_changed += 1;
            // 필터 이전에는 정답 행동 규칙이 있었는가 — 두 원인을 가른다
            if raw_correct_rule_exists(&lib, site) {
                filtered_out += 1;
            } else {
                no_rule_at_all += 1;
            }
        }
        // 결정 목록 선택 — 예외 우선 + 일반 후속(가림)을 허용
        let solo_reproduces = obj_rules_reproduce(&sel, &train);
        let dl = select_obj_cover(&lib, &train, 24);
        if !dl.is_empty() {
            dl_nonempty += 1;
            if obj_rules_reproduce(&dl, &train) {
                dl_reproduce += 1;
                let dl_test = task.test.iter().all(|p| apply_obj_rules(&dl, &p.input) == p.output);
                if dl_test {
                    dl_test_ok += 1;
                }
                if !solo_reproduces {
                    dl_rescue += 1;
                    if dl_test {
                        dl_rescue_test += 1;
                    }
                }
            }
        }
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
        "     일관 규칙 미발화 바뀐 객체 {}개/{}",
        uncovered_changed, changed_objs
    );
    println!(
        "     └ 원인 분해: 정답 규칙 **부재** {}개(경험 구멍) · 있었으나 **필터에 걸림** {}개(성질 판별력)",
        no_rule_at_all, filtered_out
    );
    println!(
        "     ★ **원리상 차단**(같은 과제에 성질 동일·행동 상이 객체 존재): {}개/{} — 라이브러리와 무관한 상한",
        inprinciple_blocked, changed_objs
    );

    println!(
        "\n  🪜 결정 목록 선택(시도 177): 목록 존재 {}건 · 훈련 재현 {}건 · **시험까지 정확 {}건**",
        dl_nonempty, dl_reproduce, dl_test_ok
    );
    println!(
        "     └ 한계 기여(단독 선택이 재현 못한 것만): 구제 {}건 · 그중 시험 정확 {}건",
        dl_rescue, dl_rescue_test
    );
    println!(
        "
  🧱 ① 상한의 정체 — 완전 기술 실패 사유: 크기 불일치 {}건 · 짝 없는 **입력** {}건(부분 변형) · 짝 없는 **출력** {}건(출현·복제) · 양쪽 {}건",
        f_size, f_in, f_out, f_both
    );
    println!(
        "     └ 짝 없는 출력 객체 {}개: 같은 모양·색 원본 있음 **{}개(복제)** · 같은 모양만 {}개 · 원본 없음 {}개(진짜 출현)",
        ap_total, ap_sc, ap_s, ap_novel
    );
    println!(
        "       → 진짜 출현이 하나도 없는(복제만으로 기술 가능한) 과제: **{}건**",
        ap_tasks_copyable
    );
    println!(
        "\n  🔷 GEN3 관계 계층: 규칙 {}개 · 일관 규칙 존재 {}건 · 훈련 재현 {}건",
        rel_lib.entries.len(),
        rel_sel_nonempty,
        rel_reproduce
    );
    println!(
        "     ★ **관계 표현의 원리상 차단**: {}개/{} (속성 표현은 {}개) — 표현 교체의 실효",
        rel_blocked, changed_objs, inprinciple_blocked
    );
    println!(
        "     └ GEN3 미발화 바뀐 객체 {}개: 정답 규칙 **부재** {}개 · **필터 탈락** {}개",
        rel_uncovered, rel_norule, rel_filtered
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
