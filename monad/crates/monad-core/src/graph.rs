//! A3 — 클론 세계 그래프 (Clone-Structured World Graph).
//!
//! PRD §4.2: **기억과 세계모델이 같은 것이다.** 별도의 가중치 행렬도, 벡터DB도,
//! KV캐시도 없다. 노드는 잠재 상태, 간선은 카운트 붙은 전이 `N(s,a→s')`.
//!
//! # 클론이란
//!
//! 같은 것을 보고 있어도 문맥이 다르면 다른 상태다. 복도 A의 파란 문과 복도 B의
//! 파란 문은 **같은 지각, 다른 상태**다. 이 구분을 못 하면 세계 지도를 만들 수 없다.
//!
//! 그래서 노드는 (지각, 클론 번호) 쌍이고, 예측이 반복해서 틀리는 자리에서
//! 새 클론을 즉석 할당한다(B3). 이것이 **1-shot 구조 학습** — 경사하강도,
//! 에폭도, 학습률도 없이 구조 자체가 자란다.
//!
//! # 상태 벡터
//!
//! `state_id = bind(percept_prototype, clone_tag)`
//!
//! 바인딩은 거리 보존 사상이므로(A1 테스트 `bind_preserves_distance`) 같은 지각의
//! 클론들은 서로 멀고, 각 클론은 고유한 정체성을 갖는다. 동시에 `unbind`로
//! 언제든 지각을 되찾을 수 있다 — 상태가 무엇을 보고 있는지 검사 가능하다(유리상자).

use crate::atom::{Atom, Val};
use crate::rng::hash64;
use crate::sbv::{Sbv, SBV_BYTES};
use crate::store::Store;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// 지각 원형과 관측이 같다고 볼 최대 해밍 거리.
/// A1 측정(최대 112블록 손상까지 식별 가능)에 비하면 매우 보수적인 값.
pub const PERCEPT_TOL: u32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Succ {
    pub to: u32,
    pub count: u32,
}

/// 세계 그래프의 노드 = 하나의 잠재 상태(클론).
#[derive(Clone, Debug)]
pub struct Node {
    /// 이 클론이 대응하는 지각의 번호.
    pub percept: u32,
    /// 같은 지각 안에서의 클론 번호(0부터).
    pub clone_ix: u32,
    /// 상태 원자. `id`가 상태 벡터, `value`가 연속량, `evidence`가 방문 횟수.
    pub atom: Atom,
}

#[inline]
fn edge_key(from: u32, action: u16) -> u64 {
    ((from as u64) << 16) | action as u64
}

/// 클론 태그 — 결정론적이므로 스냅숏 없이도 상태 벡터를 재구성할 수 있다.
#[inline]
fn clone_tag(percept: u32, clone_ix: u32) -> Sbv {
    Sbv::from_seed(hash64(&[
        (percept & 0xff) as u8,
        ((percept >> 8) & 0xff) as u8,
        ((percept >> 16) & 0xff) as u8,
        ((percept >> 24) & 0xff) as u8,
        (clone_ix & 0xff) as u8,
        ((clone_ix >> 8) & 0xff) as u8,
        ((clone_ix >> 16) & 0xff) as u8,
        ((clone_ix >> 24) & 0xff) as u8,
    ]))
}

pub struct WorldGraph {
    /// 지각 원형들(연상 색인 포함).
    percepts: Store,
    percept_vecs: Vec<Sbv>,
    /// 지각 번호 → 그 지각의 클론 노드 목록.
    clones: Vec<Vec<u32>>,
    /// 모든 노드.
    nodes: Vec<Node>,
    /// 상태 벡터 연상 색인(계획·스키마 매칭이 쓴다).
    states: Store,
    /// (from, action) → 후속 상태 분포.
    edges: HashMap<u64, Vec<Succ>>,
    /// 논리 시계.
    pub tick: u64,
    journal: Option<BufWriter<File>>,
    dir: Option<PathBuf>,
    dirty: u32,
}

// ------------------------------------------------------------------- 저널 기록

const REC_PERCEPT: u8 = 1;
const REC_CLONE: u8 = 2;
const REC_LINK: u8 = 3;
const REC_VALUE: u8 = 4;

const SNAP_MAGIC: &[u8; 8] = b"MONADG01";

impl WorldGraph {
    pub fn new() -> Self {
        WorldGraph {
            percepts: Store::new(),
            percept_vecs: Vec::new(),
            clones: Vec::new(),
            nodes: Vec::new(),
            states: Store::new(),
            edges: HashMap::new(),
            tick: 0,
            journal: None,
            dir: None,
            dirty: 0,
        }
    }

    // ---------------------------------------------------------------- 조회

    pub fn n_percepts(&self) -> usize {
        self.percept_vecs.len()
    }
    pub fn n_nodes(&self) -> usize {
        self.nodes.len()
    }
    pub fn n_edges(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }
    pub fn node(&self, id: u32) -> &Node {
        &self.nodes[id as usize]
    }
    pub fn node_mut(&mut self, id: u32) -> &mut Node {
        &mut self.nodes[id as usize]
    }
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
    pub fn percept_vec(&self, p: u32) -> Sbv {
        self.percept_vecs[p as usize]
    }
    pub fn clones_of(&self, percept: u32) -> &[u32] {
        &self.clones[percept as usize]
    }
    pub fn states(&self) -> &Store {
        &self.states
    }

    /// 관측과 가장 가까운 지각 원형. 없으면 None.
    pub fn match_percept(&self, obs: &Sbv, tol: u32) -> Option<u32> {
        self.percepts.nearest_within(obs, tol).map(|h| h.id)
    }

    /// 관측을 지각 번호로 정규화한다. 아는 것이 없으면 새 원형을 만든다.
    /// 이 단계가 "잡음 섞인 감각 → 안정된 심볼"의 경계다.
    pub fn intern_percept(&mut self, obs: &Sbv, tol: u32) -> u32 {
        if let Some(p) = self.match_percept(obs, tol) {
            return p;
        }
        let p = self.percept_vecs.len() as u32;
        self.percept_vecs.push(*obs);
        self.percepts.insert(p, *obs);
        self.clones.push(Vec::new());
        self.log_percept(obs);
        p
    }

    /// 새 클론을 할당한다 — 구조 학습의 원자적 행위.
    pub fn new_clone(&mut self, percept: u32) -> u32 {
        let ix = self.clones[percept as usize].len() as u32;
        let id = self.nodes.len() as u32;
        let sid = self.percept_vecs[percept as usize].bind(&clone_tag(percept, ix));
        let mut atom = Atom::new(sid);
        atom.evidence = 0;
        atom.t = self.tick;
        self.nodes.push(Node { percept, clone_ix: ix, atom });
        self.clones[percept as usize].push(id);
        self.states.insert(id, sid);
        self.log_clone(percept);
        id
    }

    /// 상태 방문 기록(증거 누적 + 연속량 흡수).
    pub fn visit(&mut self, id: u32, value: Option<Val>) {
        let t = self.tick;
        let n = &mut self.nodes[id as usize];
        n.atom.observe(value, t);
        if let Some(v) = value {
            self.log_value(id, &v);
        }
    }

    /// 전이 관측: `from`에서 `action`을 했더니 `to`가 되더라. 카운트 +1.
    /// **이것이 학습의 전부다.**
    pub fn link(&mut self, from: u32, action: u16, to: u32) {
        let e = self.edges.entry(edge_key(from, action)).or_default();
        if let Some(s) = e.iter_mut().find(|s| s.to == to) {
            s.count += 1;
        } else {
            e.push(Succ { to, count: 1 });
        }
        self.log_link(from, action, to);
    }

    /// 카운트 일괄 추가(꿈 병합용 — 저널에는 개별 기록 생략).
    pub fn link_n(&mut self, from: u32, action: u16, to: u32, count: u32) {
        if count == 0 {
            return;
        }
        let e = self.edges.entry(edge_key(from, action)).or_default();
        if let Some(s) = e.iter_mut().find(|s| s.to == to) {
            s.count = s.count.saturating_add(count);
        } else {
            e.push(Succ { to, count });
        }
    }

    /// 전이 한 건을 되돌린다(회고적 클론 분화가 쓴다).
    ///
    /// 놀라움이 나면 잘못된 것은 방금 본 것이 아니라 **직전 상태의 정체성**이다.
    /// 그 상태를 갈라내고 들어오던 간선을 새 클론으로 옮길 때 이 연산이 필요하다.
    pub fn unlink(&mut self, from: u32, action: u16, to: u32) {
        if let Some(e) = self.edges.get_mut(&edge_key(from, action)) {
            if let Some(p) = e.iter().position(|s| s.to == to) {
                if e[p].count > 1 {
                    e[p].count -= 1;
                } else {
                    e.swap_remove(p);
                }
            }
        }
    }

    pub fn succ(&self, from: u32, action: u16) -> &[Succ] {
        self.edges
            .get(&edge_key(from, action))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// 가장 그럴듯한 다음 상태와 그 확률.
    pub fn predict(&self, from: u32, action: u16) -> Option<(u32, f32)> {
        let s = self.succ(from, action);
        if s.is_empty() {
            return None;
        }
        let total: u32 = s.iter().map(|x| x.count).sum();
        let best = s.iter().max_by_key(|x| (x.count, std::cmp::Reverse(x.to))).unwrap();
        Some((best.to, best.count as f32 / total as f32))
    }

    /// 이 상태에서 시도해 본 적 있는 행동들.
    pub fn actions_from(&self, from: u32) -> Vec<u16> {
        let hi = (from as u64) << 16;
        let mut out: Vec<u16> = self
            .edges
            .keys()
            .filter(|&&k| (k & !0xffff) == hi)
            .map(|&k| (k & 0xffff) as u16)
            .collect();
        out.sort_unstable();
        out
    }

    /// 전이 분포의 예측 엔트로피(비트) — 정보 이득 항(B4)이 쓴다.
    pub fn entropy(&self, from: u32, action: u16) -> f32 {
        let s = self.succ(from, action);
        if s.is_empty() {
            return 1.0; // 모르는 것은 최대 불확실
        }
        let total: f32 = s.iter().map(|x| x.count as f32).sum();
        let mut h = 0.0;
        for x in s {
            let p = x.count as f32 / total;
            if p > 0.0 {
                h -= p * p.log2();
            }
        }
        h
    }

    // --------------------------------------------------------- 지속성(저널)

    /// 디스크에 붙인다. 기존 스냅숏·저널이 있으면 복원한다.
    pub fn attach(dir: impl AsRef<Path>) -> io::Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let mut g = WorldGraph::new();
        let snap = dir.join("graph.snap");
        if snap.exists() {
            g.load_snapshot(&snap)?;
        }
        let jpath = dir.join("graph.journal");
        if jpath.exists() {
            let mut buf = Vec::new();
            File::open(&jpath)?.read_to_end(&mut buf)?;
            g.replay(&buf);
        }
        g.dir = Some(dir.clone());
        g.journal = Some(BufWriter::new(
            OpenOptions::new().create(true).append(true).open(&jpath)?,
        ));
        Ok(g)
    }

    /// 저널을 OS로 밀어낸다. 이후 프로세스가 kill -9 되어도 데이터는 남는다.
    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(j) = self.journal.as_mut() {
            j.flush()?;
        }
        Ok(())
    }

    /// 스냅숏을 쓰고 저널을 비운다.
    pub fn checkpoint(&mut self) -> io::Result<()> {
        let dir = match &self.dir {
            Some(d) => d.clone(),
            None => return Ok(()),
        };
        let tmp = dir.join("graph.snap.tmp");
        self.write_snapshot(&tmp)?;
        std::fs::rename(&tmp, dir.join("graph.snap"))?;
        // 저널 비우기
        self.journal = None;
        let jpath = dir.join("graph.journal");
        File::create(&jpath)?;
        self.journal = Some(BufWriter::new(
            OpenOptions::new().create(true).append(true).open(&jpath)?,
        ));
        self.dirty = 0;
        Ok(())
    }

    fn write_snapshot(&self, path: &Path) -> io::Result<()> {
        let mut w = BufWriter::new(File::create(path)?);
        w.write_all(SNAP_MAGIC)?;
        w.write_all(&self.tick.to_le_bytes())?;
        w.write_all(&(self.percept_vecs.len() as u32).to_le_bytes())?;
        for v in &self.percept_vecs {
            w.write_all(v.as_bytes())?;
        }
        w.write_all(&(self.nodes.len() as u32).to_le_bytes())?;
        for n in &self.nodes {
            w.write_all(&n.percept.to_le_bytes())?;
            w.write_all(&n.atom.evidence.to_le_bytes())?;
            w.write_all(&n.atom.t.to_le_bytes())?;
            match &n.atom.value {
                Some(v) => {
                    w.write_all(&[v.used])?;
                    for i in 0..v.used as usize {
                        w.write_all(&v.v[i].to_le_bytes())?;
                    }
                }
                None => w.write_all(&[0xff])?,
            }
        }
        let mut keys: Vec<&u64> = self.edges.keys().collect();
        keys.sort_unstable();
        w.write_all(&(self.edges.values().map(|v| v.len()).sum::<usize>() as u32).to_le_bytes())?;
        for k in keys {
            let from = (k >> 16) as u32;
            let action = (k & 0xffff) as u16;
            for s in &self.edges[k] {
                w.write_all(&from.to_le_bytes())?;
                w.write_all(&action.to_le_bytes())?;
                w.write_all(&s.to.to_le_bytes())?;
                w.write_all(&s.count.to_le_bytes())?;
            }
        }
        w.flush()
    }

    fn load_snapshot(&mut self, path: &Path) -> io::Result<()> {
        let mut buf = Vec::new();
        File::open(path)?.read_to_end(&mut buf)?;
        if buf.len() < 8 || &buf[..8] != SNAP_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "스냅숏 서명 불일치"));
        }
        let mut p = 8usize;
        let rd_u32 = |b: &[u8], p: &mut usize| -> u32 {
            let v = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
            *p += 4;
            v
        };
        let rd_u16 = |b: &[u8], p: &mut usize| -> u16 {
            let v = u16::from_le_bytes(b[*p..*p + 2].try_into().unwrap());
            *p += 2;
            v
        };
        let rd_u64 = |b: &[u8], p: &mut usize| -> u64 {
            let v = u64::from_le_bytes(b[*p..*p + 8].try_into().unwrap());
            *p += 8;
            v
        };

        self.tick = rd_u64(&buf, &mut p);
        let np = rd_u32(&buf, &mut p) as usize;
        for i in 0..np {
            let arr: [u8; SBV_BYTES] = buf[p..p + SBV_BYTES].try_into().unwrap();
            p += SBV_BYTES;
            let v = Sbv::from_bytes(&arr);
            self.percept_vecs.push(v);
            self.percepts.insert(i as u32, v);
            self.clones.push(Vec::new());
        }
        let nn = rd_u32(&buf, &mut p) as usize;
        for id in 0..nn {
            let percept = rd_u32(&buf, &mut p);
            let evidence = rd_u32(&buf, &mut p);
            let t = rd_u64(&buf, &mut p);
            let used = buf[p];
            p += 1;
            let value = if used == 0xff {
                None
            } else {
                let mut v = Val::default();
                for i in 0..used as usize {
                    v.v[i] = f32::from_le_bytes(buf[p..p + 4].try_into().unwrap());
                    p += 4;
                }
                v.used = used;
                Some(v)
            };
            let ix = self.clones[percept as usize].len() as u32;
            let sid = self.percept_vecs[percept as usize].bind(&clone_tag(percept, ix));
            let atom = Atom { id: sid, value, evidence, t };
            self.nodes.push(Node { percept, clone_ix: ix, atom });
            self.clones[percept as usize].push(id as u32);
            self.states.insert(id as u32, sid);
        }
        let ne = rd_u32(&buf, &mut p) as usize;
        for _ in 0..ne {
            let from = rd_u32(&buf, &mut p);
            let action = rd_u16(&buf, &mut p);
            let to = rd_u32(&buf, &mut p);
            let count = rd_u32(&buf, &mut p);
            self.edges.entry(edge_key(from, action)).or_default().push(Succ { to, count });
        }
        Ok(())
    }

    fn replay(&mut self, buf: &[u8]) {
        let mut p = 0usize;
        while p < buf.len() {
            let tag = buf[p];
            p += 1;
            match tag {
                REC_PERCEPT => {
                    if p + SBV_BYTES > buf.len() {
                        break;
                    }
                    let arr: [u8; SBV_BYTES] = buf[p..p + SBV_BYTES].try_into().unwrap();
                    p += SBV_BYTES;
                    let v = Sbv::from_bytes(&arr);
                    let id = self.percept_vecs.len() as u32;
                    self.percept_vecs.push(v);
                    self.percepts.insert(id, v);
                    self.clones.push(Vec::new());
                }
                REC_CLONE => {
                    if p + 4 > buf.len() {
                        break;
                    }
                    let percept = u32::from_le_bytes(buf[p..p + 4].try_into().unwrap());
                    p += 4;
                    if (percept as usize) >= self.clones.len() {
                        break;
                    }
                    let ix = self.clones[percept as usize].len() as u32;
                    let id = self.nodes.len() as u32;
                    let sid = self.percept_vecs[percept as usize].bind(&clone_tag(percept, ix));
                    let mut atom = Atom::new(sid);
                    atom.evidence = 0;
                    self.nodes.push(Node { percept, clone_ix: ix, atom });
                    self.clones[percept as usize].push(id);
                    self.states.insert(id, sid);
                }
                REC_LINK => {
                    if p + 10 > buf.len() {
                        break;
                    }
                    let from = u32::from_le_bytes(buf[p..p + 4].try_into().unwrap());
                    let action = u16::from_le_bytes(buf[p + 4..p + 6].try_into().unwrap());
                    let to = u32::from_le_bytes(buf[p + 6..p + 10].try_into().unwrap());
                    p += 10;
                    let e = self.edges.entry(edge_key(from, action)).or_default();
                    if let Some(s) = e.iter_mut().find(|s| s.to == to) {
                        s.count += 1;
                    } else {
                        e.push(Succ { to, count: 1 });
                    }
                    if (from as usize) < self.nodes.len() {
                        self.nodes[from as usize].atom.evidence =
                            self.nodes[from as usize].atom.evidence.saturating_add(0);
                    }
                }
                REC_VALUE => {
                    if p + 5 > buf.len() {
                        break;
                    }
                    let id = u32::from_le_bytes(buf[p..p + 4].try_into().unwrap());
                    let used = buf[p + 4] as usize;
                    p += 5;
                    if p + used * 4 > buf.len() || (id as usize) >= self.nodes.len() {
                        break;
                    }
                    let mut v = Val::default();
                    for i in 0..used {
                        v.v[i] = f32::from_le_bytes(buf[p..p + 4].try_into().unwrap());
                        p += 4;
                    }
                    v.used = used as u8;
                    let n = &mut self.nodes[id as usize];
                    n.atom.observe(Some(v), n.atom.t);
                }
                _ => break, // 손상된 꼬리 — 여기까지만 복원한다
            }
        }
    }

    fn log_percept(&mut self, v: &Sbv) {
        if let Some(j) = self.journal.as_mut() {
            let _ = j.write_all(&[REC_PERCEPT]);
            let _ = j.write_all(v.as_bytes());
            self.dirty += 1;
        }
    }
    fn log_clone(&mut self, percept: u32) {
        if let Some(j) = self.journal.as_mut() {
            let _ = j.write_all(&[REC_CLONE]);
            let _ = j.write_all(&percept.to_le_bytes());
            self.dirty += 1;
        }
    }
    fn log_link(&mut self, from: u32, action: u16, to: u32) {
        if let Some(j) = self.journal.as_mut() {
            let _ = j.write_all(&[REC_LINK]);
            let _ = j.write_all(&from.to_le_bytes());
            let _ = j.write_all(&action.to_le_bytes());
            let _ = j.write_all(&to.to_le_bytes());
            self.dirty += 1;
        }
    }
    fn log_value(&mut self, id: u32, v: &Val) {
        if let Some(j) = self.journal.as_mut() {
            let _ = j.write_all(&[REC_VALUE]);
            let _ = j.write_all(&id.to_le_bytes());
            let _ = j.write_all(&[v.used]);
            for i in 0..v.used as usize {
                let _ = j.write_all(&v.v[i].to_le_bytes());
            }
            self.dirty += 1;
        }
    }

    /// 노드를 병합·삭제해 그래프를 다시 짓는다. 수면기 압축(C1)이 쓴다.
    ///
    /// `map[old] = new` (삭제는 `u32::MAX`). 같은 new로 향하는 옛 노드들은 하나로
    /// 합쳐진다: 증거는 더하고, 연속량은 증거 가중 평균하고, 전이 카운트는 합산한다.
    ///
    /// 이것이 "압축 = 일반화"의 물리적 형태다. 서로 다르다고 여겨 갈라놨던 것들이
    /// 사실 같은 것이었음을 뒤늦게 깨닫고 하나로 되돌린다.
    pub fn compact(&mut self, map: &[u32], n_new: usize) {
        debug_assert_eq!(map.len(), self.nodes.len());

        // 1) 새 노드 만들기 — 증거 가중 병합
        let mut percept_of = vec![u32::MAX; n_new];
        let mut evid = vec![0u32; n_new];
        let mut vals: Vec<Option<Val>> = vec![None; n_new];
        let mut tmax = vec![0u64; n_new];
        for (old, n) in self.nodes.iter().enumerate() {
            let new = map[old];
            if new == u32::MAX {
                continue;
            }
            let i = new as usize;
            if percept_of[i] == u32::MAX {
                percept_of[i] = n.percept;
            }
            if let Some(v) = n.atom.value {
                match &mut vals[i] {
                    Some(acc) => acc.absorb(&v, evid[i].max(1)),
                    None => vals[i] = Some(v),
                }
            }
            evid[i] = evid[i].saturating_add(n.atom.evidence);
            tmax[i] = tmax[i].max(n.atom.t);
        }

        // 2) 클론 목록과 상태 벡터 재구성
        let mut clones: Vec<Vec<u32>> = vec![Vec::new(); self.percept_vecs.len()];
        let mut nodes: Vec<Node> = Vec::with_capacity(n_new);
        let mut states = Store::new();
        for i in 0..n_new {
            let p = percept_of[i];
            debug_assert!(p != u32::MAX, "빈 블록이 남았다");
            let ix = clones[p as usize].len() as u32;
            let sid = self.percept_vecs[p as usize].bind(&clone_tag(p, ix));
            nodes.push(Node {
                percept: p,
                clone_ix: ix,
                atom: Atom { id: sid, value: vals[i], evidence: evid[i], t: tmax[i] },
            });
            clones[p as usize].push(i as u32);
            states.insert(i as u32, sid);
        }

        // 3) 간선 재구성
        let mut edges: HashMap<u64, Vec<Succ>> = HashMap::new();
        // 결정론: HashMap 순회 순서가 간선 삽입 순서로 새지 않게 키 정렬
        let mut ekeys: Vec<u64> = self.edges.keys().copied().collect();
        ekeys.sort_unstable();
        for &k in &ekeys {
            let list = &self.edges[&k];
            let from = (k >> 16) as u32;
            let action = (k & 0xffff) as u16;
            let nf = map[from as usize];
            if nf == u32::MAX {
                continue;
            }
            for s in list {
                let nt = map[s.to as usize];
                if nt == u32::MAX {
                    continue;
                }
                let e = edges.entry(edge_key(nf, action)).or_default();
                match e.iter_mut().find(|x| x.to == nt) {
                    Some(x) => x.count = x.count.saturating_add(s.count),
                    None => e.push(Succ { to: nt, count: s.count }),
                }
            }
        }

        self.nodes = nodes;
        self.clones = clones;
        self.states = states;
        self.edges = edges;
    }

    /// 증거가 부족한 간선을 걷어낸다(수면기 가지치기).
    pub fn prune_edges(&mut self, min_count: u32) -> usize {
        let mut removed = 0usize;
        for list in self.edges.values_mut() {
            let before = list.len();
            list.retain(|s| s.count >= min_count);
            removed += before - list.len();
        }
        self.edges.retain(|_, v| !v.is_empty());
        removed
    }

    /// 메모리 사용 추정(바이트) — RAM 예산 감시용(유리상자).
    pub fn memory_estimate(&self) -> usize {
        self.percept_vecs.len() * SBV_BYTES
            + self.nodes.len() * (SBV_BYTES + 64)
            + self.n_edges() * 12
            + self.clones.iter().map(|c| c.len() * 4).sum::<usize>()
    }
}

impl Default for WorldGraph {
    fn default() -> Self {
        WorldGraph::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn percept_interning_is_stable() {
        let mut r = Rng::new(1);
        let mut g = WorldGraph::new();
        let a = Sbv::random(&mut r);
        let p1 = g.intern_percept(&a, PERCEPT_TOL);
        let p2 = g.intern_percept(&a, PERCEPT_TOL);
        assert_eq!(p1, p2);
        assert_eq!(g.n_percepts(), 1);

        // 살짝 다른 관측은 같은 지각으로 흡수된다
        let mut noisy = a;
        for i in 0..10 {
            noisy.idx[i] = (noisy.idx[i] + 3) & 127;
        }
        assert_eq!(g.intern_percept(&noisy, PERCEPT_TOL), p1);

        // 전혀 다른 관측은 새 지각
        let b = Sbv::random(&mut r);
        assert_ne!(g.intern_percept(&b, PERCEPT_TOL), p1);
        assert_eq!(g.n_percepts(), 2);
    }

    #[test]
    fn clones_share_percept_but_differ_as_states() {
        let mut r = Rng::new(2);
        let mut g = WorldGraph::new();
        let p = g.intern_percept(&Sbv::random(&mut r), PERCEPT_TOL);
        let c0 = g.new_clone(p);
        let c1 = g.new_clone(p);
        assert_eq!(g.node(c0).percept, g.node(c1).percept);
        // 상태 벡터는 서로 멀어야 한다 — 그래야 계획이 둘을 혼동하지 않는다
        let s0 = g.node(c0).atom.id;
        let s1 = g.node(c1).atom.id;
        assert!(s0.sim(&s1) < 0.2, "클론 상태가 너무 비슷하다: {}", s0.sim(&s1));
        // unbind로 지각을 되찾을 수 있다(검사 가능성)
        let recovered = s0.unbind(&clone_tag(p, 0));
        assert_eq!(recovered, g.percept_vec(p));
    }

    #[test]
    fn transitions_accumulate_and_predict() {
        let mut r = Rng::new(3);
        let mut g = WorldGraph::new();
        let p = g.intern_percept(&Sbv::random(&mut r), PERCEPT_TOL);
        let (a, b, c) = (g.new_clone(p), g.new_clone(p), g.new_clone(p));
        for _ in 0..7 {
            g.link(a, 0, b);
        }
        for _ in 0..3 {
            g.link(a, 0, c);
        }
        let (best, prob) = g.predict(a, 0).unwrap();
        assert_eq!(best, b);
        assert!((prob - 0.7).abs() < 1e-5);
        // 엔트로피: 0.7/0.3 분포
        let h = g.entropy(a, 0);
        assert!((h - 0.8813).abs() < 0.01, "엔트로피 {h}");
        // 미지의 행동은 최대 불확실
        assert_eq!(g.entropy(a, 9), 1.0);
    }

    #[test]
    fn snapshot_and_journal_roundtrip() {
        let dir = std::env::temp_dir().join(format!("monad-graph-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut r = Rng::new(4);
        let obs: Vec<Sbv> = (0..5).map(|_| Sbv::random(&mut r)).collect();
        {
            let mut g = WorldGraph::attach(&dir).unwrap();
            let mut ids = Vec::new();
            for o in &obs {
                let p = g.intern_percept(o, PERCEPT_TOL);
                ids.push(g.new_clone(p));
            }
            for i in 0..4 {
                for _ in 0..(i + 1) {
                    g.link(ids[i], 1, ids[i + 1]);
                }
            }
            g.visit(ids[0], Some(Val::new(&[1.0, 2.0])));
            g.flush().unwrap();
            // 체크포인트 없이 프로세스가 죽는 상황을 흉내낸다(drop만)
        }
        {
            let g = WorldGraph::attach(&dir).unwrap();
            assert_eq!(g.n_percepts(), 5, "지각 복구");
            assert_eq!(g.n_nodes(), 5, "노드 복구");
            assert_eq!(g.succ(0, 1)[0].count, 1);
            assert_eq!(g.succ(3, 1)[0].count, 4, "카운트 복구");
            assert_eq!(g.node(0).atom.value.unwrap().as_slice()[0], 1.0);
        }
        // 체크포인트 후에도 동일해야 한다
        {
            let mut g = WorldGraph::attach(&dir).unwrap();
            g.checkpoint().unwrap();
        }
        {
            let g = WorldGraph::attach(&dir).unwrap();
            assert_eq!(g.n_nodes(), 5);
            assert_eq!(g.succ(3, 1)[0].count, 4);
            assert_eq!(g.node(0).atom.value.unwrap().as_slice()[1], 2.0);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn actions_from_lists_tried_actions() {
        let mut r = Rng::new(5);
        let mut g = WorldGraph::new();
        let p = g.intern_percept(&Sbv::random(&mut r), PERCEPT_TOL);
        let a = g.new_clone(p);
        let b = g.new_clone(p);
        g.link(a, 3, b);
        g.link(a, 1, b);
        assert_eq!(g.actions_from(a), vec![1, 3]);
        assert!(g.actions_from(b).is_empty());
    }
}
