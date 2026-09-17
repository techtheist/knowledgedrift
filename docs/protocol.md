# The protocol

Twelve operations (`src/protocol.rs`; mirrored for Python in
`adapters/adapter.py`). An adapter implements them against its system's
native API; the harness never reaches around them.

| op | meaning | a flat store does |
|---|---|---|
| `inscribe(record, mode)` | write a note; `import` is a bulk load, `write` is an assistant-style note with the system's write-time checks | insert |
| `link(from, to, verb)` | store a sentence-shaped link | nothing (`link: false`) |
| `supersede(old, new)` | `new` re-decides `old`: from now on `old` is not current | replace in place (`history: false`) |
| `release(key, reason)` | retire a note deliberately, leaving whatever trace the system leaves | delete (`trace: false`) |
| `purge(key)` | destroy a note | delete |
| `endorse(key, by)` | someone vouches for the note on one rung of the authority ladder: `retrieval` (it was delivered and used), `assistant` (confirmed still true), `user` (the owner approved it), `supervisor` (pinned above every other signal); a rung may repeat | nothing (`endorse_*: false`) |
| `settle()` | a session boundary: calibration, sweeps, consolidation | nothing |
| `recall(query, k, window?)` | top-k, optionally scoped to a capture-time window | rank, filter by date if it has one |
| `recall_path(path, k)` | the path-shaped read: what a caller about to touch `path` should see — an edit hook's channel, a code-ref index | hand the path to `recall` as a query (the default) |
| `suspects()` | every disagreement the system wants a person to judge | `None` (`suspects: false`) |
| `lineage(key)` | the supersession history reachable from a note | `None` |
| `standing_tokens()` | what the system costs every session before a question is asked | 0 (a file: its size) |

A **record** is `{key, kind, title, body, code_refs, created_at, open}`.
`kind` is one of Decision, Caution, Principle, Problem, Insight; `open` is
true for a Problem that has not been answered; `created_at` is the capture
time in unix seconds, always before the world's "now" (2026-09-01).
Verbs on links are `about`, `because`, `answers`, `builds-on`.

A write returns a **verdict**, if the system has one: `matched` (refused
as a near-duplicate, pointing at the existing note), `nli_label` on that
pair, `warnings` (`tombstoned` is the graded one), and any `suspects` the
write queued. A recall returns **hits** carrying the exact text shown to
the caller (a snippet or a whole note — that is what attention bills), a
score, a `tombstone` flag, the capture time and the keys of any notes the
hit carries as 1-hop context; plus `declined` (the system's own "not sure
this is in memory") and `dump` (the system does not rank; presence is
delivery).

## Capabilities

| capability | means |
|---|---|
| `link` | links are stored and can influence recall |
| `history` | a reader can walk from a note to what it replaced (`lineage`) |
| `trace` | a release leaves something findable (a marker, a tombstone) |
| `suspects` | the system nominates disagreements for a person to judge |
| `temporal` | a recall window is applied natively by the store |
| `verdict` | a recall can decline |
| `write_check` | a write can refuse a near-duplicate or warn about canon |
| `endorse_retrieval` | a note being delivered and used is counted on the note (a use counter) |
| `endorse_assistant` | the assistant's "still true" is stamped on the note (a confirmed date) |
| `endorse_user` | the owner's approval is stored on the note |
| `endorse_supervisor` | a supervisor's pin is stored on the note |

An `endorse_*` rung is declared when the endorsement is stored as a
first-class attribute of the note; whether ranking reads it is what the
authority family measures. Absent from an older transcript = false.

Families that need a missing capability are marked N/A with the reason:
contradiction and drift need `suspects`; the lineage task needs `history`;
temporal needs `temporal`; an authority task needs the rung its winning
signal sits on (per task, not per family). The headline still charges for
them (`docs/scoring.md`).

## The wire format

`--export DIR` writes one script per size; the shipped scripts are under
`worlds/v2/`:

```json
{ "spec": { "size": 100, "seed": 1, "k": 10, "pollution": 0.1, "chains": 5, "chain_len": 3, "edition": 2, "...": "..." },
  "digest": "3f0c…",
  "ops": [
    { "op": "inscribe", "record": { "key": "f0000", "kind": "Decision", "title": "...", "body": "...", "code_refs": [], "created_at": 1782000000, "open": false }, "mode": "import" },
    { "op": "link", "from": "f0003", "to": "f0011", "verb": "because" },
    { "op": "supersede", "old": "f0100", "new": { "key": "f0101", "...": "..." } },
    { "op": "settle" },
    { "op": "recall", "id": "R1", "query": "amber harbor lease broker retry budget", "k": 10 },
    { "op": "recall", "id": "R77", "query": "...", "k": 10, "window": { "after": 1781000000, "before": 1781900000 } },
    { "op": "recall_path", "id": "P1", "path": "src/lease_broker/state.rs", "k": 10 },
    { "op": "inscribe", "id": "W1", "record": { "key": "c0a", "...": "..." }, "mode": "write" },
    { "op": "suspects", "id": "S" },
    { "op": "release", "key": "f0004", "reason": "no longer applies after the lease broker rework" },
    { "op": "endorse", "key": "a3a", "by": "user" },
    { "op": "lineage", "id": "L1", "key": "f0102" }
  ],
  "probes": [
    { "id": "R1", "family": "retrieval", "expect": "gold", "gold": "f0000", "phrasing": "lexical", "stale": "s-f0000", "answer": "3 attempts" },
    { "id": "P1", "family": "retrieval", "expect": "bound", "path": "src/lease_broker/state.rs", "gold": ["f0000", "f0140"], "answers": ["retry budget of 3", "..."] },
    { "id": "C0", "family": "contradiction", "expect": "case", "gold": "f0000", "planted": ["c0a"], "witness": "c0a", "tier": 1, "shape": "value", "positive": true },
    { "id": "R412", "family": "authority", "expect": "ranked", "winner": "a3a", "losers": ["a3b", "f0011"], "order": false, "layer": 2, "scenario": "approve_vs_confirm", "needs": ["endorse_user"] }
  ] }
```

The operation order is load-bearing and the same in every world:

1. import the world (notes, links, stale siblings, supersession chains);
2. settle — a session boundary;
3. probe retrieval (every file the code refs name is also read by path),
   abstention, currency, rationale, temporal on the untouched world,
   so no family's plantings crowd another's questions;
4. plant the contradiction cases as assistant-style writes;
5. settle, then ask for the suspect queue (contradiction + drift);
6. release and purge the deletion targets, probe them, write them back;
7. (`--authority` worlds only) plant near-identical twins of the authority
   targets, endorse them, settle, and ask which comes first.

The adapter replays `ops` in order and returns a **transcript**: one reply
per op that carries an `id`, echoing that id, plus its capabilities,
standing cost and per-op timings:

```json
{ "arm": "mem0", "capabilities": { "link": false, "history": false, "trace": false, "suspects": false, "temporal": true, "verdict": false, "write_check": false, "endorse_retrieval": false, "endorse_assistant": false, "endorse_user": false, "endorse_supervisor": false },
  "standing_tokens": 0, "script_digest": "<the script's digest field, copied>",
  "replies": [
    { "reply": "recall", "id": "R1", "result": { "hits": [ { "key": "f0000", "text": "...", "score": 0.81, "created_at": 1782000000 } ] } },
    { "reply": "inscribe", "id": "W1", "result": { "matched": "f0000", "nli_label": "contradiction" } },
    { "reply": "suspects", "id": "S", "pairs": [ { "a": "c0a", "b": "f0000", "hint": "contradiction" } ] },
    { "reply": "lineage", "id": "L1", "keys": ["f0101", "f0100"] }
  ],
  "timing": { "recall": { "count": 340, "total_ms": 1234.5 } } }
```

`knowledgedrift --grade transcript.json --script script.json` scores it
exactly as an in-process arm is scored. The grader **panics on a missing
reply** rather than scoring it zero, and refuses a transcript whose digest
is not the script's: an adapter error that empties a recall must never
look like an honest "the graph is silent", and a transcript for another
world must never be scored as this one.

## The adapter contract, in one paragraph

An adapter keeps the key → native id map itself (the harness never
resolves a target by searching for it); returns exactly the text its
system would show a caller; declares only the capabilities it honestly
has; never reads the probes (the runner does not expose them); raises on
any error; and does not call a language model during replay.
