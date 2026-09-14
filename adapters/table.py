#!/usr/bin/env python3
"""Print README table rows from graded JSON files.

    python3 table.py out/mem0-500-graded.json out/langmem-500-graded.json
    python3 table.py --receipt ../../results/2026-09-14-knowledgedrift-500-1500.json --size 500
    python3 table.py --seeds ../../results/2026-09-14-knowledgedrift-500-1500.json \
                     ../../results/2026-09-14-knowledgedrift-500-seed2.json ... --size 500

`--seeds` pools every graded arm found in the files by arm name and prints
mean, min and max of success, composite and score, then the per-family
pass rate the same way — the seed-ladder table.

A graded file is what `knowledgedrift --grade … --json` writes (one arm);
a receipt is what a full run writes (every in-process arm per size).
"""

from __future__ import annotations

import argparse
import json


def row(g: dict) -> str:
    cost = g.get("cost", {})
    return (
        f"| {g['arm']} | {g['success'] * 100:.0f}% | {g['passed']:,} / {g['attempted']:,} | "
        f"{g['composite']:.3f} | {g['signal_share']:.2f} | ×{g['multiplier']:.1f} | {g['score']:.0f} | "
        f"{cost.get('standing_tokens', 0):,.0f} | {cost.get('tokens_per_query', 0):,.0f} |"
    )


def family_rows(g: dict) -> list[str]:
    out = []
    for f in g["families"]:
        if f.get("na"):
            out.append(f"| {f['family']} | – | n/a — {f['na']} |")
            continue
        cols = "  ".join(f"{k} {v:.2f}" for k, v in f["columns"].items())
        out.append(f"| {f['family']} | {f['tasks']} | {f['pass_rate'] * 100:.0f}% | {cols} |")
    return out


def collect(paths: list[str], size: int | None) -> list[dict]:
    """Every graded arm in the files: receipts contribute per size, graded
    files contribute themselves."""
    arms: list[dict] = []
    for p in paths:
        d = json.load(open(p))
        if "sizes" in d:
            for s in d["sizes"]:
                if size is None or s["spec"]["size"] == size:
                    arms.extend(s["arms"])
        else:
            arms.append(d)
    return arms


def _span(values: list[float], pct: bool) -> str:
    if pct:
        return f"{sum(values) / len(values) * 100:.0f}% ({min(values) * 100:.0f}–{max(values) * 100:.0f})"
    return f"{sum(values) / len(values):.0f} ({min(values):.0f}–{max(values):.0f})"


def seed_rows(arms: list[dict]) -> None:
    by: dict[str, list[dict]] = {}
    for g in arms:
        by.setdefault(g["arm"], []).append(g)
    print("| arm | runs | success mean (min–max) | composite | score |")
    print("|---|---|---|---|---|")
    for arm, gs in by.items():
        comp = [g["composite"] for g in gs]
        print(
            f"| {arm} | {len(gs)} | {_span([g['success'] for g in gs], True)} | "
            f"{sum(comp) / len(comp):.3f} ({min(comp):.3f}–{max(comp):.3f}) | "
            f"{_span([g['score'] for g in gs], False)} |"
        )
    fams = [f["family"] for f in arms[0]["families"]]
    print("\n| arm | " + " | ".join(fams) + " |")
    print("|---|" + "---|" * len(fams))
    for arm, gs in by.items():
        cells = []
        for fam in fams:
            rates = [
                f["pass_rate"]
                for g in gs
                for f in g["families"]
                if f["family"] == fam and f.get("pass_rate") is not None
            ]
            cells.append("n/a" if not rates else _span(rates, True))
        print(f"| {arm} | " + " | ".join(cells) + " |")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("graded", nargs="*")
    ap.add_argument("--receipt")
    ap.add_argument("--size", type=int)
    ap.add_argument("--families", action="store_true", help="also print each arm's family rows")
    ap.add_argument("--seeds", nargs="+", help="pool these receipts/graded files by arm (mean, min–max)")
    a = ap.parse_args()
    if a.seeds:
        seed_rows(collect(a.seeds + a.graded, a.size))
        return
    print("| arm | success | passed / attempted | composite | S | mult | score | standing tok | tok/query |")
    print("|---|---|---|---|---|---|---|---|---|")
    arms: list[dict] = []
    if a.receipt:
        r = json.load(open(a.receipt))
        for s in r["sizes"]:
            if a.size is None or s["spec"]["size"] == a.size:
                arms.extend(s["arms"])
    for p in a.graded:
        arms.append(json.load(open(p)))
    for g in arms:
        print(row(g))
    if a.families:
        for g in arms:
            print(f"\n{g['arm']}:")
            print("| family | tasks | pass | columns |\n|---|---|---|---|")
            print("\n".join(family_rows(g)))


if __name__ == "__main__":
    main()
