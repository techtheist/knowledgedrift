# KnowledgeDrift

**A research benchmark for how AI memory behaves across the scenarios of real use.**

Memory for coding agents is usually measured in one scenario: store a fact,
ask for it back. Real use looks different. Decisions get revised, two notes
quietly disagree, someone asks the memory to forget, someone asks *why*, and
an agent about to edit a file needs to know what was decided about it.
A memory can do well in one of these scenarios and fail completely in the
next.

KnowledgeDrift measures each scenario on its own, under controlled
conditions. The question it asks is not *which memory is best*. It asks
*what each kind of memory does in each situation, and how much the reader
has to read to get the answer*.

It is built to produce findings, not just a leaderboard:

- **Offline and judge-free.** No language model runs anywhere in the loop,
  and no model grades another. Every task has a pass/fail rule fixed before
  the question is asked.
- **Seeded and frozen.** Worlds are generated from a seed and regenerate
  byte for byte, so anyone can rerun any number from a clone.
- **Honest about gaps.** A system declares what it can do. A scenario it
  can't handle is reported as N/A with the reason, not as a mysterious zero.
- **Every receipt is shipped.** Each figure traces back to a graded
  transcript and the command that made it.

## The scenarios

A seeded world of invented project knowledge is written into the system.
Then it is questioned, re-decided, contradicted, asked to forget, and
questioned again. Each scenario is a family of tasks:

| scenario | what happens | passes when |
|---|---|---|
| **retrieval** — *find what we know* | each fact asked four ways (own words, paraphrase, oblique, no shared content word); every code-ref'd file asked by path | the right note is in the top five, readable, and not outranked by a stale copy |
| **abstention** — *know what you don't know* | a subject that was never written; a real subject with no note on this point | nothing is delivered, or it's delivered under the system's own decline |
| **currency** — *a decision was revised* | the current state of a re-decided subject; its history | the current version is in the top five with no retired one; the history reaches every version |
| **contradiction** — *a new note disagrees* | a planted note contradicts an existing one (value, unit, reworded, clause, synonym), or only seems to | real conflicts are raised for a person; look-alikes aren't |
| **drift** — *something went stale and nobody said so* | stale copies were imported quietly beside the facts | the pair is raised by the end of the run |
| **deletion** — *forget this* | a released or purged note; then the same note written back | it's gone, and a released note's rewrite comes back with a warning |
| **rationale** — *why did we decide that?* | *why does X …?* / *what answers the open issue where …?* | the reason is in the top five, directly or as a hit's context |
| **temporal** — *what happened around then?* | a question scoped to ±5 days around a capture | the answer is in the window and nothing comes from outside it |
| **authority** *(opt-in)* — *whose word counts?* | near-identical notes endorsed by different people, plus confounds | the note with the strongest endorsement ranks first |

Subjects are ordinary words (*amber harbor lease broker*), so nothing is
answerable from pretraining and no single word gives a note away. 10% of
the tested subjects are **polluted** with a stale copy that has a flipped
value, an earlier date, and no supersession. Every rule and column is in
[`docs/families.md`](docs/families.md).

## Results

Edition v2, **1,500 tested facts** (1,883 notes, 9,008 tasks), seed 1. Every
system uses the same local embedder (`bge-small-en-v1.5`).

| system | **score** | scenarios | signal | tokens | success | tok / query |
|---|---|---|---|---|---|---|
| [reference system](docs/reference-system.md) 0.10.3 | **800** | 698 | 32 | 69 | 69% | ~460 |
| TF-IDF over titles (`tfidf`) | **580** | 442 | 37 | 100 | 43% | ~160 |
| whole file in context | **390** | 390 | 0 | 0 | 71% | ~404,000 |
| MemContinuum 0.2.0rc5 | **380** | 326 | 7 | 47 | 44% | ~840 |
| Supermemory local 0.0.8 (no LLM) | **364** | 345 | 8 | 12 | 48% | ~2,200 |
| vector top-k (`rag`) | **359** | 340 | 7 | 11 | 48% | ~2,200 |
| LangMem 0.0.30 | **359** | 340 | 7 | 11 | 48% | ~2,200 |
| Mem0 2.0.20 (`infer=False`) | **348** | 333 | 6 | 8 | 44% | ~2,400 |
| keyword overlap (`grep`) | **325** | 315 | 6 | 3 | 41% | ~2,800 |
| cognee 1.5.4 (no LLM) | **263** | 240 | 8 | 16 | 43% | ~1,900 |
| chance | **112** | 103 | 0 | 9 | 3% | ~2,300 |
| curated 3,000-token file | **109** | 108 | 0 | 1 | 5% | ~2,960 |

**score** = scenarios (0–800) + signal (0–100) + tokens (0–100). See
[The score](#the-score).

**What the ladder shows so far:**

- **Given the same vectors, flat stores are one system.** LangMem matches
  vector top-k to the digit, and Mem0 and Supermemory land within about
  ten points of it. The store around the embedding barely matters.
- **No flat store abstains.** Every one of them answers questions it has no
  note for: abstention is 0–1% across the board.
- **Reading everything is not memory.** The whole file in context has the
  best retrieval (90%), but costs ~400,000 tokens a question and still
  can't notice that two notes disagree.
- **Efficiency and understanding pull apart.** A title index wins every
  token point and scores 0% on rationale.
- **Structure is the only cheap road to *why*.** Rationale stays at 1–7%
  for every flat store. It reaches ~100% only for systems that store links,
  or for the whole file at 400,000 tokens a question.
- **A file is not a query.** Systems that match the path itself find a
  file's notes every time. A vector store handed a path finds them about
  half the time: the embedding finds the component, not the file.

More: three seeds at 500, every scenario and column, and the threats to
validity in [`RESULTS.md`](RESULTS.md). Every receipt and the command behind
it is in [`results/v2/`](results/v2/README.md). To add your own system, see
[`CONTRIBUTING.md`](CONTRIBUTING.md).

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
                    grader (Rust, no models): tasks, scenarios, score
```

1. **World.** A seed generates a few hundred to a few thousand notes about
   an invented software project: links, revised subjects, stale copies,
   planted contradictions, deletions, code refs, capture times.
2. **Script.** The world becomes a list of protocol operations followed by
   probes with their expected outcomes. The script carries a digest, and a
   transcript for a different world is refused.
3. **Adapter.** It replays the operations through your system's own API and
   records one reply per probe: the exact text the system showed, plus the
   capabilities it declares.
4. **Grader.** It turns every probe into a task with one rule, groups tasks
   into scenarios, and adds up the score.

An adapter implements twelve operations: `inscribe`, `link`, `supersede`,
`release`, `purge`, `endorse`, `settle`, `recall`, `recall_path`,
`suspects`, `lineage`, `standing_tokens`. A recall returns the **exact text
shown to the reader**, which is what the token columns bill. The wire format
is in [`docs/protocol.md`](docs/protocol.md).

## Quick start

```sh
# grade a transcript (no models, builds in seconds)
cargo run --release -- --grade my-transcript.json \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json --json my-graded.json

# write a Python adapter (adapters/README.md), then replay a world through it
python3 adapters/run.py --adapter mysystem \
    --script worlds/v2/knowledgedrift-500-seed1-v2.json --out out/mysystem-500.json

# run the in-process arms (reference system + six baselines, real models)
cargo run --release --features fastembed -- --v2 --sizes 500,1500 --json ladder.json

# peek at what a world contains
cargo run -- --v2 --sizes 100 --sample
```

**The official ladder** is 500 and 1500 tested facts, seed 1, pollution 10%,
k = 10. A result counts as stable only with **three seeds at 500** (seeds 1–3
ship). Variants: `--seed N`, `--pollution-shape twin|late`, `--authority`;
`--export DIR` writes any world you generate.

## The score

Every task is posed to every system. **success** = passed / posed, and an
N/A task counts as failed: a memory that can't notice drift hasn't noticed
it.

**score** (out of 1,000) =

- **scenarios**: Σ pass rate × 100 over the eight scenarios (N/A = 0), 0–800
- **signal**: 100 × the mean share of delivered tokens that were the answer,
  over every retrieval probe (a miss or a hedge counts as 0), 0–100
- **tokens**: 100 at ≤ 200 tokens per query, 0 at 3,000, log-scale in
  between (the session's standing cost is amortised over 20 questions), 0–100

The bonus is capped at 200 against 800 for the scenarios, so no system wins
on efficiency alone. One scenario point is worth one bonus point.
Definitions: [`docs/scoring.md`](docs/scoring.md).

## Repository

```
worlds/v2/      frozen worlds: 100 / 500 / 1500 at seed 1; 500 at seeds 2, 3
src/            protocol, world generator, runner, grader, report
src/arms/       in-process arms (--features arms | fastembed)
adapters/       Python protocol mirror, runner, and the external adapters
results/v2/     every graded receipt, with how it was produced
docs/           protocol, families, scoring, the reference system
RESULTS.md      what the numbers say
CONTRIBUTING.md submitting an adapter or a result
```

## Editions

An edition freezes once its numbers are published: its worlds, the rules in
`src/grade.rs`, and the scoring. Changing any of them makes a new edition,
and results are never compared across editions. This is **v2**. The first
edition is archived under `worlds/v1/` and `results/v1/` and isn't
comparable with anything above.

## Related

- [Engram Alpha](https://github.com/techtheist/engram) — the reference system,
  and the evaluation harness this benchmark grew out of (its `eval/`
  directory holds the retrieval ladder, the supersession chains and the
  contradiction bench that became these families).
- [ForgetEval](https://arxiv.org/abs/2606.15903): the judge-free forgetting
  benchmark whose families this one absorbs, and the source of two of its
  rules: a system that can't do something says so (N/A, not zero), and
  every mutation names its target.

## License

MIT.
