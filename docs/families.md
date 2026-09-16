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
| code refs | on ~40% of notes, one to three paths `src/<component>/{mod,state,handler,config}.rs` | v2: the path-shaped read — every file is asked once, by path |
| controls | N/4 subjects that are never written | abstention |
| planted contradictions | max(6, N/3) cases across eleven shapes | the contradiction ladder |
| deletions | max(4, N/6) victims, half released with a reason, half purged | deletion honesty |
| authority twins (`--authority` only) | max(21, N/3) subjects, each with 1–4 planted twins | the authority ladder: which near-identical note comes first |

Target subjects are disjoint by construction: contradiction targets are
the tested facts at index ≡ 0 (mod 3), deletion targets ≡ 1 (mod 3) and
never polluted, temporal targets ≡ 2 (mod 3). Authority targets reuse the
≡ 2 (mod 3) facts: their twins are planted after every other family has
been asked, so they crowd nothing that was graded.

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
names the subject, shares no content vocabulary). On a v2 world a fourth,
**crossed**: oblique, and the component is described rather than named
(`the service that hands out grants — which one picked …`), so the question
shares no content word at all with the note that answers it. Rank of the
gold among live hits; a dump delivers what it holds at rank 1.

On a v2 world the family also poses the **path** read: every file the
world's code refs name is asked once through `recall_path`, as a path and
no question — what a caller about to edit the file should see. Gold is
every live, truthful note bound to the file (chain heads and distractors
included: an editor should see them; a stale sibling bound to the same
file is neither gold nor penalised). A system with a file channel — an
edit hook, a code-ref index — answers from it; one without searches the
path string. *Pass:* a bound note readable in the top five. Columns:
`path_r@5`, `path_cover` (of the notes bound to the file, the share the
top-k delivered, capped at k). The path reads count in the family's pass
rate and in the signal score, not in `weighted_r@5`.

*Pass:* gold in the top five **and not outranked by its stale sibling**
when the subject was polluted (a dump holding both is ambiguous and
fails). Columns: `r@1`, `r@5`, `mrr`, per-phrasing `r@5`, `weighted_r@5`
(45/45/10 on v1 — a stated assumption; the four phrasings weigh equally on
v2, `crossed_r@5` beside them), `stale_above` (over polluted questions),
`hedge` (the gold was delivered under a decline), `noise`, `tokens`.

### attention — what did the reader wade through?

No tasks; read off the retrieval probes. **focus** = the share of
delivered tokens that belonged to the answering record, when it was
delivered; this is the signal share `S` the multiplier reads. **noise** =
the share of delivered records that were not the answer. **tokens_per_query**
and **standing_tokens** price the two halves of the bill.

### abstention — does it say no?

One question per subject that was never written — a **phantom**. On a v2
world, as many **natural nulls** beside them: a question about a subject
that *was* written, in the template of a kind that subject has no note of
(*what is still broken in the teal creek ingest job?* when the only note
about that job is a Decision). Every word of a natural null exists in
memory; the answer does not. A phantom is caught by a detector that has
never seen a token; a natural null is caught only by one that knows what
it knows. *Pass:* nothing delivered, or delivered under the system's own
decline signal. Columns: `fp` (the failure rate; `phantom_fp` and
`natural_fp` split it on v2), `answered`, `declined`, and `separation` — the
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
one per target subject, rotating through eleven shapes in three tiers
(fourteen on a v2 world — the three marked *v2* are aimed at lexical
detectors, which the first field submission showed catch a one-token edit
and nothing else):

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
| 1 | `reworded` *v2* | + | the flip in a wholly different frame (*Operators pinned the X to 12 attempts for its retry budget*) — no token of the original sentence survives but the subject and the slot word |
| 2 | `clause` *v2* | + | the flip carried by a clause, not a token (*Contrary to the runbook, it is wrong that the X …; the earlier note was mistaken*) |
| 2 | `synonym` *v2* | − | the title with exactly one token swapped for a synonym (*uses* → *keeps*, *when* → *whenever*) — must not be flagged; a detector that flags any small edit fails here |

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

### authority — whose word ranks? (`--authority` worlds)

The world's twins. For each target subject the generator plants one to
four **twins** of the note — same body, same code refs, same capture time,
a different value in the title — and endorses them on different rungs of
one ladder:

| rung | `endorse(key, by)` | what it stands for | a system maps it to |
|---|---|---|---|
| 0 | — | nothing; the original note is always this rung | — |
| 1 | `retrieval` | the note was delivered and used | a use counter, `last_seen` |
| 2 | `assistant` | the assistant confirmed it still holds | a confirmed stamp, an `update` |
| 3 | `user` | the project's owner approved it | an approval, an owner-ratified ruling |
| 4 | `supervisor` | a supervisor pinned it above every other signal | a pin, a constant trust |

Then one session boundary, and the subject's paraphrase question. The
expectation is stated once and every scenario is an instance of it:
**a higher rung outranks a lower one; on the same rung the more recent
endorsement wins; count never beats rung** (three confirms lose to one
approval); **and no confound outranks a rung** — not a fresher capture
stamp, not a weightier kind (the winner wears `Insight`, the loser the
subject's own kind), not a body that *says* it was verified. Twenty-one
scenarios in four layers, named for who is in the loop:

| layer | who endorses | scenarios |
|---|---|---|
| 1 `autonomous` | the assistant alone (one rung) | `confirm_vs_none`, `confirm_vs_fresh`, `confirm_vs_kind`, `latest_confirm` |
| 2 `governed` | the owner over the assistant (two rungs, across kinds) | `approve_vs_confirm`, `approve_vs_confirms` (×3), `approve_vs_kind`, `approve_vs_claims`, `approve_vs_fresh_confirm`, `ladder2`, `latest_approve` |
| 3 `three_hands` | retrieval use beside both | `exposure_vs_none` (×10), `confirm_vs_exposure`, `approve_vs_exposure_confirm`, `exposure_vs_fresh`, `ladder3` |
| 4 `supervised` | a supervisor's pin above everything | `pin_vs_approve`, `pin_vs_everything` (approved + confirmed ×2 + used ×10 + fresher + claims), `latest_pin`, `pin_then_supersede`, `ladder4` |

*Pass:* the winner in the top five and ranked above every other twin (a
dump holding them all is ambiguous and fails); for `ladder2/3/4` every
delivered twin in the ladder's order; for `pin_then_supersede` the pinned
twin — superseded after its pin by an unendorsed successor — never
delivered at all (a pin must not resurrect what was re-decided). A task
whose winning signal sits on a rung the system did not declare is **posed,
not attempted**: `exposure_vs_none` needs `endorse_retrieval`,
`pin_vs_approve` needs `endorse_supervisor`, the graded ladders need every
rung in them. Columns: `l1_autonomous` … `l4_supervised` (pass rate per
layer), `winner_top`, `winner_r@5`, `order_exact`, `resurrected`,
`na_share`, and `s_<scenario>` for every scenario. N/A for a system with
no rung at all.

Three scenarios are stated knowing the reference system cannot pass them:
`latest_confirm`, `latest_approve` and `latest_pin` ask for the more recent
of two equal endorsements, and a trust model that reads rungs but not their
clocks has nothing to separate the twins with. They stay in the table
because "who endorsed it last" is a question a project owner does ask, and
the column that fails is the honest answer.

### cost

No tasks. `standing_tokens`, `tokens_per_query`, and mean milliseconds
per operation kind from the transcript's timings.
