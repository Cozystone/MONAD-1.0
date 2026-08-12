//! A1 DoD 벤치마크.
//!
//! 측정 항목:
//!  1. 해밍 거리 처리량 (쌍/초/코어) — DoD: ≥ 1M/s/core
//!  2. bind/unbind 처리량
//!  3. **중첩 용량 곡선**: K개를 번들했을 때 구성원 복원율 (DoD 문서화 대상)
//!
//! 실행: `cargo run --release --bin bench-sbv`

use monad_core::rng::Rng;
use monad_core::sbv::{Bundler, Sbv};
use monad_core::simd_backend;
use std::time::Instant;

fn main() {
    println!("== MONAD A1 벤치마크 ==");
    println!("SIMD 백엔드: {}", simd_backend());
    println!("Sbv 크기: {} 바이트 (개념 차원 {} 비트)\n", std::mem::size_of::<Sbv>(), monad_core::DIM);

    throughput();
    println!();
    capacity_curve();
    println!();
    cleanup_recall();
}

/// 1. 연산 처리량 — 단일 스레드 기준(저사양 타깃을 보수적으로 대변).
fn throughput() {
    println!("-- 처리량 (단일 코어) --");
    let mut r = Rng::new(1);
    let n = 200_000usize;
    let a: Vec<Sbv> = (0..n).map(|_| Sbv::random(&mut r)).collect();
    let b: Vec<Sbv> = (0..n).map(|_| Sbv::random(&mut r)).collect();

    // dist
    let t0 = Instant::now();
    let mut acc = 0u64;
    for i in 0..n {
        acc += a[i].dist(&b[i]) as u64;
    }
    let dt = t0.elapsed().as_secs_f64();
    let rate = n as f64 / dt;
    println!(
        "dist   : {:>12.0} 쌍/초  ({:.1} M/s)  [평균거리 {:.2}]  DoD 1M/s → {}",
        rate,
        rate / 1e6,
        acc as f64 / n as f64,
        if rate >= 1e6 { "통과" } else { "실패" }
    );

    // bind
    let t0 = Instant::now();
    let mut sink = Sbv::ZERO;
    for i in 0..n {
        sink = a[i].bind(&b[i]);
    }
    let dt = t0.elapsed().as_secs_f64();
    println!("bind   : {:>12.0} 회/초  ({:.1} M/s)", n as f64 / dt, n as f64 / dt / 1e6);
    std::hint::black_box(sink);

    // unbind
    let t0 = Instant::now();
    for i in 0..n {
        sink = a[i].unbind(&b[i]);
    }
    let dt = t0.elapsed().as_secs_f64();
    println!("unbind : {:>12.0} 회/초  ({:.1} M/s)", n as f64 / dt, n as f64 / dt / 1e6);
    std::hint::black_box(sink);

    // bundle add
    let mut bl = Bundler::new();
    let t0 = Instant::now();
    for i in 0..n {
        bl.add(&a[i]);
    }
    let dt = t0.elapsed().as_secs_f64();
    println!("bundle+: {:>12.0} 회/초  ({:.1} M/s)", n as f64 / dt, n as f64 / dt / 1e6);
    std::hint::black_box(bl.finalize());
}

/// 2. 중첩 용량 곡선 — A1의 핵심 DoD 산출물.
///
/// K개 벡터를 중첩한 뒤, 각 구성원이 무작위 대조군 D개보다 확실히 가까운지
/// (= 연상 인출로 되찾을 수 있는지) 측정한다.
fn capacity_curve() {
    println!("-- 중첩 용량 곡선 (구성원 복원율) --");
    println!("{:>5} | {:>10} | {:>10} | {:>9} | {:>8}", "K", "구성원sim", "잡음sim", "선명도", "복원율");
    println!("{}", "-".repeat(56));

    let distractors = 1000usize;
    let trials = 200usize;

    for k in [2usize, 4, 8, 16, 32, 64, 128] {
        let mut r = Rng::new(1000 + k as u64);
        let mut member_sim = 0.0f64;
        let mut noise_sim = 0.0f64;
        let mut sharp = 0.0f64;
        let mut recovered = 0usize;
        let mut total = 0usize;

        for _ in 0..trials {
            let items: Vec<Sbv> = (0..k).map(|_| Sbv::random(&mut r)).collect();
            let mut bl = Bundler::new();
            for s in &items {
                bl.add(s);
            }
            let sup = bl.finalize();
            sharp += bl.sharpness() as f64;

            // 대조군(잡음) 중 최대 유사도 — 이보다 높아야 구성원을 "되찾았다"
            let mut max_noise = 0.0f32;
            let mut noise_mean = 0.0f64;
            for _ in 0..distractors {
                let d = Sbv::random(&mut r);
                let s = sup.sim(&d);
                noise_mean += s as f64;
                if s > max_noise {
                    max_noise = s;
                }
            }
            noise_sim += noise_mean / distractors as f64;

            for m in &items {
                let s = sup.sim(m);
                member_sim += s as f64;
                total += 1;
                if s > max_noise {
                    recovered += 1;
                }
            }
        }

        let rate = recovered as f64 / total as f64;
        println!(
            "{:>5} | {:>10.4} | {:>10.4} | {:>9.3} | {:>7.1}%",
            k,
            member_sim / total as f64,
            noise_sim / trials as f64,
            sharp / trials as f64,
            rate * 100.0
        );
    }
    println!("\n해석: 복원율은 '중첩 후에도 구성원을 무작위 1000개 중에서 식별 가능한 비율'.");
    println!("      K가 커질수록 개별 정체성이 흐려지는 것이 물리적 한계 — 이 값이");
    println!("      클론 그래프의 노드당 중첩 예산(A3)과 스키마 슬롯 수(C2)를 결정한다.");
}

/// 3. 연상 인출(cleanup) 정확도 — A2의 사전 검증.
fn cleanup_recall() {
    println!("-- 연상 인출: 잡음 섞인 질의로 원본 찾기 --");
    println!("{:>8} | {:>10} | {:>9}", "잡음블록", "N=10k", "N=100k");
    println!("{}", "-".repeat(34));

    for noise in [0usize, 64, 96, 112, 120, 124, 126, 128] {
        let mut line = format!("{:>8} |", noise);
        for n in [10_000usize, 100_000] {
            let mut r = Rng::new(77 + noise as u64);
            let db: Vec<Sbv> = (0..n).map(|_| Sbv::random(&mut r)).collect();
            let trials = 200;
            let mut hit = 0;
            for _ in 0..trials {
                let target = r.below(n as u32) as usize;
                // 질의: 원본에서 서로 다른 noise개 블록을 실제로 다른 값으로 교란.
                // (중복 추출하면 미교란 블록이 남아 잡음 강도를 과소평가하게 된다)
                let mut q = db[target];
                let mut perm: [u8; 128] = std::array::from_fn(|i| i as u8);
                for i in 0..noise.min(128) {
                    let j = i + r.below((128 - i) as u32) as usize;
                    perm.swap(i, j);
                    let b = perm[i] as usize;
                    let old = q.idx[b];
                    // 반드시 달라지도록: 1..127 만큼 회전
                    q.idx[b] = (old + 1 + r.below(127) as u8) & 127;
                }
                // 선형 스캔 최근접(A2에서 역색인으로 대체될 기준선)
                let mut best = u32::MAX;
                let mut best_i = 0usize;
                for (i, s) in db.iter().enumerate() {
                    let d = q.dist(s);
                    if d < best {
                        best = d;
                        best_i = i;
                    }
                }
                if best_i == target {
                    hit += 1;
                }
            }
            line.push_str(&format!(" {:>9.1}% |", hit as f64 / trials as f64 * 100.0));
        }
        println!("{}", line);
    }
    println!("\n해석: 블록의 25%(32개)가 교란돼도 10만 개 중에서 원본을 찾을 수 있어야");
    println!("      노이즈 많은 지각에서 '같은 물체'를 재인식할 수 있다(B1·B2의 전제).");
}
