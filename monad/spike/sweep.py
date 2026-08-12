"""C2-S 스파이크 후속 측정: 안정성 · 표본 효율 · 구성요소 어블레이션.

세 가지를 답한다.
 1. **안정성** — 시드를 바꿔도 복구되는가(운이 아니었는가).
 2. **표본 효율** — 규칙 하나를 배우는 데 사건 몇 개가 필요한가.
    MONAD의 핵심 주장(샘플 효율)이 추상화 층에서도 성립하는지 보는 지표.
 3. **어블레이션** — 전방탐색·MDL·예외정제 각각을 끄면 무엇이 무너지는가.

실행: python sweep.py
"""

from __future__ import annotations

import json
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from abstraction import Dataset, induce  # noqa: E402
from run_spike import truth_slots  # noqa: E402
from world import get_worlds  # noqa: E402

SEEDS = [1, 2, 3, 4, 5]
SIZES = [40, 60, 80, 120, 200, 400, 800, 1500]


def trial(world, n: int, seed: int, **kw) -> tuple:
    """(통과여부, 정확도) 반환."""
    train = world.sample(n, seed)
    clean = world.noise
    world.noise = 0.0
    test = world.sample(600, seed + 99991)
    world.noise = clean

    ds = Dataset(train, world.slots)
    rs = induce(ds, **kw)
    acc = sum(1 for ev in test if rs.predict(ev) == ev["effect"]) / len(test)
    ok = (rs.slots() == truth_slots(world)) and acc >= 0.98
    return ok, acc


def stability() -> dict:
    print("=" * 74)
    print("1. 안정성 — 시드 5개 반복 (n=1500)")
    print("=" * 74)
    print(f"{'세계':<5} {'통과/시도':>10} {'평균 정확도':>12}")
    print("-" * 74)
    out = {}
    for w in get_worlds():
        oks, accs = [], []
        for s in SEEDS:
            ok, acc = trial(w, 1500, s)
            oks.append(ok)
            accs.append(acc)
        out[w.key] = {"pass": sum(oks), "of": len(SEEDS), "acc": statistics.mean(accs)}
        print(f"{w.key:<5} {sum(oks):>5}/{len(SEEDS):<4} {statistics.mean(accs)*100:>11.1f}%")
    tot = sum(v["pass"] for v in out.values())
    den = sum(v["of"] for v in out.values())
    print("-" * 74)
    print(f"전체: {tot}/{den} = {tot/den*100:.1f}%")
    return out


def sample_efficiency() -> dict:
    print("\n" + "=" * 74)
    print("2. 표본 효율 — 규칙을 되찾는 데 필요한 최소 사건 수")
    print("   (시드 5개 중 4개 이상 통과하는 최소 n)")
    print("=" * 74)
    header = f"{'세계':<5}" + "".join(f"{n:>7}" for n in SIZES) + f"{'최소n':>8}"
    print(header)
    print("-" * 74)
    out = {}
    for w in get_worlds():
        row = f"{w.key:<5}"
        min_n = None
        for n in SIZES:
            passes = sum(1 for s in SEEDS if trial(w, n, s)[0])
            row += f"{passes:>6}/5"
            if min_n is None and passes >= 4:
                min_n = n
        row += f"{(min_n if min_n else '>1500'):>8}"
        out[w.key] = min_n
        print(row)
    print("-" * 74)
    vals = [v for v in out.values() if v]
    if vals:
        print(f"중앙값 최소 사건 수: {statistics.median(vals):.0f}개")
    return out


def ablation() -> dict:
    print("\n" + "=" * 74)
    print("3. 어블레이션 — 각 구성요소를 끄면 무엇이 무너지는가 (n=1500, 시드 3개)")
    print("=" * 74)
    configs = {
        "전체(mdl-beam)": {},
        "− 전방탐색": {"method": "mdl-greedy"},
        "− 예외정제": {"exception_depth": 0},
        "− MDL(=LGG)": {"method": "lgg"},
    }
    worlds = get_worlds()
    print(f"{'구성':<16}" + "".join(f"{w.key:>6}" for w in worlds) + f"{'복구율':>9}")
    print("-" * 74)
    out = {}
    for name, kw in configs.items():
        row = f"{name:<16}"
        oks = 0
        tot = 0
        for w in worlds:
            p = sum(1 for s in SEEDS[:3] if trial(w, 1500, s, **kw)[0])
            row += f"{('○' if p >= 2 else '×'):>6}"
            oks += p
            tot += 3
        rate = oks / tot
        out[name] = rate
        row += f"{rate*100:>8.1f}%"
        print(row)
    print("-" * 74)
    print("○ = 시드 3개 중 2개 이상 복구.  각 열은 그 구성요소가 필요한 이유를 가리킨다.")
    return out


def main():
    t0 = time.perf_counter()
    st = stability()
    se = sample_efficiency()
    ab = ablation()
    dt = time.perf_counter() - t0
    print(f"\n총 소요 {dt:.1f}초")

    Path("sweep-results.json").write_text(
        json.dumps(
            {"stability": st, "min_events": se, "ablation": ab},
            ensure_ascii=False, indent=2,
        ),
        encoding="utf-8",
    )
    print("결과 저장: sweep-results.json")


if __name__ == "__main__":
    main()
