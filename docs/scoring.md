# Scoring

Every task is posed to every system; the task list is fixed by the script.
A system attempts every task its declared capabilities cover: a system with
a suspect queue, history and a clock attempts all of them; a flat store
skips the suspect, drift and lineage tasks; a file skips the temporal ones
too.

## Definitions

- **success** = passed / tasks posed. An N/A task counts as failed. This
  is the headline: a memory that cannot notice drift has not noticed it.
- **of attempted** = passed / tasks attempted. The capability-aware
  reading (ForgetEval's convention), printed beside the headline;
  identical to success for a system that attempts everything.
- **composite** = the unweighted mean of the eight families' pass rates,
  an N/A family scoring zero. A macro-average, so a family with twelve
  tasks weighs the same as one with three hundred.
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
