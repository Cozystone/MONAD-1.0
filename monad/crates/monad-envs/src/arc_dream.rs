//! W2-D — 셀 역할 꿈: ARC × 클론-HMM (시도 138 사양의 구현).
//!
//! 별칭 가설: 같은 겉모습(색+4이웃)의 셀이 다른 출력을 내는 것은 미로의 지각
//! 별칭과 동형이다. 클론-HMM이 잠재 역할을 분리하면 역할별 출력 규칙은 단순하다.
//!
//! 구조: percept = (셀색, 상하좌우색) 사전화 id · 상태 = 지각당 K 클론 ·
//! 래스터 체인 위 전방-후방 EM(방출 = 출력색 분포 확장) · Viterbi 역할열 →
//! 역할별 최빈색 → 출력 격자. 훈련쌍 전부 정확 재현 시에만 채택.

use crate::grid::Grid;
use monad_core::rng::Rng;
use std::collections::HashMap;

const K: usize = 3; // 지각당 클론 수
const ITERS: usize = 30;
const RESTARTS: u64 = 3;

fn encode(g: &Grid, vocab: &mut HashMap<(u8, [u8; 4]), u32>, grow: bool) -> Option<Vec<u32>> {
    let mut seq = Vec::with_capacity(g.w * g.h);
    for y in 0..g.h {
        for x in 0..g.w {
            let nb = |dx: i32, dy: i32| -> u8 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if g.in_bounds(nx, ny) {
                    g.get(nx as usize, ny as usize)
                } else {
                    10 // 경계 표지
                }
            };
            let key = (g.get(x, y), [nb(0, -1), nb(0, 1), nb(-1, 0), nb(1, 0)]);
            match vocab.get(&key) {
                Some(&id) => seq.push(id),
                None if grow => {
                    let id = vocab.len() as u32;
                    vocab.insert(key, id);
                    seq.push(id);
                }
                None => return None, // 시험에서 미지 지각 — 보수적 실패
            }
        }
    }
    Some(seq)
}

/// 훈련쌍에서 역할 모델을 학습하고, 훈련 정확 재현이면 시험 출력을 예측한다.
pub fn dream_cells_solve(train: &[(Grid, Grid)], test_in: &Grid) -> Option<Grid> {
    // 동일 크기 전제
    if train.iter().any(|(i, o)| i.w != o.w || i.h != o.h) {
        return None;
    }
    let mut vocab: HashMap<(u8, [u8; 4]), u32> = HashMap::new();
    let mut seqs: Vec<(Vec<u32>, Vec<u8>)> = Vec::new();
    for (i, o) in train {
        let s = encode(i, &mut vocab, true)?;
        seqs.push((s, o.cells.clone()));
    }
    let np = vocab.len();
    let ns = np * K;
    let sid = |p: u32, k: usize| -> usize { p as usize * K + k };

    let mut best: Option<(f64, Vec<Vec<f64>>, Vec<[f64; 10]>)> = None;
    for r in 0..RESTARTS {
        let mut rng = Rng::new(0xCE11 ^ r);
        // 초기화: 전이(지각 제약은 E-스텝에서 후보 제한으로) · 방출
        let mut trans = vec![vec![1.0f64; ns]; ns];
        let mut em = vec![[1.0f64; 10]; ns];
        for row in trans.iter_mut() {
            for v in row.iter_mut() {
                *v = 1.0 + 0.1 * rng.next_f64();
            }
        }
        for e in em.iter_mut() {
            for v in e.iter_mut() {
                *v = 1.0 + 0.1 * rng.next_f64();
            }
        }
        let norm_t = |t: &mut Vec<Vec<f64>>| {
            for row in t.iter_mut() {
                let s: f64 = row.iter().sum();
                for v in row.iter_mut() {
                    *v /= s;
                }
            }
        };
        let norm_e = |e: &mut Vec<[f64; 10]>| {
            for row in e.iter_mut() {
                let s: f64 = row.iter().sum();
                for v in row.iter_mut() {
                    *v /= s;
                }
            }
        };
        norm_t(&mut trans);
        norm_e(&mut em);

        let mut ll_last = f64::NEG_INFINITY;
        for _ in 0..ITERS {
            let mut tc = vec![vec![0.01f64; ns]; ns];
            let mut ec = vec![[0.01f64; 10]; ns];
            let mut ll = 0.0f64;
            for (seq, outs) in &seqs {
                let t_len = seq.len();
                // 전방(후보 = 그 지각의 K 클론만)
                let mut alpha = vec![[0.0f64; K]; t_len];
                let mut scale = vec![0.0f64; t_len];
                for k in 0..K {
                    let s = sid(seq[0], k);
                    alpha[0][k] = em[s][outs[0] as usize];
                }
                scale[0] = alpha[0].iter().sum::<f64>().max(1e-300);
                for k in 0..K {
                    alpha[0][k] /= scale[0];
                }
                for t in 1..t_len {
                    for k in 0..K {
                        let s = sid(seq[t], k);
                        let mut a = 0.0;
                        for pk in 0..K {
                            let ps = sid(seq[t - 1], pk);
                            a += alpha[t - 1][pk] * trans[ps][s];
                        }
                        alpha[t][k] = a * em[s][outs[t] as usize];
                    }
                    scale[t] = alpha[t].iter().sum::<f64>().max(1e-300);
                    for k in 0..K {
                        alpha[t][k] /= scale[t];
                    }
                }
                ll += scale.iter().map(|s| s.ln()).sum::<f64>();
                // 후방 + 카운트
                let mut beta = vec![[0.0f64; K]; t_len];
                beta[t_len - 1] = [1.0; K];
                for t in (0..t_len - 1).rev() {
                    for k in 0..K {
                        let s = sid(seq[t], k);
                        let mut b = 0.0;
                        for nk in 0..K {
                            let nsid = sid(seq[t + 1], nk);
                            b += trans[s][nsid]
                                * em[nsid][outs[t + 1] as usize]
                                * beta[t + 1][nk];
                        }
                        beta[t][k] = b / scale[t + 1];
                    }
                }
                for t in 0..t_len {
                    let denom: f64 =
                        (0..K).map(|k| alpha[t][k] * beta[t][k]).sum::<f64>().max(1e-300);
                    for k in 0..K {
                        let s = sid(seq[t], k);
                        let g = alpha[t][k] * beta[t][k] / denom;
                        ec[s][outs[t] as usize] += g;
                        if t + 1 < t_len {
                            for nk in 0..K {
                                let nsid = sid(seq[t + 1], nk);
                                let xi = alpha[t][k]
                                    * trans[s][nsid]
                                    * em[nsid][outs[t + 1] as usize]
                                    * beta[t + 1][nk]
                                    / scale[t + 1];
                                tc[s][nsid] += xi;
                            }
                        }
                    }
                }
            }
            trans = tc;
            em = ec;
            norm_t(&mut trans);
            norm_e(&mut em);
            if (ll - ll_last).abs() < 1e-6 {
                break;
            }
            ll_last = ll;
        }
        if best.as_ref().map(|(b, _, _)| ll_last > *b).unwrap_or(true) {
            best = Some((ll_last, trans, em));
        }
    }
    let (_, trans, em) = best?;

    // Viterbi + 역할별 최빈색으로 훈련 정확 재현 확인
    let viterbi = |seq: &[u32], outs: Option<&[u8]>| -> Vec<usize> {
        let t_len = seq.len();
        let mut delta = vec![[f64::NEG_INFINITY; K]; t_len];
        let mut back = vec![[0usize; K]; t_len];
        for k in 0..K {
            let s = sid(seq[0], k);
            let e = outs.map(|o| em[s][o[0] as usize].ln()).unwrap_or(0.0);
            delta[0][k] = e;
        }
        for t in 1..t_len {
            for k in 0..K {
                let s = sid(seq[t], k);
                let e = outs.map(|o| em[s][o[t] as usize].ln()).unwrap_or(0.0);
                let (mut bv, mut bk) = (f64::NEG_INFINITY, 0);
                for pk in 0..K {
                    let ps = sid(seq[t - 1], pk);
                    let v = delta[t - 1][pk] + trans[ps][s].ln();
                    if v > bv {
                        bv = v;
                        bk = pk;
                    }
                }
                delta[t][k] = bv + e;
                back[t][k] = bk;
            }
        }
        let mut k = (0..K)
            .max_by(|&a, &b| delta[t_len - 1][a].partial_cmp(&delta[t_len - 1][b]).unwrap())
            .unwrap();
        let mut path = vec![0usize; t_len];
        path[t_len - 1] = sid(seq[t_len - 1], k);
        for t in (1..t_len).rev() {
            k = back[t][k];
            path[t - 1] = sid(seq[t - 1], k);
        }
        path
    };
    let role_color = |s: usize| -> u8 {
        let mut bi = 0usize;
        for c in 1..10 {
            if em[s][c] > em[s][bi] {
                bi = c;
            }
        }
        bi as u8
    };
    for (seq, outs) in &seqs {
        let path = viterbi(seq, Some(outs));
        for (t, &s) in path.iter().enumerate() {
            if role_color(s) != outs[t] {
                return None; // 훈련 정확 재현 실패 — 채택 안 함
            }
        }
    }
    // 시험 예측(미지 지각이면 실패)
    let tseq = encode(test_in, &mut vocab.clone(), false)?;
    let path = viterbi(&tseq, None);
    let mut out = Grid::new(test_in.w, test_in.h);
    for (t, &s) in path.iter().enumerate() {
        out.cells[t] = role_color(s);
    }
    Some(out)
}
