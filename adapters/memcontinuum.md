# MemContinuum — adapter notes

[MemContinuum](https://github.com/krakozavr/MemContinuum) is an
"institutional memory engine" for long-running Claude Code projects: one
markdown **topic** file per subject in an append-only git store, a list of
**links** (rulings) under it — each with a status (`active | provisional |
superseded | historical | declined`) and a per-field **authority**
(`owner-verbatim | owner-ratified | agent-inference | reviewer-finding |
code-derived`) — indexed by `memidx.py` into SQLite FTS5 plus bge-small
vectors, and read by hooks that inject `for-path` chains before an edit.
Nothing in it calls a language model; records are meant to be authored
deliberately and promoted by the owner.

The adapter (`memcontinuum_adapter.py`) drives the engine in-process:
`import memidx`, one topic file per harness note, `cmd_reindex` before a
read whenever a write landed, `cmd_search --json` with the system's own
defaults for every recall. It is the system as a MemContinuum user would
run it from the CLI, minus the git hooks (the store is never committed).

## Setup

```sh
python3 -m venv adapters/.venv-memcontinuum
adapters/.venv-memcontinuum/bin/pip install -r adapters/vendor/memcontinuum/requirements.txt
git clone https://github.com/krakozavr/MemContinuum adapters/vendor/memcontinuum   # gitignored
# prefetch the embedder once (fastembed's registry; the engine loads it by name)
adapters/.venv-memcontinuum/bin/python3 -c "from fastembed import TextEmbedding; TextEmbedding('BAAI/bge-small-en-v1.5')"

adapters/.venv-memcontinuum/bin/python3 adapters/run.py --adapter memcontinuum \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json --out adapters/out/memcontinuum-500-seed1.json
cargo run --release -- --grade adapters/out/memcontinuum-500-seed1.json \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json --json adapters/out/memcontinuum-500-seed1-graded.json
```

`MEMCONTINUUM_SRC` points the adapter at another checkout;
`MEMCONTINUUM_HOME` is set to `adapters/out/memcontinuum-home` so the
machine's own `~/.memcontinuum` is never touched, and the index is always
passed explicitly (`adapters/out/memcontinuum-index.sqlite`).

## Version pins

| what | version |
|---|---|
| MemContinuum | `0.2.0rc5`, commit `213678b` (2026-09) |
| Python | 3.14.6 (the engine requires ≥ 3.12) |
| fastembed | 0.8.0 — `BAAI/bge-small-en-v1.5`, revision `5239827…`, fastembed's registry export (the same package LangMem's adapter uses) |
| PyYAML | 6.0.3; tree-sitter 0.26.0 and the six grammars `requirements.txt` pins (imported, unused: no code root is indexed) |
| sqlite3 | the Python build's, FTS5 on |

## Operation → API mapping

| op | what it becomes | dropped |
|---|---|---|
| `inscribe(record, mode)` | a topic file `topics/<kind>/<key>.md`: `id` = key, `title`, `area` = kind, `code_refs`, `tags: [kind]`, one link `L1` (`status: active`, `kind: adopted`, `ruling.text` = body, `ruling.authority: agent-inference`, `date` = capture day); the body renders the chain newest-first | `open` (Problems are rulings like any other); `mode` (no write-time checks exist) |
| `link(from, to, verb)` | a typed edge on the link whose sentence it is, in the one reading the schema's seven `rel` values allow: `X because Y` → on Y `led_to X`; `R answers P` → on P `led_to R`; `X builds-on Y` → on Y `led_to X`; `X about Y` → on X `applies_to Y` (the revised adapter; its first version mapped `link` to nothing) | the verb's own name |
| `supersede(old, new)` | the old link's lifecycle move `status: superseded` + `superseded_by`, a new link `L<n+1>` (`kind: reversed`, `reverses`, `reason_for_change: changed-mind`, the new body as its ruling), the topic's `title` and `current` updated (title changes are free per SCHEMA §7) | — |
| `release(key, reason)` | the lifecycle move `status: historical` on the active link ("no longer applicable, nothing replaced it"); the reason appended to the topic's free body text | the trace: default search hides non-active rulings |
| `purge(key)` | the topic file deleted; the next reindex drops its rows | — |
| `endorse(key, "user")` | SCHEMA §5 promotion: a new active link with the same text at `authority: owner-ratified`, `promoted_by` on the promoted link | — |
| `endorse(key, retrieval / assistant / supervisor)` | nothing — no use counter, no agent-side confirmation act (only the owner promotes), no pin | the endorsement |
| `settle()` | `cmd_reindex` if a write landed since the last read | — |
| `recall(query, k, window)` | `cmd_search --json --mode hybrid --limit k` with the defaults (active rulings only, inbox excluded): FTS5 BM25 and cosine over the fresh vectors, RRF-fused (k=60), one hit per topic family; each hit carries as `neighbors` the current keys of the topics its typed edges touch, read off the index's `edges` table in both directions | the window (search filters by status/type/area/topic/authority, never by date) |
| `recall_path(path, k)` | `cmd_for_path --json --with-chain-text`: every topic whose `code_refs` cover the path (exact, directory prefix or glob — the pre-edit hook's own match), each delivered as its chain text (the exact lines the hook injects), in index order, unranked; the first `k`; edge neighbours as above | the rest of a match list longer than `k` |
| `suspects()` | `None` | — |
| `lineage(key)` | the topic's links newest-first, mapped back to the keys they were written under — what `memidx chain` shows | — |
| `standing_tokens()` | 0 — the hooks inject `for-path` chains per edited file, not a standing memory file | — |

A hit's `text` is `title` + newline + `snippet` (the first 200 characters of
the ruling, exactly the two fields `search --json` shows); `score` is the
RRF sum; `key` is resolved through the topic and `matched_link_id`; a
topic-row hit answers to its newest active link's key.

## Capabilities declared

| capability | value | why |
|---|---|---|
| `link` | **true** | typed edges are stored on links and delivered as a hit's neighbours |
| `history` | **true** | the chain is the unit; `memidx chain` walks it |
| `trace` | false | a `historical` ruling stays in the file but default search does not deliver it |
| `suspects` | false | no disagreement detection |
| `temporal` | false | no date filter in search |
| `verdict` | false | search never declines |
| `write_check` | false | `memlint` validates shape, not content |
| `endorse_user` | **true** | promotion to `owner-ratified` is the system's own gesture for "the owner confirmed this" |
| `endorse_retrieval` / `endorse_assistant` / `endorse_supervisor` | false | no use counter; only the owner promotes; no pin |

## Deviations and shims

- **One model per process.** `memidx.load_embedding_model` instantiates a
  fresh `TextEmbedding` on every call by design (test isolation); the
  adapter patches it to return one cached instance. Vectors are identical;
  only wall-clock changes.
- **The store is never committed**, so the pre-commit append-only linter
  and the post-commit reindex hook never run; the adapter reindexes itself.
  Every file it writes is shaped to pass `memlint` (status/kind/authority
  from the five/five/five vocabularies, `current` = newest active link,
  `superseded_by` on every superseded link).
- **Release has no schema slot for a reason.** The lifecycle move to
  `historical` is the schema's own; the reason lands in the free body text.
- **Every write is `agent-inference`** — the harness is an agent writing
  notes — which under the schema's citation rule makes every ruling
  CONTEXT, never CONSTRAINT, until the owner promotes it. The authority
  family's `user` rung is exactly that promotion.
- **Edges are read both ways.** `chain --json` and `for-path --json` show
  a link's *outgoing* edges; a rationale question lands on the decision
  and needs the reason, which is the edge's source. The adapter reads the
  index's own `edges` table by `to_ref` as well — one query on the same
  table the CLI walks, not a search feature the system has.
- **`for-path` delivers everything bound, unranked.** The protocol asks
  for `k`; the adapter returns the first `k` matches in index order. On
  the 1500 world some files bind more topics than `k`.
- **Timing columns are contended**: every replay ran beside the in-process
  ladder on one laptop.

## Numbers

`results/v2/memcontinuum-0.2.0rc5/README.md`: **392 at 500 and 380 at
1500**, rationale 100% (`structure_only` 0.95), `path_r@5` 1.00 at
`path_cover` 0.95 / 0.87, 852 billed tokens a query. The adapter as first
written mapped `link` to nothing and answered every read with `search`;
that version scored 295 and 287 with rationale 5%, and was revised after
the author's review: the agent's channel is the hook-injected `for-path`
chain, not `search`. This adapter measures that channel where the protocol
has a place for it (the rationale walk, the path read) and leaves `search`
as the answer to a question, which is the only thing a question can ask.
What the revision did not move: `stale_above` 0.62 (its FTS5 channel
prefers the sibling's shorter title), abstention 0%, no suspect queue, no
clock, oblique 0.31, crossed 0.05.
