"""C2-S 스파이크 실행기 — 개발계획의 킬/통과 판정.

  통과: 정답 스키마 복구율 ≥ 70%
  킬  : < 50% → 접근 변경(확률적 스키마 또는 EBM식 탐색)

복구의 정의는 두 조건의 논리곱이다. 둘 다 필요하다:
  (a) **전이 정확도** — 학습에 쓰지 않은 새 사건에서 결과를 맞히는가.
      추상화의 목적은 압축이 아니라 새 상황으로의 전이이기 때문.
  (b) **인과 슬롯 일치** — 제약한 슬롯 집합이 정답의 인과 슬롯과 정확히 같은가.
      정확도만 높고 엉뚱한 슬롯을 붙들고 있으면 그것은 상관이지 이해가 아니다.

실행: python run_spike.py [--n 2000] [--seed 1]
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from abstraction import Dataset, induce  # noqa: E402
from world import get_worlds  # noqa: E402

METHODS = ["mdl-beam", "mdl-greedy", "lgg"]
ACC_TOL = 0.02  # 전이 정확도 허용 오차


def truth_slots(world) -> set:
    out = set()
    for r in world.truth:
        for c in r.constraints:
            if c[0] in ("eq", "ge", "lt"):
                out.add(c[1])
            elif c[0] == "eqslot":
                out.add(c[1])
                out.add(c[2])
    return out


def evaluate(world, method: str, n_train: int, seed: int) -> dict:
    train = world.sample(n_train, seed)
    # 시험 집합은 항상 잡음 없는 정답으로 만든다 — 규칙을 되찾았는지가 관심사이므로.
    clean = world.noise
    world.noise = 0.0
    test = world.sample(1000, seed + 99991)
    world.noise = clean

    t0 = time.perf_counter()
    ds = Dataset(train, world.slots)
    rs = induce(ds, method=method)
    elapsed = time.perf_counter() - t0

    hit = sum(1 for ev in test if rs.predict(ev) == ev["effect"])
    acc = hit / len(test)

    got = rs.slots()
    want = truth_slots(world)
    slots_ok = got == want
    passed = slots_ok and acc >= 1.0 - ACC_TOL

    return {
        "world": world.key,
        "method": method,
        "accuracy": acc,
        "slots_found": sorted(got),
        "slots_truth": sorted(want),
        "slots_ok": slots_ok,
        "passed": passed,
        "n_rules": len(rs.rules),
        "n_candidates": len(ds.cand),
        "seconds": elapsed,
        "rules": rs.describe(),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=2000, help="학습 사건 수")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--methods", nargs="*", default=METHODS)
    ap.add_argument("--worlds", nargs="*", default=None)
    ap.add_argument("--json", type=str, default=None, help="결과 JSON 저장 경로")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args()

    worlds = get_worlds(args.worlds)
    results = []

    print("=" * 78)
    print("C2-S 구조 추상화 스파이크 — 크리티컬 패스 판정")
    print(f"학습 사건 {args.n}개 · 시험 사건 1000개(무잡음) · 시드 {args.seed}")
    print("=" * 78)

    for method in args.methods:
        print(f"\n### 방법: {method}")
        print(f"{'세계':<5} {'설명':<28} {'정확도':>7} {'슬롯':>5} {'판정':>5} {'초':>6}")
        print("-" * 78)
        for w in worlds:
            r = evaluate(w, method, args.n, args.seed)
            results.append(r)
            print(
                f"{w.key:<5} {w.title[:27]:<28} {r['accuracy']*100:>6.1f}% "
                f"{'○' if r['slots_ok'] else '×':>5} "
                f"{'통과' if r['passed'] else '실패':>5} {r['seconds']:>6.2f}"
            )
            if args.verbose or not r["passed"]:
                print(f"        정답 슬롯: {r['slots_truth']}")
                print(f"        발견 슬롯: {r['slots_found']}")
                for line in r["rules"]:
                    print(f"        · {line}")

        got = [r for r in results if r["method"] == method]
        rate = sum(1 for r in got if r["passed"]) / len(got)
        print("-" * 78)
        print(f"복구율: {rate*100:.1f}%  ({sum(1 for r in got if r['passed'])}/{len(got)})")

    # ---- 최종 판정 ----
    print("\n" + "=" * 78)
    print("판정 요약")
    print("=" * 78)
    best_rate, best_method = 0.0, ""
    for method in args.methods:
        got = [r for r in results if r["method"] == method]
        rate = sum(1 for r in got if r["passed"]) / len(got)
        acc = sum(r["accuracy"] for r in got) / len(got)
        print(f"  {method:<12} 복구율 {rate*100:>5.1f}%   평균 전이 정확도 {acc*100:>5.1f}%")
        if rate > best_rate:
            best_rate, best_method = rate, method

    print()
    if best_rate >= 0.70:
        print(f"✅ 통과 — 최적 방법 '{best_method}' 복구율 {best_rate*100:.1f}% ≥ 70%")
        verdict = "PASS"
    elif best_rate >= 0.50:
        print(f"⚠️  경계 — 최적 {best_rate*100:.1f}%. 통과선 미달이나 킬선 이상. 보완 후 재시도.")
        verdict = "MARGINAL"
    else:
        print(f"❌ 킬 — 최적 {best_rate*100:.1f}% < 50%. 접근 변경 필요.")
        verdict = "KILL"

    if args.json:
        Path(args.json).write_text(
            json.dumps(
                {"verdict": verdict, "best_method": best_method,
                 "best_rate": best_rate, "n_train": args.n,
                 "seed": args.seed, "results": results},
                ensure_ascii=False, indent=2,
            ),
            encoding="utf-8",
        )
        print(f"\n결과 저장: {args.json}")

    return 0 if verdict == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
