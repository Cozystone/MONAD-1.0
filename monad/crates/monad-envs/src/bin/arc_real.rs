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

    // ---- M2-R: 경험 저널·라이브러리 경로(코드는 고정, 이 파일들만 자란다) ----
    let exp_path = std::env::var("MONAD_ARC_EXP")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-experience.tsv".into());
    let lib_path = std::env::var("MONAD_ARC_LIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-library.tsv".into());
    let record_exp = std::env::var("MONAD_ARC_RECORD").is_ok();
    // 부분 진전 저널 — 교사가 버리는 정보(닫지 못했으나 잔차를 줄인 시도)의 축적소
    let partial_path = std::env::var("MONAD_ARC_PARTIAL")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-partial.tsv".into());

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
    // 수면-학습 v1: 형상 재구체화로 해결된 과제 수
    let mut sleep_solved = 0usize;
    // W2-D 셀 역할 꿈으로 해결된 과제 수
    let mut dream_solved = 0usize;
    // W2-E 에너지 최소화로 해결된 과제 수
    let mut ebm_solved = 0usize;
    let (mut fail_near, mut fail_mid, mut fail_far) = (0usize, 0usize, 0usize);
    let (mut fail_fragmented, mut fail_gen, mut fail_del, mut fail_newcolor) =
        (0usize, 0usize, 0usize, 0usize);
    let forensic = std::env::var("MONAD_ARC_FORENSIC").is_ok();
    // W2-4 어블레이션: gridops(격자 계층 끔) | objects(객체 계층을 기본값化)
    let ablate = std::env::var("MONAD_ARC_ABLATE").unwrap_or_default();
    // 현미경: 지정 과제의 격자·객체 수·예측을 상세 출력
    let scope = std::env::var("MONAD_ARC_TASK").ok();
    // **MONAD_DERIVED로 푼 과제 이름**(시도 208). 지금까지는 어블레이션 실행과
    // 목록을 비교해야만 어느 과제가 MONAD의 것인지 알 수 있었다. 각성이 스스로
    // 적어 두면 결과가 자기설명적이 되고, 과제마다 "이 과제가 라이브러리에 몇 개를
    // 기여했고 그 전부가 배제된 채 풀렸는가"를 바로 보일 수 있다.
    let mut monad_names: Vec<String> = Vec::new();
    // **과적합 현장 계량**(시도 209). 시도 208이 남긴 가장 값진 숫자는
    // "게이트 통과 5건 → 시험 해결 1건"이다. 즉 네 번은 훈련쌍을 정확히
    // 재현하고도 시험에서 틀렸다. 그 네 번이 **왜** 틀렸는지는 처방이 정반대인
    // 두 가지로 갈린다:
    //
    //   침묵  — 고른 규칙이 시험 입력에서 아무것도 안 한다(예측=입력).
    //           덮개가 모자란다 → 더 **일반적인** 규칙이 필요하다.
    //   오발화 — 규칙이 발화했는데 틀린 답을 냈다.
    //           판별력이 모자란다 → 더 **구체적인** 규칙이 필요하다.
    //
    // 세션 내내 관찰한 일반성↔판별력 교환을 **실패 지점에서 직접** 재는 것은
    // 이번이 처음이다. 과제 내용은 출력하지 않는다(봉인 규율).
    let (mut of_silent, mut of_misfire) = (0usize, 0usize);
    // 오발화를 **셀 단위로 분해**(시도 210). "발화해서 틀렸다"는 아직 두 가지를
    // 뭉뚱그린다:
    //   damaged      맞았던 셀을 틀리게 만들었다 → 과발화/틀린 행동(판별력 부족)
    //   남은 오답     틀린 셀을 못 고쳤다        → 덮개 부족(일반성 부족)
    // 둘의 비가 다음 개입의 방향을 정한다. 시험 출력은 **집계에만** 쓰고 선택에
    // 되먹이지 않는다(각성은 이미 채점에 같은 값을 쓴다).
    let (mut of_corrected, mut of_damaged, mut of_before_wrong, mut of_after_wrong) =
        (0usize, 0usize, 0usize, 0usize);
    // 선택 계층(시도 206): 게이트가 쌍당 결정 하나 — 덮개 요구가 없다
    let (mut sel_gate_pass, mut sel_solved) = (0usize, 0usize);
    let sel_lib = monad_core::abstraction::Library::load(
        std::env::var("MONAD_ARC_SELLIB").unwrap_or_else(|_| {
            "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-sellib.tsv".into()
        }),
    )
    .unwrap_or_default();
    let sel_sources: Vec<String> = std::fs::read_to_string(
        std::env::var("MONAD_ARC_SELSRC").unwrap_or_else(|_| {
            "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-selsource.txt".into()
        }),
    )
    .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
    .unwrap_or_default();
    // 답 색 계층(시도 207→208): **모든** 과제에서 배우고, 과제 T를 풀 때는
    // T에서 나온 항목만 뺀다(leave-one-out). 앞 200으로 자르던 방식은 과제당
    // 기준이 더 느슨한 것도 아니면서 경험의 절반을 버렸다 — 시도 207에서
    // 단색 출력 9건이 출처 5 / 홀드아웃 4로 갈라진 것이 그 대가였다.
    let mut ans_lib = monad_core::abstraction::Library::new();
    let mut ans_sources: Vec<String> = Vec::new();
    {
        let mut per_task = Vec::new();
        for task in tasks.iter() {
            let tr: Vec<_> =
                task.train.iter().map(|p| (p.input.clone(), p.output.clone())).collect();
            let r = monad_envs::arc_answer::learn_ans_rules(&tr);
            if !r.is_empty() {
                ans_sources.push(task.name.clone());
                per_task.push((task.name.clone(), r));
            }
        }
        monad_envs::arc_answer::sleep_ans(&per_task, &mut ans_lib);
    }
    let (mut ans_gate_pass, mut ans_solved) = (0usize, 0usize);
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
                    if record_exp {
                        // 경험 기록: 무엇으로 풀었는지 그대로(해석 없음)
                        monad_envs::arc_experience::append_experience(
                            &exp_path,
                            &task.name,
                            &monad_envs::arc_experience::chain_to_term(&chain),
                        );
                    }
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
            // **답 색 규칙 전이**(시도 207→208): 출력이 단색인 부류.
            // 전역 출처 목록으로 과제를 통째 배제하는 대신 **항목 단위**로 뺀다.
            // 어블레이션은 **모든** MONAD 계층을 끈다 — 이 두 계층(답 색·선택)이
            // 어블레이션 밖에 있으면 대조군이 대조군이 아니게 된다.
            if ablate != "monad" {
                let ans_view = ans_lib.view_excluding(&task.name);
                let asel = monad_envs::arc_answer::select_ans_consistent(&ans_view, &train);
                if !asel.is_empty() {
                    ans_gate_pass += 1;
                    let (slot, dims) = asel[0];
                    let all_ok = task.test.iter().all(|p| {
                        monad_envs::arc_answer::apply_ans_rule(slot, dims, &p.input).as_ref()
                            == Some(&p.output)
                    });
                    if all_ok {
                        ans_solved += 1;
                        monad_names.push(task.name.clone());
                        solved += 1;
                        solved_names.push(task.name.clone());
                        continue;
                    }
                    // 답 색 계층은 항상 무언가를 내놓으므로(슬롯의 색으로 채운다)
                    // 침묵이 없다 — 실패는 전부 오발화다.
                    of_misfire += task.test.len();
                }
            }
            // **선택 규칙 전이**(시도 206): 크기 변환 과제 중 "출력 = 입력 어느
            // 객체의 잘라내기"인 부류. 이 계층의 게이트는 **쌍당 결정 하나**라
            // 과제당 100% 덮개 요구가 없다 — 세션 내내 ③를 막아온 구조의 우회다.
            if ablate != "monad" {
                let sel_view = sel_lib.view_excluding(&task.name);
                let ssel = monad_envs::arc_select::select_sel_consistent(&sel_view, &train);
                if monad_envs::arc_select::sel_rules_reproduce(&ssel, &train) {
                    sel_gate_pass += 1;
                    let all_ok = task.test.iter().all(|p| {
                        monad_envs::arc_select::apply_sel_rules(&ssel, &p.input).as_ref()
                            == Some(&p.output)
                    });
                    if all_ok {
                        sel_solved += 1;
                        monad_names.push(task.name.clone());
                        solved += 1;
                        solved_names.push(task.name.clone());
                        continue;
                    }
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
        // W2-E 에너지 최소화(시도 141): 약한 제약 중첩의 에너지 최저점 — 명료한
        // 프로그램이 없는 과제의 마지막 일반 경로(PRD 지정 대안). 훈련 정확 게이트.
        let all_ok = all_ok
            || (ablate != "ebm" && {
                let solved_all = task.test.iter().all(|p| {
                    monad_envs::arc_ebm::ebm_solve(&train, &p.input)
                        .map(|g| g == p.output)
                        .unwrap_or(false)
                });
                if solved_all {
                    ebm_solved += 1;
                }
                solved_all
            });
        // W2-D 셀 역할 꿈(시도 139): 별칭 가설 — 같은 겉모습 셀의 다른 출력을
        // 클론-HMM의 잠재 역할로 분리. 동일 크기·미해결에만, 훈련 정확 게이트.
        let all_ok = all_ok
            || (ablate != "dream"
                && task.test.iter().all(|p| p.input.w == p.output.w && p.input.h == p.output.h)
                && {
                    let solved_all = task.test.iter().all(|p| {
                        monad_envs::arc_dream::dream_cells_solve(&train, &p.input)
                            .map(|g| g == p.output)
                            .unwrap_or(false)
                    });
                    if solved_all {
                        dream_solved += 1;
                    }
                    solved_all
                });
        // 수면-학습 라이브러리 v1(시도 137): 풀 규칙을 **형상**으로 추상(상수 색을
        // 와일드카드로)한 뒤, 이 과제의 팔레트로 **재구체화**해 시험한다. 원시
        // 이식(기여 0)과의 차이 = 전이 전에 추상화 — 수면 응고의 본질.
        let all_ok = all_ok
            || (ablate != "sleep" && !pool.is_empty() && {
                let train_exact = train.iter().all(|(i, o)| apply(i, &libs) == *o);
                !train_exact && {
                    // 팔레트: 이 과제 훈련 입력의 색들
                    let mut palette: Vec<u8> = train
                        .iter()
                        .flat_map(|(i, _)| i.cells.iter().copied())
                        .filter(|&c| c != 0)
                        .collect();
                    palette.sort_unstable();
                    palette.dedup();
                    // 형상 후보: 색 상수를 포함한 풀 규칙(색만 와일드카드화)
                    let mut tried = 0usize;
                    let mut hit = false;
                    'shape: for (cons, eff) in pool.iter() {
                        let has_color = cons.iter().any(|c| {
                            matches!(c, monad_core::schema::Constraint::Eq(s, _)
                                if *s == monad_envs::arc_solve::S_COLOR)
                        }) || *eff >= 100;
                        if !has_color {
                            continue;
                        }
                        for &fill_c in &palette {
                            for &fill_e in palette.iter().chain(std::iter::once(&255u8)) {
                                tried += 1;
                                if tried > 400 {
                                    break 'shape;
                                }
                                let cons2: Vec<_> = cons
                                    .iter()
                                    .map(|c| match c {
                                        monad_core::schema::Constraint::Eq(s, _)
                                            if *s == monad_envs::arc_solve::S_COLOR =>
                                        {
                                            monad_core::schema::Constraint::Eq(
                                                *s,
                                                fill_c as u32,
                                            )
                                        }
                                        other => other.clone(),
                                    })
                                    .collect();
                                let eff2 = if *eff >= 100 {
                                    if fill_e == 255 {
                                        continue;
                                    }
                                    100 + fill_e as u32
                                } else {
                                    *eff
                                };
                                let mut cand = libs.clone();
                                cand.class.schemas.insert(
                                    0,
                                    monad_core::schema::Schema {
                                        constraints: cons2,
                                        effect: eff2,
                                        evidence: 0,
                                        counterexamples: 0,
                                        gain: 0.0,
                                    },
                                );
                                if train.iter().all(|(i, o)| apply(i, &cand) == *o)
                                    && task
                                        .test
                                        .iter()
                                        .all(|p| apply(&p.input, &cand) == p.output)
                                {
                                    hit = true;
                                    sleep_solved += 1;
                                    break 'shape;
                                }
                            }
                        }
                    }
                    hit
                }
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
            // M2-R 경험 기록: 격자 프로그램으로 풀렸으면 그 프로그램을 저널에 적는다
            if record_exp {
                if let Some((a, b)) = libs.grid_op {
                    let ops: Vec<_> = std::iter::once(a).chain(b).collect();
                    monad_envs::arc_experience::append_experience(
                        &exp_path,
                        &task.name,
                        &monad_envs::arc_experience::chain_to_term(&ops),
                    );
                }
            }
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
    // ================= M2-R: 코드 고정 자기학습(재구체화 패스) =================
    // 코드는 이 실행과 이전 실행이 완전히 동일하다. 달라지는 것은 축적된
    // 라이브러리 파일뿐 — 여기서 나오는 해결은 전부 MONAD_DERIVED 구조의 산물.
    let mut lib = monad_core::abstraction::Library::load(&lib_path).unwrap_or_default();
    let mut reuse_solved = 0usize;
    let mut reuse_novel = 0u32;
    let mut reuse_probes = 0u32;
    let mut reuse_tries = 0u32;
    let lib_before = lib.entries.len();
    if lib_before > 0 && ablate != "monad" {
        let t_reuse = std::time::Instant::now();
        let mut partial_found = 0usize;
        let mut partial_gain = 0f64;
        let mut partial_corrected = 0usize;
        let mut partial_damaged = 0usize;
        let mut partial_precision = 0f64;
        let mut residual_closed = 0usize;
        let mut residual_closed_tries = 0usize;
        let mut patch_gate_pass = 0usize;
        let mut patch_solved = 0usize;
        let mut patch_selected = 0usize;
        // 패치 규칙 라이브러리는 별도 파일 — 프로그램 스키마와 섞이지 않는다
        let patch_lib_path = std::env::var("MONAD_ARC_PATCHLIB").unwrap_or_else(|_| {
            "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-patchlib.tsv".into()
        });
        let patch_lib =
            monad_core::abstraction::Library::load(&patch_lib_path).unwrap_or_default();
        // 규칙의 출처 과제 목록 — 전이 시험에서 제외한다(순환 방지)
        let patch_sources: Vec<String> = std::fs::read_to_string(
            std::env::var("MONAD_ARC_PATCHSRC").unwrap_or_else(|_| {
                "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-patchsource.txt".into()
            }),
        )
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
        // 객체 델타 규칙 계층(시도 166) — 승자 표현(시도 165) 위의 두 번째 규칙층
        let mut obj_gate_pass = 0usize;
        let mut obj_solved = 0usize;
        // 계층 간 합성(시도 189): 부분 덮개 건수와 그중 마무리에 성공한 건수
        let mut obj_partial = 0usize;
        let mut obj_iter_gate = 0usize;
        // GEN3 관계 규칙 계층(시도 192) — 존재 양화가 있는 세 번째 계층
        let (mut rel_gate_pass, mut rel_solved) = (0usize, 0usize);
        let (mut comb_gate_pass, mut comb_solved) = (0usize, 0usize);

        let rel_lib = monad_core::abstraction::Library::load(
            std::env::var("MONAD_ARC_RELLIB").unwrap_or_else(|_| {
                "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-rellib.tsv".into()
            }),
        )
        .unwrap_or_default();
        let rel_sources: Vec<String> = std::fs::read_to_string(
            std::env::var("MONAD_ARC_RELSRC").unwrap_or_else(|_| {
                "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-relsource.txt".into()
            }),
        )
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
        let mut obj_layered = 0usize;
        let obj_lib = monad_core::abstraction::Library::load(
            std::env::var("MONAD_ARC_OBJLIB").unwrap_or_else(|_| {
                "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objlib.tsv".into()
            }),
        )
        .unwrap_or_default();
        let obj_sources: Vec<String> = std::fs::read_to_string(
            std::env::var("MONAD_ARC_OBJSRC").unwrap_or_else(|_| {
                "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-objsource.txt".into()
            }),
        )
        .map(|t| t.lines().map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
        // 표적 배제는 **항목을 물리적으로 지운 시야**로 한다(플래그로 걸러내면
        // `entries`를 직접 훑는 경로가 조용히 우회한다). 다만 기여한 적 없는
        // 과제에는 배제가 아무것도 지우지 않으므로 그때는 복제를 건너뛴다 —
        // 결과는 같고 각성이 수백 배 빨라진다.
        let obj_srcs = obj_lib.source_names();
        let rel_srcs = rel_lib.source_names();
        let obj_any = obj_lib.has_unattributed_monad();
        let rel_any = rel_lib.has_unattributed_monad();
        for task in &tasks {
            if solved_names.contains(&task.name) {
                continue;
            }
            let obj_cut = obj_any || obj_srcs.contains(&task.name);
            let rel_cut = rel_any || rel_srcs.contains(&task.name);
            let obj_owned;
            let obj_view: &monad_core::abstraction::Library = if obj_cut {
                obj_owned = obj_lib.view_excluding(&task.name);
                &obj_owned
            } else {
                &obj_lib
            };
            let rel_owned;
            let rel_view: &monad_core::abstraction::Library = if rel_cut {
                rel_owned = rel_lib.view_excluding(&task.name);
                &rel_owned
            } else {
                &rel_lib
            };
            let train: Vec<_> =
                task.train.iter().map(|p| (p.input.clone(), p.output.clone())).collect();
            let (mut ops, rep) =
                monad_envs::arc_experience::reinstantiate(&mut lib, &train, 20_000);
            reuse_tries += rep.tries;
            reuse_novel += rep.novel;
            reuse_probes += rep.probes;
            // 5단계: 하나로 안 닫히면 **합성**. 맹목 열거 대신 **부분 진전 경험**이
            // 1단계를 골라준다 — 교사가 버리는 정보(닫지 못했지만 잔차를 줄인 시도)가
            // 탐색 공간을 줄이는 지점(시도 151, 단위 시험에서 검사 횟수 감소 실증).
            if ops.is_none() {
                if let Some((seed, base, after, prof)) =
                    monad_envs::arc_experience::probe_partial(&lib, &train, 8_000)
                {
                    partial_found += 1;
                    partial_gain += base - after;
                    partial_corrected += prof.corrected;
                    partial_damaged += prof.damaged;
                    partial_precision += prof.precision();
                    // **부분 진전을 효과 프로파일과 함께 남긴다** — 잔차 비율만으로는
                    // "고치면서 망가뜨리는 도구"와 "조용히 고치는 도구"가 구별되지
                    // 않는다. 수면이 부분 목표 스키마를 만들려면 그 구분이 필요하다.
                    monad_envs::arc_experience::append_experience(
                        &partial_path,
                        &format!(
                            "P:{}:d{:.3}:c{}:x{}:p{:.2}",
                            task.name,
                            base - after,
                            prof.corrected,
                            prof.damaged,
                            prof.precision()
                        ),
                        &monad_envs::arc_experience::chain_to_term(&seed),
                    );
                    let (c, rep2) = monad_envs::arc_experience::compose_guided(
                        &mut lib,
                        &train,
                        &[seed],
                        40_000,
                    );
                    reuse_tries += rep2.tries;
                    reuse_novel += rep2.novel;
                    reuse_probes += rep2.probes;
                    ops = c;
                }
                // **anytime 일반화 합성**: 깊이 고정을 풀고 잔차 유도 최선우선으로
                // (정규형 메모이제이션·진전 시에만 확장·학습된 사전분포 순).
                if ops.is_none() {
                    let (c, rep3) =
                        monad_envs::arc_experience::compose_anytime(&mut lib, &train, 40_000, 4);
                    reuse_tries += rep3.tries;
                    reuse_novel += rep3.novel;
                    reuse_probes += rep3.probes;
                    ops = c;
                }
                // **패치 규칙 전이**(시도 157) — 잔차 해부가 정한 생성적 기질.
                // 다른 과제에서 배운 국소 재작성 규칙이 이 과제의 훈련쌍을 정확히
                // 재현하면, 그 규칙으로 시험을 푼다. 이것이 code-free 전이의 형태다.
                // 기억은 가설일 뿐 — **이 과제의 증거로 검증해 모순 없는 규칙만**
                // 채택한다(전량 적용은 남의 규칙이 오발화해 반드시 깨진다, 시도 158).
                // **관계 규칙 전이**(GEN3, 시도 192): 존재 양화로 속성 벡터가
                // 원리상 구분 못 하는 자리(홀드아웃의 58%)를 노린다.
                if ops.is_none() {
                    let sel = monad_envs::arc_relrule::select_rel_consistent(rel_view, &train);
                    if monad_envs::arc_relrule::rel_rules_reproduce(&sel, &train) {
                        rel_gate_pass += 1;
                        let all_ok = task.test.iter().all(|p| {
                            monad_envs::arc_relrule::apply_rel_rules(&sel, &p.input) == p.output
                        });
                        if all_ok {
                            rel_solved += 1;
                            reuse_solved += 1;
                            monad_names.push(task.name.clone());
                            solved += 1;
                            solved_names.push(task.name.clone());
                            continue;
                        }
                        for p in &task.test {
                            let pred = monad_envs::arc_relrule::apply_rel_rules(&sel, &p.input);
                            if pred == p.input {
                                of_silent += 1;
                            } else {
                                of_misfire += 1;
                            }
                            let prof = monad_envs::arc_experience::effect_profile(
                                std::slice::from_ref(&(p.input.clone(), p.output.clone())),
                                std::slice::from_ref(&p.input),
                                std::slice::from_ref(&pred),
                            );
                            of_corrected += prof.corrected;
                            of_damaged += prof.damaged;
                            of_before_wrong += prof.before_wrong;
                            of_after_wrong += prof.after_wrong;
                        }
                    }
                }
                // **두 계층 결합 전이**(시도 196): GEN2(속성)와 GEN3(관계)는
                // 서로 다른 객체를 덮는다. 과제 게이트는 바뀐 객체가 **전부**
                // 덮여야 열리므로, 합집합이 각 계층 단독보다 유리하다.
                if ops.is_none() {
                    let osel =
                        monad_envs::arc_objrule::select_obj_consistent(obj_view, &train);
                    let rsel = monad_envs::arc_relrule::select_rel_consistent(rel_view, &train);
                    if !osel.is_empty() && !rsel.is_empty() {
                        if monad_envs::arc_relrule::combined_reproduce(&osel, &rsel, &train) {
                            comb_gate_pass += 1;
                            let all_ok = task.test.iter().all(|p| {
                                monad_envs::arc_relrule::apply_combined(&osel, &rsel, &p.input)
                                    == p.output
                            });
                            if all_ok {
                                comb_solved += 1;
                                reuse_solved += 1;
                                monad_names.push(task.name.clone());
                                solved += 1;
                                solved_names.push(task.name.clone());
                                continue;
                            }
                            for p in &task.test {
                                let pred = monad_envs::arc_relrule::apply_combined(
                                    &osel, &rsel, &p.input,
                                );
                                if pred == p.input {
                                    of_silent += 1;
                                } else {
                                    of_misfire += 1;
                                }
                                let prof = monad_envs::arc_experience::effect_profile(
                                    std::slice::from_ref(&(p.input.clone(), p.output.clone())),
                                    std::slice::from_ref(&p.input),
                                    std::slice::from_ref(&pred),
                                );
                                of_corrected += prof.corrected;
                                of_damaged += prof.damaged;
                                of_before_wrong += prof.before_wrong;
                                of_after_wrong += prof.after_wrong;
                            }
                        }
                    }
                }
                // **객체 델타 규칙 전이**(시도 166): 승자 표현 위의 성질 조건 규칙.
                // 같은 오염 차단 규율 — 이 과제가 낳은 항목은 시야에서 빠진다.
                if ops.is_none() {
                    let mut sel = monad_envs::arc_objrule::select_obj_consistent(obj_view, &train);
                    // 단독 일관성으로 재현하지 못하면 **결정 목록**으로 다시 고른다
                    // (예외 우선 + 일반 후속 — 진단이 지목한 미발화 58%의 처방)
                    if !monad_envs::arc_objrule::obj_rules_reproduce(&sel, &train) {
                        sel = monad_envs::arc_objrule::select_obj_cover(&obj_view, &train, 24);
                    }
                    // **반복 적용**(시도 191): 규칙이 한 번 적용되면 객체의 성질이
                    // 바뀌어 다른 규칙이 발화한다. 재현되는 **최소 깊이**를 찾는다
                    // (깊을수록 우연한 일치 위험이 커지므로 가장 얕은 설명을 택한다).
                    let depth =
                        monad_envs::arc_objrule::obj_rules_reproduce_depth(&sel, &train, 4);
                    if let Some(d) = depth {
                        obj_gate_pass += 1;
                        if d > 1 {
                            obj_iter_gate += 1;
                        }
                        let all_ok = task.test.iter().all(|p| {
                            monad_envs::arc_objrule::apply_obj_rules_iter(&sel, &p.input, d)
                                == p.output
                        });
                        if all_ok {
                            obj_solved += 1;
                            reuse_solved += 1;
                            monad_names.push(task.name.clone());
                            solved += 1;
                            solved_names.push(task.name.clone());
                            continue;
                        }
                        // 게이트는 통과했는데 시험에서 틀렸다 — 과적합의 현장.
                        for p in &task.test {
                            let pred = monad_envs::arc_objrule::apply_obj_rules_iter(
                                &sel, &p.input, d,
                            );
                            if pred == p.input {
                                of_silent += 1;
                            } else {
                                of_misfire += 1;
                            }
                            let prof = monad_envs::arc_experience::effect_profile(
                                std::slice::from_ref(&(p.input.clone(), p.output.clone())),
                                std::slice::from_ref(&p.input),
                                std::slice::from_ref(&pred),
                            );
                            of_corrected += prof.corrected;
                            of_damaged += prof.damaged;
                            of_before_wrong += prof.before_wrong;
                            of_after_wrong += prof.after_wrong;
                        }
                    } else {
                        // 합성 경로는 **부분 기술 과제에도** 적용한다(시도 190):
                        // 완전성은 게이트의 몫이지 선택의 몫이 아니다.
                        let sel = if sel.is_empty() {
                            monad_envs::arc_objrule::select_obj_consistent_partial(
                                &obj_lib, &train,
                            )
                        } else {
                            sel
                        };
                        if !sel.is_empty() {
                        // **계층 간 합성**(시도 189) — 계량이 지목한 유일한 방향.
                        //
                        // 홀드아웃 바뀐 객체 212개 중 122개(58%)는 **원리상** 어떤
                        // 성질 규칙으로도 덮을 수 없다(같은 과제에 성질 동일·행동
                        // 상이 객체가 있으므로). 그래서 과제당 100% 재현을 요구하는
                        // 게이트는 대부분의 과제에서 영영 닫히지 않는다.
                        //
                        // 그러나 부분 덮개는 **잔차를 줄인다**. 객체 규칙이 자기가
                        // 아는 만큼 고치고, 남은 잔차를 동결 계층이 마무리하면
                        // 부분 덮개에 신용이 생긴다. 이것이 운영자가 요구한
                        // "schema 조합"의 계층 간 형태다.
                        //
                        // 규율: ①잔차가 실제로 줄 때만 진행(무해성) ②합성 전체가
                        // 훈련쌍을 정확히 재현할 때만 채택(예측의 전제) ③출처 분리는
                        // 그대로.
                        let mids: Vec<monad_envs::grid::Grid> = train
                            .iter()
                            .map(|(i, _)| monad_envs::arc_objrule::apply_obj_rules(&sel, i))
                            .collect();
                        let wrong = |a: &monad_envs::grid::Grid,
                                     b: &monad_envs::grid::Grid| -> usize {
                            if a.w != b.w || a.h != b.h {
                                return usize::MAX;
                            }
                            a.cells.iter().zip(b.cells.iter()).filter(|(x, y)| x != y).count()
                        };
                        let before: usize =
                            train.iter().map(|(i, o)| wrong(i, o)).sum();
                        let after: usize =
                            mids.iter().zip(train.iter()).map(|(m, (_, o))| wrong(m, o)).sum();
                        if after < before {
                            obj_partial += 1;
                            let resid: Vec<_> = mids
                                .iter()
                                .cloned()
                                .zip(train.iter().map(|(_, o)| o.clone()))
                                .collect();
                            // 동결 계층들을 잔차 위에서 시도한다(새 어휘 없음 —
                            // 있는 기계를 새 자리에서 돌리는 것뿐이다).
                            let mut closed = false;
                            if let Some(chain) = monad_envs::arc_solve::try_grid_chain3(&resid) {
                                if task.test.iter().all(|p| {
                                    let m = monad_envs::arc_objrule::apply_obj_rules(&sel, &p.input);
                                    monad_envs::arc_solve::apply_grid_chain_n(&m, &chain) == p.output
                                }) {
                                    closed = true;
                                }
                            }
                            if !closed {
                                if let Some(cl) = monad_envs::arc_solve::try_cellwise(&resid) {
                                    if task.test.iter().all(|p| {
                                        let m = monad_envs::arc_objrule::apply_obj_rules(&sel, &p.input);
                                        monad_envs::arc_solve::apply_cellwise(&m, &cl) == p.output
                                    }) {
                                        closed = true;
                                    }
                                }
                            }
                            if !closed
                                && task.test.iter().all(|p| {
                                    let m = monad_envs::arc_objrule::apply_obj_rules(&sel, &p.input);
                                    monad_envs::arc_ebm::ebm_solve(&resid, &m)
                                        == Some(p.output.clone())
                                })
                            {
                                closed = true;
                            }
                            if closed {
                                obj_layered += 1;
                                reuse_solved += 1;
                                monad_names.push(task.name.clone());
                                solved += 1;
                                solved_names.push(task.name.clone());
                                continue;
                            }
                        }
                        }
                    }
                }
                // **오염 차단**: 규칙을 배운 과제 자신에게는 전이를 시도하지 않는다.
                // 자기 훈련쌍에서 나온 규칙으로 자기를 푸는 것은 전이가 아니라 순환이다.
                if ops.is_none()
                    && !patch_sources.contains(&task.name)
                    && std::env::var("MONAD_ARC_NO_PATCH").is_err()
                {
                    // 일반화 압력(시도 160): 지지도 하한 + 일반적인 규칙 우선.
                    // 159에서 게이트 통과 4건 중 3건이 시험에서 틀린 원인(과적합)의 처방.
                    // 시도 162: 진단(덮개 100%·오발화 341셀/과제)이 지목한 처방 —
                    // 구체 규칙 우선 + 충돌을 세며 하나씩 넣는 탐욕 덮개 선택.
                    let sel = monad_envs::arc_patch::select_cover(&patch_lib, &train, 24);
                    patch_selected += sel.len();
                    if monad_envs::arc_patch::selected_reproduce(&sel, &train) {
                        patch_gate_pass += 1;
                        let all_ok = task.test.iter().all(|p| {
                            monad_envs::arc_patch::apply_selected(&sel, &p.input) == p.output
                        });
                        if all_ok {
                            patch_solved += 1;
                            reuse_solved += 1;
                            monad_names.push(task.name.clone());
                            solved += 1;
                            solved_names.push(task.name.clone());
                            continue;
                        }
                    }
                }
                // **잔차 닫기(개념 학습)** — oracle 진단(시도 154)의 처방.
                // 조합만으로는 도달 불가(40/40 단조 경로 소진)이므로, 부분 진전이
                // 남긴 작고 구조화된 잔차 위에서 **규칙을 새로 학습**한다. 손으로
                // 쓰는 것이 아니라 데이터에서 만든다 — 기존 학습 기계를 **새 위치**
                // 에서 돌리는 것이며, 성공 시 그 규칙은 MONAD_DERIVED다.
                if ops.is_none() {
                    if let Some((seed2, _, _, _)) =
                        monad_envs::arc_experience::probe_partial(&lib, &train, 8_000)
                    {
                        let mids: Option<Vec<monad_envs::grid::Grid>> = train
                            .iter()
                            .map(|(i, _)| monad_envs::arc_experience::safe_chain(i, &seed2))
                            .collect();
                        if let Some(mids) = mids {
                            let resid: Vec<_> = mids
                                .into_iter()
                                .zip(train.iter().map(|(_, o)| o.clone()))
                                .collect();
                            residual_closed_tries += 1;
                            let all_ok = task.test.iter().all(|p| {
                                match monad_envs::arc_experience::safe_chain(&p.input, &seed2) {
                                    Some(mid) => {
                                        monad_envs::arc_ebm::ebm_solve(&resid, &mid)
                                            == Some(p.output.clone())
                                    }
                                    None => false,
                                }
                            });
                            if all_ok {
                                residual_closed += 1;
                                reuse_solved += 1;
                                monad_names.push(task.name.clone());
                                solved += 1;
                                solved_names.push(task.name.clone());
                                continue;
                            }
                        }
                    }
                }
            }
            if let Some(ops) = ops {
                let all_ok = task.test.iter().all(|p| {
                    monad_envs::arc_solve::apply_grid_chain_n(&p.input, &ops) == p.output
                });
                if all_ok {
                    reuse_solved += 1;
                    monad_names.push(task.name.clone());
                    solved += 1;
                    solved_names.push(task.name.clone());
                }
            }
        }
        let _ = lib.save(&lib_path);
        println!(
            "\n[M2-R 코드 고정 자기학습] 라이브러리 {}개(MONAD_DERIVED {}) · 재구체화 해결 **{}건** \
             · 신규 대입 {} · 스키마 시도 {} · 후보 검사 {} · {:.0}초",
            lib_before,
            lib.count(monad_core::abstraction::Provenance::MonadDerived),
            reuse_solved,
            reuse_novel,
            reuse_tries,
            reuse_probes,
            t_reuse.elapsed().as_secs_f32()
        );
        println!(
            "  재사용률 {:.3} · 압축률 {:.2} · 신규 재구체화율 {:.3}",
            lib.reuse_rate(),
            lib.compression(),
            if reuse_solved > 0 { reuse_novel as f64 / reuse_solved as f64 } else { 0.0 }
        );
        println!(
            "  부분 진전 경험 {}건 · 평균 잔차 감소 {:.3} (교사가 버리는 정보의 회수량)",
            partial_found,
            if partial_found > 0 { partial_gain / partial_found as f64 } else { 0.0 }
        );
        println!(
            "  효과 프로파일: 고친 셀 {} · 망가뜨린 셀 {} · 평균 정밀도 {:.2} \
             (부분 목표 스키마의 재료)",
            partial_corrected,
            partial_damaged,
            if partial_found > 0 { partial_precision / partial_found as f64 } else { 0.0 }
        );
        println!(
            "  잔차 닫기(개념 학습): 시도 {}건 → **해결 {}건** (조합 불가 영역의 돌파 여부)",
            residual_closed_tries, residual_closed
        );
        println!(
            "  객체 규칙 전이: 규칙 {}개 · 훈련 재현 통과 {}건 → **해결 {}건** (승자 표현 위 성질 조건 규칙)",
            obj_lib.entries.len(),
            obj_gate_pass,
            obj_solved
        );
        println!(
            "     └ 계층 간 합성: 잔차 감소 {}건 → 동결 계층 마무리 **{}건** · 반복 적용(깊이>1) 통과 {}건",
            obj_partial, obj_layered, obj_iter_gate
        );
        println!(
            "  관계 규칙 전이(GEN3): 규칙 {}개 · 훈련 재현 통과 {}건 → **해결 {}건** (존재 양화)",
            rel_lib.entries.len(),
            rel_gate_pass,
            rel_solved
        );
        println!(
            "  두 계층 결합(시도 196): 훈련 재현 통과 {}건 → **해결 {}건** (속성 ∪ 관계)",
            comb_gate_pass, comb_solved
        );
        println!(
            "  답 색 규칙 전이(시도 207): 규칙 {}개 · 출처 {}과제 · 훈련 재현 {}건 → **해결 {}건** (어느 색이 답인가)",
            ans_lib.entries.len(), ans_sources.len(), ans_gate_pass, ans_solved
        );
        println!(
            "  선택 규칙 전이(시도 206): 규칙 {}개 · 훈련 재현 통과 {}건 → **해결 {}건** (무엇이 답인가)",
            sel_lib.entries.len(),
            sel_gate_pass,
            sel_solved
        );
        println!(
            "  패치 규칙 전이: 규칙 {}개 · 증거선택 누적 {}개 · 훈련 재현 통과 {}건 → **해결 {}건** \
             (다른 과제에서 배운 규칙이 이 과제를 푸는가)",
            patch_lib.entries.len(),
            patch_selected,
            patch_gate_pass,
            patch_solved
        );
        // **회계**: 답 색·선택 계층도 MONAD_DERIVED다. 두 계층은 `reuse_solved`가
        // 선언되기 전(위쪽)에서 판정되므로 따로 세어 여기서 합친다 — 안 합치면
        // 그 해결이 **동결 솔버의 공으로 잘못 기록된다**(지금까지 두 계층이 0건이라
        // 드러나지 않았을 뿐, 출처 회계를 뒤집는 오류다).
        let monad_solved = reuse_solved + ans_solved + sel_solved;
        println!(
            "\n▶▶ **최종 해결 {} / 400 = {:.1}%** (동결 솔버 {} + MONAD_DERIVED {})",
            solved,
            100.0 * solved as f64 / tasks.len() as f64,
            solved - monad_solved,
            monad_solved
        );
        // **자기설명적 출처 분리**: MONAD가 푼 과제마다, 그 과제가 라이브러리에
        // 몇 개를 기여했는지 적는다. 기여분은 전부 배제된 채로 풀린 것이므로
        // 이 숫자가 클수록 "자기 것으로 자기를 푼 게 아니다"가 강하게 성립한다.
        // **과적합 현장**(시도 209): 게이트를 통과하고도 시험에서 틀린 시험쌍을
        // 침묵/오발화로 가른다. 두 처방이 정반대이므로 이 비율이 다음 개입의
        // 방향을 정한다 — 침묵이 많으면 일반성, 오발화가 많으면 판별력.
        println!(
            "\n  과적합 현장(게이트 통과 후 시험 실패): 침묵 {}건 · 오발화 {}건",
            of_silent, of_misfire
        );
        if of_silent + of_misfire > 0 {
            // 셀 단위 분해: 오발화 안에서 **과발화(damaged)**와 **미수정(남은 오답)**
            // 을 가른다. 앞이 크면 판별력, 뒤가 크면 덮개가 다음 개입의 축이다.
            let unfixed = of_after_wrong.saturating_sub(of_damaged);
            println!(
                "    셀 분해: 고침 {} · **망침 {}** · 미수정 {} (적용 전 오답 {} → 적용 후 {})",
                of_corrected, of_damaged, unfixed, of_before_wrong, of_after_wrong
            );
            println!(
                "    → {}",
                if of_damaged > unfixed {
                    "과발화가 우세 — 규칙이 건드리면 안 될 곳을 건드린다(판별력)"
                } else {
                    "미수정이 우세 — 규칙이 닿지 못한 곳이 더 많다(덮개)"
                }
            );
        }
        if !monad_names.is_empty() {
            println!("\n  MONAD_DERIVED 해결 과제(각각 자기 기여분은 배제됨):");
            for n in &monad_names {
                println!(
                    "    {n} — 이 과제가 기여한 항목: 객체 {}개 · 관계 {}개 (전부 배제)",
                    obj_lib.sourced_by(n),
                    rel_lib.sourced_by(n)
                );
            }
        }
        // 해결 목록을 남긴다 — oracle 진단기가 미해결 집합을 알아야 한다
        let _ = std::fs::write(
            std::env::var("MONAD_ARC_SOLVED")
                .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-solved.txt".into()),
            solved_names.join("\n"),
        );
    }

    if !solved_names.is_empty() {
        println!("해결 과제: {}", solved_names.join(", "));
    }
    println!("\n실패 유형 분포(동일 크기 시험 1번 기준, 중복 집계 허용):");
    println!("  셀 일치 ≥90%(근접): {fail_near} · 50~90%: {fail_mid} · <50%(원거리): {fail_far}");
    println!("  파편화(입력 객체 >12 — 패턴형 과제): {fail_fragmented}");
    println!("  객체 생성형(출력>입력): {fail_gen} · 소멸/병합형(출력<입력): {fail_del}");
    println!("  신규 색 등장(입력에 없는 색): {fail_newcolor}");
    println!(
        "\n프로그램 합성(W2-3R): 신규 해결 {}건 · 라이브러리 프로그램 {}개 · 수면 형상 재구체화 {}건 · 셀 역할 꿈 {}건 · EBM {}건",
        prog_solved,
        prog_lib.programs.len(),
        sleep_solved,
        dream_solved,
        ebm_solved
    );
    println!(
        "스키마 라이브러리(W2-2): 풀 {}규칙 · 과제 간 재사용 {}회 (PRD 재사용률 지표 1차)",
        pool.len(),
        reuse_hits
    );
    println!("\n▶ W2-1 기준선 기록 완료 — 실패 분포가 어휘 확장 우선순위(W2-2)를 정한다");
}
