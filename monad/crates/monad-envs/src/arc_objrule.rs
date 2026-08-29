//! M2-R — **객체 델타 규칙**: 표현 경쟁의 승자 위에 세운 두 번째 규칙 계층.
//!
//! # 왜 이 표현인가 (시도 165의 계량이 정했다 — 사람이 고르지 않았다)
//!
//! 홀드아웃 바뀐 셀의 기술 가능성: 셀 국소 재작성 **17.4%** vs 객체 델타
//! (단색 4-연결) **72.9%**, 전량 기술 가능 과제 64/100. ARC 과제 간에 공유되는
//! 것은 리터럴 국소 패턴이 아니라 "가장 작은 것을 지워라" 같은 **객체 수준의
//! 추상 관계**다 — 그것만이 팔레트·배치가 다른 과제로 전이될 수 있다.
//!
//! # 규칙의 문법 (내용은 전부 경험이 채운다)
//!
//! `ORULE(OPROPS(p0..p11), OACT(kind, param))` — 조건은 객체의 성질 벡터,
//! 결과는 행동(재색/삭제). 무엇이 변수인지는 LGG가, 채택은 MDL이 정한다.
//! `param`이 성질 자리와 **변수를 공유**하면 "다수색이 된다" 같은 팔레트 독립
//! 규칙이 된다(시도 164에서 세운 바인딩 일관 의미론을 그대로 쓴다).
//!
//! 성질 12종은 전부 동결 기질의 객체 분해에서 기계적으로 나온다(해석 없음):
//! own color · size rank · unique-max/min · border touch · area log2 ·
//! object count · majority/rarest color · largest/smallest obj color ·
//! shape twin 유무 · color frequency rank.
//!
//! # v1의 정직한 범위
//!
//! 재색·삭제만 다룬다. 이동(param=(dx,dy))·출현(원본 객체 없음)은 다음 단계 —
//! 기술률 72.9% 중 재색·삭제가 차지하는 몫만 노린다.

use crate::grid::{components_bg, Grid, Obj};
use monad_core::abstraction::{generalize, Library, Provenance, Term};
use std::collections::HashMap;

const F_ORULE: u32 = 910;
const F_OPROPS: u32 = 911;
const F_OACT: u32 = 912;
/// 행동 종류.
const ACT_RECOLOR: u64 = 1;
const ACT_DELETE: u64 = 2;
/// v2(시도 169): 이동을 1급 델타로. param = 인코딩된 (dx,dy).
const ACT_MOVE: u64 = 3;
/// v3(시도 180): **복제** — 원본은 남고 사본이 생긴다. 다른 행동과 달리
/// **가산적**이다(한 객체가 여러 사본을 낳을 수 있다). param = 인코딩된 (dx,dy).
const ACT_COPY: u64 = 4;
/// 시도 170~171의 계량 판정: 18종 확장(x/y 순위·모양 클래스·구멍·색 유일·비율)은
/// 모호쌍을 428→164로 줄였지만 **홀드아웃 전이를 2→0으로 죽였다** — 성질 수에
/// 비례해 쌍 LGG에서 우연히 상수로 굳는 슬롯이 늘기 때문(과잉 구체화). 스키마
/// 정련 1라운드로도 회복 불가. **④를 실증한 12종을 유지**하고, 확장은 일반화
/// 사다리가 여러 라운드로 강해진 뒤 재시도한다(측정으로 기각, 추측 아님).
pub const NPROPS: usize = 12;

/// 이동 벡터 인코딩(격자 ≤30이므로 ±30이면 충분). 델타 표기와 규칙 param 공용.
const MOVE_BASE: u64 = 1000;
fn encode_move(dx: i64, dy: i64) -> u64 {
    MOVE_BASE + ((dx + 30) as u64) * 61 + ((dy + 30) as u64)
}
fn decode_move(v: u64) -> Option<(i64, i64)> {
    if v < MOVE_BASE {
        return None;
    }
    let r = v - MOVE_BASE;
    let dx = (r / 61) as i64 - 30;
    let dy = (r % 61) as i64 - 30;
    (dx.abs() <= 30 && dy.abs() <= 30).then_some((dx, dy))
}

/// 승자 표현(시도 165): 단색 4-연결, 배경 0.
fn decompose(g: &Grid) -> Vec<Obj> {
    components_bg(g, false, 0)
}

fn obj_color(o: &Obj) -> u8 {
    o.mask
        .iter()
        .zip(o.colors.iter())
        .find(|(m, _)| **m)
        .map(|(_, &c)| c)
        .unwrap_or(0)
}

fn shape_key(o: &Obj) -> (usize, usize, &Vec<bool>) {
    (o.w, o.h, &o.mask)
}

fn log2_bucket(v: usize) -> u64 {
    (usize::BITS - v.max(1).leading_zeros() - 1) as u64
}

/// 한 격자의 객체 성질 벡터들(객체별 12칸). 전부 분해에서 기계적으로 나온다.
pub fn object_props(g: &Grid, objs: &[Obj]) -> Vec<[u64; NPROPS]> {
    let n = objs.len();
    // 크기 순위
    let mut areas: Vec<usize> = objs.iter().map(|o| o.area).collect();
    areas.sort_unstable_by(|a, b| b.cmp(a));
    let max_a = areas.first().copied().unwrap_or(0);
    let min_a = areas.last().copied().unwrap_or(0);
    let max_unique = areas.iter().filter(|&&a| a == max_a).count() == 1;
    let min_unique = areas.iter().filter(|&&a| a == min_a).count() == 1;
    // 격자 다수색·희소색(배경 제외)
    let mut freq = [0usize; 10];
    for &c in &g.cells {
        if c != 0 && c <= 9 {
            freq[c as usize] += 1;
        }
    }
    let majority = (1..10).filter(|&c| freq[c] > 0).max_by_key(|&c| freq[c]).unwrap_or(0) as u64;
    let rarest = (1..10).filter(|&c| freq[c] > 0).min_by_key(|&c| freq[c]).unwrap_or(0) as u64;
    // 최대·최소 객체의 색
    let largest_c = objs.iter().max_by_key(|o| o.area).map(obj_color).unwrap_or(0) as u64;
    let smallest_c = objs.iter().min_by_key(|o| o.area).map(obj_color).unwrap_or(0) as u64;
    // 객체 색 빈도(객체 수 기준)
    let mut cfreq = [0usize; 10];
    for o in objs {
        cfreq[obj_color(o) as usize] += 1;
    }
    let cmax = cfreq.iter().copied().max().unwrap_or(0);
    let cmin = cfreq.iter().copied().filter(|&v| v > 0).min().unwrap_or(0);

    objs.iter()
        .map(|o| {
            let c = obj_color(o);
            let rank = if o.area == max_a {
                0
            } else if o.area == min_a {
                2
            } else {
                1
            };
            let twin = objs
                .iter()
                .filter(|p| !std::ptr::eq(*p, o))
                .any(|p| shape_key(p) == shape_key(o));
            let cf = cfreq[c as usize];
            let cfrank = if cf == cmax {
                0
            } else if cf == cmin {
                2
            } else {
                1
            };
            [
                c as u64,
                rank,
                (max_unique && o.area == max_a) as u64,
                (min_unique && o.area == min_a) as u64,
                (o.x0 == 0 || o.y0 == 0 || o.x0 + o.w == g.w || o.y0 + o.h == g.h) as u64,
                log2_bucket(o.area),
                (n.min(9)) as u64,
                majority,
                rarest,
                largest_c,
                smallest_c,
                (twin as u64) * 3 + cfrank, // twin(0/1)×3 + 색빈도순위(0..2) — 한 칸 절약
            ]
        })
        .collect()
}

fn rule_term(props: &[u64; NPROPS], kind: u64, param: u64) -> Term {
    Term::App(
        F_ORULE,
        vec![
            Term::App(F_OPROPS, props.iter().map(|&v| Term::Const(v)).collect()),
            Term::App(F_OACT, vec![Term::Const(kind), Term::Const(param)]),
        ],
    )
}

fn split_orule(t: &Term) -> Option<(&Vec<Term>, &Term, &Term)> {
    match t {
        Term::App(f, args) if *f == F_ORULE && args.len() == 2 => {
            match (&args[0], &args[1]) {
                (Term::App(pf, props), Term::App(af, act))
                    if *pf == F_OPROPS && props.len() == NPROPS && *af == F_OACT
                        && act.len() == 2 =>
                {
                    Some((props, &act[0], &act[1]))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// 입력↔출력 객체를 짝짓는 공통 코어. 반환: (객체별 델타, 완전 기술 여부).
///
/// 델타: stay=None | recolor(newc)=Some(c) | delete=Some(10). `complete`가 거짓이면
/// 짝 없는 객체(이동·출현·부분 변형)가 남아 있다 — **확정된 재색·삭제 델타는
/// 그래도 유효하다**(부분 추출의 근거).
fn match_deltas(i: &Grid, o: &Grid) -> (Vec<Option<u64>>, Vec<bool>, Vec<Vec<u64>>, bool) {
    let oi = decompose(i);
    let oo = decompose(o);
    let mut used_o = vec![false; oo.len()];
    let mut deltas: Vec<Option<u64>> = vec![None; oi.len()];
    // 유지(완전 동일)
    for (a, ia) in oi.iter().enumerate() {
        for (b, ob) in oo.iter().enumerate() {
            if !used_o[b]
                && ia.x0 == ob.x0
                && ia.y0 == ob.y0
                && shape_key(ia) == shape_key(ob)
                && obj_color(ia) == obj_color(ob)
            {
                used_o[b] = true;
                deltas[a] = None;
                break;
            }
        }
    }
    let mut matched = vec![false; oi.len()];
    for (a, ia) in oi.iter().enumerate() {
        if deltas[a].is_none() {
            // 유지로 이미 짝지어졌는지 구별: 다시 검사
            let stayed = oo.iter().enumerate().any(|(b, ob)| {
                used_o[b]
                    && ia.x0 == ob.x0
                    && ia.y0 == ob.y0
                    && shape_key(ia) == shape_key(ob)
                    && obj_color(ia) == obj_color(ob)
            });
            if stayed {
                matched[a] = true;
            }
        }
    }
    // 재색(위치·모양 동일, 색만 다름)
    for (a, ia) in oi.iter().enumerate() {
        if matched[a] {
            continue;
        }
        for (b, ob) in oo.iter().enumerate() {
            if !used_o[b]
                && ia.x0 == ob.x0
                && ia.y0 == ob.y0
                && shape_key(ia) == shape_key(ob)
            {
                used_o[b] = true;
                matched[a] = true;
                deltas[a] = Some(obj_color(ob) as u64);
                break;
            }
        }
    }
    // 삭제(출력에서 그 자리가 전부 배경)
    for (a, ia) in oi.iter().enumerate() {
        if matched[a] {
            continue;
        }
        // **이동 매칭**(v2, 시도 169): 같은 모양·색의 짝 없는 출력 객체가 다른
        // 자리에 있으면 이동이다 — v1에서는 삭제 오분류만 막았지만(가드), 이제
        // 1급 델타로 승격한다. 델타 = 인코딩된 (dx,dy). 후보가 여럿이면 가장
        // 가까운 것(맨해튼 거리)을 짝으로 — 결정론적.
        let mv = oo
            .iter()
            .enumerate()
            .filter(|(b, ob)| {
                !used_o[*b] && shape_key(ia) == shape_key(ob) && obj_color(ia) == obj_color(ob)
            })
            .min_by_key(|(_, ob)| {
                (ob.x0 as i64 - ia.x0 as i64).abs() + (ob.y0 as i64 - ia.y0 as i64).abs()
            })
            .map(|(b, ob)| (b, ob.x0 as i64 - ia.x0 as i64, ob.y0 as i64 - ia.y0 as i64));
        if let Some((b, dx, dy)) = mv {
            used_o[b] = true;
            matched[a] = true;
            deltas[a] = Some(encode_move(dx, dy));
            continue;
        }
        let all_bg = (0..ia.h)
            .flat_map(|dy| (0..ia.w).map(move |dx| (dx, dy)))
            .filter(|&(dx, dy)| ia.mask[dy * ia.w + dx])
            .all(|(dx, dy)| o.get(ia.x0 + dx, ia.y0 + dy) == 0);
        if all_bg {
            matched[a] = true;
            deltas[a] = Some(10); // 10 = 삭제 표지(색 0..9와 구별)
        }
    }
    // **복제 매칭**(v3): 남은 출력 객체 중 입력에 같은 모양·색 원본이 있는 것은
    // 사본이다. 원본은 이미 유지/재색으로 짝지어졌을 수 있다 — 복제는 배타적
    // 행동이 아니라 **덧붙는** 행동이기 때문이다. 가장 가까운 원본에 귀속한다.
    let mut copies: Vec<Vec<u64>> = vec![Vec::new(); oi.len()];
    for (b, ob) in oo.iter().enumerate() {
        if used_o[b] {
            continue;
        }
        let src = oi
            .iter()
            .enumerate()
            .filter(|(_, ia)| shape_key(ia) == shape_key(ob) && obj_color(ia) == obj_color(ob))
            .min_by_key(|(_, ia)| {
                (ob.x0 as i64 - ia.x0 as i64).abs() + (ob.y0 as i64 - ia.y0 as i64).abs()
            })
            .map(|(a, ia)| (a, ob.x0 as i64 - ia.x0 as i64, ob.y0 as i64 - ia.y0 as i64));
        if let Some((a, dx, dy)) = src {
            used_o[b] = true;
            copies[a].push(encode_move(dx, dy));
        }
    }
    for c in copies.iter_mut() {
        c.sort_unstable();
        c.dedup();
    }
    let complete = matched.iter().all(|m| *m) && used_o.iter().all(|u| *u);
    (deltas, matched, copies, complete)
}

/// 완전 기술이 **왜** 실패했는지(진단용 — 시도 178).
///
/// ①(시도 가능)이 홀드아웃 254건 중 17건뿐이라는 것이 전체 상한이다. 선택을
/// 완벽히 해도 17을 넘을 수 없으므로, 무엇이 237건을 탈락시키는지 재야 한다.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DescribeFail {
    /// 격자 크기가 다르다(이 표현의 범위 밖).
    SizeMismatch,
    /// 짝 없는 **입력** 객체 — 부분 변형·분해 등.
    UnmatchedInput,
    /// 짝 없는 **출력** 객체 — 출현·복제.
    UnmatchedOutput,
    /// 양쪽 모두.
    Both,
}

/// 한 훈련쌍이 현재 델타 어휘로 완전 기술되는가, 아니면 왜 안 되는가.
pub fn describe_failure(i: &Grid, o: &Grid) -> Option<DescribeFail> {
    if i.w != o.w || i.h != o.h {
        return Some(DescribeFail::SizeMismatch);
    }
    let (_, matched, _, complete) = match_deltas(i, o);
    if complete {
        return None;
    }
    let left_in = matched.iter().any(|m| !m);
    // 짝 없는 출력 객체 존재 여부는 match_deltas가 소비 표시로 남긴다 —
    // 여기서는 다시 계산한다(진단이므로 비용보다 명확성 우선).
    let oi = decompose(i);
    let oo = decompose(o);
    let matched_out = {
        let (_, m2, _, _) = match_deltas(i, o);
        // 입력 짝이 확정된 수만큼 출력도 소비됐다
        m2.iter().filter(|x| **x).count()
    };
    let left_out = matched_out < oo.len();
    let _ = oi;
    Some(match (left_in, left_out) {
        (true, true) => DescribeFail::Both,
        (true, false) => DescribeFail::UnmatchedInput,
        (false, true) => DescribeFail::UnmatchedOutput,
        (false, false) => DescribeFail::Both, // 도달 불가지만 안전하게
    })
}

/// 짝 없는 **출력** 객체의 성질(진단용 — 시도 179).
///
/// ① 상한의 최대 가용 원인이 "짝 없는 출력"(78건)으로 나왔다. 무엇을 만들지
/// 정하기 전에 그 객체들이 어떤 종류인지 잰다:
/// - `same_shape_color` = 입력에 **같은 모양·같은 색** 원본이 있다 → **복제**로 기술 가능
/// - `same_shape_only` = 같은 모양, 다른 색 → 복제+재색
/// - `novel` = 모양 자체가 입력에 없다 → 진짜 **출현**(모양 합성이 필요)
#[derive(Clone, Copy, Debug, Default)]
pub struct AppearanceStats {
    pub unmatched_out: usize,
    pub same_shape_color: usize,
    pub same_shape_only: usize,
    pub novel: usize,
}

/// 한 훈련쌍의 짝 없는 출력 객체들을 위 세 갈래로 센다.
pub fn appearance_stats(i: &Grid, o: &Grid) -> AppearanceStats {
    let mut s = AppearanceStats::default();
    if i.w != o.w || i.h != o.h {
        return s;
    }
    let oi = decompose(i);
    let oo = decompose(o);
    // match_deltas와 같은 순서로 출력 소비를 재현한다
    let (_, _, _, _) = match_deltas(i, o);
    let mut used_o = vec![false; oo.len()];
    let mut matched = vec![false; oi.len()];
    // 유지
    for (a, ia) in oi.iter().enumerate() {
        for (b, ob) in oo.iter().enumerate() {
            if !used_o[b]
                && ia.x0 == ob.x0
                && ia.y0 == ob.y0
                && shape_key(ia) == shape_key(ob)
                && obj_color(ia) == obj_color(ob)
            {
                used_o[b] = true;
                matched[a] = true;
                break;
            }
        }
    }
    // 재색
    for (a, ia) in oi.iter().enumerate() {
        if matched[a] {
            continue;
        }
        for (b, ob) in oo.iter().enumerate() {
            if !used_o[b] && ia.x0 == ob.x0 && ia.y0 == ob.y0 && shape_key(ia) == shape_key(ob) {
                used_o[b] = true;
                matched[a] = true;
                break;
            }
        }
    }
    // 이동
    for (a, ia) in oi.iter().enumerate() {
        if matched[a] {
            continue;
        }
        let mv = oo
            .iter()
            .enumerate()
            .filter(|(b, ob)| {
                !used_o[*b] && shape_key(ia) == shape_key(ob) && obj_color(ia) == obj_color(ob)
            })
            .min_by_key(|(_, ob)| {
                (ob.x0 as i64 - ia.x0 as i64).abs() + (ob.y0 as i64 - ia.y0 as i64).abs()
            })
            .map(|(b, _)| b);
        if let Some(b) = mv {
            used_o[b] = true;
            matched[a] = true;
        }
    }
    // 남은 출력 객체를 분류한다
    for (b, ob) in oo.iter().enumerate() {
        if used_o[b] {
            continue;
        }
        s.unmatched_out += 1;
        let same_sc = oi
            .iter()
            .any(|ia| shape_key(ia) == shape_key(ob) && obj_color(ia) == obj_color(ob));
        let same_s = oi.iter().any(|ia| shape_key(ia) == shape_key(ob));
        if same_sc {
            s.same_shape_color += 1;
        } else if same_s {
            s.same_shape_only += 1;
        } else {
            s.novel += 1;
        }
    }
    s
}

/// 완전 기술 판정(선택·게이트용): 이동·출현이 섞이면 None — 전이 판정의
/// 엄격함은 유지한다.
pub fn actual_deltas(i: &Grid, o: &Grid) -> Option<Vec<Option<u64>>> {
    if i.w != o.w || i.h != o.h {
        return None;
    }
    let (deltas, _, _, complete) = match_deltas(i, o);
    if complete {
        Some(deltas)
    } else {
        None
    }
}

/// 한 객체의 관측 지점: 성질 · 배타적 델타 · **덧붙는 복제들**.
#[derive(Clone, Debug)]
pub struct Site {
    pub props: [u64; NPROPS],
    /// 배타적 행동(유지=None · 재색 · 이동 · 삭제).
    pub delta: Option<u64>,
    /// 이 객체가 낳은 사본들의 오프셋(인코딩) — 없으면 빈 벡터.
    pub copies: Vec<u64>,
}

/// 훈련쌍에서 객체 델타 경험을 뽑는다(재색·삭제만, 기계적).
///
/// **부분 추출**(시도 167): 이동·출현이 섞인 쌍이라도 짝이 확정된 재색·삭제
/// 델타는 경험으로 남긴다. 완전 기술 요구는 추출이 아니라 **선택·게이트**의
/// 몫이다 — 교사가 버리던 정보를 회수하자던 원칙(시도 151)을 추출기가 다시
/// 어기고 있었다(200과제 중 10개만 경험 생산의 원인).
pub fn extract_obj_rules(train: &[(Grid, Grid)]) -> Vec<Term> {
    let mut out = Vec::new();
    for (i, o) in train {
        if i.w != o.w || i.h != o.h {
            continue;
        }
        let (deltas, matched, copies, _complete) = match_deltas(i, o);
        let objs = decompose(i);
        let props = object_props(i, &objs);
        for (a, d) in deltas.iter().enumerate() {
            // 복제는 배타적 행동과 독립이므로 짝 확정 여부와 무관하게 기록한다
            for &c in &copies[a] {
                out.push(rule_term(&props[a], ACT_COPY, c));
            }
            if !matched[a] {
                continue; // 짝 미확정(이동 후보 등) — 델타를 단정하지 않는다
            }
            match d {
                Some(10) => out.push(rule_term(&props[a], ACT_DELETE, 0)),
                Some(v) if *v >= MOVE_BASE => {
                    out.push(rule_term(&props[a], ACT_MOVE, *v))
                }
                Some(c) => out.push(rule_term(&props[a], ACT_RECOLOR, *c)),
                None => {}
            }
        }
    }
    out
}

/// 수면: 델타 경험을 일반화한다 — 이웃쌍 + 3창(과제 내 구조). 채택은 MDL.
pub fn sleep_obj_abstract(rules: &[Term], lib: &mut Library) -> (usize, usize) {
    let (mut tried, mut added) = (0usize, 0usize);
    for w in rules.windows(2) {
        tried += 1;
        if let Some(a) = generalize(w) {
            if lib.insert(&a, Provenance::MonadDerived) {
                added += 1;
            }
        }
    }
    // 같은 행동끼리 더 넓게 접기(3개 창 — 그룹 전체는 과일반화라 이웃 3개까지만)
    for w in rules.windows(3) {
        tried += 1;
        if let Some(a) = generalize(w) {
            if lib.insert(&a, Provenance::MonadDerived) {
                added += 1;
            }
        }
    }
    (tried, added)
}

/// **과제 간 수면**(시도 168): 서로 다른 과제의 경험끼리 접는다 — 전이 규칙의
/// 원천은 이것이다. 이웃쌍(`windows`)은 대부분 같은 과제 안의 쌍이라, 과제 간
/// 공통 구조(팔레트가 달라도 성립하는 조건)가 거의 생성되지 않고 있었다.
///
/// 같은 행동 종류끼리만 쌍을 만든다(종류가 다르면 LGG가 행동을 변수로 만들어
/// 실행 불가 규칙이 된다). 채택은 여전히 MDL + 중복 병합.
pub fn sleep_obj_cross(groups: &[Vec<Term>], lib: &mut Library) -> (usize, usize) {
    let kind_of = |t: &Term| -> u64 {
        split_orule(t)
            .and_then(|(_, k, _)| match k {
                Term::Const(v) => Some(*v),
                _ => None,
            })
            .unwrap_or(0)
    };
    let (mut tried, mut added) = (0usize, 0usize);
    for gi in 0..groups.len() {
        for gj in gi + 1..groups.len() {
            for a in &groups[gi] {
                let ka = kind_of(a);
                for b in &groups[gj] {
                    if ka != kind_of(b) {
                        continue;
                    }
                    tried += 1;
                    if let Some(abs) = generalize(&[a.clone(), b.clone()]) {
                        if lib.insert(&abs, Provenance::MonadDerived) {
                            added += 1;
                        }
                    }
                }
            }
        }
    }
    (tried, added)
}

/// **스키마 정련**(시도 171): 라이브러리의 규칙들끼리 한 번 더 접는다.
///
/// 성질을 18종으로 늘리자(시도 170) 모호쌍은 428→164로 줄었지만 v2가 풀던
/// 홀드아웃 2건이 0으로 후퇴했다 — 쌍 LGG에서 **우연히 상수로 굳는 슬롯**이
/// 성질 수에 비례해 늘기 때문이다(일반화-특이성 트레이드오프의 실측). 처방은
/// 성질 축소가 아니라 **일반화 사다리의 다음 칸**: 같은 행동의 스키마끼리
/// 다시 LGG를 돌리면, 세 과제 이상에 공통인 조건만 상수로 남는다. 어느 수준이
/// 옳은지는 미리 정하지 않는다 — 여러 수준이 라이브러리에 공존하고, 과제의
/// 증거(select)가 고른다. 채택은 여전히 MDL.
/// **일반화 사다리를 고정점까지 오른다**(시도 173).
///
/// 진단이 지목한 병목은 ②(일관 규칙 존재 11건) → ③(훈련 재현 2건)이고, 원인은
/// 바뀐 객체 208개 중 151개에 **어떤 규칙도 발화하지 않는 것**이다. 선택된 규칙은
/// 정의상 모두 일관적이므로 순서 문제가 아니다 — 규칙이 충분히 일반적이지 않다.
///
/// 한 라운드는 인접 스키마쌍을 접는다. 그 결과를 다시 접으면 더 일반적인 층이
/// 생기고, **여러 층이 라이브러리에 공존한 채 과제의 증거가 고른다**. 사다리는
/// 스스로 멈춘다: MDL 이득 = Σ|구체| − (|스키마| + Σ|대입|)이므로, 두 스키마
/// (크기 16)를 접을 때 다른 슬롯이 8칸 이상이면 이득이 음수가 되어 기각된다.
/// 과일반화로 무너지지 않는 이유가 여기 있다 — 사람이 정한 한계가 아니라 MDL이다.
///
/// 반환: 라운드별 (시도, 추가) — 수렴 양상을 그대로 기록한다.
pub fn sleep_obj_refine_rounds(lib: &mut Library, max_rounds: usize) -> Vec<(usize, usize)> {
    let mut log = Vec::new();
    for _ in 0..max_rounds {
        let (tried, added) = sleep_obj_refine(lib);
        log.push((tried, added));
        if added == 0 {
            break; // 고정점 — 더 일반화할 것이 없다
        }
    }
    log
}

pub fn sleep_obj_refine(lib: &mut Library) -> (usize, usize) {
    let mut by_kind: HashMap<u64, Vec<Term>> = HashMap::new();
    for e in &lib.entries {
        if let Some((_, Term::Const(k), _)) = split_orule(&e.schema) {
            by_kind.entry(*k).or_default().push(e.schema.clone());
        }
    }
    let (mut tried, mut added) = (0usize, 0usize);
    let mut kinds: Vec<u64> = by_kind.keys().copied().collect();
    kinds.sort_unstable();
    for k in kinds {
        let mut group = by_kind.remove(&k).unwrap();
        // 결정론적 순서(문자열 표기) — 인접쌍이 재현 가능해야 한다
        group.sort_by_key(|t| format!("{t}"));
        for w in group.windows(2) {
            tried += 1;
            if let Some(a) = generalize(w) {
                if lib.insert(&a, Provenance::MonadDerived) {
                    added += 1;
                }
            }
        }
    }
    (tried, added)
}

/// 바인딩 일관 발화(시도 164의 의미론을 객체 성질에 적용).
/// 반환: (행동 종류, 매개값).
fn orule_fire(
    props_cond: &[Term],
    kind: &Term,
    param: &Term,
    props: &[u64; NPROPS],
) -> Option<(u64, u64)> {
    let mut bind: Vec<(u32, u64)> = Vec::new();
    for (t, &v) in props_cond.iter().zip(props.iter()) {
        match t {
            Term::Const(c) if *c != v => return None,
            Term::Const(_) => {}
            Term::Var(i) => match bind.iter().find(|(b, _)| b == i) {
                Some((_, prev)) if *prev != v => return None,
                Some(_) => {}
                None => bind.push((*i, v)),
            },
            Term::App(_, _) => return None,
        }
    }
    // 행동 종류는 실행 가능해야 하므로 상수만
    let k = match kind {
        Term::Const(k) => *k,
        _ => return None,
    };
    let p = match param {
        Term::Const(p) => *p,
        Term::Var(v) => bind.iter().find(|(b, _)| b == v).map(|(_, val)| *val)?,
        Term::App(_, _) => return None,
    };
    Some((k, p))
}

/// **증거 기반 선택**: 이 과제의 모든 객체(유지 포함)에 대해 모순 없이 발화하는
/// 규칙만 채택한다. 유지 객체에서 변경 행동이 발화하면 모순이다.
/// 과제의 모든 (성질, 실제 델타) 지점. 하나라도 기술 불가면 빈 벡터 —
/// 선택·진단이 공유하는 표준 좌표계다.
pub fn task_props(train: &[(Grid, Grid)]) -> Vec<Site> {
    let mut sites: Vec<Site> = Vec::new();
    for (i, o) in train {
        if i.w != o.w || i.h != o.h {
            return Vec::new();
        }
        let (deltas, _, copies, complete) = match_deltas(i, o);
        if !complete {
            return Vec::new();
        }
        let objs = decompose(i);
        let props = object_props(i, &objs);
        for ((p, d), c) in props.into_iter().zip(deltas).zip(copies) {
            sites.push(Site { props: p, delta: d, copies: c });
        }
    }
    sites
}

/// 이 규칙이 이 성질 지점에서 발화하는가(진단용 — 시도 170).
pub fn rule_covers(rule: &(Vec<Term>, Term, Term), props: &[u64; NPROPS]) -> bool {
    orule_fire(&rule.0, &rule.1, &rule.2, props).is_some()
}

/// **일관성 필터 이전**에 이 성질 지점에서 발화하며 **정답 행동을 주장하는**
/// 라이브러리 규칙이 있는가(진단용 — 시도 176).
///
/// 이것이 미발화의 두 원인을 가른다:
/// - `false` = 그런 규칙이 아예 없다 → **경험의 구멍**(더 많은/다른 경험 필요)
/// - `true` = 있는데 선택에서 걸렸다 → **성질 판별력 부족**(그 규칙이 다른 자리에서
///   오발화하므로 일관성 검사가 버린 것 — 성질 어휘가 자리를 구분하지 못한다)
pub fn raw_correct_rule_exists(lib: &Library, site: &Site) -> bool {
    for e in &lib.entries {
        let Some((cond, kind, param)) = split_orule(&e.schema) else { continue };
        let Some((k, p)) = orule_fire(cond, kind, param, &site.props) else { continue };
        if action_ok(k, p, site) {
            return true;
        }
    }
    false
}

pub fn select_obj_consistent(
    lib: &Library,
    train: &[(Grid, Grid)],
) -> Vec<(Vec<Term>, Term, Term)> {
    // 훈련쌍별 (성질, 실제 델타) — 하나라도 기술 불가면 빈 손
    let sites = task_props(train);
    if sites.is_empty() {
        return Vec::new();
    }
    let mut kept = Vec::new();
    for e in lib.by_prior() {
        let Some((cond, kind, param)) = split_orule(&lib.entries[e].schema) else { continue };
        let mut consistent = true;
        let mut useful = false;
        for site in &sites {
            let Some((k, p)) = orule_fire(cond, kind, param, &site.props) else { continue };
            if !action_ok(k, p, site) {
                consistent = false;
                break;
            }
            if site.delta.is_some() || !site.copies.is_empty() {
                useful = true;
            }
        }
        if consistent && useful {
            kept.push((cond.clone(), kind.clone(), param.clone()));
        }
    }
    kept
}

/// 한 발화가 실제 델타와 맞는가(선택·진단이 공유하는 판정).
fn action_ok(k: u64, p: u64, site: &Site) -> bool {
    if k == ACT_COPY {
        // 복제는 가산적 — 이 객체가 실제로 그 오프셋의 사본을 낳았는가
        return site.copies.contains(&p);
    }
    match site.delta {
        None => k == ACT_RECOLOR && p == site.props[0], // 자기 색 재색 = 유지와 동치
        Some(10) => k == ACT_DELETE,
        Some(v) if v >= MOVE_BASE => k == ACT_MOVE && p == v,
        Some(c) => k == ACT_RECOLOR && p == c,
    }
}

/// **결정 목록 선택**(시도 177) — 진단이 지목한 처방.
///
/// 미발화 바뀐 객체 151개의 원인을 가르니 **87개(58%)가 "정답 규칙이 있었는데
/// 선택이 버린 것"**이었다. [`select_obj_consistent`]가 각 규칙에게 *모든* 자리에서
/// 단독으로 옳을 것을 요구하기 때문이다. 실제 프로그램은 그렇게 쓰이지 않는다:
/// 구체적 예외가 앞서고 일반 규칙이 뒤를 받치면, 뒤 규칙이 어딘가에서 과발화해도
/// 앞 규칙이 **가려준다**(shadowing).
///
/// 그래서 규칙 하나가 아니라 **순서 있는 목록**을 고른다. 목록에 덧붙이는 규칙은
/// "아직 아무 규칙도 발화하지 않은 자리"에서만 옳으면 된다 — 결정 목록
/// (Rivest 1987)의 학습 규율이며, 도메인 중립이다. 탐욕 기준은 **아직 못 덮은
/// 바뀐 객체를 가장 많이 덮는 것**이고, 오류를 하나라도 내면 후보에서 제외한다.
pub fn select_obj_cover(
    lib: &Library,
    train: &[(Grid, Grid)],
    max_rules: usize,
) -> Vec<(Vec<Term>, Term, Term)> {
    let sites = task_props(train);
    if sites.is_empty() {
        return Vec::new();
    }
    // 1차 통과: 바뀐 자리에서 **한 번이라도 옳게** 발화하는 규칙만 후보로 남기고,
    // 그 발화 위치를 성기게(sparse) 기록한다. 이후 탐욕 루프는 이 표만 본다.
    let mut cands: Vec<((Vec<Term>, Term, Term), Vec<(usize, u64, u64)>)> = Vec::new();
    for e in lib.by_prior() {
        let Some((cond, kind, param)) = split_orule(&lib.entries[e].schema) else { continue };
        let mut fires = Vec::new();
        let mut useful = false;
        for (ix, site) in sites.iter().enumerate() {
            let Some((k, p)) = orule_fire(cond, kind, param, &site.props) else { continue };
            if (site.delta.is_some() || !site.copies.is_empty()) && action_ok(k, p, site) {
                useful = true;
            }
            fires.push((ix, k, p));
        }
        if useful {
            cands.push(((cond.clone(), kind.clone(), param.clone()), fires));
        }
    }
    if cands.is_empty() {
        return Vec::new();
    }

    let mut chosen: Vec<(Vec<Term>, Term, Term)> = Vec::new();
    // shadowed[i] = 앞선 규칙이 이미 이 자리에서 발화함
    let mut shadowed = vec![false; sites.len()];
    let mut used = vec![false; cands.len()];
    for _ in 0..max_rules {
        // 미해결 = 아직 아무 규칙도 발화하지 않은 **바뀐** 자리
        let remaining = sites
            .iter()
            .enumerate()
            .filter(|(ix, s)| (s.delta.is_some() || !s.copies.is_empty()) && !shadowed[*ix])
            .count();
        if remaining == 0 {
            return chosen;
        }
        let mut best: Option<(usize, usize)> = None; // (덮는 수, 후보 색인)
        for (ci, (_, fires)) in cands.iter().enumerate() {
            if used[ci] {
                continue;
            }
            let mut cover = 0usize;
            let mut bad = false;
            for &(ix, k, p) in fires {
                if shadowed[ix] {
                    continue; // 앞 규칙이 가린다 — 이 자리에서의 오발화는 무해
                }
                let site = &sites[ix];
                if !action_ok(k, p, site) {
                    bad = true;
                    break;
                }
                if site.delta.is_some() || !site.copies.is_empty() {
                    cover += 1;
                }
            }
            if bad || cover == 0 {
                continue;
            }
            if best.map(|(bc, _)| cover > bc).unwrap_or(true) {
                best = Some((cover, ci));
            }
        }
        let Some((_, ci)) = best else { return chosen };
        used[ci] = true;
        for &(ix, _, _) in &cands[ci].1 {
            shadowed[ix] = true;
        }
        chosen.push(cands[ci].0.clone());
    }
    chosen
}

/// 선택 규칙 적용: 발화한 객체에 행동을 실행(재색/삭제), 나머지는 유지.
pub fn apply_obj_rules(rules: &[(Vec<Term>, Term, Term)], g: &Grid) -> Grid {
    let objs = decompose(g);
    let props = object_props(g, &objs);
    // 1패스: 배타적 행동은 **첫 발화**(사전분포 순서), 복제는 **전부 수집**한다.
    // 복제만 다른 이유는 성질이 다르기 때문이다 — 한 객체가 여러 사본을 낳는다.
    let mut acts: Vec<Option<(u64, u64)>> = vec![None; objs.len()];
    let mut copy_acts: Vec<Vec<u64>> = vec![Vec::new(); objs.len()];
    for (ix, p) in props.iter().enumerate() {
        for (cond, kind, param) in rules {
            let Some((k, val)) = orule_fire(cond, kind, param, p) else { continue };
            if k == ACT_COPY {
                if !copy_acts[ix].contains(&val) {
                    copy_acts[ix].push(val);
                }
            } else if acts[ix].is_none() {
                acts[ix] = Some((k, val));
            }
        }
    }
    let mut out = g.clone();
    // 2패스-지우기: 삭제·이동의 옛 자리를 먼저 비운다(이동이 서로의 옛 자리로
    // 들어가도 안전하도록 그리기와 분리)
    for (o, a) in objs.iter().zip(acts.iter()) {
        let clear = matches!(a, Some((k, _)) if *k == ACT_DELETE || *k == ACT_MOVE);
        if !clear {
            continue;
        }
        for dy in 0..o.h {
            for dx in 0..o.w {
                if o.mask[dy * o.w + dx] {
                    out.set(o.x0 + dx, o.y0 + dy, 0);
                }
            }
        }
    }
    // 2패스-그리기: 재색과 이동의 새 자리
    for (o, a) in objs.iter().zip(acts.iter()) {
        match a {
            Some((k, val)) if *k == ACT_RECOLOR && *val <= 9 => {
                for dy in 0..o.h {
                    for dx in 0..o.w {
                        if o.mask[dy * o.w + dx] {
                            out.set(o.x0 + dx, o.y0 + dy, *val as u8);
                        }
                    }
                }
            }
            Some((k, val)) if *k == ACT_MOVE => {
                let Some((mdx, mdy)) = decode_move(*val) else { continue };
                let c = obj_color(o);
                for dy in 0..o.h {
                    for dx in 0..o.w {
                        if !o.mask[dy * o.w + dx] {
                            continue;
                        }
                        let nx = o.x0 as i64 + dx as i64 + mdx;
                        let ny = o.y0 as i64 + dy as i64 + mdy;
                        if nx >= 0 && ny >= 0 && (nx as usize) < g.w && (ny as usize) < g.h {
                            out.set(nx as usize, ny as usize, c);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // 3패스: 복제 — 원본을 그대로 두고 오프셋 자리에 사본을 그린다
    for (o, offs) in objs.iter().zip(copy_acts.iter()) {
        let c = obj_color(o);
        for &v in offs {
            let Some((mdx, mdy)) = decode_move(v) else { continue };
            for dy in 0..o.h {
                for dx in 0..o.w {
                    if !o.mask[dy * o.w + dx] {
                        continue;
                    }
                    let nx = o.x0 as i64 + dx as i64 + mdx;
                    let ny = o.y0 as i64 + dy as i64 + mdy;
                    if nx >= 0 && ny >= 0 && (nx as usize) < g.w && (ny as usize) < g.h {
                        out.set(nx as usize, ny as usize, c);
                    }
                }
            }
        }
    }
    out
}

/// 선택 규칙이 훈련쌍을 완전히 재현하는가(전이 게이트).
pub fn obj_rules_reproduce(rules: &[(Vec<Term>, Term, Term)], train: &[(Grid, Grid)]) -> bool {
    !rules.is_empty()
        && train
            .iter()
            .all(|(i, o)| i.w == o.w && i.h == o.h && &apply_obj_rules(rules, i) == o)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(g: &mut Grid, x0: usize, y0: usize, w: usize, h: usize, c: u8) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                g.set(x, y, c);
            }
        }
    }

    /// 추출은 바뀐 객체만, 재색·삭제로 분류한다(해석 없음).
    #[test]
    fn extraction_classifies_recolor_and_delete() {
        let mut i = Grid::new(8, 8);
        place(&mut i, 0, 0, 3, 3, 4); // 큰 것
        place(&mut i, 5, 5, 1, 1, 7); // 작은 것
        let mut o = i.clone();
        place(&mut o, 0, 0, 3, 3, 2); // 재색
        place(&mut o, 5, 5, 1, 1, 0); // 삭제
        let r = extract_obj_rules(&[(i, o)]);
        assert_eq!(r.len(), 2, "재색 1 + 삭제 1이어야 한다");
        let kinds: Vec<u64> = r
            .iter()
            .filter_map(|t| split_orule(t).map(|(_, k, _)| match k {
                Term::Const(v) => *v,
                _ => 99,
            }))
            .collect();
        assert!(kinds.contains(&ACT_RECOLOR) && kinds.contains(&ACT_DELETE));
    }

    /// **이동 1급 델타**(시도 169): 이동이 섞인 쌍이 완전 기술되고, 이동은
    /// 삭제로 오분류되지 않으며(옛 자리 배경화만으로는 삭제가 아니다), 재색과
    /// 이동이 각각 올바른 행동·매개로 추출된다.
    #[test]
    fn move_is_first_class_and_never_misread_as_delete() {
        let mut i = Grid::new(9, 9);
        place(&mut i, 0, 0, 2, 2, 3); // 재색될 것
        place(&mut i, 6, 6, 2, 2, 5); // 이동할 것
        let mut o = Grid::new(9, 9);
        place(&mut o, 0, 0, 2, 2, 7); // 재색됨
        place(&mut o, 2, 6, 2, 2, 5); // 왼쪽으로 4칸 이동
        // v2: 이동 포함 쌍도 완전 기술된다
        let deltas = actual_deltas(&i, &o).expect("이동 1급인데 완전 기술 실패");
        assert!(deltas.iter().any(|d| matches!(d, Some(v) if *v >= MOVE_BASE)));
        let r = extract_obj_rules(&[(i, o)]);
        assert_eq!(r.len(), 2, "재색 1 + 이동 1이어야 한다: {}건", r.len());
        let mut kinds: Vec<u64> = r
            .iter()
            .filter_map(|t| split_orule(t).and_then(|(_, k, _)| match k {
                Term::Const(v) => Some(*v),
                _ => None,
            }))
            .collect();
        kinds.sort_unstable();
        assert_eq!(kinds, vec![ACT_RECOLOR, ACT_MOVE]);
        // 이동 매개가 정확히 (-4, 0)인가
        let mv = r
            .iter()
            .find_map(|t| split_orule(t).and_then(|(_, k, p)| {
                (k == &Term::Const(ACT_MOVE)).then_some(p.clone())
            }))
            .unwrap();
        assert_eq!(mv, Term::Const(encode_move(-4, 0)));
    }

    /// **이동 전이**: "오른쪽으로 2칸"을 색·배치가 다른 두 과제에서 경험 →
    /// 본 적 없는 세 번째 과제를 재현하고 시험까지 푼다. 이동 벡터는 상수로
    /// 공유되고 나머지 성질은 LGG가 변수로 접는다.
    #[test]
    fn move_rule_transfers_across_tasks() {
        let mk = |x: usize, y: usize, c: u8| {
            let mut i = Grid::new(9, 9);
            place(&mut i, x, y, 2, 2, c);
            let mut o = Grid::new(9, 9);
            place(&mut o, x + 2, y, 2, 2, c);
            (i, o)
        };
        let mut rules = extract_obj_rules(&[mk(1, 1, 3)]);
        rules.extend(extract_obj_rules(&[mk(2, 4, 5)]));
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        // (경험과 같은 성질 부류: 비테두리 — x0=0이면 border-touch 성질이 달라
        //  발화하지 않는 것이 올바른 동작이다)
        let (ci, co) = mk(1, 3, 7);
        let train = [(ci, co)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty(), "이동 일관 규칙을 못 골랐다");
        assert!(obj_rules_reproduce(&sel, &train), "이동 재현 실패");
        let (ti, to) = mk(3, 2, 9);
        assert_eq!(apply_obj_rules(&sel, &ti), to, "이동 시험 실패");
    }

    /// **과제 간 전이**: "가장 작은 객체를 지워라"를 배치·색이 다른 두 과제에서
    /// 경험 → 본 적 없는 세 번째 과제의 훈련쌍을 재현하고 시험까지 푼다.
    /// 리터럴이 아니라 성질(최소 크기)이 조건이 됐다는 뜻이다.
    #[test]
    fn delete_smallest_transfers_across_tasks() {
        let mk = |big: (usize, usize, u8), small: (usize, usize, u8)| {
            let mut i = Grid::new(9, 9);
            place(&mut i, big.0, big.1, 3, 3, big.2);
            place(&mut i, small.0, small.1, 1, 1, small.2);
            let mut o = i.clone();
            place(&mut o, small.0, small.1, 1, 1, 0);
            (i, o)
        };
        // 과제 A·B: 다른 배치·다른 색
        let mut rules = extract_obj_rules(&[mk((0, 0, 3), (7, 7, 5))]);
        rules.extend(extract_obj_rules(&[mk((5, 2, 6), (1, 6, 8))]));
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        assert!(!lib.entries.is_empty(), "수면이 규칙을 만들지 못했다");

        // 과제 C: 또 다른 배치·색 — 본 적 없는 조합
        // (경험한 작은 객체들과 같은 성질 부류: 비테두리·아래쪽 — 성질이 조건이
        //  됐으므로 그 부류를 벗어나면 발화하지 않는 것이 올바른 동작이다)
        let (ci, co) = mk((2, 4, 9), (7, 7, 1));
        let train = [(ci.clone(), co.clone())];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty(), "일관 규칙을 못 골랐다");
        assert!(obj_rules_reproduce(&sel, &train), "훈련 재현 실패");
        // 시험쌍(같은 규칙, 또 다른 배치)
        let (ti, to) = mk((3, 3, 2), (1, 7, 4));
        assert_eq!(apply_obj_rules(&sel, &ti), to, "시험 실패");
    }

    /// **팔레트 독립 재색**: "전부 다수색이 된다"를 서로 다른 팔레트의 두 과제에서
    /// 경험 → LGG가 param과 다수색 성질 자리를 같은 변수로 접는다 → 세 번째
    /// 팔레트에서 재현. 상수 param으로는 원리상 불가능한 전이다.
    #[test]
    fn recolor_to_majority_is_palette_independent() {
        let mk = |maj: u8, minor: u8| {
            let mut i = Grid::new(9, 5);
            place(&mut i, 0, 0, 4, 4, maj); // 다수색 덩어리
            place(&mut i, 6, 1, 1, 1, minor);
            place(&mut i, 6, 3, 1, 1, minor);
            let mut o = i.clone();
            place(&mut o, 6, 1, 1, 1, maj);
            place(&mut o, 6, 3, 1, 1, maj);
            (i, o)
        };
        let mut rules = extract_obj_rules(&[mk(3, 5)]);
        rules.extend(extract_obj_rules(&[mk(6, 2)]));
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        // param이 변수인 규칙이 실제로 생겼는가
        let has_var_param = lib.entries.iter().any(|e| {
            split_orule(&e.schema)
                .map(|(_, _, p)| matches!(p, Term::Var(_)))
                .unwrap_or(false)
        });
        assert!(has_var_param, "param 변수 규칙이 수면에서 나오지 않았다");

        let (ci, co) = mk(9, 1); // 본 적 없는 팔레트
        let train = [(ci, co)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty() && obj_rules_reproduce(&sel, &train), "팔레트 독립 전이 실패");
    }

    /// **일반화 사다리**(시도 173): 두 과제만으로는 조건이 과도하게 구체적이라
    /// 세 번째 과제에서 발화하지 못한다. 라운드를 더 올리면 스키마끼리 다시
    /// 접혀 발화 범위가 넓어지고, MDL이 그 상승을 스스로 멈춘다(고정점).
    #[test]
    fn refinement_ladder_widens_firing_then_converges() {
        // 같은 규칙("최소 객체 삭제")을 색만 다른 여러 과제에서 경험한다.
        // (성질이 크게 다르면 쌍 LGG의 MDL 이득이 음수가 되어 기각된다 — 사다리가
        //  오르려면 접을 만큼 닮은 경험이 있어야 한다는 것 자체가 MDL의 규율이다.)
        let mk = |bc: u8, sc: u8| {
            let mut i = Grid::new(12, 12);
            place(&mut i, 1, 1, 3, 3, bc);
            place(&mut i, 8, 5, 1, 1, sc);
            let mut o = i.clone();
            place(&mut o, 8, 5, 1, 1, 0);
            (i, o)
        };
        let groups: Vec<Vec<Term>> = [(3u8, 5u8), (6, 8), (2, 4)]
            .iter()
            .map(|&(b, s)| extract_obj_rules(&[mk(b, s)]))
            .collect();
        let mut lib = Library::new();
        let all: Vec<Term> = groups.iter().flatten().cloned().collect();
        sleep_obj_abstract(&all, &mut lib);
        sleep_obj_cross(&groups, &mut lib);

        // 사다리의 목적은 규칙 **수**가 아니라 **발화 범위**다. 본 적 없는 팔레트
        // 과제에서 바뀐 객체를 덮는 규칙이 몇 개인지로 잰다.
        let unseen = [mk(7, 9)];
        let sites = task_props(&unseen);
        let covered = |l: &Library| -> usize {
            let sel = select_obj_consistent(l, &unseen);
            sites
                .iter()
                .filter(|s| s.delta.is_some() && sel.iter().any(|r| rule_covers(r, &s.props)))
                .count()
        };
        let before_cov = covered(&lib);

        let log = sleep_obj_refine_rounds(&mut lib, 8);
        assert!(!log.is_empty());
        // 스스로 멈춘다(고정점) — 사람이 라운드 수를 조율하지 않는다
        assert!(
            log.last().map(|(_, added)| *added == 0).unwrap_or(false) || log.len() == 8,
            "고정점에 닿지 않았고 라운드 상한에도 걸리지 않았다: {log:?}"
        );
        // 사다리는 덮개를 **깎지 않는다**(더 일반적인 층이 더해질 뿐)
        assert!(
            covered(&lib) >= before_cov,
            "사다리가 발화 범위를 줄였다: {before_cov} → {}",
            covered(&lib)
        );
        // 과일반화는 사람이 아니라 MDL이 막는다 — 라이브러리 전체가 이득 양수
        assert!(lib.entries.iter().all(|e| e.gain > 0), "MDL 불변식 위반");
    }

    /// **결정 목록 선택**(시도 177): 예외 규칙이 앞서 가려주면, 뒤의 일반 규칙이
    /// 어딘가에서 과발화해도 쓸 수 있다. 단독 일관성만 요구하는 선택은 이 조합을
    /// 못 찾는다 — 정답 규칙이 있는데도 버린다(홀드아웃 미발화의 58%).
    #[test]
    fn decision_list_uses_rules_that_solo_consistency_would_discard() {
        // 규칙: "가장 작은 것은 지운다. 그 외 모든 것은 4로 칠한다."
        // 뒤 규칙(일반)은 단독으로 보면 최소 객체에서 오발화하므로 버려진다.
        let mk = |sc: u8, bc: u8, mc: u8| {
            let mut i = Grid::new(12, 12);
            place(&mut i, 0, 0, 3, 3, bc); // 최대
            place(&mut i, 5, 5, 2, 2, mc); // 중간
            place(&mut i, 9, 9, 1, 1, sc); // 최소 → 삭제
            let mut o = i.clone();
            place(&mut o, 9, 9, 1, 1, 0);
            place(&mut o, 0, 0, 3, 3, 4);
            place(&mut o, 5, 5, 2, 2, 4);
            (i, o)
        };
        let groups: Vec<Vec<Term>> = [(5u8, 3u8, 7u8), (8, 6, 2), (1, 9, 3)]
            .iter()
            .map(|&(s, b, m)| extract_obj_rules(&[mk(s, b, m)]))
            .collect();
        let mut lib = Library::new();
        let all: Vec<Term> = groups.iter().flatten().cloned().collect();
        sleep_obj_abstract(&all, &mut lib);
        sleep_obj_cross(&groups, &mut lib);
        sleep_obj_refine_rounds(&mut lib, 6);

        let unseen = [mk(6, 2, 9)];
        // 단독 일관성 선택으로는 재현하지 못한다(일반 규칙이 버려지므로)
        let solo = select_obj_consistent(&lib, &unseen);
        let solo_ok = obj_rules_reproduce(&solo, &unseen);
        // 결정 목록은 예외 우선 + 일반 후속으로 재현한다
        let list = select_obj_cover(&lib, &unseen, 12);
        assert!(
            obj_rules_reproduce(&list, &unseen),
            "결정 목록이 재현에 실패했다(단독 선택 재현={solo_ok})"
        );
        assert!(list.len() >= 2, "예외+일반 두 층이 나와야 한다: {}", list.len());
    }

    /// **복제는 가산적**(시도 180): 한 객체가 여러 사본을 낳고, 원본은 남는다.
    /// 배타적 행동(첫 발화 승)과 달리 발화한 복제 규칙을 **전부** 실행해야 한다.
    #[test]
    fn copy_is_additive_and_transfers() {
        // 규칙: "작은 사각형을 오른쪽 3칸·아래 3칸에 각각 복제한다"
        let mk = |x: usize, y: usize, c: u8| {
            let mut i = Grid::new(14, 14);
            place(&mut i, x, y, 2, 2, c);
            let mut o = i.clone();
            place(&mut o, x + 5, y, 2, 2, c);
            place(&mut o, x, y + 5, 2, 2, c);
            (i, o)
        };
        // 추출: 사본 2개가 모두 복제 경험으로 나오고, 원본은 유지(배타 행동 없음)
        let r = extract_obj_rules(&[mk(1, 1, 3)]);
        assert_eq!(r.len(), 2, "복제 2건이어야 한다: {}건", r.len());
        assert!(r.iter().all(|t| matches!(
            split_orule(t).map(|(_, k, _)| k),
            Some(Term::Const(x)) if *x == ACT_COPY
        )));
        // 완전 기술로 인정된다(예전에는 짝 없는 출력으로 탈락)
        let (i0, o0) = mk(1, 1, 3);
        assert!(actual_deltas(&i0, &o0).is_some(), "복제 포함 쌍이 완전 기술 실패");

        // 전이: 색·위치가 다른 두 과제 경험 → 본 적 없는 세 번째에서 재현·시험
        let groups: Vec<Vec<Term>> = [(1usize, 1usize, 3u8), (4, 2, 6)]
            .iter()
            .map(|&(x, y, c)| extract_obj_rules(&[mk(x, y, c)]))
            .collect();
        let mut lib = Library::new();
        let all: Vec<Term> = groups.iter().flatten().cloned().collect();
        sleep_obj_abstract(&all, &mut lib);
        sleep_obj_cross(&groups, &mut lib);
        let (ci, co) = mk(2, 5, 8);
        let train = [(ci, co)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!sel.is_empty(), "복제 일관 규칙을 못 골랐다");
        assert!(obj_rules_reproduce(&sel, &train), "복제 재현 실패");
        let (ti, to) = mk(6, 3, 2);
        assert_eq!(apply_obj_rules(&sel, &ti), to, "복제 시험 실패");
    }

    /// 맞지 않는 과제에서는 게이트가 막는다(거짓 양성 방지).
    #[test]
    fn gate_rejects_unrelated_task() {
        let mut i = Grid::new(6, 6);
        place(&mut i, 0, 0, 2, 2, 3);
        place(&mut i, 4, 4, 1, 1, 5);
        let mut o = i.clone();
        place(&mut o, 4, 4, 1, 1, 0);
        let rules = {
            let mut r = extract_obj_rules(&[(i.clone(), o.clone())]);
            r.extend(r.clone());
            r
        };
        let mut lib = Library::new();
        sleep_obj_abstract(&rules, &mut lib);
        // 전혀 다른 변환: 큰 것을 재색(작은 것 유지)
        let mut o2 = i.clone();
        place(&mut o2, 0, 0, 2, 2, 7);
        let train = [(i, o2)];
        let sel = select_obj_consistent(&lib, &train);
        assert!(!obj_rules_reproduce(&sel, &train), "맞지 않는데 재현을 통과시켰다");
    }
}
