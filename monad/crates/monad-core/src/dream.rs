//! 꿈 (Dream) — 수면기 전역 지도 추론.
//!
//! 각성의 지도 만들기는 1-shot·국소·탐욕적이라 필연적으로 근사다. 문맥 두 걸음으로
//! 같은 칸을 다른 클론으로 갈라놓기도 하고, 다른 칸을 한 클론에 뭉치기도 한다.
//! 그 근사를 **전체 경험에 대한 전역 추론**으로 바로잡는 것이 이 모듈이다.
//!
//! # 알고리즘: 클론-HMM 전방-후방 추론 (EM)
//!
//! 세계를 "지각당 K개의 클론 상태를 갖는 행동 조건부 은닉 마르코프 모델"로 놓고,
//! 에피소드 기억 전체에 대해 어느 순간 어느 클론에 있었는지의 사후분포를 계산한다
//! (전방-후방). 그 사후로 전이 카운트를 다시 추정하고, 수렴하면 최우도 경로
//! (Viterbi)로 경험 전체를 다시 라벨링해 그래프를 재건한다.
//!
//! 방출이 결정론적(상태의 지각 = 관측)이므로 각 시점의 은닉상태 후보는 그 지각의
//! 클론들뿐이다 — 전방-후방이 지각당 클론 수 K의 제곱으로만 비싸다(K≤32면 순식간).
//!
//! CSCG(George et al., Nature Communications 2021)가 별칭 미로를 푼 바로 그
//! 절차의 구현이다.
//!
//! # 교리 검토
//!
//! - 경사하강이 아니다: E-스텝은 확률 추론, M-스텝은 카운트 정규화 — 닫힌형
//!   베이지안 갱신이다.
//! - 리플레이 버퍼가 아니다: 에피소드 기억은 SGD 미니배치가 아니라 **일화 기억**이며,
//!   수면이 그것을 재생하며 지도를 굳힌다(해마→피질 압축의 동형).
//! - 각성은 온라인·즉답(1-shot 쓰기), 수면은 오프라인·전역(추론) — PRD §4.5의 분업.

use crate::atom::Val;
use crate::graph::WorldGraph;
use crate::rng::Rng;
use crate::wake::{Agent, EpStep};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct DreamConfig {
    /// 지각당 클론 수 상한(EM의 상태 예산). 남는 클론은 사용률 0으로 수렴해 소멸.
    pub max_clones: usize,
    /// EM 반복 횟수.
    pub iters: usize,
    /// 전이 디리클레 유사카운트.
    pub pseudo: f64,
    /// 대칭 깨기 잡음의 크기.
    pub jitter: f64,
    /// 결정론적 시드.
    pub seed: u64,
    /// Viterbi 후 이 미만으로 쓰인 상태는 지운다.
    pub min_usage: u32,
    /// (내부용) 지도별 하위 꿈에서 은퇴·버퍼 소거를 미룬다.
    ///
    /// 하위 꿈이 각자 은퇴(압축)를 하면 노드 번호가 재배열되어, **다음 그룹의
    /// 에피소드가 든 정착 상태 id가 전부 무효**가 되고 정렬 병합이 엉뚱한 노드와
    /// 짝지어진다(만성 환경2 저조의 원인으로 실측). 은퇴는 밤의 끝에 한 번만.
    pub skip_retire: bool,
    /// **응고화 모드** (연속 학습의 핵심).
    ///
    /// false(기본): 전 생애 에피소드로 그래프를 **전면 재건** — 단일 환경에 최적.
    /// true: 미소화 에피소드(해마 버퍼)만 꿈꾸고, 결과를 기존 그래프(피질)에
    /// **병합**한 뒤 버퍼를 비운다. 비용이 생애가 아니라 최근 경험에 비례하고,
    /// 옛 환경의 구조를 건드리지 않는다 — 창 없는 전 생애 꿈은 10환경에서
    /// 3시간·습득 붕괴로 실측 실패했다(LAB-NOTEBOOK 참조).
    pub consume: bool,
}

impl Default for DreamConfig {
    fn default() -> Self {
        DreamConfig {
            max_clones: 32,
            iters: 12,
            pseudo: 0.05,
            jitter: 0.02,
            seed: 0xD2EA,
            min_usage: 1,
            skip_retire: false,
            consume: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DreamReport {
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub steps_used: usize,
    pub iters: usize,
    /// 반복별 로그우도가 오르다 멈춘 지점의 값(자연로그, 스텝당).
    pub final_ll: f64,
    /// (응고화) 정렬로 기존 피질에 흡수된 EM 상태 수.
    pub aligned: usize,
    /// (응고화) 새 클론으로 병합된 EM 상태 수.
    pub created: usize,
}

/// (지각, 클론) → 전역 상태 번호.
struct StateSpace {
    offset: Vec<u32>,
    k: Vec<u32>,
    total: usize,
}

impl StateSpace {
    fn new(n_percepts: usize, k_of: impl Fn(usize) -> usize) -> Self {
        let mut offset = Vec::with_capacity(n_percepts);
        let mut k = Vec::with_capacity(n_percepts);
        let mut total = 0u32;
        for p in 0..n_percepts {
            offset.push(total);
            let kk = k_of(p) as u32;
            k.push(kk);
            total += kk;
        }
        StateSpace { offset, k, total: total as usize }
    }
    #[inline]
    fn id(&self, percept: u32, clone: u32) -> u32 {
        self.offset[percept as usize] + clone
    }
    #[inline]
    fn k_of(&self, percept: u32) -> u32 {
        self.k[percept as usize]
    }
}

/// 전이 파라미터: (상태, 행동) → 후속 상태 확률(희소).
struct Trans {
    map: HashMap<(u32, u16), Vec<(u32, f64)>>,
}

impl Trans {
    #[inline]
    fn get(&self, s: u32, a: u16) -> Option<&Vec<(u32, f64)>> {
        self.map.get(&(s, a))
    }
}

/// EM으로 지도를 다시 세우고 그래프를 재건한다.
pub fn dream(agent: &mut Agent, cfg: DreamConfig) -> DreamReport {
    let mut rep = DreamReport {
        nodes_before: agent.graph.n_nodes(),
        ..Default::default()
    };

    let n_percepts = agent.graph.n_percepts();
    if n_percepts == 0 {
        return rep;
    }

    // 응고화 모드에서 지도가 여럿이면 **지도별로 따로 꿈꾼다** — 서로 다른 세계의
    // 경험을 한 EM에 섞으면 세계가 섞인다.
    let distinct_tags = {
        let mut set: std::collections::HashSet<u32> = Default::default();
        for (i, ep) in agent.episodes.iter().enumerate() {
            if ep.len() >= 2 {
                set.insert(agent.episode_maps.get(i).copied().unwrap_or(0));
            }
        }
        set.len()
    };
    let _ = distinct_tags;
    if cfg.consume && !cfg.skip_retire {
        // **밤의 재분류**: 라이브 태그를 버리고, 각 에피소드를 내용으로 판정한다 —
        // 지도별 체인 생존율(폭 3, 카운트≥2)의 argmax. 어떤 지도도 절반을 못
        // 설명하면 "새 세계의 기록"으로 묶어 새 지도를 개설한다.
        //
        // 라이브 argmax는 흔들리고(감지 지연·과도기 소음), 흔들릴 때마다 에피소드가
        // 잘못 태그되면 응고가 환경 지식을 여러 지도로 파편화한다 — 여섯 가지 감지
        // 설계가 전부 같은 45% 대역에 갇힌 최종 원인. 깨어서는 살고(스코핑),
        // 자면서 어디 있었는지 정리한다(소속 판정).
        let fresh_set_cls: std::collections::HashSet<u32> =
            agent.fresh_clones.iter().copied().collect();
        let episode_score = |agent: &Agent, ep: &[EpStep], m: u32, shuffle: u32| -> f32 {
            if ep.len() < 2 {
                return 0.0;
            }
            let mut chain: Vec<u32> = Vec::new();
            let mut ok = 0usize;
            for t in 1..ep.len() {
                // 셔플 기준선: 행동 순서를 결정론적으로 흐트리면 진짜 동역학은
                // 깨지고 지도 크기 효과(우연 체인)만 남는다. 변주 3종의 평균으로
                // 기준선 분산을 조여 "운 좋은 단일 셔플"의 우연 흡수를 차단한다.
                let a = match shuffle {
                    0 => ep[t].action,
                    1 => ep[(t * 7 + 3) % ep.len()].action,
                    2 => ep[(t * 11 + 5) % ep.len()].action,
                    _ => ep[(t * 13 + 7) % ep.len()].action,
                };
                let p_cur = ep[t].percept;
                let mut next: Vec<(u32, u32)> = Vec::new();
                for &b in &chain {
                    for s in agent.graph.succ(b, a) {
                        if s.count >= 2
                            && !fresh_set_cls.contains(&s.to)
                            && agent.node_map.get(s.to as usize) == Some(&m)
                            && agent.graph.node(s.to).percept == p_cur
                            && !next.iter().any(|x| x.0 == s.to)
                        {
                            next.push((s.to, s.count));
                        }
                    }
                }
                if next.is_empty() {
                    let mut seed: Vec<(u32, u32)> = agent
                        .graph
                        .clones_of(p_cur)
                        .iter()
                        .filter(|&&c| {
                            agent.node_map.get(c as usize) == Some(&m)
                                && !fresh_set_cls.contains(&c)
                        })
                        .map(|&c| (c, agent.graph.node(c).atom.evidence))
                        .collect();
                    seed.sort_unstable_by_key(|x| std::cmp::Reverse(x.1));
                    seed.truncate(3);
                    chain = seed.into_iter().map(|x| x.0).collect();
                } else {
                    ok += 1;
                    next.sort_unstable_by_key(|x| std::cmp::Reverse(x.1));
                    next.truncate(3);
                    chain = next.into_iter().map(|x| x.0).collect();
                }
            }
            ok as f32 / (ep.len() - 1) as f32
        };

        let mut by_map: HashMap<u32, Vec<Vec<EpStep>>> = HashMap::new();
        let eps = std::mem::take(&mut agent.episodes);
        let tags = std::mem::take(&mut agent.episode_maps);
        for (ei, ep) in eps.into_iter().enumerate() {
            if ep.len() < 2 {
                continue;
            }
            // 오라클/수동 스코핑(지도 추론 꺼짐)에서는 라이브 태그가 곱 정답이다 —
            // 내용 재분류로 세계 번호를 흔들면 "지도 i = 환경 i" 계약이 깨진다.
            if !agent.cfg.map_inference {
                let m = tags.get(ei).copied().unwrap_or(agent.active_map);
                by_map.entry(m).or_default().push(ep);
                continue;
            }
            let mut best = (0u32, -1.0f32, 0.0f32);
            for m in 0..agent.n_maps {
                let real = episode_score(agent, &ep, m, 0);
                let base = (episode_score(agent, &ep, m, 1)
                    + episode_score(agent, &ep, m, 2)
                    + episode_score(agent, &ep, m, 3))
                    / 3.0;
                // 셔플 마진: 비만 지도의 우연 체인 효과를 기준선으로 상쇄 — 흡수 폭주 차단
                let marg = real - base;
                if marg > best.1 {
                    best = (m, marg, real);
                }
            }
            // AND 결합: 마진 ≥0.35는 비만 지도의 우연 체인을(크기 보정), 절대
            // ≥0.8은 같은 어휘 미로들 사이의 진짜 통계 중첩 감염을 차단한다
            // (자기 환경 실제 ~0.9 vs 외부 ~0.65~0.8 — 3변주 평균이 무변화라
            //  감염은 기준선 잡음이 아닌 실질 중첩임이 증명됨, 시도 53). 강화
            // 에피소드는 실제 0.85+라 추가 손실 없음. (시도 43의 OR 결합은 두
            //  문턱의 약점을 합집합으로 열어 실패 — AND는 교집합으로 닫는다.)
            let m = if best.1 >= 0.35 && best.2 >= 0.8 {
                best.0
            } else {
                // 에피소드마다 제 세계 — 미설명분을 한 버킷에 섞으면 학습·측정
                // 에피소드가 합쳐진 잡종 지도(비만·저ll)가 태어나 포획의 씨앗이
                // 된다(시도 41~43 실패의 공통 근원). 중복 세계는 유한 비용,
                // 잡종은 복리 비용이다.
                let new = agent.n_maps;
                agent.n_maps += 1;
                agent.map_birth.push(agent.graph.tick);
                agent.map_post.push(0.1);
                agent.map_chain.push(Vec::new());
                agent.map_cortical.push(0);
                new
            };
            by_map.entry(m).or_default().push(ep);
        }
        let mut total = DreamReport { nodes_before: rep.nodes_before, ..Default::default() };
        let mut maps: Vec<u32> = by_map.keys().copied().collect();
        maps.sort_unstable();
        // 하위 꿈들은 병합만 한다(skip_retire) — 은퇴·압축을 그룹마다 하면 노드
        // 번호가 재배열되어 다음 그룹 에피소드의 정착 상태 id가 무효가 된다.
        let sub_cfg = DreamConfig { skip_retire: true, ..cfg };
        for m in maps {
            let save_map = agent.active_map;
            agent.active_map = m;
            agent.episodes = by_map.remove(&m).unwrap();
            agent.episode_maps = vec![m; agent.episodes.len()];
            let sub = dream(agent, sub_cfg);
            agent.active_map = save_map;
            total.steps_used += sub.steps_used;
            total.iters = total.iters.max(sub.iters);
            total.final_ll = sub.final_ll;
            total.aligned += sub.aligned;
            total.created += sub.created;
        }
        // 은퇴는 밤의 끝에 한 번: 해마 흔적을 걷어내고 번호를 정리한다.
        let retire: std::collections::HashSet<u32> = agent.fresh_clones.iter().copied().collect();
        let n_total = agent.graph.n_nodes();
        let mut map = vec![u32::MAX; n_total];
        let mut next = 0u32;
        for id in 0..n_total as u32 {
            if !retire.contains(&id) {
                map[id as usize] = next;
                next += 1;
            }
        }
        agent.graph.compact(&map, next as usize);
        agent.apply_node_map(&map);
        agent.fresh_clones.clear();
        agent.episodes = vec![Vec::new()];
        agent.episode_maps = vec![agent.active_map];
        agent.reset_episode();
        agent.map_check_grace = 300; // 잠에서 깬 직후엔 세계를 의심하지 않는다
        // 밤의 끝: 지도 상태 스냅숏 + 에피소드 저널 소거(소화된 경험은 구조가 됐다)
        agent.ep_checkpoint();
        total.nodes_after = agent.graph.n_nodes();
        return total;
    }

    let episodes: Vec<&[EpStep]> = agent
        .episodes
        .iter()
        .filter(|e| e.len() >= 2)
        .map(|e| e.as_slice())
        .collect();
    let total_steps: usize = episodes.iter().map(|e| e.len()).sum();
    if total_steps < 8 {
        return rep;
    }
    rep.steps_used = total_steps;

    // 응고화 모드: 버퍼에 등장하는 지각만 상태 공간에 올린다(국소성 = 비용·간섭 차단).
    let mut in_buffer: std::collections::HashSet<u32> = Default::default();
    if cfg.consume {
        for ep in &episodes {
            for s in ep.iter() {
                in_buffer.insert(s.percept);
            }
        }
    }

    // 상태 예산: 지금 각성이 쓰는 클론 수(±)를 출발점으로, 상한을 씌운다.
    let ss = StateSpace::new(n_percepts, |p| {
        if cfg.consume {
            if in_buffer.contains(&(p as u32)) {
                let cur = agent.graph.clones_of(p as u32).len();
                cur.clamp(4, cfg.max_clones)
            } else {
                0
            }
        } else {
            let cur = agent.graph.clones_of(p as u32).len();
            cur.clamp(2, cfg.max_clones)
        }
    });

    // --- 초기 전이: 현재 그래프 카운트 + 유사카운트 + 대칭 깨기 ---
    // 응고화 모드에서는 그래프 카운트를 넣지 않는다 — 옛 환경의 구조를 이번 꿈에
    // 끌어들이면 국소성이 깨진다. 버퍼는 백지에서 스스로 정렬된다.
    //
    // 전면 재건 모드의 K-슬롯 배정은 **지각별 최근성 순위**로 한다. 원래 구현은
    // `clone_ix < K` 필터였는데, clone_ix는 생성 순번이라 여러 환경을 거치면
    // 후기 환경의 클론이 전부 잘려나가 초기값에서 사라진다 — 10환경 1차 시도에서
    // 후기 환경 습득이 붕괴한 숨은 원인이다.
    let mut rng = Rng::new(cfg.seed);
    let mut counts: HashMap<(u32, u16), HashMap<u32, f64>> = HashMap::new();
    if !cfg.consume {
        let mut slot_of: Vec<Option<u32>> = vec![None; agent.graph.n_nodes()];
        for p in 0..n_percepts as u32 {
            let mut cs: Vec<u32> = agent.graph.clones_of(p).to_vec();
            cs.sort_unstable_by_key(|&c| std::cmp::Reverse(agent.graph.node(c).atom.t));
            for (rank, &c) in cs.iter().enumerate() {
                if (rank as u32) < ss.k_of(p) {
                    slot_of[c as usize] = Some(rank as u32);
                }
            }
        }
        for from in 0..agent.graph.n_nodes() as u32 {
            let nf = agent.graph.node(from);
            let kf = match slot_of[from as usize] {
                Some(k) => k,
                None => continue,
            };
            let sf = ss.id(nf.percept, kf);
            for a in agent.graph.actions_from(from) {
                for s in agent.graph.succ(from, a) {
                    let nt = agent.graph.node(s.to);
                    let kt = match slot_of[s.to as usize] {
                        Some(k) => k,
                        None => continue,
                    };
                    let st = ss.id(nt.percept, kt);
                    *counts.entry((sf, a)).or_default().entry(st).or_insert(0.0) += s.count as f64;
                }
            }
        }
    }

    // 에피소드에 실제로 등장하는 (지각→지각, 행동) 조합에 대해 모든 클론 쌍을
    // 후보로 열어둔다 — EM이 재배치할 여지를 만든다.
    let mut pair_seen: std::collections::HashSet<(u32, u16, u32)> = Default::default();
    for ep in &episodes {
        for t in 1..ep.len() {
            pair_seen.insert((ep[t - 1].percept, ep[t].action, ep[t].percept));
        }
    }
    // 결정론: HashSet 순회 순서가 RNG 소비 순서로 새면 대칭 깨기 지터가
    // 프로세스마다 다르게 배정된다 — 정렬 목록으로 소비 순서를 고정.
    let mut pair_list: Vec<(u32, u16, u32)> = pair_seen.iter().copied().collect();
    pair_list.sort_unstable();
    let seed_pairs = |counts: &mut HashMap<(u32, u16), HashMap<u32, f64>>, rng: &mut Rng| {
        for &(pf, a, pt) in &pair_list {
            for kf in 0..ss.k_of(pf) {
                let sf = ss.id(pf, kf);
                let e = counts.entry((sf, a)).or_default();
                for kt in 0..ss.k_of(pt) {
                    let st = ss.id(pt, kt);
                    e.entry(st)
                        .or_insert(cfg.pseudo * (1.0 + cfg.jitter * rng.next_f64()));
                }
            }
        }
    };
    seed_pairs(&mut counts, &mut rng);

    let normalize = |counts: &HashMap<(u32, u16), HashMap<u32, f64>>| -> Trans {
        let mut map = HashMap::with_capacity(counts.len());
        for (&key, row) in counts {
            let sum: f64 = row.values().sum();
            if sum <= 0.0 {
                continue;
            }
            let mut v: Vec<(u32, f64)> = row.iter().map(|(&s, &c)| (s, c / sum)).collect();
            v.sort_unstable_by_key(|x| x.0);
            map.insert(key, v);
        }
        Trans { map }
    };

    // 응고화 모드는 그래프 초기값 없이 출발하므로 국소해에 민감하다 —
    // 무작위 재시작 여러 번 중 로그우도가 가장 높은 해를 채택한다(CSCG 방식).
    // (재시작 수가 꿈별 품질 분산을 직접 좌우함이 오라클 실험으로 확인됨:
    //  같은 코드가 어느 밤은 53노드, 어느 밤은 442노드 — 복불복을 돈으로 산다)
    let restarts = if cfg.consume { 10 } else { 1 };
    let iters = if cfg.consume { cfg.iters.max(50) } else { cfg.iters };
    let mut best_trans: Option<Trans> = None;
    let mut best_ll = f64::NEG_INFINITY;

    for restart in 0..restarts {
        if restart > 0 {
            counts = HashMap::new();
            let mut r2 = Rng::new(cfg.seed ^ (0x9E37 * restart as u64 + 1));
            seed_pairs(&mut counts, &mut r2);
        }
        let mut trans = normalize(&counts);
        let mut last_ll = f64::NEG_INFINITY;
        run_em(
            &episodes, &ss, &mut trans, &mut counts, cfg, iters, total_steps, &mut last_ll,
        );
        rep.iters = iters;
        if last_ll > best_ll {
            best_ll = last_ll;
            best_trans = Some(trans);
        }
    }
    let trans = best_trans.expect("EM 재시작이 하나도 없다");
    rep.final_ll = best_ll;

    #[allow(clippy::too_many_arguments)]
    fn run_em(
        episodes: &[&[crate::wake::EpStep]],
        ss: &StateSpace,
        trans: &mut Trans,
        counts: &mut HashMap<(u32, u16), HashMap<u32, f64>>,
        cfg: DreamConfig,
        iters: usize,
        total_steps: usize,
        out_ll: &mut f64,
    ) {
        for _it in 0..iters {
        let mut new_counts: HashMap<(u32, u16), HashMap<u32, f64>> = HashMap::new();
        let mut ll = 0.0f64;

        for ep in episodes {
            let tlen = ep.len();
            // 전방
            let mut alpha: Vec<Vec<f64>> = Vec::with_capacity(tlen);
            let mut scale: Vec<f64> = Vec::with_capacity(tlen);
            {
                let k0 = ss.k_of(ep[0].percept) as usize;
                let mut a0 = vec![1.0 / k0 as f64; k0];
                let s: f64 = a0.iter().sum();
                for v in &mut a0 {
                    *v /= s;
                }
                scale.push(s);
                alpha.push(a0);
            }
            for t in 1..tlen {
                let (pp, pc) = (ep[t - 1].percept, ep[t].percept);
                let act = ep[t].action;
                let kp = ss.k_of(pp);
                let kc = ss.k_of(pc) as usize;
                let mut at = vec![1e-300f64; kc];
                for kf in 0..kp {
                    let w = alpha[t - 1][kf as usize];
                    if w <= 0.0 {
                        continue;
                    }
                    let sf = ss.id(pp, kf);
                    if let Some(row) = trans.get(sf, act) {
                        let base = ss.offset[pc as usize];
                        for &(st, p) in row {
                            if st >= base && st < base + kc as u32 {
                                at[(st - base) as usize] += w * p;
                            }
                        }
                    }
                }
                let s: f64 = at.iter().sum();
                let s = if s > 0.0 { s } else { 1e-300 };
                for v in &mut at {
                    *v /= s;
                }
                scale.push(s);
                alpha.push(at);
                ll += s.ln();
            }
            // 후방 + 기대 카운트
            let mut beta = vec![1.0f64; ss.k_of(ep[tlen - 1].percept) as usize];
            for t in (1..tlen).rev() {
                let (pp, pc) = (ep[t - 1].percept, ep[t].percept);
                let act = ep[t].action;
                let kp = ss.k_of(pp) as usize;
                let kc = ss.k_of(pc) as usize;
                let base = ss.offset[pc as usize];
                let mut bprev = vec![1e-300f64; kp];
                for kf in 0..kp {
                    let sf = ss.id(pp, kf as u32);
                    let aw = alpha[t - 1][kf];
                    if let Some(row) = trans.get(sf, act) {
                        let mut acc = 0.0;
                        for &(st, p) in row {
                            if st >= base && st < base + kc as u32 {
                                let kt = (st - base) as usize;
                                let g = aw * p * beta[kt];
                                if g > 0.0 {
                                    *new_counts
                                        .entry((sf, act))
                                        .or_default()
                                        .entry(st)
                                        .or_insert(0.0) += g;
                                }
                                acc += p * beta[kt];
                            }
                        }
                        bprev[kf] = acc / scale[t];
                    }
                }
                beta = bprev;
            }
        }

        // M-스텝
        for row in new_counts.values_mut() {
            for v in row.values_mut() {
                *v += cfg.pseudo * 0.01;
            }
        }
        // 지역 정규화(외부 클로저는 중첩 fn에서 못 쓴다)
        let mut map = HashMap::with_capacity(new_counts.len());
        for (&key, row) in &new_counts {
            let sum: f64 = row.values().sum();
            if sum <= 0.0 {
                continue;
            }
            let mut v: Vec<(u32, f64)> = row.iter().map(|(&s, &c)| (s, c / sum)).collect();
            v.sort_unstable_by_key(|x| x.0);
            map.insert(key, v);
        }
        *trans = Trans { map };
        *counts = new_counts;
        *out_ll = ll / total_steps as f64;
        }
    }

    // --- Viterbi 재라벨링 → 그래프 재건 ---
    let mut usage: HashMap<u32, u32> = HashMap::new();
    let mut edge_counts: HashMap<(u32, u16, u32), u32> = HashMap::new();
    let mut vals: HashMap<u32, (Val, u32)> = HashMap::new();
    // 정렬 병합용: EM 상태 × 각성 정착 상태의 동시발생 (응고화 모드).
    let mut cooc: HashMap<u32, HashMap<u32, u32>> = HashMap::new();

    // 재주소화용: 에피소드별 Viterbi 경로와 (지각,행동) 사본(차용 수명 분리)
    let mut paths: Vec<Vec<u32>> = Vec::new();
    let mut ep_data: Vec<Vec<(u32, u16)>> = Vec::new();
    for ep in &episodes {
        let tlen = ep.len();
        let k0 = ss.k_of(ep[0].percept) as usize;
        let mut delta = vec![0.0f64; k0];
        let mut back: Vec<Vec<u32>> = Vec::with_capacity(tlen);
        back.push(Vec::new());
        for t in 1..tlen {
            let (pp, pc) = (ep[t - 1].percept, ep[t].percept);
            let act = ep[t].action;
            let kp = ss.k_of(pp) as usize;
            let kc = ss.k_of(pc) as usize;
            let base = ss.offset[pc as usize];
            let mut nd = vec![f64::NEG_INFINITY; kc];
            let mut nb = vec![0u32; kc];
            for kf in 0..kp {
                let sf = ss.id(pp, kf as u32);
                let dw = delta[kf];
                if let Some(row) = trans.get(sf, act) {
                    for &(st, p) in row {
                        if st >= base && st < base + kc as u32 {
                            let kt = (st - base) as usize;
                            let cand = dw + p.max(1e-300).ln();
                            if cand > nd[kt] {
                                nd[kt] = cand;
                                nb[kt] = kf as u32;
                            }
                        }
                    }
                }
            }
            // 수치 안정화
            let m = nd.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            if m.is_finite() {
                for v in &mut nd {
                    *v -= m;
                }
            }
            delta = nd;
            back.push(nb);
        }
        // 역추적
        let mut k = delta
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i as u32)
            .unwrap_or(0);
        let mut path = vec![0u32; tlen];
        path[tlen - 1] = ss.id(ep[tlen - 1].percept, k);
        for t in (1..tlen).rev() {
            k = back[t][k as usize];
            path[t - 1] = ss.id(ep[t - 1].percept, k);
        }
        for t in 0..tlen {
            *usage.entry(path[t]).or_insert(0) += 1;
            if let Some(v) = ep[t].val {
                let e = vals.entry(path[t]).or_insert((v, 0));
                e.0.absorb(&v, e.1);
                e.1 += 1;
            }
            if cfg.consume && ep[t].state != u32::MAX {
                *cooc.entry(path[t]).or_default().entry(ep[t].state).or_insert(0) += 1;
            }
            if t >= 1 {
                *edge_counts
                    .entry((path[t - 1], ep[t].action, path[t]))
                    .or_insert(0) += 1;
            }
        }
        ep_data.push(ep.iter().map(|s| (s.percept, s.action)).collect());
        paths.push(path);
    }

    if cfg.consume {
        // --- 응고화 4단계 ---
        // (0) **정렬**: EM이 재구성한 각 상태를, 같은 순간들을 함께 산 기존 피질
        //     상태와 짝짓는다(동시발생 ≥ 사용량의 절반 + 지각 일치). 짝이 있으면
        //     새 클론을 만들지 않고 그 상태에 증거를 흡수시킨다 — 중복이 생기는
        //     원천을 제거한다(가설 e). 각성이 헤매던 순간의 상태(해마 흔적·타지도
        //     오정착)는 짝이 못 되므로 자연히 걸러진다.
        // (1) 짝 없는 산출물만 새 클론으로 병합한다.
        // (2) **해마 흔적 은퇴**: 각성의 임시 클론 제거.
        // (3) 버퍼 소거 — 소화된 경험은 구조가 됐다.
        let fresh_set: std::collections::HashSet<u32> =
            agent.fresh_clones.iter().copied().collect();
        let mut new_id: HashMap<u32, u32> = HashMap::new();
        // (기록) 정렬 일대일 제약은 5-시드 A/B에서 기각됨: 다대일 흡수의 대부분은
        // 같은 참 상태의 EM 조각들이 재흡수되는 정당한 경로였고, 막으면 미정렬
        // 조각이 신규 클론으로 흘러 그래프가 폭증(1315~4174노드)·유지율 악화.
        let mut order: Vec<(&u32, &u32)> = usage.iter().collect();
        order.sort_unstable_by_key(|(s, _)| **s);
        for (&s, &u) in order {
            if u < cfg.min_usage {
                continue;
            }
            let percept = match ss.offset.binary_search(&s) {
                Ok(p) => p as u32,
                Err(i) => (i - 1) as u32,
            };
            // (0) 정렬 시도: 피질(비-해마) 상태 중 최대 동시발생
            // 상대 다수결: 피질 후보 중 최대 동시발생이 사용량의 1/4 이상이면 흡수.
            // (절대 과반 기준은 각성 재파편화 탓에 정착 이력이 흩어지면 미달 —
            //  시도 13에서 흡수 실패·재팽창의 원인으로 실측)
            let aligned = cooc.get(&s).and_then(|row| {
                row.iter()
                    .filter(|(&w, _)| {
                        (w as usize) < agent.graph.n_nodes()
                            && !fresh_set.contains(&w)
                            && agent.graph.node(w).percept == percept
                            // 정렬은 같은 지도 안에서만 — 혼란기 동시발생을 타고
                            // 다른 세계의 피질로 새어나가면 이번 지도의 셀이 소실된다
                            && agent.node_map.get(w as usize) == Some(&agent.active_map)
                    })
                    .max_by_key(|(&w, &c)| (c, std::cmp::Reverse(w)))
                    .filter(|(_, &c)| c * 2 >= u)
                    .map(|(&w, _)| w)
            });
            if let Some(w) = aligned {
                rep.aligned += 1;
                new_id.insert(s, w);
                let n = agent.graph.node_mut(w);
                n.atom.evidence = n.atom.evidence.saturating_add(u);
                if let Some((v, cnt)) = vals.get(&s) {
                    match &mut n.atom.value {
                        Some(old) => old.absorb(v, *cnt),
                        None => n.atom.value = Some(*v),
                    }
                }
                continue;
            }
            // 응고 상한은 **피질(비-해마) 클론만** 센다 — 이 자리에서 스크래치를
            // 세면, 곧 은퇴할 스크래치가 부풀린 카운트로 응고 자체가 거부된다
            // (위치 6+에서 "신규 0"으로 붕괴하던 하드캡 버그 — 순서 반전 실험으로 검거).
            let cortical = agent
                .graph
                .clones_of(percept)
                .iter()
                .filter(|&&c| !fresh_set.contains(&c))
                .count();
            if cortical >= 1024 {
                continue;
            }
            let id = agent.graph.new_clone(percept);
            rep.created += 1;
            // 응고화된 노드는 현재 꿈이 속한 지도의 것이다
            let i = id as usize;
            if agent.node_map.len() <= i {
                agent.node_map.resize(i + 1, 0);
            }
            agent.node_map[i] = agent.active_map;
            while agent.map_cortical.len() <= agent.active_map as usize { agent.map_cortical.push(0); }
            agent.map_cortical[agent.active_map as usize] += 1;
            new_id.insert(s, id);
            let n = agent.graph.node_mut(id);
            n.atom.evidence = u;
            if let Some((v, _)) = vals.get(&s) {
                n.atom.value = Some(*v);
            }
        }
        // 결정론: HashMap 순회 순서가 간선 삽입 순서로 새지 않게 키 정렬
        let mut ec: Vec<((u32, u16, u32), u32)> =
            edge_counts.iter().map(|(&k, &c)| (k, c)).collect();
        ec.sort_unstable_by_key(|&(k, _)| k);
        for ((f, a, t), c) in ec {
            if let (Some(&nf), Some(&nt)) = (new_id.get(&f), new_id.get(&t)) {
                agent.graph.link_n(nf, a, nt, c);
            }
        }

        // 밤의 재주소화: 응고 피질에 해마 주소(문맥 벡터)를 재등록한다.
        // 문맥 등록이 각성의 클론 생성 순간에만 있으므로, 은퇴가 스크래치와
        // 함께 주소록을 지우면 피질은 주소 없는 지식이 된다 — 복귀 측정에서
        // ctx 조회가 항상 빗나가 over-이벤트마다 클론이 신설되던(+350~760)
        // 원인. Viterbi 경로에서 각 노드의 이른 등장 순간 문맥을 재구성
        // (각성 context_vec와 동일 연산)해 노드당 밤당 최대 3개 등록한다.
        let order = agent.cfg.context_order;
        let mut reg_count: HashMap<u32, u32> = HashMap::new();
        let mut regs: Vec<(crate::sbv::Sbv, u32)> = Vec::new();
        for (ei, path) in paths.iter().enumerate() {
            let data = &ep_data[ei];
            for t in 0..path.len() {
                let node = match new_id.get(&path[t]) {
                    Some(&n) => n,
                    None => continue,
                };
                let c = reg_count.entry(node).or_insert(0);
                if *c >= 3 {
                    continue;
                }
                *c += 1;
                let mut b = crate::sbv::Bundler::new();
                b.add(&agent.graph.percept_vec(data[t].0));
                for i in 1..=order {
                    if t < i {
                        break;
                    }
                    let (pp, pa) = data[t - i];
                    let term = agent
                        .graph
                        .percept_vec(pp)
                        .bind(&crate::sbv::Sbv::from_seed(0xAC7100 ^ pa as u64));
                    b.add(&term.permute(i));
                }
                regs.push((b.finalize(), node));
            }
        }
        for (v, node) in regs {
            agent.register_context(v, node);
        }

        // (2)(3) 은퇴와 버퍼 소거 — 지도별 하위 꿈에서는 미룬다(호출자가 한 번에).
        if !cfg.skip_retire {
            let retire: std::collections::HashSet<u32> =
                agent.fresh_clones.iter().copied().collect();
            let n_total = agent.graph.n_nodes();
            let mut map = vec![u32::MAX; n_total];
            let mut next = 0u32;
            for id in 0..n_total as u32 {
                if !retire.contains(&id) {
                    map[id as usize] = next;
                    next += 1;
                }
            }
            agent.graph.compact(&map, next as usize);
            agent.apply_node_map(&map);
            agent.fresh_clones.clear();
            agent.episodes = vec![Vec::new()];
            agent.reset_episode();
            agent.map_check_grace = 300;
            // 밤의 끝: 지도 상태 스냅숏 + 에피소드 저널 소거(소화된 경험은 구조가 됐다)
            agent.ep_checkpoint();
        }
        rep.nodes_after = agent.graph.n_nodes();
        return rep;
    }

    // 새 그래프: 쓰인 상태만 노드로 (전면 재건 모드)
    let mut new_id: HashMap<u32, u32> = HashMap::new();
    let mut g = WorldGraph::new();
    for p in 0..n_percepts as u32 {
        let v = agent.graph.percept_vec(p);
        let np = g.intern_percept(&v, 0);
        debug_assert_eq!(np, p);
    }
    let mut order: Vec<(&u32, &u32)> = usage.iter().collect();
    order.sort_unstable_by_key(|(s, _)| **s);
    for (&s, &u) in order {
        if u < cfg.min_usage {
            continue;
        }
        // 상태 → 지각 역산
        let percept = match ss.offset.binary_search(&s) {
            Ok(p) => p as u32,
            Err(i) => (i - 1) as u32,
        };
        let id = g.new_clone(percept);
        new_id.insert(s, id);
        let n = g.node_mut(id);
        n.atom.evidence = u;
        if let Some((v, _)) = vals.get(&s) {
            n.atom.value = Some(*v);
        }
    }
    // 결정론: HashMap 순회 순서가 간선 삽입 순서로 새지 않게 키 정렬
    let mut ec: Vec<((u32, u16, u32), u32)> =
        edge_counts.iter().map(|(&k, &c)| (k, c)).collect();
    ec.sort_unstable_by_key(|&(k, _)| k);
    for ((f, a, t), c) in ec {
        if let (Some(&nf), Some(&nt)) = (new_id.get(&f), new_id.get(&t)) {
            for _ in 0..c {
                g.link(nf, a, nt);
            }
        }
    }
    g.tick = agent.graph.tick;

    agent.graph = g;
    agent.after_remap();

    rep.nodes_after = agent.graph.n_nodes();
    rep
}
