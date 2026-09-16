# Scoring

Two editions. **v1** (the frozen worlds under `worlds/v1/`, every receipt
under `results/v1/`) scores composite × an attention multiplier. **v2**
(`--v2`, `worlds/v2/`, `results/v2/`) scores additively: family points plus
an efficiency bonus. The grader reads the world's `edition` and applies the
matching rule; a v1 receipt regraded today gets its v1 number.

Every task is posed to every system; the task list is fixed by the script.
A system attempts every task its declared capabilities cover: a system with
a suspect queue, history and a clock attempts all of them; a flat store
skips the suspect, drift and lineage tasks; a file skips the temporal ones
too. In the authority family the check is per task: a task whose winning
signal sits on a rung the system did not declare (`endorse_retrieval`,
`endorse_assistant`, `endorse_user`, `endorse_supervisor`) is posed, not
attempted — the column `na_share` says how many.

## Definitions

- **success** = passed / tasks posed. An N/A task counts as failed. This
  is the headline: a memory that cannot notice drift has not noticed it.
- **of attempted** = passed / tasks attempted. The capability-aware
  reading (ForgetEval's convention), printed beside the headline;
  identical to success for a system that attempts everything.
- **composite** = the unweighted mean of the pass rates of the families
  the world poses (eight; nine when it was built with `--authority`), an
  N/A family scoring zero. A macro-average, so a family with twelve tasks
  weighs the same as one with three hundred. A family the world does not
  pose is not averaged, so the plain and the authority world agree on the
  first eight.
- **S** (signal share) = mean **focus** over the retrieval tasks that
  delivered the answer, where focus is the share of delivered tokens that
  belonged to the answering record.
- **multiplier** = clamp(10 · S, 0.1, 10). Ten percent signal is the ×1
  baseline, half is ×5, a memory that hands the reader nothing but the
  answer is ×10, and a dump whose answer is one percent of what it shows
  keeps a tenth of its composite.
- **score** = 100 × composite × multiplier, read as a whole number: a
  composite of 0.87 at ×6.4 is **550**; a dump at 0.49 and ×0.1 is **5**.

## Why the multiplier

The bill for a memory is paid in the reader's attention. Ten whole notes
per question, nine of them noise, cost a model context and cost a person
time whether or not the answer was among them; a delivery that is mostly
the answer costs almost nothing. The composite says whether the answer was
there; the multiplier says how much had to be read to find it. Their
product is the score.

The multiplier is a stated sketch, not a measurement of anything but
token counts: it makes token efficiency the headline instead of a side
column, and it is printed beside the unmultiplied composite so a reader
can take it or leave it. Two things keep it honest:

- a system that answers nothing has no delivered answers, so S is 0 and
  the multiplier bottoms out at 0.1 — silence cannot buy the ×10;
- misses count noise 1.00 in the attention columns, so a system cannot
  raise its focus by declining questions it would have got wrong.

## Attention columns (no tasks)

Read off the retrieval probes:

- **focus** — share of delivered tokens that belonged to the answering
  record, when it was delivered;
- **noise** — share of delivered records that were not the answer (a miss
  with ten hits is 1.00; an empty return is 0.00, because saying nothing
  tells no lies);
- **tokens_per_query** and **standing_tokens** — the two halves of the
  bill: what a question costs, and what the system injects every session
  before a question is asked (a memory file's size; a brief's length).

Tokens are counted with one fixed approximation (`tokens` in
`src/protocol.rs`) for every arm, so the columns compare arms, not tokenizers.

## Stability

A percentage is quoted as stable only over three seeds at 500 (worlds
shipped). On v1 the success percentages hold to about one point per arm
and the score to about three percent (it multiplies two means); the
widest family column is the tier-3 contradiction false alarm, which sits
on six to ten cases per world.

## v2: the additive score

v1's multiplier let a system that answers in a few tokens earn up to ×10 on
whatever it got right — and the first lexical submission did exactly that:
TF-IDF over the titles, one short snippet per answer, signal share 0.94,
score 823 at 76% success against the reference system's 511 at 80%. The
bill for attention is real, but it must not outweigh what the memory knows.

The v2 score is a sum:

- **family points** = Σ over the eight plain families of pass rate × 100
  (N/A = 0): 0–800. The authority family, when the world poses it, is
  reported beside this and not summed.
- **signal score** = 100 × the mean **focus** over **every** retrieval
  probe (the path reads included): the share of delivered tokens that
  belonged to the answering record, counted as zero when the answer was not delivered, was delivered
  without its value readable in the text, or was delivered under a decline
  — a miss is noise, and so is hedging on a question the memory could
  answer. 0–100.
- **token score** = 100 × clamp(ln(3000 / t) / ln 15, 0, 1), where t is the
  tokens per query plus the standing tokens amortised over twenty questions
  a session (a dump is billed once — its delivered text *is* its standing
  text). 100 at 200 tokens, 15 at 2,000, 0 at 3,000, logarithmic between,
  so halving the bill is worth the same everywhere and there is no cliff to
  clip toward. 0–100.

**v2 score** = family points + signal score + token score, out of 1,000.

One family point trades one-for-one against one bonus point, and the bonus
caps at 200 against the families' 800, so no system wins on efficiency
alone: a memory that abandons a family (100 points) to shorten its answers
(at most 100) loses. The equilibrium a developer is asked to find is between
knowing more, showing less, and showing the right thing.

Two rules that hold only on v2 worlds:

- **A hit counts only when the answer is readable.** Every v2 retrieval,
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

The v1 columns (`composite`, `S`, `multiplier`) are still computed and
printed on a v2 receipt, so the two editions can be read side by side.
