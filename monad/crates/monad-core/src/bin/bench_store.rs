//! A2 DoD 벤치마크: 대규모 연상 메모리.
//!
//! DoD: 100만 원자에서 top-8 조회 < 1ms, 20% 잡음 리콜@8 ≥ 99%.
//! 실행: `cargo run --release --bin bench-store`

use monad_core::rng::Rng;
use monad_core::sbv::{Sbv, NBLOCKS};
use monad_core::store::Store;
use std::time::Instant;

fn corrupt(v: &Sbv, blocks: usize, r: &mut Rng) -> Sbv {
    let mut q = *v;
    let mut perm: [u8; NBLOCKS] = std::array::from_fn(|i| i as u8);
    for i in 0..blocks.min(NBLOCKS) {
        let j = i + r.below((NBLOCKS - i) as u32) as usize;
        perm.swap(i, j);
        let b = perm[i] as usize;
        q.idx[b] = (q.idx[b] + 1 + r.below(127) as u8) & 127;
    }
    q
}

fn main() {
    println!("== MONAD A2 연상 메모리 벤치마크 ==\n");
    println!("{:>10} {:>10} {:>12} {:>12} {:>10} {:>10}",
             "원자 수", "삽입(초)", "조회(µs)", "선형(µs)", "가속", "메모리MB");
    println!("{}", "-".repeat(70));

    let mut r = Rng::new(4242);
    for n in [10_000usize, 100_000, 1_000_000] {
        let vs: Vec<Sbv> = (0..n).map(|_| Sbv::random(&mut r)).collect();

        let t0 = Instant::now();
        let mut s = Store::new();
        for (i, v) in vs.iter().enumerate() {
            s.insert(i as u32, *v);
        }
        let build = t0.elapsed().as_secs_f64();

        // 색인 조회
        let trials = if n >= 1_000_000 { 2000 } else { 5000 };
        let queries: Vec<Sbv> = (0..trials)
            .map(|_| {
                let t = r.below(n as u32) as usize;
                corrupt(&vs[t], 26, &mut r)
            })
            .collect();

        let t0 = Instant::now();
        let mut sink = 0u32;
        for q in &queries {
            let h = s.query(q, 8);
            sink = sink.wrapping_add(h.len() as u32);
        }
        let idx_us = t0.elapsed().as_secs_f64() * 1e6 / trials as f64;
        std::hint::black_box(sink);

        // 선형 스캔 기준선(표본 축소)
        let lin_trials = (trials / 20).max(20);
        let t0 = Instant::now();
        for q in queries.iter().take(lin_trials) {
            std::hint::black_box(s.query_exact(q, 8));
        }
        let lin_us = t0.elapsed().as_secs_f64() * 1e6 / lin_trials as f64;

        let (_buckets, entries, _avg) = s.index_stats();
        let mem_mb = (n * 128 + entries * 4 + n * 16) as f64 / 1e6;

        println!(
            "{:>10} {:>10.2} {:>12.1} {:>12.1} {:>9.0}× {:>10.0}",
            n, build, idx_us, lin_us, lin_us / idx_us, mem_mb
        );

        if n == 1_000_000 {
            println!(
                "\nDoD: top-8 조회 < 1000µs → {:.1}µs = {}",
                idx_us,
                if idx_us < 1000.0 { "통과" } else { "실패" }
            );
        }
    }

    // 잡음 대비 리콜
    println!("\n-- 리콜@8 대 잡음 (N=100k) --");
    println!("{:>10} {:>10} {:>12}", "잡음블록", "리콜@8", "후보수(평균)");
    println!("{}", "-".repeat(36));
    let n = 100_000usize;
    let mut r = Rng::new(99);
    let vs: Vec<Sbv> = (0..n).map(|_| Sbv::random(&mut r)).collect();
    let mut s = Store::new();
    for (i, v) in vs.iter().enumerate() {
        s.insert(i as u32, *v);
    }
    for noise in [0usize, 13, 26, 38, 51, 64, 77] {
        let trials = 1000;
        let mut hit = 0;
        let mut cands = 0usize;
        for _ in 0..trials {
            let t = r.below(n as u32) as usize;
            let q = corrupt(&vs[t], noise, &mut r);
            let res = s.query(&q, 8);
            cands += res.len();
            if res.iter().any(|h| h.id == t as u32) {
                hit += 1;
            }
        }
        println!(
            "{:>9}% {:>9.1}% {:>12.1}",
            noise * 100 / 128,
            hit as f64 / trials as f64 * 100.0,
            cands as f64 / trials as f64
        );
    }
    println!("\n해석: 밴딩 색인은 잡음이 커지면 후보를 놓친다(리콜 하락). 정착 루프(B2)는");
    println!("      직전 상태의 이웃을 함께 후보로 넣어 이 구멍을 메운다.");
}
