# KnowledgeDrift — what the numbers say

A reading of the receipts under `results/v2/`. The benchmark itself —
world, protocol, families, rules, scoring — is in the README and `docs/`;
this file says what the numbers mean and what they do not. Every figure is
quoted from a named receipt; nothing is rounded past what three seeds
support.

**Environment.** One Apple-silicon laptop, CPU only, 2026-09-16;
`BAAI/bge-small-en-v1.5` for every embedding arm; the reference system
(Engram Alpha 0.9.6, arm `engram`) with `jina-reranker-v1-turbo-en` and
`deberta-v3-small-tasksource-nli`. The previous reference version (0.9.5)
was measured the same day on the same worlds; its receipts stay under
`results/v2/reference-arms/0.9.5/` and its rows are quoted where the two
differ. Eleven systems: the reference system,
six in-process baselines (`tfidf`, `whole`, `rag`, `grep`, `curated`,
`chance`) and four external adapters (MemContinuum, LangMem, Mem0, cognee)
replaying the exported scripts through their own APIs. External systems
ran one seed; the in-process arms ran three at 500.

## In one paragraph

A memory that knows what it knows leads by 221 points at 1,500 tested
facts and 235 at 500, and the distance is not recall. The reference
system's retrieval family is *under* the vector store's (57% against 57%
at 1500, 63% against 65% at 500): what it earns is abstention (99%
where every flat store answers every question it should not), a suspect
queue that notices planted contradictions (70%) and silent drift (91–93%),
a resurrection warning when a deleted note comes back (100%), rationale
through its edges, and an answer a fifth the size. Its previous version
(0.9.5) scored 718 and 739 on the same worlds with the queue noticing
under half of both; the retrieval, abstention and token columns did not
move between the two. The lexical arm shows the
other edge of the score: 100 token points and 37–42 signal points for
answering in a title, and a family column that says exactly what a title
cannot do. The three flat stores given the same vectors are one system —
LangMem is `rag` to the digit at both sizes — and the one external system
with a structural channel, MemContinuum, is the only one whose rationale
family is not a rounding error.

## The ladder: 1,500 tested facts (seed 1)

1,883 notes, 9,008 tasks: 6,064 retrieval (four phrasings and 64 path
reads), 714 abstention, 300 currency, 500 contradiction, 158 drift, 375
deletion, 522 rationale, 375 temporal.
`results/v2/reference-arms/1500-seed1.json` and
`results/v2/<system>/1500-seed1.json`.

| system | success | passed / attempted | families (800) | signal (100) | tokens (100) | **score** | billed tok/query |
|---|---|---|---|---|---|---|---|
| **engram 0.9.6** | 69% | 6,177 / 9,008 | 700 | 32 | 69 | **801** | 457 |
| engram 0.9.5 | 66% | 5,927 / 9,008 | 616 | 32 | 69 | 718 | 457 |
| tfidf | 43% | 3,886 / 8,808 | 442 | 37 | 100 | **580** | 158 |
| whole file | 71% | 6,429 / 7,775 | 390 | 0 | 0 | **390** | 404,164 |
| MemContinuum | 44% | 3,984 / 7,850 | 326 | 7 | 47 | **380** | 844 |
| rag | 48% | 4,283 / 8,150 | 340 | 7 | 11 | **359** | 2,208 |
| LangMem | 48% | 4,281 / 8,150 | 340 | 7 | 11 | **359** | 2,201 |
| Mem0 | 44% | 4,001 / 8,150 | 333 | 6 | 8 | **348** | 2,438 |
| grep | 41% | 3,651 / 8,150 | 315 | 6 | 3 | **325** | 2,766 |
| cognee | 43% | 3,836 / 7,775 | 240 | 8 | 16 | **263** | 1,934 |
| chance | 3% | 285 / 8,150 | 103 | 0 | 9 | **112** | 2,333 |
| curated (3k) | 5% | 424 / 7,775 | 108 | 0 | 1 | **109** | 2,959 |

**By family** (pass rate; n/a = the family needs a capability the system
does not have, and is scored zero in the headline):

| system | retrieval | abstention | currency | contradiction | drift | deletion | rationale | temporal |
|---|---|---|---|---|---|---|---|---|
| engram 0.9.6 | 57% | 99% | 81% | 70% | 93% | 100% | 99% | 100% |
| engram 0.9.5 | 57% | 96% | 81% | 47% | 48% | 87% | 99% | 100% |
| tfidf | 45% | 0% | 67% | 58% | 73% | 100% | 0% | 100% |
| whole file | 90% | 0% | 100% | n/a | n/a | 100% | 100% | n/a |
| MemContinuum | 49% | 0% | 77% | n/a | n/a | 100% | 100% | n/a |
| rag | 57% | 0% | 80% | n/a | n/a | 100% | 4% | 99% |
| LangMem | 57% | 0% | 80% | n/a | n/a | 100% | 4% | 99% |
| Mem0 | 52% | 0% | 74% | n/a | n/a | 100% | 7% | 100% |
| grep | 47% | 0% | 67% | n/a | n/a | 100% | 1% | 100% |
| cognee | 56% | 0% | 80% | n/a | n/a | 100% | 3% | n/a |
| chance | 0% | 0% | 0% | n/a | n/a | 100% | 0% | 2% |
| curated (3k) | 2% | 0% | 0% | n/a | n/a | 100% | 6% | n/a |

**Columns worth reading at 1500:**

| system | crossed_r@5 | oblique_r@5 | path_r@5 | path_cover | stale_above | phantom_fp | natural_fp | hedge | t1_recall |
|---|---|---|---|---|---|---|---|---|---|
| engram 0.9.6 | 0.02 | 0.32 | 1.00 | 0.98 | 0.17 | 0.01 | 0.02 | 0.35 | 0.85 |
| engram 0.9.5 | 0.02 | 0.32 | 1.00 | 0.98 | 0.17 | 0.01 | 0.06 | 0.34 | 0.04 |
| tfidf | 0.00 | 0.01 | 0.08 | 0.03 | 0.63 | 1.00 | 1.00 | 0.00 | 0.42 |
| MemContinuum | 0.02 | 0.14 | 1.00 | 0.87 | 0.58 | 1.00 | 1.00 | 0.00 | – |
| rag | 0.09 | 0.34 | 0.56 | 0.18 | 0.28 | 1.00 | 1.00 | 0.00 | – |
| LangMem | 0.09 | 0.34 | 0.56 | 0.19 | 0.28 | 1.00 | 1.00 | 0.00 | – |
| Mem0 | 0.03 | 0.18 | 0.61 | 0.18 | 0.29 | 1.00 | 1.00 | 0.00 | – |
| grep | 0.00 | 0.05 | 1.00 | 0.92 | 0.44 | 1.00 | 1.00 | 0.00 | – |
| cognee | 0.09 | 0.29 | 0.47 | 0.18 | 0.26 | 1.00 | 1.00 | 0.00 | – |

## Is it stable? Three seeds at 500

630 notes and 3,042 tasks per world (2,063 retrieval, of which 63 path
reads; 238 abstention, of which 125 natural nulls; 100 currency; 166
contradiction; 55 drift; 125 deletion; 170 rationale; 125 temporal).
Mean, then min–max, over `results/v2/reference-arms/500-seed{1,2,3}.json`.
The score is a sum of means, so it is stable to about a point per arm —
two for the reference system, whose calibrated decline line refits on
every run (816 / 818 / 813 on the three seeds; on 0.9.5, 741 / 736 / 739,
and an identical rerun of its seed 1 gave 739).

| arm | success | families (800) | signal (100) | tokens (100) | **score** |
|---|---|---|---|---|---|
| **engram 0.9.6** | **73% (72–73)** | **709 (705–711)** | 36 (35–37) | 71 (70–71) | **816 (813–818)** |
| engram 0.9.5 | 70% (70–71) | 632 (629–636) | 36 (35–37) | 71 (70–71) | 739 (736–741) |
| tfidf | 44% (43–46) | 439 (433–445) | 42 (41–44) | 100 | **581 (574–586)** |
| whole file | 72% (70–75) | 390 (387–394) | 0 | 0 | **390 (387–394)** |
| rag | 53% (52–55) | 358 (355–360) | 8 (8–9) | 9 (7–10) | **375 (374–376)** |
| grep | 43% (41–45) | 322 (320–323) | 9 | 3 (3–4) | **334 (332–335)** |
| curated (3k) | 8% (8–9) | 122 (120–125) | 0 | 1 | **123 (120–126)** |
| chance | 4% (3–4) | 107 (106–107) | 0 | 9 (8–10) | **116 (116–117)** |

The external systems on seed 1 at 500 (`results/v2/<system>/500-seed1.json`;
their three-seed spread is not yet measured):

| system | success | families (800) | signal (100) | tokens (100) | **score** | billed tok/query |
|---|---|---|---|---|---|---|
| MemContinuum | 48% | 337 | 9 | 46 | **392** | 852 |
| LangMem | 53% | 359 | 8 | 9 | **376** | 2,360 |
| Mem0 | 47% | 334 | 7 | 6 | **347** | 2,567 |
| cognee | 48% | 259 | 8 | 14 | **281** | 2,053 |

**By family at 500** (three seeds for the in-process arms, seed 1 for the
external systems):

| system | retrieval | abstention | currency | contradiction | drift | deletion | rationale | temporal |
|---|---|---|---|---|---|---|---|---|
| engram 0.9.6 | 63% (62–64) | 99% (98–100) | 88% | 70% (68–72) | 91% | 100% (99–100) | 98% (97–99) | 100% |
| engram 0.9.5 | 63% (62–64) | 100% (99–100) | 88% | 46% | 44% (41–47) | 93% (91–94) | 98% (97–99) | 100% |
| tfidf | 46% (44–48) | 0% | 69% | 59% | 65% (60–73) | 100% | 0% | 100% |
| whole file | 90% (87–94) | 0% | 100% | n/a | n/a | 100% | 100% | n/a |
| MemContinuum | 55% | 0% | 82% | n/a | n/a | 100% | 100% | n/a |
| LangMem | 64% | 0% | 91% | n/a | n/a | 100% | 4% | 100% |
| rag | 65% (64–66) | 0% | 89% (87–91) | n/a | n/a | 100% | 4% (3–5) | 100% |
| Mem0 | 56% | 0% | 73% | n/a | n/a | 100% | 5% | 100% |
| grep | 51% (49–52) | 0% | 68% (68–69) | n/a | n/a | 100% | 2% (2–3) | 100% |
| cognee | 63% | 0% | 92% | n/a | n/a | 100% | 4% | n/a |
| curated (3k) | 7% | 0% | 0% | n/a | n/a | 100% | 15% (13–17) | n/a |
| chance | 1% | 0% | 0% | n/a | n/a | 100% | 1% (0–3) | 5% (3–6) |

**Columns by seed** (seed 1 / 2 / 3): engram `crossed_r@5` 0.02 / 0.04 /
0.02, `hedge` 0.39 / 0.40 / 0.38, `stale_above` 0.15 / 0.22 / 0.19,
`natural_fp` 0.02 / 0.00 / 0.02, `phantom_fp` 0.01 / 0.00 / 0.00, tier-1
contradiction recall 0.83 / 0.79 / 0.75 (0.9.5: 0.04 / 0.04 / 0.00), tier-2
0.56 / 0.58 / 0.54, tier-3 0.12 / 0.04 / 0.04, tier-3 false alarm 0.05 on
every seed, drift noticed 0.91 on every seed, `resurrection_warned` 1.00 /
0.98 / 1.00, `path_r@5` 1.00 on every seed.
tfidf `crossed_r@5` 0.00 / 0.00 / 0.01, `stale_above` 0.70 / 0.71 / 0.71,
`natural_fp` and `phantom_fp` 1.00 on every seed, tier-1 recall 0.38 on
every seed, `path_r@5` 0.21 / 0.44 / 0.32. rag `path_r@5` 0.68 / 0.66 /
0.60 at `path_cover` 0.34–0.36; grep 1.00 at 0.97–0.99.

## What the numbers say

**The flat stores are one system.** LangMem and `rag` agree to the digit
at both sizes (376 / 359); Mem0 and cognee sit within a few points of
them on every family they attempt. Given the same embedder, a vector
store is a vector store: it retrieves 57–65% of questions, answers every
phantom and every natural null (`phantom_fp` and `natural_fp` 1.00), puts
the stale sibling above the truth on about a quarter of polluted
questions, and cannot say why anything was decided (rationale 3–7%). The
difference between them is a clock (cognee has none: temporal N/A) and
the length of the text they show.

**The reference system's lead is not recall.** At 1500 its retrieval
family is 57%, level with `rag`; its `crossed_r@5` is 0.02 against
`rag`'s 0.09 (a question sharing no content word with its note is where
the keyword channel and the reranker vote have nothing to hold); and its
calibrated "not in memory" line hedges on 35–39% of answerable questions,
which the signal score counts as zero — the signal score is 32–36 where
the retrieval focus alone would give about 60. The lead is the families
the flat stores score zero on: abstention 99%, contradiction 70% (tier-1
recall 0.75–0.85, tier-2 0.52–0.58, tier-3 0.04–0.12, and a 0.05–0.06
false alarm on the `historical` trap), drift 91–93%, rationale 98–99%
through its edges, deletion 100% with a resurrection warning on every
released victim, and 450 billed tokens a query against 2,200–2,800. Its
0.9.5 rows show what a queue that does not fire looks like: contradiction
46–47%, drift 44–48%, deletion 87–93%, the same retrieval.

**The lexical arm shows the ceiling of a title.** `tfidf` earns 100 token
points and 37–42 signal points by answering in a snippet, and 58–59% on
contradiction from token differencing (`value` and `unit` flips, tier-1
recall 0.38–0.42). Its columns show what a title index cannot do: the
crossed phrasing 0.00, the oblique 0.01–0.05, abstention 0% (every word of
a coined subject was written somewhere, so an unseen token is not an
oracle), the stale sibling above the truth 63–71% of the time, the path
read 0.08–0.44, rationale 0%.

**The whole file is the cheapest honest memory and the most expensive
reader.** Every note in context scores 71–72% success — the highest of
any system — and 390: it pays 404,164 tokens a query at 1500 for 0 signal
and 0 token points, and it cannot abstain, cannot notice a contradiction,
and holds the stale sibling beside the truth on every polluted question
(`stale_above` 1.00). The curated 3,000-token file is the same idea with a
budget, and at 500 notes the budget holds 7% of the answers.

**MemContinuum's channel is the chain, and the chain carries the
rationale.** Its author's review of an earlier measurement said the bench
had read `search` where the agent reads the hook-injected
`for-path`/`chain`. The adapter now writes the schema's typed edges on
`link`, delivers a hit's edge-neighbours, and answers the path read with
`for-path`: rationale is 100% (all of it `structure_only` — the reason
arrives as context, never in the top five itself), the path read is 1.00
at `path_cover` 0.95 / 0.87, and the score is 392 / 380. What the author
did not dispute stayed where it was: `stale_above` 0.58–0.62 (the widest
of any system: its FTS5 channel prefers the sibling's shorter title),
abstention 0%, no suspect queue, no clock. Its unranked chain text costs
850 tokens a query, the only external bonus worth having (46–47 token
points).

**The path read separates three kinds of system.** A file channel —
engram's code-ref match, grep's substring match over the note files,
MemContinuum's `for-path` — answers every file and covers 0.87–1.00 of
what is bound. A vector store handed a path embeds the string and finds
the *component's* notes: one bound note in the top five for 0.47–0.68 of
files, never the file (cover 0.18–0.36). A title index barely sees it
(0.08–0.44). The read is 63–64 of 3,042–9,008 tasks, so it moves no
headline by more than a point; it is a column.

## Threats to validity

- **One seed for the external systems.** The in-process arms are stable
  to a point across three seeds at 500; the external rows are quoted from
  seed 1 and should be read with that spread in mind until their seeds
  2 and 3 are run.
- **The reference system moves by two points between identical runs.**
  Its decline line is fitted per run on graph-vocabulary probes; the
  three-seed range (813–818) is partly that refit, not only the seeds,
  and the abstention column (99%, `natural_fp` 0.00–0.02) sits on that
  line. Every other arm reproduces to the digit.
- **Contended wall-clock.** Every receipt was taken with three chains
  running side by side on one laptop; the timing columns say nothing
  about any system's speed.
- **One embedder, one register.** Every embedding arm uses the same
  384-dimensional model, and every world is one corpus register
  (software-project notes with three-word subjects). A system tuned for
  another embedder or another register is not measured here.
- **Adapters are the adapter author's reading of a system.** Each maps
  twelve operations onto a native API and declares capabilities; a
  deviation is listed in the adapter's notes, and a system's author can
  submit a different mapping by pull request.

## Not yet run

Seeds 2 and 3 at 500 for the external systems; the 100 rung; the
authority world; a path read on a directory rather than a file, and a
path that binds no note; a second corpus register; systems with a
language model in the loop, once a judge-free way to hold the model fixed
exists.
