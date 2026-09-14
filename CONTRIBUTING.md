# Contributing

Two kinds of pull request are expected: **an adapter** for a memory system,
and **a result** for a system on the v1 worlds. Anything else — a new
family, a generator change, a scoring change — is a v2 conversation; open
an issue first.

## Submitting an adapter

An adapter is one file, `adapters/<name>_adapter.py`, with a class
`Adapter(MemoryAdapter)` (see `adapters/adapter.py` for the protocol and
`adapters/langmem_adapter.py` for a small worked example), plus a notes
file `adapters/<name>.md`. The notes file is not optional; it is half the
submission. It states:

1. **Setup** — exact install commands into a gitignored venv
   (`adapters/.venv-<name>`), the model prefetch if any, and the replay +
   grade commands.
2. **Version pins** — every package version the numbers were measured on,
   the embedder weights (name, revision, precision, runtime).
3. **Operation → API mapping** — one row per protocol operation saying
   which native call it became, and what was dropped (a release reason
   with nowhere to go, a link a flat store cannot keep).
4. **Capabilities declared, with the why** — one row per capability. A
   capability is declared only if the system honestly has it: `temporal`
   means the window is applied natively by the store, not by the adapter
   filtering a wider fetch; `history` means a reader could walk from a note
   to what it replaced; `trace` means a release leaves something findable.
5. **Deviations and shims** — every place the adapter did something the
   system's own caller would not, and every place the system was
   configured away from its defaults. An LLM-driven system run without its
   LLM says so in the first paragraph.

Rules the runner enforces and a review checks:

- **Same embedder where possible.** The v1 tables embed with
  `BAAI/bge-small-en-v1.5` everywhere, so a difference in a table is a
  difference in mechanism. If your system cannot take an external embedder,
  say which one it used and expect the review to read your retrieval
  columns against `rag` with that in mind.
- **No LLM in the loop.** The benchmark is judge-free and offline. An
  adapter may not call a language model during replay; a system whose
  memory layer is inseparable from one is out of scope for v1.
- **Keys are yours to map.** The harness addresses notes by its own key;
  the adapter keeps the key → native id map and never resolves a target by
  searching for it.
- **Exact text.** A hit's `text` is what the system would show a caller —
  a snippet if it snippets, the whole note if it does not. That is what the
  attention columns bill; trimming it in the adapter is a shim and must be
  declared.
- **Raise, never record silence.** An adapter error must raise. The runner
  never writes an empty reply for a failure, because an empty reply is
  indistinguishable from an honest "the store is silent" and the grader
  would score it as one.
- **Determinism.** Two replays of the same script should grade the same.
  Seed whatever your system seeds.

## Submitting a result

A result is a directory `results/v1/<system>-<version>/` containing:

- the graded receipts, one per world: `<size>-seed<N>.json` as written by
  `knowledgedrift --grade … --json` (the transcript itself is not
  committed — they run to tens of megabytes — but keep it; a reviewer may
  ask for one);
- a `README.md` with the environment (machine, OS, Python and Rust
  versions), the commit of this repository and of the adapter, the exact
  commands, wall-clock per world, and the `table.py` rows;
- the adapter notes, or a pointer to `adapters/<name>.md` if the adapter is
  already merged.

To be quoted in the leaderboard a result needs **the official ladder**:
500 and 1500 tested facts on seed 1, and **three seeds at 500** (seeds 1, 2
and 3, all shipped under `worlds/v1/`). A single-seed result is accepted as
preliminary and labelled so. Any receipt is regraded by the reviewer from
the repository's grader before it is merged; a receipt whose script digest
is not one of the v1 digests is not a v1 result.

The leaderboard row states the system **and its version**, the
capabilities it declared, success with its three-seed spread, composite,
score, tokens per answer, and a one-line description of what was actually
exercised (for LangMem: "the LangGraph store with its semantic index";
for Mem0: "`infer=False`, hybrid BM25 on"). A system's marketing name
alone is not a description.

## What a review looks for

- The capabilities match the mechanism. `trace: true` with nothing
  findable after a release, or `temporal: true` with an adapter-side
  filter, is corrected before merge.
- The notes file explains every N/A and every shim.
- The numbers are reproducible: the reviewer replays at least the 100
  world (`worlds/v1/knowledgedrift-100-seed1.json`) with the submitted
  adapter and expects the graded families to match.
- Nothing in the adapter reads the probes. The runner does not expose
  them; an adapter that opens the script file itself is refused.

## Developing the harness

```sh
cargo test                          # the grader, the generator, the golden v1 digests — no models
cargo test --features arms          # + the in-process arms on fake models
cargo clippy --all-targets --features arms
cargo fmt --check
```

The in-process arms depend on the `engram-core` crate (git, pinned by
revision in `Cargo.toml`); to develop against a local checkout, add an
uncommitted `.cargo/config.toml`:

```toml
[patch."https://github.com/techtheist/engram"]
engram-core = { path = "../engram/crates/engram-core" }
```

`worlds/v1/*.json` are frozen. If a generator change alters any of their
digests, `the_v1_worlds_regenerate_from_this_crate` fails, and that is the
signal that the change is a v2.
