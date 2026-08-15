//! 표현 가설 — **무엇을 객체로 볼 것인가도 가설이다.**
//!
//! 지금까지 MONAD는 *주어진* 표현 위에서 스키마를 일반화했다. 표현 자체는
//! 사람이 골랐다(4-연결이냐 8-연결이냐, 배경이 0이냐 최빈색이냐…). 그 선택이
//! 사람 손에 있는 한, 시스템이 아무리 잘 일반화해도 **가장 중요한 결정은 사람이
//! 한 것**이다.
//!
//! 이 모듈은 그 결정을 시스템에게 돌려준다:
//!
//! ```text
//! 관측 → 표현 후보들 → 각 표현에서 스키마 귀납 → MDL·증거로 경쟁 → 승자 유지
//! ```
//!
//! # 점수
//!
//! ```text
//! score = 예측 적합 + 압축 이득 + 스키마 재사용 이득 − 복잡도 비용
//! ```
//!
//! 넷 다 필요하다. 적합만 보면 과적합 표현이 이기고, 압축만 보면 아무것도
//! 설명하지 않는 뭉갬이 이기고, 재사용만 보면 과거에 갇히고, 복잡도를 빼지
//! 않으면 장황한 표현이 이긴다.
//!
//! # 교리
//!
//! - **도메인 중립**: 여기에 격자도 색도 없다. 관측을 항으로 바꾸는 일은 도메인이
//!   하고, 어느 표현이 나은지 고르는 일은 여기서 한다.
//! - **표현도 축적된다**: 어떤 표현이 자주 이겼는지가 [`RepLibrary`]에 남아
//!   다음 문제의 사전분포가 된다(표현 수준의 학습).
//! - **출처를 남긴다**: 사람이 심은 분해기와 시스템이 발견한 분해기를 섞어 세지
//!   않는다.

use crate::abstraction::{generalize, Library, Provenance, Term};

/// 표현 하나의 점수 구성 — 왜 이겼는지 사람이 읽을 수 있어야 한다.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RepScore {
    /// 예측 적합도(도메인이 잰다): 이 표현 위에서 관측이 얼마나 설명되는가 [0,1].
    pub fit: f64,
    /// 압축 이득: 이 표현의 항들이 공통 구조로 얼마나 줄어드는가(MDL).
    pub compression: f64,
    /// 재사용 이득: 이미 아는 스키마가 이 표현의 항을 얼마나 덮는가.
    pub reuse: f64,
    /// 복잡도 비용: 이 표현의 서술 길이.
    pub complexity: f64,
}

impl RepScore {
    pub fn total(&self) -> f64 {
        self.fit + self.compression + self.reuse - self.complexity
    }
}

/// 경쟁에 나온 표현 후보 하나.
#[derive(Clone, Debug)]
pub struct RepCandidate {
    pub id: u32,
    pub name: String,
    pub provenance: Provenance,
    /// 이 표현으로 인코딩된 관측들(항).
    pub terms: Vec<Term>,
    /// 도메인이 잰 예측 적합도 [0,1] — 이 표현에서 규칙이 관측을 재현하는 비율.
    pub fit: f64,
}

/// 후보 하나를 채점한다. 압축·재사용·복잡도는 여기서 계산하고, 적합만 받는다.
pub fn score(cand: &RepCandidate, lib: &Library) -> RepScore {
    let total_size: usize = cand.terms.iter().map(|t| t.size()).sum();
    let n = cand.terms.len().max(1) as f64;

    // 압축: **절대 이득이 아니라 비율**(아낀 서술 / 원래 서술).
    // 절대값으로 재면 자리만 많은 장황한 표현이 이긴다 — 패딩도 잘 압축되기
    // 때문이다. MDL의 의미는 "몇 비트를 아꼈나"가 아니라 "얼마나 짧아졌나"다.
    let compression = generalize(&cand.terms)
        .map(|a| a.gain as f64 / total_size.max(1) as f64)
        .unwrap_or(0.0);

    // 재사용: 라이브러리의 기존 스키마가 이 표현의 항을 덮는 비율
    let covered = cand
        .terms
        .iter()
        .filter(|t| lib.entries.iter().any(|e| e.schema.matches(t).is_some()))
        .count() as f64;
    let reuse = covered / n;

    // 복잡도: 항당 평균 서술 길이(로그 스케일 — 크기 차이가 지수적으로 벌어지지
    // 않도록; 표현 간 비교는 규모가 아니라 자릿수의 문제다)
    let complexity = ((total_size as f64 / n) + 1.0).ln();

    RepScore { fit: cand.fit, compression, reuse, complexity }
}

/// 후보들을 채점해 승자를 고른다. **사람이 고르지 않는다.**
/// 동점이면 ①단순한 표현(복잡도 낮음) ②먼저 등록된 순.
pub fn select<'a>(
    cands: &'a [RepCandidate],
    lib: &Library,
    reps: &RepLibrary,
) -> Option<(&'a RepCandidate, RepScore)> {
    let mut best: Option<(&RepCandidate, RepScore, f64)> = None;
    for c in cands {
        let s = score(c, lib);
        // 표현 수준의 학습된 사전분포를 소액 가산(과거에 통한 표현을 먼저 본다)
        let total = s.total() + 0.25 * reps.prior(c.id);
        let better = match &best {
            None => true,
            Some((_, bs, bt)) => {
                total > *bt || ((total - *bt).abs() < 1e-9 && s.complexity < bs.complexity)
            }
        };
        if better {
            best = Some((c, s, total));
        }
    }
    best.map(|(c, s, _)| (c, s))
}

/// 표현 하나의 이력(선택 횟수·성공 횟수) — 표현 수준의 학습.
#[derive(Clone, Debug)]
pub struct RepStats {
    pub id: u32,
    pub name: String,
    pub provenance: Provenance,
    /// 경쟁에서 선택된 횟수.
    pub chosen: u32,
    /// 선택된 뒤 실제로 문제를 푼 횟수.
    pub wins: u32,
}

/// 표현 이력의 축적소. 디스크에 남아 **다음 실행의 표현 사전분포**가 된다.
#[derive(Clone, Debug, Default)]
pub struct RepLibrary {
    pub stats: Vec<RepStats>,
}

impl RepLibrary {
    pub fn new() -> Self {
        RepLibrary::default()
    }

    fn slot(&mut self, id: u32, name: &str, p: Provenance) -> &mut RepStats {
        if let Some(i) = self.stats.iter().position(|s| s.id == id) {
            return &mut self.stats[i];
        }
        self.stats.push(RepStats {
            id,
            name: name.to_string(),
            provenance: p,
            chosen: 0,
            wins: 0,
        });
        self.stats.last_mut().unwrap()
    }

    pub fn note_choice(&mut self, c: &RepCandidate) {
        let s = self.slot(c.id, &c.name, c.provenance);
        s.chosen = s.chosen.saturating_add(1);
    }

    pub fn note_win(&mut self, id: u32) {
        if let Some(s) = self.stats.iter_mut().find(|s| s.id == id) {
            s.wins = s.wins.saturating_add(1);
        }
    }

    /// 이 표현이 과거에 통했던 정도 [0,1) — 라플라스 평활(신규도 기회를 받는다).
    pub fn prior(&self, id: u32) -> f64 {
        match self.stats.iter().find(|s| s.id == id) {
            Some(s) => (s.wins as f64 + 1.0) / (s.chosen as f64 + 2.0),
            None => 0.5,
        }
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        use std::io::Write as _;
        let mut s = String::from("MONAD-REPRESENTATION-LIB v1\n");
        for e in &self.stats {
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                e.id,
                e.name,
                match e.provenance {
                    Provenance::HumanDerived => "H",
                    Provenance::MonadDerived => "M",
                },
                e.chosen,
                e.wins
            ));
        }
        std::fs::File::create(path)?.write_all(s.as_bytes())
    }

    pub fn load(path: impl AsRef<std::path::Path>) -> std::io::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RepLibrary::new()),
            Err(e) => return Err(e),
        };
        let mut lib = RepLibrary::new();
        for line in text.lines().skip(1) {
            let mut it = line.split('\t');
            let (Some(id), Some(name), Some(p), Some(ch), Some(w)) =
                (it.next(), it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            lib.stats.push(RepStats {
                id: id.parse().unwrap_or(0),
                name: name.to_string(),
                provenance: if p == "M" {
                    Provenance::MonadDerived
                } else {
                    Provenance::HumanDerived
                },
                chosen: ch.parse().unwrap_or(0),
                wins: w.parse().unwrap_or(0),
            });
        }
        Ok(lib)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abstraction::Term;

    fn app(f: u32, args: Vec<Term>) -> Term {
        Term::App(f, args)
    }
    fn c(v: u64) -> Term {
        Term::Const(v)
    }

    fn cand(id: u32, name: &str, terms: Vec<Term>, fit: f64) -> RepCandidate {
        RepCandidate {
            id,
            name: name.into(),
            provenance: Provenance::HumanDerived,
            terms,
            fit,
        }
    }

    /// 같은 관측을 두 가지로 인코딩했을 때, **공통 구조가 드러나는 표현**이 이긴다.
    /// 사람이 고르지 않는다 — 압축이 고른다.
    #[test]
    fn representation_with_shared_structure_wins() {
        let lib = Library::new();
        let reps = RepLibrary::new();
        // A: 같은 함자·같은 자리 — 구조가 보인다
        let a = cand(
            1,
            "structured",
            vec![
                app(1, vec![c(7), c(3), c(0)]),
                app(1, vec![c(7), c(5), c(0)]),
                app(1, vec![c(7), c(8), c(0)]),
            ],
            0.9,
        );
        // B: 함자가 제각각 — 같은 관측이지만 구조가 안 보이는 인코딩
        let b = cand(
            2,
            "scrambled",
            vec![
                app(1, vec![c(7), c(3), c(0)]),
                app(2, vec![c(7), c(5), c(0)]),
                app(3, vec![c(7), c(8), c(0)]),
            ],
            0.9,
        );
        let cands = [a, b];
        let (win, s) = select(&cands, &lib, &reps).unwrap();
        assert_eq!(win.id, 1, "구조가 보이는 표현이 져서는 안 된다");
        assert!(s.compression > 0.0);
    }

    /// 적합도가 같고 압축도 같다면 **덜 장황한 표현**이 이긴다(복잡도 비용).
    #[test]
    fn verbose_representation_pays_a_complexity_cost() {
        let lib = Library::new();
        let reps = RepLibrary::new();
        let lean = cand(1, "lean", vec![app(1, vec![c(1)]), app(1, vec![c(2)])], 0.8);
        let bloated = cand(
            2,
            "bloated",
            vec![
                app(1, vec![c(1), c(0), c(0), c(0), c(0), c(0)]),
                app(1, vec![c(2), c(0), c(0), c(0), c(0), c(0)]),
            ],
            0.8,
        );
        let cands = [bloated, lean];
        let (win, _) = select(&cands, &lib, &reps).unwrap();
        assert_eq!(win.id, 1, "장황한 표현이 이겼다 — 복잡도 비용이 없다");
    }

    /// **아는 스키마가 덮는 표현**이 가산점을 받는다(재사용 이득).
    #[test]
    fn known_schemas_pull_the_choice_toward_reusable_representation() {
        let reps = RepLibrary::new();
        let mut lib = Library::new();
        // 라이브러리는 f9(?0) 꼴을 안다
        let known = generalize(&[app(9, vec![c(1), c(0)]), app(9, vec![c(2), c(0)])]).unwrap();
        lib.insert(&known, Provenance::MonadDerived);

        let familiar = cand(
            1,
            "familiar",
            vec![app(9, vec![c(4), c(0)]), app(9, vec![c(5), c(0)])],
            0.5,
        );
        let alien = cand(
            2,
            "alien",
            vec![app(8, vec![c(4), c(0)]), app(8, vec![c(5), c(0)])],
            0.5,
        );
        let s_fam = score(&familiar, &lib);
        let s_ali = score(&alien, &lib);
        assert!(s_fam.reuse > s_ali.reuse, "재사용 이득이 반영되지 않았다");
        let cands = [alien, familiar];
        let (win, _) = select(&cands, &lib, &reps).unwrap();
        assert_eq!(win.id, 1);
    }

    /// 적합도가 압도적이면 압축이 낮아도 이긴다 — 네 항의 균형.
    #[test]
    fn predictive_fit_can_outweigh_compression() {
        let lib = Library::new();
        let reps = RepLibrary::new();
        let fits = cand(1, "fits", vec![app(1, vec![c(1)]), app(2, vec![c(9)])], 1.0);
        let compresses = cand(
            2,
            "compresses",
            vec![app(3, vec![c(1), c(1)]), app(3, vec![c(2), c(1)])],
            0.0,
        );
        let cands = [compresses, fits];
        let (win, _) = select(&cands, &lib, &reps).unwrap();
        assert_eq!(win.id, 1, "예측을 못 하는 표현이 압축만으로 이겼다");
    }

    /// 표현 수준의 학습: 과거에 통한 표현이 다음 경쟁에서 먼저 선택된다.
    #[test]
    fn representation_prior_is_learned_and_persists() {
        let lib = Library::new();
        let mut reps = RepLibrary::new();
        let a = cand(1, "A", vec![app(1, vec![c(1)]), app(1, vec![c(2)])], 0.5);
        let b = cand(2, "B", vec![app(2, vec![c(1)]), app(2, vec![c(2)])], 0.5);

        // 동률 상태에서는 먼저 등록된 쪽
        let first = [a.clone(), b.clone()];
        let (w0, _) = select(&first, &lib, &reps).unwrap();
        assert_eq!(w0.id, 1);

        // B가 여러 번 성공하면 사전분포가 B를 밀어올린다
        for _ in 0..5 {
            reps.note_choice(&b);
            reps.note_win(2);
        }
        reps.note_choice(&a);
        let second = [a, b];
        let (w1, _) = select(&second, &lib, &reps).unwrap();
        assert_eq!(w1.id, 2, "표현 수준 학습이 선택을 못 바꿨다");

        // 영속 왕복
        let dir = std::env::temp_dir().join(format!("monad_rep_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("reps.tsv");
        reps.save(&path).unwrap();
        let back = RepLibrary::load(&path).unwrap();
        assert_eq!(back.stats.len(), reps.stats.len());
        assert!((back.prior(2) - reps.prior(2)).abs() < 1e-9);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
