//! M2-R — **수면 패스**: 경험 저널 → 구조 추상화 → 스키마 라이브러리.
//!
//! 이 실행기는 ARC를 풀지 않는다. 과제를 보지도 않는다. 하는 일은 하나 —
//! 깨어 있는 동안 쌓인 경험(무엇을 무엇으로 풀었는가)에서 **공통 구조를 스스로
//! 발견**해 라이브러리에 축적하는 것이다. 무엇이 변수인지는 anti-unification이,
//! 채택 여부는 MDL이 정한다. 사람의 판단이 들어가는 자리가 없다.
//!
//! 실행: `cargo run --release --bin arc-sleep`
//! 환경: `MONAD_ARC_EXP`(경험 저널) · `MONAD_ARC_LIB`(라이브러리)

use monad_core::abstraction::{Library, Provenance};
use monad_envs::arc_experience::{load_experience, sleep_abstract};

fn main() {
    let exp_path = std::env::var("MONAD_ARC_EXP")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-experience.tsv".into());
    let lib_path = std::env::var("MONAD_ARC_LIB")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-library.tsv".into());

    println!("=========================================================================");
    println!("M2-R 수면 패스 — 경험에서 구조를 스스로 발견한다 (anti-unification + MDL)");
    println!("=========================================================================");

    // 성공 경험 + **부분 진전 경험**(교사가 버린 정보). 둘 다 경험이다 —
    // "무엇으로 풀었나"만이 아니라 "무엇이 가까워지게 했나"에서도 구조가 나온다.
    let partial_path = std::env::var("MONAD_ARC_PARTIAL")
        .unwrap_or_else(|_| "C:\\0.ASKIM ALL-VIN\\31.Homage AI\\monad-partial.tsv".into());
    let mut exp = load_experience(&exp_path);
    let n_success = exp.len();
    let partial = load_experience(&partial_path);
    exp.extend(partial.iter().cloned());
    println!(
        "경험 적재: 성공 {}건 + 부분 진전 {}건 = {}건",
        n_success,
        partial.len(),
        exp.len()
    );
    if exp.len() < 2 {
        println!("경험이 2건 미만 — 일반화할 것이 없다(각성을 더 돌릴 것).");
        return;
    }

    let mut lib = Library::load(&lib_path).unwrap_or_default();
    let before = lib.entries.len();
    let t0 = std::time::Instant::now();
    let (tried, added) = sleep_abstract(&exp, &mut lib);
    let _ = lib.save(&lib_path);

    println!(
        "\n일반화 시도 {tried}회 → **새 스키마 {added}개** (라이브러리 {} → {}) · {:.1}초",
        before,
        lib.entries.len(),
        t0.elapsed().as_secs_f32()
    );
    println!(
        "출처: MONAD_DERIVED {} · HUMAN_DERIVED {} · 압축률 {:.2} · 재사용률 {:.3}",
        lib.count(Provenance::MonadDerived),
        lib.count(Provenance::HumanDerived),
        lib.compression(),
        lib.reuse_rate()
    );

    // 유리상자: 가장 압축이 큰 스키마 몇 개를 사람이 읽는 형태로
    let mut ix: Vec<usize> = (0..lib.entries.len()).collect();
    ix.sort_by_key(|&i| std::cmp::Reverse(lib.entries[i].gain));
    println!("\n상위 스키마(압축 이득 순):");
    for &i in ix.iter().take(8) {
        let e = &lib.entries[i];
        println!(
            "  이득 {:>3} · 근거 {:>2} · 변수 {} · 시도 {}/성공 {} · {}",
            e.gain,
            e.support,
            e.schema.vars().len(),
            e.tries,
            e.wins,
            e.schema
        );
    }
    println!("\n▶ 수면 완료 — 다음 각성이 이 라이브러리를 탐색 사전분포로 쓴다.");
}
