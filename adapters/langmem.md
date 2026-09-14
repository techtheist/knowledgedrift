# langmem — KnowledgeDrift adapter notes

`langmem_adapter.py`, arm name `langmem`. **LLM-free by construction**: the
benchmark cannot run a model, so this arm exercises the memory layer that
LangMem's LLM-facing tools sit on — a LangGraph `BaseStore` with LangMem's
semantic index — through the store API, and nothing else.

## Setup

```sh
cd adapters
python3 -m venv .venv-langmem                      # gitignored (.venv-*)
.venv-langmem/bin/pip install --upgrade pip
.venv-langmem/bin/pip install langmem langgraph fastembed

# first run downloads BAAI/bge-small-en-v1.5 (~130 MB) into the HF cache;
# verified to load afterwards with HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1.

.venv-langmem/bin/python3 run.py --adapter langmem \
    --script ../worlds/v1/knowledgedrift-100-seed1.json --out out/langmem-100.json

cd ..   # repo root
cargo run --release -- --grade adapters/out/langmem-100.json \
    --script worlds/v1/knowledgedrift-100-seed1.json \
    --json adapters/out/langmem-100-graded.json
```

### Version pins (what the numbers below were measured on)

| package | version |
|---|---|
| Python | 3.14.6 (macOS, arm64) |
| langmem | 0.0.30 |
| langgraph | 1.2.11 |
| langgraph-checkpoint | 4.2.0 |
| langchain-core | 1.6.3 |
| fastembed | 0.8.0 |
| onnxruntime | 1.30.0 |
| numpy | 2.5.3 |
| embedder | `BAAI/bge-small-en-v1.5`, 384 dims, fp32 ONNX via fastembed, CPU, local |

## What is exercised, and what is not

LangMem 0.0.30 has two halves:

1. **The memory layer** — a LangGraph `BaseStore` with a semantic index
   (`index={"dims", "embed", "fields"}`). LangMem's own tools reduce to
   exactly three store calls: `create_manage_memory_tool` does
   `store.put(namespace, key, {"content": ...})` / `store.delete(namespace, key)`
   (`langmem/knowledge/tools.py` ~L328–336), and `create_search_memory_tool`
   does `store.search(namespace, query=, filter=, limit=, offset=)` (~L465).
   **This is what the adapter exercises**, with
   `langgraph.store.memory.InMemoryStore` — the store LangMem's own quickstart
   uses — constructed as
   `InMemoryStore(index={"dims": 384, "embed": <fastembed callable>, "fields": ["text"]})`.
2. **The LLM half** — `create_memory_manager` / `create_memory_store_manager`
   (extraction, consolidation, "update vs insert" decisions),
   `create_prompt_optimizer`, and the LLM deciding *when* to call the manage /
   search tools. **Out of scope**: it needs a chat model, and the benchmark has
   none. Consequently nothing here is extracted, merged, or judged; each script
   note lands as one store item verbatim.

The same embedder the in-process arms use, so a difference against them is
a difference in mechanism, not in embedding.

## Operation → API mapping

| op | adapter call | note |
|---|---|---|
| `reset()` | `InMemoryStore(index=…)` fresh; key set cleared | one embedder per process, one store per run |
| `inscribe(record, mode)` | `store.put(("kd",), key, {"text": title+"\n"+body, "kind", "created_at", "key"})` | `import` and `write` are the same call — the store has no write-time checks; returns the silent default verdict `{}` |
| `link(from, to, verb)` | — (default `False`) | a flat key/value store, no edges |
| `supersede(old, new)` | `store.delete(old)` then `store.put(new)` | flat replacement; nothing keeps the retired generation |
| `release(key, reason)` | `store.delete(key)` | the reason is dropped; no marker |
| `purge(key)` | `store.delete(key)` | |
| `settle()` | — (default `None`) | no maintenance concept |
| `recall(q, k, window)` | `store.search(("kd",), query=q, limit=k, filter={"created_at": {"$gte": after, "$lt": before}} or None)` | `text` = stored `text` verbatim, `score` = the `SearchItem.score` (cosine similarity), `created_at` from the value, `key` = the store key |
| `suspects()` | — (default `None`) | no disagreement queue |
| `lineage(key)` | — (default `None`) | no history |
| `standing_tokens()` | 0 | nothing is injected before a question |

Keys: the script key is the store key, one namespace `("kd",)`. The adapter
keeps its own set of live keys and raises `KeyError` on a delete of an unknown
key — `InMemoryStore.delete` is a silent no-op, and a script error must reach
the runner, not vanish.

## Capabilities declared

| capability | value | why |
|---|---|---|
| `link` | false | no edges in a `BaseStore` |
| `history` | false | supersede is delete + put; `lineage` returns `None` |
| `trace` | false | release / purge are plain deletes; no marker survives |
| `suspects` | false | no queue, no NLI, no model |
| `temporal` | **true** | `created_at` is stored on the value and filtered **natively at search time** (see below) |
| `verdict` | false | `put` is silent; no near-duplicate refusal |
| `write_check` | false | no write-time canon check |

### Temporal filter — verified, applied before the top-k cut

Checked in the installed `langgraph` 1.2.11
(`langgraph/store/memory/__init__.py`): `_batch_search` calls
`_filter_items(op)`, which walks every item in the namespace and applies the
filter (`_compare_values` → `_apply_operator`, which supports `$eq $ne $gt
$gte $lt $lte` with float comparison), and only the survivors are
cosine-scored, sorted, and then sliced `[offset : offset + limit]`. So the
window is a true pre-filter and the adapter does **not** over-fetch. The
half-open `[after, before)` window maps to `{"$gte": after, "$lt": before}`
exactly. Confirmed on the transcript: 25 windowed recalls, 0 hits outside the
window; one of them returned 9 hits because only nine notes fell inside its
window.

## Deviations and shims (read these first)

- **No LLM.** LangMem's extraction / consolidation managers and the
  agent-driven tool loop are not run. This arm measures LangMem's storage and
  semantic-search substrate, not what an LLM-driven LangMem agent would
  remember. That is the only honest way to put it on a judge-free benchmark,
  and it should be read as such.
- **Value shape.** LangMem's manage tool stores `{"content": <str>}` under a
  uuid4 key; the adapter stores `{"text": title + "\n" + body, "kind",
  "created_at", "key"}` under the script key and indexes `text`. The field
  name is only the index target — the ranking is identical to indexing
  `content` — but the extra `created_at` field is what makes the temporal
  filter possible; a stock LangMem agent stores no capture time on the value
  (the store keeps `created_at` on the `Item`, but the filter reads
  `item.value`, so it could not window on it).
- **Every hit is the whole note.** The store returns the full value; there is
  no snippeting, so `text` is title + body verbatim (that is what the
  attention columns bill: 2 439 tok/query at k=10).
- **Delete on an unknown key raises** (adapter-side check), where the store
  itself would silently no-op.
- **`import` = `write`.** Both modes are one `put`; the store has no
  assistant-style write path.
- **Embedding batch.** `InMemoryStore.put` embeds each item in its own call
  (one text per `embed` invocation); queries are embedded one per search. The
  adapter does not batch around the store — that is the store's cost model.
- Nothing else: no re-ranking, no thresholds, no decline rule, no over-fetch.

## Graded result — 100 world (`knowledgedrift-100-seed1.json`, digest `280739585b56cf43…`)

Transcript `out/langmem-100.json`, receipt `out/langmem-100-graded.json`.
Replay wall-clock **7 s** (8.6 s including interpreter + model load), Apple
silicon CPU.

```
== 100 tested facts, 125 notes ==
  arm       posed attempted passed  success   of att. composite     S  mult   score  standing    tok/q
  langmem     474       418    335      71%       80%     0.516  0.10   1.0      54         0     2439
  langmem  success  71% = 335 passed of 474 posed (attempted 418/474,  80% of those passed)   composite 0.516   S 0.10 ×1.0   score 54   standing 0 tok   2439 tok/query
    retrieval       300   90%  r@1 0.77  r@5 0.94  lexical_r@5 1.00  paraphrase_r@5 0.99  oblique_r@5 0.82  stale_above 0.37  hedge 0.00  noise 0.90
    abstention       22    0%  fp 1.00  answered 1.00  declined 0.00  separation 0.89
    currency         15  100%  head_r@1 0.93  head_r@5 1.00  pollution 0.00
    contradiction     -  n/a — no suspect nomination
    drift             -  n/a — no suspect nomination
    deletion         16  100%  released_gone 1.00  purged_gone 1.00  resurrection_warned 0.00  purged_rewrite_warned 0.00
    rationale        40   22%  direct_r@5 0.23  assisted_r@5 0.23  structure_only 0.00
    temporal         25  100%  in_window_r@5 1.00  leak 0.00
```

Reading it: lexical retrieval is perfect because the text is stored verbatim
and the query is its own words; `stale_above 0.37` is the price of a flat
store — a polluted subject's stale sibling outranks the current note in 37% of
those questions because nothing marks it stale. Abstention `fp 1.00`: the store
never declines, every control question gets ten hits (`separation 0.89` says a
threshold *could* be fitted on the scores; none is shipped). Currency is 100%
because supersede physically deletes the retired generation; lineage is N/A
for the same reason. Deletion is 100% gone with no trace and no warning — the
victim is simply absent. Rationale 22% is pure vector luck: there are no edges,
so `structure_only 0.00`. Temporal is 100% with zero leak — the native
pre-filter.

Per-op means (ms) from the transcript: inscribe 24.8 (one embedder call per
put), supersede 31.7, recall 6.4, link/settle/release/purge/lineage/suspects
≈ 0.

## Wall-clock for the larger worlds

**500 world — measured** (`knowledgedrift-500-seed1.json`, 3 593 ops):
replay **55 s** (56 s wall), 2 348 replies. Per-op means grew with the
store: inscribe 33.7 ms, supersede 45.5 ms, **recall 11.8 ms** (6.4 at 100 —
`_filter_items` walks every item in Python and the cosine is over all of
them, so recall is linear in store size). Graded for the record
(`out/langmem-500.json`, `out/langmem-500-graded.json`):

```
== 500 tested facts, 630 notes ==
  arm       posed attempted passed  success   of att. composite     S  mult   score  standing    tok/q
  langmem    2346      2058   1477      63%       72%     0.469  0.11   1.1      51         0     2327
  langmem  success  63% = 1477 passed of 2346 posed (attempted 2058/2346,  72% of those passed)   composite 0.469   S 0.11 ×1.1   score 51   standing 0 tok   2327 tok/query
    retrieval      1500   80%  r@1 0.64  r@5 0.84  lexical_r@5 1.00  paraphrase_r@5 0.98  oblique_r@5 0.53  stale_above 0.38  hedge 0.00  noise 0.91
    abstention      110    0%  fp 1.00  answered 1.00  declined 0.00  separation 0.79
    currency         75   93%  head_r@1 0.77  head_r@5 0.93  pollution 0.00
    contradiction     -  n/a — no suspect nomination
    drift             -  n/a — no suspect nomination
    deletion         83  100%  released_gone 1.00  purged_gone 1.00  resurrection_warned 0.00  purged_rewrite_warned 0.00
    rationale       165    3%  direct_r@5 0.03  assisted_r@5 0.03  structure_only 0.00
    temporal        125   99%  in_window_r@5 0.99  leak 0.00
```

**1500 world — measured** (`results/v1/langmem-0.0.30/1500-seed1.json`: 57% success, score 52; the estimate below, written before the run, held). Estimate: (10 816 ops: 2 528 inscribes, 150 supersedes,
6 220 recalls, ~1 900 live items): recall ≈ 6 ms + ~0.011 ms/item × 1 900 ≈
25 ms; 2 528 × 34 ms + 150 × 46 ms + 6 220 × 25 ms ≈ 250 s, so **about 4–5
minutes** of replay. Nothing needs a GPU or a service; the store lives in
process memory (1 900 × 384 floats is negligible).
