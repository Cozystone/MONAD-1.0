//! 수면 패스 (Sleep) — C1 베이지안 모델 축약(BMR).
//!
//! PRD §4.5: **압축이 곧 추상화다.**
//!
//! 각성은 확신 없이 넉넉히 갈라둔다. 같은 곳을 다른 문맥으로 만났다는 이유만으로
//! 새 클론을 만든다 — 1-shot이므로 숙고할 시간이 없다. 그 결과 그래프는 실제
//! 세계보다 훨씬 잘게 쪼개져 있다.
//!
//! 수면은 그 과분할을 되돌린다. **행동이 구별되지 않는 두 상태는 같은 상태다.**
//! 이것은 오토마타 최소화(bisimulation)와 같은 문제이며, 여기서는 데이터가
//! 불완전하다는 점만 다르다(아직 해보지 않은 행동이 있다).
//!
//! ```text
//! 각성:  1177개 상태 (같은 칸을 문맥마다 다르게 봄)
//! 수면:  →  36개 상태 (방의 실제 칸 수)
//! ```
//!
//! 이 압축이 일반화다. 병합된 상태는 여러 문맥에서 얻은 전이를 공유하므로,
//! 한 문맥에서 배운 것이 다른 문맥으로 **전이**된다.

use crate::graph::WorldGraph;

#[derive(Clone, Copy, Debug)]
pub struct SleepConfig {
    /// 이 횟수 미만으로 관측된 전이는 잡음으로 보고 걷어낸다.
    pub min_edge_count: u32,
    /// 이 횟수 미만으로 방문된 상태는 지운다(고아 제거).
    pub min_evidence: u32,
    /// 병합 반복 상한.
    pub max_rounds: usize,
    /// 두 상태를 비교할 때 최소 이만큼의 공통 행동에서 일치해야 병합한다.
    /// 0이면 근거 없이도 합치므로 위험하다.
    pub min_shared_actions: usize,
    /// 행동 수(비교 대상 범위).
    pub n_actions: u16,
}

impl Default for SleepConfig {
    fn default() -> Self {
        SleepConfig {
            min_edge_count: 1,
            min_evidence: 1,
            max_rounds: 12,
            min_shared_actions: 1,
            n_actions: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SleepReport {
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub edges_before: usize,
    pub edges_after: usize,
    pub edges_pruned: usize,
    pub rounds: usize,
    /// 압축률 = after / before.
    pub ratio: f32,
}

/// 유니온-파인드.
struct Uf {
    p: Vec<u32>,
}
impl Uf {
    fn new(n: usize) -> Self {
        Uf { p: (0..n as u32).collect() }
    }
    fn find(&mut self, x: u32) -> u32 {
        let mut r = x;
        while self.p[r as usize] != r {
            r = self.p[r as usize];
        }
        let mut c = x;
        while self.p[c as usize] != r {
            let n = self.p[c as usize];
            self.p[c as usize] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: u32, b: u32) -> bool {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        // 낮은 번호로 합쳐 결정론을 유지한다
        if ra < rb {
            self.p[rb as usize] = ra;
        } else {
            self.p[ra as usize] = rb;
        }
        true
    }
}

const UNKNOWN: i64 = -1;

/// 상태 s에서 행동 a의 최빈 후속이 속한 블록. 간선이 없으면 UNKNOWN.
#[inline]
fn succ_block(g: &WorldGraph, block: &[u32], s: u32, a: u16) -> i64 {
    let list = g.succ(s, a);
    match list.iter().max_by_key(|x| (x.count, std::cmp::Reverse(x.to))) {
        Some(best) => block[best.to as usize] as i64,
        None => UNKNOWN,
    }
}

/// 수면 압축을 수행한다.
///
/// # 왜 '분할 정련'인가
///
/// 합치기만 하는 알고리즘(유니온-파인드)으로 시작하면 처음에 모든 노드가 따로
/// 있으므로, 두 노드가 "같은 곳으로 간다"를 확인할 방법이 없다 — 아직 아무것도
/// 합쳐지지 않았으니 후속도 전부 다른 것으로 보인다. 닭과 달걀 문제다.
///
/// 그래서 반대로 간다: **처음엔 같은 지각을 전부 한 덩어리로 놓고**, 행동으로
/// 구별되는 것이 드러날 때만 쪼갠다. 오토마타 최소화(Moore/Hopcroft)와 같은 절차이며,
/// 안정점이 곧 가장 거친 bisimulation — 즉 구별 불가능한 것들을 최대한 합친 지도다.
///
/// 반환값의 `map[old] = new`로 상위 계층(문맥 색인·선호)이 자기 상태를 갱신한다.
pub fn consolidate(g: &mut WorldGraph, cfg: SleepConfig) -> (SleepReport, Vec<u32>) {
    consolidate_grouped(g, cfg, None)
}

/// `groups[node]`가 주어지면 같은 그룹(지도) 안에서만 병합한다 — 다른 세계에서
/// 우연히 같은 행동을 하는 상태를 합치면 세계가 섞인다.
pub fn consolidate_grouped(
    g: &mut WorldGraph,
    cfg: SleepConfig,
    groups: Option<&[u32]>,
) -> (SleepReport, Vec<u32>) {
    let mut rep = SleepReport {
        nodes_before: g.n_nodes(),
        edges_before: g.n_edges(),
        ..Default::default()
    };

    if cfg.min_edge_count > 1 {
        rep.edges_pruned = g.prune_edges(cfg.min_edge_count);
    }

    let n = g.n_nodes();
    if n == 0 {
        return (rep, Vec::new());
    }

    // --- 1단계: 분할 정련 (거친 데서 시작해 구별될 때만 쪼갠다) ---
    // 초기 블록 = (지각, 그룹). 그룹이 없으면 지각만.
    let mut seed_id: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    let mut block: Vec<u32> = (0..n)
        .map(|i| {
            let p = g.node(i as u32).percept;
            let grp = groups.and_then(|gs| gs.get(i).copied()).unwrap_or(0);
            let next = seed_id.len() as u32;
            *seed_id.entry((p, grp)).or_insert(next)
        })
        .collect();
    let mut n_blocks = seed_id.len();

    for round in 0..cfg.max_rounds {
        let mut sig_id: std::collections::HashMap<Vec<i64>, u32> = std::collections::HashMap::new();
        let mut next = vec![0u32; n];
        for id in 0..n {
            let mut sig: Vec<i64> = Vec::with_capacity(cfg.n_actions as usize + 1);
            sig.push(block[id] as i64);
            for a in 0..cfg.n_actions {
                sig.push(succ_block(g, &block, id as u32, a));
            }
            let k = sig_id.len() as u32;
            next[id] = *sig_id.entry(sig).or_insert(k);
        }
        let m = sig_id.len();
        block = next;
        rep.rounds = round + 1;
        if m == n_blocks {
            break; // 안정
        }
        n_blocks = m;
    }

    // --- 2단계: 와일드카드 병합 ---
    // 아직 해보지 않은 행동(UNKNOWN)은 반증이 아니다. 1단계는 그것까지 서명에
    // 넣어 쪼개므로, 여기서 "겪어본 행동에서 서로 어긋나지 않는" 블록들을 되붙인다.
    let mut rows: std::collections::HashMap<u32, Vec<i64>> = std::collections::HashMap::new();
    let mut percept_of_block: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut group_of_block: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    for id in 0..n {
        let b = block[id];
        percept_of_block.entry(b).or_insert_with(|| g.node(id as u32).percept);
        group_of_block
            .entry(b)
            .or_insert_with(|| groups.and_then(|gs| gs.get(id).copied()).unwrap_or(0));
        let row = rows.entry(b).or_insert_with(|| vec![UNKNOWN; cfg.n_actions as usize]);
        for a in 0..cfg.n_actions as usize {
            let s = succ_block(g, &block, id as u32, a as u16);
            if s != UNKNOWN {
                row[a] = s; // 같은 블록 안에서는 일치한다고 본다(1단계가 보장)
            }
        }
    }
    let mut ids: Vec<u32> = rows.keys().copied().collect();
    ids.sort_unstable();
    let idx: std::collections::HashMap<u32, usize> =
        ids.iter().enumerate().map(|(i, &b)| (b, i)).collect();
    let mut uf = Uf::new(ids.len());
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let (bi, bj) = (ids[i], ids[j]);
            if percept_of_block[&bi] != percept_of_block[&bj]
                || group_of_block[&bi] != group_of_block[&bj]
            {
                continue;
            }
            let (ri, rj) = (&rows[&bi], &rows[&bj]);
            let mut shared = 0usize;
            let mut ok = true;
            for a in 0..cfg.n_actions as usize {
                match (ri[a], rj[a]) {
                    (UNKNOWN, _) | (_, UNKNOWN) => {}
                    (x, y) => {
                        if x != y {
                            ok = false;
                            break;
                        }
                        shared += 1;
                    }
                }
            }
            if ok && shared >= cfg.min_shared_actions {
                uf.union(idx[&bi] as u32, idx[&bj] as u32);
            }
        }
    }

    // --- 3단계: 번호 재배열 + 증거 부족 노드 정리 ---
    let mut final_of: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut next_id = 0u32;
    let mut keep: Vec<bool> = vec![false; ids.len()];
    for id in 0..n {
        if g.node(id as u32).atom.evidence >= cfg.min_evidence {
            keep[idx[&block[id]]] = true;
        }
    }
    let mut map = vec![u32::MAX; n];
    for id in 0..n {
        let root = uf.find(idx[&block[id]] as u32);
        if !keep[root as usize] && !keep[idx[&block[id]]] {
            continue;
        }
        let nid = *final_of.entry(root).or_insert_with(|| {
            let v = next_id;
            next_id += 1;
            v
        });
        map[id] = nid;
    }

    g.compact(&map, next_id as usize);

    rep.nodes_after = g.n_nodes();
    rep.edges_after = g.n_edges();
    rep.ratio = if rep.nodes_before == 0 {
        1.0
    } else {
        rep.nodes_after as f32 / rep.nodes_before as f32
    };
    (rep, map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::Obs;
    use crate::wake::{Agent, Config};

    #[test]
    fn merges_duplicate_states_in_a_ring() {
        // 4칸 고리를 문맥 과분할이 일어나도록 여러 지점에서 끊어가며 돌린다
        let mut a = Agent::with_config(Config { context_order: 2, ..Config::default() });
        let role = 0u16;
        a.encoder.declare(role, "cell");
        for start in 0..4usize {
            a.reset_episode();
            for step in 0..24usize {
                a.perceive(&Obs::new().cat(role, ((start + step) % 4) as u32), 0);
            }
        }
        let before = a.graph.n_nodes();
        let (rep, _) = consolidate(
            &mut a.graph,
            SleepConfig { n_actions: 1, ..SleepConfig::default() },
        );
        assert_eq!(rep.nodes_before, before);
        // 요구는 "최종 지도가 최소"다. 각성이 이미 최소로 만들었다면 병합할 것이
        // 없는 것도 정답이고, 과분할했다면 병합으로 줄어야 한다.
        assert!(
            rep.nodes_after <= 6,
            "4칸 고리가 {}개로 남았다(압축 실패)",
            rep.nodes_after
        );
        assert!(rep.nodes_after <= rep.nodes_before);
    }

    #[test]
    fn keeps_distinguishable_states_apart() {
        // A→B, A→C 두 갈래는 서로 다른 상태이므로 합쳐지면 안 된다
        let mut g = WorldGraph::new();
        let mut r = crate::rng::Rng::new(1);
        let pa = g.intern_percept(&crate::sbv::Sbv::random(&mut r), 32);
        let pb = g.intern_percept(&crate::sbv::Sbv::random(&mut r), 32);
        let pc = g.intern_percept(&crate::sbv::Sbv::random(&mut r), 32);
        let a1 = g.new_clone(pa);
        let a2 = g.new_clone(pa);
        let b = g.new_clone(pb);
        let c = g.new_clone(pc);
        for _ in 0..5 {
            g.link(a1, 0, b);
            g.link(a2, 0, c);
            g.visit(a1, None);
            g.visit(a2, None);
            g.visit(b, None);
            g.visit(c, None);
        }
        let (rep, map) = consolidate(&mut g, SleepConfig { n_actions: 1, ..Default::default() });
        assert_eq!(rep.nodes_after, 4, "구별되는 상태를 합쳤다");
        assert_ne!(map[a1 as usize], map[a2 as usize]);
    }
}
