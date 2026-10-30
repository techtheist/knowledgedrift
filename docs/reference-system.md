# The reference system

The top row of the ladder is **[Engram Alpha](https://github.com/techtheist/engram)**,
a local, LLM-free reasoning memory for coding agents. It keeps decisions,
cautions and their reasons as a typed graph, and it queues contradictions
for a person to judge instead of resolving them itself. KnowledgeDrift grew
out of its evaluation harness: Engram's `eval/` directory held the retrieval
ladder, the supersession chains and the contradiction bench that became
this benchmark's families.

## Why it's here

A ladder needs a fixed point that exercises every family. Most of the
systems tested here declare no `suspects`, `trace` or `verdict`, so whole
families are N/A for them. The reference system declares every capability
the eight plain families need, so every one of their rules is exercised by
at least one implementation. A rule nobody can pass is a rule nobody has
checked. (It does not declare `endorse_retrieval`: retrieval use is not a
trust rung in its design, so those authority tasks are N/A for it too.)

It plays by the same rules as everyone else: the same scripts, the same
grader, and no arm-specific code in `src/grade.rs`. It also has the same
author as the benchmark. Read its row with that in mind, and check the
receipts.

## How it runs

It runs in-process as the `engram` arm (`src/arms/`) with engram-core at
the revision `Cargo.toml` pins. It uses the same embedder as every other
arm (`BAAI/bge-small-en-v1.5`), plus its own reranker
(`jina-reranker-v1-turbo-en`) and contradiction judge
(`deberta-v3-small-tasksource-nli`). `--nli-dir` swaps the judge for any
export engram-core can load.

## Releases measured

Each release is re-measured on the frozen v2 worlds. The baselines don't
load the engram engine, so they aren't re-run for a new release.

| release | engram-core | score @500 (3 seeds) | score @1500 | receipts |
|---|---|---|---|---|
| **0.10.3** | `c4d83dc` | **815 (813–818)** | **800** | [`engram-0.10.3/`](../results/v2/reference-arms/engram-0.10.3/) |
| 0.9.9 | `ffbcd1a` | 815 (813–818) | 800 | [`engram-0.9.9/`](../results/v2/reference-arms/engram-0.9.9/) |
| 0.9.9, Laya int4 judge | `ffbcd1a` | 820 (817–827) | — | [`engram-0.9.9-laya-int4/`](../results/v2/reference-arms/engram-0.9.9-laya-int4/) |
| 0.9.6 | `2c760e9` | 816 (813–818) | 801 | [`reference-arms/`](../results/v2/reference-arms/) |

What moved between releases, column by column, is in
[`RESULTS.md`](../RESULTS.md).

## Reproduce

```sh
cargo build --release --features fastembed
results/v2/reference-arms/engram-0.10.3/run.sh   # 500 × 3 seeds + 1500, ~75 min on a laptop CPU
```

To measure a new release: bump the `engram-core` revision in `Cargo.toml`,
copy the previous release's `run.sh` into a new `engram-<version>/`
directory, run it, and add a row above.
