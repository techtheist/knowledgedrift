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
    --script ../worlds/v2/knowledgedrift-100-seed1-v2.json --out out/langmem-100.json

cd ..   # repo root
cargo run --release -- --grade adapters/out/langmem-100.json \
    --script worlds/v2/knowledgedrift-100-seed1-v2.json \
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

## Results — 500 and 1500 tested facts, seed 1

Receipts under `results/v2/langmem-0.0.30/` (`500-seed1.json`,
`1500-seed1.json`, the `.log` beside each). One Apple-silicon laptop, CPU
embedder, the three external chains replaying side by side so wall-clock is
contended.

```
  arm       posed attempted passed  success   of att.  families  signal  tokens   score   billed
  langmem    3042      2754   1600      53%       58%       359       8       9     376     2360
    retrieval      2063   64%  r@1 0.51  r@5 0.67  lexical_r@5 1.00  paraphrase_r@5 0.99  oblique_r@5 0.51  crossed_r@5 0.17  path_r@5 0.67  path_cover 0.34  stale_above 0.27  hedge 0.00  noise 0.92
    abstention      238    0%  fp 1.00  phantom_fp 1.00  natural_fp 1.00  answered 1.00  declined 0.00  separation 0.60
    currency         75   91%  head_r@1 0.77  head_r@5 0.91  pollution 0.00
    contradiction     -  n/a — no suspect nomination
    drift             -  n/a — no suspect nomination
    deletion         83  100%  released_gone 1.00  purged_gone 1.00  resurrection_warned 0.00  purged_rewrite_warned 0.00
    rationale       170    4%  direct_r@5 0.04  assisted_r@5 0.04  structure_only 0.00
    temporal        125  100%  in_window_r@5 1.00  leak 0.00
```

At 1500: **48% success, score 359** (families 340, signal 7, tokens 11,
2,201 billed tokens a query); retrieval 57% (oblique 0.34, crossed 0.09,
path 0.56 / 0.19), currency 80%, rationale 4%, temporal 99%.

Reading it: LangMem is the in-process `rag` arm to the digit (376 and 359
against 376 and 359) — a flat store given the same embedder is one system,
and the three calls its tools make add nothing a vector top-k does not.
Lexical retrieval is perfect because the text is stored verbatim and the
query is its own words; the crossed question, which shares no content word
with its note, is found 17% of the time. `stale_above 0.27` is the price of
a flat store — a polluted subject's stale sibling outranks the current note
in 27% of those questions because nothing marks it stale. Abstention
`phantom_fp` and `natural_fp` 1.00: the store never declines, every control
question gets ten hits (`separation 0.60` says a threshold would barely
help; none is shipped). Currency 91% because supersede physically deletes
the retired generation; lineage is N/A for the same reason. Deletion is
100% gone with no trace and no warning — the victim is simply absent.
Rationale 4% is pure vector luck: there are no edges, so `structure_only
0.00`. Temporal is 100% with zero leak — the native pre-filter. The path
read (0.67 / 0.34) embeds the path string and finds the component's notes,
never the file.

Per-op means (ms) from the 500 transcript: inscribe 50 (one embedder call
per put), supersede 58, recall 22, the path read 13,
link/settle/release/purge/lineage/suspects ≈ 0. Recall is linear in store
size — `_filter_items` walks every item in Python and the cosine is over
all of them — so the 1500 world replays in about five minutes of op time.
Nothing needs a GPU or a service; the store lives in process memory
(1,900 × 384 floats is negligible).
