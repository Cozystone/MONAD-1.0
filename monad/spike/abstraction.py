"""C2-S 스파이크: 구조 추상화 — "어떤 슬롯을 변수화할 것인가".

MONAD 전체의 크리티컬 패스(개발계획 §C2-S). 사건들에서 반복 구조를 찾아
스키마로 만들 때, 어떤 슬롯을 상수로 고정하고 어떤 슬롯을 변수로 열어둘지를
자동으로 결정해야 한다.

## 판정 원리: MDL 압축 이득

스키마는 **결과를 예측하는 데 필요한 비트를 줄일 때만** 채택한다.

    이득 = L(스키마 없이 결과를 기술하는 비용)
         − L(스키마) − L(스키마가 있을 때 결과를 기술하는 비용)

이 하나의 기준이 두 실패를 동시에 막는다:
- **과일반화**(전부 변수화) → 덮은 집합이 불순해져 결과 기술 비용이 오른다.
- **과특수화**(전부 고정) → 스키마 비용만 늘고 덮는 사건이 적어 이득이 없다.

## 탐색: 전방탐색 빔서치

논리곱 규칙(움직임 ∧ 강체)은 **각 조건 단독으로는 이득이 0**이다. 그래서
단순 탐욕법은 가짜 상관 슬롯에 먼저 잡아먹힌다(세계 R3). 한 수 앞을 내다보는
낙관적 점수로 빔을 정렬해 이 근시안을 깬다.

## MONAD 기질로의 이식 경로

발견된 제약 집합은 그대로 스키마 하이퍼벡터가 된다:

    schema = bundle([ bind(role_slot, value_atom) for (slot, value) in 제약 ])

변수화된 슬롯은 번들에 넣지 않는다 — "열린 슬롯"은 물리적으로 '없음'이다.
따라서 이 스파이크가 고르는 제약 집합이 곧 C2에서 만들 SBV 스키마의 내용이다.
A1 용량 곡선(K=16에서 98.7%)이 스키마당 제약 수의 상한을 준다.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

LAMBDA = 0.5  # KT 평활 — 0 확률을 피하고 소표본을 보수적으로 다룬다


# ------------------------------------------------------------------ 데이터셋

@dataclass
class Pattern:
    """스키마 후보: 제약 집합. 제약이 없는 슬롯은 곧 '변수화된 슬롯'이다."""
    constraints: frozenset
    mask: int          # 이 패턴이 덮는 사건의 비트마스크
    effect: str = ""
    gain: float = 0.0
    support: int = 0

    def slots(self) -> set:
        out = set()
        for c in self.constraints:
            if c[0] in ("eq", "ge", "lt"):
                out.add(c[1])
            elif c[0] == "eqslot":
                out.add(c[1])
                out.add(c[2])
        return out

    def describe(self) -> str:
        if not self.constraints:
            return f"(기본) → {self.effect}"
        parts = []
        for c in sorted(self.constraints, key=str):
            if c[0] == "eq":
                parts.append(f"{c[1]}={c[2]}")
            elif c[0] == "eqslot":
                parts.append(f"{c[1]}=={c[2]}")
            elif c[0] == "ge":
                parts.append(f"{c[1]}≥{c[2]:.2f}")
            elif c[0] == "lt":
                parts.append(f"{c[1]}<{c[2]:.2f}")
        return " ∧ ".join(parts) + f" → {self.effect}"

    def matches(self, ev: dict) -> bool:
        for c in self.constraints:
            if c[0] == "eq":
                if ev.get(c[1]) != c[2]:
                    return False
            elif c[0] == "eqslot":
                if ev.get(c[1]) != ev.get(c[2]):
                    return False
            elif c[0] == "ge":
                if not (ev.get(c[1], float("-inf")) >= c[2]):
                    return False
            elif c[0] == "lt":
                if not (ev.get(c[1], float("inf")) < c[2]):
                    return False
        return True


class Dataset:
    """사건 집합 + 후보 제약의 비트마스크 색인.

    비트마스크 덕에 '패턴이 덮는 집합'이 정수 AND 한 번으로 계산된다.
    탐색에서 수만 번 평가하므로 이 표현이 스파이크의 실행 가능성을 좌우한다.
    """

    def __init__(self, events: list, slots: list, max_num_thresholds: int = 24):
        self.events = events
        self.slots = slots
        self.n = len(events)
        self.all_mask = (1 << self.n) - 1

        self.effects = sorted({e["effect"] for e in events})
        self.effect_mask = {}
        for eff in self.effects:
            m = 0
            for i, ev in enumerate(events):
                if ev["effect"] == eff:
                    m |= 1 << i
            self.effect_mask[eff] = m

        self.cand: list = []
        self.mask: dict = {}
        self._build_candidates(max_num_thresholds)
        self.vocab_bits = math.log2(len(self.cand) + 1) if self.cand else 1.0

    # -- 후보 제약 생성 ----------------------------------------------------

    def _add(self, c, m: int):
        if m == 0 or m == self.all_mask:
            return  # 아무것도 못 가르는 제약은 후보가 아니다
        if c in self.mask:
            return
        self.mask[c] = m
        self.cand.append(c)

    def _build_candidates(self, max_num_thresholds: int):
        from world import BOOL, CAT, NUM

        by_name = {s.name: s for s in self.slots}

        # 1) 값 고정: slot == value
        for s in self.slots:
            if s.kind in (CAT, BOOL):
                buckets: dict = {}
                for i, ev in enumerate(self.events):
                    buckets.setdefault(ev[s.name], 0)
                    buckets[ev[s.name]] |= 1 << i
                for v, m in buckets.items():
                    self._add(("eq", s.name, v), m)

        # 2) 관계: slot_a == slot_b (같은 종류·같은 도메인끼리만)
        names = [s.name for s in self.slots]
        for i in range(len(names)):
            for j in range(i + 1, len(names)):
                a, b = by_name[names[i]], by_name[names[j]]
                if a.kind != b.kind or a.kind == NUM:
                    continue
                if set(a.domain) != set(b.domain):
                    continue
                m = 0
                for k, ev in enumerate(self.events):
                    if ev[a.name] == ev[b.name]:
                        m |= 1 << k
                self._add(("eqslot", a.name, b.name), m)

        # 3) 수치 임계: 결과가 바뀌는 경계의 중점만 후보로 삼는다
        #    (결정트리의 표준 기법 — 무의미한 임계를 폭발적으로 만들지 않는다)
        for s in self.slots:
            if s.kind != NUM:
                continue
            pairs = sorted((ev[s.name], ev["effect"]) for ev in self.events)
            thr = []
            for k in range(1, len(pairs)):
                if pairs[k][1] != pairs[k - 1][1] and pairs[k][0] != pairs[k - 1][0]:
                    thr.append((pairs[k][0] + pairs[k - 1][0]) / 2.0)
            if not thr:
                continue
            # 균등 간격으로 솎아낸다
            if len(thr) > max_num_thresholds:
                step = len(thr) / max_num_thresholds
                thr = [thr[int(i * step)] for i in range(max_num_thresholds)]
            for t in sorted(set(round(x, 3) for x in thr)):
                mge = 0
                for k, ev in enumerate(self.events):
                    if ev[s.name] >= t:
                        mge |= 1 << k
                self._add(("ge", s.name, t), mge)
                self._add(("lt", s.name, t), self.all_mask & ~mge)

    # -- MDL ---------------------------------------------------------------

    def code_len(self, mask: int) -> float:
        """마스크가 가리키는 사건들의 결과를 기술하는 비용(비트)."""
        n = mask.bit_count()
        if n == 0:
            return 0.0
        k = len(self.effects)
        denom = n + LAMBDA * k
        total = 0.0
        for eff in self.effects:
            c = (mask & self.effect_mask[eff]).bit_count()
            if c:
                total -= c * math.log2((c + LAMBDA) / denom)
        return total

    def pattern_cost(self, k_constraints: int) -> float:
        """스키마 자체를 적는 비용. 제약 하나당 어휘 크기만큼의 비트."""
        return (k_constraints + 1) * self.vocab_bits

    def gain_of(self, active: int, covered: int, k_constraints: int) -> float:
        """active 범위 안에서 covered를 떼어냈을 때의 압축 이득."""
        n_cov = covered.bit_count()
        if n_cov == 0:
            return float("-inf")
        rest = active & ~covered
        after = self.code_len(covered) + self.code_len(rest) + self.pattern_cost(k_constraints)
        return self.code_len(active) - after

    def majority(self, mask: int) -> tuple:
        best, best_c = self.effects[0], -1
        for eff in self.effects:
            c = (mask & self.effect_mask[eff]).bit_count()
            if c > best_c:
                best, best_c = eff, c
        n = mask.bit_count()
        return best, (best_c / n if n else 0.0)


# ------------------------------------------------------------------ 탐색기

def _refine(ds: Dataset, base: frozenset, base_mask: int):
    """패턴에 제약 하나를 더한 모든 후보를 생성."""
    used = set()
    for c in base:
        if c[0] in ("eq", "ge", "lt"):
            used.add(c[1])
        elif c[0] == "eqslot":
            used.add(c[1])
            used.add(c[2])
    for c in ds.cand:
        if c in base:
            continue
        # 같은 슬롯을 값 고정으로 두 번 제약하지 않는다(모순/중복 방지)
        if c[0] == "eq" and c[1] in used:
            continue
        m = base_mask & ds.mask[c]
        if m == 0 or m == base_mask:
            continue
        yield c, m


def beam_search(
    ds: Dataset,
    active: int,
    beam_width: int = 10,
    max_depth: int = 4,
    lookahead: bool = True,
) -> Pattern | None:
    """가장 큰 압축 이득을 주는 패턴 하나를 찾는다.

    lookahead=True면 '한 수 뒤의 최대 이득'으로 빔을 정렬한다. 논리곱 규칙처럼
    단독으로는 이득이 0인 조건을 살려두기 위한 장치이며, 이것이 있고 없고가
    가짜 상관 세계(R3)의 성패를 가른다.
    """
    best: Pattern | None = None
    beam = [(frozenset(), active)]
    seen = {frozenset()}

    for _depth in range(max_depth):
        scored = []
        for base, bmask in beam:
            for c, m in _refine(ds, base, bmask):
                nxt = base | {c}
                if nxt in seen:
                    continue
                g = ds.gain_of(active, m, len(nxt))
                if best is None or g > best.gain:
                    eff, _ = ds.majority(m)
                    best = Pattern(nxt, m, eff, g, m.bit_count())
                # 빔 정렬 점수: 낙관적(한 수 앞) 이득
                score = g
                if lookahead:
                    for _c2, m2 in _refine(ds, nxt, m):
                        g2 = ds.gain_of(active, m2, len(nxt) + 1)
                        if g2 > score:
                            score = g2
                scored.append((score, nxt, m))

        if not scored:
            break
        scored.sort(key=lambda t: -t[0])
        beam = []
        for _s, nxt, m in scored[:beam_width]:
            seen.add(nxt)
            beam.append((nxt, m))

    return best


def lgg_search(ds: Dataset, active: int) -> Pattern | None:
    """대조군: 순수 anti-unification (least general generalization).

    결과별로 양성 사건을 모아 '모두가 공유하는 값'만 남긴다. 음성 사건을
    보지 않으므로 무관 슬롯이 우연히 일치하면 그대로 제약으로 남고(과특수화),
    조건을 놓치면 음성까지 덮는다(과일반화). MDL 없이 이것만으로 되는지 확인용.
    """
    best: Pattern | None = None
    for eff in ds.effects:
        pos = active & ds.effect_mask[eff]
        if pos == 0:
            continue
        idxs = [i for i in range(ds.n) if (pos >> i) & 1]
        cons = set()
        for c in ds.cand:
            if c[0] == "lt" or c[0] == "ge":
                continue  # 수치 임계는 LGG의 표현 범위 밖
            m = ds.mask[c]
            if all((m >> i) & 1 for i in idxs):
                cons.add(c)
        # 값 고정이 있으면 같은 슬롯의 관계 제약은 중복 — 값 고정을 우선
        cons = frozenset(cons)
        mask = active
        for c in cons:
            mask &= ds.mask[c]
        g = ds.gain_of(active, mask, len(cons))
        if best is None or g > best.gain:
            best = Pattern(cons, mask, eff, g, mask.bit_count())
    return best


# -------------------------------------------------------- 순차 피복(규칙 집합)

@dataclass
class RuleSet:
    rules: list                 # list[Pattern], 구체적인 것이 앞
    default: str
    method: str

    def predict(self, ev: dict) -> str:
        for r in self.rules:
            if r.matches(ev):
                return r.effect
        return self.default

    def slots(self) -> set:
        out = set()
        for r in self.rules:
            out |= r.slots()
        return out

    def describe(self) -> list:
        return [r.describe() for r in self.rules] + [f"(그 외) → {self.default}"]


def _exceptions(
    ds: Dataset,
    parent: Pattern,
    method: str,
    min_support: int,
    depth: int,
) -> list:
    """규칙이 덮은 범위 안에서 **체계적으로 틀리는** 사건을 찾아 예외 규칙으로 만든다.

    "움직이는 물체가 강체에 부딪히면 반사된다 — 단, 유리는 깨진다."

    반례를 잡음으로 흘려보내지 않고 더 구체적인 스키마로 분화시키는 이 동작이
    곧 C4(스키마 개정)의 핵심이다. 순차 피복만으로는 부모 규칙이 예외 사례까지
    먹어치우고 사라지므로, 덮은 범위 **안에서** 다시 압축을 시도해야 한다.
    """
    if depth <= 0:
        return []
    errs = parent.mask & ~ds.effect_mask[parent.effect]
    if errs.bit_count() < min_support:
        return []

    sub = (
        beam_search(ds, parent.mask, beam_width=1, lookahead=False)
        if method == "mdl-greedy"
        else beam_search(ds, parent.mask)
    )
    if sub is None or sub.gain <= 0 or not sub.constraints:
        return []

    eff, purity = ds.majority(sub.mask)
    if eff == parent.effect or purity < 0.5:
        return []

    # 부모의 제약을 물려받아 반드시 더 구체적이게 만든다 → 적용 순서에서 앞선다
    child = Pattern(
        parent.constraints | sub.constraints,
        sub.mask,
        eff,
        sub.gain,
        sub.mask.bit_count(),
    )
    return [child] + _exceptions(ds, child, method, min_support, depth - 1)


def induce(
    ds: Dataset,
    method: str = "mdl-beam",
    min_support: int = 8,
    max_rules: int = 8,
    exception_depth: int = 2,
) -> RuleSet:
    """순차 피복 + 예외 정제.

    규칙을 하나 찾고 → 그 안의 체계적 반례를 예외 규칙으로 분화시키고 →
    덮은 사건을 빼고 → 다시 찾는다.
    """
    active = ds.all_mask
    rules: list = []

    for _ in range(max_rules):
        if active.bit_count() < min_support:
            break
        if method == "lgg":
            p = lgg_search(ds, active)
        elif method == "mdl-greedy":
            p = beam_search(ds, active, beam_width=1, lookahead=False)
        else:
            p = beam_search(ds, active)

        if p is None or p.gain <= 0 or not p.constraints:
            break
        # 덮은 집합의 다수 결과를 규칙의 결론으로 삼는다
        eff, purity = ds.majority(p.mask)
        p.effect = eff
        rules.append(p)
        rules.extend(_exceptions(ds, p, method, min_support, exception_depth))
        active &= ~p.mask
        if purity < 0.5:
            break

    default = ds.majority(active)[0] if active else ds.majority(ds.all_mask)[0]
    # 더 구체적인 규칙(제약이 많은 것)이 먼저 적용되도록 정렬 — 예외 처리의 핵심
    rules.sort(key=lambda r: -len(r.constraints))
    return RuleSet(rules, default, method)
