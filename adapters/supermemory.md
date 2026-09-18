# Supermemory — KnowledgeDrift adapter

`supermemory_adapter.py` scores **Supermemory local** — the self-hosted
`supermemory-server` binary, "the same memory engine behind the hosted
platform" — as a **raw, LLM-free memory store**: notes written straight
into the memory graph through `POST /v4/memories` (the documented direct
route: "memories are embedded and immediately searchable"), re-decisions
through the *versioned* `PATCH /v4/memories`, soft forgetting with a
reason, a hard delete, and `POST /v4/search` at the server's own defaults
with a native numeric filter on a stored capture time. The local embedder
is switched to the benchmark's reference model (`bge-small-en-v1.5`, 384
dims). **What Supermemory sells — LLM memory extraction from documents and
conversations, LLM-resolved `updates` / `extends` / `derives` relations,
"dreaming", automatic forgetting, user profiles — is out of scope for this
offline, judge-free benchmark and is not measured here.** The row reads
"Supermemory's memory graph as a store you write facts into", nothing
more. The versioned update is the part of the graph that survives without
the model, and it is the part this row is about.

## Setup

```sh
# the server: one release binary, sha256-verified by Supermemory's own
# installer, kept out of $HOME (adapters/vendor/ is gitignored)
curl -fsSL https://supermemory.ai/install -o /tmp/supermemory-install.sh   # read it first
SUPERMEMORY_INSTALL_DIR=$PWD/adapters/vendor/supermemory \
SUPERMEMORY_BIN_DIR=$PWD/adapters/vendor/supermemory/wrapper \
SUPERMEMORY_NO_START=1 SUPERMEMORY_NO_PROMPT=1 \
    bash /tmp/supermemory-install.sh 0.0.8

# replay + grade — the adapter is stdlib-only, so there is no venv
python3 adapters/run.py --adapter supermemory \
    --script worlds/v2/knowledgedrift-100-seed1-v2.json --out adapters/out/supermemory-100.json
cargo run --release -- \
    --grade adapters/out/supermemory-100.json \
    --script worlds/v2/knowledgedrift-100-seed1-v2.json \
    --json adapters/out/supermemory-100-graded.json
```

The first replay is the one step that touches the network: the server
downloads the embedder weights (33 MB) from Hugging Face into
`adapters/out/supermemory-models/`, which every later `reset()` links back
into the fresh data directory. The adapter starts and stops the server
itself; nothing has to be running.

Adapter kwargs (`--kw k=v`): `store=<dir>` (default
`out/supermemory-store`, wiped on every `reset()`), `port=6795`,
`threshold=<0..1>` and `search_mode=memories|hybrid|documents` (both unset
by default: the field is not sent and the server's default applies),
`binary=<path>`.

## Version pins (what the numbers below were measured on)

| component | version |
|---|---|
| supermemory-server | **0.0.8** ("supermemory lite", release tag `server-v0.0.8`, `darwin-arm64`, sha256 `12b7817a105ed0a9e70f96c461fb6f8dded5d70eaeb6034e774778c257bed78a`); a Bun 1.3.4 single-file executable |
| its stores | the embedded graph engine ("encrypted local storage") and a `rivet` workflow engine the server starts as a child process — both inside the data directory |
| embedder weights | `Xenova/bge-small-en-v1.5`, file `onnx/model_quantized.onnx` (34,014,426 bytes — the **int8** export), 384 dims, run by the server's own local provider ("native backend", one worker) |
| Python | 3.14.6, standard library only (`http.client`, `subprocess`) |
| API docs read | `github.com/supermemoryai/supermemory` @ `bf2db3d` (2026-09-18), and the server's own `GET /v4/openapi` |

The repository is MIT and holds the SDKs, the docs, the MCP server and the
web app; the server itself ships as a release binary, so the mechanism
notes below are what the API shows, not what a source says. The binary
announces a licence cap of 10,000 documents; the 1500 world writes 2,669.

## Environment handed to the server

```sh
HOME=<store>/home  PATH=/usr/bin:/bin:/usr/sbin:/sbin      # nothing else inherited
PORT=6795  SUPERMEMORY_DATA_DIR=<store>/data
SUPERMEMORY_EMBEDDING_PROVIDER=local
SUPERMEMORY_EMBEDDING_MODEL=Xenova/bge-small-en-v1.5
SUPERMEMORY_EMBEDDING_DIMENSIONS=384
SUPERMEMORY_DISABLE_TELEMETRY=1  SUPERMEMORY_NO_UPDATE_CHECK=1
SUPERMEMORY_NO_OPEN=1  SUPERMEMORY_NO_STARTUP_ANIMATION=1
OPENAI_API_KEY=sk-knowledgedrift-no-llm
OPENAI_BASE_URL=http://127.0.0.1:<tripwire port>/v1  OPENAI_MODEL=never-called
```

## Operation → Supermemory API

| op | Supermemory call | notes |
|---|---|---|
| `reset()` | stop the server's process group, `rm -rf` the store, start the binary on a fresh data directory, poll until it stops answering `503 Service is starting` | a boot that never answers in 60 s is stopped and redone on a fresh directory (up to three; see deviation 9); asserts the container lists no memory afterwards |
| `inscribe(record, mode)` | `POST /v4/memories {containerTag: "knowledgedrift", memories: [{content: f"{title}\n{body}", metadata: {key, kind, created_unix}, temporalContext: {documentDate: ISO}}]}` | one memory per call, so one source document per note (what `purge` deletes); `mode` ignored — import and write take the same path, there is no write-time check; a still-live duplicate key raises |
| `link(from, to, verb)` | nothing | `link: false` |
| `supersede(old, new)` | `PATCH /v4/memories {id: id_of(old), containerTag, newContent, metadata: metadata(new)}` | a **new memory id** comes back with `version + 1`, `parentMemoryId = id_of(old)` and the chain's `rootMemoryId`; the old version stays in the store with `isLatest=false` and leaves search. The adapter checks `parentMemoryId` and keeps the old id → key entry, so a retired version that was ever delivered would be named |
| `release(key, reason)` | `DELETE /v4/memories {id, containerTag, reason}` | the soft forget: `isForgotten=true`, the reason stored as `forgetReason` |
| `purge(key)` | `DELETE /v3/documents/{source document id}` | the hard delete: the memory row goes with its document (verified: it leaves `/v4/memories/list`, where a forgotten memory's chain mates stay) |
| `settle()` | nothing | returns the receipt note (server version, search knobs, tripwire hits, re-sent reads) |
| `recall(query, k, window)` | `POST /v4/search {q, containerTag, limit: k, filters: {AND: [{filterType: "numeric", key: "created_unix", value, numericOperator: ">="}, {… "<"}]}}` | `searchMode` and `threshold` not sent: the server's defaults (`memories`, **0.6**) apply; hit `text` = the `memory` field verbatim, `score` = `similarity`, `created_at` = the stored `created_unix`, `key` from the id map, cross-checked against the hit's own `metadata.key` |
| `recall_path(path, k)` | the default: the path string handed to `recall` | `filepath` exists in the API but belongs to *documents* (one path per ingested file, the SMFS channel) — a directly written memory cannot carry one, and a note here names up to three files |
| `suspects()` | — | `None` |
| `lineage(key)` | `POST /v4/search {q: the head's own text, threshold: 0, filters: {AND: [{key: "key", value: key}]}, include: {relatedMemories: true}}` → `context.parents[]` with `relation: "updates"` | the ancestry is the server's: every retired generation comes back with its own stored metadata, and the keys are read off that, not off the adapter's map |
| `standing_tokens()` | — | 0 |

## Capabilities declared

| capability | value | why |
|---|---|---|
| `link` | false | the graph's edges (`updates`, `extends`, `derives`) are minted by the LLM pipeline; no endpoint stores an edge a caller states, and `include.relatedMemories` has nothing to deliver for a `because` |
| `history` | **true** | `PATCH` versions instead of overwriting, and a reader can walk it: the head's hit carries every ancestor under `context.parents` (three generations verified, all 25 / 75 lineage probes pass), and `/v4/memories/list` shows the same chain as `history[]` |
| `trace` | false | a forgotten memory is kept (`isForgotten`, `forgetReason`), but nothing on 0.0.8 delivers it: `include.forgottenMemories: true` — documented as the way to "recover them" — returned nothing for a forgotten memory's own text at `threshold: 0`, and `/v4/memories/list` leaves it out. A marker nobody can find is not a trace |
| `suspects` | false | contradiction handling is the LLM's job at ingestion; there is no queue |
| `temporal` | **true** | the window is a native numeric metadata filter inside `/v4/search` (half-open `>=` / `<`, like the grader's), no over-fetch-and-filter in the adapter |
| `verdict` | false | a search whose candidates all sit under the threshold comes back empty — that is "nothing delivered", which the abstention family already credits; there is no decline signal *beside* hits |
| `write_check` | false | a verbatim duplicate is stored silently (201, a second memory), and rewriting a forgotten memory's exact text raises nothing |

## Deviations and shims — read these before the numbers

1. **No LLM, verified by a tripwire.** The server refuses to boot without
   a provider key ("No model provider API key configured"), and it is a
   separate process, so a Python socket guard cannot cover it. It gets a
   dummy OpenAI key whose base URL is an HTTP listener inside the adapter
   process that records every request and answers 503; any hit raises
   `LlmAttempted` on the next call and again at `close()`. The guard is
   live, not decorative: a probe `POST /v3/documents` (the ingestion
   pipeline) put four `POST /v1/chat/completions` on it within eight
   seconds — three summary prompts and one "memory extraction agent".
   The direct-memory route used here never touched it: **0 hits on every
   receipt** (the transcript's `settle_note`). Telemetry and the update
   check are switched off by the two environment variables above.
2. **Direct memories, not documents.** `POST /v3/documents` is the front
   door of the product and the LLM's; `POST /v4/memories` is the
   documented way to store "facts where you already know the exact memory
   content". Everything below is that route. Its memories have no chunks,
   so `searchMode: "hybrid"` — the docs' recommendation — has nothing
   extra to search: on the 100 world it is identical to the default to the
   digit (429), and the default is what runs.
3. **The threshold is the server's, and it is the story.** `/v4/search`
   drops every candidate under `threshold`, default **0.6** on 0.0.8 (the
   server's OpenAPI; the docs page says 0.5). It is kept, the way Mem0's
   0.1 was kept — except that this one bites: at 500 it empties 97 of
   2,754 recalls (3.5%) and shortens another 472 (17%); at 1500, 163 of
   8,150 (2.0%) and 575 (7%). An empty reply passes a phantom and fails
   everything else. The ablation with the floor off (`--kw threshold=0`)
   is in the results below; it scores *lower*, so the default is also the
   kinder setting.
4. **Capture time lives in metadata.** A memory's `createdAt` is the
   server's wall clock and cannot be set. The script's capture time goes
   into `temporalContext.documentDate` (the memory's own date slot, ISO
   8601) and into a numeric `metadata.created_unix`, which is what the
   window filter reads and what a hit reports. After `supersede` the new
   version carries the new record's capture time.
5. **The embedder is the int8 export.** The server's local provider loads
   `onnx/model_quantized.onnx`; the in-process arms embed with the fp32
   ONNX weights. Same model, same dimensions, quantization noise — the
   footnote Mem0's notes carry for fastembed-python, here without an
   alternative: the provider does not expose the file choice.
6. **`rerank` is off, and inert here anyway**: on 0.0.8 local it returns
   the same similarities with or without it, and calls nothing.
   `rewriteQuery` and `aggregate` are LLM features and stay off.
7. **Whole notes are what Supermemory shows.** A hit's `memory` is the
   stored text verbatim (`title\nbody`), so the attention columns bill the
   whole note.
8. **One keep-alive connection, reads retried.** Opening a connection per
   request had the server reset one about 4,000 requests into the 1500
   world (nothing in its log, no crash). The adapter now holds one
   connection; a dropped *read* is re-sent on a fresh one and counted in
   the receipt note, a dropped *write* fails the run — a blind retry could
   store a note twice.
9. **The server is a process group, and a boot can stall.** It starts a
   `rivet` workflow engine as a child (fixed loopback ports 30385–30387);
   the adapter launches it in its own session, stops the whole group and
   waits until nothing names the store, with its own `HOME` so
   `~/.supermemory` is never read or written. Twice, a boot started
   within a second of a finished replay hung at "encrypted local storage"
   with nothing logged and the port never opened. `reset()` therefore
   gives a boot 60 s, keeps a stalled boot's log under `out/`, and boots
   again on a fresh directory — the store is empty either way, and the
   count is in the receipt note (0 on every receipt here).
10. **The search reads dates out of the question.** With no LLM in the
    loop (the tripwire stayed at 0), `/v4/search` parses temporal
    expressions in `q` and filters on the memory's date: *"which replay
    buffer serves yesterday's answers confidently"* returns nothing even
    at `threshold: 0`, where *"… serves stale answers …"* returns the
    note; "last week", "today" and "in 2019" do the same. In these worlds
    "serves yesterday's answers" is a Caution's predicate, not a time
    scope: 8 of 2,691 recalls at 500 and 20 of 8,086 at 1500 carry the
    word — all oblique or crossed retrieval questions — and every one
    came back empty. It is the system's behaviour and is measured as it
    is; the windowed temporal family, which states its window as a
    filter, is unaffected (100% / 99%).
11. `release` drops nothing the system keeps: the reason is stored. It is
    `trace: false` only because nothing reads it back (see the table).

## Results — 500 and 1500 tested facts, seed 1

Receipts under `results/v2/supermemory-0.0.8/` (`500-seed1.json`,
`1500-seed1.json`, the `.log` beside each; `*-threshold0.*` for the
ablation), produced by `results/v2/run-adapters-d.sh`. One Apple-silicon
laptop, CPU, 2026-09-18, the chain running alone. The 500 row was
replayed three times and the 1500 row twice over the adapter's transport
and lifecycle fixes, identical to the digit each time; every `settle_note` reads
"LLM tripwire hits 0, reads re-sent 0, stalled boots redone 0", and the
grader raised no capability flag.

```
  arm       posed attempted passed  success   of att.  families  signal  tokens   score   billed
  supermemory 3042      2779   1590      52%       57%       363       9      12     385     2157
    retrieval      2063   62%  r@1 0.51  r@5 0.66  lexical_r@5 0.99  paraphrase_r@5 0.98  oblique_r@5 0.53  crossed_r@5 0.12  path_r@5 0.43  path_cover 0.18  stale_above 0.29  hedge 0.00  noise 0.87
    abstention      238    2%  fp 0.98  phantom_fp 0.96  natural_fp 1.00  answered 0.98  declined 0.00  separation 0.62
    currency        100   96%  head_r@1 0.76  head_r@5 0.95  pollution 0.00  lineage 1.00
    contradiction     -  n/a — no suspect nomination
    drift             -  n/a — no suspect nomination
    deletion         83  100%  released_gone 1.00  purged_gone 1.00  resurrection_warned 0.00  purged_rewrite_warned 0.00
    rationale       170    4%  direct_r@5 0.04  assisted_r@5 0.04  structure_only 0.00
    temporal        125  100%  in_window_r@5 1.00  leak 0.00
```

At 1500: **48% success, score 364** (families 345, signal 8, tokens 12,
2,195 billed tokens a query); retrieval 56% (oblique 0.35, crossed 0.08,
path 0.53 / 0.14, `stale_above` 0.32), abstention 1% (`phantom_fp` 0.98),
currency 85% (`head_r@5` 0.80, `lineage` 1.00 on all 75 walks), rationale
4%, temporal 99%.

Per-op means from the 500 transcript: inscribe 53 ms, supersede 72 ms,
recall 17 ms, the path read 14 ms, lineage 64 ms, release 5 ms, purge
10 ms; link/settle/suspects are no-ops. At 1500 recall is 22 ms; the
replay takes 104 s at 500 and 334 s at 1500.

**The ablation — the similarity floor off (`--kw threshold=0`):**

| | score @500 | retrieval | abstention | path_r@5 / cover | billed tok/query | score @1500 | retrieval | billed |
|---|---|---|---|---|---|---|---|---|
| **server default (0.6) — the row** | **385** = 363 + 9 + 12 | 62% | 2% | 0.43 / 0.18 | 2,157 | **364** = 345 + 8 + 12 | 56% | 2,195 |
| `threshold=0` | 378 = 363 + 8 + 7 | 64% | 0% | 0.68 / 0.34 | 2,452 | 361 = 345 + 7 + 9 | 57% | 2,321 |

With the floor off the row is `rag` within two points (376 / 359): the
family points do not move at all (363 and 345 either way — the two
retrieval points the floor costs are the two abstention points it earns),
and the whole difference is the bill. Currency, deletion, rationale and
temporal are identical to the digit in both settings. `searchMode:
"hybrid"` was checked on the 100 world only, where it is identical to the
default (429): a direct memory has no chunks for it to add.

Reading it, next to the in-process arms on the same script (`rag` 376 at
53%, `grep` 335 at 43%, the reference system 816 at 73%):

- **Retrieval 62%** — lexical and paraphrase 0.99 / 0.98, oblique 0.53,
  crossed 0.12: a cosine top-k over the reference embedder, two points
  under `rag` because the floor drops a gold now and then. `stale_above
  0.29`: on polluted subjects the untouched stale sibling outranks the
  gold 29% of the time — nothing but an LLM would have told the graph the
  sibling was an `updates`.
- **Abstention 2%** — the first external store that is not at zero, and
  not by much: the 0.6 floor returns nothing for 4% of phantoms and for
  no natural null (`natural_fp` 1.00). `separation 0.62` says why a floor
  cannot do more: answerable and control top scores overlap.
- **Currency 96%** — the versioned `PATCH` retires the old generation from
  search (`pollution 0.00`) and keeps it reachable: all 25 lineage walks
  read every retired generation off the head's `updates` parents. The
  head itself misses on the oblique question (`head_r@5` 0.95).
- **Deletion 100% on absence**, 0 on warnings — a forgotten memory is out
  of search and a purged one is out of the store, and a write-back is
  just another `POST`; the stored `forgetReason` is read by nothing.
- **Rationale 4%** — no caller-written edges, so only rationale notes
  that are lexically close to the question surface (`structure_only 0.00`).
- **Temporal 100%** — the native numeric filter delivers the gold inside
  the window with zero leak (99% at 1500).
- **Signal 9, tokens 12** — whole notes, but fewer of them: the floor
  trims a fifth of the replies, so the bill is 2,157 tokens a query
  against LangMem's 2,360 and Mem0's 2,567 (cognee's chunks bill 2,053).

## Files

- `supermemory_adapter.py` — the adapter (standard library only)
- `vendor/supermemory/` — the server binary and the installer's manifest (gitignored)
- `out/supermemory-*.json` — transcripts and graded files (scratch, gitignored; the graded receipts that count live in `results/v2/supermemory-0.0.8/`)
- `out/supermemory-store*/` — the server's data directory, its `HOME` and `server.log`; `out/supermemory-models/` — the embedder weights, kept across resets (all scratch, gitignored)
