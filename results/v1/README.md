# v1 results

Every graded receipt behind the tables in `RESULTS.md` and the README, and
how each was produced. A receipt is what `knowledgedrift --json` (the
in-process ladder, every arm per size) or `knowledgedrift --grade … --json`
(one external transcript) writes: the world spec, the script digest, the
capabilities, every family's pass rate and columns, the attention and cost
numbers, and the ids of every failed task, so a number can be read back
to the case that produced it. CI checks that every receipt's digest is one
of the v1 worlds.

## Environment

All receipts: one Apple-silicon laptop (macOS), CPU only, 2026-09-14.
Models: `BAAI/bge-small-en-v1.5` (fp32) as the embedder for every arm;
the engram arm additionally loads `jina-reranker-v1-turbo-en` and
`deberta-v3-small-tasksource-nli`, its shipped stack. Harness: this
repository at v1.0.0; the in-process receipts were produced by the same
sources inside the engram repository at the revision `Cargo.toml` pins,
and a re-run from this repository reproduces them to the digit (the 100
world was re-run as the check).

## Directories

### `reference-arms/` — the six in-process arms

`500-seed1-engram-0.9.6.{json,log}` is the engram arm alone on the v1
500 world with Engram Alpha 0.9.6 (2026-09-16), the version the v2 tables
quote — 86% success, score 518, against the 0.9.5-era 85% / 511 in
`500-seed1.json`; the v1 tables are not re-quoted for it.

`cargo run --release --features fastembed -- <flags> --json <file>`;
the `.log` beside each receipt is the terminal report.

| file | flags | worlds |
|---|---|---|
| `ladder-500-1500-seed1.json` | `--sizes 500,1500` | the official ladder, seed 1 |
| `500-seed2.json` | `--sizes 500 --seed 2` | seed ladder |
| `500-seed3.json` | `--sizes 500 --seed 3` | seed ladder |
| `500-seed1-twin.json` | `--sizes 500 --pollution-shape twin` | pollution shape |
| `500-seed1-late.json` | `--sizes 500 --pollution-shape late` | pollution shape |
| `500-seed1-authority.json` | `--sizes 500 --authority` | the authority family (plus the plain eight) |
| `1500-seed1-authority.json` | `--sizes 1500 --authority` | the authority family at 1500 |
| `500-seed1-tfidf.json` | `--sizes 500 --arms tfidf` | the in-process `tfidf` arm (v2 branch): the lexical crack reproduced on the v1 500 world — 564 at 61% |

Arms in every file: `engram` (engram-core 0.9.4, in-memory store, driven
the way its daemon drives it), `rag` (vector top-k, same embedder, nothing
else), `grep` (keyword overlap, whole records), `curated` (a 3,000-token
memory file, always in context), `whole` (every record in context), `chance`.

### `langmem-0.0.30/` — LangMem's memory layer

`adapters/langmem_adapter.py`, notes and every shim in
`adapters/langmem.md`: the LangGraph `InMemoryStore` with LangMem's semantic
index, through the three store calls LangMem's tools make; bge-small via
fastembed; temporal native, every other capability false. Files
`<size>-seed<N>.json` for 100/500/1500 seed 1 and 500 seeds 2, 3.

### `memcontinuum-0.2.0rc5/` — MemContinuum's decision chains

`adapters/memcontinuum_adapter.py`, notes in `adapters/memcontinuum.md`:
the engine (`memidx`) driven in-process — one append-only markdown topic
per note, lifecycle moves for supersede and release, `cmd_search --json`
hybrid (FTS5 + bge-small via fastembed, RRF) with its defaults; history
native (the chain), the owner's promotion the one endorsement rung, every
other capability false. Files `500-seed{1,2,3}.json`, `1500-seed1.json`,
and the authority world `500-seed1-authority.json` (its 1500 authority
replay was not run: at 2% on the 500 world there was nothing a second
rung would change).

### `cognee-1.5.4/` — cognee without its LLM

`adapters/cognee_adapter.py`, notes in `adapters/cognee.md`: documents
through `cognee.add`, cognee's `cognify()` pipeline with the LLM
extraction task removed (classify → chunk → `add_data_points`, the shape
of its own no-LLM routes), its `fastembed` provider with bge-small, its
default LanceDB + LadybugDB stores, `SearchType.CHUNKS`; every capability
false — cognee's chunk search has no time filter, so temporal is N/A too.
Files `500-seed{1,2,3}.json`, `1500-seed1.json`.

### `mem0-2.0.20/` — Mem0 OSS as a raw store

`adapters/mem0_adapter.py`, notes in `adapters/mem0.md`: `add(infer=False)`
(no LLM anywhere; the run is socket-guarded), embedded Qdrant, Mem0's
hybrid dense + BM25 scoring on, bge-small fp32 via sentence-transformers;
temporal native via a stored capture time, every other capability false.
Same five files.

## The tables

`python3 adapters/table.py --receipt results/v1/reference-arms/ladder-500-1500-seed1.json --size 500`
prints a size's rows; `--seeds <files…> --size 500` pools receipts by arm
into the three-seed table.

### The official ladder, seed 1

**500 tested facts** (630 notes, 2,346 tasks):

| arm | success | passed / attempted | composite | S | mult | score | standing tok | tok/query |
|---|---|---|---|---|---|---|---|---|
| langmem | 63% | 1,477 / 2,058 | 0.469 | 0.11 | ×1.1 | 51 | 0 | 2,328 |
| rag | 63% | 1,478 / 2,058 | 0.469 | 0.11 | ×1.1 | 51 | 0 | 2,335 |
| mem0 | 58% | 1,361 / 2,058 | 0.438 | 0.10 | ×1.0 | 45 | 0 | 2,581 |
| cognee | 57% | 1,341 / 1,933 | 0.341 | 0.11 | ×1.1 | 39 | 0 | 2,002 |
| grep | 53% | 1,241 / 2,058 | 0.418 | 0.10 | ×1.0 | 44 | 0 | 2,606 |
| memcontinuum | 52% | 1,224 / 1,958 | 0.322 | 0.10 | ×1.0 | 33 | 0 | 804 |
| whole file | 71% | 1,658 / 1,933 | 0.486 | 0.00 | ×0.1 | 5 | 141,258 | 134,043 |
| curated (3k) | 9% | 203 / 1,933 | 0.151 | 0.03 | ×0.3 | 4 | 2,988 | 2,950 |
| chance | 4% | 99 / 2,058 | 0.132 | 0.11 | ×1.1 | 15 | 0 | 2,311 |
| **engram** | **85%** | 2,005 / 2,346 | **0.905** | 0.57 | ×5.7 | **511** | 3,875 | 263 |

**1500 tested facts** (1,883 notes, 7,078 tasks):

| arm | success | passed / attempted | composite | S | mult | score | standing tok | tok/query |
|---|---|---|---|---|---|---|---|---|
| langmem | 57% | 4,066 / 6,220 | 0.442 | 0.12 | ×1.2 | 52 | 0 | 2,147 |
| rag | 57% | 4,064 / 6,220 | 0.442 | 0.12 | ×1.2 | 52 | 0 | 2,153 |
| mem0 | 55% | 3,880 / 6,220 | 0.430 | 0.11 | ×1.1 | 47 | 0 | 2,452 |
| cognee | 52% | 3,672 / 5,845 | 0.318 | 0.12 | ×1.2 | 39 | 0 | 1,876 |
| grep | 51% | 3,575 / 6,220 | 0.412 | 0.10 | ×1.0 | 43 | 0 | 2,620 |
| memcontinuum | 48% | 3,378 / 5,920 | 0.304 | 0.10 | ×1.0 | 32 | 0 | 799 |
| whole file | 71% | 5,041 / 5,845 | 0.487 | 0.00 | ×0.1 | 5 | 423,672 | 401,934 |
| curated (3k) | 5% | 376 / 5,845 | 0.134 | 0.03 | ×0.3 | 4 | 2,982 | 2,932 |
| chance | 4% | 273 / 6,220 | 0.129 | 0.10 | ×1.0 | 13 | 0 | 2,322 |
| **engram** | **80%** | 5,694 / 7,078 | **0.877** | 0.52 | ×5.2 | **459** | 3,769 | 289 |

### Three seeds at 500 (mean, min–max)

| arm | success | composite | score |
|---|---|---|---|
| langmem | 63% (61–66) | 0.468 (0.461–0.473) | 52 (51–54) |
| rag | 63% (61–66) | 0.468 (0.461–0.473) | 52 (51–54) |
| mem0 | 59% (57–61) | 0.443 (0.438–0.447) | 46 (45–48) |
| cognee | 58% (56–60) | 0.342 (0.337–0.348) | 40 (39–42) |
| grep | 53% (51–55) | 0.418 (0.414–0.422) | 43 (42–44) |
| memcontinuum | 52% (49–55) | 0.321 (0.314–0.327) | 33 (33–34) |
| whole file | 71% (69–74) | 0.487 (0.483–0.492) | 5 (5–5) |
| curated (3k) | 9% (9–9) | 0.156 (0.151–0.160) | 4 (4–4) |
| chance | 4% (4–4) | 0.131 (0.131–0.132) | 13 (11–15) |
| **engram** | **85% (84–85)** | 0.907 (0.898–0.918) | **525 (511–543)** |

Per family, three seeds:

| arm | retrieval | abstention | currency | contradiction | drift | deletion | rationale | temporal |
|---|---|---|---|---|---|---|---|---|
| langmem | 80% (78–83) | 0% | 90% (87–93) | n/a | n/a | 100% | 4% (3–5) | 100% (99–100) |
| rag | 80% (78–83) | 0% | 90% (87–93) | n/a | n/a | 100% | 4% (3–5) | 100% (99–100) |
| mem0 | 74% (72–76) | 0% | 78% (76–80) | n/a | n/a | 100% | 2% (2–3) | 100% |
| cognee | 80% (78–82) | 0% | 91% (89–92) | n/a | n/a | 100% | 3% (3–4) | n/a |
| grep | 65% (63–67) | 0% | 69% (68–69) | n/a | n/a | 100% | 0% (0–1) | 100% |
| memcontinuum | 70% (67–74) | 0% | 85% (83–86) | n/a | n/a | 100% | 2% (1–2) | n/a |
| curated | 6% (6–7) | 0% | 0% | n/a | n/a | 100% | 18% (15–21) | n/a |
| whole | 90% (87–94) | 0% | 100% | n/a | n/a | 100% | 100% | n/a |
| chance | 1% | 0% | 0% | n/a | n/a | 100% | 0% (0–1) | 4% |
| engram | 81% (80–82) | 100% (99–100) | 89% (87–91) | 74% (72–75) | 93% (91–97) | 90% (90–90) | 100% (99–100) | 100% (99–100) |

### Pollution shapes at 500, seed 1

`stale_above` = share of polluted questions whose stale sibling outranked
the answer; `noticed` = drift pairs the suspect queue raised.

| shape | engram success | engram stale_above | engram drift noticed | rag stale_above | grep stale_above |
|---|---|---|---|---|---|
| `stale` (default) | 85% | 0.21 | 0.91 | 0.38 | 0.58 |
| `twin` | 82% | 0.64 | 0.98 | 0.55 | 0.78 |
| `late` | 85% | 0.35 | 0.91 | 0.38 | 0.58 |

### Authority at 500 and 1500, seed 1

`results/v1/reference-arms/{500,1500}-seed1-authority.json` and
`results/v1/memcontinuum-0.2.0rc5/500-seed1-authority.json`. The
authority world's first eight families reproduce the plain world within
three tasks (engram 2,004 vs 2,005 passed at 500, 5,697 vs 5,694 at 1500:
its trust prior reads the wall clock); the ninth family poses 166 tasks at
500 and 500 at 1500. A task whose winning rung a system did not
declare is posed, not attempted (`na_share`); every flat store declares no
rung and is N/A.

| arm | rungs declared | attempted / posed | pass | l1 autonomous | l2 governed | l3 three hands | l4 supervised | winner r@5 | winner first | order exact | resurrected |
|---|---|---|---|---|---|---|---|---|---|---|---|
| engram @500 | assistant, user, supervisor | 135 / 166 | **49%** | 0.47 | 0.61 | 0.50 | 0.29 | 0.99 | 0.52 | 0.50 | 0.86 |
| engram @1500 | assistant, user, supervisor | 405 / 500 | **56%** | 0.59 | 0.65 | 0.58 | 0.34 | 0.99 | 0.59 | 0.58 | 0.78 |
| memcontinuum @500 | user | 56 / 166 | 2% | – | 0.02 | 0.00 | – | – | 0.02 | – | – |
| rag, grep, curated, whole, chance | none | 0 / 166 | n/a | | | | | | | | |

engram per scenario, 500 / 1500 (eight and twenty-four cases each; `n/a`
= the winning rung is not one it declares):

| scenario | 500 | 1500 | scenario | 500 | 1500 |
|---|---|---|---|---|---|
| `confirm_vs_none` | 0.88 | 0.92 | `exposure_vs_none` | n/a | n/a |
| `confirm_vs_fresh` | 0.25 | 0.46 | `confirm_vs_exposure` | 0.50 | 0.58 |
| `confirm_vs_kind` | 0.38 | 0.62 | `approve_vs_exposure_confirm` | 0.50 | 0.58 |
| `latest_confirm` | 0.38 | 0.38 | `exposure_vs_fresh` | n/a | n/a |
| `approve_vs_confirm` | 0.62 | 0.71 | `ladder3` | n/a | n/a |
| `approve_vs_confirms` | 0.88 | 0.83 | `pin_vs_approve` | 0.50 | 0.42 |
| `approve_vs_kind` | 0.62 | 0.62 | `pin_vs_everything` | 0.25 | 0.35 |
| `approve_vs_claims` | 0.50 | 0.62 | `latest_pin` | 0.38 | 0.43 |
| `approve_vs_fresh_confirm` | 0.62 | 0.67 | `pin_then_supersede` | 0.00 | 0.17 (retired twin delivered 0.86 / 0.78) |
| `ladder2` | 0.50 | 0.58 | `ladder4` | n/a | n/a |
| `latest_approve` | 0.50 | 0.50 | | | |

### The 100 world, seed 1 (preliminary rung, single seed)

| arm | success | composite | score |
|---|---|---|---|
| langmem | 71% | 0.516 | 54 |
| rag | 71% | 0.516 | 54 |
| mem0 | 68% | 0.510 | 51 |
| grep | 62% | 0.459 | 50 |
| whole file | 72% | 0.488 | 5 |
| curated (3k) | 29% | 0.317 | 9 |
| chance | 7% | 0.168 | 18 |
| engram | 92% | 0.947 | 601 |

(The 100 rung for the in-process arms is not a committed receipt; it is
reproduced by `cargo run --release --features fastembed -- --sizes 100`.)
