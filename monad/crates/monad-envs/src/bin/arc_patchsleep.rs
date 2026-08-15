//! M2-R — **패치 규칙 수면**: 모든 과제의 국소 변화에서 재작성 규칙을 일반화한다.
//!
//! 이 실행기는 과제를 풀지 않고, 사람에게 보여주지도 않는다. 훈련쌍에서 **바뀐
//! 자리의 이웃 패턴**을 기계적으로 뽑아(해석 없음) LGG+MDL로 일반화해 패치 규칙
//! 라이브러리에 쌓는다. 목적은 성능이 아니라 **전이 가능성**이다 — 여기서 쌓인
//! 규칙이 다른 과제의 훈련쌍을 재현하면, 그것이 code-free 학습의 증거가 된다.
//!
//! 실행: `cargo run --release --bin arc-patchsleep`

use monad_core::abstraction::{Library, Provenance};
use monad_envs::arc_data::load_dir;
use monad_envs::arc_patch::{extract_rules, sleep_patch_abstract};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\data\\ARC-AGI\\data\\training".into()
    });
    let lib_path = std::env::var("MONAD_ARC_PATCHLIB").unwrap_or_else(|_| {
        "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-patchlib.tsv".into()
    });
    // 학습에 쓸 과제 수(홀드아웃 분리를 위해 앞쪽만 쓴다)
    let take: usize = std::env::var("MONAD_ARC_PATCH_TAKE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);

    println!("=========================================================================");
    println!("M2-R 패치 규칙 수면 — 국소 변화에서 재작성 규칙을 스스로 만든다");
    println!("=========================================================================");

    let tasks = load_dir(std::path::Path::new(&dir));
    let mut lib = Library::load(&lib_path).unwrap_or_default();
    let before = lib.entries.len();

    let mut rules = Vec::new();
    let mut used = 0usize;
    for task in tasks.iter().take(take) {
        let train: Vec<_> = task
            .train
            .iter()
            .map(|p| (p.input.clone(), p.output.clone()))
            .collect();
        let r = extract_rules(&train);
        if !r.is_empty() {
            used += 1;
            rules.extend(r);
        }
    }
    println!("규칙 경험 추출: {}건 (과제 {}개에서, 앞 {}개 대상)", rules.len(), used, take);
    if rules.len() < 2 {
        println!("일반화할 것이 없다.");
        return;
    }

    let t0 = std::time::Instant::now();
    let (tried, added) = sleep_patch_abstract(&rules, &mut lib);
    let _ = lib.save(&lib_path);
    println!(
        "일반화 시도 {tried}회 → **새 규칙 {added}개** (라이브러리 {} → {}) · {:.1}초",
        before,
        lib.entries.len(),
        t0.elapsed().as_secs_f32()
    );
    println!(
        "출처: MONAD_DERIVED {} · 압축률 {:.2}",
        lib.count(Provenance::MonadDerived),
        lib.compression()
    );
    println!("\n▶ 다음 각성이 이 규칙들을 **다른 과제에** 적용해 본다(전이 시험).");
}
