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
}

impl Entry {
    /// 탐색 사전분포 점수 — 성공 이력이 있는 스키마를 먼저 시도한다.
    /// (라플라스 평활: 시도 없는 신규 스키마도 기회를 얻는다.)
    pub fn prior(&self) -> f64 {
        let w = (self.wins as f64 + 1.0) / (self.tries as f64 + 2.0);
        w * (1.0 + (self.gain.max(0) as f64).ln_1p())
    }
}

/// 축적되는 스키마 라이브러리. **디스크에 영속**한다 —
/// 코드가 고정된 채 경험만 쌓여도 다음 실행이 달라지는 것이 목적.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub entries: Vec<Entry>,
}

impl Library {
    pub fn new() -> Self {
        Library::default()
    }

    /// 압축하는 일반화만 받아들인다. 이미 같은 스키마가 있으면 근거만 보강한다.
    pub fn insert(&mut self, abs: &Abstraction, provenance: Provenance) -> bool {
        if abs.gain <= 0 {
            return false;
        }
        if let Some(e) = self.entries.iter_mut().find(|e| e.schema == abs.schema) {
            e.support = e.support.saturating_add(abs.instances.len() as u32);
            e.gain = e.gain.max(abs.gain);
            return false;
        }
        self.entries.push(Entry {
            schema: abs.schema.clone(),
            provenance,
            gain: abs.gain,
            support: abs.instances.len() as u32,
            tries: 0,
            wins: 0,
        });
        true
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
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                write_term(&e.schema),
                match e.provenance {
                    Provenance::HumanDerived => "H",
                    Provenance::MonadDerived => "M",
                },
                e.gain,
                e.support,
                e.tries,
                e.wins
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
}
