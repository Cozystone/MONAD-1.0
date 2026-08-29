//! C2 — 구조 추상화 (anti-unification + MDL).
//!
//! PRD가 크리티컬 패스로 지정한 work package. `schema.rs`는 **명제 규칙 귀납**
//! (제약 → 결과)이고, 이 모듈은 **구조 일반화**다: 두 개의 구체 구조에서
//! 가장 구체적인 공통 일반화(least general generalization, Plotkin 1970)를
//! 계산하고, MDL로 그 일반화를 채택할지 판정한다.
//!
//! # 왜 이것이 학습인가
//!
//! 경험은 구체적이다("빨강 사각형을 오른쪽으로 2칸"). 지능은 그 구체에서
//! 변수를 발견한다("색 X의 도형을 방향 D로 N칸"). 이 모듈이 그 발견을 한다.
//! 발견의 판정 기준은 사람의 판단이 아니라 **압축**이다: 일반화 하나 + 대입
//! 목록이 원본들보다 짧으면 그 일반화는 실재하는 구조다(MDL).
//!
//! # 교리
//!
//! - 이 기계는 **도메인을 모른다.** 항(term)이면 무엇이든 일반화한다 —
//!   ARC 프로그램이든, 미로 전이든, 언어 시퀀스든.
//! - 발견된 스키마에는 **출처가 붙는다**([`Provenance`]). 사람이 심은 원시어와
//!   시스템이 경험에서 뽑은 구조를 절대 섞어 세지 않는다.
//! - 라이브러리는 **디스크에 축적된다.** 코드가 고정된 채로 경험만 쌓여도
//!   다음 실행이 더 잘하는 것 — 그것이 이 모듈의 존재 이유다.

use std::collections::HashMap;
use std::fmt;

/// 구조의 출처. 실험 회계의 기본 단위 — 절대 혼합 집계하지 않는다.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provenance {
    /// 사람이 작성한 원시어·기질(baseline).
    HumanDerived,
    /// MONAD가 경험에서 발견한 구조(schema/program/relation).
    MonadDerived,
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provenance::HumanDerived => write!(f, "HUMAN_DERIVED"),
            Provenance::MonadDerived => write!(f, "MONAD_DERIVED"),
        }
    }
}

/// 일반화의 대상이 되는 항. 도메인 중립.
///
/// - [`Term::Const`] — 원자값(색·번호·연산 id 등 무엇이든 u64로 인코딩)
/// - [`Term::Var`] — 구멍(일반화가 만든 변수)
/// - [`Term::App`] — 함자와 인자들(구조)
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Term {
    Var(u32),
    Const(u64),
    App(u32, Vec<Term>),
}

impl Term {
    /// 항의 크기(노드 수) = MDL 서술 비용.
    pub fn size(&self) -> usize {
        match self {
            Term::Var(_) | Term::Const(_) => 1,
            Term::App(_, args) => 1 + args.iter().map(|t| t.size()).sum::<usize>(),
        }
    }

    /// 이 항이 포함한 변수 번호들(오름차순, 중복 제거).
    pub fn vars(&self) -> Vec<u32> {
        let mut v = Vec::new();
        self.collect_vars(&mut v);
        v.sort_unstable();
        v.dedup();
        v
    }

    fn collect_vars(&self, out: &mut Vec<u32>) {
        match self {
            Term::Var(i) => out.push(*i),
            Term::Const(_) => {}
            Term::App(_, args) => {
                for a in args {
                    a.collect_vars(out);
                }
            }
        }
    }

    /// 변수가 하나도 없으면 구체 항(ground).
    pub fn is_ground(&self) -> bool {
        match self {
            Term::Var(_) => false,
            Term::Const(_) => true,
            Term::App(_, args) => args.iter().all(|a| a.is_ground()),
        }
    }

    /// 대입 적용 — 변수를 값으로 채운다(재구체화).
    pub fn substitute(&self, bindings: &HashMap<u32, Term>) -> Term {
        match self {
            Term::Var(i) => bindings.get(i).cloned().unwrap_or(Term::Var(*i)),
            Term::Const(c) => Term::Const(*c),
            Term::App(f, args) => {
                Term::App(*f, args.iter().map(|a| a.substitute(bindings)).collect())
            }
        }
    }

    /// 단방향 매칭: 이 스키마가 `concrete`를 포섭하는가(subsumption).
    /// 성공하면 그 대입을 돌려준다 — 스키마가 이미 아는 사례인지 판정하는 데 쓴다.
    pub fn matches(&self, concrete: &Term) -> Option<HashMap<u32, Term>> {
        let mut b = HashMap::new();
        if self.match_into(concrete, &mut b) {
            Some(b)
        } else {
            None
        }
    }

    fn match_into(&self, c: &Term, b: &mut HashMap<u32, Term>) -> bool {
        match (self, c) {
            (Term::Var(i), _) => match b.get(i) {
                Some(prev) => prev == c,
                None => {
                    b.insert(*i, c.clone());
                    true
                }
            },
            (Term::Const(a), Term::Const(x)) => a == x,
            (Term::App(f, xs), Term::App(g, ys)) => {
                f == g
                    && xs.len() == ys.len()
                    && xs.iter().zip(ys.iter()).all(|(x, y)| x.match_into(y, b))
            }
            _ => false,
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Var(i) => write!(f, "?{i}"),
            Term::Const(c) => write!(f, "{c}"),
            Term::App(fun, args) => {
                write!(f, "f{fun}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// 두 항의 최소 일반화 결과.
#[derive(Clone, Debug)]
pub struct Generalization {
    /// 일반화된 항(변수 포함).
    pub term: Term,
    /// 변수 → 첫 항에서의 값.
    pub left: HashMap<u32, Term>,
    /// 변수 → 둘째 항에서의 값.
    pub right: HashMap<u32, Term>,
}

/// **Anti-unification (Plotkin LGG)**: 두 구체 구조의 가장 구체적인 공통 일반화.
///
/// 같은 자리에 같은 값이면 그 값을 남기고, 다르면 변수를 만든다. 같은 (좌,우)
/// 값 쌍이 여러 자리에 나오면 **같은 변수를 공유**한다 — 이것이 "두 자리가
/// 함께 변한다"는 관계를 포착하는 지점이다(공유 없는 순진한 버전은 이 구조를
/// 잃는다).
pub fn lgg(a: &Term, b: &Term) -> Generalization {
    let mut shared: HashMap<(Term, Term), u32> = HashMap::new();
    let mut left = HashMap::new();
    let mut right = HashMap::new();
    let mut next = 0u32;
    let term = lgg_rec(a, b, &mut shared, &mut left, &mut right, &mut next);
    Generalization { term, left, right }
}

fn lgg_rec(
    a: &Term,
    b: &Term,
    shared: &mut HashMap<(Term, Term), u32>,
    left: &mut HashMap<u32, Term>,
    right: &mut HashMap<u32, Term>,
    next: &mut u32,
) -> Term {
    match (a, b) {
        (Term::Const(x), Term::Const(y)) if x == y => Term::Const(*x),
        (Term::App(f, xs), Term::App(g, ys)) if f == g && xs.len() == ys.len() => Term::App(
            *f,
            xs.iter()
                .zip(ys.iter())
                .map(|(x, y)| lgg_rec(x, y, shared, left, right, next))
                .collect(),
        ),
        _ => {
            let key = (a.clone(), b.clone());
            if let Some(&v) = shared.get(&key) {
                return Term::Var(v);
            }
            let v = *next;
            *next += 1;
            shared.insert(key, v);
            left.insert(v, a.clone());
            right.insert(v, b.clone());
            Term::Var(v)
        }
    }
}

/// 여러 구체 항의 공통 일반화 + MDL 이득.
///
/// 이득 = Σ|구체| − (|스키마| + Σ|대입|). 양수면 그 스키마는 **경험을 압축한다**
/// = 실재하는 구조다. 음수면 일반화가 과해 아무것도 설명하지 못한다는 뜻.
#[derive(Clone, Debug)]
pub struct Abstraction {
    pub schema: Term,
    /// 구체 사례별 대입(스키마의 변수 → 값).
    pub instances: Vec<HashMap<u32, Term>>,
    /// MDL 압축 이득(양수여야 채택).
    pub gain: i64,
}

/// 구체 항들을 접어 하나의 스키마로 일반화한다(2개 이상 필요).
pub fn generalize(terms: &[Term]) -> Option<Abstraction> {
    if terms.len() < 2 {
        return None;
    }
    let mut schema = terms[0].clone();
    for t in &terms[1..] {
        schema = lgg(&schema, t).term;
    }
    // 전부 변수로 뭉개진 일반화는 구조를 설명하지 않는다
    if matches!(schema, Term::Var(_)) {
        return None;
    }
    let mut instances = Vec::with_capacity(terms.len());
    for t in terms {
        instances.push(schema.matches(t)?);
    }
    let concrete: usize = terms.iter().map(|t| t.size()).sum();
    let cost = schema.size()
        + instances
            .iter()
            .map(|m| m.values().map(|v| v.size()).sum::<usize>())
            .sum::<usize>();
    Some(Abstraction {
        gain: concrete as i64 - cost as i64,
        schema,
        instances,
    })
}

/// 라이브러리 한 항목 — 발견된 스키마 + 그 효용의 역사.
#[derive(Clone, Debug)]
pub struct Entry {
    pub schema: Term,
    pub provenance: Provenance,
    /// 발견 시점의 MDL 이득.
    pub gain: i64,
    /// 발견의 근거가 된 구체 사례 수.
    pub support: u32,
    /// 재구체화를 시도한 횟수.
    pub tries: u32,
    /// 그중 성공(새 문제를 실제로 푼) 횟수.
    pub wins: u32,
    /// 발견 근거가 된 구체 대입들(상한 [`MAX_KEPT_BINDINGS`]).
    ///
    /// 두 가지 일을 한다: ①재구체화 후보값의 출처(경험이 본 값들)
    /// ②**신규 재구체화 판정** — 여기에 없는 대입으로 풀면 그것은 경험의
    /// 복사가 아니라 일반화의 산물이다.
    pub bindings: Vec<HashMap<u32, Term>>,
    /// **이 스키마를 낳은 경험의 이름들**(과제 단위).
    ///
    /// 지금까지 출처 분리는 계층마다 "출처 과제 이름 목록" 파일 하나로 했다.
    /// 그것은 거칠다 — 어떤 과제가 무엇 하나라도 기여했으면 그 과제는 영원히
    /// 집계에서 빠진다. 항목마다 출처를 달면 **표적별 배제**(leave-one-out)가
    /// 가능해진다: 과제 T를 풀 때 T에서 나온 항목만 빼고 나머지 399과제의
    /// 경험을 전부 쓴다. 과제당 기준은 **더 엄격**해지고(출처가 자기 표적이
    /// 되는 일이 원리적으로 없다) 경험량은 두 배가 된다.
    pub sources: Vec<String>,
    /// **혼자서** 이 스키마를 구성한 과제들(`sources`의 부분집합).
    ///
    /// 한 과제의 증거만으로 이 스키마가 만들어졌다면 그 과제는 여기 들어간다.
    /// 두 과제를 접어 만든 것(과제 간 반일반화)은 어느 쪽도 혼자가 아니므로
    /// 들어가지 않는다.
    ///
    /// 이 구분이 필요한 이유: 표적 T가 기여했다는 사실만으로 배제하면 **너무
    /// 엄격**하다. 다른 과제 U가 **혼자서** 같은 스키마를 만든 적이 있다면 그
    /// 스키마는 T가 없어도 존재했을 것이고, 그것으로 T를 푸는 것은 순환이 아니다.
    /// 실제로 시도 207까지 유일한 code-free 획득 과제(`aabf363d`)는 출처 풀이
    /// 400으로 넓어지자 자기도 565개 항목의 출처가 되었다 — 이 구분이 없으면
    /// 정당한 전이까지 함께 지워진다.
    pub solo_sources: Vec<String>,
}

/// 항목당 보관하는 구체 대입의 상한(라이브러리 비대 방지).
pub const MAX_KEPT_BINDINGS: usize = 12;

impl Entry {
    /// 탐색 사전분포 점수 — 성공 이력이 있는 스키마를 먼저 시도한다.
    /// (라플라스 평활: 시도 없는 신규 스키마도 기회를 얻는다.)
    pub fn prior(&self) -> f64 {
        let w = (self.wins as f64 + 1.0) / (self.tries as f64 + 2.0);
        w * (1.0 + (self.gain.max(0) as f64).ln_1p())
    }

    /// **표적 배제 판정**: 이 항목을 과제 `target`을 풀 때 써도 되는가.
    ///
    /// 규칙은 가장 엄격한 것을 쓴다 — **`target`이 조금이라도 기여했으면 못
    /// 쓴다.** 느슨한 대안("`target` 아닌 출처가 하나라도 있으면 쓴다")도
    /// 생각했지만 **틀렸다**: 그 논리는 각 출처가 자기 증거만으로 같은 스키마를
    /// 독립 구성했을 때만 성립하고, 그것은 [`Library::insert`]의 병합 경로에서만
    /// 참이다. 과제를 가로지르는 반일반화(`sleep_obj_abstract`/`_cross`)는
    /// 여러 과제의 증거로 스키마를 **함께** 만들므로, 출처 하나를 빼면 결과가
    /// 달라진다. 두 경우를 항목 단위로 구분해 관리하는 것보다 전부 엄격하게
    /// 배제하는 편이 수율은 낮아도 주장이 무너지지 않는다.
    ///
    /// 순서대로 세 가지를 묻는다:
    ///
    /// 1. 사람이 심은 원시어인가 → 과제에서 나온 것이 아니므로 늘 쓴다.
    /// 2. 출처가 **비어 있는** MONAD 항목인가 → **쓰지 않는다.** 출처를 모르면
    ///    `target` 자신에게서 나왔을 가능성을 배제할 수 없다. 이 조항이 없으면
    ///    출처 열이 없던 옛 라이브러리 파일을 읽는 것만으로 배제가 통째로
    ///    무력화된다(조용히, 아무 오류 없이).
    /// 3. `target`이 아닌 과제가 **혼자서** 이 스키마를 만든 적이 있는가 →
    ///    그렇다면 이 스키마는 `target` 없이도 존재했으므로 쓴다.
    ///    아니라면 `target`이 조금이라도 기여했는지로 판정한다.
    ///
    /// 3번이 [`solo_sources`](Entry::solo_sources)가 필요한 이유다. 그것 없이
    /// "기여했으면 무조건 배제"만 쓰면, 여러 과제가 **각자 독립적으로** 재구성한
    /// 가장 일반적인 규칙일수록 더 많은 표적에서 지워진다 — 전이에 가장 값진
    /// 규칙을 골라서 버리는 셈이다.
    pub fn usable_for(&self, target: &str) -> bool {
        self.usable_for_mode(target, false)
    }

    /// `strict = true`면 3번(독립 재구성 예외)을 **끄고** "기여했으면 무조건 배제"로
    /// 판정한다. 두 수치를 나란히 보고하기 위한 것이다 — 엄격판은 반박하기 가장
    /// 어려운 하한이고, 기본판은 과학적으로 더 정확하다. 어느 쪽을 썼는지 모른 채
    /// 숫자만 비교하는 일이 없도록 스위치로 남긴다.
    pub fn usable_for_mode(&self, target: &str, strict: bool) -> bool {
        match self.provenance {
            Provenance::HumanDerived => true,
            Provenance::MonadDerived => {
                if self.sources.is_empty() {
                    return false;
                }
                if !strict && self.solo_sources.iter().any(|s| s != target) {
                    return true;
                }
                !self.sources.iter().any(|s| s == target)
            }
        }
    }
}

/// 축적되는 스키마 라이브러리. **디스크에 영속**한다 —
/// 코드가 고정된 채 경험만 쌓여도 다음 실행이 달라지는 것이 목적.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub entries: Vec<Entry>,
    /// 지금 **어느 경험들에서** 배우는 중인가 — [`insert`](Library::insert)가
    /// 이 이름들을 새 항목의 출처로 찍는다. 수면 루프가 과제마다 세팅한다.
    /// 과제를 가로지르는 반일반화는 기여한 이름을 **전부** 넣는다. 영속되지
    /// 않는다(기록은 항목의 `sources`에 남는다).
    pub minting: Vec<String>,
    /// 스키마 표기 → 항목 위치. [`insert`](Library::insert)의 중복 검사를
    /// 선형 탐색에서 상수 시간으로 바꾼다(**의미는 그대로** — 같은 스키마를
    /// 찾아 병합하는 동작이 동일하다). 사다리 수면이 라이브러리 크기의
    /// 제곱으로 느려지던 원인이었다.
    ///
    /// `entries`가 공개 필드라 밖에서 직접 밀어 넣을 수 있으므로, 길이가
    /// 어긋나면 **스스로 다시 만든다**. 어긋난 채로 쓰이는 일이 없다.
    index: HashMap<String, usize>,
    /// 색인을 만들 때의 `entries` 길이. 맵 크기와 비교하지 **않는다** — 파일에
    /// 같은 스키마가 둘 있으면 맵이 항상 더 작아 매번 다시 만들게 되고, 없애려던
    /// 제곱 비용이 그대로 돌아온다.
    index_len: usize,
}

impl Library {
    pub fn new() -> Self {
        Library::default()
    }

    /// **표적 배제 시야**(leave-one-out): 과제 `target`에서 나온 항목만 뺀
    /// 라이브러리. 이것으로 풀린 것은 "다른 경험에서 얻은 구조가 미접촉 과제를
    /// 풀었다"가 정의대로 참이다.
    pub fn view_excluding(&self, target: &str) -> Library {
        self.view_excluding_mode(target, std::env::var("MONAD_PROV_STRICT").is_ok())
    }

    /// 배제 규칙을 명시해 시야를 만든다([`Entry::usable_for_mode`] 참고).
    pub fn view_excluding_mode(&self, target: &str, strict: bool) -> Library {
        Library {
            entries: self
                .entries
                .iter()
                .filter(|e| e.usable_for_mode(target, strict))
                .cloned()
                .collect(),
            minting: Vec::new(),
            // 비워 두면 다음 insert에서 스스로 만들어진다(길이 불일치 감지).
            index: HashMap::new(),
            index_len: 0,
        }
    }

    /// 이 과제가 기여한 항목 수(보고용).
    pub fn sourced_by(&self, task: &str) -> usize {
        self.entries.iter().filter(|e| e.sources.iter().any(|s| s == task)).count()
    }

    /// 이 라이브러리에 기여한 **모든 과제 이름**.
    pub fn source_names(&self) -> std::collections::HashSet<String> {
        self.entries.iter().flat_map(|e| e.sources.iter().cloned()).collect()
    }

    /// 출처가 비어 있는 MONAD 항목이 하나라도 있는가.
    ///
    /// 없다면 **기여한 적 없는 과제에 대해 [`view_excluding`](Library::view_excluding)은
    /// 원본과 완전히 같다** — 그런 과제에서는 복제를 건너뛰어도 결과가 바뀌지
    /// 않는다. 각성 한 번에 라이브러리를 수백 번 복제하던 비용의 대부분이
    /// 여기서 사라진다. 이 조건이 깨지면(옛 파일을 읽는 등) 지름길을 쓰지 않는다.
    pub fn has_unattributed_monad(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.provenance == Provenance::MonadDerived && e.sources.is_empty())
    }

    /// 색인이 `entries`와 어긋났으면 다시 만든다(밖에서 직접 밀어 넣은 경우).
    fn ensure_index(&mut self) {
        if self.index_len == self.entries.len() {
            return;
        }
        self.index.clear();
        for (i, e) in self.entries.iter().enumerate() {
            self.index.insert(write_term(&e.schema), i);
        }
        self.index_len = self.entries.len();
    }

    /// 압축하는 일반화만 받아들인다. 이미 같은 스키마가 있으면 근거만 보강한다.
    pub fn insert(&mut self, abs: &Abstraction, provenance: Provenance) -> bool {
        if abs.gain <= 0 {
            return false;
        }
        self.ensure_index();
        let key = write_term(&abs.schema);
        if let Some(e) = self.index.get(&key).and_then(|&i| self.entries.get_mut(i)) {
            e.support = e.support.saturating_add(abs.instances.len() as u32);
            e.gain = e.gain.max(abs.gain);
            for b in &abs.instances {
                if e.bindings.len() < MAX_KEPT_BINDINGS && !e.bindings.contains(b) {
                    e.bindings.push(b.clone());
                }
            }
            // 같은 스키마를 다시 구성한 경험도 출처다 — 병합 경로에서 놓치면
            // 그 과제가 자기 자신을 푸는 데 쓰이게 된다.
            for src in &self.minting {
                if !e.sources.contains(src) {
                    e.sources.push(src.clone());
                }
            }
            // 한 과제의 증거만으로 같은 스키마가 다시 나왔다면 그 과제는
            // **혼자서** 이것을 만든 것이다 — 전이의 정당성이 여기서 나온다.
            if self.minting.len() == 1 {
                let s = &self.minting[0];
                if !e.solo_sources.contains(s) {
                    e.solo_sources.push(s.clone());
                }
            }
            return false;
        }
        let mut bindings: Vec<HashMap<u32, Term>> = Vec::new();
        for b in &abs.instances {
            if bindings.len() < MAX_KEPT_BINDINGS && !bindings.contains(b) {
                bindings.push(b.clone());
            }
        }
        self.entries.push(Entry {
            schema: abs.schema.clone(),
            provenance,
            gain: abs.gain,
            support: abs.instances.len() as u32,
            tries: 0,
            wins: 0,
            bindings,
            sources: self.minting.clone(),
            solo_sources: if self.minting.len() == 1 { self.minting.clone() } else { Vec::new() },
        });
        self.index.insert(key, self.entries.len() - 1);
        self.index_len = self.entries.len();
        true
    }

    /// 이 대입이 경험에 없던 것인가(신규 재구체화율의 판정자).
    pub fn is_novel(&self, ix: usize, b: &HashMap<u32, Term>) -> bool {
        self.entries
            .get(ix)
            .map(|e| !e.bindings.contains(b))
            .unwrap_or(true)
    }

    /// 변수별로 경험이 본 값들(재구체화 후보의 씨앗).
    pub fn observed(&self, ix: usize, var: u32) -> Vec<Term> {
        let mut out: Vec<Term> = Vec::new();
        if let Some(e) = self.entries.get(ix) {
            for b in &e.bindings {
                if let Some(t) = b.get(&var) {
                    if !out.contains(t) {
                        out.push(t.clone());
                    }
                }
            }
        }
        out
    }

    /// 학습된 사전분포 순서로 스키마를 돌려준다(탐색 감소의 원천).
    pub fn by_prior(&self) -> Vec<usize> {
        let mut ix: Vec<usize> = (0..self.entries.len()).collect();
        ix.sort_by(|&a, &b| {
            self.entries[b]
                .prior()
                .partial_cmp(&self.entries[a].prior())
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        });
        ix
    }

    /// 출처별 개수(회계 의무).
    pub fn count(&self, p: Provenance) -> usize {
        self.entries.iter().filter(|e| e.provenance == p).count()
    }

    /// 재사용률 = 성공 / 시도(전체).
    pub fn reuse_rate(&self) -> f64 {
        let t: u32 = self.entries.iter().map(|e| e.tries).sum();
        let w: u32 = self.entries.iter().map(|e| e.wins).sum();
        if t == 0 {
            0.0
        } else {
            w as f64 / t as f64
        }
    }

    /// 압축률 = 근거 사례 총합 대비 스키마 총 크기(작을수록 잘 압축).
    pub fn compression(&self) -> f64 {
        let s: usize = self.entries.iter().map(|e| e.schema.size()).sum();
        let g: i64 = self.entries.iter().map(|e| e.gain).sum();
        if s == 0 {
            0.0
        } else {
            (s as i64 + g) as f64 / s as f64
        }
    }

    // ---------------------------------------------------------------- 영속

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        use std::io::Write as _;
        let mut s = String::from("MONAD-ABSTRACTION-LIB v1\n");
        for e in &self.entries {
            let binds: Vec<String> = e
                .bindings
                .iter()
                .map(|b| {
                    let mut kv: Vec<(u32, &Term)> = b.iter().map(|(k, v)| (*k, v)).collect();
                    kv.sort_by_key(|x| x.0);
                    kv.iter()
                        .map(|(k, v)| format!("{k}:{}", write_term(v)))
                        .collect::<Vec<_>>()
                        .join(";")
                })
                .collect();
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                write_term(&e.schema),
                match e.provenance {
                    Provenance::HumanDerived => "H",
                    Provenance::MonadDerived => "M",
                },
                e.gain,
                e.support,
                e.tries,
                e.wins,
                binds.join("|"),
                e.sources.join(","),
                e.solo_sources.join(",")
            ));
        }
        let mut f = std::fs::File::create(path)?;
        f.write_all(s.as_bytes())
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Library::new()),
            Err(e) => return Err(e),
        };
        let mut lib = Library::new();
        for line in text.lines().skip(1) {
            let mut it = line.split('\t');
            let (Some(t), Some(p), Some(g), Some(s), Some(tr), Some(w)) =
                (it.next(), it.next(), it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            let Some(schema) = read_term(t) else { continue };
            let mut bindings = Vec::new();
            if let Some(bs) = it.next() {
                for one in bs.split('|').filter(|x| !x.is_empty()) {
                    let mut m = HashMap::new();
                    for kv in one.split(';').filter(|x| !x.is_empty()) {
                        if let Some((k, v)) = kv.split_once(':') {
                            if let (Ok(k), Some(v)) = (k.parse::<u32>(), read_term(v)) {
                                m.insert(k, v);
                            }
                        }
                    }
                    if !m.is_empty() {
                        bindings.push(m);
                    }
                }
            }
            lib.entries.push(Entry {
                schema,
                provenance: if p == "M" {
                    Provenance::MonadDerived
                } else {
                    Provenance::HumanDerived
                },
                gain: g.parse().unwrap_or(0),
                support: s.parse().unwrap_or(0),
                tries: tr.parse().unwrap_or(0),
                wins: w.parse().unwrap_or(0),
                bindings,
                // 8·9열이 없는 옛 파일도 읽힌다. 출처가 비면 MONAD 항목은
                // `usable_for`에서 쓰이지 않으므로, 빠진 열은 누출이 아니라
                // 손실로 끝난다(안전한 방향의 실패).
                sources: it
                    .next()
                    .map(|c| c.split(',').filter(|x| !x.is_empty()).map(str::to_string).collect())
                    .unwrap_or_default(),
                solo_sources: it
                    .next()
                    .map(|c| c.split(',').filter(|x| !x.is_empty()).map(str::to_string).collect())
                    .unwrap_or_default(),
            });
        }
        Ok(lib)
    }
}

/// 항 직렬화: `V<n>` / `C<n>` / `A<f>[t,t,...]`
pub fn write_term(t: &Term) -> String {
    match t {
        Term::Var(i) => format!("V{i}"),
        Term::Const(c) => format!("C{c}"),
        Term::App(f, args) => {
            let inner: Vec<String> = args.iter().map(write_term).collect();
            format!("A{f}[{}]", inner.join(","))
        }
    }
}

/// 직렬화의 역연산.
pub fn read_term(s: &str) -> Option<Term> {
    let (t, rest) = parse_term(s.trim())?;
    if rest.trim().is_empty() {
        Some(t)
    } else {
        None
    }
}

fn parse_term(s: &str) -> Option<(Term, &str)> {
    let b = s.as_bytes();
    match b.first()? {
        b'V' | b'C' => {
            let tag = b[0];
            let end = s[1..]
                .find(|c: char| !c.is_ascii_digit())
                .map(|i| i + 1)
                .unwrap_or(s.len());
            let n: u64 = s[1..end].parse().ok()?;
            let t = if tag == b'V' {
                Term::Var(n as u32)
            } else {
                Term::Const(n)
            };
            Some((t, &s[end..]))
        }
        b'A' => {
            let br = s.find('[')?;
            let f: u32 = s[1..br].parse().ok()?;
            let mut rest = &s[br + 1..];
            let mut args = Vec::new();
            if rest.starts_with(']') {
                return Some((Term::App(f, args), &rest[1..]));
            }
            loop {
                let (t, r) = parse_term(rest)?;
                args.push(t);
                rest = r;
                match rest.as_bytes().first()? {
                    b',' => rest = &rest[1..],
                    b']' => return Some((Term::App(f, args), &rest[1..])),
                    _ => return None,
                }
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(f: u32, args: Vec<Term>) -> Term {
        Term::App(f, args)
    }
    fn c(v: u64) -> Term {
        Term::Const(v)
    }

    /// 같은 자리의 다른 값이 변수가 되고, 같은 값은 남는다.
    #[test]
    fn lgg_finds_the_variable_position() {
        let a = app(1, vec![c(7), c(3)]);
        let b = app(1, vec![c(7), c(5)]);
        let g = lgg(&a, &b);
        assert_eq!(g.term, app(1, vec![c(7), Term::Var(0)]));
        assert_eq!(g.left[&0], c(3));
        assert_eq!(g.right[&0], c(5));
    }

    /// 같은 (좌,우) 쌍이 두 자리에 나오면 **한 변수를 공유**한다 —
    /// "두 자리가 함께 변한다"는 관계의 포착.
    #[test]
    fn lgg_shares_variables_across_positions() {
        let a = app(1, vec![c(3), c(9), c(3)]);
        let b = app(1, vec![c(5), c(9), c(5)]);
        let g = lgg(&a, &b);
        assert_eq!(g.term, app(1, vec![Term::Var(0), c(9), Term::Var(0)]));
        assert_eq!(g.term.vars().len(), 1, "공유 실패 시 변수가 2개가 된다");
    }

    /// 함자가 다르면 통째로 변수(구조가 없다는 정직한 보고).
    #[test]
    fn lgg_of_unrelated_is_a_bare_variable() {
        let g = lgg(&app(1, vec![c(1)]), &app(2, vec![c(1)]));
        assert!(matches!(g.term, Term::Var(_)));
    }

    /// MDL: 구조가 실재하면 압축 이득이 양수, 무관하면 채택되지 않는다.
    #[test]
    fn mdl_accepts_real_structure_rejects_noise() {
        let real = vec![
            app(1, vec![c(7), c(3), c(0)]),
            app(1, vec![c(7), c(5), c(0)]),
            app(1, vec![c(7), c(8), c(0)]),
        ];
        let a = generalize(&real).unwrap();
        assert!(a.gain > 0, "실재 구조인데 이득 {}", a.gain);
        assert_eq!(a.instances.len(), 3);

        let noise = vec![app(1, vec![c(1)]), app(2, vec![c(2)])];
        assert!(generalize(&noise).is_none(), "무관한 항이 스키마가 됐다");
    }

    /// 재구체화: 스키마 + 대입 = 원래 사례를 복원한다(왕복 무손실).
    #[test]
    fn instantiation_round_trips() {
        let terms = vec![
            app(1, vec![c(7), c(3)]),
            app(1, vec![c(7), c(5)]),
        ];
        let a = generalize(&terms).unwrap();
        for (t, b) in terms.iter().zip(a.instances.iter()) {
            assert_eq!(&a.schema.substitute(b), t);
        }
    }

    /// 새 값으로의 재구체화(novel instantiation) — 경험에 없던 사례를 만든다.
    #[test]
    fn novel_instantiation_produces_unseen_terms() {
        let terms = vec![app(1, vec![c(7), c(3)]), app(1, vec![c(7), c(5)])];
        let a = generalize(&terms).unwrap();
        let v = a.schema.vars()[0];
        let mut b = HashMap::new();
        b.insert(v, c(99));
        let novel = a.schema.substitute(&b);
        assert_eq!(novel, app(1, vec![c(7), c(99)]));
        assert!(!terms.contains(&novel), "새 사례가 아니다");
    }

    /// 라이브러리: 압축하는 것만 받고, 사전분포는 성공 이력을 따른다.
    #[test]
    fn library_admits_only_compressing_schemas_and_learns_order() {
        let mut lib = Library::new();
        let good = generalize(&[
            app(1, vec![c(7), c(3), c(0)]),
            app(1, vec![c(7), c(5), c(0)]),
        ])
        .unwrap();
        assert!(lib.insert(&good, Provenance::MonadDerived));
        assert_eq!(lib.count(Provenance::MonadDerived), 1);

        let second = generalize(&[
            app(2, vec![c(1), c(2), c(3), c(4)]),
            app(2, vec![c(1), c(9), c(3), c(4)]),
        ])
        .unwrap();
        lib.insert(&second, Provenance::MonadDerived);
        lib.entries[1].tries = 4;
        lib.entries[1].wins = 3;
        assert_eq!(lib.by_prior()[0], 1, "성공한 스키마가 먼저 와야 한다");
        assert!(lib.reuse_rate() > 0.0);
    }

    /// 영속: 저장→적재 왕복에서 스키마와 이력이 보존된다.
    #[test]
    fn library_persists_across_runs() {
        let dir = std::env::temp_dir().join(format!("monad_abs_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lib.txt");
        let mut lib = Library::new();
        let a = generalize(&[
            app(3, vec![c(7), c(3), app(4, vec![c(1)])]),
            app(3, vec![c(7), c(5), app(4, vec![c(1)])]),
        ])
        .unwrap();
        lib.insert(&a, Provenance::MonadDerived);
        lib.entries[0].tries = 2;
        lib.entries[0].wins = 1;
        lib.save(&path).unwrap();

        let back = Library::load(&path).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].schema, lib.entries[0].schema);
        assert_eq!(back.entries[0].wins, 1);
        assert_eq!(back.entries[0].provenance, Provenance::MonadDerived);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 직렬화 왕복(중첩 구조 포함).
    #[test]
    fn term_serialization_round_trips() {
        let t = app(9, vec![Term::Var(2), c(42), app(0, vec![]), app(1, vec![c(5)])]);
        let s = write_term(&t);
        assert_eq!(read_term(&s).unwrap(), t);
    }

    /// **표적 배제**: 과제 A에서 나온 항목은 A를 풀 때 안 보이고, B를 풀 때 보인다.
    #[test]
    fn view_excluding_hides_only_the_targets_own_entries() {
        let mut lib = Library::new();
        let mk = |n: u64| Abstraction {
            schema: app(1, vec![Term::Var(0), c(n)]),
            instances: vec![HashMap::new()],
            gain: 5,
        };
        lib.minting = vec!["A".into()];
        assert!(lib.insert(&mk(1), Provenance::MonadDerived));
        lib.minting = vec!["B".into()];
        assert!(lib.insert(&mk(2), Provenance::MonadDerived));
        lib.minting.clear();

        assert_eq!(lib.entries.len(), 2);
        assert_eq!(lib.view_excluding("A").entries.len(), 1, "A의 항목이 A에게 보인다");
        assert_eq!(lib.view_excluding("B").entries.len(), 1, "B의 항목이 B에게 보인다");
        assert_eq!(lib.view_excluding("C").entries.len(), 2, "무관한 과제가 손해를 본다");
    }

    /// **독립 재구성은 전이의 근거다.** 같은 스키마를 A와 B가 **각자 혼자서**
    /// 만들었으면 둘 다 `solo_sources`에 들어가고, 그 스키마는 A에게도 B에게도
    /// 보인다 — B 혼자 만든 적이 있으니 A가 없어도 존재했을 것이기 때문이다.
    /// 이것을 구분하지 않고 "기여했으면 무조건 배제"하면, 여러 과제가 독립적으로
    /// 재구성한 **가장 일반적인 규칙**일수록 더 많이 지워진다.
    #[test]
    fn independently_reconstructed_schema_stays_usable_for_its_sources() {
        let mut lib = Library::new();
        let same = Abstraction {
            schema: app(1, vec![Term::Var(0), c(7)]),
            instances: vec![HashMap::new()],
            gain: 5,
        };
        lib.minting = vec!["A".into()];
        assert!(lib.insert(&same, Provenance::MonadDerived));
        lib.minting = vec!["B".into()];
        assert!(!lib.insert(&same, Provenance::MonadDerived), "같은 스키마는 병합된다");
        lib.minting.clear();

        assert_eq!(lib.entries[0].sources, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(lib.entries[0].solo_sources, vec!["A".to_string(), "B".to_string()]);
        assert_eq!(
            lib.view_excluding_mode("A", false).entries.len(),
            1,
            "B가 혼자 만든 것이 A에게 가렸다"
        );
        assert_eq!(
            lib.view_excluding_mode("B", false).entries.len(),
            1,
            "A가 혼자 만든 것이 B에게 가렸다"
        );
        // 엄격판은 같은 항목을 두 기여자 모두에게서 가린다 — 반박하기 가장
        // 어려운 하한. 두 규칙의 차이가 실제로 존재함을 여기서 고정한다.
        assert!(lib.view_excluding_mode("A", true).entries.is_empty(), "엄격판이 안 가렸다");
        assert!(lib.view_excluding_mode("B", true).entries.is_empty(), "엄격판이 안 가렸다");
        assert_eq!(lib.view_excluding_mode("C", true).entries.len(), 1);
    }

    /// **함께 만든 것은 기여자 모두에게서 가려진다.** 두 과제를 접어 만든 스키마는
    /// 어느 쪽도 혼자 만들지 않았으므로 `solo_sources`가 비고, A에게도 B에게도
    /// 보이지 않는다. 여기서 병합 논리를 잘못 적용하면 과제가 자기 자신을 푸는 데
    /// 쓰인다 — 이 세션에서 가장 비쌌던 오류 유형이다.
    #[test]
    fn jointly_built_schema_is_hidden_from_every_contributor() {
        let mut lib = Library::new();
        lib.minting = vec!["A".into(), "B".into()];
        assert!(lib.insert(
            &Abstraction {
                schema: app(1, vec![Term::Var(0), c(9)]),
                instances: vec![HashMap::new()],
                gain: 5,
            },
            Provenance::MonadDerived
        ));
        lib.minting.clear();

        assert!(lib.entries[0].solo_sources.is_empty(), "함께 만든 것이 단독으로 기록됐다");
        assert!(lib.view_excluding("A").entries.is_empty(), "A가 기여했는데 A에게 보인다");
        assert!(lib.view_excluding("B").entries.is_empty(), "B가 기여했는데 B에게 보인다");
        assert_eq!(lib.view_excluding("C").entries.len(), 1, "무관한 과제가 손해를 본다");
    }

    /// 출처는 디스크를 왕복해도 살아남아야 한다 — 배제는 영속된 라이브러리
    /// 위에서 이뤄지므로, 직렬화에서 빠지면 배제가 통째로 무력화된다.
    #[test]
    fn sources_survive_save_and_load() {
        let dir = std::env::temp_dir().join("monad_src_roundtrip");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("lib.tsv");
        let mut lib = Library::new();
        lib.minting = vec!["task-a".into(), "task-b".into()];
        lib.insert(
            &Abstraction {
                schema: app(1, vec![Term::Var(0), c(3)]),
                instances: vec![HashMap::new()],
                gain: 4,
            },
            Provenance::MonadDerived,
        );
        lib.save(&path).unwrap();
        let back = Library::load(&path).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].sources, vec!["task-a".to_string(), "task-b".to_string()]);
        // 두 과제가 **함께** 만든 것이므로 단독 출처는 없다 → 둘 다에게서 가려진다
        assert!(back.entries[0].solo_sources.is_empty());
        assert!(back.view_excluding("task-a").entries.is_empty());
        assert!(back.view_excluding("task-b").entries.is_empty());
        assert_eq!(back.view_excluding("other").entries.len(), 1);
        let _ = std::fs::remove_file(&path);
    }

    /// **출처 없는 MONAD 항목은 아무에게도 안 보인다.** 출처 열이 없던 옛
    /// 라이브러리 파일을 그대로 읽어 쓰면 배제가 조용히 무력화되는데, 그 실패는
    /// 오류를 내지 않아 눈에 띄지 않는다. 사람이 심은 원시어는 영향받지 않는다.
    #[test]
    fn unattributed_monad_entries_are_never_usable() {
        let mut lib = Library::new();
        lib.minting.clear();
        lib.insert(
            &Abstraction {
                schema: app(1, vec![Term::Var(0), c(1)]),
                instances: vec![HashMap::new()],
                gain: 3,
            },
            Provenance::MonadDerived,
        );
        lib.insert(
            &Abstraction {
                schema: app(2, vec![Term::Var(0), c(2)]),
                instances: vec![HashMap::new()],
                gain: 3,
            },
            Provenance::HumanDerived,
        );
        assert_eq!(lib.entries.len(), 2);
        let v = lib.view_excluding("anything");
        assert_eq!(v.entries.len(), 1, "출처 없는 MONAD 항목이 살아남았다");
        assert_eq!(v.entries[0].provenance, Provenance::HumanDerived);
    }

    /// 스키마 색인은 **속도만** 바꾸고 중복 병합 동작은 그대로여야 한다.
    /// `entries`를 밖에서 직접 밀어 넣어 색인을 어긋나게 한 뒤에도 병합이
    /// 옳게 되는지 본다 — 어긋난 색인은 조용히 중복 항목을 만든다.
    #[test]
    fn schema_index_self_heals_and_preserves_dedup() {
        let mut lib = Library::new();
        let a = Abstraction {
            schema: app(1, vec![Term::Var(0), c(11)]),
            instances: vec![HashMap::new()],
            gain: 3,
        };
        lib.minting = vec!["t1".into()];
        assert!(lib.insert(&a, Provenance::MonadDerived));

        // 색인을 모르는 경로로 직접 밀어 넣는다(load가 하는 일과 같다)
        lib.entries.push(Entry {
            schema: app(1, vec![Term::Var(0), c(22)]),
            provenance: Provenance::MonadDerived,
            gain: 3,
            support: 1,
            tries: 0,
            wins: 0,
            bindings: Vec::new(),
            sources: vec!["t2".into()],
            solo_sources: vec!["t2".into()],
        });

        // 직접 밀어 넣은 스키마를 다시 insert하면 **병합**되어야 한다
        let dup = Abstraction {
            schema: app(1, vec![Term::Var(0), c(22)]),
            instances: vec![HashMap::new()],
            gain: 3,
        };
        lib.minting = vec!["t3".into()];
        assert!(!lib.insert(&dup, Provenance::MonadDerived), "중복이 새 항목으로 들어갔다");
        assert_eq!(lib.entries.len(), 2, "색인 불일치가 중복 항목을 만들었다");
        assert_eq!(lib.entries[1].sources, vec!["t2".to_string(), "t3".to_string()]);
    }
}
