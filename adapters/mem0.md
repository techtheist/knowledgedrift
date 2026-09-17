# Mem0 — KnowledgeDrift adapter

`mem0_adapter.py` scores the open-source `mem0ai` package as a **raw,
LLM-free store**: `add(infer=False)`, Mem0's embedded Qdrant with its own
hybrid dense + BM25 scoring, the benchmark's reference embedder
(`BAAI/bge-small-en-v1.5`, 384 dims, local), native range filters on a
stored capture time. **What Mem0 sells — LLM fact extraction, LLM-decided
add/update/delete, LLM-built graph memory — is out of scope for this
offline, judge-free benchmark and is not measured here.** The row reads
"Mem0 as a vector memory with hybrid search", nothing more.

## Setup

```sh
cd adapters
python3.14 -m venv .venv-mem0                    # gitignored
.venv-mem0/bin/pip install mem0ai sentence-transformers fastembed

# one-time model prefetch (the only step that touches the network;
# the adapter itself runs with HF_HUB_OFFLINE=1 and a socket guard)
.venv-mem0/bin/python3 -c "from sentence_transformers import SentenceTransformer; SentenceTransformer('BAAI/bge-small-en-v1.5')"
.venv-mem0/bin/python3 -c "from fastembed import SparseTextEmbedding; SparseTextEmbedding('Qdrant/bm25')"

# replay + grade (grader from the repo root)
.venv-mem0/bin/python3 run.py --adapter mem0 \
    --script ../worlds/v2/knowledgedrift-100-seed1-v2.json --out out/mem0-100.json
cd .. && cargo run --release -- \
    --grade adapters/out/mem0-100.json \
    --script worlds/v2/knowledgedrift-100-seed1-v2.json \
    --json adapters/out/mem0-100-graded.json
```

Adapter kwargs (`--kw k=v`): `store=<dir>` (default `out/mem0-store`,
wiped on every `reset()`), `device=cpu` (sentence-transformers device).

## Version pins (what the numbers below were measured on)

| component | version |
|---|---|
| Python | 3.14.6 (Homebrew; the only ≥3.10 interpreter on the box — every wheel below has a 3.14 build) |
| mem0ai | **2.0.20** |
| qdrant-client | 1.19.0 (embedded local mode, `path=` under `out/`) |
| sentence-transformers / transformers / torch | 6.0.1 / 5.17.0 / 2.14.0 (CPU) |
| fastembed / onnxruntime | 0.8.0 / 1.30.0 — only for Mem0's `Qdrant/bm25` sparse encoder, not for the dense embedder |
| openai / posthog / pydantic / numpy | 3.13.0 / 7.53.0 / 2.13.5 / 2.5.3 (pulled by mem0ai; neither client is ever used) |
| embedder weights | `BAAI/bge-small-en-v1.5` HF snapshot `5c38ec7c405ec4b44b94cc5a9bb96e735b38267a`, fp32 PyTorch, `Transformer → Pooling → Normalize` |

## Configuration handed to `Memory.from_config`

```python
{"vector_store": {"provider": "qdrant", "config": {"collection_name": "knowledgedrift",
                  "embedding_model_dims": 384, "path": "out/mem0-store/qdrant", "on_disk": False}},
 "embedder": {"provider": "huggingface", "config": {"model": "BAAI/bge-small-en-v1.5",
              "embedding_dims": 384, "model_kwargs": {"device": "cpu"}}},
 "llm": {"provider": "openai", "config": {"model": "never-called",
         "api_key": "sk-knowledgedrift-no-llm", "openai_base_url": "http://127.0.0.1:9/v1"}},
 "history_db_path": "out/mem0-store/history.db"}
```

Environment set by the adapter module before `import mem0`:
`MEM0_TELEMETRY=False`, `MEM0_DIR=out/mem0-home` (keeps `~/.mem0`
untouched), `HF_HUB_OFFLINE=1`, `TRANSFORMERS_OFFLINE=1`.

## Operation → Mem0 API

| op | Mem0 call | notes |
|---|---|---|
| `reset()` | close the previous `Memory`, `rm -rf` the store dir, `Memory.from_config(...)` | asserts the collection count is 0 afterwards |
| `inscribe(record, mode)` | `add(f"{title}\n{body}", user_id="knowledgedrift", metadata={key, kind, created_at: ISO, created_unix: int}, infer=False)` | one raw memory per note; `mode` ignored (import and write take the same path — Mem0 has no write-time check without an LLM); a still-live duplicate key raises |
| `link(from, to, verb)` | nothing | `link: false` |
| `supersede(old, new)` | `update(id_of(old), text=new_text, metadata=metadata(new))` | the id survives; the map now points `new.key` at it and forgets `old` |
| `release(key, reason)` | `delete(id_of(key))` | the reason has nowhere to go |
| `purge(key)` | `delete(id_of(key))` | identical to release |
| `settle()` | nothing | returns `None` |
| `recall(query, k, window)` | `search(query, filters={"user_id": …, "created_unix": {"gte": after, "lt": before}}, top_k=k)` | Mem0's default `threshold=0.1` kept; hit `text` = the `memory` field verbatim, `score` = Mem0's blended score, `created_at` = the stored `created_unix`, `key` from the id map (cross-checked against the payload's `key`) |
| `suspects()` | — | `None` |
| `lineage(key)` | — | `None` |
| `standing_tokens()` | — | 0 |

## Capabilities declared

| capability | value | why |
|---|---|---|
| `link` | false | Mem0 OSS has no edges without its LLM-driven graph memory |
| `history` | false | `update()` keeps the id and overwrites the text; `history()` is a change log of one id (ADD/UPDATE/DELETE rows), not a chain a reader can walk from a note, and the adapter does not fake one from its own key map |
| `trace` | false | `delete()` leaves nothing findable |
| `suspects` | false | no conflict queue |
| `temporal` | **true** | the capture time is a natively stored payload field and the window is a native Qdrant `Range` filter (`gte`/`lt`, half-open like the grader's) applied inside the vector search — no over-fetch-and-filter in the adapter |
| `verdict` | false | `search()` never declines |
| `write_check` | false | `add(infer=False)` stores silently; `Inscribed()` is returned empty |

## Deviations and shims — read these before the numbers

1. **No LLM, verified.** `add(infer=False)` never reaches Mem0's extraction
   pipeline. `Memory()` still insists on constructing an LLM client, so it
   gets a dummy key and `openai_base_url=http://127.0.0.1:9/v1` (a loopback
   port nothing listens on). On top of that the adapter monkeypatches
   `socket.socket.connect` / `connect_ex` / `socket.create_connection` to
   raise `NetworkAttempted` for the life of the process, and the run
   completed under that guard — so nothing in the replay opened a socket.
   A direct `llm.generate_response()` from the probe script dies with
   `APIConnectionError`; `socket.create_connection` dies with the guard's
   exception. Telemetry (PostHog), the remote notice config fetch, and
   Mem0's `~/.mem0/migrations_qdrant` telemetry store are all disabled by
   `MEM0_TELEMETRY=False`.
2. **Capture time is written into Mem0's own `created_at`.** Mem0's
   `_create_memory` honours a caller-supplied `created_at` in metadata, so
   the memory's native timestamp is the script's capture time (ISO 8601
   UTC), not the wall clock. A numeric twin `created_unix` is stored beside
   it for the range filter (`MemoryItem.created_at` is typed `str`, so the
   native slot cannot hold an int). **After `supersede`, Mem0's `update()`
   pins its own `created_at` to the original insert; the adapter's
   `created_unix` is updated to the new record's capture time and that is
   what the hit reports.** The temporal family is probed on the untouched
   world, so this only matters for chain heads.
3. **The temporal filter is native, not a shim.** `{"created_unix":
   {"gte": after, "lt": before}}` goes through Mem0's advanced-filter path
   into a Qdrant `FieldCondition(range=Range(...))`; either bound may be
   absent. Verified in embedded Qdrant on a three-note probe (two-sided and
   one-sided windows both exclude correctly). Mem0's `_search_vector_store`
   itself over-fetches `max(4k, 60)` candidates before scoring — that is
   Mem0's own pipeline, not the adapter's.
4. **Hybrid search is on.** mem0ai 2.0.20 scores `(semantic + BM25) / 2`
   when the Qdrant collection carries a `bm25` sparse slot, which needs the
   `fastembed` package for its `Qdrant/bm25` encoder (optional `extras`
   group, same group as sentence-transformers). It is installed, so the
   arm is Mem0's full default scoring. Without it Mem0 silently degrades
   to dense-only; that variant was not run. **spaCy (`mem0ai[nlp]`) is not
   installed**: Mem0's lemmatizer falls back to the raw text (fastembed's
   BM25 does its own tokenizing/stemming/stop-words) and the entity-boost
   term is always 0 — it would be anyway, since entities are only linked
   on the LLM path and on `update()`, and both need spaCy. Mem0 logs one
   warning per process for each.
5. **Mem0's default `threshold=0.1`** on the semantic score is kept: hits
   under 0.1 cosine are dropped before blending. Nothing in this world
   comes near it (417 of 418 recalls returned the full 10; the one
   nine-hit reply, R401, is a window with exactly nine notes inside it and
   its lowest score was 0.24), so `fp` stays 1.00.
6. **Embedder runtime differs from the in-process arms'.** They embed
   with fp32 ONNX weights through the `fastembed` crate; this arm loads the
   same fp32 weights through sentence-transformers/PyTorch on CPU. Same
   model, same normalisation, float-noise differences. Mem0's own
   `fastembed` embedder provider was not used because fastembed-python's
   `BAAI/bge-small-en-v1.5` entry resolves to the int8-quantized ONNX
   export by default.
7. **Whole notes are what Mem0 shows.** `text` on a hit is the stored
   memory verbatim (`title\nbody`), so the attention columns bill the
   whole note — that is what a Mem0 caller reads.
8. `release` and `purge` are the same `delete()`; the release reason is
   dropped because Mem0 has no field for it and no marker to attach it to.
9. **Python 3.14.** Only 3.14 and the system 3.9 were available; mem0ai
   needs ≥3.10 and every dependency had a 3.14 wheel, so no interpreter was
   installed for this.

## Results — 500 and 1500 tested facts, seed 1

Receipts under `results/v2/mem0-2.0.20/` (`500-seed1.json`,
`1500-seed1.json`, the `.log` beside each). One Apple-silicon laptop, CPU
embedder, the three external chains replaying side by side so wall-clock is
contended.

```
  arm       posed attempted passed  success   of att.  families  signal  tokens   score   billed
  mem0       3042      2754   1427      47%       52%       334       7       6     347     2567
    retrieval      2063   56%  r@1 0.48  r@5 0.59  lexical_r@5 1.00  paraphrase_r@5 1.00  oblique_r@5 0.29  crossed_r@5 0.06  path_r@5 0.65  path_cover 0.33  stale_above 0.30  hedge 0.00  noise 0.93
    abstention      238    0%  fp 1.00  phantom_fp 1.00  natural_fp 1.00  answered 1.00  declined 0.00  separation 0.59
    currency         75   73%  head_r@1 0.72  head_r@5 0.73  pollution 0.00
    contradiction     -  n/a — no suspect nomination
    drift             -  n/a — no suspect nomination
    deletion         83  100%  released_gone 1.00  purged_gone 1.00  resurrection_warned 0.00  purged_rewrite_warned 0.00
    rationale       170    5%  direct_r@5 0.05  assisted_r@5 0.05  structure_only 0.00
    temporal        125  100%  in_window_r@5 1.00  leak 0.00
```

At 1500: **44% success, score 348** (families 333, signal 6, tokens 8,
2,438 billed tokens a query); retrieval 52% (oblique 0.18, crossed 0.03,
path 0.61 / 0.18), currency 74%, rationale 7%, temporal 100%.

Per-op means from the 500 transcript: inscribe 39 ms, supersede 45 ms,
recall 42 ms, the path read 37 ms, release/purge ~2 ms;
link/settle/lineage/suspects are no-ops. At 1500 recall grows to 87 ms —
embedded Qdrant scans vectors in NumPy and the BM25 sparse query is a
Python-side scan, so recall cost grows with the collection.

Reading it, next to the in-process arms on the same script (`rag` 376 at
53%, `grep` 335 at 43%, the reference system 816 at 73%):

- **Retrieval 56%** — lexical and paraphrase both 1.00, oblique 0.29,
  crossed 0.06. Hybrid BM25 pays on the surface forms and costs on the
  questions that share no words with the note: eight points under `rag`
  on the same embedder. `stale_above 0.30`: on polluted subjects the
  untouched stale sibling outranks the gold 30% of the time — a flat store
  has no reason to prefer one.
- **Abstention 0%** — `phantom_fp` and `natural_fp` 1.00, as expected:
  Mem0 never declines and the 0.1 semantic threshold never bites.
- **Currency 73%** — `update()` in place means the retired generation is
  gone, so no pollution; the head misses on the oblique question. Lineage
  is N/A (`history: false`).
- **Deletion 100% on absence**, 0 on warnings — deleted is deleted, and a
  write-back is just another `add`.
- **Rationale 5%** — no edges, so only rationale notes that are lexically
  close to the question surface (`structure_only 0.00`).
- **Temporal 100%** — the native range filter delivers the gold inside the
  window with zero leak.
- **Signal 7, tokens 6** — whole notes, ten of them per question: the
  largest bill of the flat stores (2,567 tokens a query).

## Files

- `mem0_adapter.py` — the adapter
- `.venv-mem0/` — the venv (gitignored)
- `out/mem0-*.json` — transcripts and graded files (scratch, gitignored; the graded receipts that count live in `results/v2/`)
- `out/mem0-smoke.json` — the 272-op smoke (first 40 recalls; the grader rightly refuses it at probe R41)
- `out/mem0-store/`, `out/mem0-home/` — Mem0's embedded Qdrant + history.db, and its `MEM0_DIR` (config.json only); both scratch, gitignored
