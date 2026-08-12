//! W2-0 — 격자 세계 (합성 미니 ARC).
//!
//! ARC는 시간 스트림이 아니라 **함수 귀납**(입력 격자 → 출력 격자)이다. MONAD와의
//! 다리는 객체 수준 이벤트: 격자를 연결 성분(객체)으로 분해하고, 훈련쌍의 정렬된
//! 객체쌍을 "변환 이벤트"로 만들어 수면 스키마 귀납(MDL)에 공급한다.
//!
//! 이 모듈은 스파이크의 재료만 담는다: 격자 표현, 연결 성분 분해, 합성 과제 생성기
//! (ARC 최빈 원시 변환 10족). 실 ARC JSON 하네스는 W2-1.

use monad_core::rng::Rng;

pub const MAX_W: usize = 12;
pub const MAX_H: usize = 12;

#[derive(Clone, PartialEq, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    /// 행 우선, 0 = 배경(검정).
    pub cells: Vec<u8>,
}

impl Grid {
    pub fn new(w: usize, h: usize) -> Grid {
        Grid { w, h, cells: vec![0; w * h] }
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> u8 {
        self.cells[y * self.w + x]
    }
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: u8) {
        self.cells[y * self.w + x] = c;
    }
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h
    }
}

/// 연결 성분(4-이웃, 같은 색) — ARC 객체의 최소 정의.
#[derive(Clone, Debug)]
pub struct Obj {
    pub color: u8,
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
    /// bbox 기준 상대 마스크(행 우선).
    pub mask: Vec<bool>,
    pub area: usize,
}

impl Obj {
    /// 형태 지문 — 마스크의 정규화 해시(색 무관).
    pub fn shape_id(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        h = h.wrapping_mul(0x100000001b3) ^ self.w as u64;
        h = h.wrapping_mul(0x100000001b3) ^ self.h as u64;
        for &b in &self.mask {
            h = h.wrapping_mul(0x100000001b3) ^ (b as u64 + 1);
        }
        h
    }
}

/// 연결 성분 분해(conn8=true면 8-이웃 — 대각으로 이어진 덩어리를 하나로).
pub fn components_conn(g: &Grid, conn8: bool) -> Vec<Obj> {
    if !conn8 {
        return components(g);
    }
    let mut seen = vec![false; g.w * g.h];
    let mut out = Vec::new();
    for sy in 0..g.h {
        for sx in 0..g.w {
            let c = g.get(sx, sy);
            if c == 0 || seen[sy * g.w + sx] {
                continue;
            }
            let mut q = vec![(sx, sy)];
            seen[sy * g.w + sx] = true;
            let mut cells = Vec::new();
            while let Some((x, y)) = q.pop() {
                cells.push((x, y));
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if g.in_bounds(nx, ny) {
                            let (ux, uy) = (nx as usize, ny as usize);
                            if !seen[uy * g.w + ux] && g.get(ux, uy) == c {
                                seen[uy * g.w + ux] = true;
                                q.push((ux, uy));
                            }
                        }
                    }
                }
            }
            let x0 = cells.iter().map(|p| p.0).min().unwrap();
            let y0 = cells.iter().map(|p| p.1).min().unwrap();
            let x1 = cells.iter().map(|p| p.0).max().unwrap();
            let y1 = cells.iter().map(|p| p.1).max().unwrap();
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut mask = vec![false; w * h];
            for &(x, y) in &cells {
                mask[(y - y0) * w + (x - x0)] = true;
            }
            out.push(Obj { color: c, x0, y0, w, h, area: cells.len(), mask });
        }
    }
    out
}

/// 4-이웃 연결 성분 분해(배경 0 제외). 결정론: 스캔 순서 고정.
pub fn components(g: &Grid) -> Vec<Obj> {
    let mut seen = vec![false; g.w * g.h];
    let mut out = Vec::new();
    for sy in 0..g.h {
        for sx in 0..g.w {
            let c = g.get(sx, sy);
            if c == 0 || seen[sy * g.w + sx] {
                continue;
            }
            // BFS
            let mut q = vec![(sx, sy)];
            seen[sy * g.w + sx] = true;
            let mut cells = Vec::new();
            while let Some((x, y)) = q.pop() {
                cells.push((x, y));
                let nb = [
                    (x as i32 + 1, y as i32),
                    (x as i32 - 1, y as i32),
                    (x as i32, y as i32 + 1),
                    (x as i32, y as i32 - 1),
                ];
                for (nx, ny) in nb {
                    if g.in_bounds(nx, ny) {
                        let (ux, uy) = (nx as usize, ny as usize);
                        if !seen[uy * g.w + ux] && g.get(ux, uy) == c {
                            seen[uy * g.w + ux] = true;
                            q.push((ux, uy));
                        }
                    }
                }
            }
            let x0 = cells.iter().map(|p| p.0).min().unwrap();
            let y0 = cells.iter().map(|p| p.1).min().unwrap();
            let x1 = cells.iter().map(|p| p.0).max().unwrap();
            let y1 = cells.iter().map(|p| p.1).max().unwrap();
            let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
            let mut mask = vec![false; w * h];
            for &(x, y) in &cells {
                mask[(y - y0) * w + (x - x0)] = true;
            }
            out.push(Obj { color: c, x0, y0, w, h, area: cells.len(), mask });
        }
    }
    out
}

/// 객체를 격자에 찍는다(색 지정, 위치 지정).
pub fn stamp(g: &mut Grid, o: &Obj, x0: usize, y0: usize, color: u8) {
    for dy in 0..o.h {
        for dx in 0..o.w {
            if o.mask[dy * o.w + dx] {
                let (x, y) = (x0 + dx, y0 + dy);
                if x < g.w && y < g.h {
                    g.set(x, y, color);
                }
            }
        }
    }
}

/// 무작위 소형 객체(1~3개)를 겹치지 않게 뿌린 입력 격자.
fn random_input(r: &mut Rng, w: usize, h: usize, n_obj: usize) -> Grid {
    let mut g = Grid::new(w, h);
    let shapes: [&[(usize, usize)]; 5] = [
        &[(0, 0)],
        &[(0, 0), (1, 0)],
        &[(0, 0), (0, 1), (1, 0)],
        &[(0, 0), (1, 0), (0, 1), (1, 1)],
        &[(0, 0), (1, 0), (2, 0)],
    ];
    let mut placed = 0usize;
    let mut guard = 0;
    while placed < n_obj && guard < 200 {
        guard += 1;
        let sh = shapes[r.below(shapes.len() as u32) as usize];
        let color = 1 + r.below(8) as u8;
        let sw = sh.iter().map(|p| p.0).max().unwrap() + 1;
        let shh = sh.iter().map(|p| p.1).max().unwrap() + 1;
        // 변환 여유(이동·스케일)를 위해 가장자리 2칸 비움
        if w < sw + 5 || h < shh + 5 {
            continue;
        }
        let x0 = 1 + r.below((w - sw - 3) as u32) as usize;
        let y0 = 1 + r.below((h - shh - 3) as u32) as usize;
        // 겹침·인접 검사(분해 안정성: 같은 색 인접 병합 방지 위해 1칸 간격)
        let mut ok = true;
        for &(dx, dy) in sh {
            let (x, y) = (x0 + dx, y0 + dy);
            for ny in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    if g.get(nx, ny) != 0 {
                        ok = false;
                    }
                }
            }
        }
        if !ok {
            continue;
        }
        for &(dx, dy) in sh {
            g.set(x0 + dx, y0 + dy, color);
        }
        placed += 1;
    }
    g
}

/// 객체 내부의 갇힌 배경 셀(구멍) — bbox 테두리에서 못 닿는 배경.
pub fn holes(g: &Grid, o: &Obj) -> Vec<(usize, usize)> {
    let (w, h) = (o.w, o.h);
    // bbox 내 배경(마스크 밖) 중 테두리-연결 성분을 빼면 구멍이 남는다
    let mut open = vec![false; w * h];
    let mut q: Vec<(usize, usize)> = Vec::new();
    for x in 0..w {
        for &y in &[0usize, h - 1] {
            if !o.mask[y * w + x] && !open[y * w + x] {
                open[y * w + x] = true;
                q.push((x, y));
            }
        }
    }
    for y in 0..h {
        for &x in &[0usize, w - 1] {
            if !o.mask[y * w + x] && !open[y * w + x] {
                open[y * w + x] = true;
                q.push((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop() {
        let nb = [(x as i32 + 1, y as i32), (x as i32 - 1, y as i32), (x as i32, y as i32 + 1), (x as i32, y as i32 - 1)];
        for (nx, ny) in nb {
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                let (ux, uy) = (nx as usize, ny as usize);
                if !o.mask[uy * w + ux] && !open[uy * w + ux] {
                    open[uy * w + ux] = true;
                    q.push((ux, uy));
                }
            }
        }
    }
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if !o.mask[y * w + x] && !open[y * w + x] {
                out.push((o.x0 + x, o.y0 + y));
            }
        }
    }
    let _ = g;
    out
}

/// 열 분리 2객체 입력(왼쪽 반/오른쪽 반) — 낙하·수직 변환에서 충돌 없음 보장.
fn sep_col_input(r: &mut Rng, w: usize, h: usize) -> Grid {
    let mut g = Grid::new(w, h);
    let shapes: [&[(usize, usize)]; 3] = [
        &[(0, 0)],
        &[(0, 0), (1, 0)],
        &[(0, 0), (0, 1), (1, 0)],
    ];
    for half in 0..2usize {
        let sh = shapes[r.below(shapes.len() as u32) as usize];
        let color = 1 + r.below(8) as u8;
        let sw = sh.iter().map(|p| p.0).max().unwrap() + 1;
        let shh = sh.iter().map(|p| p.1).max().unwrap() + 1;
        let x_lo = half * (w / 2) + 1;
        let x_hi = (half + 1) * (w / 2) - sw - 1;
        let x0 = x_lo + r.below((x_hi - x_lo + 1) as u32) as usize;
        let y0 = 1 + r.below((h - shh - 3) as u32) as usize;
        for &(dx, dy) in sh {
            g.set(x0 + dx, y0 + dy, color);
        }
    }
    g
}

/// 과제 = 훈련쌍 몇 개 + 시험쌍 하나. 정답은 채점에만 쓴다.
pub struct Task {
    pub name: &'static str,
    pub train: Vec<(Grid, Grid)>,
    pub test_in: Grid,
    pub test_out: Grid,
}

/// 원시 변환 10족 — ARC 최빈 패턴의 최소 대표.
pub fn make_tasks(seed: u64) -> Vec<Task> {
    let mut r = Rng::new(seed);
    let mut tasks = Vec::new();

    // 각 족: 규칙 파라미터는 과제 안에서 고정, 훈련 3쌍 + 시험 1쌍.
    // 1) 평행 이동 (dx, dy 고정)
    {
        let (dx, dy) = (2i32, 1i32);
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            for obj in components(g) {
                stamp(
                    &mut o,
                    &obj,
                    (obj.x0 as i32 + dx) as usize,
                    (obj.y0 as i32 + dy) as usize,
                    obj.color,
                );
            }
            o
        };
        tasks.push(build("translate", &mut r, 10, 8, 2, f));
    }
    // 2) 색 치환 (a→b 전역)
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for c in o.cells.iter_mut() {
                if *c != 0 {
                    *c = if *c == 3 { 6 } else { *c };
                }
            }
            o
        };
        // 3색 객체가 반드시 있도록 후처리로 하나 심는다
        let mut t = build("recolor", &mut r, 10, 8, 2, f);
        for (i, o) in t.train.iter_mut().enumerate() {
            let _ = i;
            o.0.set(0, 0, 3);
            *o = (o.0.clone(), f(&o.0));
        }
        t.test_in.set(0, 0, 3);
        t.test_out = f(&t.test_in);
        tasks.push(t);
    }
    // 3) 수평 거울
    {
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(g.w - 1 - x, y, g.get(x, y));
                }
            }
            o
        };
        tasks.push(build("mirror_h", &mut r, 10, 8, 2, f));
    }
    // 4) 수직 거울
    {
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(x, g.h - 1 - y, g.get(x, y));
                }
            }
            o
        };
        tasks.push(build("mirror_v", &mut r, 10, 8, 2, f));
    }
    // 5) 중력(객체를 바닥까지 낙하) — 낙하 충돌이 없도록 열 분리 배치
    {
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            for obj in components(g) {
                let drop = g.h - obj.y0 - obj.h;
                stamp(&mut o, &obj, obj.x0, obj.y0 + drop, obj.color);
            }
            o
        };
        let mut train = Vec::new();
        for _ in 0..3 {
            let i = sep_col_input(&mut r, 10, 8);
            let o = f(&i);
            train.push((i, o));
        }
        let test_in = sep_col_input(&mut r, 10, 8);
        let test_out = f(&test_in);
        tasks.push(Task { name: "gravity", train, test_in, test_out });
    }
    // 6) 단색화(모든 객체를 색 5로)
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for c in o.cells.iter_mut() {
                if *c != 0 {
                    *c = 5;
                }
            }
            o
        };
        tasks.push(build("paint_all", &mut r, 10, 8, 2, f));
    }
    // 7) 최대 객체만 남기기
    {
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            let objs = components(g);
            if let Some(big) = objs.iter().max_by_key(|o| (o.area, o.color)) {
                stamp(&mut o, big, big.x0, big.y0, big.color);
            }
            o
        };
        tasks.push(build("keep_largest", &mut r, 10, 8, 3, f));
    }
    // 8) 수평 복제(오른쪽으로 +4에 사본) — 사본이 항상 격자 안에 들어가는 배치
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for obj in components(g) {
                stamp(&mut o, &obj, obj.x0 + 4, obj.y0, obj.color);
            }
            o
        };
        let gen = |r: &mut Rng| -> Grid {
            let (w, h) = (12usize, 8usize);
            let mut g = Grid::new(w, h);
            let shapes: [&[(usize, usize)]; 3] =
                [&[(0, 0)], &[(0, 0), (1, 0)], &[(0, 0), (0, 1), (1, 0)]];
            let sh = shapes[r.below(3) as usize];
            let color = 1 + r.below(8) as u8;
            let sw = sh.iter().map(|p| p.0).max().unwrap() + 1;
            let shh = sh.iter().map(|p| p.1).max().unwrap() + 1;
            // 사본(x0+4..x0+4+sw)이 격자 안: x0 ≤ w - sw - 4
            let x0 = 1 + r.below((w - sw - 4 - 1 + 1) as u32) as usize;
            let y0 = 1 + r.below((h - shh - 1) as u32) as usize;
            for &(dx, dy) in sh {
                g.set(x0 + dx, y0 + dy, color);
            }
            g
        };
        let mut train = Vec::new();
        for _ in 0..3 {
            let i = gen(&mut r);
            let o = f(&i);
            train.push((i, o));
        }
        let test_in = gen(&mut r);
        let test_out = f(&test_in);
        tasks.push(Task { name: "duplicate_right", train, test_in, test_out });
    }
    // 9) 테두리 상자(각 객체 bbox를 색 7로 두름)
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for obj in components(g) {
                let (x0, y0) = (obj.x0 as i32 - 1, obj.y0 as i32 - 1);
                let (x1, y1) = ((obj.x0 + obj.w) as i32, (obj.y0 + obj.h) as i32);
                for x in x0..=x1 {
                    for &y in &[y0, y1] {
                        if o.in_bounds(x, y) && o.get(x as usize, y as usize) == 0 {
                            o.set(x as usize, y as usize, 7);
                        }
                    }
                }
                for y in y0..=y1 {
                    for &x in &[x0, x1] {
                        if o.in_bounds(x, y) && o.get(x as usize, y as usize) == 0 {
                            o.set(x as usize, y as usize, 7);
                        }
                    }
                }
            }
            o
        };
        tasks.push(build("outline", &mut r, 10, 8, 1, f));
    }
    // 10) 대각 이동(-1, -1)
    {
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            for obj in components(g) {
                let nx = obj.x0.saturating_sub(1);
                let ny = obj.y0.saturating_sub(1);
                stamp(&mut o, &obj, nx, ny, obj.color);
            }
            o
        };
        tasks.push(build("translate_neg", &mut r, 10, 8, 2, f));
    }
    // 11) 표식 복제: 큰 객체(원본)를 단일 셀 표식(색 9) 자리마다 복사, 표식은 소멸
    {
        let f = |g: &Grid| -> Grid {
            let mut o = Grid::new(g.w, g.h);
            let objs = components(g);
            let src = objs.iter().filter(|x| x.color != 9).max_by_key(|x| (x.area, x.color));
            if let Some(src) = src {
                stamp(&mut o, src, src.x0, src.y0, src.color);
                for m in objs.iter().filter(|x| x.color == 9) {
                    stamp(&mut o, src, m.x0, m.y0, src.color);
                }
            }
            o
        };
        let mut train = Vec::new();
        for _ in 0..3 {
            let i = marker_input(&mut r, 12, 10);
            let o = f(&i);
            train.push((i, o));
        }
        let test_in = marker_input(&mut r, 12, 10);
        let test_out = f(&test_in);
        tasks.push(Task { name: "marker_copy", train, test_in, test_out });
    }
    // 12) 구멍 채움: 고리 객체의 내부를 색 4로 채움
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for obj in components(g) {
                for (x, y) in holes(g, &obj) {
                    o.set(x, y, 4);
                }
            }
            o
        };
        let mut train = Vec::new();
        for _ in 0..3 {
            let i = ring_input(&mut r, 10, 8);
            let o = f(&i);
            train.push((i, o));
        }
        let test_in = ring_input(&mut r, 10, 8);
        let test_out = f(&test_in);
        tasks.push(Task { name: "fill_hole", train, test_in, test_out });
    }
    // 13) 광선: 각 객체에서 오른쪽 가장자리까지 색 8 선
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for obj in components(g) {
                let y = obj.y0;
                for x in (obj.x0 + obj.w)..g.w {
                    if o.get(x, y) == 0 {
                        o.set(x, y, 8);
                    }
                }
            }
            o
        };
        tasks.push(build("ray_right", &mut r, 10, 8, 1, f));
    }
    // 14) 바닥 표시: 각 객체 중심 열의 바닥 행에 색 4 점
    {
        let f = |g: &Grid| -> Grid {
            let mut o = g.clone();
            for obj in components(g) {
                let cx = obj.x0 + obj.w / 2;
                o.set(cx, g.h - 1, 4);
            }
            o
        };
        tasks.push(build("mark_floor", &mut r, 10, 8, 2, f));
    }

    tasks
}

/// 표식 복제 입력: 원본 객체 1개(색 1~8) + 단일 셀 표식(색 9) 2개, 충돌 없는 배치.
fn marker_input(r: &mut Rng, w: usize, h: usize) -> Grid {
    let mut g = Grid::new(w, h);
    // 원본: 2×2 정사각(안정된 형태), 왼쪽 위 구역
    let color = 1 + r.below(7) as u8;
    let sx = 1 + r.below(2) as usize;
    let sy = 1 + r.below(2) as usize;
    for dy in 0..2 {
        for dx in 0..2 {
            g.set(sx + dx, sy + dy, color);
        }
    }
    // 표식 2개: 오른쪽/아래 구역, 복사(2×2)가 서로·원본과 안 겹치게 4칸 간격
    let m1 = (6 + r.below(3) as usize, 1 + r.below(2) as usize);
    let m2 = (2 + r.below(3) as usize, 6 + r.below(2) as usize);
    g.set(m1.0, m1.1, 9);
    g.set(m2.0, m2.1, 9);
    g
}

/// 고리 입력: 속이 빈 사각 고리 1개(구멍 보장).
fn ring_input(r: &mut Rng, w: usize, h: usize) -> Grid {
    let mut g = Grid::new(w, h);
    let color = 1 + r.below(8) as u8;
    let rw = 4 + r.below(2) as usize; // 4~5
    let rh = 4;
    let x0 = 1 + r.below((w - rw - 2) as u32) as usize;
    let y0 = 1 + r.below((h - rh - 2) as u32) as usize;
    for x in x0..x0 + rw {
        g.set(x, y0, color);
        g.set(x, y0 + rh - 1, color);
    }
    for y in y0..y0 + rh {
        g.set(x0, y, color);
        g.set(x0 + rw - 1, y, color);
    }
    g
}

fn build(
    name: &'static str,
    r: &mut Rng,
    w: usize,
    h: usize,
    n_obj: usize,
    f: impl Fn(&Grid) -> Grid,
) -> Task {
    let mut train = Vec::new();
    for _ in 0..3 {
        let i = random_input(r, w, h, n_obj);
        let o = f(&i);
        train.push((i, o));
    }
    let test_in = random_input(r, w, h, n_obj);
    let test_out = f(&test_in);
    Task { name, train, test_in, test_out }
}
