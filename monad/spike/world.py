"""C2-S 스파이크: 정답을 아는 합성 규칙 세계.

구조 추상화 알고리즘을 시험하려면 "무엇을 변수화해야 정답인지"를 아는 환경이
필요하다. 이 모듈은 규칙(정답 스키마)을 명시적으로 정의하고, 그 규칙이 생성한
사건들만 알고리즘에 넘긴다. 알고리즘이 규칙을 되찾아내는지가 유일한 평가 기준.

핵심 난제(개발계획 C2-S): 사건
    빨간 공(반지름5, 속도3) + 회색 벽(강체) → 반사
    파란 정육면체(반지름2, 속도9) + 갈색 벽(강체) → 반사
에서 "색·모양·크기·속도는 무관하고 강체 여부만 중요하다"를 스스로 알아내야 한다.
색이 정말 중요한 세계(R4)도 섞어두었으므로 "전부 변수화"라는 값싼 답은 실패한다.
"""

from __future__ import annotations

import random
from dataclasses import dataclass, field
from typing import Any, Callable

# ---------------------------------------------------------------- 슬롯 정의

CAT = "cat"  # 범주형
NUM = "num"  # 수치형
BOOL = "bool"


@dataclass(frozen=True)
class Slot:
    name: str
    kind: str
    domain: tuple = ()      # CAT/BOOL용
    lo: float = 0.0         # NUM용
    hi: float = 1.0


# 공통 어휘 — 여러 규칙 세계가 공유한다(같은 지각이 다른 규칙을 만나는 상황을 재현).
COLORS = ("red", "blue", "green", "gray", "brown", "white")
SHAPES = ("round", "square", "triangle", "star", "hex")
MATERIALS = ("rubber", "wood", "metal", "glass", "stone")

BASE_SLOTS = [
    Slot("obj.color", CAT, COLORS),
    Slot("obj.shape", CAT, SHAPES),
    Slot("obj.material", CAT, MATERIALS),
    Slot("obj.size", NUM, lo=1.0, hi=10.0),
    Slot("obj.speed", NUM, lo=0.0, hi=12.0),
    Slot("obj.moving", BOOL, (False, True)),
    Slot("surf.color", CAT, COLORS),
    Slot("surf.material", CAT, MATERIALS),
    Slot("surf.rigid", BOOL, (False, True)),
]


# ------------------------------------------------------------ 정답 스키마 표현

# 제약 술어. 알고리즘이 발견해야 할 목표 형태와 동일한 어휘를 쓴다.
#   ("eq",     slot, value)   슬롯이 특정 값
#   ("eqslot", s1,   s2)      두 슬롯이 서로 같음 (관계적 추상화)
#   ("ge",     slot, thr)     수치 슬롯이 임계 이상
#   ("lt",     slot, thr)     수치 슬롯이 임계 미만
Constraint = tuple


@dataclass
class GroundTruthRule:
    """정답 규칙 하나. 제약 집합 → 결과."""
    constraints: frozenset
    effect: str

    def describe(self) -> str:
        if not self.constraints:
            return f"(항상) → {self.effect}"
        parts = []
        for c in sorted(self.constraints):
            if c[0] == "eq":
                parts.append(f"{c[1]}={c[2]}")
            elif c[0] == "eqslot":
                parts.append(f"{c[1]}=={c[2]}")
            elif c[0] == "ge":
                parts.append(f"{c[1]}≥{c[2]:g}")
            elif c[0] == "lt":
                parts.append(f"{c[1]}<{c[2]:g}")
        return " ∧ ".join(parts) + f" → {self.effect}"


@dataclass
class RuleWorld:
    key: str
    title: str
    slots: list
    effect_fn: Callable[[dict], str]
    truth: list                      # list[GroundTruthRule]
    note: str = ""
    noise: float = 0.0
    effects: tuple = ()
    extra: dict = field(default_factory=dict)

    def sample_event(self, rng: random.Random) -> dict:
        ev = {}
        for s in self.slots:
            if s.kind == NUM:
                ev[s.name] = round(rng.uniform(s.lo, s.hi), 2)
            else:
                ev[s.name] = rng.choice(s.domain)
        # 세계별 후처리(상관 잡음 슬롯 주입 등)
        hook = self.extra.get("post")
        if hook:
            hook(ev, rng)
        ev["effect"] = self.effect_fn(ev)
        if self.noise > 0 and rng.random() < self.noise:
            others = [e for e in self.effects if e != ev["effect"]]
            if others:
                ev["effect"] = rng.choice(others)
        return ev

    def sample(self, n: int, seed: int) -> list:
        rng = random.Random(seed)
        return [self.sample_event(rng) for _ in range(n)]

    def slot_names(self) -> list:
        return [s.name for s in self.slots]


def _slots_with(extra: list) -> list:
    return list(BASE_SLOTS) + extra


# ------------------------------------------------------------------ 규칙 세계

def _r1() -> RuleWorld:
    """단일 조건. 무관 슬롯 8개를 전부 변수화할 수 있는가."""
    def fn(e):
        return "reverse" if e["surf.rigid"] else "pass"

    return RuleWorld(
        key="R1",
        title="단일 조건 (무관 슬롯 8개)",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("reverse", "pass"),
        truth=[
            GroundTruthRule(frozenset({("eq", "surf.rigid", True)}), "reverse"),
            GroundTruthRule(frozenset({("eq", "surf.rigid", False)}), "pass"),
        ],
        note="색·모양·재질·크기·속도는 전부 무관. 가장 기본적인 일반화.",
    )


def _r2() -> RuleWorld:
    """논리곱. 두 조건이 함께여야 한다는 것을 발견할 수 있는가."""
    def fn(e):
        return "reverse" if (e["obj.moving"] and e["surf.rigid"]) else "nothing"

    return RuleWorld(
        key="R2",
        title="논리곱 조건 (움직임 ∧ 강체)",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("reverse", "nothing"),
        truth=[
            GroundTruthRule(
                frozenset({("eq", "obj.moving", True), ("eq", "surf.rigid", True)}),
                "reverse",
            ),
        ],
        note="한 조건만 보면 예측이 안 된다 — 조합을 찾아야 한다.",
    )


def _r3() -> RuleWorld:
    """가짜 상관. 결과와 85% 일치하는 무관 슬롯의 유혹을 견디는가."""
    spur = Slot("obj.tag", CAT, ("t0", "t1"))
    noise_slots = [Slot(f"noise{i}", CAT, ("a", "b", "c", "d")) for i in range(8)]

    def fn(e):
        return "reverse" if (e["obj.moving"] and e["surf.rigid"]) else "nothing"

    def post(e, rng):
        # tag를 정답과 85% 상관시킨다 — 인과가 아니라 상관.
        want = e["obj.moving"] and e["surf.rigid"]
        if rng.random() < 0.85:
            e["obj.tag"] = "t1" if want else "t0"
        else:
            e["obj.tag"] = "t0" if want else "t1"

    return RuleWorld(
        key="R3",
        title="가짜 상관 + 잡음 슬롯 9개",
        slots=_slots_with([spur] + noise_slots),
        effect_fn=fn,
        effects=("reverse", "nothing"),
        truth=[
            GroundTruthRule(
                frozenset({("eq", "obj.moving", True), ("eq", "surf.rigid", True)}),
                "reverse",
            ),
        ],
        note="obj.tag는 결과와 85% 일치하지만 원인이 아니다. 압축 이득이 이를 걸러야 한다.",
        extra={"post": post},
    )


def _r4() -> RuleWorld:
    """관계적 추상화. '값'이 아니라 '두 슬롯이 같음'을 발견해야 한다."""
    def fn(e):
        return "camouflage" if e["obj.color"] == e["surf.color"] else "reverse"

    return RuleWorld(
        key="R4",
        title="관계 조건 (물체색 == 표면색)",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("camouflage", "reverse"),
        truth=[
            GroundTruthRule(frozenset({("eqslot", "obj.color", "surf.color")}), "camouflage"),
        ],
        note="어떤 개별 색도 답이 아니다. 슬롯 간 동일성이라는 관계가 답이다.",
    )


def _r5() -> RuleWorld:
    """수치 임계. 연속량을 이산 술어로 바꿔야 한다(v0.2 인지 원자의 value 필드)."""
    def fn(e):
        return "shatter" if e["obj.speed"] >= 7.0 else "reverse"

    return RuleWorld(
        key="R5",
        title="수치 임계 (속도 ≥ 7.0)",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("shatter", "reverse"),
        truth=[
            GroundTruthRule(frozenset({("ge", "obj.speed", 7.0)}), "shatter"),
            GroundTruthRule(frozenset({("lt", "obj.speed", 7.0)}), "reverse"),
        ],
        note="속도 슬롯을 변수화하면 안 되고, 특정 값으로 고정해도 안 된다 — 임계가 답.",
    )


def _r6() -> RuleWorld:
    """다중 규칙. 하나의 압축이 아니라 규칙 집합을 찾아야 한다."""
    def fn(e):
        if not e["surf.rigid"]:
            return "pass"
        if e["obj.material"] == "glass":
            return "shatter"
        return "reverse"

    return RuleWorld(
        key="R6",
        title="다중 규칙 3개",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("pass", "shatter", "reverse"),
        truth=[
            GroundTruthRule(frozenset({("eq", "surf.rigid", False)}), "pass"),
            GroundTruthRule(
                frozenset({("eq", "surf.rigid", True), ("eq", "obj.material", "glass")}),
                "shatter",
            ),
            GroundTruthRule(frozenset({("eq", "surf.rigid", True)}), "reverse"),
        ],
        note="세 규칙을 모두 찾고, 더 구체적인 것이 먼저 적용돼야 한다.",
    )


def _r7() -> RuleWorld:
    """예외 계층. 일반 규칙과 그 예외를 동시에 유지할 수 있는가(C4의 전초전)."""
    def fn(e):
        if e["obj.moving"] and e["surf.rigid"]:
            return "shatter" if e["obj.material"] == "glass" else "reverse"
        return "nothing"

    return RuleWorld(
        key="R7",
        title="일반 규칙 + 예외 (유리는 깨진다)",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("shatter", "reverse", "nothing"),
        truth=[
            GroundTruthRule(
                frozenset(
                    {
                        ("eq", "obj.moving", True),
                        ("eq", "surf.rigid", True),
                        ("eq", "obj.material", "glass"),
                    }
                ),
                "shatter",
            ),
            GroundTruthRule(
                frozenset({("eq", "obj.moving", True), ("eq", "surf.rigid", True)}),
                "reverse",
            ),
        ],
        note="예외를 못 찾으면 일반 규칙이 유리 사례에서 계속 틀린다.",
    )


def _r8() -> RuleWorld:
    """관측 잡음 10%. 완벽하지 않은 세계에서도 규칙이 남는가."""
    def fn(e):
        return "reverse" if (e["obj.moving"] and e["surf.rigid"]) else "nothing"

    return RuleWorld(
        key="R8",
        title="논리곱 + 라벨 잡음 10%",
        slots=list(BASE_SLOTS),
        effect_fn=fn,
        effects=("reverse", "nothing"),
        noise=0.10,
        truth=[
            GroundTruthRule(
                frozenset({("eq", "obj.moving", True), ("eq", "surf.rigid", True)}),
                "reverse",
            ),
        ],
        note="잡음에 맞추려고 제약을 덧붙이면(과적합) 실패로 친다.",
    )


ALL_WORLDS = [_r1(), _r2(), _r3(), _r4(), _r5(), _r6(), _r7(), _r8()]


def get_worlds(keys: list | None = None) -> list:
    if not keys:
        return ALL_WORLDS
    idx = {w.key: w for w in ALL_WORLDS}
    return [idx[k] for k in keys]
