# cognee — KnowledgeDrift adapter

`cognee_adapter.py` scores the open-source `cognee` package (1.5.4) as an
**LLM-free document store with chunk search**. What cognee sells — the
`cognify()` step that has a language model extract entities, relationships
and summaries into a knowledge graph, and the completion searches that walk
it — is out of scope for this offline, judge-free benchmark and is **not
measured here**. What is measured is the layer that step sits on and that
cognee's own `SearchType.CHUNKS` reads: documents added through
`cognee.add`, run through cognee's ingestion pipeline with the LLM task
left out (classify → chunk → `add_data_points`), embedded by cognee's
`fastembed` provider with the benchmark's reference model, stored in
cognee's default LanceDB vector store and LadybugDB graph store, and
ranked by cognee's own chunk retriever. The row reads "cognee as a chunk
store with vector search", nothing more.

## Setup

```sh
cd adapters
python3.14 -m venv .venv-cognee                     # gitignored
.venv-cognee/bin/pip install "cognee[fastembed]==1.5.4"

# one-time model prefetch (the only step that touches the network; the
# adapter runs with HF_HUB_OFFLINE=1 and a socket guard) — fastembed
# caches under $TMPDIR/fastembed_cache unless FASTEMBED_CACHE_PATH is set
.venv-cognee/bin/python3 -c "from fastembed import TextEmbedding; TextEmbedding('BAAI/bge-small-en-v1.5')"

# replay + grade (from the repository root)
cd ..
adapters/.venv-cognee/bin/python3 adapters/run.py --adapter cognee \
    --script worlds/v2/knowledgedrift-100-seed1-v2.json --out adapters/out/cognee-100.json
cargo run --release -- \
    --grade adapters/out/cognee-100.json \
    --script worlds/v2/knowledgedrift-100-seed1-v2.json \
    --json adapters/out/cognee-100-graded.json
```

The store lives under `adapters/out/cognee-store/` (data files, the SQLite
metadata db, LanceDB, LadybugDB, logs), wiped on every `reset()`; set
`KD_COGNEE_STORE=<dir>` before the run to move it. The adapter takes no
constructor kwargs.

## Version pins (what the numbers below were measured on)

| component | version |
|---|---|
| Python | 3.14.6 (Homebrew; cognee accepts `>=3.10,<3.15`) |
| cognee | **1.5.4** (PyPI; the source read for this adapter is commit `c0d18c8` of the same version) |
| fastembed / onnxruntime | 0.8.0 / 1.30.0 — cognee's `fastembed` embedding provider |
| lancedb / pylance | 0.38.0 / 0.36.0 — the default vector store |
| ladybug | 0.19.0 — the default graph store in 1.5.4 (the Kùzu successor cognee pins) |
| litellm / openai / instructor | 1.96.2 / 2.54.0 / 1.15.1 — pulled by cognee; the LLM client is constructed and never called |
| SQLAlchemy / aiosqlite | 2.0.53 — the metadata store (`cognee_db`, SQLite) |
| numpy / tiktoken / tokenizers | 2.5.3 / 0.14.0 / 0.23.2 |
| embedder weights | `BAAI/bge-small-en-v1.5` as fastembed 0.8.0 registers it (`qdrant/bge-small-en-v1.5-onnx-q`, the ONNX export in fastembed's registry — the same package and cache the LangMem and MemContinuum adapters use), 384 dims, CPU |

## Configuration

Set in the environment by the adapter module before `import cognee`
(cognee's settings objects read the environment once and cache):

| variable | value | why |
|---|---|---|
| `COGNEE_SKIP_CONNECTION_TEST` | `true` | every first pipeline run otherwise makes a real LLM completion as a connectivity probe (30 s timeout, then an error); it also skips the provider preflight |
| `CACHING` / `AUTO_FEEDBACK` | `false` / `false` | cognee 1.x runs a "session turn analysis" — an LLM structured-output call — on **every search** while its session cache is on; with the cache off the search path has no LLM step |
| `ENABLE_BACKEND_ACCESS_CONTROL` | `false` | on by default for the LanceDB + LadybugDB pair; off = one store rather than one database per dataset, and a flat result list |
| `LLM_PROVIDER` / `LLM_MODEL` / `LLM_API_KEY` / `LLM_ENDPOINT` | `openai` / `never-called` / a dummy key / `http://127.0.0.1:9/v1` | `add()` insists on a consistent LLM configuration; the endpoint is a loopback port nothing listens on and the socket guard would raise before a packet left |
| `EMBEDDING_PROVIDER` / `EMBEDDING_MODEL` / `EMBEDDING_DIMENSIONS` | `fastembed` / `BAAI/bge-small-en-v1.5` / `384` | cognee's local embedding provider with the reference model; the dimension is set explicitly (cognee's fallback is 3072) |
| `KUZU_MAX_DB_SIZE` | `274877906944` (2³⁸) | LadybugDB's buffer-manager cap — virtual address space, not disk; the 1500 world exhausted cognee's 32 GB default after 71 minutes ("Maximum database size … has been reached") with a 34 MB graph on disk, because every pipeline pass and delete reopens the store's mapping |
| `TELEMETRY_DISABLED` / `COGNEE_TRACING_ENABLED` | `1` / `false` | no product telemetry, no OpenTelemetry |
| `HF_HUB_OFFLINE` / `TRANSFORMERS_OFFLINE` | `1` / `1` | no model downloads during a run |
| `DATA_ROOT_DIRECTORY` / `SYSTEM_ROOT_DIRECTORY` / `CACHE_ROOT_DIRECTORY` / `COGNEE_LOGS_DIR` | under `out/cognee-store/` | cognee's defaults are inside the installed package directory and `~/.cognee` |
| `LOG_LEVEL` / `COGNEE_LOG_FILE` | `WARNING` / `false` | cognee logs every pipeline task at INFO and rotates a log file by default |

Everything else is cognee's default: LanceDB vector store, LadybugDB graph
store, SQLite metadata, `TextChunker`, cognee's own chunk retriever with
its cosine distance.

## Operation → cognee API

| op | cognee call | notes |
|---|---|---|
| `reset()` | `rm -rf` the store dir, `config.data_root_directory` / `system_root_directory`, `prune.prune_data()`, `prune.prune_system(metadata=True)` | |
| `inscribe(record, mode)` | `add(DataItem(data=f"{title}\n{body}", label=key, data_id=uuid5(key), external_metadata={key, kind, created_at, code_refs}), dataset_name="knowledgedrift", node_set=[key])` | one document per note; the adapter picks the data id (`DataItem.data_id`) so a key resolves without a lookup; the key is also the note's `NodeSet`, which is how a chunk hit names its note (`belongs_to_set` on the payload); `mode` ignored (import and write take the same path — cognee has no write-time check without an LLM); a still-live duplicate key raises |
| *(lazy index pass)* | `run_pipeline(tasks=[classify_documents, extract_chunks_from_documents(max_chunk_size=512, TextChunker), add_data_points], datasets=["knowledgedrift"], pipeline_name="cognify_pipeline", incremental_loading=True)` | cognee's `cognify()` task list with `extract_graph_and_summarize` (the LLM task) removed — the same three tasks its DLT and code routes run; executed once per write→read boundary (on `recall` / `settle` after any add), and cognee's incremental loading skips every document the pipeline already processed |
| `link(from, to, verb)` | nothing | `link: false` — cognee's edges come from the LLM extraction |
| `supersede(old, new)` | `datasets.delete_data(old)` then `add(new)` | replaced in place; nothing links the generations |
| `release(key, reason)` | `datasets.delete_data(dataset_id, data_id, mode="soft")` | the document, its chunks, its graph nodes and its vector rows go; the reason has nowhere to go |
| `purge(key)` | the same `delete_data` | identical to release |
| `endorse(key, by)` | nothing, returns False | no rung (see below) |
| `settle()` | the lazy index pass, if anything was added | returns `None` |
| `recall(query, k, window)` | `search(query, query_type=SearchType.CHUNKS, datasets=["knowledgedrift"], top_k=k, only_context=True, verbose=True)` | cognee's chunk retriever: one LanceDB cosine search over `DocumentChunk_text`; hit `text` = the chunk's text (the whole note — every note is one chunk), `score` = the negated cosine distance (cognee's `ScoredResult.score` is a distance, lower is better; the grader wants higher = better), `key` from the chunk's `belongs_to_set`; the window is **ignored** (`temporal: false`) |
| `suspects()` | — | `None` |
| `lineage(key)` | — | `None` |
| `standing_tokens()` | — | 0 |

## Capabilities declared

| capability | value | why |
|---|---|---|
| `link` | false | cognee's graph edges are produced by the LLM extraction task, which does not run; the graph store holds Document → Chunk → NodeSet structure only |
| `history` | false | `supersede` is delete + add; cognee's `update()` keeps the data id but its fallback path calls `cognify()` (LLM), so it is not used, and nothing links a chunk to what it replaced |
| `trace` | false | `delete_data` leaves nothing findable |
| `suspects` | false | cognee's `detect_contradictions` is an LLM task (and off by default) |
| `temporal` | false | cognee's vector search filters only by node set (`belongs_to_set`); there is no native time-range filter on the chunk path, and the adapter does not over-fetch-and-filter — the window is dropped and the family is N/A |
| `verdict` | false | search never declines |
| `write_check` | false | `add()` stores silently |
| `endorse_*` | false ×4 | cognee stores an `importance_weight` (write-time, default 0.5) and a `feedback_weight` (moved by its LLM feedback loop) on every data point; neither is an endorsement a caller places on an existing note through a non-LLM API, and the chunk retriever reads neither |

## Deviations and shims — read these before the numbers

1. **No LLM, verified.** Four of cognee's paths would reach a language
   model and each is closed: the first-run connection probe
   (`COGNEE_SKIP_CONNECTION_TEST`), the `cognify()` extraction task (the
   adapter runs cognee's pipeline with that task removed — a public
   `Task` list through the same `run_pipeline` executor `cognify()` uses,
   under the same `cognify_pipeline` name so cognee's own incremental
   bookkeeping applies), the per-search session-turn analysis (`CACHING`
   off; the smoke run without it retried a structured-output call for four
   minutes per search), and `only_context=True` on the search call, which
   skips the session-turn preparation even when the cache is on. On top of
   that the adapter monkeypatches `socket.socket.connect` / `connect_ex` /
   `socket.create_connection` to raise `NetworkAttempted` for the life of
   the process for every address family except `AF_UNIX` (cognee's
   LanceDB and LadybugDB workers are local `multiprocessing` children), and
   the runs completed under that guard.
2. **The index pass is cognee's pipeline minus one task.** `cognify()`
   has no `tasks=` argument; its list is `classify_documents` →
   `extract_chunks_from_documents` → `extract_graph_and_summarize` →
   `add_data_points`. The adapter runs the same list without the third
   task, through the same executor, under the same pipeline name. That is
   the shape cognee itself uses for its no-LLM routes (`get_dlt_tasks`,
   the code-graph route), so nothing here is foreign to it — but no
   summaries, entities or relationships exist, and `SearchType.SUMMARIES`
   and every `*_COMPLETION` type are not exercised.
3. **Search is `CHUNKS` with `only_context=True, verbose=True`.**
   cognee's `SearchType.CHUNKS` returns bare chunk payloads and drops the
   score; `verbose=True` returns the retriever's `ScoredResult` objects
   (`score` = cosine distance) beside them, and the hit score is that
   distance negated. `only_context=True` skips the completion step, which
   for the chunk retriever is a no-op anyway. cognee's other LLM-free text
   search, `CHUNKS_LEXICAL` (BM25 over chunks loaded from the graph store),
   was not run.
4. **The adapter picks the data id.** `DataItem.data_id = uuid5(key)`, so
   a key never needs a lookup and a re-used key after a delete gets the
   same id back (cognee's `resolve_data_id` accepts a pinned id). A
   still-live key raises.
5. **Chunk sizing uses cognee's tiktoken fallback.** cognee resolves the
   fastembed model's own tokenizer through `transformers`, which is not
   installed (cognee's `huggingface` extra); it logs a warning and counts
   with tiktoken instead. Every note is well under the 512-token budget
   so every note is one chunk either way — the hit is the whole note, as
   with the other flat stores.
6. **Incremental loading is cognee's.** A pass processes only the Data
   rows without a completed `cognify_pipeline` status; a deleted-then-re-
   added id is a new row. Between write→read boundaries the store is
   stale, which is why the pass runs before every recall that follows a
   write — a world has a few hundred such boundaries, and the deletion
   phase alone is one per case.
7. **The release reason is dropped** (no field, no marker); release and
   purge are the same `delete_data`.
8. **Whole notes are what cognee shows.** A chunk is a note here, so the
   attention columns bill the whole note — what a cognee caller reads.
9. **Two cognee warnings are expected and harmless**: "failed to persist
   prune_data record" on `reset()` (the metadata db is created after the
   prune), and "Stored file to clean up was already gone" on delete
   (cognee's data-file refcount, one line per deleted note).
10. **The graph store's size cap is raised** (`KUZU_MAX_DB_SIZE`, see the configuration table); the first 1500 replay died on the default. Nothing else about LadybugDB is changed.
11. **Python 3.14.** Every dependency had a 3.14 wheel (`cognee[fastembed]`
    pins `onnxruntime>=1.24.1` on 3.14); no interpreter was installed for
    this.

## Results — 500 and 1500 tested facts, seed 1

Receipts under `results/v2/cognee-1.5.4/` (`500-seed1.json`,
`1500-seed1.json`, the `.log` beside each). One Apple-silicon laptop, CPU
embedder, the three external chains replaying side by side so wall-clock is
contended.

```
  arm       posed attempted passed  success   of att.  families  signal  tokens   score   billed
  cognee     3042      2629   1455      48%       55%       259       8      14     281     2053
    retrieval      2063   63%  r@1 0.50  r@5 0.66  lexical_r@5 0.99  paraphrase_r@5 0.99  oblique_r@5 0.48  crossed_r@5 0.17  path_r@5 0.62  path_cover 0.32  stale_above 0.26  hedge 0.00  noise 0.92
    abstention      238    0%  fp 1.00  phantom_fp 1.00  natural_fp 1.00  answered 1.00  declined 0.00  separation 0.59
    currency         75   92%  head_r@1 0.77  head_r@5 0.92  pollution 0.00
    contradiction     -  n/a — no suspect nomination
    drift             -  n/a — no suspect nomination
    deletion         83  100%  released_gone 1.00  purged_gone 1.00  resurrection_warned 0.00  purged_rewrite_warned 0.00
    rationale       170    4%  direct_r@5 0.04  assisted_r@5 0.04  structure_only 0.00
    temporal          -  n/a — no capture-time scoping
```

At 1500: **43% success, score 263** (families 240, signal 8, tokens 16,
1,934 billed tokens a query); retrieval 56% (oblique 0.29, crossed 0.09,
path 0.47 / 0.18), currency 80%, rationale 3%.

Per-op means from the 500 transcript: inscribe 119 ms, supersede 306 ms,
release/purge ~1.6 s (cognee's delete walks the graph and the vector
rows), recall 193 ms, the path read 120 ms; the two `settle` calls, which
absorb the bulk imports into the index, took 109 s each (654 s each at
1500). link/lineage/suspects are no-ops.

Reading it, next to the in-process arms on the same script (`rag` 376 at
53%, `grep` 335 at 43%, the reference system 816 at 73%):

- **Retrieval 63%** — cognee's chunk retriever is a cosine top-k over the
  same embedder, so it lands where `rag` does (64%): lexical and paraphrase
  perfect, oblique 0.48, crossed 0.17. `stale_above 0.26`: a flat store has
  no reason to prefer the truth over its untouched stale sibling. The path
  read (0.62 / 0.32) finds the component's notes, never the file.
- **Abstention 0%** — `phantom_fp` and `natural_fp` 1.00: search never
  declines. `separation 0.59` says the distance carries little a caller
  could threshold on.
- **Currency 92%** — delete + add means the retired generation is gone;
  lineage is N/A (`history: false`).
- **Deletion 100% on absence**, 0 on warnings — deleted is deleted.
- **Rationale 4%** — no edges (they would come from the LLM), so only
  rationale notes lexically close to the question surface.
- **Temporal N/A** — the one family the other flat stores attempt and
  cognee cannot: its vector search has no time filter, so the 125 tasks
  count as failed in the headline, which is most of the gap to `rag`.
- **Signal 8, tokens 14** — whole notes, ten per question: the same bill
  as `rag` (~2,000 tokens per query).

## Files

- `cognee_adapter.py` — the adapter
- `.venv-cognee/` — the venv (gitignored)
- `out/cognee-*.json` — transcripts and graded files (scratch, gitignored; the graded receipts that count live in `results/v2/`)
- `out/cognee-store*/` — cognee's data files, SQLite, LanceDB and LadybugDB per run (scratch, gitignored)
