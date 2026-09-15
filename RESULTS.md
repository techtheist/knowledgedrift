# KnowledgeDrift v1 — what the numbers say

A reading of the receipts under `results/v1/`. The benchmark itself —
world, protocol, families, rules, scoring — is in the README and `docs/`;
this file says what the v1 numbers mean, what they do not, and which
design decisions were made because an earlier version of the number was
misleading. Every figure is quoted from a named receipt; nothing is
rounded past what three seeds support.

## In one paragraph

Coding agents forget, but the failures that cost a project are not
amnesia: they are answering from the note that was re-decided, not
noticing that two notes disagree, bringing back what a person deleted,
and being unable to say *why* anything was decided. KnowledgeDrift pours
one seeded, invented software project into a memory system through a
ten-operation protocol, then questions, re-decides, contradicts, deletes
and re-asks it, grading every probe by a rule fixed before the question.
Measured on the official 500/1500 ladder with the same local embedder
everywhere, the reference system (Engram Alpha 0.9.4, arm `engram`) scores 85% / 80% success
(score 511 / 459) against 63% / 57% for LangMem's store, 58% / 55% for
Mem0 as a raw store, and 53% / 51% for keyword overlap — and the three
flat stores, given the same vectors, are one system: they fail the same
tasks for the same reason. The distance is not recall. It is abstention,
rationale, the suspect queue, the resurrection warning, and roughly an
eighth of the tokens per answer.

## The official ladder (seed 1)

`results/v1/reference-arms/ladder-500-1500-seed1.json`,
`results/v1/{langmem-0.0.30,mem0-2.0.20,memcontinuum-0.2.0rc5,cognee-1.5.4}/{500,1500}-seed1.json`.

| arm | success @500 | score @500 | success @1500 | score @1500 | tok/query |
|---|---|---|---|---|---|
| Engram Alpha 0.9.4 | **85%** | **511** | **80%** | **459** | 263–289 |
| langmem 0.0.30 | 63% | 51 | 57% | 52 | ~2,200 |
| rag | 63% | 51 | 57% | 52 | ~2,200 |
| mem0 2.0.20 | 58% | 45 | 55% | 47 | ~2,500 |
| cognee 1.5.4 | 57% | 39 | 52% | 39 | ~2,000 |
| grep | 53% | 44 | 51% | 43 | ~2,600 |
| memcontinuum 0.2.0rc5 | 52% | 33 | 48% | 32 | ~800 |
| whole file | 71% | 5 | 71% | 5 | 134k–402k |
| curated 3k | 9% | 4 | 5% | 4 | ~2,900 |
| chance | 4% | 15 | 4% | 13 | ~2,300 |

Five readings:

1. **Three flat stores are one system.** `rag` (the harness's own vector
   top-k), LangMem's store and Mem0 with `infer=False` land within four
   points of each other at every rung and fail identically: fp 1.00
   (nothing declines), rationale 1% at 1500 (a *why* question that only
   structure answers), the stale sibling above the answer on ~40% of
   polluted questions, and no suspect, drift or lineage task attempted.
   Mem0 is lowest because its hybrid BM25 rescoring costs oblique recall
   (0.19 vs 0.35 at 1500) — a keyword channel weighted on the wrong
   register.
2. **Recall is not where the reference system wins.** On retrieval alone
   the flat stores match its r@5 (0.76 vs 0.78 at 1500). Its lead is the
   families they cannot attempt and the bill: abstention fp 0.02 vs 1.00,
   rationale 99% vs 1%, contradiction 75% and drift 91% vs N/A,
   resurrection warned 50–69% vs N/A, and 263–289 tokens per answer at
   focus 0.52–0.57 against ~2,300 at 0.11.
3. **MemContinuum is a flat store with a chain.** Its append-only topics
   carry supersession natively — currency 85%, lineage 1.00, pollution
   0.00, the only external system to attempt the lineage walk — and it
   shows snippets, so it answers in ~800 tokens instead of ~2,300. But its
   hybrid search ranks the stale sibling above the truth on 81–89% of
   polluted questions (the sibling's short "kept for reference" ruling
   wins BM25's length normalisation and the vector over title-plus-short-
   body), rationale is 2%, nothing declines, and the per-field authority
   its schema is built around never reaches ranking (see the authority
   section). 52% (49–55) over three seeds, score 33; 48% at 1500, where
   oblique recall falls to 0.14 and *why* to 0%.
4. **cognee without its LLM is `rag` minus the clock.** With the
   extraction task out of its pipeline, cognee is documents → chunks →
   one LanceDB cosine search, and its retrieval lands where the flat
   stores land (80% (78–82) over three seeds, r@5 0.83–0.84, oblique
   0.52–0.57, the stale sibling above the truth on 37–45% of polluted
   questions). Its currency is 91% because delete + add leaves no retired
   generation to pollute, and its bill is the flat store's (~2,000 tokens
   per answer, whole notes). What separates it from `rag` is the one
   family every other flat store attempts: its chunk search has no time
   filter, so the temporal family is N/A and the headline charges for it —
   58% (56–60), score 40. At 1500 it holds the pattern: 52%, score 39,
   retrieval 72% (r@5 0.75, oblique 0.33), currency 81%, rationale 1%.
5. **The whole file is the honest ceiling on recall and the floor on
   cost.** 71% of tasks at every rung by showing everything; composite
   0.49, score 5. The curated 3,000-token file, the thing most agent
   frameworks ship as "memory", loses by 100 notes (29% → 9% → 5%).

The reference system's families at 1500 (pass rate, then the columns that
explain it):

| family | tasks | pass | reading |
|---|---|---|---|
| retrieval | 4,500 | 75% | r@5 0.78, oblique 0.34, `stale_above` 0.23 — one polluted question in four hands the reader the stale value first |
| abstention | 330 | 98% | fp 0.02, declined 0.98, separation 0.76 |
| currency | 300 | 80% | head r@5 0.74, pollution 0.00, lineage 1.00 — retired generations never come back and the history is always reachable |
| contradiction | 500 | 75% | t1 0.78, t2 0.91, t3 0.01; false alarms t2 0.03, t3 0.15 (the `historical` trap) |
| drift | 158 | 91% | noticed 0.91, flagged 0.73 |
| deletion | 375 | 83% | released and purged both gone 1.00; trace 1.00; `marker_leak` 1.00; resurrection warned 0.50 (0.69 at 500, 1.00 at 100 — the marker is crowded out of the nearest neighbours as the graph grows) |
| rationale | 540 | 99% | direct 0.00, assisted 0.99 — *why* is answered by structure, never by ranking |
| temporal | 375 | 99% | in-window r@5 0.99, leak 0.00 |

What moves with scale for it: oblique recall 0.54 → 0.34, head r@5
0.84 → 0.74, resurrection warning 0.69 → 0.50, tier-1 contradiction
0.94 → 0.78 — every one a ranking-depth effect on a growing graph; the
structural columns (pollution 0.00, lineage 1.00, gone 1.00, leak 0.00) do
not move.

## Is it stable? Three seeds at 500

`results/v1/reference-arms/{ladder-500-1500-seed1,500-seed2,500-seed3}.json`
and the matching external files. Same models, same flags; only the world
changes (607–641 notes, 2,318–2,373 tasks). Mean, then min–max:

| arm | success | composite | score |
|---|---|---|---|
| engram | **85% (84–85)** | 0.907 (0.898–0.918) | **525 (511–543)** |
| langmem | 63% (61–66) | 0.468 (0.461–0.473) | 52 (51–54) |
| rag | 63% (61–66) | 0.468 (0.461–0.473) | 52 (51–54) |
| mem0 | 59% (57–61) | 0.443 (0.438–0.447) | 46 (45–48) |
| cognee | 58% (56–60) | 0.342 (0.337–0.348) | 40 (39–42) |
| grep | 53% (51–55) | 0.418 (0.414–0.422) | 43 (42–44) |
| memcontinuum | 52% (49–55) | 0.321 (0.314–0.327) | 33 (33–34) |
| whole file | 71% (69–74) | 0.487 (0.483–0.492) | 5 (5–5) |
| curated 3k | 9% (9–9) | 0.156 (0.151–0.160) | 4 (4–4) |
| chance | 4% (4–4) | 0.131 (0.131–0.132) | 13 (11–15) |

The reference system's families over the three seeds: retrieval 81%
(80–82), abstention 100% (99–100), currency 89% (87–91), contradiction
74% (72–75), drift 93% (91–97), deletion 90% (90–90), rationale 100%
(99–100), temporal 100% (99–100). Under the contradiction number, tier-1
recall is 0.94 / 1.00 / 0.88 and tier-2 0.97 / 0.90 / 0.87 by seed; the
tier-3 false-alarm rate (the `historical` trap) is 0.24 / 0.21 / 0.37 —
the widest-swinging column, because it sits on six to ten cases. The
stale sibling outranks the answer on 0.21 / 0.32 / 0.26 of polluted
questions; the resurrection warning is 0.69 on every seed; abstention fp
never exceeds 0.01; tokens per answer 249–263.

So: the success percentages are stable to about one point for every arm,
the score to about three percent (it multiplies two means), and the
ordering never changes. LangMem and `rag` agree **on every seed** to the
same 61 / 63 / 66 — a flat store with this embedder is one system. The
ten-point gaps between the reference system and the nearest flat store,
and between the flat stores and grep, are real; a one-point gap anywhere
in this table is not.

## Pollution shapes: what gives the stale sibling away

`results/v1/reference-arms/500-seed1-{twin,late}.json` against the
default. The stale sibling is the same flipped restatement each time;
`twin` takes away the body hint (it wears the truth's own body, so only
the value and the date differ), `late` takes away the clock (it is
stamped 20–40 days *after* the truth, as a migration would). The column
that reads the shape is `stale_above`: the share of polluted questions
whose stale sibling outranked the answer.

| shape | engram success | engram `stale_above` | engram drift noticed | rag `stale_above` | grep `stale_above` |
|---|---|---|---|---|---|
| `stale` (default) | 85% | 0.21 | 0.91 | 0.38 | 0.58 |
| `twin` | 82% | **0.64** | **0.98** | 0.55 | 0.78 |
| `late` | 85% | **0.35** | 0.91 | 0.38 | 0.58 |

Two findings, both of them the point of the benchmark:

- **The body hint was carrying the ranking-side separation for
  everybody.** Take it away and the stale sibling outranks the truth on
  64% of polluted questions for the reference system, 55% for the vector
  store, 78% for grep. A ranker cannot tell apart two notes that differ
  by a number and a date, and no amount of reranking changes that — a
  reranker reads the hint, not the calendar. What holds the family up is
  the second channel: the drift queue notices 98% of twin pairs (a twin
  is closer in similarity than a hinted sibling), and the reference
  system's success falls three points, all of it in retrieval. **Ranking
  cannot resolve drift; a queue a person judges can.** That is the
  benchmark's thesis stated as a number.
- **Recency is a prior, not a truth.** `rag` and `grep` are byte-identical
  under `late` — they never look at capture time. The reference system's
  `stale_above` moves 0.21 → 0.35: a fresher stamp earns a little trust in
  its ranking, so a migration that restamps import time as capture time
  makes it prefer the re-imported past on a seventh more polluted
  questions, while success is unchanged because the drift queue is
  clock-blind and still raises every pair. The receipt says by how much.

## Authority: whose word ranks?

`results/v1/reference-arms/{500,1500}-seed1-authority.json`. The ninth family is
opt-in (`--authority`) and lands after the eighth has been asked, so its
world reproduces the plain ladder's first eight families within one task
and adds 166 tasks of its own. Each plants one to four **twins** of a note —
same body, same capture time, a different value in the title — endorses
them on a four-rung ladder (retrieval use, the assistant's confirm, the
owner's approve, a supervisor's pin) through the new `endorse` operation,
and asks the subject's paraphrase question. The stated expectation:
*higher rung wins; same rung, the later endorsement wins; count never
beats rung; no confound — a fresher stamp, a weightier kind, a body that
says it is verified — outranks a rung.* A task whose winning rung a system
does not declare is posed, not attempted.

The reference system declares three rungs (confirm, approve, pin;
retrieval use is deliberately not evidence in its trust model) and passes
**49%** of the 135 tasks it attempts at 500 and **56%** of 405 at 1500:
layers 0.47 / 0.61 / 0.50 / 0.29, then 0.59 / 0.65 / 0.58 / 0.34. The
winner is in the top five 99% of the time and first among its twins 52%
(59%) of the time. Read per scenario (500, then 1500 in brackets), the
numbers say exactly where its trust ladder reaches its ranking and where
it does not:

- **An endorsement wins when nothing else separates the twins.**
  `confirm_vs_none` 0.88 (0.92), `approve_vs_confirms` 0.88 (0.83) — one
  approval over three confirms; count does not beat rung.
- **It loses to every confound of its own size.** Trust enters the fused
  retrieval score as a multiplier, `1 + 0.15·trust`, beside a recency
  factor and a per-type rank prior of the same magnitude: a confirm (0.6
  against 0.5) is a 1.5% nudge and loses to a stamp ten days fresher
  (`confirm_vs_fresh` 0.25, 0.46 at 1500) and to the `Insight`-vs-`Decision`
  prior (`confirm_vs_kind` 0.38, 0.62); an approval (1.0 against 0.5, a 7%
  nudge) wins the retrieval half of the final rank vote and then meets the
  reranker's half, which reads title and snippet and no trust at all — so
  `approve_vs_confirm`, `approve_vs_kind`, `approve_vs_fresh_confirm` sit at
  0.62–0.71 and `approve_vs_claims`, `ladder2` near the coin flip
  (0.50–0.62).
- **Pin and approve are one number.** Both compute trust 1.0;
  `pin_vs_approve` 0.50 (0.42), `pin_vs_everything` 0.25 (0.35).
- **Equal rungs have no clock.** `latest_confirm` 0.38 (0.38),
  `latest_approve` 0.50 (0.50), `latest_pin` 0.38 (0.43) — the endorsement
  stamps are stored and never compared, so "which did the owner approve
  last" is unanswerable.
- **A pinned note survives its own supersession.** `pin_then_supersede`
  0/8 at 500 and 4/24 at 1500, with the superseded pinned twin still
  delivered 86% (78%) of the time: the
  reference system exempts pinned notes from the archive that a `replaces`
  edge otherwise performs (a pin is the user's "never fade"). The benchmark
  states the opposite reading — a re-decision must not be resurrected by a
  pin — and the column is the honest record of the disagreement.

MemContinuum declares one rung (the owner's promotion to an
`owner-ratified` ruling) and passes 2% of the 56 tasks it attempts: the
promotion writes an authority field its hybrid search filters by and never
ranks by. Every flat store declares no rung and is N/A across the family.

What the family is for, then, is not the headline: it is a per-scenario
map of which endorsements a system's *ranking* can see, against a stated
ladder. On this evidence the reference system's ladder is real in its trust
numbers and mostly invisible in its ranking, which is a product finding the
plain families could not have produced.

## Design decisions the numbers forced

Kept here because the next benchmark author will think of them too.

- **Success over attempted tasks as the headline.** The first tables read
  "83% of the tasks attempted" and made a store that cannot notice drift
  look like one that did not miss it. The headline now counts every posed
  task; the capability-aware rate is the column beside it.
- **A multiplier floored at 1.** With it, a dump scored like a memory. The
  floor is 0.1 and the score reads as a whole number (0.05 → 5, 5.50 →
  550), so a delivery that spends the reader's attention pays for it.
- **Similarity as a contradiction detector.** Full-note cosine 0.69–0.81
  for planted contradictions against 0.64–0.77 for the agreeing
  negatives; title-only and claim cosine the same. Similarity finds
  *related*, not *disagreeing*, which is why the contradiction ladder
  carries negatives in every tier and why a raised pair with no label
  fails.
- **A generated bench needs real-prose negatives.** The `collider` shape
  (a different, unrelated claim about the same subject) was added after
  sentence-pair judges were found to call two facts about one subject a
  contradiction on real project notes. Without it the tier-2 false-alarm
  column was flattering every system.
- **The `historical` tier-3 trap** (*until the rollout X …; the current
  note stands*). Every sentence-pair judge reads it as a contradiction;
  it is the family's documented false-alarm floor (t3 false alarm
  0.15–0.37), not a bug to chase.
- **No LLM arms.** Lethe and every LLM-driven memory manager are
  deliberately out: their numbers depend on a model in the loop and the
  comparison would be model, not mechanism.

## Threats to validity

- **One generator, one style.** One subject, one slot, one value per note;
  the contradiction tiers are templates. Real notes are mushier — long,
  multi-clause titles about one subject are a shape the generator does
  not produce, and the `collider` negative is the one place it tries.
- **The reference system's models grade the reference system.** Its
  reranker and NLI model are its own; the flat stores get the embedder
  only, which is the point of the comparison (mechanism, not model) and
  also its limit. A submission with a stronger embedder should be read
  against `rag` with that embedder, not against these rows.
- **Pollution is three shapes of one thing,** and drift in the wild has
  more: a note that quietly stopped being true, a tombstone whose victim
  was re-derived in different words, a supersession whose successor sits
  in another component.
- **Trust reads the wall clock** in the reference system while the
  world's clock is fixed, so its trust-modulated rankings move by a hair
  between days. The structural columns do not.
- **No behaviour.** Whether an agent *acts* on what it recalls is a
  separate, online question this benchmark does not ask.

## What v2 adds (the `v2` branch)

v1 was cracked the day it shipped: a TF-IDF store that shows one title per
answer scored 823 at 76% success (the reference system: 511 at 80%) by
riding the ×10 attention multiplier, by treating a coined subject token
nobody had written as "not in memory", by matching lexical and paraphrase
questions on the subject name, and by flagging a contradiction whenever
one token of a title changed. The in-process `tfidf` arm reproduces the
mechanism (`results/v1/reference-arms/500-seed1-tfidf.json`: 564 at 61%
on the 500 world, 99 tokens per answer, signal share 0.82). v2 keeps the
benchmark judge-free and changes what it rewards:

- **An additive score** — Σ family pass rates × 100 (800) + a signal score
  (100) + a logarithmic token score (100), so a family point trades
  one-for-one against a bonus point and efficiency alone cannot win
  (`docs/scoring.md`).
- **Signal over every probe** — a miss, an unreadable hit and a decline on
  an answerable question all count as zero signal for the tokens they cost.
- **Answer-readable credit** — every retrieval, currency, rationale and
  temporal probe carries the answer substring; a hit that does not show it
  is a miss.
- **Shared-vocabulary subjects** — `amber harbor lease broker` from two
  pools of ordinary words, so no token names a note and an unseen token is
  not an oracle.
- **A fourth, crossed phrasing** that shares no content word with its
  note, weighed equally with the other three.
- **Natural-null controls** beside the phantoms — a written subject asked
  a kind of question it has no note for.
- **Three contradiction shapes** for token differencing: `reworded`,
  `clause`, `synonym`.
- **Capability flags** — a declared capability whose column never showed
  is named in the receipt.

Measured (`results/v2/`, seven in-process arms, no external adapter yet):
at 500 over three seeds engram 738 (737–739) at 69% success, tfidf 583
(576–587) at 44%, the whole file 390, rag 375, grep 329, curated 122,
chance 116; at 1500 engram 721, tfidf 580, whole 390, rag 359,
grep 323. The score is a sum of means, so it is stable to a point per
arm across seeds. The lexical arm keeps what it honestly earns (100 token
points, ~40 signal points, the families it attempts) and loses the
oracles: phantom and natural false positives 1.00, crossed recall 0.00,
the stale sibling above the truth 70% of the time.

Still owed: a `collider`-style negative for every family; authority
scenarios with judged evidence and endorsements spread across sessions;
a pollution shape with no sibling; a second corpus register; systems with
a language model in the loop, once a judge-free way to hold the model
fixed exists.
