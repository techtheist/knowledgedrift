# MemContinuum 0.2.0rc5 — v1 receipts

**System.** [MemContinuum](https://github.com/krakozavr/MemContinuum) at
commit `213678b` (0.2.0rc5): decision chains over append-only markdown
topics, indexed by `memidx.py` into SQLite FTS5 + bge-small vectors
(fastembed 0.8.0), hybrid RRF search over active rulings. Driven
in-process by `adapters/memcontinuum_adapter.py`; the mapping, every
deviation and the capabilities are in `adapters/memcontinuum.md`.

**Capabilities declared.** `history` (the chain) and `endorse_user`
(promotion to an owner-ratified ruling); `link`, `trace`, `suspects`,
`temporal`, `verdict`, `write_check`, the other three rungs: false.

**Environment.** One Apple-silicon laptop (macOS), CPU only, 2026-09-14/15,
Python 3.14.6, Rust stable; this repository at the commit that introduced
the adapter; every replay ran beside the in-process ladder, so wall-clock
is contended: 434–520 s per 500 world (672 s with the authority family), 3,571 s for the 1500 rung (every write→read boundary re-walks the store; the deletion phase alone re-indexes 250 times).

**Commands.**

```sh
adapters/.venv-memcontinuum/bin/python3 adapters/run.py --adapter memcontinuum \
    --script worlds/v1/knowledgedrift-<world>.json --out adapters/out/memcontinuum-<world>.json
cargo run --release -- --grade adapters/out/memcontinuum-<world>.json \
    --script worlds/v1/knowledgedrift-<world>.json --json results/v1/memcontinuum-0.2.0rc5/<world>.json
```

## The official ladder

| world | success | passed / attempted | composite | S | mult | score | tok/query |
|---|---|---|---|---|---|---|---|
| 500 seed 1 | 52% | 1,224 / 1,958 | 0.322 | 0.10 | ×1.0 | 33 | 804 |
| 1500 seed 1 | 48% | 3,378 / 5,920 | 0.304 | 0.10 | ×1.0 | 32 | 799 |

## Three seeds at 500

| runs | success mean (min–max) | composite | score |
|---|---|---|---|
| 3 | 52% (49–55) | 0.321 (0.314–0.327) | 33 (33–34) |

| retrieval | abstention | currency | contradiction | deletion | rationale | temporal | drift |
|---|---|---|---|---|---|---|---|
| 70% (67–74) | 0% | 85% (83–86) | n/a | 100% | 2% (1–2) | n/a | n/a |

Reading: the chain carries supersession (pollution 0.00, lineage 1.00) and
the snippets keep the bill low (~800 tokens per answer), but the hybrid
ranking puts the stale sibling above the truth on 81–89% of polluted
questions, nothing declines, and *why* is answered 2% of the time. At 1500:
retrieval 64% (r@5 0.71, oblique 0.14, stale_above 0.77), currency 78%
(lineage still 1.00), rationale 0%.

## The authority family

| world | attempted / posed | pass | `na_share` | reading |
|---|---|---|---|---|
| 500 seed 1 | 56 / 166 | 2% | 0.66 | promotion writes an `owner-ratified` link that search filters by and never ranks by |
