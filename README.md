# KnowledgeDrift v1

An offline, judge-free benchmark for AI memory in software development.

One seeded world of invented project knowledge is poured into a memory
system through an eleven-operation protocol, and then the system is questioned,
re-decided, contradicted, asked to forget, endorsed, and asked again. Every probe is a
**task** with a pass/fail rule fixed before the question was asked. The
headline is the **mean success over every task**, beside a macro-averaged
**composite** and an attention-multiplied **score**. Nothing in the loop is
a language model, and nothing asks a model whether a model did well.

| system (v1, 500 tested facts, 3 seeds)                     | success | score | tok / answer |
|------------------------------------------------------------|---|---|---|
| LangMem 0.0.30 (store + semantic index)                    | 63% (61–66) | 52 (51–54) | ~2,300 |
| vector top-k (`rag`)                                       | 63% (61–66) | 52 (51–54) | ~2,300 |
| Mem0 2.0.20 (`infer=False`)                                | 59% (57–61) | 46 (45–48) | ~2,600 |
| keyword overlap (`grep`)                                   | 53% (51–55) | 43 (42–44) | ~2,600 |
| MemContinuum 0.2.0rc5 (append-only topics, hybrid FTS5 + bge-small) | 52% (49–55) | 33 (33–34) | ~800 |
| the whole file in context                                  | 71% (69–74) | 5 (5–5) | ~134,000 |
| a curated 3,000-token file                                 | 9% (9–9) | 4 (4–4) | ~2,900 |
| [Engram Alpha](https://github.com/techtheist/engram) 0.9.4 | 85% (84–85) | 525 (511–543) | ~260 |

Full tables, the 1500 rung, per-family readings and receipts:
[`results/v1/`](results/v1/README.md) and [`RESULTS.md`](RESULTS.md).
Submit your own system: [`CONTRIBUTING.md`](CONTRIBUTING.md).

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
   invented software project (subjects like *Vanor lease broker* — nothing
   answerable from pretraining), with links, re-decided subjects, a polluted
   share of stale siblings, planted contradictions, deletions and capture
   times. The ten v1 worlds are frozen under `worlds/v1/` and regenerate
   byte for byte from this crate (a test checks their digests).
2. **A script** is the world as a list of operations — import, link,
   supersede, release, purge, settle, recall, suspects, lineage — followed
   by probes with their expected outcomes. The script carries a digest;
   a transcript for a different world is refused rather than scored.
3. **An adapter** replays the operations against a memory system through
   its own API and writes a transcript: one reply per probe with the exact
   text the system showed, plus the capabilities it honestly has.
4. **The grader** scores the transcript: every probe becomes a task with
   one rule, tasks group into eight families, and the headline is passed
   over posed.

## Quick start

```sh
# grade a transcript — no models, no features, seconds to build
cargo run --release -- --grade my-transcript.json \
    --script worlds/v1/knowledgedrift-500-seed1.json --json my-graded.json

# write an adapter in Python (see adapters/README.md), then replay
python3 adapters/run.py --adapter mysystem \
    --script worlds/v1/knowledgedrift-500-seed1.json --out out/mysystem-500.json

# run the reference arms in-process (Engram Alpha + five baselines, real models)
cargo run --release --features fastembed -- --sizes 500,1500 --json ladder.json

# look at what a world contains
cargo run -- --sizes 100 --sample
```

The official ladder is **500 and 1500 tested facts**, seed 1, pollution
10%, k 10; a result is quoted as stable only with **three seeds at 500**
(seeds 1–3 are shipped). `--pollution-shape twin|late`, `--authority` and
`--seed N` generate the variants; `--export DIR` writes any world you generate.

## The protocol

Eleven operations an adapter implements against its native API
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
| `suspects()` | every disagreement the system wants a person to judge |
| `lineage(key)` | the supersession history reachable from a note |
| `standing_tokens()` | what the system costs every session before a question is asked |

A recall returns hits carrying the **exact text shown to the caller** — a
snippet or a whole note, whatever the reader would have read — which is
what the attention columns bill. A system declares which capabilities it
has (`link`, `history`, `trace`, `suspects`, `temporal`, `verdict`,
`write_check`, and the four endorsement rungs `endorse_retrieval` /
`endorse_assistant` / `endorse_user` / `endorse_supervisor`); the families —
and, in the authority family, the tasks — that need a missing one are N/A.

## The families

Eight families — nine with `--authority` — each with one rule per task
([`docs/families.md`](docs/families.md) has every rule and column):

| family | asks | pass |
|---|---|---|
| **retrieval** | three questions per fact — its own words, a paraphrase, an oblique one that never names the subject | gold in the top five, and not outranked by its stale sibling |
| **abstention** | a question about a subject that was never written | nothing delivered, or delivered under the system's own decline |
| **currency** | the current state of a re-decided subject; the history behind it | the head in the top five and no retired generation delivered; the walk reaches every generation |
| **contradiction** | after a planted note contradicts (or only seems to contradict) an existing one | raised for a person to judge; a negative never raised, or raised with a clearing label |
| **drift** | nothing — the stale siblings were imported beside the facts and nobody said so | the pair raised by the end of the run |
| **deletion** | a released or purged note, by its own question; then the note written back | gone as live knowledge; a released note's rewrite comes back with a warning |
| **rationale** | *why does the X …?* and *what answers the open issue where …?* | the reason in the top five, directly or carried as context by a hit |
| **temporal** | a question scoped to ±5 days around its capture | gold inside the window and nothing delivered from outside it |
| **authority** (`--authority`) | near-identical twins of a note, endorsed on different rungs — retrieval use, the assistant's confirm, the owner's approve, a supervisor's pin — and confounded by a fresher stamp, a weightier kind, a body that *claims* to be verified | the twin the authority ladder favours ranked first, in the top five; a graded ladder in order; a superseded pin never delivered |

10% of the tested subjects are **polluted** by default: a stale sibling with
a flipped value, dated earlier, imported and never superseded by anyone.
Three shapes of it exist (`stale`, `twin`, `late`); the drift family and
the retrieval column `stale_above` read them.

The **authority** family is opt-in (`--authority`; worlds
`knowledgedrift-{100,500,1500}-seed1-authority.json`): it plants its twins after
every other family has been asked, so a run of the authority world reproduces
the plain world's eight families to the digit and adds the ninth. Twenty-one
scenarios in four layers — the assistant alone, owner governance over the
assistant, retrieval use counted beside both, a supervisor's pin above
everything — state one expectation: *retrieval < assistant < user <
supervisor*, a later endorsement beats an earlier one on the same rung, count
never beats rung, and no confound outranks a rung. A task whose winning rung
the system does not declare is posed, not attempted.

## The numbers

Every task is posed to every system. **success** = passed / posed, with an
N/A task counted as failed — a memory that cannot notice drift has not
noticed it. **of attempted** is printed beside it for the capability-aware
reading. **composite** is the unweighted mean of the eight family pass
rates the world poses (eight, or nine with `--authority`). **S** is the share
of delivered tokens that belonged to the answer;
the **multiplier** is clamp(10·S, 0.1, 10); **score** = 100 × composite ×
multiplier, read as a whole number (0.87 at ×6.4 is 550; a dump at 0.49 and
×0.1 is 5).

The multiplier is the benchmark's stance: the less polluted a delivery is,
the less of the model's attention it spends, and a memory that hands the
reader nothing but the answer is worth ten times one that buries it in ten
whole notes. It is printed beside the unmultiplied composite so a reader
can take it or leave it, and floored so that silence cannot buy it.
[`docs/scoring.md`](docs/scoring.md) has the definitions.

## Layout

```
worlds/v1/         the frozen v1 worlds (100/500/1500 seed 1; 500 seeds 2, 3; 500 twin, late; 100/500/1500 authority)
src/               protocol, script, world generator (vendored corpus), runner, grader, report
src/arms/          the in-process arms (--features arms | fastembed)
adapters/          the Python protocol mirror, the runner, and the Mem0, LangMem and MemContinuum adapters
results/v1/        every graded receipt behind the tables, and how each was produced
docs/              protocol, families, scoring
RESULTS.md         what the v1 numbers say
CONTRIBUTING.md    submitting an adapter or a result
```

## Versioning

**v1** is frozen: the worlds under `worlds/v1/`, the rules in `src/grade.rs`
and the scoring are what every v1 result was measured against. A change to
any of them is a v2 with its own worlds and its own tables; results are
never compared across versions.

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
