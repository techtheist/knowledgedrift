# cognee 1.5.4 — v1 receipts

**System.** [cognee](https://github.com/topoteretes/cognee) 1.5.4 (PyPI)
**without its LLM**: documents through `cognee.add`, cognee's `cognify()`
pipeline with the LLM extraction task removed (classify → chunk →
`add_data_points`, the shape of its own no-LLM routes), its `fastembed`
provider with bge-small, its default LanceDB vector store and LadybugDB
graph store, `SearchType.CHUNKS`. Driven in-process by
`adapters/cognee_adapter.py`; the mapping, every deviation, the
configuration that closes cognee's four LLM paths, and the capabilities
are in `adapters/cognee.md`. What cognee sells — the LLM-built graph and
the completion searches over it — is not measured.

**Capabilities declared.** None: `link`, `history`, `trace`, `suspects`,
`temporal`, `verdict`, `write_check` and the four endorsement rungs are
all false. cognee's chunk search has no time filter, so temporal is N/A —
the one family the other flat stores attempt and this one cannot.

**Environment.** One Apple-silicon laptop (macOS), CPU only, 2026-09-15,
Python 3.14.6, Rust stable; this repository at the commit that introduced
the adapter; the replays ran one after another on a laptop in use, so
wall-clock is contended and variable: 642 / 876 / 1,914 s for the three
500 worlds, 5,719 s for the 1500 rung (every write→read boundary
runs cognee's incremental pipeline pass, and cognee's delete walks the
graph and the vector rows per note).

**Commands.**

```sh
KD_COGNEE_STORE=$PWD/adapters/out/cognee-store-<world> \
adapters/.venv-cognee/bin/python3 adapters/run.py --adapter cognee \
    --script worlds/v1/knowledgedrift-<world>.json --out adapters/out/cognee-<world>.json
cargo run --release -- --grade adapters/out/cognee-<world>.json \
    --script worlds/v1/knowledgedrift-<world>.json --json results/v1/cognee-1.5.4/<world>.json
```

## The official ladder

| world | success | passed / attempted | composite | S | mult | score | tok/query |
|---|---|---|---|---|---|---|---|
| 500 seed 1 | 57% | 1,341 / 1,933 | 0.341 | 0.11 | ×1.1 | 39 | 2,002 |
| 1500 seed 1 | 52% | 3,672 / 5,845 | 0.318 | 0.12 | ×1.2 | 39 | 1,876 |

## Three seeds at 500

| runs | success mean (min–max) | composite | score |
|---|---|---|---|
| 3 | 58% (56–60) | 0.342 (0.337–0.348) | 40 (39–42) |

| retrieval | abstention | currency | contradiction | deletion | rationale | temporal | drift |
|---|---|---|---|---|---|---|---|
| 80% (78–82) | 0% | 91% (89–92) | n/a | 100% | 3% (3–4) | n/a | n/a |

Reading: a cosine top-k over the same embedder lands where `rag` lands —
r@5 0.83 / 0.84 / 0.83, oblique 0.52 / 0.57 / 0.54, the stale sibling above
the truth on 0.37 / 0.38 / 0.45 of polluted questions, nothing declines
(fp 1.00, separation 0.79–0.85), and *why* is answered 3–4% of the time.
Currency 91% because delete + add leaves no retired generation behind
(pollution 0.00); lineage is N/A. The bill is the flat store's: ~2,000
tokens per answer, whole notes. The eight points between cognee and `rag`
on the headline are the temporal family (125 tasks at 500) counted as
failed, not a retrieval difference. At 1500: retrieval 72% (r@5 0.75, oblique 0.33,
stale_above 0.38), currency 81%, rationale 1%, 1,876 tokens per answer —
52%, score 39, four points under `rag` for the same reason.
