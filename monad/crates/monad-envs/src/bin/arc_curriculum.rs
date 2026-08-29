//! M2-R — **Active Epistemic Curriculum**: 무엇을 다음에 볼지 스스로 고른다.
//!
//! # 왜 "생성"이 아니라 "선택"인가 (v1의 정직한 설계)
//!
//! 자기 과제 생성의 근본 문제는 **정답의 출처**다. MONAD가 만든 문제의 답을
//! 자기 가설로 채우면 순환이고, 아는 것만 반복하는 자기만족 루프가 된다
//! ("나는 above를 안다 → above 문제 생성 → 성공 → 똑똑해졌다").
//!
//! 그래서 v1은 정답이 **실재하는 환경**(훈련셋)에서 *무엇을 언제 볼지*를 스스로
//! 고른다. 정보이득 개념은 그대로 유지하면서 순환을 원천 차단하는 형태다.
//! 환경이 오라클이므로, 고른 경험이 실제로 가설을 가른다.
//!
//! # 불확실성의 정의 (계량 가능해야 한다)
//!
//! 한 관측 지점에서 라이브러리는 셋 중 하나다:
//!
//! - **침묵**: 어떤 규칙도 발화하지 않는다 → 무지
//! - **경합**: 둘 이상이 발화하는데 **서로 다른 행동**을 주장한다 → 혼동
//! - 합의: 발화하는 규칙들이 같은 행동을 말한다 → 앎
//!
//! 침묵 + 경합 = **불확실성**. 그것을 가장 많이 줄이는 과제가 다음 경험이다.
//! 이미 자신 있는 것을 반복하면 점수가 0이므로 자기만족 루프가 성립하지 않는다.
//!
//! # 봉인 규율
//!
//! 커리큘럼은 **출처 풀에서만** 고른다. 홀드아웃은 손대지 않는다 — 능력 증가는
//! 자기가 고르지 않은 미접촉 과제에서만 잰다.
//!
//! 실행: `arc-curriculum` (env: MONAD_ARC_CUR_BUDGET · MONAD_ARC_CUR_POOL)

use monad_core::abstraction::Library;
use monad_envs::arc_data::load_dir;
use monad_envs::arc_objrule::{
    extract_obj_rules, obj_rule_action, sleep_obj_abstract, sleep_obj_cross, sleep_obj_drop,
    task_props_partial, Site,
};
use monad_envs::grid::Grid;

/// 라이브러리에서 발화 판정에 쓸 규칙들(사전분포 상위 cap개 — 비용 상한).
fn top_rules(
    lib: &Library,
    cap: usize,
) -> Vec<(Vec<monad_core::abstraction::Term>, monad_core::abstraction::Term,
    monad_core::abstraction::Term)> {
    lib.by_prior()
        .into_iter()
        .take(cap)
        .filter_map(|ix| monad_envs::arc_objrule::split_orule_pub(&lib.entries[ix].schema))
        .collect()
}

/// 선택 모드. 능동의 우월성을 주장하려면 **순서 운**을 배제해야 하므로,
/// 파일 순서만이 아니라 **무작위 다중 시드**와도 비교한다(운영자 지시).
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Active,
    Random,
    Sequential,
}

/// 정보이득 대리지표(시도 198). 무작위 대조에서 "침묵+경합 개수"가 무작위보다
/// 나빴던 원인은 그것이 **객체 수**를 고른다는 데 있었다. 경합만 세고 크기로
/// 정규화한다 — 경합이야말로 정답이 가설을 **판정**해 주는 자리다.
fn score(conflict: usize, silent: usize, _total: usize, w_silent: f64) -> f64 {
    // 이미 **구별되는 패턴 수**이므로 다시 나누지 않는다 — 나누면 빈 라이브러리에서
    // 모두 1.0으로 동점이 되어 sequential로 퇴화한다(시도 198에서 실측).
    conflict as f64 + w_silent * silent as f64
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let out_path = std::env::var("MONAD_ARC_CUR_LIST").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-curriculum.txt".into()
    });
    let budget: usize = std::env::var("MONAD_ARC_CUR_BUDGET")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);
    // 커리큘럼이 고를 수 있는 풀 = 출처 후보. 홀드아웃은 여기 없다(봉인).
    let pool: usize = std::env::var("MONAD_ARC_CUR_POOL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let rule_cap: usize = std::env::var("MONAD_ARC_CUR_RULECAP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000);
    let mode = match std::env::var("MONAD_ARC_CUR_MODE").unwrap_or_default().as_str() {
        "random" => Mode::Random,
        "sequential" => Mode::Sequential,
        _ => Mode::Active,
    };
    let seed: u64 = std::env::var("MONAD_ARC_CUR_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    // 침묵의 가중치. 0이면 **경합만** 본다(정답이 가설을 판정하는 자리).
    let w_silent: f64 = std::env::var("MONAD_ARC_CUR_WSILENT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.0);

    println!("=========================================================================");
    println!("M2-R Active Epistemic Curriculum — 불확실성을 가장 줄이는 경험을 스스로 고른다");
    println!("=========================================================================");

    let tasks = load_dir(std::path::Path::new(&dir));
    let cands: Vec<(String, Vec<(Grid, Grid)>)> = tasks
        .iter()
        .take(pool)
        .map(|t| {
            let tr: Vec<(Grid, Grid)> = t
                .train
                .iter()
                .map(|p| (p.input.clone(), p.output.clone()))
                .collect();
            (t.name.clone(), tr)
        })
        .filter(|(_, tr)| !extract_obj_rules(tr).is_empty())
        .collect();
    println!(
        "후보 풀 {}개(경험 생산 가능) · 예산 {}과제 · 규칙 상한 {}\n",
        cands.len(),
        budget,
        rule_cap
    );

    let mut lib = Library::new();
    let mut chosen: Vec<String> = Vec::new();
    let mut taken = vec![false; cands.len()];
    let mut u_log: Vec<usize> = Vec::new();

    let mut rng = monad_core::rng::Rng::new(seed);
    for step in 0..budget.min(cands.len()) {
        let rules = top_rules(&lib, rule_cap);
        // 각 후보의 불확실성을 재고 가장 큰 것을 고른다(정보이득 대리지표).
        // **선택은 입력만 본다** — task_uncertainty의 시그니처가 &[Grid]라서
        // 정답에 접근하는 것이 타입 수준에서 불가능하다(시도 197).
        // 이미 아는 것을 반복하면 점수가 0이므로 자기만족 루프가 성립하지 않는다.
        let mut best: Option<(usize, usize)> = None; // (불확실성, 색인)
        let remaining: Vec<usize> =
            (0..cands.len()).filter(|&ix| !taken[ix]).collect();
        if remaining.is_empty() {
            break;
        }
        match mode {
            Mode::Active => {
                let mut bs = f64::NEG_INFINITY;
                for &ix in &remaining {
                    let inputs: Vec<Grid> =
                        cands[ix].1.iter().map(|(i, _)| i.clone()).collect();
                    let (c, si, t) =
                        monad_envs::arc_objrule::task_uncertainty_split(&rules, &inputs);
                    let sc = score(c, si, t, w_silent);
                    if sc > bs {
                        bs = sc;
                        best = Some((c + si, ix));
                    }
                }
            }
            Mode::Random => {
                let pick = remaining[(rng.next_u64() as usize) % remaining.len()];
                let inputs: Vec<Grid> = cands[pick].1.iter().map(|(i, _)| i.clone()).collect();
                best = Some((
                    monad_envs::arc_objrule::task_uncertainty(&rules, &inputs),
                    pick,
                ));
            }
            Mode::Sequential => {
                let pick = remaining[0];
                let inputs: Vec<Grid> = cands[pick].1.iter().map(|(i, _)| i.clone()).collect();
                best = Some((
                    monad_envs::arc_objrule::task_uncertainty(&rules, &inputs),
                    pick,
                ));
            }
        }
        let Some((u, ix)) = best else { break };
        taken[ix] = true;
        chosen.push(cands[ix].0.clone());
        u_log.push(u);

        // 고른 경험으로 즉시 학습한다(다음 선택이 갱신된 앎 위에서 이뤄지도록)
        let tr = &cands[ix].1;
        let r = extract_obj_rules(tr);
        sleep_obj_abstract(&r, &mut lib);
        sleep_obj_cross(&[r], &mut lib);
        let st: Vec<Site> = task_props_partial(tr);
        if !st.is_empty() {
            sleep_obj_drop(&[st], &mut lib);
        }
        if step % 10 == 0 || step + 1 == budget {
            println!(
                "  [{:>3}] 불확실성 {:>3} 지점 · 규칙 {:>6}개 · 선택 {}",
                step + 1,
                u,
                lib.entries.len(),
                cands[ix].0
            );
        }
    }

    let _ = std::fs::write(&out_path, chosen.join("\n"));
    let first: usize = u_log.iter().take(5).sum();
    let last: usize = u_log.iter().rev().take(5).sum();
    println!(
        "\n선택 {}과제 기록 · 불확실성 추이: 처음 5회 합 {} → 마지막 5회 합 {}",
        chosen.len(),
        first,
        last
    );
    println!(
        "▶ 모드 {} · 시드 {} — 같은 예산의 무작위 다중 시드와 비교해야 순서 운을 배제한다.",
        match mode {
            Mode::Active => "active",
            Mode::Random => "random",
            Mode::Sequential => "sequential",
        },
        seed
    );
}
