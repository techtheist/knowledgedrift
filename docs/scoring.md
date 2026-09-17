# Scoring

Every task is posed to every system; the task list is fixed by the script.
A system attempts every task its declared capabilities cover: a system with
a suspect queue, history and a clock attempts all of them; a flat store
skips the suspect, drift and lineage tasks; a file skips the temporal ones
too. In the authority family the check is per task: a task whose winning
signal sits on a rung the system did not declare (`endorse_retrieval`,
`endorse_assistant`, `endorse_user`, `endorse_supervisor`) is posed, not
attempted — the column `na_share` says how many.

## The score

The **score** is a sum, out of 1,000:

- **family points** = Σ over the eight plain families of pass rate × 100
  (N/A = 0): 0–800. The authority family, when the world poses it, is
  reported beside this and not summed.
- **signal score** = 100 × the mean **focus** over **every** retrieval
  probe (the path reads included): the share of delivered tokens that
  belonged to the answering record, counted as zero when the answer was
  not delivered, was delivered without its value readable in the text, or
  was delivered under a decline — a miss is noise, and so is hedging on a
  question the memory could answer. 0–100.
- **token score** = 100 × clamp(ln(3000 / t) / ln 15, 0, 1), where t is the
  tokens per query plus the standing tokens amortised over twenty questions
  a session (a dump is billed once — its delivered text *is* its standing
  text). 100 at 200 tokens, 15 at 2,000, 0 at 3,000, logarithmic between,
  so halving the bill is worth the same everywhere and there is no cliff to
  clip toward. 0–100.

One family point trades one-for-one against one bonus point, and the bonus
caps at 200 against the families' 800, so no system wins on efficiency
alone: a memory that abandons a family (100 points) to shorten its answers
(at most 100) loses. The equilibrium a developer is asked to find is between
knowing more, showing less, and showing the right thing.

Three rules hold on every world:

- **A hit counts only when the answer is readable.** Every retrieval,
  currency, rationale and temporal probe carries the answer substring; a
  hit whose delivered text does not contain it is a miss, however well it
  is keyed. A title-only snippet that names the note but not its value
  earns nothing.
- **A file is a query.** Every file the world's code refs name is read by
  path through `recall_path`; a system without a file channel searches the
  path string. `path_r@5` and `path_cover` sit in the retrieval family.
- **Declared capabilities are checked against their columns.** `trace`
  with no marker ever delivered, `verdict` with no recall ever declined,
  `suspects` with an empty queue, `history` with no lineage walk,
  `write_check` with no verdict on any write — each is a `flags` line in
  the receipt and in the report. Flags change no number; a review reads
  them before the numbers.

## Why the bonus is capped

The bill for a memory is paid in the reader's attention. Ten whole notes
per question, nine of them noise, cost a model context and cost a person
time whether or not the answer was among them; a delivery that is mostly
the answer costs almost nothing. So the score bills attention — but it
must not outweigh what the memory knows.

The benchmark's first edition multiplied a composite pass rate by an
attention multiplier, up to ×10 for a system that hands the reader nothing
but the answer, and the first lexical submission showed what that buys:
TF-IDF over the titles, one short snippet per answer, signal share 0.94,
scored above the reference system at a lower success rate. The additive
score with a capped bonus is the answer to that: efficiency is worth at
most 200 of 1,000 points, and only on top of what was actually known.

Every receipt still prints that earlier form beside the score —
`composite` (the unweighted mean of the eight families' pass rates, N/A =
0), `S` (mean focus over the retrieval tasks that delivered the answer)
and `multiplier` (clamp(10·S, 0.1, 10)) — as side columns, never the
headline.

## Definitions

- **success** = passed / tasks posed. An N/A task counts as failed. This
  is the headline percentage: a memory that cannot notice drift has not
  noticed it.
- **of attempted** = passed / tasks attempted. The capability-aware
  reading (ForgetEval's convention), printed beside the headline;
  identical to success for a system that attempts everything.
- **focus** — the share of delivered tokens that belonged to the answering
  record; the signal score is its mean over every retrieval probe.
- **noise** — the share of delivered records that were not the answer (a
  miss with ten hits is 1.00; an empty return is 0.00, because saying
  nothing tells no lies).
- **tokens_per_query** and **standing_tokens** — the two halves of the
  bill: what a question costs, and what the system injects every session
  before a question is asked (a memory file's size; a brief's length).

Tokens are counted with one fixed approximation (`tokens` in
`src/protocol.rs`) for every arm, so the columns compare arms, not
tokenizers.

## Stability

A percentage is quoted as stable only over three seeds at 500 (worlds
shipped). The score is a sum of means, so it holds to about a point per
arm — two for the reference system, whose calibrated decline line refits
on every run; the widest family column is the tier-3 contradiction false
alarm, which sits on six to twenty cases per world.
