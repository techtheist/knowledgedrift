#!/usr/bin/env python3
"""Replay a KnowledgeDrift script through an adapter and write the transcript.

    python3 run.py --adapter mem0 --script ../../data/knowledgedrift/knowledgedrift-100-seed1.json \
                   --out out/mem0-100.json
    cargo run --release -- --grade out/mem0-100.json \
                   --script ../../data/knowledgedrift/knowledgedrift-100-seed1.json --json out/mem0-100-graded.json

The runner is deliberately dumb: it does not know what a probe expects, it
times every operation, and it RAISES on an adapter error rather than
recording an empty reply — an error that empties a recall is
indistinguishable from an honest "the store is silent", and the grader
would score it as one.
"""

from __future__ import annotations

import argparse
import importlib
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from adapter import CAPABILITY_NAMES, MemoryAdapter  # noqa: E402


def load_adapter(name: str, **kwargs) -> MemoryAdapter:
    mod = importlib.import_module(f"{name}_adapter")
    cls = getattr(mod, "Adapter")
    return cls(**kwargs)


def replay(script: dict, mem: MemoryAdapter, limit_ops: int | None = None, progress=None) -> dict:
    replies: list[dict] = []
    timing: dict[str, dict[str, float]] = {}
    settle_note = None
    ops = script["ops"] if limit_ops is None else script["ops"][:limit_ops]

    def clock(kind: str, started: float) -> None:
        t = timing.setdefault(kind, {"count": 0, "total_ms": 0.0})
        t["count"] += 1
        t["total_ms"] += (time.perf_counter() - started) * 1000.0

    mem.reset()
    for i, op in enumerate(ops):
        started = time.perf_counter()
        kind = op["op"]
        if kind == "inscribe":
            result = mem.inscribe(op["record"], op["mode"])
            if op.get("id") is not None:
                replies.append({"reply": "inscribe", "id": op["id"], "result": result.to_json()})
        elif kind == "link":
            mem.link(op["from"], op["to"], op["verb"])
        elif kind == "supersede":
            mem.supersede(op["old"], op["new"])
        elif kind == "release":
            mem.release(op["key"], op["reason"])
        elif kind == "purge":
            mem.purge(op["key"])
        elif kind == "settle":
            note = mem.settle()
            if note:
                settle_note = note
        elif kind == "recall":
            result = mem.recall(op["query"], op["k"], op.get("window"))
            replies.append({"reply": "recall", "id": op["id"], "result": result.to_json()})
        elif kind == "suspects":
            pairs = mem.suspects()
            r: dict = {"reply": "suspects", "id": op["id"]}
            if pairs is not None:
                r["pairs"] = pairs
            replies.append(r)
        elif kind == "lineage":
            keys = mem.lineage(op["key"])
            r = {"reply": "lineage", "id": op["id"]}
            if keys is not None:
                r["keys"] = keys
            replies.append(r)
        else:
            raise ValueError(f"unknown op {kind!r} at index {i}")
        clock(kind, started)
        if progress and (i + 1) % 250 == 0:
            progress(i + 1, len(ops))

    caps = mem.capabilities()
    return {
        "arm": mem.name,
        "capabilities": {n: bool(caps.get(n, False)) for n in CAPABILITY_NAMES},
        "standing_tokens": int(mem.standing_tokens()),
        "script_digest": script["digest"],
        **({"settle_note": settle_note} if settle_note else {}),
        "replies": replies,
        "timing": timing,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--adapter", required=True, help="module name prefix: <name>_adapter.py with class Adapter")
    ap.add_argument("--script", required=True, help="exported script JSON")
    ap.add_argument("--out", required=True, help="transcript JSON to write")
    ap.add_argument("--limit-ops", type=int, default=None, help="replay only the first N ops (smoke)")
    ap.add_argument("--kw", action="append", default=[], help="adapter constructor kwarg, key=value")
    args = ap.parse_args()

    kwargs = {}
    for kv in args.kw:
        k, _, v = kv.partition("=")
        kwargs[k] = v
    script = json.load(open(args.script))
    if not script.get("digest"):
        print("script carries no digest — re-export it with the current knowledgedrift", file=sys.stderr)
        return 2
    mem = load_adapter(args.adapter, **kwargs)
    print(f"{mem.name}: {len(script['ops'])} ops, {len(script['probes'])} probes, world {script['spec']['size']}", file=sys.stderr)
    started = time.perf_counter()
    try:
        transcript = replay(script, mem, args.limit_ops, progress=lambda i, n: print(f"  {i}/{n} ops", file=sys.stderr))
    finally:
        mem.close()
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    json.dump(transcript, open(args.out, "w"), ensure_ascii=False)
    print(f"wrote {args.out} ({len(transcript['replies'])} replies, {time.perf_counter() - started:.0f}s)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
