# KnowledgeDrift adapters

External memory systems are scored the same way the in-process arms are:
replay the exported script, hand the transcript to the Rust grader.

All paths below are relative to the repository root.

```sh
# 1. the worlds are in worlds/v2/ (they regenerate byte-identical with
#    `cargo run -- --v2 --sizes 100,500,1500 --export worlds/v2`)

# 2. replay through an adapter (each has its own venv — see its section)
adapters/.venv-langmem/bin/python3 adapters/run.py \
    --adapter langmem --script worlds/v2/knowledgedrift-500-seed1-v2.json \
    --out adapters/out/langmem-500.json

# 3. grade
cargo run --release -- --grade adapters/out/langmem-500.json \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json \
    --json adapters/out/langmem-500-graded.json

# 4. table rows for a results README
python3 adapters/table.py adapters/out/langmem-500-graded.json
```

A smoke replay (`--limit-ops N`) needs N past the first recall — the first
~250 ops of every world are imports and links — and its partial transcript
makes the grader complain about missing replies; inspect the JSON instead.
The grader poses fewer tasks than the script has probes: the rewrite of a
purged note is a column, never a task.

`adapter.py` is the protocol (twelve operations, the hit shape, capabilities);
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

- **cognee** — `cognee_adapter.py`, notes in `cognee.md`: cognee 1.5.4 with the LLM left out (the `cognify()` pipeline minus its extraction task: classify → chunk → `add_data_points`, `CHUNKS` search), cognee's own `fastembed` provider with bge-small, its default LanceDB + LadybugDB stores; no window on its chunk search, so temporal is N/A beside every other capability.
- **langmem** — `langmem_adapter.py`, notes in `langmem.md`: LangMem's memory layer (the LangGraph store with its semantic index, the three calls its tools make) with the same bge-small embedder; temporal native, everything else N/A.
- **memcontinuum** — `memcontinuum_adapter.py`, notes in `memcontinuum.md`: MemContinuum (decision chains over append-only markdown topics, SQLite FTS5 + bge-small via fastembed, RRF hybrid search) driven in-process through its own `memidx` commands; history native (the chain), the owner's endorsement native (a promotion link), everything else N/A.
- **mem0** — `mem0_adapter.py`, notes in `mem0.md`: Mem0 OSS (mem0ai 2.0) with `infer=False` (no LLM anywhere; the mandatory client is pointed at an unreachable address and the run is socket-guarded), local Qdrant, hybrid BM25 on, bge-small fp32 via sentence-transformers (fastembed-python's bge-small entry is the int8 export — the one place the "same embedder" line above needs that footnote); temporal native via a numeric capture-time field, everything else N/A.
- **supermemory** — `supermemory_adapter.py`, notes in `supermemory.md`: Supermemory local (the self-hosted `supermemory-server` 0.0.8 binary, started and stopped by the adapter) with the LLM left out — notes written straight into the memory graph through `POST /v4/memories`, never through the document pipeline; the server will not boot without a provider key, so it gets a dummy one aimed at a tripwire that fails the run on any LLM call (0 on every receipt); its local embedder switched to bge-small (the int8 export — the same footnote as mem0's); `/v4/search` at the server's defaults, similarity floor 0.6 included; stdlib-only, no venv; history native (the versioned `PATCH`, read back through `include.relatedMemories`), temporal native (a numeric metadata filter), everything else N/A.
