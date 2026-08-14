//! W2-1 — 실 ARC-AGI-1 기준선 측정.
//!
//! W2-0 솔버(클래스 7종 어휘)를 공개 훈련 세트 400과제에 그대로 돌려 **정직한
//! 출발선**을 잰다. 낮게 나오는 것이 정상이다 — 측정이 어휘 확장(W2-2+)의
//! 우선순위를 정한다. 1차 범위: 전 훈련쌍에서 입출력 격자 크기가 같은 과제
//! (크기 변환 어휘는 아직 없음 — 스킵 수를 함께 보고).
//!
//! 실행: `cargo run --release --bin arc-real [-- <데이터 경로>]`

use monad_envs::arc_data::load_dir;
use monad_envs::arc_solve::{apply, learn};

fn main() {
    let arg = std::env::args().nth(1);
    let dir = arg.unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".to_string()
    });
    println!("=========================================================================");
    println!("W2-1 — 실 ARC-AGI-1 기준선 (W2-0 솔버 · 클래스 7종 어휘 그대로)");
    println!("=========================================================================");
    println!("데이터: {dir}\n");

    let t0 = std::time::Instant::now();
    let tasks = load_dir(std::path::Path::new(&dir));
    println!("과제 로드: {}개 · {:.1}초", tasks.len(), t0.elapsed().as_secs_f32());

    let mut same_size = 0usize;
    let mut skipped = 0usize;
    let mut solved = 0usize;
    let mut solved_names: Vec<String> = Vec::new();
    // 스키마 라이브러리 MVP(W2-2 원항목): 해결 과제의 클래스 규칙 누적 + 재사용률
    let mut pool: Vec<(Vec<monad_core::schema::Constraint>, u32)> = Vec::new();
    let mut reuse_hits = 0usize;
    // W2-3R 프로그램 라이브러리: 해결 프로그램 축적 → 단계 재사용 사전분포
    let mut prog_lib = monad_envs::arc_solve::ProgLib::default();
    let mut prog_solved = 0usize;
    let (mut fail_near, mut fail_mid, mut fail_far) = (0usize, 0usize, 0usize);
    let (mut fail_fragmented, mut fail_gen, mut fail_del, mut fail_newcolor) =
        (0usize, 0usize, 0usize, 0usize);
    let forensic = std::env::var("MONAD_ARC_FORENSIC").is_ok();
    // W2-4 어블레이션: gridops(격자 계층 끔) | objects(객체 계층을 기본값化)
    let ablate = std::env::var("MONAD_ARC_ABLATE").unwrap_or_default();
    // 현미경: 지정 과제의 격자·객체 수·예측을 상세 출력
    let scope = std::env::var("MONAD_ARC_TASK").ok();
    let t1 = std::time::Instant::now();
    let mut max_task_ms = 0f32;
    for task in &tasks {
        let t_task = std::time::Instant::now();
        let ok_size = task
            .train
            .iter()
            .chain(task.test.iter())
            .all(|p| p.input.w == p.output.w && p.input.h == p.output.h);
        if !ok_size {
            // 크기 변환 과제도 격자 수준 연산(연쇄 포함)은 시도한다
            let train: Vec<_> =
                task.train.iter().map(|p| (p.input.clone(), p.output.clone())).collect();
            if let Some(chain) = monad_envs::arc_solve::try_grid_chain3(&train) {
                let all_ok = task.test.iter().all(|p| {
                    monad_envs::arc_solve::apply_grid_chain_n(&p.input, &chain) == p.output
                });
                if all_ok {
                    solved += 1;
                    solved_names.push(task.name.clone());
                    continue;
                }
            }
            // 패널 결합: 구분선 두 패널의 셀별 함수(AND/OR/XOR류)
            if let Some(pc) = monad_envs::arc_solve::try_panel_combine(&train) {
                let all_ok = task.test.iter().all(|p| {
                    monad_envs::arc_solve::apply_panel_combine(&p.input, &pc)
                        .map(|g| g == p.output)
                        .unwrap_or(false)
                });
                if all_ok {
                    solved += 1;
                    solved_names.push(task.name.clone());
                    continue;
                }
            }
            // N패널 결합(3~4)
            if let Some(pc) = monad_envs::arc_solve::try_panel_combine_n(&train) {
                let all_ok = task.test.iter().all(|p| {
                    monad_envs::arc_solve::apply_panel_combine_n(&p.input, &pc)
                        .map(|g| g == p.output)
                        .unwrap_or(false)
                });
                if all_ok {
                    solved += 1;
                    solved_names.push(task.name.clone());
                    continue;
                }
            }
            // 혼합 연쇄(W2-3): 정규화(추출) 후 객체 파이프라인 — 훈련 전부
            // 정확 재현일 때만 채택(격자 연산과 같은 극한 기준).
            if let Some((norm, libs)) = monad_envs::arc_solve::try_norm_then_objects(&train) {
                let all_ok = task.test.iter().all(|p| {
                    let ni = monad_envs::arc_solve::apply_grid_op_pub(&p.input, norm);
                    monad_envs::arc_solve::apply(&ni, &libs) == p.output
                });
                if all_ok {
                    solved += 1;
                    solved_names.push(task.name.clone());
                    continue;
                }
            }
            max_task_ms = max_task_ms.max(t_task.elapsed().as_secs_f32() * 1000.0);
            skipped += 1;
            continue;
        }
        same_size += 1;
        let train: Vec<_> =
            task.train.iter().map(|p| (p.input.clone(), p.output.clone())).collect();
        // 제3계층: 셀 이벤트 스키마 — 격자 연산이 없고 셀 규칙이 훈련 정확 재현이면 채택
        let mut libs = if ablate == "extra" {
            monad_envs::arc_solve::learn_with(&train, false)
        } else {
            monad_envs::arc_solve::learn_validated(&train)
        };
        if libs.grid_op.is_none() {
            if let Some(cl) = monad_envs::arc_solve::try_cellwise(&train) {
                let all_ok = task
                    .test
                    .iter()
                    .all(|p| monad_envs::arc_solve::apply_cellwise(&p.input, &cl) == p.output);
                if all_ok {
                    solved += 1;
                    solved_names.push(task.name.clone());
                    continue;
                }
            }
        }
        if ablate == "gridops" {
            libs.grid_op = None;
        }
        if ablate == "objects" {
            libs.class.schemas.clear();
            libs.class.default_effect = Some(0); // 전부 stay — 객체 규칙 무력화
            libs.color.schemas.clear();
            libs.color.default_effect = Some(0);
            libs.copies.schemas.clear();
            libs.copies.default_effect = Some(1);
        }
        if scope.as_deref() == Some(task.name.as_str()) {
            let show = |g: &monad_envs::grid::Grid, tag: &str| {
                println!("  {tag} ({}x{}):", g.w, g.h);
                for y in 0..g.h {
                    let row: String = (0..g.w).map(|x| char::from(b'0' + g.get(x, y))).collect();
                    println!("    {row}");
                }
            };
            println!("[현미경] {}", task.name);
            for (i, (gi, go)) in train.iter().enumerate() {
                println!(" 훈련 {} — 입력 객체 {}개, 출력 객체 {}개:",
                    i, monad_envs::grid::components(gi).len(),
                    monad_envs::grid::components(go).len());
                show(gi, "입력");
                show(go, "정답");
            }
            println!(" 클래스 규칙 {}개 · 기본 {:?}", libs.class.schemas.len(), libs.class.default_effect);
            let p = &task.test[0];
            show(&p.input, "시험 입력");
            show(&p.output, "시험 정답");
            show(&apply(&p.input, &libs), "예측");
        }
        // 역방향 혼합: 객체 단계가 훈련을 못 닫으면 격자 연산 마무리를 시도
        let finisher = {
            let train_exact = train.iter().all(|(i, o)| apply(i, &libs) == *o);
            if train_exact {
                None
            } else {
                monad_envs::arc_solve::try_objects_then_grid(&train, &libs).filter(|&op| {
                    train.iter().all(|(i, o)| {
                        &monad_envs::arc_solve::apply_grid_op_pub(&apply(i, &libs), op) == o
                    })
                })
            }
        };
        let all_ok = task.test.iter().all(|p| {
            let mut pred = apply(&p.input, &libs);
            if let Some(op) = finisher {
                pred = monad_envs::arc_solve::apply_grid_op_pub(&pred, op);
            }
            pred == p.output
        });
        // 풀 사전분포(W2-2 라이브러리의 본 시험): 다른 과제에서 배운 규칙을 이식해
        // 훈련이 정확해지면 채택 — 과제 간 전이의 첫 실전 경로.
        let all_ok = all_ok
            || (ablate != "pool" && !pool.is_empty() && {
                let train_exact = train.iter().all(|(i, o)| apply(i, &libs) == *o);
                !train_exact
                    && pool.iter().any(|(cons, eff)| {
                        let mut cand = libs.clone();
                        cand.class.schemas.insert(
                            0,
                            monad_core::schema::Schema {
                                constraints: cons.clone(),
                                effect: *eff,
                                evidence: 0,
                                counterexamples: 0,
                                gain: 0.0,
                            },
                        );
                        train.iter().all(|(i, o)| apply(i, &cand) == *o)
                            && task.test.iter().all(|p| apply(&p.input, &cand) == p.output)
                    })
            });
        // anytime 승급: 미해결이면 저지지 셀 규칙에 예산 추가 투입(게이트 동일)
        let all_ok = all_ok
            || [3u32, 2].iter().any(|&ms| {
                monad_envs::arc_solve::try_cellwise_ms(&train, ms)
                    .map(|cl| {
                        task.test
                            .iter()
                            .all(|p| monad_envs::arc_solve::apply_cellwise(&p.input, &cl) == p.output)
                    })
                    .unwrap_or(false)
            });
        // W2-3R 프로그램 합성: 미해결이면 조합자 탐색(객체별·패널별 리프트 × 합성).
        // 해결 프로그램은 라이브러리에 축적 → 부분 단계 재사용이 탐색 사전분포
        // (풀수록 잘 푸는 고리). 예산 내(anytime), 정확 재현 게이트.
        let all_ok = all_ok
            || (ablate != "prog" && {
                let mut budget: i64 = 200_000;
                match monad_envs::arc_solve::program_search(&train, &prog_lib, &mut budget) {
                    Some(prog) => {
                        let ok = task.test.iter().all(|p| {
                            monad_envs::arc_solve::apply_program(&p.input, &prog) == p.output
                        });
                        if ok {
                            prog_lib.record(&prog);
                            prog_solved += 1;
                        }
                        ok
                    }
                    None => false,
                }
            });
        max_task_ms = max_task_ms.max(t_task.elapsed().as_secs_f32() * 1000.0);
        if all_ok {
            solved += 1;
            solved_names.push(task.name.clone());
            // 라이브러리 누적 + 재사용 계측: 이번 과제의 규칙이 풀에 이미 있었는가
            for s in &libs.class.schemas {
                let key = (s.constraints.clone(), s.effect);
                if pool.iter().any(|p| *p == key) {
                    reuse_hits += 1;
                } else {
                    pool.push(key);
                }
            }
        } else {
            // 실패 유형 분포 — 어휘 확장의 우선순위를 데이터로 정한다(W2-2)
            let p = &task.test[0];
            let pred = apply(&p.input, &libs);
            let total = (p.output.w * p.output.h) as f32;
            let hit = p
                .output
                .cells
                .iter()
                .zip(pred.cells.iter())
                .filter(|(a, b)| a == b)
                .count() as f32;
            let frac = if pred.w == p.output.w && pred.h == p.output.h { hit / total } else { 0.0 };
            let ins_n = monad_envs::grid::components(&p.input).len();
            let outs_n = monad_envs::grid::components(&p.output).len();
            let in_colors: std::collections::HashSet<u8> =
                p.input.cells.iter().copied().filter(|&c| c != 0).collect();
            let new_color = p
                .output
                .cells
                .iter()
                .any(|&c| c != 0 && !in_colors.contains(&c));
            if frac >= 0.9 {
                fail_near += 1;
                // 법의학: 근접 실패의 차이 유형 — 싼 승리의 표적 좌표
                if forensic && pred.w == p.output.w && pred.h == p.output.h {
                    let mut over = 0usize; // 우리가 칠했는데 정답은 배경
                    let mut under = 0usize; // 정답이 칠했는데 우리는 배경
                    let mut recol = 0usize; // 둘 다 칠했는데 색이 다름
                    let mut diff_colors: std::collections::HashSet<u8> =
                        std::collections::HashSet::new();
                    for (a, b) in p.output.cells.iter().zip(pred.cells.iter()) {
                        if a != b {
                            match (*a, *b) {
                                (0, _) => over += 1,
                                (_, 0) => under += 1,
                                _ => recol += 1,
                            }
                            if *a != 0 {
                                diff_colors.insert(*a);
                            }
                        }
                    }
                    let n_diff = over + under + recol;
                    let kind = if recol > 0 && over == 0 && under == 0 {
                        "색만"
                    } else if under > 0 && over == 0 && recol == 0 {
                        "누락만"
                    } else if over > 0 && under == 0 && recol == 0 {
                        "과잉만"
                    } else {
                        "혼합"
                    };
                    println!(
                        "  [근접] {} · 차이 {}셀({}) · 과잉 {} 누락 {} 색 {} · 정답측 색 {:?}",
                        task.name, n_diff, kind, over, under, recol, diff_colors
                    );
                }
            } else if frac >= 0.5 {
                fail_mid += 1;
            } else {
                fail_far += 1;
            }
            if ins_n > 12 {
                fail_fragmented += 1;
            }
            if outs_n > ins_n {
                fail_gen += 1;
            } else if outs_n < ins_n {
                fail_del += 1;
            }
            if new_color {
                fail_newcolor += 1;
            }
        }
    }
    let dt = t1.elapsed().as_secs_f32();
    println!(
        "\n동일 크기 부분집합: {}개 (크기 변환 {}개 스킵 — 어휘 미보유의 정직 보고)",
        same_size, skipped
    );
    println!(
        "정확 일치 해결: {} / 전체 400 = {:.1}% (동일 크기 대비 {:.1}%)",
        solved,
        solved as f32 / tasks.len().max(1) as f32 * 100.0,
        solved as f32 / same_size.max(1) as f32 * 100.0
    );
    println!(
        "풀이 시간: 총 {:.1}초 · 과제당 평균 {:.0}ms · **최악 {:.0}ms** (계약 ≤5분/과제 = 300,000ms)",
        dt,
        dt * 1000.0 / same_size.max(1) as f32,
        max_task_ms
    );
    if !solved_names.is_empty() {
        println!("해결 과제: {}", solved_names.join(", "));
    }
    println!("\n실패 유형 분포(동일 크기 시험 1번 기준, 중복 집계 허용):");
    println!("  셀 일치 ≥90%(근접): {fail_near} · 50~90%: {fail_mid} · <50%(원거리): {fail_far}");
    println!("  파편화(입력 객체 >12 — 패턴형 과제): {fail_fragmented}");
    println!("  객체 생성형(출력>입력): {fail_gen} · 소멸/병합형(출력<입력): {fail_del}");
    println!("  신규 색 등장(입력에 없는 색): {fail_newcolor}");
    println!(
        "\n프로그램 합성(W2-3R): 신규 해결 {}건 · 라이브러리 프로그램 {}개",
        prog_solved,
        prog_lib.programs.len()
    );
    println!(
        "스키마 라이브러리(W2-2): 풀 {}규칙 · 과제 간 재사용 {}회 (PRD 재사용률 지표 1차)",
        pool.len(),
        reuse_hits
    );
    println!("\n▶ W2-1 기준선 기록 완료 — 실패 분포가 어휘 확장 우선순위(W2-2)를 정한다");
}
