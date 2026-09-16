# MemContinuum 0.2.0rc5 — v2 receipts

decision chains over append-only topics, hybrid FTS5 + bge-small
(`adapters/memcontinuum.md`), **the v2 adapter** — `link` writes the
schema's typed edges and a hit carries its edge-neighbours, `recall_path`
is the system's own `for-path` — replayed on
`worlds/v2/knowledgedrift-{500,1500}-seed1-v2.json` (2026-09-16, one
Apple-silicon laptop, CPU, the three external chains and the in-process
ladder running side by side so wall-clock is contended). Files
`500-seed1.json` and `1500-seed1.json` are what
`knowledgedrift --grade … --json` wrote; the `.log` beside each is the
terminal report. The rows and the before/after reading are in
`results/v2/README.md`.

| world | success | families | signal | tokens | v2 score | billed tok/query | rationale | path_r@5 / path_cover |
|---|---|---|---|---|---|---|---|---|
| 500 seed 1 | 48% | 337 | 9 | 46 | **392** | 852 | 100% (structure_only 0.95) | 1.00 / 0.95 |
| 1500 seed 1 | 44% | 326 | 7 | 47 | **380** | 844 | 100% (structure_only 0.93) | 1.00 / 0.87 |

With the v1 adapter on the same worlds (before 2026-09-16): 295 and 287,
rationale 5% at both sizes. The v1 receipts under `results/v1/` were taken
with that adapter and are left as they were.
