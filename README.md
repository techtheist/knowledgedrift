# KnowledgeDrift v2

An offline, judge-free benchmark for AI memory in software development.

One seeded world of invented project knowledge is poured into a memory
system through a twelve-operation protocol, and then the system is
questioned, re-decided, contradicted, asked to forget, endorsed, and asked
again. Every probe is a **task** with a pass/fail rule fixed before the
question was asked. The headline is the **score**: every family's pass
rate as points out of 100 (eight families, 800), plus a capped efficiency
bonus — a signal score for how much of what the reader was shown was the
answer, and a token score for how little had to be read (100 each).
Nothing in the loop is a language model, and nothing asks a model whether
a model did well.

## The ladder: 1,500 tested facts

1,883 notes, 9,008 tasks, one seed, every system with the same local
embedder (`results/v2/`):

| system                                                         | success | families | signal | tokens | **score** | tok / answer |
|----------------------------------------------------------------|---|---|---|---|---|---|
| TF-IDF over titles, one snippet per answer (`tfidf`)           | 43% | 442 | 37 | 100 | **580** | ~160 |
| the whole file in context                                      | 71% | 390 | 0 | 0 | **390** | ~404,000 |
| MemContinuum 0.2.0rc5 (topics + `for-path` chains)             | 44% | 326 | 7 | 47 | **380** | ~840 |
| Supermemory local 0.0.8 (no LLM: direct memories, versioned)   | 48% | 345 | 8 | 12 | **364** | ~2,200 |
| vector top-k (`rag`)                                           | 48% | 340 | 7 | 11 | **359** | ~2,200 |
| LangMem 0.0.30 (store + semantic index)                        | 48% | 340 | 7 | 11 | **359** | ~2,200 |
| Mem0 2.0.20 (`infer=False`)                                    | 44% | 333 | 6 | 8 | **348** | ~2,400 |
| keyword overlap (`grep`)                                       | 41% | 315 | 6 | 3 | **325** | ~2,800 |
| cognee 1.5.4 (no LLM: chunk store)                             | 43% | 240 | 8 | 16 | **263** | ~1,900 |
| chance                                                         | 3% | 103 | 0 | 9 | **112** | ~2,300 |
| a curated 3,000-token file                                     | 5% | 108 | 0 | 1 | **109** | ~2,960 |
| **[Reference System](https://github.com/techtheist/engram) 0.9.9**     | **68%** | **698** | 32 | 69 | **800** | ~460 |

Three seeds at 500 tested facts, every family's pass rate, every column,
and what the numbers mean: [`RESULTS.md`](RESULTS.md). Receipts and how
each was produced: [`results/v2/`](results/v2/README.md). Submit your own
system: [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Why

Coding agents forget, but the failures that cost a project are not amnesia.
They are answering from the note that was re-decided, not noticing that two
notes disagree, bringing back what a person deleted, being unable to say
*why* anything was decided, and making the reader wade through thousands of
tokens to find the one that answers. Published memory benchmarks score
recall, and most score it with an LLM judge over chat-shaped facts.
KnowledgeDrift asks the other questions, over software-project knowledge,
with rules a script can check:

- *Is the note you found the current one, or the one we re-decided?*
- *Did you notice that these two notes disagree — including the two nobody
  told you about?*
- *Is this still in memory after I told you to forget it, and do you know
  why it left?*
- *Why did we decide that?*
- *What did we decide around the time of the outage?*
- *What should I know before I touch this file?*
- *How much did the reader have to read to get the answer?*

Two design rules are taken from [ForgetEval](https://arxiv.org/abs/2606.15903),
the judge-free forgetting benchmark whose families
this one absorbs and extends: **a system that cannot do something says
so** — capabilities are declared, and a family that needs a missing one is
marked N/A with the reason rather than scored as a mysterious zero — and
**every mutation names its target**, so the harness measures the mutation,
not the resolver.

## How it works

```
world (seeded, invented)  ──►  script (JSON: ops + probes + digest)
                                   │
                    ┌──────────────┼──────────────────┐
             in-process arms   your adapter       any adapter
             (Rust, optional)  (Python or not)    (replays the ops)
                    └──────────────┼──────────────────┘
                                   ▼
                          transcript (JSON: replies, capabilities, timings)
                                   │
                                   ▼
                    grader (Rust, no models): tasks, families, success, score
```

1. **A world** is generated from a seed: a few hundred notes about an
   invented software project, with links, re-decided subjects, a polluted
   share of stale siblings, planted contradictions, deletions, code refs
   and capture times. Subjects are a few ordinary words (*amber harbor
   lease broker*): nothing answerable from pretraining, and no single
   token names a note. The shipped worlds are frozen under `worlds/v2/`
   and regenerate byte for byte from this crate (a test checks their
   digests).
2. **A script** is the world as a list of operations — import, link,
   supersede, release, purge, endorse, settle, recall, recall by path,
   suspects, lineage — followed by probes with their expected outcomes.
   The script carries a digest; a transcript for a different world is
   refused rather than scored.
3. **An adapter** replays the operations against a memory system through
   its own API and writes a transcript: one reply per probe with the exact
   text the system showed, plus the capabilities it honestly has.
4. **The grader** scores the transcript: every probe becomes a task with
   one rule, tasks group into eight families, and the headline is the
   families' points plus the efficiency bonus.

## Quick start

```sh
# grade a transcript — no models, no features, seconds to build
cargo run --release -- --grade my-transcript.json \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json --json my-graded.json

# write an adapter in Python (see adapters/README.md), then replay
python3 adapters/run.py --adapter mysystem \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json --out out/mysystem-500.json

# run the reference arms in-process (Engram Alpha + six baselines, real models)
cargo run --release --features fastembed -- --v2 --sizes 500,1500 --json ladder.json

# look at what a world contains
cargo run -- --v2 --sizes 100 --sample
```

The official ladder is **500 and 1500 tested facts**, seed 1, pollution
10%, k 10; a result is quoted as stable only with **three seeds at 500**
(seeds 1–3 are shipped). `--pollution-shape twin|late`, `--authority` and
`--seed N` generate the variants; `--export DIR` writes any world you generate.

## The protocol

Twelve operations an adapter implements against its native API
([`docs/protocol.md`](docs/protocol.md) has the wire format):

| op | meaning |
|---|---|
| `inscribe(record, mode)` | write a note; `import` is a bulk load, `write` an assistant-style note with the system's write-time checks |
| `link(from, to, verb)` | store a sentence-shaped link (`about`, `because`, `answers`, `builds-on`) |
| `supersede(old, new)` | `new` re-decides `old`: from now on `old` is not current |
| `release(key, reason)` | retire a note deliberately, leaving whatever trace the system leaves |
| `purge(key)` | destroy a note |
| `endorse(key, by)` | someone vouches for a note: `retrieval` (delivered and used), `assistant` (confirmed still true), `user` (the owner approved it), `supervisor` (pinned) |
| `settle()` | a session boundary: calibration, sweeps, consolidation |
| `recall(query, k, window?)` | top-k, optionally scoped to a capture-time window |
| `recall_path(path, k)` | what a caller about to touch a file should see — an edit hook's channel; without one, the path is searched as a query |
| `suspects()` | every disagreement the system wants a person to judge |
| `lineage(key)` | the supersession history reachable from a note |
| `standing_tokens()` | what the system costs every session before a question is asked |

A recall returns hits carrying the **exact text shown to the caller** — a
snippet or a whole note, whatever the reader would have read — which is
what the attention columns bill. A system declares which capabilities it
has (`link`, `history`, `trace`, `suspects`, `temporal`, `verdict`,
`write_check`, and the four endorsement rungs `endorse_retrieval` /
`endorse_assistant` / `endorse_user` / `endorse_supervisor`); the families —
and, in the authority family, the tasks — that need a missing one are N/A,
and a declared capability whose column never shows is flagged in the
receipt.

## The families

Eight families — nine with `--authority` — each with one rule per task
([`docs/families.md`](docs/families.md) has every rule and column):

| family | asks | pass |
|---|---|---|
| **retrieval** | four questions per fact — its own words, a paraphrase, an oblique one that never names the subject, a crossed one that shares no content word with the note — and every file the code refs name, read by path | gold in the top five with its value readable, and not outranked by its stale sibling; for a path, a bound note in the top five |
| **abstention** | a question about a subject that was never written, and a question a written subject has no note for | nothing delivered, or delivered under the system's own decline |
| **currency** | the current state of a re-decided subject; the history behind it | the head in the top five and no retired generation delivered; the walk reaches every generation |
| **contradiction** | after a planted note contradicts (or only seems to contradict) an existing one — by value, by unit, reworded, by clause, by synonym | raised for a person to judge; a negative never raised, or raised with a clearing label |
| **drift** | nothing — the stale siblings were imported beside the facts and nobody said so | the pair raised by the end of the run |
| **deletion** | a released or purged note, by its own question; then the note written back | gone as live knowledge; a released note's rewrite comes back with a warning |
| **rationale** | *why does the X …?* and *what answers the open issue where …?* | the reason in the top five, directly or carried as context by a hit |
| **temporal** | a question scoped to ±5 days around its capture | gold inside the window and nothing delivered from outside it |
| **authority** (`--authority`) | near-identical twins of a note, endorsed on different rungs — retrieval use, the assistant's confirm, the owner's approve, a supervisor's pin — and confounded by a fresher stamp, a weightier kind, a body that *claims* to be verified | the twin the authority ladder favours ranked first, in the top five; a graded ladder in order; a superseded pin never delivered |

10% of the tested subjects are **polluted** by default: a stale sibling with
a flipped value, dated earlier, imported and never superseded by anyone.
Three shapes of it exist (`stale`, `twin`, `late`); the drift family and
the retrieval column `stale_above` read them.

The **authority** family is opt-in (`--authority`): it plants its twins
after every other family has been asked, so a run of the authority world
reproduces the plain world's eight families to the digit and adds the
ninth. Twenty-one scenarios in four layers — the assistant alone, owner
governance over the assistant, retrieval use counted beside both, a
supervisor's pin above everything — state one expectation: *retrieval <
assistant < user < supervisor*, a later endorsement beats an earlier one on
the same rung, count never beats rung, and no confound outranks a rung. A
task whose winning rung the system does not declare is posed, not
attempted.

## The score

Every task is posed to every system. **success** = passed / posed, with an
N/A task counted as failed — a memory that cannot notice drift has not
noticed it. The **score** is a sum, out of 1,000:

- **family points** — Σ over the eight families of pass rate × 100 (N/A =
  0): 0–800;
- **signal score** — 100 × the mean share of delivered tokens that belonged
  to the answer, over every retrieval probe, a miss or a hedge on an
  answerable question counting as zero: 0–100;
- **token score** — 100 at 200 tokens per query, 0 at 3,000, logarithmic
  between, the standing cost of a session amortised over twenty questions:
  0–100.

One family point trades one-for-one against one bonus point, and the bonus
caps at 200 against the families' 800, so no system wins on efficiency
alone: the equilibrium a developer is asked to find is between knowing
more, showing less, and showing the right thing.
[`docs/scoring.md`](docs/scoring.md) has the definitions.

## Layout

```
worlds/v2/         the frozen worlds (100/500/1500 seed 1; 500 seeds 2, 3)
src/               protocol, script, world generator (vendored corpus), runner, grader, report
src/arms/          the in-process arms (--features arms | fastembed)
adapters/          the Python protocol mirror, the runner, and the Mem0, LangMem, MemContinuum, cognee and Supermemory adapters
results/v2/        every graded receipt behind the tables, and how each was produced
docs/              protocol, families, scoring
RESULTS.md         what the numbers say
CONTRIBUTING.md    submitting an adapter or a result
```

## Versioning

An edition is frozen once its numbers are published: its worlds, the rules
in `src/grade.rs` and the scoring are what every result in its tables was
measured against. A change to any of them is a new edition with its own
worlds and its own tables; results are never compared across editions.
This is the second edition (`worlds/v2/`, `results/v2/`); the first keeps
its worlds, receipts and readings under `worlds/v1/` and `results/v1/` as
an archive, and nothing in it is comparable with the tables above.

## Related

- [Engram Alpha](https://github.com/techtheist/engram) — the reference system,
  and the evaluation harness this benchmark grew out of (its `eval/`
  directory holds the retrieval ladder, the supersession chains and the
  contradiction bench that became these families).
- [ForgetEval](https://arxiv.org/abs/2606.15903) — the judge-free
  forgetting benchmark whose families (supersession, drift, purge, release,
  and the adversarial probes) this benchmark absorbs, and whose two design
  rules it adopts.

## License

MIT.
