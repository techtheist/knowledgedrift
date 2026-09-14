# The world and the families

## The world

Every subject is coined (*Vanor lease broker*), so nothing can be answered
from pretraining and every answer is a substring the generator knows —
grading never needs a judge. Every note is shaped to a real graph's
profile (title and body length by quartile, code refs, edge mix) by the
vendored corpus generator (`src/corpus/`), and `src/world.rs` adds what
the benchmark is about:

| ingredient | how many at `--sizes N` | what it is for |
|---|---|---|
| tested facts | N, five kinds evenly (Decision, Caution, Principle, Problem, Insight) | every one is questioned three ways; the others are the noise |
| links | ~1.06 per note, verbs `about` / `builds-on` / `because` / `answers` | structure the rationale family walks |
| re-decided subjects | max(4, N/20) chains of `--chain-len` generations, a month apart | currency: is the head current, is the history reachable |
| **pollution** | `--pollution` share of tested subjects (default **10%**), shaped by `--pollution-shape` | a stale sibling of the fact — same subject, flipped value — imported and **never superseded by anyone** |
| capture times | spread over 60 days | temporal scoping; the world's "now" is 2026-09-01 |
| controls | N/4 subjects that are never written | abstention |
| planted contradictions | max(6, N/3) cases across eleven shapes | the contradiction ladder |
| deletions | max(4, N/6) victims, half released with a reason, half purged | deletion honesty |

Target subjects are disjoint by construction: contradiction targets are
the tested facts at index ≡ 0 (mod 3), deletion targets ≡ 1 (mod 3) and
never polluted, temporal targets ≡ 2 (mod 3).

### Pollution shapes

The stale sibling is the same flipped restatement under every shape; what
`--pollution-shape` changes is which of the three signals that could give
it away — the wording, the body, the clock — is left in. Each shape
removes one, so a family's pollution number can be attributed to a
mechanism instead of assumed.

| shape | title | body | capture time | what it isolates |
|---|---|---|---|---|
| `stale` (default) | flipped, generator's wording | *"Recorded before the X was re-tuned; kept for reference."* | 20–40 days **before** the truth | everything a reader could use is present |
| `twin` | flipped, generator's wording | **the truth's own body**, verbatim | 20–40 days before | the unlinked successor's victim: value and clock are the only difference — can anything but a suspect queue tell them apart? |
| `late` | flipped, generator's wording | the `stale` hint | 20–40 days **after** the truth (a migration stamps import time as capture time) | the clock now lies: does any arm read recency as truth? |

The default world's digest does not change because the knob exists (the
shape is omitted from the file when it is the default).

## The families

Each family poses tasks; each task has one rule. Columns are means over
the family's tasks unless noted.

### retrieval — does it come back?

Three questions per tested fact: **lexical** (the fact's own words),
**paraphrase** (names the subject, rewords the rest), **oblique** (never
names the subject, shares no content vocabulary). Rank of the gold among
live hits; a dump delivers what it holds at rank 1.

*Pass:* gold in the top five **and not outranked by its stale sibling**
when the subject was polluted (a dump holding both is ambiguous and
fails). Columns: `r@1`, `r@5`, `mrr`, per-phrasing `r@5`, `weighted_r@5`
(45/45/10 — a stated assumption), `stale_above` (over polluted questions),
`hedge` (the gold was delivered under a decline), `noise`, `tokens`.

### attention — what did the reader wade through?

No tasks; read off the retrieval probes. **focus** = the share of
delivered tokens that belonged to the answering record, when it was
delivered; this is the signal share `S` the multiplier reads. **noise** =
the share of delivered records that were not the answer. **tokens_per_query**
and **standing_tokens** price the two halves of the bill.

### abstention — does it say no?

One question per subject that was never written. *Pass:* nothing
delivered, or delivered under the system's own decline signal. Columns:
`fp` (the failure rate), `answered`, `declined`, and `separation` — the
balanced accuracy of the best threshold between answerable and control
top scores, the threshold-free number a system with no decline rule can
still be read on. FP is never printed without recall beside it: a mute
system wins it for free.

### currency — is that the current one?

Every re-decided subject is asked about its current state, three ways.
*Pass:* the head in the top five and **no retired generation delivered at
all**. One lineage task per chain: *pass* = walking the history from the
head reaches every retired generation (N/A for a system without history).
Columns: `head_r@1`, `head_r@5`, `pollution`, `lineage`.

### contradiction — does it notice the disagreement?

Cases are planted as assistant-style writes after the world is probed,
one per target subject, rotating through eleven shapes in three tiers:

| tier | shape | polarity | what it plants |
|---|---|---|---|
| 1 | `value` | + | the same claim with a flipped value or polarity, near the original's wording — similarity alone should catch it |
| 2 | `negation` | + | *It is not the case that the X …* |
| 2 | `unit` | + | a different value in a converted unit (7 seconds vs 19000 milliseconds) |
| 2 | `quantifier` | + | *every deployment / without exception / has not happened once* |
| 2 | `paraphrase` | − | an agreeing restatement — must not be flagged |
| 2 | `unit_agree` | − | the same value in a converted unit — must not be flagged |
| 2 | `coreference` | − | the flipped claim about a *different* subject in the same component — must not be flagged |
| 2 | `collider` | − | a different, unrelated claim about the *same* subject (another kind's predicate re-attached to it) — must not be flagged |
| 3 | `transitive` | + | two notes: *X mirrors every setting of G* and the flipped claim about G — the contradiction exists only across the bridge |
| 3 | `compound` | + | a sentence that agrees with the note and then, as a separate matter, contradicts it |
| 3 | `historical` | − | *until the 3.1 rollout X …; the rollout changed that and the current note stands* — reads as a contradiction, is history |

Tier 2 is where a sentence-pair logic layer earns its place; tier 3 is
what an encoder-only system is not expected to survive, and the score
documents that ceiling rather than hiding it. A pair counts as **raised**
if it sits in the suspect queue at the end of the run, was queued by the
write itself, or the write was refused as a near-duplicate of the gold.
The strongest hint attached is the system's label.

*Pass, positive:* raised, not **absorbed** (merged away as a duplicate
without a contradiction label), and not hinted `entailment`. *Pass,
negative:* never raised, or raised with a clearing hint (`entailment` /
`neutral`), or merged as the duplicate it is. Raised with a contradiction
hint or with no hint at all is a wasted human judgment and fails. Columns:
`t1_recall` `t2_recall` `t3_recall`, `t2_false_alarm` `t3_false_alarm`,
`queued`, `flagged`, `absorbed`. N/A for a system with no suspect queue.

### drift — did it notice what nobody told it about?

One task per polluted subject: the stale sibling was imported beside the
fact, never superseded, never planted as a "case". *Pass:* the system
raised the pair on its own by the end of the run, without hinting
`entailment`. This is the family the benchmark is named for. N/A without a
suspect queue.

### deletion — does it stay gone, and does it know why?

Half the victims are **released** with a reason, half **purged**. Each is
then asked for by its own lexical question, and then written back. *Pass
(absent):* the victim is gone as live knowledge (a deletion marker flagged
`tombstone` does not count as the victim). *Pass (resurrect, released
only):* writing the victim again comes back with a `tombstoned` warning.
Columns: `released_gone`, `purged_gone`, `trace` (a marker was delivered),
`marker_leak` (a marker's text still carries the victim's answer — the
ForgetEval finding, measured), `resurrection_warned`,
`purged_rewrite_warned` (a column, never a task: nobody can warn about a
note that no longer exists).

### rationale — can it answer *why*?

For `because` edges: *why does the X …?* with the reason at the far end
as gold. For `answers` edges: *what answers the open issue where …?* with
the resolution as gold. *Pass:* gold in the top five directly, **or
carried as 1-hop context by a top-five hit**. Columns: `direct_r@5`,
`assisted_r@5`, `structure_only` — the share only the graph could reach.

### temporal — what did we decide around then?

The paraphrase question for a subject, scoped to ±5 days around its
capture. *Pass:* gold in the top five and no delivered hit captured
outside the window. Columns: `in_window_r@5`, `leak`. N/A for a system
without a clock (a file).

### cost

No tasks. `standing_tokens`, `tokens_per_query`, and mean milliseconds
per operation kind from the transcript's timings.
