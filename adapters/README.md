# KnowledgeDrift adapters

External memory systems are scored the same way the in-process arms are:
replay the exported script, hand the transcript to the Rust grader.

All paths below are relative to the repository root.

```sh
# 1. the v1 worlds are in worlds/v1/ (they regenerate byte-identical with
#    `cargo run -- --sizes 100,500,1500 --export worlds/v1`)

# 2. replay through an adapter (each has its own venv — see its section)
adapters/.venv-langmem/bin/python3 adapters/run.py \
    --adapter langmem --script worlds/v1/knowledgedrift-500-seed1.json \
    --out adapters/out/langmem-500.json

# 3. grade
cargo run --release -- --grade adapters/out/langmem-500.json \
    --script worlds/v1/knowledgedrift-500-seed1.json \
    --json adapters/out/langmem-500-graded.json

# 4. table rows for a results README
python3 adapters/table.py adapters/out/langmem-500-graded.json
```

A smoke replay (`--limit-ops N`) needs N past the first recall — the first
~250 ops of every world are imports and links — and its partial transcript
makes the grader complain about missing replies; inspect the JSON instead.
The grader poses fewer tasks than the script has probes: the rewrite of a
purged note is a column, never a task.

`adapter.py` is the protocol (eleven operations, the hit shape, capabilities);
`run.py` is the loop. An adapter is one file, `<name>_adapter.py`, with a
class `Adapter(MemoryAdapter)`. It maps keys to native ids itself, returns
exactly the text its system would show a caller, declares only the
capabilities it honestly has, and raises on any error — the runner never
records an empty reply for a failure.

Every arm below embeds with the same model the in-process arms use
(`BAAI/bge-small-en-v1.5`), so a difference in the tables is a difference
in mechanism, not in embedder. `CONTRIBUTING.md` says what a submitted
adapter and its results need to carry.

## Arms

- **langmem** — `langmem_adapter.py`, notes in `langmem.md`: LangMem's memory layer (the LangGraph store with its semantic index, the three calls its tools make) with the same bge-small embedder; temporal native, everything else N/A.
- **memcontinuum** — `memcontinuum_adapter.py`, notes in `memcontinuum.md`: MemContinuum (decision chains over append-only markdown topics, SQLite FTS5 + bge-small via fastembed, RRF hybrid search) driven in-process through its own `memidx` commands; history native (the chain), the owner's endorsement native (a promotion link), everything else N/A.
- **mem0** — `mem0_adapter.py`, notes in `mem0.md`: Mem0 OSS (mem0ai 2.0) with `infer=False` (no LLM anywhere; the mandatory client is pointed at an unreachable address and the run is socket-guarded), local Qdrant, hybrid BM25 on, bge-small fp32 via sentence-transformers (fastembed-python's bge-small entry is the int8 export — the one place the "same embedder" line above needs that footnote); temporal native via a numeric capture-time field, everything else N/A.
