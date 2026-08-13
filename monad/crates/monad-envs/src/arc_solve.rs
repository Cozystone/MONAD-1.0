//! W2 — ARC 솔버 (W2-0 스파이크에서 검증된 파이프라인의 공용 모듈).
//!
//! 격자 → 객체(연결 성분) → 정렬 → 관계 오차 후보 라벨링(모호성 보존) →
//! 전역 일관성 투표 → 인수분해 스키마 lib 5종(클래스/dx/dy/색/사본수) → 적용.
//!
//! 합성 10과제군 10/10(시도 62). 실 ARC의 낮은 커버리지는 클래스·슬롯 어휘의
//! 확장(W2-2+)으로 좁힌다 — 측정이 어휘 확장의 우선순위를 정한다.

use crate::grid::{components, stamp, Grid, Obj};
use monad_core::schema::{induce, Event, InduceConfig, SchemaLib};

// 슬롯 어휘
pub const S_COLOR: u16 = 1;
pub const S_LARGEST: u16 = 2;
pub const S_COPY_K: u16 = 3;
pub const S_GAP: u16 = 4;
/// 파라미터 lib 전용: 클래스 조건(파라미터 의미가 클래스마다 다르므로).
pub const S_CLASS: u16 = 5;
/// 면적 순위(0=최대, 3=그 밖) — 크기 조건 규칙의 원시.
pub const S_RANK: u16 = 6;
/// 격자 테두리 접촉 여부 — 위치 조건 규칙의 원시.
pub const S_BORDER: u16 = 7;
/// 최소 면적 객체 여부(동률은 색 오름차) — "가장 작은 것" 조건의 원시.
pub const S_SMALLEST: u16 = 8;
/// 정확 면적(0..15 클램프) — "면적=6이면" 류 조건의 원시.
pub const S_AREA: u16 = 9;
/// 갇힌 배경(구멍) 보유 여부 — 고리/막힘 조건의 원시.
pub const S_HOLED: u16 = 10;
/// 관계 슬롯 1호: 같은 색의 행/열 정렬 파트너 보유(객체 간 관계의 시작).
pub const S_ALIGNED: u16 = 11;
/// 관계 슬롯 2호: 같은 형태(마스크) 파트너 보유 — 형태 쌍 관계.
pub const S_PAIRED: u16 = 12;
/// 관계 슬롯 3·4호: 최대 객체 기준 상대 위치(0=앞, 1=같음, 2=뒤).
pub const S_REL_X: u16 = 13;
pub const S_REL_Y: u16 = 14;
/// 관계 슬롯 5·6호: 기준점까지 체비셰프 거리 버킷 · 기준 객체의 색.
pub const S_DIST: u16 = 15;
pub const S_ANCHOR_COLOR: u16 = 16;

// 변환 클래스
pub const C_STAY: u32 = 0;
pub const C_TRANS: u32 = 1;
pub const C_MIR_H: u32 = 2;
pub const C_MIR_V: u32 = 3;
pub const C_GRAV: u32 = 4;
pub const C_DEL: u32 = 5;
pub const C_OUTLINE: u32 = 6;
// 생성 클래스(W2-2 — 실패 60%가 객체 생성형이라는 실측에서 나온 어휘)
pub const C_AT_MARKER: u32 = 7;
pub const C_FILL: u32 = 8;
pub const C_RAY: u32 = 9;
/// 표시: 부모 중심 열의 바닥 행에 1셀.
pub const C_MARK_FLOOR: u32 = 10;
/// 표시: 부모 바닥중심 기준 고정 오프셋(param1=dx+16, param2=dy+16)에 1셀.
pub const C_MARK_REL: u32 = 11;
/// 제자리 180도 회전(bbox 불변).
pub const C_ROT180_OBJ: u32 = 12;
/// 형태 변형: 1링 팽창 / 침식(중심 유지).
pub const C_DILATE: u32 = 13;
pub const C_ERODE: u32 = 14;
/// 밴드 광선: 객체 전체 높이(또는 폭)의 사각 밴드가 가장자리까지(param1=방향).
pub const C_RAY_BAND: u32 = 15;
/// 표식 복제(면적 기반): param1=표식 면적 — 색 무관 "모든 1셀 자리에 사본" 류.
pub const C_AT_MARKER_AREA: u32 = 16;
/// 객체 내 색 교환: 그 객체가 가진 두 색을 맞바꾼다(다색 표현 전용 어휘).
pub const C_COLORSWAP: u32 = 17;
/// 고형화: 객체의 bbox를 제 색으로 가득 채운다(속 빈 도형 메우기).
pub const C_SOLIDIFY: u32 = 18;

pub const CLASS_NAMES: [&str; 19] = [
    "stay", "translate", "mirror_h", "mirror_v", "gravity", "delete", "outline",
    "at_marker", "fill", "ray", "mark_floor", "mark_rel", "rot180", "dilate", "erode",
    "ray_band", "at_marker_area", "colorswap", "solidify",
];

/// 1링 팽창 마스크(bbox +2, 4-이웃).
pub fn dilate_mask(o: &Obj) -> (usize, usize, Vec<bool>) {
    let (w, h) = (o.w + 2, o.h + 2);
    let mut m = vec![false; w * h];
    for y in 0..o.h {
        for x in 0..o.w {
            if o.mask[y * o.w + x] {
                for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x as i32 + 1 + dx, y as i32 + 1 + dy);
                    m[ny as usize * w + nx as usize] = true;
                }
            }
        }
    }
    (w, h, m)
}

/// 침식 마스크(4-이웃 전부 참인 셀만, bbox 유지).
pub fn erode_mask(o: &Obj) -> Vec<bool> {
    let mut m = vec![false; o.w * o.h];
    for y in 0..o.h {
        for x in 0..o.w {
            if !o.mask[y * o.w + x] {
                continue;
            }
            let ok = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                nx >= 0
                    && ny >= 0
                    && (nx as usize) < o.w
                    && (ny as usize) < o.h
                    && o.mask[ny as usize * o.w + nx as usize]
            });
            if ok {
                m[y * o.w + x] = true;
            }
        }
    }
    m
}

pub fn rot180_mask(o: &Obj) -> Vec<bool> {
    let mut m = vec![false; o.w * o.h];
    for y in 0..o.h {
        for x in 0..o.w {
            m[(o.h - 1 - y) * o.w + (o.w - 1 - x)] = o.mask[y * o.w + x];
        }
    }
    m
}

pub fn hflip(o: &Obj) -> Vec<bool> {
    let mut m = vec![false; o.w * o.h];
    for y in 0..o.h {
        for x in 0..o.w {
            m[y * o.w + (o.w - 1 - x)] = o.mask[y * o.w + x];
        }
    }
    m
}
pub fn vflip(o: &Obj) -> Vec<bool> {
    let mut m = vec![false; o.w * o.h];
    for y in 0..o.h {
        for x in 0..o.w {
            m[(o.h - 1 - y) * o.w + x] = o.mask[y * o.w + x];
        }
    }
    m
}

/// 정렬: 출력 객체를 입력 객체에 짝짓는다(같은 형태·색 우선, 거리 최소).
fn align(ins: &[Obj], outs: &[Obj]) -> Vec<(usize, Vec<usize>)> {
    let mut taken = vec![false; outs.len()];
    let mut pairs: Vec<(usize, Vec<usize>)> = (0..ins.len()).map(|i| (i, Vec::new())).collect();
    let mut cands: Vec<(usize, usize, i64)> = Vec::new();
    for (ii, io) in ins.iter().enumerate() {
        for (oi, oo) in outs.iter().enumerate() {
            let shape_rel = oo.w == io.w
                && oo.h == io.h
                && (oo.mask == io.mask || oo.mask == hflip(io) || oo.mask == vflip(io));
            if shape_rel {
                let d = (io.x0 as i64 - oo.x0 as i64).pow(2) + (io.y0 as i64 - oo.y0 as i64).pow(2);
                let color_bonus = if io.color == oo.color { 0 } else { 1000 };
                cands.push((ii, oi, d + color_bonus));
            }
        }
    }
    cands.sort_by_key(|c| c.2);
    for (ii, oi, _) in cands {
        if !taken[oi] {
            // 이색 형태 일치는 1차 사본(재색칠+이동)에만 — 추가 사본까지 이색을
            // 허용하면 1셀 생성물(표시)이 1셀 객체의 사본으로 도둑 매칭된다.
            let cross = ins[ii].color != outs[oi].color;
            if cross && !pairs[ii].1.is_empty() {
                continue;
            }
            taken[oi] = true;
            pairs[ii].1.push(oi);
        }
    }
    for (oi, oo) in outs.iter().enumerate() {
        if taken[oi] {
            continue;
        }
        let mut best = (usize::MAX, i64::MAX);
        for (ii, io) in ins.iter().enumerate() {
            let d = (io.x0 as i64 - oo.x0 as i64).pow(2) + (io.y0 as i64 - oo.y0 as i64).pow(2);
            // 관계 앵커 신호: 생성물이 부모 중심 열/행에 정렬되면 강한 귀속 근거
            // (바닥 표시류). 불일치 시 균등 페널티라 기존 근접 귀속에 중립.
            let cx = (io.x0 + io.w / 2) as i64;
            let cy = (io.y0 + io.h / 2) as i64;
            let ox = (oo.x0 + oo.w / 2) as i64;
            let oy = (oo.y0 + oo.h / 2) as i64;
            let align_bonus = if ox == cx || oy == cy { 0 } else { 1000 };
            let d = d + align_bonus;
            if d < best.1 {
                best = (ii, d);
            }
        }
        if best.0 != usize::MAX {
            pairs[best.0].1.push(oi);
        }
    }
    pairs
}

/// 관계 오차로 후보 변환 클래스들을 라벨링(모호성 보존).
/// `ins`는 관계 앵커(표식 위치 등) 탐지용 전체 입력 객체 목록.
fn candidates(g_in: &Grid, ins: &[Obj], ii: usize, oo: &Obj) -> Vec<(u32, i32, i32)> {
    let io = &ins[ii];
    let dx = oo.x0 as i32 - io.x0 as i32;
    let dy = oo.y0 as i32 - io.y0 as i32;
    let same_mask = oo.mask == io.mask;
    let mut out = Vec::new();
    if !same_mask && oo.w == io.w + 2 && oo.h == io.h + 2 {
        out.push((C_OUTLINE, 0, 0));
        return out;
    }
    if same_mask && dx == 0 && dy == 0 {
        // 다색 표현에서: 제자리인데 색 배열이 두 색의 교환이면 색 교환 연산.
        // 가드(시도 119의 −5 회귀 교훈): **객체가 실제로 2색 이상**일 때만 —
        // 단색 객체의 전역 재색칠은 색 lib의 몫이며, 여기서 가로채면 손해다.
        let io_multi = {
            let mut cs: Vec<u8> = io
                .colors
                .iter()
                .enumerate()
                .filter(|(i, _)| io.mask[*i])
                .map(|(_, &c)| c)
                .collect();
            cs.sort_unstable();
            cs.dedup();
            cs.len() >= 2
        };
        if io_multi
            && io.colors.len() == io.mask.len()
            && oo.colors.len() == oo.mask.len()
            && io.colors != oo.colors
        {
            let mut pair: Option<(u8, u8)> = None;
            let mut consistent = true;
            for i in 0..io.colors.len() {
                if !io.mask[i] {
                    continue;
                }
                let (a, b) = (io.colors[i], oo.colors[i]);
                if a == b {
                    continue;
                }
                match pair {
                    None => pair = Some((a.min(b), a.max(b))),
                    Some((p, q)) => {
                        if !((a == p && b == q) || (a == q && b == p)) {
                            consistent = false;
                        }
                    }
                }
            }
            if consistent && pair.is_some() {
                out.push((C_COLORSWAP, 0, 0));
                return out;
            }
        }
        out.push((C_STAY, 0, 0));
        return out;
    }
    // 표식 복제: 사본이 다른 입력 객체(표식)의 위치에 정확히 놓임 — 관계 앵커.
    // 파라미터 = 표식의 색.
    if same_mask {
        for (mi, m) in ins.iter().enumerate() {
            if mi != ii && (m.x0, m.y0) == (oo.x0, oo.y0) && m.mask != io.mask {
                out.push((C_AT_MARKER, m.color as i32, 0));
                out.push((C_AT_MARKER_AREA, m.area.min(15) as i32, 0));
                break;
            }
        }
    }
    // 구멍 채움: 생성물이 이 객체의 갇힌 배경 안에 정확히 들어앉음.
    if !same_mask && oo.area > 0 {
        let hs: std::collections::HashSet<(usize, usize)> =
            crate::grid::holes(g_in, io).into_iter().collect();
        let mut all_in = true;
        'h: for y in 0..oo.h {
            for x in 0..oo.w {
                if oo.mask[y * oo.w + x] && !hs.contains(&(oo.x0 + x, oo.y0 + y)) {
                    all_in = false;
                    break 'h;
                }
            }
        }
        if all_in && !hs.is_empty() {
            out.push((C_FILL, oo.color as i32, 0));
            return out;
        }
    }
    // 표시: 생성물이 1셀 — 부모의 관계 위치(바닥 행 중심열 / 바닥중심 오프셋).
    // 가드: 부모가 다중 셀이거나 색이 다를 때만(1셀 객체 자신의 이동과 구별).
    if oo.area == 1 && (io.area > 1 || io.color != oo.color) {
        let cx = io.x0 + io.w / 2;
        let by = io.y0 + io.h; // 바닥 바로 아래 기준
        if oo.y0 == g_in.h - 1 && oo.x0 == cx {
            out.push((C_MARK_FLOOR, 0, 0));
        }
        let rdx = oo.x0 as i32 - cx as i32;
        let rdy = oo.y0 as i32 - by as i32;
        if rdx.abs() <= 4 && rdy.abs() <= 4 {
            out.push((C_MARK_REL, rdx, rdy));
        }
        if !out.is_empty() {
            return out;
        }
    }
    // 밴드 광선: 객체 전체 높이/폭의 꽉 찬 사각형이 변에서 가장자리까지
    if !same_mask && oo.mask.iter().all(|&b| b) {
        if oo.h == io.h
            && oo.y0 == io.y0
            && oo.x0 == io.x0 + io.w
            && oo.x0 + oo.w == g_in.w
        {
            out.push((C_RAY_BAND, 0, 0));
            return out;
        }
        if oo.h == io.h && oo.y0 == io.y0 && oo.x0 == 0 && oo.x0 + oo.w == io.x0 {
            out.push((C_RAY_BAND, 1, 0));
            return out;
        }
        if oo.w == io.w
            && oo.x0 == io.x0
            && oo.y0 == io.y0 + io.h
            && oo.y0 + oo.h == g_in.h
        {
            out.push((C_RAY_BAND, 2, 0));
            return out;
        }
        if oo.w == io.w && oo.x0 == io.x0 && oo.y0 == 0 && oo.y0 + oo.h == io.y0 {
            out.push((C_RAY_BAND, 3, 0));
            return out;
        }
    }
    // 광선: 생성물이 1폭 직선이고 이 객체의 변에서 격자 가장자리까지 닿음.
    // 파라미터 = 방향(0우 1좌 2하 3상).
    if !same_mask && (oo.w == 1 || oo.h == 1) {
        if oo.h == 1
            && oo.y0 >= io.y0
            && oo.y0 < io.y0 + io.h
            && oo.x0 == io.x0 + io.w
            && oo.x0 + oo.w == g_in.w
        {
            out.push((C_RAY, 0, 0));
            return out;
        }
        if oo.h == 1
            && oo.y0 >= io.y0
            && oo.y0 < io.y0 + io.h
            && oo.x0 == 0
            && oo.x0 + oo.w == io.x0
        {
            out.push((C_RAY, 1, 0));
            return out;
        }
        if oo.w == 1
            && oo.x0 >= io.x0
            && oo.x0 < io.x0 + io.w
            && oo.y0 == io.y0 + io.h
            && oo.y0 + oo.h == g_in.h
        {
            out.push((C_RAY, 2, 0));
            return out;
        }
        if oo.w == 1
            && oo.x0 >= io.x0
            && oo.x0 < io.x0 + io.w
            && oo.y0 == 0
            && oo.y0 + oo.h == io.y0
        {
            out.push((C_RAY, 3, 0));
            return out;
        }
    }
    if same_mask && dx == 0 && (oo.y0 + oo.h == g_in.h) {
        out.push((C_GRAV, 0, dy));
    }
    let mir_x = (g_in.w - io.w) as i32 - io.x0 as i32;
    let mir_y = (g_in.h - io.h) as i32 - io.y0 as i32;
    if oo.mask == hflip(io) && oo.x0 as i32 == mir_x && dy == 0 {
        out.push((C_MIR_H, 0, 0));
    }
    if oo.mask == vflip(io) && oo.y0 as i32 == mir_y && dx == 0 {
        out.push((C_MIR_V, 0, 0));
    }
    // 제자리 180도(bbox 불변·마스크 비대칭일 때만 유의미)
    if dx == 0 && dy == 0 && !same_mask && oo.mask == rot180_mask(io) {
        out.push((C_ROT180_OBJ, 0, 0));
    }
    // 고형화: 같은 bbox인데 출력이 꽉 찬 사각형(입력은 아니었음)
    if dx == 0
        && dy == 0
        && oo.w == io.w
        && oo.h == io.h
        && oo.mask.iter().all(|&b| b)
        && !io.mask.iter().all(|&b| b)
    {
        out.push((C_SOLIDIFY, 0, 0));
        return out;
    }
    // 형태 변형: 팽창(bbox +2 중심 유지) / 침식(bbox 내 축소)
    if !same_mask {
        let (dw, dh, dm) = dilate_mask(io);
        if oo.w == dw
            && oo.h == dh
            && oo.x0 as i32 == io.x0 as i32 - 1
            && oo.y0 as i32 == io.y0 as i32 - 1
            && oo.mask == dm
        {
            out.push((C_DILATE, 0, 0));
        }
        if oo.x0 >= io.x0 && oo.y0 >= io.y0 && oo.x0 + oo.w <= io.x0 + io.w {
            let em = erode_mask(io);
            // 침식 결과를 oo 좌표계로 대조(비어있지 않을 때)
            let mut fit = em.iter().any(|&b| b);
            if fit {
                'chk: for y in 0..io.h {
                    for x in 0..io.w {
                        let inside = x + io.x0 >= oo.x0
                            && y + io.y0 >= oo.y0
                            && x + io.x0 < oo.x0 + oo.w
                            && y + io.y0 < oo.y0 + oo.h;
                        let ov = if inside {
                            oo.mask
                                [(y + io.y0 - oo.y0) * oo.w + (x + io.x0 - oo.x0)]
                        } else {
                            false
                        };
                        if em[y * io.w + x] != ov {
                            fit = false;
                            break 'chk;
                        }
                    }
                }
            }
            if fit {
                out.push((C_ERODE, 0, 0));
            }
        }
    }
    if same_mask {
        out.push((C_TRANS, dx, dy));
    }
    if out.is_empty() {
        out.push((C_TRANS, dx, dy));
    }
    out
}

fn obj_event(objs: &[Obj], i: usize, copy_k: u32, grid_h: usize, effect: u32, extra: bool) -> Event {
    let o = &objs[i];
    let best = objs.iter().map(|x| (x.area, x.color)).max().unwrap_or((0, 0));
    let largest = (o.area, o.color) == best;
    let grounded = grid_h == o.y0 + o.h;
    // 면적 순위(동률은 색 내림차) — "두 번째로 큰 것" 류 조건의 원시
    let rank = objs
        .iter()
        .filter(|x| (x.area, x.color) > (o.area, o.color))
        .count()
        .min(3) as u32;
    // 테두리 접촉(x0/y0=0 또는 끝변 — grid 폭은 objs로 알 수 없어 x0·y0만은 부정확
    // 하므로 접촉 판정은 y0==0 || x0==0 || 바닥(grounded)로 근사, 우변은 생략)
    let border = (o.x0 == 0 || o.y0 == 0 || grounded) as u32;
    let worst = objs.iter().map(|x| (x.area, std::cmp::Reverse(x.color))).min();
    let smallest = worst == Some((o.area, std::cmp::Reverse(o.color)));
    // 구멍 보유: bbox 테두리에서 못 닿는 배경(마스크 로컬 판정)
    let holed = {
        let (w, h) = (o.w, o.h);
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
            let nb =
                [(x as i32 + 1, y as i32), (x as i32 - 1, y as i32), (x as i32, y as i32 + 1), (x as i32, y as i32 - 1)];
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
        (0..w * h).any(|i| !o.mask[i] && !open[i])
    };
    let mut ev = Event {
        cats: vec![
            (S_COLOR, o.color as u32),
            (S_LARGEST, largest as u32),
            (S_COPY_K, copy_k),
            (S_GAP, grounded as u32),
            (S_RANK, rank),
            (S_BORDER, border),
            (S_SMALLEST, smallest as u32),
            (S_AREA, (o.area.min(15)) as u32),
            (S_HOLED, holed as u32),
            (S_ALIGNED, {
                let cy = o.y0 + o.h / 2;
                let cx = o.x0 + o.w / 2;
                objs.iter()
                    .enumerate()
                    .any(|(j, m)| {
                        j != i
                            && m.color == o.color
                            && (m.y0 + m.h / 2 == cy || m.x0 + m.w / 2 == cx)
                    }) as u32
            }),
        ],
        nums: vec![],
        effect,
    };
    // 확장 슬롯: 교차 검증(LOO)이 승인한 과제에서만 쓰인다
    if extra {
        let paired = objs
            .iter()
            .enumerate()
            .any(|(j, m)| j != i && m.w == o.w && m.h == o.h && m.mask == o.mask);
        ev.cats.push((S_PAIRED, paired as u32));
        // 최대 객체를 기준점으로 한 상대 위치 — 관계 추론의 최소 좌표계
        if let Some(anchor) = objs.iter().max_by_key(|x| (x.area, x.color)) {
            let (ax, ay) = (anchor.x0 + anchor.w / 2, anchor.y0 + anchor.h / 2);
            let (cx, cy) = (o.x0 + o.w / 2, o.y0 + o.h / 2);
            let rel = |a: usize, b: usize| -> u32 {
                if a < b {
                    0
                } else if a == b {
                    1
                } else {
                    2
                }
            };
            ev.cats.push((S_REL_X, rel(cx, ax)));
            ev.cats.push((S_REL_Y, rel(cy, ay)));
            // 거리 버킷(체비셰프): 인접·근거리·중거리·원거리
            let d = (cx as i32 - ax as i32).abs().max((cy as i32 - ay as i32).abs());
            let bucket = if d <= 1 {
                0
            } else if d <= 3 {
                1
            } else if d <= 6 {
                2
            } else {
                3
            };
            ev.cats.push((S_DIST, bucket));
            ev.cats.push((S_ANCHOR_COLOR, anchor.color as u32));
        }
    }
    ev
}

#[derive(Clone)]
pub struct Libs {
    pub class: SchemaLib,
    pub dx: SchemaLib,
    pub dy: SchemaLib,
    pub color: SchemaLib,
    pub copies: SchemaLib,
    /// 격자 수준 연산 연쇄(객체 파이프라인보다 먼저 시도되는 전역 가설, 깊이≤2).
    pub grid_op: Option<(GridOp, Option<GridOp>)>,
    /// 이 lib이 확장 슬롯으로 학습되었는가(apply가 같은 슬롯으로 조회해야 한다).
    pub extra: bool,
    /// 8-연결 분해로 학습되었는가(apply도 같은 분해를 써야 한다).
    pub conn8: bool,
    /// 배경으로 취급한 색(0이면 기존 가정).
    pub bg: u8,
    /// 다색 객체 분해로 학습되었는가.
    pub multi: bool,
}

/// 격자 수준 연산 — 객체 분해로는 안 보이는 전역 구조(대각 벽의 갇힌 영역 등).
/// 훈련쌍 전부를 정확히 재현하는 가설만 채택된다(과제 수준 MDL의 극한).
#[derive(Clone, Copy, Debug)]
pub enum GridOp {
    /// 테두리에서 4-연결 도달 불가한 배경을 색 c로 채움.
    FillEnclosed(u8),
    /// 배경 셀을 수평/수직 거울상 값으로 복원(대칭 완성).
    SymFillH,
    SymFillV,
    /// 정수 확대: 각 셀 → k×k 블록.
    Scale(u8),
    /// 타일: 입력을 nx×ny로 반복.
    Tile(u8, u8),
    /// 2×2 거울 타일: [원본|좌우거울 / 상하거울|점거울] — ARC 최빈 모티프.
    TileMirror4,
    /// 추출: 최대 면적 객체의 bbox로 자르기.
    ExtractLargest,
    /// 추출: 색이 유일한(그 색 객체가 1개뿐) 객체의 bbox로 자르기.
    ExtractUniqueColor,
    /// 추출: 비배경 전체의 최소 bbox로 자르기(테두리 정리).
    ExtractContent,
    /// 추출: 속 빈 사각 액자 객체의 내부 내용물.
    ExtractFrameInterior,
    /// 추출: 선택 규칙별 객체 bbox(0=형태 유일, 1=형태 최빈, 2=색 최빈, 3=색 최소빈, 4=8연결 최대).
    ExtractBy(u8),
    /// 부분격자 추출: 고정 크기 (w,h) 창을 규칙으로 고른다
    /// (0=비배경 최다, 1=비배경 최소, 2=색 종류 최다, 3=색 종류 최소, 4=고유 내용).
    ExtractWindow(u8, u8, u8),
    /// 색 c의 셀을 전부 배경으로(잡음 제거).
    RemoveColor(u8),
    /// 전역 팔레트 치환: 색 i → map[i].
    PaletteMap([u8; 10]),
    /// 주기 수선: (px,py) 주기 타일링의 다수결 대표로 전 셀을 복원(구멍·잡음 수리).
    PeriodicRepair(u8, u8),
    /// 잇기: 행/열 정렬된 같은 색 객체 쌍 사이를 그 색 선으로 연결(배경 위만).
    ConnectPairs,
    /// 대칭 복원 완전형: H·V·180·전치 대칭을 고정점까지 반복해 배경을 메움.
    SymFillAll,
    /// 내용물 bbox 안에서만 대칭 복원(국소 모티프의 대칭 완성).
    SymFillBBox,
    /// 각 객체를 자기 크기만큼 반복 복제해 가장자리까지 채운다(0=우,1=좌,2=하,3=상,4=양방향 수평,5=양방향 수직).
    RepeatToEdge(u8),
    /// 대칭 복원 후 **가려졌던 조각만** 반환(폐색 패치 복구 — ARC 고전 가족).
    SymFillPatch,
    /// 폐색 표시색 c를 대칭으로 복원한 뒤 그 영역만 반환.
    SymFillPatchColor(u8),
    /// 주기 구조로 폐색을 메운다(비폐색 셀은 보존). mark=0이면 배경이 폐색.
    PeriodicFill(u8),
    /// 주기로 폐색을 메운 뒤 그 영역만 반환.
    PeriodicPatch(u8),
    /// 패널 선택: 0=고유(나머지와 다른 하나), 1=최빈, 2=비배경 최다, 3=비배경 최소.
    PanelSelect(u8),
    /// 패널 요약: 2차원 패널 격자를 패널당 한 셀로 축약(0=대표색, 1=비었나 채웠나).
    PanelSummary(u8),
    /// 프랙탈 자기합성: 비배경(invert면 배경) 셀 자리마다 입력 자신을 찍는다.
    Fractal(bool),
    /// 프랙탈 재색: 사본을 그 셀의 색으로 칠한다(색 정보까지 전파).
    FractalRecolor,
    /// 객체 수준 대칭 복원: 각 객체의 bbox 안에서 H·V·180 대칭으로 자기를 메운다.
    ObjSymFill,
    /// 정수 축소: k×k 블록이 균일할 때 대표 셀로 다운스케일.
    ScaleDown(u8),
    /// 1×1 답: 규칙 코드(0=다수색, 1=최대 객체색, 2=유일 색 객체의 색, 3=최소색).
    SingleCell(u8),
    /// 단색 답: (w, h) 크기의 격자를 규칙이 고른 색으로 채운다(속성 판정형).
    SolidAnswer(u8, u8, u8),
    /// 객체 재색칠: 속성 → 색 사상을 훈련에서 학습한다(마스크 보존 가족).
    /// 속성 코드: 0=면적, 1=면적 순위, 2=구멍 수, 3=형태 지문, 4=폭×높이.
    RecolorBy(u8, [u8; 10]),
    /// 주석 표시: 객체가 점유한 행/열 전체를 색 c로 표시(0=행, 1=열, 2=행+열).
    MarkLines(u8, u8),
    /// 쌍 잇기(신규색): 정렬된 같은 색 쌍 사이를 색 c로 연결.
    ConnectPairsColor(u8),
    /// 교차점 표시: 표식 셀들의 (열, 행) 교차 자리를 색 c로 찍는다.
    MarkIntersections(u8),
    /// 대칭 완성(신규색): 대칭으로 채워지는 칸을 색 c로 칠한다(0=전역, 1=내용 bbox).
    SymFillColor(u8, u8),
    /// 대각 광선 X: 각 1셀 객체에서 4대각 방향으로 그 색 광선(배경 위만).
    DiagRaysX,
    /// 전역 기하: 회전·전치·거울.
    Rot90,
    Rot180,
    Rot270,
    Transpose,
    MirrorHGrid,
    MirrorVGrid,
}

fn crop(g: &Grid, x0: usize, y0: usize, w: usize, h: usize) -> Grid {
    let mut o = Grid::new(w, h);
    for y in 0..h {
        for x in 0..w {
            o.set(x, y, g.get(x0 + x, y0 + y));
        }
    }
    o
}

/// 격자의 갇힌 배경(테두리 4-연결 도달 불가) 좌표.
pub fn grid_enclosed(g: &Grid) -> Vec<(usize, usize)> {
    let mut open = vec![false; g.w * g.h];
    let mut q: Vec<(usize, usize)> = Vec::new();
    for x in 0..g.w {
        for &y in &[0usize, g.h - 1] {
            if g.get(x, y) == 0 && !open[y * g.w + x] {
                open[y * g.w + x] = true;
                q.push((x, y));
            }
        }
    }
    for y in 0..g.h {
        for &x in &[0usize, g.w - 1] {
            if g.get(x, y) == 0 && !open[y * g.w + x] {
                open[y * g.w + x] = true;
                q.push((x, y));
            }
        }
    }
    while let Some((x, y)) = q.pop() {
        let nb = [(x as i32 + 1, y as i32), (x as i32 - 1, y as i32), (x as i32, y as i32 + 1), (x as i32, y as i32 - 1)];
        for (nx, ny) in nb {
            if g.in_bounds(nx, ny) {
                let (ux, uy) = (nx as usize, ny as usize);
                if g.get(ux, uy) == 0 && !open[uy * g.w + ux] {
                    open[uy * g.w + ux] = true;
                    q.push((ux, uy));
                }
            }
        }
    }
    let mut out = Vec::new();
    for y in 0..g.h {
        for x in 0..g.w {
            if g.get(x, y) == 0 && !open[y * g.w + x] {
                out.push((x, y));
            }
        }
    }
    out
}

/// 객체 속성 키(재색칠 사상의 정의역) — 0..9로 클램프.
fn obj_attr_key(objs: &[Obj], i: usize, attr: u8) -> u8 {
    let o = &objs[i];
    match attr {
        0 => o.area.min(9) as u8,
        1 => objs
            .iter()
            .filter(|q| (q.area, q.color) > (o.area, o.color))
            .count()
            .min(9) as u8,
        2 => {
            // 구멍 수(마스크 로컬, 4-연결 배경 성분 중 테두리 미접촉)
            let (w, h) = (o.w, o.h);
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
                for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                        let (ux, uy) = (nx as usize, ny as usize);
                        if !o.mask[uy * w + ux] && !open[uy * w + ux] {
                            open[uy * w + ux] = true;
                            q.push((ux, uy));
                        }
                    }
                }
            }
            let mut seen = open.clone();
            let mut holes = 0u8;
            for y in 0..h {
                for x in 0..w {
                    if o.mask[y * w + x] || seen[y * w + x] {
                        continue;
                    }
                    holes = holes.saturating_add(1);
                    let mut st = vec![(x, y)];
                    seen[y * w + x] = true;
                    while let Some((cx, cy)) = st.pop() {
                        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                            let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h {
                                let (ux, uy) = (nx as usize, ny as usize);
                                if !o.mask[uy * w + ux] && !seen[uy * w + ux] {
                                    seen[uy * w + ux] = true;
                                    st.push((ux, uy));
                                }
                            }
                        }
                    }
                }
            }
            holes.min(9)
        }
        3 => (o.shape_id() % 10) as u8,
        _ => ((o.w * o.h).min(9)) as u8,
    }
}

/// 하네스용 공개 래퍼(크기 변환 과제의 직접 적용 경로).
pub fn apply_grid_op_pub(g: &Grid, op: GridOp) -> Grid {
    apply_grid_op(g, op)
}

fn apply_grid_op(g: &Grid, op: GridOp) -> Grid {
    match op {
        GridOp::FillEnclosed(c) => {
            let mut o = g.clone();
            for (x, y) in grid_enclosed(g) {
                o.set(x, y, c);
            }
            o
        }
        GridOp::SymFillH => {
            let mut o = g.clone();
            for y in 0..g.h {
                for x in 0..g.w {
                    if o.get(x, y) == 0 {
                        let m = g.get(g.w - 1 - x, y);
                        if m != 0 {
                            o.set(x, y, m);
                        }
                    }
                }
            }
            o
        }
        GridOp::SymFillV => {
            let mut o = g.clone();
            for y in 0..g.h {
                for x in 0..g.w {
                    if o.get(x, y) == 0 {
                        let m = g.get(x, g.h - 1 - y);
                        if m != 0 {
                            o.set(x, y, m);
                        }
                    }
                }
            }
            o
        }
        GridOp::Scale(k) => {
            let k = k as usize;
            let mut o = Grid::new(g.w * k, g.h * k);
            for y in 0..o.h {
                for x in 0..o.w {
                    o.set(x, y, g.get(x / k, y / k));
                }
            }
            o
        }
        GridOp::Tile(nx, ny) => {
            let (nx, ny) = (nx as usize, ny as usize);
            let mut o = Grid::new(g.w * nx, g.h * ny);
            for y in 0..o.h {
                for x in 0..o.w {
                    o.set(x, y, g.get(x % g.w, y % g.h));
                }
            }
            o
        }
        GridOp::TileMirror4 => {
            let mut o = Grid::new(g.w * 2, g.h * 2);
            for y in 0..g.h {
                for x in 0..g.w {
                    let c = g.get(x, y);
                    o.set(x, y, c);
                    o.set(2 * g.w - 1 - x, y, c);
                    o.set(x, 2 * g.h - 1 - y, c);
                    o.set(2 * g.w - 1 - x, 2 * g.h - 1 - y, c);
                }
            }
            o
        }
        GridOp::ExtractLargest => {
            let objs = components(g);
            match objs.iter().max_by_key(|o| (o.area, o.color)) {
                Some(b) => crop(g, b.x0, b.y0, b.w, b.h),
                None => g.clone(),
            }
        }
        GridOp::ExtractUniqueColor => {
            let objs = components(g);
            let mut count: std::collections::HashMap<u8, usize> = std::collections::HashMap::new();
            for o in &objs {
                *count.entry(o.color).or_insert(0) += 1;
            }
            let uniq = objs
                .iter()
                .filter(|o| count.get(&o.color) == Some(&1))
                .min_by_key(|o| o.color);
            match uniq {
                Some(b) => crop(g, b.x0, b.y0, b.w, b.h),
                None => g.clone(),
            }
        }
        GridOp::ExtractWindow(w, h, rule) => {
            let (ww, hh) = (w as usize, h as usize);
            if ww == 0 || hh == 0 || ww > g.w || hh > g.h {
                return g.clone();
            }
            // 고유 내용 규칙의 중복 수를 해시 1패스로 미리 센다(O(n⁴) → O(n²·창))
            let dup_count: std::collections::HashMap<Vec<u8>, i64> = if rule == 4 {
                let mut m: std::collections::HashMap<Vec<u8>, i64> = Default::default();
                for y in 0..=(g.h - hh) {
                    for x in 0..=(g.w - ww) {
                        *m.entry(crop(g, x, y, ww, hh).cells).or_insert(0) += 1;
                    }
                }
                m
            } else {
                Default::default()
            };
            let mut best: Option<(i64, usize, usize)> = None;
            for y in 0..=(g.h - hh) {
                for x in 0..=(g.w - ww) {
                    let sub = crop(g, x, y, ww, hh);
                    let nz = sub.cells.iter().filter(|&&c| c != 0).count() as i64;
                    let mut set = [false; 10];
                    for &c in sub.cells.iter() {
                        set[c as usize] = true;
                    }
                    let kinds = set.iter().filter(|&&b| b).count() as i64;
                    let score = match rule {
                        0 => nz,
                        1 => -nz,
                        2 => kinds,
                        3 => -kinds,
                        // 고유 내용: 같은 내용의 창이 적을수록 좋다(해시 사전 계산)
                        _ => -dup_count.get(&sub.cells).copied().unwrap_or(1),
                    };
                    if best.map(|(b, _, _)| score > b).unwrap_or(true) {
                        best = Some((score, x, y));
                    }
                }
            }
            match best {
                Some((_, x, y)) => crop(g, x, y, ww, hh),
                None => g.clone(),
            }
        }
        GridOp::ExtractBy(rule) => {
            let objs = if rule == 4 {
                crate::grid::components_conn(g, true)
            } else {
                components(g)
            };
            if objs.is_empty() {
                return g.clone();
            }
            let pick = match rule {
                0 => {
                    // 형태(마스크)가 유일한 객체
                    objs.iter().position(|o| {
                        objs.iter().filter(|q| q.shape_id() == o.shape_id()).count() == 1
                    })
                }
                1 => {
                    // 형태가 최빈인 그룹의 첫 객체
                    (0..objs.len()).max_by_key(|&i| {
                        objs.iter().filter(|q| q.shape_id() == objs[i].shape_id()).count()
                    })
                }
                2 => (0..objs.len())
                    .max_by_key(|&i| objs.iter().filter(|q| q.color == objs[i].color).count()),
                3 => (0..objs.len())
                    .min_by_key(|&i| objs.iter().filter(|q| q.color == objs[i].color).count()),
                _ => (0..objs.len()).max_by_key(|&i| (objs[i].area, objs[i].color)),
            };
            match pick {
                Some(i) => {
                    let b = &objs[i];
                    crop(g, b.x0, b.y0, b.w, b.h)
                }
                None => g.clone(),
            }
        }
        GridOp::ExtractContent => {
            let mut bb: Option<(usize, usize, usize, usize)> = None;
            for y in 0..g.h {
                for x in 0..g.w {
                    if g.get(x, y) != 0 {
                        bb = Some(match bb {
                            None => (x, y, x, y),
                            Some((x0, y0, x1, y1)) => {
                                (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
                            }
                        });
                    }
                }
            }
            match bb {
                Some((x0, y0, x1, y1)) => crop(g, x0, y0, x1 - x0 + 1, y1 - y0 + 1),
                None => g.clone(),
            }
        }
        GridOp::ExtractFrameInterior => {
            // 속 빈 사각 액자(마스크 = bbox 테두리 링) 중 가장 큰 것의 내부
            let objs = components(g);
            let is_ring = |o: &Obj| -> bool {
                if o.w < 3 || o.h < 3 {
                    return false;
                }
                for y in 0..o.h {
                    for x in 0..o.w {
                        let border = x == 0 || y == 0 || x == o.w - 1 || y == o.h - 1;
                        if o.mask[y * o.w + x] != border {
                            return false;
                        }
                    }
                }
                true
            };
            match objs.iter().filter(|o| is_ring(o)).max_by_key(|o| o.area) {
                Some(f) => crop(g, f.x0 + 1, f.y0 + 1, f.w - 2, f.h - 2),
                None => g.clone(),
            }
        }
        GridOp::RemoveColor(c) => {
            let mut o = g.clone();
            for cell in o.cells.iter_mut() {
                if *cell == c {
                    *cell = 0;
                }
            }
            o
        }
        GridOp::PaletteMap(map) => {
            let mut o = g.clone();
            for cell in o.cells.iter_mut() {
                *cell = map[*cell as usize];
            }
            o
        }
        GridOp::Rot90 => {
            let mut o = Grid::new(g.h, g.w);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(g.h - 1 - y, x, g.get(x, y));
                }
            }
            o
        }
        GridOp::Rot180 => {
            let mut o = Grid::new(g.w, g.h);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(g.w - 1 - x, g.h - 1 - y, g.get(x, y));
                }
            }
            o
        }
        GridOp::Rot270 => {
            let mut o = Grid::new(g.h, g.w);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(y, g.w - 1 - x, g.get(x, y));
                }
            }
            o
        }
        GridOp::Transpose => {
            let mut o = Grid::new(g.h, g.w);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(y, x, g.get(x, y));
                }
            }
            o
        }
        GridOp::MirrorHGrid => {
            let mut o = Grid::new(g.w, g.h);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(g.w - 1 - x, y, g.get(x, y));
                }
            }
            o
        }
        GridOp::MirrorVGrid => {
            let mut o = Grid::new(g.w, g.h);
            for y in 0..g.h {
                for x in 0..g.w {
                    o.set(x, g.h - 1 - y, g.get(x, y));
                }
            }
            o
        }
        GridOp::ScaleDown(k) => {
            let k = k as usize;
            if k == 0 || g.w % k != 0 || g.h % k != 0 {
                return g.clone();
            }
            let mut o = Grid::new(g.w / k, g.h / k);
            for y in 0..o.h {
                for x in 0..o.w {
                    o.set(x, y, g.get(x * k, y * k));
                }
            }
            o
        }
        GridOp::MarkIntersections(fill) => {
            let mut o = g.clone();
            let mut marks: Vec<(usize, usize)> = Vec::new();
            for y in 0..g.h {
                for x in 0..g.w {
                    if g.get(x, y) != 0 {
                        marks.push((x, y));
                    }
                }
            }
            for &(px, _) in &marks {
                for &(_, qy) in &marks {
                    if o.get(px, qy) == 0 {
                        o.set(px, qy, fill);
                    }
                }
            }
            o
        }
        GridOp::ConnectPairsColor(fill) => {
            let mut o = g.clone();
            let objs = components(g);
            for i in 0..objs.len() {
                for j in (i + 1)..objs.len() {
                    let (a, b) = (&objs[i], &objs[j]);
                    if a.color != b.color {
                        continue;
                    }
                    let (acy, bcy) = (a.y0 + a.h / 2, b.y0 + b.h / 2);
                    let (acx, bcx) = (a.x0 + a.w / 2, b.x0 + b.w / 2);
                    if acy == bcy {
                        let (x1, x2) =
                            if acx < bcx { (a.x0 + a.w, b.x0) } else { (b.x0 + b.w, a.x0) };
                        for x in x1..x2 {
                            if o.get(x, acy) == 0 {
                                o.set(x, acy, fill);
                            }
                        }
                    } else if acx == bcx {
                        let (y1, y2) =
                            if acy < bcy { (a.y0 + a.h, b.y0) } else { (b.y0 + b.h, a.y0) };
                        for y in y1..y2 {
                            if o.get(acx, y) == 0 {
                                o.set(acx, y, fill);
                            }
                        }
                    }
                }
            }
            o
        }
        GridOp::SymFillColor(scope, fill) => {
            let base = if scope == 0 {
                apply_grid_op(g, GridOp::SymFillAll)
            } else {
                apply_grid_op(g, GridOp::SymFillBBox)
            };
            let mut o = g.clone();
            for i in 0..o.cells.len() {
                if g.cells[i] == 0 && base.cells[i] != 0 {
                    o.cells[i] = fill;
                }
            }
            o
        }
        GridOp::MarkLines(mode, c) => {
            let mut o = g.clone();
            let mut rows: Vec<bool> = vec![false; g.h];
            let mut cols: Vec<bool> = vec![false; g.w];
            for y in 0..g.h {
                for x in 0..g.w {
                    if g.get(x, y) != 0 {
                        rows[y] = true;
                        cols[x] = true;
                    }
                }
            }
            if mode == 0 || mode == 2 {
                for y in 0..g.h {
                    if rows[y] {
                        for x in 0..g.w {
                            if o.get(x, y) == 0 {
                                o.set(x, y, c);
                            }
                        }
                    }
                }
            }
            if mode == 1 || mode == 2 {
                for x in 0..g.w {
                    if cols[x] {
                        for y in 0..g.h {
                            if o.get(x, y) == 0 {
                                o.set(x, y, c);
                            }
                        }
                    }
                }
            }
            o
        }
        GridOp::RecolorBy(attr, map) => {
            let objs = components(g);
            let mut o = g.clone();
            for (i, ob) in objs.iter().enumerate() {
                let key = obj_attr_key(&objs, i, attr);
                let c = map[key.min(9) as usize];
                if c == 0 {
                    continue;
                }
                stamp(&mut o, ob, ob.x0, ob.y0, c);
            }
            o
        }
        GridOp::SolidAnswer(w, h, rule) => {
            let objs = components(g);
            let mut cnt = [0usize; 10];
            for &c in g.cells.iter() {
                if c != 0 {
                    cnt[c as usize] += 1;
                }
            }
            let color = match rule {
                0 => (1..10).max_by_key(|&c| (cnt[c], c)).unwrap_or(0) as u8,
                1 => objs.iter().max_by_key(|o| (o.area, o.color)).map(|o| o.color).unwrap_or(0),
                2 => objs
                    .iter()
                    .find(|o| objs.iter().filter(|q| q.shape_id() == o.shape_id()).count() == 1)
                    .map(|o| o.color)
                    .unwrap_or(0),
                3 => objs
                    .iter()
                    .max_by_key(|o| objs.iter().filter(|q| q.shape_id() == o.shape_id()).count())
                    .map(|o| o.color)
                    .unwrap_or(0),
                _ => (1..10)
                    .filter(|&c| cnt[c] > 0)
                    .min_by_key(|&c| (cnt[c], c))
                    .unwrap_or(0) as u8,
            };
            let mut o = Grid::new(w as usize, h as usize);
            for c in o.cells.iter_mut() {
                *c = color;
            }
            o
        }
        GridOp::SingleCell(rule) => {
            let objs = components(g);
            let mut cnt = [0usize; 10];
            for &c in g.cells.iter() {
                if c != 0 {
                    cnt[c as usize] += 1;
                }
            }
            let color = match rule {
                0 => (1..10).max_by_key(|&c| (cnt[c], c)).unwrap_or(0) as u8,
                1 => objs
                    .iter()
                    .max_by_key(|o| (o.area, o.color))
                    .map(|o| o.color)
                    .unwrap_or(0),
                2 => {
                    let mut per: std::collections::HashMap<u8, usize> = Default::default();
                    for o in &objs {
                        *per.entry(o.color).or_insert(0) += 1;
                    }
                    // 결정론: 유일 색이 여럿이면 최소 색 — HashMap 순회의 find는
                    // 실행마다 다른 답(74↔73 요동의 원인 후보)을 낸다.
                    per.iter()
                        .filter(|(_, &n)| n == 1)
                        .map(|(&c, _)| c)
                        .min()
                        .unwrap_or(0)
                }
                _ => (1..10)
                    .filter(|&c| cnt[c] > 0)
                    .min_by_key(|&c| (cnt[c], c))
                    .unwrap_or(0) as u8,
            };
            let mut o = Grid::new(1, 1);
            o.set(0, 0, color);
            o
        }
        GridOp::DiagRaysX => {
            let mut o = g.clone();
            for obj in components(g) {
                if obj.area != 1 {
                    continue;
                }
                let (cx, cy) = (obj.x0 as i32, obj.y0 as i32);
                for (sx, sy) in [(1i32, 1i32), (1, -1), (-1, 1), (-1, -1)] {
                    let (mut x, mut y) = (cx + sx, cy + sy);
                    while o.in_bounds(x, y) {
                        if o.get(x as usize, y as usize) == 0 {
                            o.set(x as usize, y as usize, obj.color);
                        }
                        x += sx;
                        y += sy;
                    }
                }
            }
            o
        }
        GridOp::SymFillAll => {
            let mut o = g.clone();
            for _ in 0..8 {
                let before = o.cells.clone();
                for y in 0..o.h {
                    for x in 0..o.w {
                        if o.get(x, y) != 0 {
                            continue;
                        }
                        // 후보 대칭 짝들: 수평·수직·180°·전치(정사각일 때)
                        let mut v = o.get(o.w - 1 - x, y);
                        if v == 0 {
                            v = o.get(x, o.h - 1 - y);
                        }
                        if v == 0 {
                            v = o.get(o.w - 1 - x, o.h - 1 - y);
                        }
                        if v == 0 && o.w == o.h {
                            v = o.get(y, x);
                        }
                        if v != 0 {
                            o.set(x, y, v);
                        }
                    }
                }
                if o.cells == before {
                    break;
                }
            }
            o
        }
        GridOp::Fractal(invert) => {
            let mut o = Grid::new(g.w * g.w, g.h * g.h);
            for by in 0..g.h {
                for bx in 0..g.w {
                    let on = if invert { g.get(bx, by) == 0 } else { g.get(bx, by) != 0 };
                    if !on {
                        continue;
                    }
                    for y in 0..g.h {
                        for x in 0..g.w {
                            o.set(bx * g.w + x, by * g.h + y, g.get(x, y));
                        }
                    }
                }
            }
            o
        }
        GridOp::ObjSymFill => {
            let mut o = g.clone();
            for obj in components(g) {
                // bbox 안에서 자기 대칭(H·V·180)으로 빈 칸을 메운다
                for _ in 0..4 {
                    let mut changed = false;
                    for dy in 0..obj.h {
                        for dx in 0..obj.w {
                            let (x, y) = (obj.x0 + dx, obj.y0 + dy);
                            if o.get(x, y) != 0 {
                                continue;
                            }
                            let mut v = o.get(obj.x0 + (obj.w - 1 - dx), y);
                            if v == 0 {
                                v = o.get(x, obj.y0 + (obj.h - 1 - dy));
                            }
                            if v == 0 {
                                v = o.get(
                                    obj.x0 + (obj.w - 1 - dx),
                                    obj.y0 + (obj.h - 1 - dy),
                                );
                            }
                            if v != 0 {
                                o.set(x, y, v);
                                changed = true;
                            }
                        }
                    }
                    if !changed {
                        break;
                    }
                }
            }
            o
        }
        GridOp::FractalRecolor => {
            let mut o = Grid::new(g.w * g.w, g.h * g.h);
            for by in 0..g.h {
                for bx in 0..g.w {
                    let c = g.get(bx, by);
                    if c == 0 {
                        continue;
                    }
                    for y in 0..g.h {
                        for x in 0..g.w {
                            if g.get(x, y) != 0 {
                                o.set(bx * g.w + x, by * g.h + y, c);
                            }
                        }
                    }
                }
            }
            o
        }
        GridOp::PanelSummary(rule) => match split_grid_cells(g) {
            None => g.clone(),
            Some((rows, cols, ps)) => {
                let mut o = Grid::new(cols, rows);
                for r in 0..rows {
                    for c in 0..cols {
                        let p = &ps[r * cols + c];
                        let mut cnt = [0usize; 10];
                        for &v in p.cells.iter() {
                            if v != 0 {
                                cnt[v as usize] += 1;
                            }
                        }
                        let dom = (1..10).max_by_key(|&k| (cnt[k], k)).unwrap_or(0);
                        let filled = cnt.iter().skip(1).any(|&n| n > 0);
                        let v = match rule {
                            0 => {
                                if filled {
                                    dom as u8
                                } else {
                                    0
                                }
                            }
                            _ => filled as u8,
                        };
                        o.set(c, r, v);
                    }
                }
                o
            }
        },
        GridOp::PanelSelect(rule) => {
            let panels = split_panels_any(g);
            match panels {
                None => g.clone(),
                Some(ps) => {
                    let nz = |p: &Grid| p.cells.iter().filter(|&&c| c != 0).count();
                    let pick = match rule {
                        0 => (0..ps.len()).find(|&i| {
                            (0..ps.len()).all(|j| j == i || ps[j].cells != ps[i].cells)
                                && (0..ps.len()).filter(|&j| j != i).count() > 0
                        }),
                        1 => {
                            // 최빈: 같은 내용이 가장 많은 그룹의 대표
                            let mut best: Option<(usize, usize)> = None;
                            for i in 0..ps.len() {
                                let n = ps.iter().filter(|q| q.cells == ps[i].cells).count();
                                if best.map(|(_, bn)| n > bn).unwrap_or(true) {
                                    best = Some((i, n));
                                }
                            }
                            best.map(|(i, _)| i)
                        }
                        2 => (0..ps.len()).max_by_key(|&i| nz(&ps[i])),
                        _ => (0..ps.len()).min_by_key(|&i| nz(&ps[i])),
                    };
                    match pick {
                        Some(i) => ps[i].clone(),
                        None => g.clone(),
                    }
                }
            }
        }
        GridOp::PeriodicFill(mark) | GridOp::PeriodicPatch(mark) => {
            let occ = |c: u8| c == mark;
            // 비폐색 셀에 모순 없는 최소 주기를 찾는다(합동류 색 일치)
            let mut best: Option<(usize, usize)> = None;
            'outer: for py in 1..=g.h {
                for px in 1..=g.w {
                    if px == g.w && py == g.h {
                        continue;
                    }
                    let mut cls: std::collections::HashMap<(usize, usize), u8> =
                        Default::default();
                    let mut ok_p = true;
                    let mut covered = true;
                    for y in 0..g.h {
                        for x in 0..g.w {
                            let c = g.get(x, y);
                            if occ(c) {
                                continue;
                            }
                            let k = (x % px, y % py);
                            match cls.get(&k) {
                                None => {
                                    cls.insert(k, c);
                                }
                                Some(&e) if e != c => {
                                    ok_p = false;
                                }
                                _ => {}
                            }
                        }
                        if !ok_p {
                            break;
                        }
                    }
                    if !ok_p {
                        continue;
                    }
                    // 폐색 셀이 전부 채워질 수 있어야 한다
                    for y in 0..g.h {
                        for x in 0..g.w {
                            if occ(g.get(x, y)) && !cls.contains_key(&(x % px, y % py)) {
                                covered = false;
                            }
                        }
                    }
                    if covered {
                        best = Some((px, py));
                        break 'outer;
                    }
                }
            }
            let mut o = g.clone();
            if let Some((px, py)) = best {
                let mut cls: std::collections::HashMap<(usize, usize), u8> = Default::default();
                for y in 0..g.h {
                    for x in 0..g.w {
                        let c = g.get(x, y);
                        if !occ(c) {
                            cls.entry((x % px, y % py)).or_insert(c);
                        }
                    }
                }
                for y in 0..g.h {
                    for x in 0..g.w {
                        if occ(g.get(x, y)) {
                            if let Some(&c) = cls.get(&(x % px, y % py)) {
                                o.set(x, y, c);
                            }
                        }
                    }
                }
            }
            if let GridOp::PeriodicPatch(_) = op {
                let mut bb: Option<(usize, usize, usize, usize)> = None;
                for y in 0..g.h {
                    for x in 0..g.w {
                        if occ(g.get(x, y)) {
                            bb = Some(match bb {
                                None => (x, y, x, y),
                                Some((x0, y0, x1, y1)) => {
                                    (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
                                }
                            });
                        }
                    }
                }
                if let Some((x0, y0, x1, y1)) = bb {
                    return crop(&o, x0, y0, x1 - x0 + 1, y1 - y0 + 1);
                }
            }
            o
        }
        GridOp::RepeatToEdge(dir) => {
            let mut o = g.clone();
            for obj in components(g) {
                let (sx, sy): (i32, i32) = match dir {
                    0 => (obj.w as i32, 0),
                    1 => (-(obj.w as i32), 0),
                    2 => (0, obj.h as i32),
                    3 => (0, -(obj.h as i32)),
                    4 => (obj.w as i32, 0),
                    _ => (0, obj.h as i32),
                };
                let both = dir >= 4;
                for sign in [1i32, -1] {
                    if !both && sign == -1 {
                        continue;
                    }
                    let (dx, dy) = (sx * sign, sy * sign);
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (mut x, mut y) = (obj.x0 as i32 + dx, obj.y0 as i32 + dy);
                    while x >= 0
                        && y >= 0
                        && (x as usize) + obj.w <= g.w
                        && (y as usize) + obj.h <= g.h
                    {
                        stamp(&mut o, &obj, x as usize, y as usize, obj.color);
                        x += dx;
                        y += dy;
                    }
                }
            }
            o
        }
        GridOp::SymFillBBox => {
            // 비배경 내용물의 bbox를 잘라 대칭 복원 후 제자리에 되붙인다
            let mut bb: Option<(usize, usize, usize, usize)> = None;
            for y in 0..g.h {
                for x in 0..g.w {
                    if g.get(x, y) != 0 {
                        bb = Some(match bb {
                            None => (x, y, x, y),
                            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                        });
                    }
                }
            }
            match bb {
                None => g.clone(),
                Some((x0, y0, x1, y1)) => {
                    let sub = crop(g, x0, y0, x1 - x0 + 1, y1 - y0 + 1);
                    let filled = apply_grid_op(&sub, GridOp::SymFillAll);
                    let mut o = g.clone();
                    for y in 0..filled.h {
                        for x in 0..filled.w {
                            o.set(x0 + x, y0 + y, filled.get(x, y));
                        }
                    }
                    o
                }
            }
        }
        GridOp::SymFillPatch | GridOp::SymFillPatchColor(_) => {
            // 폐색 마스크: 배경(0) 또는 지정된 표시색
            let occ = |c: u8| match op {
                GridOp::SymFillPatchColor(k) => c == k,
                _ => c == 0,
            };
            let mut work = g.clone();
            if let GridOp::SymFillPatchColor(_) = op {
                for c in work.cells.iter_mut() {
                    if occ(*c) {
                        *c = 0;
                    }
                }
            }
            let filled = apply_grid_op(&work, GridOp::SymFillAll);
            // 원래 폐색이던 셀들의 bbox만 잘라 반환
            let mut bb: Option<(usize, usize, usize, usize)> = None;
            for y in 0..g.h {
                for x in 0..g.w {
                    if occ(g.get(x, y)) {
                        bb = Some(match bb {
                            None => (x, y, x, y),
                            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                        });
                    }
                }
            }
            match bb {
                Some((x0, y0, x1, y1)) => crop(&filled, x0, y0, x1 - x0 + 1, y1 - y0 + 1),
                None => filled,
            }
        }
        GridOp::ConnectPairs => {
            let mut o = g.clone();
            let objs = components(g);
            for i in 0..objs.len() {
                for j in (i + 1)..objs.len() {
                    let (a, b) = (&objs[i], &objs[j]);
                    if a.color != b.color {
                        continue;
                    }
                    // 행 정렬(같은 y0·같은 h=1 중심행 근사: 중심행 일치)
                    let acy = a.y0 + a.h / 2;
                    let bcy = b.y0 + b.h / 2;
                    let acx = a.x0 + a.w / 2;
                    let bcx = b.x0 + b.w / 2;
                    if acy == bcy {
                        let (x1, x2) = if acx < bcx {
                            (a.x0 + a.w, b.x0)
                        } else {
                            (b.x0 + b.w, a.x0)
                        };
                        for x in x1..x2 {
                            if o.get(x, acy) == 0 {
                                o.set(x, acy, a.color);
                            }
                        }
                    } else if acx == bcx {
                        let (y1, y2) = if acy < bcy {
                            (a.y0 + a.h, b.y0)
                        } else {
                            (b.y0 + b.h, a.y0)
                        };
                        for y in y1..y2 {
                            if o.get(acx, y) == 0 {
                                o.set(acx, y, a.color);
                            }
                        }
                    }
                }
            }
            o
        }
        GridOp::PeriodicRepair(px, py) => {
            let (px, py) = (px as usize, py as usize);
            let mut o = g.clone();
            for cy in 0..py.min(g.h) {
                for cx in 0..px.min(g.w) {
                    // 합동류의 다수결 대표(배경 제외)
                    let mut cnt = [0usize; 10];
                    let mut y = cy;
                    while y < g.h {
                        let mut x = cx;
                        while x < g.w {
                            let c = g.get(x, y);
                            if c != 0 {
                                cnt[c as usize] += 1;
                            }
                            x += px;
                        }
                        y += py;
                    }
                    let rep = (1..10).max_by_key(|&c| cnt[c]).unwrap_or(1) as u8;
                    if cnt[rep as usize] == 0 {
                        continue;
                    }
                    let mut y = cy;
                    while y < g.h {
                        let mut x = cx;
                        while x < g.w {
                            o.set(x, y, rep);
                            x += px;
                        }
                        y += py;
                    }
                }
            }
            o
        }
    }
}

/// 훈련쌍 전부를 정확 재현하는 격자 수준 연산 탐색(크기 변환 포함).
pub fn try_grid_ops(train: &[(Grid, Grid)]) -> Option<GridOp> {
    let ok = |op: GridOp| train.iter().all(|(gi, go)| &apply_grid_op(gi, op) == go);
    // 후보 색: 훈련 출력에서 입력에 없던 색들
    let mut cand_colors: std::collections::BTreeSet<u8> = Default::default();
    for (gi, go) in train {
        let in_c: std::collections::HashSet<u8> =
            gi.cells.iter().copied().filter(|&c| c != 0).collect();
        for &c in go.cells.iter() {
            if c != 0 && !in_c.contains(&c) {
                cand_colors.insert(c);
            }
        }
    }
    for &c in &cand_colors {
        let op = GridOp::FillEnclosed(c);
        if ok(op) {
            return Some(op);
        }
    }
    // 객체 재색칠(마스크 보존 가족 52건 표적): 속성→색 사상을 훈련에서 학습
    if train.iter().all(|(gi, go)| {
        gi.w == go.w
            && gi.h == go.h
            && gi
                .cells
                .iter()
                .zip(go.cells.iter())
                .all(|(a, b)| (*a == 0) == (*b == 0))
    }) {
        for attr in 0..5u8 {
            let mut map = [0u8; 10];
            let mut consistent = true;
            'pairs: for (gi, go) in train {
                let objs = components(gi);
                for (i, ob) in objs.iter().enumerate() {
                    let key = obj_attr_key(&objs, i, attr).min(9) as usize;
                    // 출력에서 이 객체 자리의 색(첫 마스크 셀 기준)
                    let mut oc = 0u8;
                    'find: for dy in 0..ob.h {
                        for dx in 0..ob.w {
                            if ob.mask[dy * ob.w + dx] {
                                oc = go.get(ob.x0 + dx, ob.y0 + dy);
                                break 'find;
                            }
                        }
                    }
                    if map[key] == 0 {
                        map[key] = oc;
                    } else if map[key] != oc {
                        consistent = false;
                        break 'pairs;
                    }
                }
            }
            // 가드(시도 126의 스파이크 회귀 교훈): **속성 의존적일 때만** 채택한다.
            // 모든 객체가 같은 색으로 가면 그것은 전역 재색칠(PaletteMap)의 몫이고,
            // 여기서 가로채면 더 단순한 설명을 밀어낸다 — 오컴의 면도날.
            let distinct: std::collections::HashSet<u8> =
                map.iter().copied().filter(|&c| c != 0).collect();
            if consistent && distinct.len() >= 2 {
                let op = GridOp::RecolorBy(attr, map);
                if ok(op) {
                    return Some(op);
                }
            }
        }
    }
    // 신규색 주석 가족(현미경으로 분리한 세 기전 중 둘): 쌍 잇기·대칭 완성의 색 변주
    for &c in &cand_colors {
        if ok(GridOp::ConnectPairsColor(c)) {
            return Some(GridOp::ConnectPairsColor(c));
        }
        if ok(GridOp::MarkIntersections(c)) {
            return Some(GridOp::MarkIntersections(c));
        }
        for scope in 0..2u8 {
            let op = GridOp::SymFillColor(scope, c);
            if ok(op) {
                return Some(op);
            }
        }
        for mode in 0..3u8 {
            let op = GridOp::MarkLines(mode, c);
            if ok(op) {
                return Some(op);
            }
        }
    }
    if ok(GridOp::ConnectPairs) {
        return Some(GridOp::ConnectPairs);
    }
    if ok(GridOp::DiagRaysX) {
        return Some(GridOp::DiagRaysX);
    }
    if ok(GridOp::ObjSymFill) {
        return Some(GridOp::ObjSymFill);
    }
    // 생성형 최대 버킷의 일반 기계: 객체를 제 크기만큼 반복해 가장자리까지
    for d in 0..6u8 {
        let op = GridOp::RepeatToEdge(d);
        if ok(op) {
            return Some(op);
        }
    }
    for op in [
        GridOp::SymFillH,
        GridOp::SymFillV,
        GridOp::SymFillAll,
        GridOp::SymFillBBox,
        GridOp::Rot90,
        GridOp::Rot180,
        GridOp::Rot270,
        GridOp::Transpose,
        GridOp::MirrorHGrid,
        GridOp::MirrorVGrid,
    ] {
        if ok(op) {
            return Some(op);
        }
    }
    // 전역 팔레트 치환(동일 크기 전제): 셀별 대응에서 사상 학습, 충돌 시 기각
    if train.iter().all(|(gi, go)| gi.w == go.w && gi.h == go.h) {
        let mut map = [255u8; 10];
        let mut consistent = true;
        'outer: for (gi, go) in train {
            for (a, b) in gi.cells.iter().zip(go.cells.iter()) {
                let e = &mut map[*a as usize];
                if *e == 255 {
                    *e = *b;
                } else if *e != *b {
                    consistent = false;
                    break 'outer;
                }
            }
        }
        if consistent {
            // 미관측 색의 완성: 관측된 비배경 사상이 전부 한 색(상수 사상)이면
            // 미관측도 그 상수로(칠하기형 일반화), 아니면 항등으로(교환형 일반화).
            let seen_targets: std::collections::HashSet<u8> = (1..10)
                .filter(|&i| map[i] != 255)
                .map(|i| map[i])
                .collect();
            let constant = if seen_targets.len() == 1 {
                seen_targets.iter().next().copied()
            } else {
                None
            };
            for i in 0..10 {
                if map[i] == 255 {
                    map[i] = match (i, constant) {
                        (0, _) => 0,
                        (_, Some(c)) => c,
                        (_, None) => i as u8,
                    };
                }
            }
            let op = GridOp::PaletteMap(map);
            let identity = (0..10).all(|i| map[i] == i as u8);
            if !identity && ok(op) {
                return Some(op);
            }
        }
    }
    // 주기 수선(동일 크기): 작은 주기부터 — 훈련 전부 정확 재현만 채택
    if train.iter().all(|(gi, go)| gi.w == go.w && gi.h == go.h) {
        let (w0, h0) = (train[0].0.w, train[0].0.h);
        for py in 1..=h0.min(8) {
            for px in 1..=w0.min(8) {
                if px == w0 && py == h0 {
                    continue;
                }
                let op = GridOp::PeriodicRepair(px as u8, py as u8);
                if ok(op) {
                    return Some(op);
                }
            }
        }
    }
    // 색 소멸(입력에 있고 출력에 없는 색) → 잡음 제거 가설
    {
        let mut vanish: std::collections::BTreeSet<u8> = Default::default();
        for (gi, go) in train {
            let out_c: std::collections::HashSet<u8> =
                go.cells.iter().copied().filter(|&c| c != 0).collect();
            for &c in gi.cells.iter() {
                if c != 0 && !out_c.contains(&c) {
                    vanish.insert(c);
                }
            }
        }
        for &c in &vanish {
            let op = GridOp::RemoveColor(c);
            if ok(op) {
                return Some(op);
            }
        }
    }
    // 크기 변환 후보는 첫 쌍의 크기 비율에서 도출
    if let Some((gi0, go0)) = train.first() {
        if go0.w % gi0.w == 0 && go0.h % gi0.h == 0 {
            let (nx, ny) = (go0.w / gi0.w, go0.h / gi0.h);
            if nx == ny && nx > 1 && nx <= 6 && ok(GridOp::Scale(nx as u8)) {
                return Some(GridOp::Scale(nx as u8));
            }
            if nx == 2 && ny == 2 && ok(GridOp::TileMirror4) {
                return Some(GridOp::TileMirror4);
            }
            if nx * ny > 1 && nx <= 4 && ny <= 4 && ok(GridOp::Tile(nx as u8, ny as u8)) {
                return Some(GridOp::Tile(nx as u8, ny as u8));
            }
            // 프랙탈: 출력이 입력의 (w×w, h×h)일 때 자기합성 가설
            if go0.w == gi0.w * gi0.w && go0.h == gi0.h * gi0.h {
                for inv in [false, true] {
                    if ok(GridOp::Fractal(inv)) {
                        return Some(GridOp::Fractal(inv));
                    }
                }
                if ok(GridOp::FractalRecolor) {
                    return Some(GridOp::FractalRecolor);
                }
            }
        }
    }
    // 단색 답 가족(속성 판정형): 출력이 전부 같은 크기이고 단색일 때
    {
        let same_size = train
            .windows(2)
            .all(|w| w[0].1.w == w[1].1.w && w[0].1.h == w[1].1.h);
        let solid = train
            .iter()
            .all(|(_, go)| go.cells.windows(2).all(|c| c[0] == c[1]));
        if same_size && solid && !train.is_empty() {
            let (ow, oh) = (train[0].1.w, train[0].1.h);
            if ow <= 30 && oh <= 30 {
                for rule in 0..5u8 {
                    let op = GridOp::SolidAnswer(ow as u8, oh as u8, rule);
                    if ok(op) {
                        return Some(op);
                    }
                }
            }
        }
    }
    // 1×1 답 가족
    if train.iter().all(|(_, go)| go.w == 1 && go.h == 1) {
        for rule in 0..4u8 {
            let op = GridOp::SingleCell(rule);
            if ok(op) {
                return Some(op);
            }
        }
    }
    // 정수 축소(비율에서 k 도출)
    if let Some((gi0, go0)) = train.first() {
        if go0.w > 0 && gi0.w % go0.w == 0 && gi0.h % go0.h.max(1) == 0 {
            let kx = gi0.w / go0.w;
            let ky = gi0.h / go0.h.max(1);
            if kx == ky && kx > 1 && kx <= 6 && ok(GridOp::ScaleDown(kx as u8)) {
                return Some(GridOp::ScaleDown(kx as u8));
            }
        }
    }
    // 주기 폐색 복구(동일 크기): 비폐색 보존 + 주기로 메우기
    if train.iter().all(|(gi, go)| gi.w == go.w && gi.h == go.h) {
        if ok(GridOp::PeriodicFill(0)) {
            return Some(GridOp::PeriodicFill(0));
        }
        let mut marks: std::collections::BTreeSet<u8> = Default::default();
        for (gi, go) in train {
            let out_c: std::collections::HashSet<u8> = go.cells.iter().copied().collect();
            for &c in gi.cells.iter() {
                if c != 0 && !out_c.contains(&c) {
                    marks.insert(c);
                }
            }
        }
        for &m in &marks {
            if ok(GridOp::PeriodicFill(m)) {
                return Some(GridOp::PeriodicFill(m));
            }
        }
    }
    // 폐색 패치 복구(원리 축): 출력이 입력보다 작을 때 대칭/주기 복원 후 가려진 조각
    if train.iter().all(|(gi, go)| go.w <= gi.w && go.h <= gi.h) {
        if ok(GridOp::SymFillPatch) {
            return Some(GridOp::SymFillPatch);
        }
        if ok(GridOp::PeriodicPatch(0)) {
            return Some(GridOp::PeriodicPatch(0));
        }
        // 패널 선택(원리 축): 고유·최빈·최다·최소
        for rule in 0..4u8 {
            let op = GridOp::PanelSelect(rule);
            if ok(op) {
                return Some(op);
            }
        }
        // 패널 요약: 2차원 패널 격자 → 패널당 한 셀
        for rule in 0..2u8 {
            let op = GridOp::PanelSummary(rule);
            if ok(op) {
                return Some(op);
            }
        }
        // 표시색 후보: 입력에 있고 출력에 없는 색(가림막)
        let mut marks: std::collections::BTreeSet<u8> = Default::default();
        for (gi, go) in train {
            let out_c: std::collections::HashSet<u8> = go.cells.iter().copied().collect();
            for &c in gi.cells.iter() {
                if c != 0 && !out_c.contains(&c) {
                    marks.insert(c);
                }
            }
        }
        for &m in &marks {
            let op = GridOp::SymFillPatchColor(m);
            if ok(op) {
                return Some(op);
            }
            let op2 = GridOp::PeriodicPatch(m);
            if ok(op2) {
                return Some(op2);
            }
        }
    }
    // 축소 계열: 출력이 입력보다 작으면 추출 가설
    if train.iter().all(|(gi, go)| go.w <= gi.w && go.h <= gi.h) {
        for op in [
            GridOp::ExtractLargest,
            GridOp::ExtractUniqueColor,
            GridOp::ExtractContent,
            GridOp::ExtractFrameInterior,
        ] {
            if ok(op) {
                return Some(op);
            }
        }
        // 선택 규칙별 추출(형태 유일·형태 최빈·색 최빈/최소빈·8연결 최대)
        for rule in 0..5u8 {
            let op = GridOp::ExtractBy(rule);
            if ok(op) {
                return Some(op);
            }
        }
        // 부분격자 창 추출(출력 크기가 전 훈련쌍에서 같을 때)
        if let Some((first, rest)) = train.split_first() {
            let (ow, oh) = (first.1.w, first.1.h);
            if rest.iter().all(|(_, o)| o.w == ow && o.h == oh) && ow <= 30 && oh <= 30 {
                for rule in 0..5u8 {
                    let op = GridOp::ExtractWindow(ow as u8, oh as u8, rule);
                    if ok(op) {
                        return Some(op);
                    }
                }
            }
        }
    }
    None
}

/// W2-3 최소형 — 2단 연쇄: 1단 후보(정규화·추출류)를 적용한 중간 훈련쌍에
/// 단일 연산 탐색을 재귀 적용한다. "추출 후 확대" 같은 합성이 잡힌다.
/// anytime 계약의 씨앗: 깊이가 예산이다(현재 2).
pub fn try_grid_chain(train: &[(Grid, Grid)]) -> Option<(GridOp, Option<GridOp>)> {
    if let Some(op) = try_grid_ops(train) {
        return Some((op, None));
    }
    let mut firsts: Vec<GridOp> = vec![
        GridOp::ExtractLargest,
        GridOp::ExtractUniqueColor,
        GridOp::ExtractContent,
        GridOp::ExtractFrameInterior,
        GridOp::SymFillH,
        GridOp::SymFillV,
        GridOp::SymFillAll,
        GridOp::Rot90,
        GridOp::Rot180,
        GridOp::Transpose,
        GridOp::MirrorHGrid,
        GridOp::MirrorVGrid,
    ];
    // 동일 크기면 팔레트/주기 정규화도 1단 후보로(정규화 후 기하·추출 합성)
    if train.iter().all(|(gi, go)| gi.w == go.w && gi.h == go.h) {
        for py in [1usize, 2, 3] {
            for px in [1usize, 2, 3] {
                if px * py > 1 {
                    firsts.push(GridOp::PeriodicRepair(px as u8, py as u8));
                }
            }
        }
    }
    // 소멸 색 → 잡음 제거 1단 후보(제거 후 추출/확대가 흔한 합성)
    for (gi, go) in train {
        let out_c: std::collections::HashSet<u8> =
            go.cells.iter().copied().filter(|&c| c != 0).collect();
        for &c in gi.cells.iter() {
            if c != 0
                && !out_c.contains(&c)
                && !firsts.iter().any(|f| matches!(f, GridOp::RemoveColor(x) if *x == c))
            {
                firsts.push(GridOp::RemoveColor(c));
            }
        }
    }
    // 채움 1단 후보 색(출력에서 새로 등장하는 색)
    for (gi, go) in train {
        let in_c: std::collections::HashSet<u8> =
            gi.cells.iter().copied().filter(|&c| c != 0).collect();
        for &c in go.cells.iter() {
            if c != 0 && !in_c.contains(&c) {
                let op = GridOp::FillEnclosed(c);
                if !firsts.iter().any(|f| matches!(f, GridOp::FillEnclosed(x) if *x == c)) {
                    firsts.push(op);
                }
            }
        }
    }
    for op1 in firsts {
        let mid: Vec<(Grid, Grid)> = train
            .iter()
            .map(|(i, o)| (apply_grid_op(i, op1), o.clone()))
            .collect();
        // 1단이 입력을 바꾸지 못했다면 연쇄 의미 없음
        if mid.iter().zip(train.iter()).all(|((m, _), (i, _))| m == i) {
            continue;
        }
        if let Some(op2) = try_grid_ops(&mid) {
            return Some((op1, Some(op2)));
        }
    }
    None
}

/// 깊이 3 연쇄(W2-3): 1단 후보 × 2단 후보 × 단일 탐색 — 훈련 정확 재현 게이트.
/// anytime 계약: 깊이가 예산이다(현재 3, 검증은 ~ms라 전수 가능).
pub fn try_grid_chain3(train: &[(Grid, Grid)]) -> Option<Vec<GridOp>> {
    if let Some((a, b)) = try_grid_chain(train) {
        let mut v = vec![a];
        if let Some(b) = b {
            v.push(b);
        }
        return Some(v);
    }
    let firsts = [
        GridOp::ExtractLargest,
        GridOp::ExtractUniqueColor,
        GridOp::ExtractContent,
        GridOp::ExtractFrameInterior,
        GridOp::MirrorHGrid,
        GridOp::MirrorVGrid,
        GridOp::Rot90,
        GridOp::Rot180,
        GridOp::Transpose,
        // anytime 2단: 크기 연산도 1단 후보로(추출→확대, 축소→기하 등 합성)
        GridOp::Scale(2),
        GridOp::Scale(3),
        GridOp::ScaleDown(2),
        GridOp::TileMirror4,
    ];
    for op1 in firsts {
        let mid1: Vec<(Grid, Grid)> =
            train.iter().map(|(i, o)| (apply_grid_op(i, op1), o.clone())).collect();
        if mid1.iter().zip(train.iter()).all(|((m, _), (i, _))| m == i) {
            continue;
        }
        for op2 in firsts {
            let mid2: Vec<(Grid, Grid)> =
                mid1.iter().map(|(i, o)| (apply_grid_op(i, op2), o.clone())).collect();
            if mid2.iter().zip(mid1.iter()).all(|((m, _), (i, _))| m == i) {
                continue;
            }
            if let Some(op3) = try_grid_ops(&mid2) {
                return Some(vec![op1, op2, op3]);
            }
        }
    }
    None
}

/// 깊이 N 연쇄 적용.
pub fn apply_grid_chain_n(g: &Grid, chain: &[GridOp]) -> Grid {
    let mut cur = g.clone();
    for &op in chain {
        cur = apply_grid_op(&cur, op);
    }
    cur
}

/// 연쇄 적용(하네스용).
pub fn apply_grid_chain(g: &Grid, chain: (GridOp, Option<GridOp>)) -> Grid {
    let mid = apply_grid_op(g, chain.0);
    match chain.1 {
        Some(op2) => apply_grid_op(&mid, op2),
        None => mid,
    }
}

/// 패널 결합: 구분선(단색 행/열)으로 나뉜 두 동형 패널의 셀별 함수.
/// (a,b)→c 사상을 훈련 전체에서 학습(충돌 시 기각), 훈련 정확 재현만 채택.
pub struct PanelCombine {
    /// true=세로 구분(좌|우), false=가로 구분(상/하)
    pub vertical: bool,
    pub table: std::collections::HashMap<(u8, u8), u8>,
}

fn split_panels(g: &Grid) -> Option<(bool, Grid, Grid)> {
    // 세로 구분선: 어떤 열 전체가 같은 비배경 색이고 좌우 폭 동일
    for x in 1..g.w.saturating_sub(1) {
        let c0 = g.get(x, 0);
        if c0 != 0 && (0..g.h).all(|y| g.get(x, y) == c0) && x == g.w - x - 1 {
            let mut l = Grid::new(x, g.h);
            let mut r = Grid::new(x, g.h);
            for y in 0..g.h {
                for xx in 0..x {
                    l.set(xx, y, g.get(xx, y));
                    r.set(xx, y, g.get(x + 1 + xx, y));
                }
            }
            return Some((true, l, r));
        }
    }
    for y in 1..g.h.saturating_sub(1) {
        let c0 = g.get(0, y);
        if c0 != 0 && (0..g.w).all(|x| g.get(x, y) == c0) && y == g.h - y - 1 {
            let mut t = Grid::new(g.w, y);
            let mut b = Grid::new(g.w, y);
            for yy in 0..y {
                for x in 0..g.w {
                    t.set(x, yy, g.get(x, yy));
                    b.set(x, yy, g.get(x, y + 1 + yy));
                }
            }
            return Some((false, t, b));
        }
    }
    // 구분선 없는 정확 반분(짝수 변) — 좌|우 우선, 다음 상/하
    if g.w % 2 == 0 && g.w >= 4 {
        let hw = g.w / 2;
        let mut l = Grid::new(hw, g.h);
        let mut r = Grid::new(hw, g.h);
        for y in 0..g.h {
            for x in 0..hw {
                l.set(x, y, g.get(x, y));
                r.set(x, y, g.get(hw + x, y));
            }
        }
        return Some((true, l, r));
    }
    if g.h % 2 == 0 && g.h >= 4 {
        let hh = g.h / 2;
        let mut t = Grid::new(g.w, hh);
        let mut b = Grid::new(g.w, hh);
        for y in 0..hh {
            for x in 0..g.w {
                t.set(x, y, g.get(x, y));
                b.set(x, y, g.get(x, hh + y));
            }
        }
        return Some((false, t, b));
    }
    None
}

pub fn try_panel_combine(train: &[(Grid, Grid)]) -> Option<PanelCombine> {
    let mut table: std::collections::HashMap<(u8, u8), u8> = Default::default();
    let mut vertical = None;
    for (gi, go) in train {
        let (v, a, b) = split_panels(gi)?;
        if a.w != go.w || a.h != go.h {
            return None;
        }
        match vertical {
            None => vertical = Some(v),
            Some(pv) if pv != v => return None,
            _ => {}
        }
        for i in 0..a.cells.len() {
            let key = (a.cells[i], b.cells[i]);
            let out = go.cells[i];
            match table.get(&key) {
                None => {
                    table.insert(key, out);
                }
                Some(&e) if e != out => return None,
                _ => {}
            }
        }
    }
    let pc = PanelCombine { vertical: vertical?, table };
    // 훈련 정확 재현 검증
    for (gi, go) in train {
        if apply_panel_combine(gi, &pc).as_ref() != Some(go) {
            return None;
        }
    }
    Some(pc)
}

pub fn apply_panel_combine(g: &Grid, pc: &PanelCombine) -> Option<Grid> {
    let (v, a, b) = split_panels(g)?;
    if v != pc.vertical {
        return None;
    }
    let mut o = Grid::new(a.w, a.h);
    for i in 0..a.cells.len() {
        let key = (a.cells[i], b.cells[i]);
        o.cells[i] = pc.table.get(&key).copied().unwrap_or(0);
    }
    Some(o)
}

/// 제3계층 — 셀 이벤트 스키마(파편화·패턴형): 셀당 이벤트로 스키마 귀납.
/// 슬롯 = 자기색·4이웃색(경계=10)·좌표 패리티. 수백 이벤트 = induce의 본래
/// 체급(증폭 불요). 훈련 전부 정확 재현일 때만 채택.
pub const CS_SELF: u16 = 20;
pub const CS_N: u16 = 21;
pub const CS_S: u16 = 22;
pub const CS_E: u16 = 23;
pub const CS_W: u16 = 24;
pub const CS_PX: u16 = 25;
pub const CS_PY: u16 = 26;
pub const CS_NE: u16 = 27;
pub const CS_NW: u16 = 28;
pub const CS_SE: u16 = 29;
pub const CS_SW: u16 = 30;
pub const CS_PX3: u16 = 31;
pub const CS_PY3: u16 = 32;
pub const CS_EDGE: u16 = 33;

fn cell_event(g: &Grid, x: usize, y: usize, effect: u32) -> Event {
    let nb = |dx: i32, dy: i32| -> u32 {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if g.in_bounds(nx, ny) {
            g.get(nx as usize, ny as usize) as u32
        } else {
            10
        }
    };
    Event {
        cats: vec![
            (CS_SELF, g.get(x, y) as u32),
            (CS_N, nb(0, -1)),
            (CS_S, nb(0, 1)),
            (CS_E, nb(1, 0)),
            (CS_W, nb(-1, 0)),
            (CS_PX, (x % 2) as u32),
            (CS_PY, (y % 2) as u32),
            (CS_NE, nb(1, -1)),
            (CS_NW, nb(-1, -1)),
            (CS_SE, nb(1, 1)),
            (CS_SW, nb(-1, 1)),
            (CS_PX3, (x % 3) as u32),
            (CS_PY3, (y % 3) as u32),
            (CS_EDGE, (x == 0 || y == 0 || x == g.w - 1 || y == g.h - 1) as u32),
        ],
        nums: vec![],
        effect,
    }
}

pub fn try_cellwise(train: &[(Grid, Grid)]) -> Option<SchemaLib> {
    try_cellwise_ms(train, 6)
}

/// anytime 승급용: 지지 문턱을 낮춘 셀 규칙(정확 재현 게이트는 동일).
pub fn try_cellwise_ms(train: &[(Grid, Grid)], min_support: u32) -> Option<SchemaLib> {
    if !train.iter().all(|(i, o)| i.w == o.w && i.h == o.h) {
        return None;
    }
    let mut ev = Vec::new();
    for (gi, go) in train {
        for y in 0..gi.h {
            for x in 0..gi.w {
                ev.push(cell_event(gi, x, y, go.get(x, y) as u32));
            }
        }
    }
    let lib = induce(&ev, InduceConfig { min_support, ..Default::default() });
    let exact = train.iter().all(|(gi, go)| &apply_cellwise(gi, &lib) == go);
    exact.then_some(lib)
}

pub fn apply_cellwise(g: &Grid, lib: &SchemaLib) -> Grid {
    let mut o = Grid::new(g.w, g.h);
    for y in 0..g.h {
        for x in 0..g.w {
            let ev = cell_event(g, x, y, 0);
            o.set(x, y, lib.predict(&ev).unwrap_or(0).min(9) as u8);
        }
    }
    o
}

/// N패널 결합(3~4): 같은 색 구분선들로 등분된 패널들의 셀별 N-튜플 사상.
pub struct PanelCombineN {
    pub vertical: bool,
    pub n: usize,
    pub table: std::collections::HashMap<Vec<u8>, u8>,
}

/// 2차원 패널 격자: 양방향 구분선으로 나뉜 (행, 열) 패널 배열.
fn split_grid_cells(g: &Grid) -> Option<(usize, usize, Vec<Grid>)> {
    let bounds_of = |n_major: usize, n_minor: usize, at: &dyn Fn(usize, usize) -> u8| {
        let mut divs: Vec<usize> = Vec::new();
        let mut col: Option<u8> = None;
        for i in 0..n_major {
            let c0 = at(i, 0);
            if c0 != 0 && (0..n_minor).all(|j| at(i, j) == c0) {
                if col.map(|d| d == c0).unwrap_or(true) {
                    col = Some(c0);
                    divs.push(i);
                }
            }
        }
        let mut bounds: Vec<(usize, usize)> = Vec::new();
        let mut start = 0usize;
        for &d in &divs {
            if d > start {
                bounds.push((start, d));
            }
            start = d + 1;
        }
        if start < n_major {
            bounds.push((start, n_major));
        }
        bounds
    };
    let cols = bounds_of(g.w, g.h, &|i, j| g.get(i, j));
    let rows = bounds_of(g.h, g.w, &|i, j| g.get(j, i));
    if cols.len() < 2 && rows.len() < 2 {
        return None;
    }
    if cols.is_empty() || rows.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for &(y0, y1) in &rows {
        for &(x0, x1) in &cols {
            let (pw, ph) = (x1 - x0, y1 - y0);
            if pw == 0 || ph == 0 {
                return None;
            }
            out.push(crop(g, x0, y0, pw, ph));
        }
    }
    Some((rows.len(), cols.len(), out))
}

/// 임의 개수의 등분 패널(구분선 1~5개, 세로 우선 → 가로) 또는 반분.
fn split_panels_any(g: &Grid) -> Option<Vec<Grid>> {
    // 세로 구분선
    for dir in 0..2 {
        let (n_major, n_minor) = if dir == 0 { (g.w, g.h) } else { (g.h, g.w) };
        let at = |i: usize, j: usize| -> u8 {
            if dir == 0 {
                g.get(i, j)
            } else {
                g.get(j, i)
            }
        };
        let mut divs: Vec<usize> = Vec::new();
        let mut col: Option<u8> = None;
        for i in 0..n_major {
            let c0 = at(i, 0);
            if c0 != 0 && (0..n_minor).all(|j| at(i, j) == c0) {
                if col.map(|d| d == c0).unwrap_or(true) {
                    col = Some(c0);
                    divs.push(i);
                }
            }
        }
        if divs.is_empty() || divs.len() > 5 {
            continue;
        }
        let mut bounds: Vec<(usize, usize)> = Vec::new();
        let mut start = 0usize;
        for &d in &divs {
            if d > start {
                bounds.push((start, d));
            }
            start = d + 1;
        }
        if start < n_major {
            bounds.push((start, n_major));
        }
        if bounds.len() < 2 {
            continue;
        }
        let w0 = bounds[0].1 - bounds[0].0;
        if w0 == 0 || bounds.iter().any(|b| b.1 - b.0 != w0) {
            continue;
        }
        let mut out = Vec::new();
        for (a, b) in bounds {
            let (pw, ph) = if dir == 0 { (b - a, n_minor) } else { (n_minor, b - a) };
            let mut p = Grid::new(pw, ph);
            for y in 0..ph {
                for x in 0..pw {
                    let v = if dir == 0 { g.get(a + x, y) } else { g.get(x, a + y) };
                    p.set(x, y, v);
                }
            }
            out.push(p);
        }
        return Some(out);
    }
    None
}

fn split_panels_n(g: &Grid) -> Option<(bool, Vec<Grid>)> {
    // 세로: 전열 단색 구분선들 수집(같은 색), 등폭 검사
    let mut divs: Vec<usize> = Vec::new();
    let mut dcol: Option<u8> = None;
    for x in 0..g.w {
        let c0 = g.get(x, 0);
        if c0 != 0 && (0..g.h).all(|y| g.get(x, y) == c0) {
            if dcol.map(|d| d == c0).unwrap_or(true) {
                dcol = Some(c0);
                divs.push(x);
            }
        }
    }
    if divs.len() >= 2 && divs.len() <= 3 {
        let mut bounds = vec![0usize];
        for &d in &divs {
            bounds.push(d);
            bounds.push(d + 1);
        }
        bounds.push(g.w);
        let widths: Vec<usize> =
            bounds.chunks(2).map(|c| c[1].saturating_sub(c[0])).collect();
        if widths.iter().all(|&w| w == widths[0] && w > 0) {
            let mut panels = Vec::new();
            for c in bounds.chunks(2) {
                let mut p = Grid::new(widths[0], g.h);
                for y in 0..g.h {
                    for x in 0..widths[0] {
                        p.set(x, y, g.get(c[0] + x, y));
                    }
                }
                panels.push(p);
            }
            return Some((true, panels));
        }
    }
    // 가로: 전행 단색 구분선들
    let mut rdivs: Vec<usize> = Vec::new();
    let mut rcol: Option<u8> = None;
    for y in 0..g.h {
        let c0 = g.get(0, y);
        if c0 != 0 && (0..g.w).all(|x| g.get(x, y) == c0) {
            if rcol.map(|d| d == c0).unwrap_or(true) {
                rcol = Some(c0);
                rdivs.push(y);
            }
        }
    }
    if rdivs.len() >= 2 && rdivs.len() <= 3 {
        let mut bounds = vec![0usize];
        for &d in &rdivs {
            bounds.push(d);
            bounds.push(d + 1);
        }
        bounds.push(g.h);
        let heights: Vec<usize> =
            bounds.chunks(2).map(|c| c[1].saturating_sub(c[0])).collect();
        if heights.iter().all(|&h| h == heights[0] && h > 0) {
            let mut panels = Vec::new();
            for c in bounds.chunks(2) {
                let mut p = Grid::new(g.w, heights[0]);
                for y in 0..heights[0] {
                    for x in 0..g.w {
                        p.set(x, y, g.get(x, c[0] + y));
                    }
                }
                panels.push(p);
            }
            return Some((false, panels));
        }
    }
    None
}

pub fn try_panel_combine_n(train: &[(Grid, Grid)]) -> Option<PanelCombineN> {
    let mut table: std::collections::HashMap<Vec<u8>, u8> = Default::default();
    let mut meta: Option<(bool, usize)> = None;
    for (gi, go) in train {
        let (v, ps) = split_panels_n(gi)?;
        if ps[0].w != go.w || ps[0].h != go.h {
            return None;
        }
        match meta {
            None => meta = Some((v, ps.len())),
            Some((pv, pn)) if pv != v || pn != ps.len() => return None,
            _ => {}
        }
        for i in 0..go.cells.len() {
            let key: Vec<u8> = ps.iter().map(|p| p.cells[i]).collect();
            match table.get(&key) {
                None => {
                    table.insert(key, go.cells[i]);
                }
                Some(&e) if e != go.cells[i] => return None,
                _ => {}
            }
        }
    }
    let (vertical, n) = meta?;
    let pc = PanelCombineN { vertical, n, table };
    for (gi, go) in train {
        if apply_panel_combine_n(gi, &pc).as_ref() != Some(go) {
            return None;
        }
    }
    Some(pc)
}

pub fn apply_panel_combine_n(g: &Grid, pc: &PanelCombineN) -> Option<Grid> {
    let (v, ps) = split_panels_n(g)?;
    if v != pc.vertical || ps.len() != pc.n {
        return None;
    }
    let mut o = Grid::new(ps[0].w, ps[0].h);
    for i in 0..o.cells.len() {
        let key: Vec<u8> = ps.iter().map(|p| p.cells[i]).collect();
        o.cells[i] = pc.table.get(&key).copied().unwrap_or(0);
    }
    Some(o)
}

/// 역방향 혼합(W2-3): 객체 파이프라인 후 격자 연산 마무리.
/// 객체 단계가 훈련을 못 닫을 때, 잔차를 단일 격자 연산이 정확히 닫으면 채택.
pub fn try_objects_then_grid(train: &[(Grid, Grid)], libs: &Libs) -> Option<GridOp> {
    let mid: Vec<(Grid, Grid)> =
        train.iter().map(|(i, o)| (apply(i, libs), o.clone())).collect();
    if mid.iter().all(|(m, o)| m == o) {
        return None; // 객체 단계 단독으로 이미 정확 — 마무리 불필요
    }
    if mid.iter().zip(train.iter()).all(|((m, _), (i, _))| m == i) {
        return None; // 객체 단계가 항등 — 의미 없음
    }
    try_grid_ops(&mid)
}

/// 혼합 연쇄(W2-3): 정규화(추출류) 후 객체 파이프라인.
/// 정규화된 입력 크기 = 출력 크기이고, 학습된 lib이 훈련 전부를 정확 재현할 때만.
pub fn try_norm_then_objects(train: &[(Grid, Grid)]) -> Option<(GridOp, Libs)> {
    for norm in [
        GridOp::ExtractContent,
        GridOp::ExtractLargest,
        GridOp::ExtractUniqueColor,
        GridOp::ExtractFrameInterior,
    ] {
        let nt: Vec<(Grid, Grid)> = train
            .iter()
            .map(|(i, o)| (apply_grid_op(i, norm), o.clone()))
            .collect();
        if !nt.iter().all(|(ni, o)| ni.w == o.w && ni.h == o.h) {
            continue;
        }
        let libs = learn(&nt);
        if nt.iter().all(|(ni, o)| &apply(ni, &libs) == o) {
            return Some((norm, libs));
        }
    }
    None
}

/// 쌍 간 교차 검증(leave-one-pair-out) — 신규/위험 슬롯의 전제 장치.
/// 훈련쌍 하나를 빼고 배운 규칙이 그 쌍을 정확히 맞히는 비율로 구성을 채점하고,
/// 더 일반화하는 슬롯 집합을 고른다(동점이면 단순한 기본 슬롯).
pub fn loo_score(train: &[(Grid, Grid)], extra: bool) -> usize {
    loo_score_cfg(train, extra, 4)
}

/// 구성별 LOO 점수(확장 슬롯 × 지지 문턱).
/// 분해 방식까지 포함한 LOO 채점.
/// 배경색까지 포함한 LOO 채점.
/// 다색 축까지 포함한 LOO 채점.
pub fn loo_score_full(
    train: &[(Grid, Grid)],
    extra: bool,
    conn8: bool,
    bg: u8,
    multi: bool,
) -> usize {
    if train.len() < 2 {
        return 0;
    }
    let mut hits = 0usize;
    for i in 0..train.len() {
        let rest: Vec<(Grid, Grid)> = train
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| p.clone())
            .collect();
        let libs = learn_full(&rest, extra, 4, None, conn8, bg, multi);
        if apply(&train[i].0, &libs) == train[i].1 {
            hits += 1;
        }
    }
    hits
}

pub fn loo_score_bg(train: &[(Grid, Grid)], extra: bool, conn8: bool, bg: u8) -> usize {
    if train.len() < 2 {
        return 0;
    }
    let mut hits = 0usize;
    for i in 0..train.len() {
        let rest: Vec<(Grid, Grid)> = train
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| p.clone())
            .collect();
        let libs = learn_seg_bg(&rest, extra, 4, None, conn8, bg);
        if apply(&train[i].0, &libs) == train[i].1 {
            hits += 1;
        }
    }
    hits
}

pub fn loo_score_seg(train: &[(Grid, Grid)], extra: bool, conn8: bool) -> usize {
    if train.len() < 2 {
        return 0;
    }
    let mut hits = 0usize;
    for i in 0..train.len() {
        let rest: Vec<(Grid, Grid)> = train
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| p.clone())
            .collect();
        let libs = learn_seg(&rest, extra, 4, None, conn8);
        if apply(&train[i].0, &libs) == train[i].1 {
            hits += 1;
        }
    }
    hits
}

pub fn loo_score_cfg(train: &[(Grid, Grid)], extra: bool, min_support: u32) -> usize {
    if train.len() < 2 {
        return 0;
    }
    let mut hits = 0usize;
    for i in 0..train.len() {
        let rest: Vec<(Grid, Grid)> = train
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| p.clone())
            .collect();
        let libs = learn_cfg(&rest, extra, min_support);
        if apply(&train[i].0, &libs) == train[i].1 {
            hits += 1;
        }
    }
    hits
}

/// LOO로 슬롯 구성을 선택해 학습한다(W2-2 규칙 교차 검증 항목).
/// 구조적 기계: 기본 해석이 훈련을 정확히 재현하지 못하면 클래스 해석을
/// 바꿔가며(모호성 탐색) 훈련을 닫는 해석을 찾는다. anytime 예산의 학습측 투입.
pub fn learn_best(train: &[(Grid, Grid)]) -> Libs {
    let base = learn_validated(train);
    if train.iter().all(|(i, o)| apply(i, &base) == *o) {
        return base;
    }
    let classes = [
        C_TRANS, C_MIR_H, C_MIR_V, C_GRAV, C_AT_MARKER, C_AT_MARKER_AREA,
        C_ROT180_OBJ, C_DILATE, C_ERODE, C_RAY, C_RAY_BAND, C_MARK_FLOOR,
        C_MARK_REL, C_FILL, C_OUTLINE,
    ];
    for &fc in classes.iter() {
        for &ex in [false, true].iter() {
            let cand = learn_forced(train, ex, 4, Some(fc));
            if train.iter().all(|(i, o)| apply(i, &cand) == *o) {
                return cand;
            }
        }
    }
    base
}

pub fn learn_validated(train: &[(Grid, Grid)]) -> Libs {
    // anytime 구성 탐색: (확장 슬롯, 지지 문턱) 4조합을 LOO로 채점.
    // 동점이면 단순한 구성(기본 슬롯·높은 문턱)을 택한다 — 오컴의 면도날.
    // 배경 후보: 훈련 입력의 최빈색이 0이 아니면 그 색도 배경 가설로 시험
    let dom = {
        let mut cnt = [0usize; 10];
        for (gi, _) in train {
            for &c in gi.cells.iter() {
                cnt[c as usize] += 1;
            }
        }
        (0..10).max_by_key(|&i| cnt[i]).unwrap_or(0) as u8
    };
    let bgs: Vec<u8> = if dom != 0 { vec![0, dom] } else { vec![0] };
    // 다색 분해가 단색 분해와 다른 객체 수를 낼 때만 후보에 추가(비용 절약)
    let multi_useful = train.iter().any(|(gi, _)| {
        crate::grid::components_multi(gi, false, 0).len()
            != crate::grid::components_bg(gi, false, 0).len()
    });
    let multis: Vec<bool> = if multi_useful { vec![false, true] } else { vec![false] };
    let cands = [(false, false), (true, false), (false, true), (true, true)];
    let mut best = (0usize, false, false, 0u8, false);
    let mut first = true;
    for &mu in multis.iter() {
        for &bg in bgs.iter() {
            for &(ex, c8) in cands.iter() {
                let sc = loo_score_full(train, ex, c8, bg, mu);
                if first || sc > best.0 {
                    best = (sc, ex, c8, bg, mu);
                    first = false;
                }
            }
        }
    }
    learn_full(train, best.1, 4, None, best.2, best.3, best.4)
}

pub fn learn(train: &[(Grid, Grid)]) -> Libs {
    learn_with(train, false)
}

/// 교차 검증(LOO) 선택용: 확장 슬롯 on/off를 지정해 학습한다.
pub fn learn_with(train: &[(Grid, Grid)], extra: bool) -> Libs {
    learn_cfg(train, extra, 4)
}

/// LOO 구성 탐색용: 확장 슬롯·지지 문턱을 함께 지정한다.
pub fn learn_cfg(train: &[(Grid, Grid)], extra: bool, min_support: u32) -> Libs {
    learn_forced(train, extra, min_support, None)
}

/// 모호성 탐색용: 전역 일관성 투표의 결과를 지정 클래스로 강제한다.
pub fn learn_forced(
    train: &[(Grid, Grid)],
    extra: bool,
    min_support: u32,
    forced: Option<u32>,
) -> Libs {
    learn_seg(train, extra, min_support, forced, false)
}

/// 분해 방식(4/8-연결)까지 지정하는 최종 학습 진입점.
pub fn learn_seg(
    train: &[(Grid, Grid)],
    extra: bool,
    min_support: u32,
    forced: Option<u32>,
    conn8: bool,
) -> Libs {
    learn_seg_bg(train, extra, min_support, forced, conn8, 0)
}

/// 배경색까지 지정하는 학습(표현 공백 2호).
pub fn learn_seg_bg(
    train: &[(Grid, Grid)],
    extra: bool,
    min_support: u32,
    forced: Option<u32>,
    conn8: bool,
    bg: u8,
) -> Libs {
    learn_full(train, extra, min_support, forced, conn8, bg, false)
}

/// 표현 공백 3호: 다색 객체 분해까지 지정하는 최종 학습.
#[allow(clippy::too_many_arguments)]
pub fn learn_full(
    train: &[(Grid, Grid)],
    extra: bool,
    min_support: u32,
    forced: Option<u32>,
    conn8: bool,
    bg: u8,
    multi: bool,
) -> Libs {
    struct Row {
        pair: usize,
        ii: usize,
        k: u32,
        cands: Vec<(u32, i32, i32)>,
        out_color: u8,
    }
    let mut rows: Vec<Row> = Vec::new();
    let mut per_pair: Vec<(Vec<Obj>, usize)> = Vec::new();
    let mut deletes: Vec<(usize, usize)> = Vec::new();
    let mut copies_ev: Vec<(usize, usize, u32)> = Vec::new();
    for (pi, (gi, go)) in train.iter().enumerate() {
        let seg = |g: &Grid| {
            if multi {
                crate::grid::components_multi(g, conn8, bg)
            } else {
                crate::grid::components_bg(g, conn8, bg)
            }
        };
        let ins = seg(gi);
        let outs = seg(go);
        for (ii, ois) in align(&ins, &outs) {
            copies_ev.push((pi, ii, ois.len() as u32));
            if ois.is_empty() {
                deletes.push((pi, ii));
                continue;
            }
            let mut ord = ois.clone();
            ord.sort_by_key(|&oi| (outs[oi].x0, outs[oi].y0));
            for (k, &oi) in ord.iter().enumerate() {
                rows.push(Row {
                    pair: pi,
                    ii,
                    k: k as u32,
                    cands: candidates(gi, &ins, ii, &outs[oi]),
                    out_color: outs[oi].color,
                });
            }
        }
        per_pair.push((ins, gi.h));
    }

    // 전역 일관성 투표: 상수 translate > mirror > gravity.
    let moving: Vec<&Row> = rows
        .iter()
        .filter(|r| !r.cands.iter().any(|c| c.0 == C_STAY || c.0 == C_OUTLINE))
        .collect();
    let uniform = |class: u32| moving.iter().all(|r| r.cands.iter().any(|c| c.0 == class));
    // 표식 변종 우열: 색 기반이 전 사례를 설명하면 색, 아니면 면적 기반
    let uniform_area = uniform(C_AT_MARKER_AREA) && !uniform(C_AT_MARKER);
    let const_trans = {
        let ds: Vec<(i32, i32)> = moving
            .iter()
            .filter_map(|r| r.cands.iter().find(|c| c.0 == C_TRANS).map(|c| (c.1, c.2)))
            .collect();
        !moving.is_empty() && ds.len() == moving.len() && ds.windows(2).all(|w| w[0] == w[1])
    };
    let chosen_uniform: Option<u32> = if let Some(fc) = forced {
        if moving.iter().any(|r| r.cands.iter().any(|c| c.0 == fc)) {
            Some(fc)
        } else {
            None
        }
    } else if moving.is_empty() {
        None
    } else if const_trans {
        Some(C_TRANS)
    } else if uniform(C_AT_MARKER) {
        Some(C_AT_MARKER)
    } else if uniform_area {
        Some(C_AT_MARKER_AREA)
    } else if uniform(C_MIR_H) {
        Some(C_MIR_H)
    } else if uniform(C_MIR_V) {
        Some(C_MIR_V)
    } else if uniform(C_GRAV) {
        Some(C_GRAV)
    } else {
        None
    };
    let resolve = |r: &Row| -> (u32, i32, i32) {
        if let Some(u) = chosen_uniform {
            if let Some(&c) = r.cands.iter().find(|c| c.0 == u) {
                return c;
            }
        }
        r.cands[0]
    };

    let mut ev_class = Vec::new();
    let mut ev_dx = Vec::new();
    let mut ev_dy = Vec::new();
    let mut ev_color = Vec::new();
    let mut ev_copies = Vec::new();
    for r in &rows {
        let (ins, gh) = &per_pair[r.pair];
        let (class, p1, p2) = resolve(r);
        ev_class.push(obj_event(ins, r.ii, r.k, *gh, class, extra));
        // 파라미터 lib은 클래스 조건 슬롯을 갖는다(클래스마다 파라미터 의미가 다름:
        // translate=dx·dy, at_marker=표식 색, ray=방향).
        if class == C_TRANS
            || class == C_AT_MARKER
            || class == C_AT_MARKER_AREA
            || class == C_RAY
            || class == C_MARK_REL
            || class == C_RAY_BAND
        {
            let mut e1 = obj_event(ins, r.ii, r.k, *gh, (p1 + 16).max(0) as u32, extra);
            e1.cats.push((S_CLASS, class));
            ev_dx.push(e1);
        }
        if class == C_TRANS || class == C_MARK_REL {
            let mut e2 = obj_event(ins, r.ii, r.k, *gh, (p2 + 16).max(0) as u32, extra);
            e2.cats.push((S_CLASS, class));
            ev_dy.push(e2);
        }
        let ce = if r.out_color == ins[r.ii].color { 0 } else { 100 + r.out_color as u32 };
        let mut ec = obj_event(ins, r.ii, r.k, *gh, ce, extra);
        ec.cats.push((S_CLASS, class));
        ev_color.push(ec);
    }
    for &(pi, ii) in &deletes {
        let (ins, gh) = &per_pair[pi];
        ev_class.push(obj_event(ins, ii, 0, *gh, C_DEL, extra));
    }
    for &(pi, ii, n) in &copies_ev {
        let (ins, gh) = &per_pair[pi];
        ev_copies.push(obj_event(ins, ii, 0, *gh, n, extra));
    }
    // 소표본 증폭(4×) + 확신 필터 — 시도 62 참조.
    let amp = |ev: &[Event]| -> Vec<Event> {
        let mut v = Vec::with_capacity(ev.len() * 4);
        for _ in 0..4 {
            v.extend_from_slice(ev);
        }
        v
    };
    let cfg = InduceConfig { min_support, ..Default::default() };
    let trim = |mut l: SchemaLib| -> SchemaLib {
        l.schemas.retain(|s| s.confidence() >= 0.5);
        l
    };
    // 균일 목표 가드: 전 이벤트의 효과가 같으면 압축할 엔트로피가 0 — 규칙은
    // 전부 우연 슬롯 과적합이다(예: 훈련 객체 면적이 우연히 균일 → "면적=3이면 2"가
    // 미관측 면적을 기본값으로 떨어뜨림). 순수 기본값 lib으로 대체.
    let mk = |ev: &[Event]| -> SchemaLib {
        if let Some(first) = ev.first().map(|e| e.effect) {
            if ev.iter().all(|e| e.effect == first) {
                return SchemaLib { schemas: Vec::new(), default_effect: Some(first) };
            }
        }
        trim(induce(&amp(ev), cfg))
    };
    Libs {
        class: mk(&ev_class),
        dx: mk(&ev_dx),
        dy: mk(&ev_dy),
        color: mk(&ev_color),
        copies: mk(&ev_copies),
        grid_op: try_grid_chain(train),
        extra,
        conn8,
        bg,
        multi,
    }
}

pub fn apply(gi: &Grid, libs: &Libs) -> Grid {
    // 격자 수준 가설이 훈련 전부를 설명했다면 그것이 답이다
    if let Some(chain) = libs.grid_op {
        return apply_grid_chain(gi, chain);
    }
    let mut out = Grid::new(gi.w, gi.h);
    // 비-0 배경이면 캔버스를 그 색으로 채운다
    if libs.bg != 0 {
        for c in out.cells.iter_mut() {
            *c = libs.bg;
        }
    }
    let ins = if libs.multi {
        crate::grid::components_multi(gi, libs.conn8, libs.bg)
    } else {
        crate::grid::components_bg(gi, libs.conn8, libs.bg)
    };
    for (ii, io) in ins.iter().enumerate() {
        let n = libs
            .copies
            .predict(&obj_event(&ins, ii, 0, gi.h, 0, libs.extra))
            .unwrap_or(1) as usize;
        for k in 0..n {
            let ev = obj_event(&ins, ii, k as u32, gi.h, 0, libs.extra);
            let class = libs.class.predict(&ev).unwrap_or(C_STAY);
            // 파라미터·색 조회는 클래스 조건을 실어 보낸다
            let mut evp = ev.clone();
            evp.cats.push((S_CLASS, class));
            let color = match libs.color.predict(&evp) {
                Some(c) if c >= 100 => (c - 100) as u8,
                _ => io.color,
            };
            match class {
                C_DEL => {}
                C_STAY => {
                    if libs.multi {
                        crate::grid::stamp_colors(&mut out, io, io.x0, io.y0);
                    } else {
                        stamp(&mut out, io, io.x0, io.y0, color);
                    }
                }
                C_TRANS => {
                    let dx = libs.dx.predict(&evp).map(|v| v as i32 - 16).unwrap_or(0);
                    let dy = libs.dy.predict(&evp).map(|v| v as i32 - 16).unwrap_or(0);
                    let nx = (io.x0 as i32 + dx).max(0) as usize;
                    let ny = (io.y0 as i32 + dy).max(0) as usize;
                    if libs.multi {
                        crate::grid::stamp_colors(&mut out, io, nx, ny);
                    } else {
                        stamp(&mut out, io, nx, ny, color);
                    }
                }
                C_AT_MARKER => {
                    // 표식 색의 모든 객체 위치에 사본 — 관계 앵커의 적용
                    let mc = libs.dx.predict(&evp).map(|v| v as i32 - 16).unwrap_or(-1);
                    if mc >= 0 {
                        for m in ins.iter() {
                            if m.color as i32 == mc && m.mask != io.mask {
                                stamp(&mut out, io, m.x0, m.y0, color);
                            }
                        }
                    }
                    // 사본 열거는 표식 순회가 대신한다 — k 루프 종료
                    break;
                }
                C_FILL => {
                    stamp(&mut out, io, io.x0, io.y0, io.color);
                    for (x, y) in crate::grid::holes(gi, io) {
                        out.set(x, y, color);
                    }
                }
                C_ROT180_OBJ => {
                    let m = Obj { mask: rot180_mask(io), ..io.clone() };
                    stamp(&mut out, &m, io.x0, io.y0, color);
                }
                C_AT_MARKER_AREA => {
                    let ma = libs.dx.predict(&evp).map(|v| v as i32 - 16).unwrap_or(-1);
                    if ma >= 0 {
                        for m in ins.iter() {
                            if m.area.min(15) as i32 == ma && m.mask != io.mask {
                                stamp(&mut out, io, m.x0, m.y0, color);
                            }
                        }
                    }
                    break;
                }
                C_RAY_BAND => {
                    stamp(&mut out, io, io.x0, io.y0, io.color);
                    let dir = libs.dx.predict(&evp).map(|v| v as i32 - 16).unwrap_or(0);
                    match dir {
                        0 => {
                            for y in io.y0..io.y0 + io.h {
                                for x in (io.x0 + io.w)..gi.w {
                                    if out.get(x, y) == 0 {
                                        out.set(x, y, color);
                                    }
                                }
                            }
                        }
                        1 => {
                            for y in io.y0..io.y0 + io.h {
                                for x in 0..io.x0 {
                                    if out.get(x, y) == 0 {
                                        out.set(x, y, color);
                                    }
                                }
                            }
                        }
                        2 => {
                            for y in (io.y0 + io.h)..gi.h {
                                for x in io.x0..io.x0 + io.w {
                                    if out.get(x, y) == 0 {
                                        out.set(x, y, color);
                                    }
                                }
                            }
                        }
                        _ => {
                            for y in 0..io.y0 {
                                for x in io.x0..io.x0 + io.w {
                                    if out.get(x, y) == 0 {
                                        out.set(x, y, color);
                                    }
                                }
                            }
                        }
                    }
                }
                C_DILATE => {
                    let (dw, dh, dm) = dilate_mask(io);
                    let m = Obj { w: dw, h: dh, mask: dm, ..io.clone() };
                    let nx = io.x0.saturating_sub(1);
                    let ny = io.y0.saturating_sub(1);
                    stamp(&mut out, &m, nx, ny, color);
                }
                C_ERODE => {
                    let m = Obj { mask: erode_mask(io), ..io.clone() };
                    stamp(&mut out, &m, io.x0, io.y0, color);
                }
                C_SOLIDIFY => {
                    for y in io.y0..(io.y0 + io.h).min(gi.h) {
                        for x in io.x0..(io.x0 + io.w).min(gi.w) {
                            out.set(x, y, color);
                        }
                    }
                }
                C_COLORSWAP => {
                    // 객체가 가진 두 색을 맞바꿔 찍는다
                    let mut present: Vec<u8> = io
                        .colors
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| io.mask[*i])
                        .map(|(_, &c)| c)
                        .collect();
                    present.sort_unstable();
                    present.dedup();
                    if present.len() == 2 {
                        let (a, b) = (present[0], present[1]);
                        let swapped: Vec<u8> = io
                            .colors
                            .iter()
                            .map(|&c| if c == a { b } else if c == b { a } else { c })
                            .collect();
                        let m = Obj { colors: swapped, ..io.clone() };
                        crate::grid::stamp_colors(&mut out, &m, io.x0, io.y0);
                    } else {
                        crate::grid::stamp_colors(&mut out, io, io.x0, io.y0);
                    }
                }
                C_MARK_FLOOR => {
                    let cx = io.x0 + io.w / 2;
                    if cx < gi.w {
                        out.set(cx, gi.h - 1, color);
                    }
                }
                C_MARK_REL => {
                    let dx = libs.dx.predict(&evp).map(|v| v as i32 - 16).unwrap_or(0);
                    let dy = libs.dy.predict(&evp).map(|v| v as i32 - 16).unwrap_or(0);
                    let x = io.x0 as i32 + (io.w / 2) as i32 + dx;
                    let y = (io.y0 + io.h) as i32 + dy;
                    if x >= 0 && y >= 0 && (x as usize) < gi.w && (y as usize) < gi.h {
                        out.set(x as usize, y as usize, color);
                    }
                }
                C_RAY => {
                    stamp(&mut out, io, io.x0, io.y0, io.color);
                    let dir = libs.dx.predict(&evp).map(|v| v as i32 - 16).unwrap_or(0);
                    let y = io.y0;
                    let x = io.x0;
                    match dir {
                        0 => {
                            for xx in (io.x0 + io.w)..gi.w {
                                if out.get(xx, y) == 0 {
                                    out.set(xx, y, color);
                                }
                            }
                        }
                        1 => {
                            for xx in 0..io.x0 {
                                if out.get(xx, y) == 0 {
                                    out.set(xx, y, color);
                                }
                            }
                        }
                        2 => {
                            for yy in (io.y0 + io.h)..gi.h {
                                if out.get(x, yy) == 0 {
                                    out.set(x, yy, color);
                                }
                            }
                        }
                        _ => {
                            for yy in 0..io.y0 {
                                if out.get(x, yy) == 0 {
                                    out.set(x, yy, color);
                                }
                            }
                        }
                    }
                }
                C_MIR_H => {
                    let m = Obj { mask: hflip(io), ..io.clone() };
                    let nx = ((gi.w - io.w) as i32 - io.x0 as i32).max(0) as usize;
                    stamp(&mut out, &m, nx, io.y0, color);
                }
                C_MIR_V => {
                    let m = Obj { mask: vflip(io), ..io.clone() };
                    let ny = ((gi.h - io.h) as i32 - io.y0 as i32).max(0) as usize;
                    stamp(&mut out, &m, io.x0, ny, color);
                }
                C_GRAV => {
                    let ny = gi.h - io.h;
                    stamp(&mut out, io, io.x0, ny, color);
                }
                C_OUTLINE => {
                    stamp(&mut out, io, io.x0, io.y0, io.color);
                    let oc = match libs.color.predict(&evp) {
                        Some(c) if c >= 100 => (c - 100) as u8,
                        _ => 7,
                    };
                    let (x0, y0) = (io.x0 as i32 - 1, io.y0 as i32 - 1);
                    let (x1, y1) = ((io.x0 + io.w) as i32, (io.y0 + io.h) as i32);
                    for x in x0..=x1 {
                        for &y in &[y0, y1] {
                            if out.in_bounds(x, y) && out.get(x as usize, y as usize) == 0 {
                                out.set(x as usize, y as usize, oc);
                            }
                        }
                    }
                    for y in y0..=y1 {
                        for &x in &[x0, x1] {
                            if out.in_bounds(x, y) && out.get(x as usize, y as usize) == 0 {
                                out.set(x as usize, y as usize, oc);
                            }
                        }
                    }
                }
                _ => stamp(&mut out, io, io.x0, io.y0, color),
            }
        }
    }
    out
}
