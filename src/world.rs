//! The world: an invented software project poured into a script.
//!
//! Every subject is coined (`Vanor lease broker`), so nothing can be answered
//! from pretraining and every answer is a substring the generator knows —
//! grading never needs a judge. The notes come from `engram-eval`'s
//! generator (shaped to a real graph's profile: body length, code refs, edge
//! mix) and this module adds what the benchmark is about: re-decided
//! subjects, a polluted share of stale siblings, planted contradictions in
//! three tiers, deliberate deletions, structure-only questions and capture
//! times to scope by.
//!
//! The script's operation order is load-bearing and stated once here:
//!
//! 1. import the world (notes, links, stale siblings, supersession chains);
//! 2. settle — a session boundary;
//! 3. probe retrieval, abstention, currency, rationale, temporal on the
//!    UNTOUCHED world, so no family's plantings crowd another's questions;
//! 4. plant the contradiction cases as assistant-style writes;
//! 5. settle, then ask for the suspect queue (contradiction + drift);
//! 6. release and purge the deletion targets, probe them, write them back;
//! 7. (`--authority`) plant near-identical twins of the authority targets,
//!    endorse them on different rungs, settle, and ask which comes first.
//!
//! Target subjects are disjoint by construction: contradiction targets are
//! the tested facts at index ≡ 0 (mod 3), deletion targets ≡ 1 (mod 3) and
//! never polluted, temporal targets ≡ 2 (mod 3). Authority targets reuse
//! the ≡ 2 (mod 3) facts: their temporal probes ran on the untouched world
//! in step 3, so twins planted in step 7 crowd nothing that was graded.

use std::collections::{HashMap, HashSet};

use crate::corpus::profile::Profile;
use crate::corpus::rng::Rng;
use crate::corpus::{
    Corpus, CorpusShape, Fact, KINDS, Kind, Phrasing, corpus_chained_shaped, natural_null,
};

use crate::protocol::{Authority, Record, Window, WriteMode};
use crate::script::{Expect, Family, Op, PollutionShape, Probe, Script, WorldSpec};

pub const DAY: i64 = 86_400;
/// The world's "now". Every capture time sits before it so the engine's
/// future-clamp never fires and reruns share one timeline.
pub const BASE_DAY: &str = "2026-09-01";

#[derive(Debug, Clone)]
pub struct WorldConfig {
    /// Tested facts — every one is questioned (ladder convention: no
    /// untested distractors; the other facts ARE the noise).
    pub size: usize,
    pub seed: u64,
    /// Results a recall may return.
    pub k: usize,
    pub chain_len: usize,
    /// Share of tested subjects imported with a stale, never-superseded
    /// sibling — the drift the benchmark is named for.
    pub pollution: f64,
    /// How that sibling is shaped (`--pollution-shape`).
    pub shape: PollutionShape,
    pub spread_days: i64,
    pub window_days: i64,
    /// Generate the authority family (step 7). Off by default so the v1
    /// worlds keep their digests; on, the world grows its twins and probes
    /// after every other family has been asked.
    pub authority: bool,
    /// Benchmark edition: 1 = the frozen v1 worlds; 2 = shared-vocabulary
    /// subjects, the crossed phrasing, natural-null controls, the reworded
    /// contradiction shapes and answer-bearing probes (`--v2`).
    pub edition: u8,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            size: 100,
            seed: 1,
            k: 10,
            chain_len: 3,
            pollution: 0.10,
            shape: PollutionShape::Stale,
            spread_days: 60,
            window_days: 5,
            authority: false,
            edition: 1,
        }
    }
}

/// `YYYY-MM-DD` → unix seconds at 00:00 UTC (proleptic Gregorian, the
/// days-from-civil algorithm), so the world's clock needs no date crate.
pub fn day_to_unix(day: &str) -> Option<i64> {
    let mut parts = day.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * DAY)
}

fn uniform_mix() -> Vec<(Kind, u32)> {
    KINDS.iter().map(|k| (*k, 1)).collect()
}

fn kind_name(k: Kind) -> String {
    k.type_name().to_string()
}

/// The component is a subject's last two words (`lease broker`), whatever
/// precedes them — a coined name in v1, two shared words in v2.
fn component_of(subject: &str) -> &str {
    let (head, last) = subject.rsplit_once(' ').unwrap_or(("", subject));
    let (_, prev) = head.rsplit_once(' ').unwrap_or(("", head));
    if prev.is_empty() || prev.len() + 1 + last.len() > subject.len() {
        return subject;
    }
    &subject[subject.len() - prev.len() - 1 - last.len()..]
}

/// `(parameter, value, unit)` of a Decision's predicate
/// (`uses a retry budget of 7 attempts`), or `None` for every other kind.
fn decision_parts(f: &Fact) -> Option<(String, u64, String)> {
    let rest = f.predicate.strip_prefix("uses a ")?;
    let (param, value_unit) = rest.split_once(" of ")?;
    let (value, unit) = value_unit.split_once(' ')?;
    Some((param.to_string(), value.parse().ok()?, unit.to_string()))
}

fn altered(value: u64, salt: usize) -> u64 {
    value + 1 + (salt % 40) as u64
}

/// Integer unit conversions the unit shapes can express without fractions.
fn convert(unit: &str) -> Option<(&'static str, u64)> {
    match unit {
        "seconds" => Some(("milliseconds", 1000)),
        "minutes" => Some(("seconds", 60)),
        "hours" => Some(("minutes", 60)),
        "days" => Some(("hours", 24)),
        "megabytes" => Some(("kilobytes", 1024)),
        _ => None,
    }
}

/// A flipped restatement in the generator's own wording — the stale sibling
/// a polluted subject carries. Same template family as the true note, so it
/// reads as an earlier version of the same knowledge.
fn flip_a(f: &Fact, salt: usize) -> String {
    let s = &f.subject;
    match f.kind {
        Kind::Decision => match decision_parts(f) {
            Some((param, v, unit)) => {
                format!("{s} uses a {param} of {} {unit}", altered(v, salt))
            }
            None => format!("{s} is configured differently from {}", f.answer),
        },
        Kind::Caution => format!("{s}: {} — never observed, in any environment", f.predicate),
        Kind::Principle => format!("{s} is permitted to {}", f.answer),
        Kind::Problem => format!("{s} no longer {} (closed)", f.predicate),
        Kind::Insight => format!("{s} {}, and {} is not why", f.predicate, f.answer),
    }
}

/// A flipped restatement in different wording — the tier-1 planted note.
fn flip_b(subject: &str, f: &Fact, salt: usize) -> String {
    match f.kind {
        Kind::Decision => match decision_parts(f) {
            Some((_, v, unit)) => {
                format!("{subject} is configured with {} {unit}", altered(v, salt))
            }
            None => format!(
                "{subject} is configured with something other than {}",
                f.answer
            ),
        },
        Kind::Caution => format!("{subject} never {}, whatever happens", f.predicate),
        Kind::Principle => format!("{subject} may freely {}", f.answer),
        Kind::Problem => format!("{subject} no longer {}", f.predicate),
        Kind::Insight => format!(
            "It is not true that {subject} {} because {}",
            f.predicate, f.answer
        ),
    }
}

/// The flipped claim as a clause about "it", for compound sentences.
fn flip_clause(f: &Fact, salt: usize) -> String {
    match f.kind {
        Kind::Decision => match decision_parts(f) {
            Some((_, v, unit)) => format!("it is configured with {} {unit}", altered(v, salt)),
            None => format!("it is configured with something other than {}", f.answer),
        },
        Kind::Caution => format!("it never {}, whatever happens", f.predicate),
        Kind::Principle => format!("it may freely {}", f.answer),
        Kind::Problem => format!("it no longer {}", f.predicate),
        Kind::Insight => format!("it {} for a reason other than {}", f.predicate, f.answer),
    }
}

/// A flipped claim in a wholly different frame — the v2 `reworded` shape:
/// no token of the original sentence survives but the subject and the
/// slot word, so a token-differencing detector sees a new sentence, not a
/// one-token edit.
fn flip_c(f: &Fact, salt: usize) -> String {
    let s = &f.subject;
    match f.kind {
        Kind::Decision => match decision_parts(f) {
            Some((param, v, unit)) => format!(
                "Operators pinned the {s} to {} {unit} for its {param}, and the dashboard agrees",
                altered(v, salt)
            ),
            None => format!("Operators moved the {s} off {} some time ago", f.answer),
        },
        Kind::Caution => format!(
            "Nobody has ever seen the {s} {}; it has been stable in every environment",
            f.predicate
        ),
        Kind::Principle => format!(
            "Doing {} inside the {s} is fine and routinely done",
            f.answer
        ),
        Kind::Problem => format!(
            "The {s} has stopped showing that it {}; the issue is resolved",
            f.predicate
        ),
        Kind::Insight => format!(
            "The explanation for the {s} {} was traced elsewhere; {} plays no part",
            f.predicate, f.answer
        ),
    }
}

/// An agreeing restatement one token away from the original title — the
/// v2 `synonym` shape, the trap for a detector that flags any small edit.
/// `None` when the title has no token to swap.
fn synonym(f: &Fact) -> Option<String> {
    let t = &f.title;
    let swapped = match f.kind {
        Kind::Decision => t
            .contains(" uses a ")
            .then(|| t.replacen(" uses a ", " keeps a ", 1)),
        Kind::Caution => t
            .contains(" when ")
            .then(|| t.replacen(" when ", " whenever ", 1)),
        Kind::Principle => t
            .contains(" must never ")
            .then(|| t.replacen(" must never ", " should never ", 1)),
        Kind::Problem => Some(format!("Confirmed: {t}")),
        Kind::Insight => t
            .contains(" because ")
            .then(|| t.replacen(" because ", " since ", 1)),
    }?;
    (swapped != *t).then_some(swapped)
}

/// An agreeing restatement — the paraphrase trap that must not be flagged.
fn agree(f: &Fact) -> String {
    let s = &f.subject;
    match f.kind {
        Kind::Decision => format!("The agreed setting for the {s} is {}.", f.answer),
        Kind::Caution => format!("In production, {s} {} without warning.", f.predicate),
        Kind::Principle => format!("It is forbidden for the {s} to {}.", f.answer),
        Kind::Problem => format!("There is an open issue where the {s} {}.", f.predicate),
        Kind::Insight => format!("The reason the {s} {} is that {}.", f.predicate, f.answer),
    }
}

/// The ten contradiction shapes, in planting rotation. Tier and polarity are
/// fixed per shape; applicability depends on the target's kind.
const SHAPES: [(&str, u8, bool); 11] = [
    ("value", 1, true),
    ("negation", 2, true),
    ("paraphrase", 2, false),
    ("unit", 2, true),
    ("coreference", 2, false),
    ("transitive", 3, true),
    ("quantifier", 2, true),
    ("collider", 2, false),
    ("compound", 3, true),
    ("unit_agree", 2, false),
    ("historical", 3, false),
];

/// The v2 rotation: the eleven above plus three shapes aimed at lexical
/// contradiction detectors — a flip that shares no frame with the original
/// (`reworded`), a flip carried by a clause rather than a token (`clause`),
/// and an agreeing restatement exactly one token from the title (`synonym`).
const SHAPES_V2: [(&str, u8, bool); 14] = [
    ("value", 1, true),
    ("reworded", 1, true),
    ("negation", 2, true),
    ("paraphrase", 2, false),
    ("synonym", 2, false),
    ("unit", 2, true),
    ("clause", 2, true),
    ("coreference", 2, false),
    ("transitive", 3, true),
    ("quantifier", 2, true),
    ("collider", 2, false),
    ("compound", 3, true),
    ("unit_agree", 2, false),
    ("historical", 3, false),
];

struct Case {
    shape: &'static str,
    tier: u8,
    positive: bool,
    /// Planted notes, witness first.
    planted: Vec<(String, String)>,
}

/// Build one case of `shape` for `f`, or `None` when the shape does not
/// apply to this kind (or the scaffolding it needs is unavailable).
#[allow(clippy::too_many_arguments)]
fn make_case(
    shape: &'static str,
    tier: u8,
    positive: bool,
    f: &Fact,
    salt: usize,
    other_subject: Option<&str>,
    other_predicate: Option<&str>,
    ghost: Option<&str>,
) -> Option<Case> {
    let s = &f.subject;
    let parts = decision_parts(f);
    let planted: Vec<(String, String)> = match shape {
        "value" => vec![(
            flip_b(s, f, salt),
            "Recorded while re-reading the runbook.".into(),
        )],
        "negation" => vec![(
            format!("It is not the case that the {s} {}.", f.predicate),
            "Checked against the current deployment.".into(),
        )],
        "paraphrase" => vec![(agree(f), "Restated for the onboarding notes.".into())],
        "reworded" => vec![(
            flip_c(f, salt),
            "Written up fresh after the last operations review.".into(),
        )],
        "clause" => vec![(
            format!(
                "Contrary to the runbook, it is wrong that the {s} {}; the earlier note was mistaken",
                f.predicate
            ),
            "Corrected after the incident that the runbook version caused.".into(),
        )],
        "synonym" => vec![(
            synonym(f)?,
            "Restated word for word from the original note.".into(),
        )],
        "unit" => {
            let (param, v, unit) = parts.clone()?;
            let (unit2, factor) = convert(&unit)?;
            vec![(
                format!(
                    "{s} is set to a {param} of {} {unit2}",
                    altered(v, salt) * factor
                ),
                "Converted from the ops dashboard's units.".into(),
            )]
        }
        "unit_agree" => {
            let (param, v, unit) = parts.clone()?;
            let (unit2, factor) = convert(&unit)?;
            vec![(
                format!("{s} keeps its {param} at {} {unit2}", v * factor),
                "Same setting, read off the dashboard in its own units.".into(),
            )]
        }
        "coreference" => {
            let other = other_subject?;
            vec![(
                flip_b(other, f, salt),
                "Noted while comparing the two.".into(),
            )]
        }
        // Same subject, a different claim: the shape the dogfood graph
        // raised fifty-six of on the first live sweep — two facts about one
        // thing are not a contradiction, however unlike they read.
        "collider" => {
            let pred = other_predicate?;
            vec![(
                format!("{s} {pred}"),
                "Observed on the same component, unrelated to the setting above.".into(),
            )]
        }
        "transitive" => {
            let ghost = ghost?;
            vec![
                (
                    flip_b(ghost, f, salt),
                    "Captured from the sibling's runbook.".into(),
                ),
                (
                    format!("{s} mirrors every setting and rule of the {ghost}"),
                    "The two are provisioned from one template; whatever holds for one holds for the other.".into(),
                ),
            ]
        }
        "quantifier" => {
            let text = match f.kind {
                Kind::Decision => {
                    let (param, v, unit) = parts.clone()?;
                    format!(
                        "Every deployment of the {s} runs with a {param} of {} {unit}, without exception",
                        altered(v, salt)
                    )
                }
                Kind::Principle => {
                    format!("{s} is allowed to {} whenever it is convenient", f.answer)
                }
                Kind::Caution | Kind::Problem => format!(
                    "In every environment tried so far the {s} runs clean: {} has not happened once",
                    f.predicate
                ),
                Kind::Insight => format!(
                    "{s} {} for reasons that have nothing to do with {}",
                    f.predicate, f.answer
                ),
            };
            vec![(text, "Surveyed across every environment.".into())]
        }
        "compound" => vec![(
            format!(
                "{s} {}; separately, operators report that {}",
                f.predicate,
                flip_clause(f, salt)
            ),
            "Two observations from one incident review.".into(),
        )],
        "historical" => vec![(
            format!(
                "Until the 3.1 rollout, {}; the rollout changed that, and the current note stands",
                flip_b(s, f, salt)
            ),
            "History, kept so nobody re-litigates the rollout.".into(),
        )],
        _ => return None,
    };
    Some(Case {
        shape,
        tier,
        positive,
        planted,
    })
}

/// A near-identical restatement of `f` with a different value — twin `j`
/// (1-based) of the authority family. Every kind has four distinct titles
/// so a five-note ladder never repeats one; the body is the truth's own.
fn twin_title(f: &Fact, j: usize, salt: usize) -> String {
    let s = &f.subject;
    match f.kind {
        Kind::Decision => match decision_parts(f) {
            Some((param, v, unit)) => {
                let v2 = altered(v, salt + 7 * j);
                match j {
                    1 => format!("{s} uses a {param} of {v2} {unit}"),
                    2 => format!("{s} is configured with {v2} {unit}"),
                    3 => format!("{s} runs with a {param} of {v2} {unit}"),
                    _ => format!("{s}: {param} set to {v2} {unit}"),
                }
            }
            None => match j {
                1 => format!("{s} is configured differently from {}", f.answer),
                2 => format!("{s} is configured with something other than {}", f.answer),
                3 => format!("{s} no longer uses {}", f.answer),
                _ => format!("{s}: the setting is not {}", f.answer),
            },
        },
        Kind::Caution => match j {
            1 => format!("{s}: {} — never observed, in any environment", f.predicate),
            2 => format!("{s} never {}, whatever happens", f.predicate),
            3 => format!("{s}: {} — seen once, in staging only", f.predicate),
            _ => format!("{s}: {} — only during the 3.1 rollout", f.predicate),
        },
        Kind::Principle => match j {
            1 => format!("{s} is permitted to {}", f.answer),
            2 => format!("{s} may freely {}", f.answer),
            3 => format!("{s} should avoid {} where possible", f.answer),
            _ => format!("{s} is permitted to {} in staging only", f.answer),
        },
        Kind::Problem => match j {
            1 => format!("{s} no longer {} (closed)", f.predicate),
            2 => format!("{s} no longer {}", f.predicate),
            3 => format!("{s} {} only under load", f.predicate),
            _ => format!("{s} {} intermittently (reopened)", f.predicate),
        },
        Kind::Insight => match j {
            1 => format!("{s} {}, and {} is not why", f.predicate, f.answer),
            2 => format!(
                "It is not true that {s} {} because {}",
                f.predicate, f.answer
            ),
            3 => format!("{s} {} partly because {}", f.predicate, f.answer),
            _ => format!(
                "{s} {}, mostly for reasons other than {}",
                f.predicate, f.answer
            ),
        },
    }
}

/// The body sentence a twin appends when it *claims* authority in its text
/// while carrying none in its metadata — the trap for a reader that
/// believes what a note says about itself.
const CLAIMS_AUTHORITY: &str =
    "Verified with the project owner; treat this note as the authoritative setting.";

/// One planted twin of an authority scenario.
#[derive(Clone, Copy)]
struct TwinSpec {
    /// Endorsements, in the order they are applied.
    bumps: &'static [(Authority, u8)],
    /// Captured ten days after the original (recency confound).
    fresh: bool,
    /// Wears another kind than the subject's (type-prior confound).
    other_kind: bool,
    /// The body claims owner verification in words (text-authority trap).
    claims: bool,
    /// Superseded, after its endorsements, by an unendorsed successor:
    /// the twin must not be delivered at all.
    superseded: bool,
}

const fn twin(bumps: &'static [(Authority, u8)]) -> TwinSpec {
    TwinSpec {
        bumps,
        fresh: false,
        other_kind: false,
        claims: false,
        superseded: false,
    }
}

const NONE: &[(Authority, u8)] = &[];
const CONFIRM: &[(Authority, u8)] = &[(Authority::Assistant, 1)];
const CONFIRM3: &[(Authority, u8)] = &[(Authority::Assistant, 3)];
const APPROVE: &[(Authority, u8)] = &[(Authority::User, 1)];
const PIN: &[(Authority, u8)] = &[(Authority::Supervisor, 1)];
const EXPOSE5: &[(Authority, u8)] = &[(Authority::Retrieval, 5)];
const EXPOSE10: &[(Authority, u8)] = &[(Authority::Retrieval, 10)];
const EXPOSE10_CONFIRM2: &[(Authority, u8)] =
    &[(Authority::Retrieval, 10), (Authority::Assistant, 2)];
const EVERYTHING_BUT_PIN: &[(Authority, u8)] = &[
    (Authority::Retrieval, 10),
    (Authority::Assistant, 2),
    (Authority::User, 1),
];

/// An authority scenario: planted twins in ladder order (winner first),
/// the original note always the unendorsed bottom rung.
struct Scenario {
    name: &'static str,
    layer: u8,
    twins: &'static [TwinSpec],
    /// Grade the whole ladder's order, not only the winner.
    order: bool,
    /// The bumps are applied last-twin-first, so the winner's endorsement
    /// is the most recent — the "latest wins" tie-break scenarios.
    reverse_bumps: bool,
}

/// The twenty-one scenarios, in planting rotation. Layers: 1 = the assistant
/// alone (one rung), 2 = owner governance over the assistant (two rungs,
/// across kinds), 3 = retrieval use counted beside both, 4 = a supervisor's
/// pin above everything. Every scenario is a stated expectation of the
/// ladder retrieval < assistant < user < supervisor; equal rungs are
/// broken by the more recent endorsement; count never beats rung; and no
/// confound — a fresher stamp, a weightier kind, a body that *says* it is
/// verified — outranks a rung.
const SCENARIOS: [Scenario; 21] = [
    // ---- layer 1: autonomous -----------------------------------------
    Scenario {
        name: "confirm_vs_none",
        layer: 1,
        twins: &[twin(CONFIRM)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "confirm_vs_fresh",
        layer: 1,
        twins: &[
            twin(CONFIRM),
            TwinSpec {
                fresh: true,
                ..twin(NONE)
            },
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "confirm_vs_kind",
        layer: 1,
        twins: &[
            TwinSpec {
                other_kind: true,
                ..twin(CONFIRM)
            },
            twin(NONE),
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "latest_confirm",
        layer: 1,
        twins: &[twin(CONFIRM), twin(CONFIRM)],
        order: false,
        reverse_bumps: true,
    },
    // ---- layer 2: governed -------------------------------------------
    Scenario {
        name: "approve_vs_confirm",
        layer: 2,
        twins: &[twin(APPROVE), twin(CONFIRM)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "approve_vs_confirms",
        layer: 2,
        twins: &[twin(APPROVE), twin(CONFIRM3)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "approve_vs_kind",
        layer: 2,
        twins: &[
            TwinSpec {
                other_kind: true,
                ..twin(APPROVE)
            },
            twin(CONFIRM),
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "approve_vs_claims",
        layer: 2,
        twins: &[
            twin(APPROVE),
            TwinSpec {
                claims: true,
                ..twin(CONFIRM)
            },
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "approve_vs_fresh_confirm",
        layer: 2,
        twins: &[
            twin(APPROVE),
            TwinSpec {
                fresh: true,
                ..twin(CONFIRM)
            },
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "ladder2",
        layer: 2,
        twins: &[twin(APPROVE), twin(CONFIRM)],
        order: true,
        reverse_bumps: false,
    },
    Scenario {
        name: "latest_approve",
        layer: 2,
        twins: &[twin(APPROVE), twin(APPROVE)],
        order: false,
        reverse_bumps: true,
    },
    // ---- layer 3: three hands ----------------------------------------
    Scenario {
        name: "exposure_vs_none",
        layer: 3,
        twins: &[twin(EXPOSE10)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "confirm_vs_exposure",
        layer: 3,
        twins: &[twin(CONFIRM), twin(EXPOSE10)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "approve_vs_exposure_confirm",
        layer: 3,
        twins: &[twin(APPROVE), twin(EXPOSE10_CONFIRM2)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "exposure_vs_fresh",
        layer: 3,
        twins: &[
            twin(EXPOSE10),
            TwinSpec {
                fresh: true,
                ..twin(NONE)
            },
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "ladder3",
        layer: 3,
        twins: &[twin(APPROVE), twin(CONFIRM), twin(EXPOSE5)],
        order: true,
        reverse_bumps: false,
    },
    // ---- layer 4: supervised -----------------------------------------
    Scenario {
        name: "pin_vs_approve",
        layer: 4,
        twins: &[twin(PIN), twin(APPROVE)],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "pin_vs_everything",
        layer: 4,
        twins: &[
            twin(PIN),
            TwinSpec {
                fresh: true,
                claims: true,
                ..twin(EVERYTHING_BUT_PIN)
            },
        ],
        order: false,
        reverse_bumps: false,
    },
    Scenario {
        name: "latest_pin",
        layer: 4,
        twins: &[twin(PIN), twin(PIN)],
        order: false,
        reverse_bumps: true,
    },
    Scenario {
        name: "pin_then_supersede",
        layer: 4,
        twins: &[
            twin(CONFIRM),
            TwinSpec {
                superseded: true,
                ..twin(PIN)
            },
        ],
        order: false,
        reverse_bumps: false,
    },
    // ---- layer 4, the whole ladder -----------------------------------
    Scenario {
        name: "ladder4",
        layer: 4,
        twins: &[twin(PIN), twin(APPROVE), twin(CONFIRM), twin(EXPOSE5)],
        order: true,
        reverse_bumps: false,
    },
];

fn record(f: &Fact, created_at: i64) -> Record {
    Record {
        key: f.key.clone(),
        kind: kind_name(f.kind),
        title: f.title.clone(),
        body: f.body.clone(),
        code_refs: f.code_refs.clone(),
        created_at: Some(created_at),
        open: matches!(f.kind, Kind::Problem),
    }
}

fn question(f: &Fact, phrasing: Phrasing) -> Option<String> {
    f.questions
        .iter()
        .find(|q| q.phrasing == phrasing)
        .map(|q| q.text.clone())
}

fn phrasing_name(p: Phrasing) -> &'static str {
    match p {
        Phrasing::Lexical => "lexical",
        Phrasing::Paraphrase => "paraphrase",
        Phrasing::Oblique => "oblique",
        Phrasing::Crossed => "crossed",
    }
}

fn cap(size: usize, div: usize, floor: usize) -> usize {
    (size / div).max(floor)
}

/// Build the script for one world.
pub fn build(cfg: &WorldConfig) -> Script {
    let base = day_to_unix(BASE_DAY).expect("BASE_DAY parses");
    let v2 = cfg.edition >= 2;
    let n_chains = cap(cfg.size, 20, 4);
    let c: Corpus = corpus_chained_shaped(
        cfg.size,
        0,
        cfg.seed,
        &Profile::default(),
        &uniform_mix(),
        n_chains,
        cfg.chain_len,
        CorpusShape {
            shared_vocab: v2,
            crossed: v2,
            ..CorpusShape::default()
        },
    );
    // v2 probes carry the answer substring; v1 probes stay key-graded.
    let answer_of = |f: &Fact| if v2 { f.answer.clone() } else { String::new() };

    let chain_keys: HashSet<&str> = c
        .chains
        .iter()
        .flat_map(|ch| ch.keys.iter().map(String::as_str))
        .collect();
    let facts: Vec<&Fact> = c
        .facts
        .iter()
        .filter(|f| !chain_keys.contains(f.key.as_str()))
        .collect();
    let by_key: HashMap<&str, &Fact> = c.facts.iter().map(|f| (f.key.as_str(), f)).collect();

    // Capture time per base fact: scattered over the spread with an
    // avalanched index so time never correlates with kind or key order.
    let ts_of = |i: usize| -> i64 {
        let mix = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 20;
        let day = (mix % cfg.spread_days.max(1) as u64) as i64;
        let hour = (mix / 7 % 20) as i64 + 1;
        base - day * DAY - hour * 3600
    };

    // Pollution: a seeded share of tested subjects gain a stale sibling
    // nobody superseded — older by default, shaped by `cfg.shape`.
    let mut rng = Rng::new(cfg.seed ^ 0x5eed_d21f_7a11);
    let polluted: HashSet<usize> = facts
        .iter()
        .enumerate()
        .filter(|(_, f)| f.tested)
        .filter(|_| (rng.below(10_000) as f64) < cfg.pollution * 10_000.0)
        .map(|(i, _)| i)
        .collect();

    let mut ops: Vec<Op> = Vec::new();
    let mut probes: Vec<Probe> = Vec::new();
    let mut recall_n = 0usize;
    let mut write_n = 0usize;
    let mut lineage_n = 0usize;

    // ---- 1. import --------------------------------------------------------
    let mut notes = 0usize;
    for (i, f) in facts.iter().enumerate() {
        let ts = ts_of(i);
        if polluted.contains(&i) {
            let offset = (20 + (i % 20) as i64) * DAY;
            let hint = format!(
                "Recorded before the {} was re-tuned; kept for reference.",
                component_of(&f.subject)
            );
            let (stale_ts, body) = match cfg.shape {
                PollutionShape::Stale => (ts - offset, hint),
                PollutionShape::Twin => (ts - offset, f.body.clone()),
                // A migration stamps import time as capture time: the
                // sibling lands after the truth, but never after "now".
                PollutionShape::Late => ((ts + offset).min(base - 1800), hint),
            };
            ops.push(Op::Inscribe {
                id: None,
                record: Record {
                    key: format!("s-{}", f.key),
                    kind: kind_name(f.kind),
                    title: flip_a(f, i),
                    body,
                    code_refs: f.code_refs.clone(),
                    created_at: Some(stale_ts),
                    open: matches!(f.kind, Kind::Problem),
                },
                mode: WriteMode::Import,
            });
            notes += 1;
        }
        ops.push(Op::Inscribe {
            id: None,
            record: record(f, ts),
            mode: WriteMode::Import,
        });
        notes += 1;
    }
    let mut edges = 0usize;
    for e in &c.edges {
        if chain_keys.contains(e.from.as_str()) || chain_keys.contains(e.to.as_str()) {
            continue;
        }
        ops.push(Op::Link {
            from: e.from.clone(),
            to: e.to.clone(),
            verb: e.verb.to_string(),
        });
        edges += 1;
    }
    // Supersession chains: the first generation is imported, every later one
    // re-decides the one before it, a month apart.
    for ch in &c.chains {
        let len = ch.keys.len() as i64;
        for (g, key) in ch.keys.iter().enumerate() {
            let f = by_key[key.as_str()];
            let ts = base - (len - g as i64) * 30 * DAY;
            if g == 0 {
                ops.push(Op::Inscribe {
                    id: None,
                    record: record(f, ts),
                    mode: WriteMode::Import,
                });
            } else {
                ops.push(Op::Supersede {
                    old: ch.keys[g - 1].clone(),
                    new: record(f, ts),
                });
            }
            notes += 1;
        }
    }

    // ---- 2. settle --------------------------------------------------------
    ops.push(Op::Settle);

    // ---- 3. probes on the untouched world ---------------------------------
    let mut recall = |ops: &mut Vec<Op>,
                      probes: &mut Vec<Probe>,
                      family: Family,
                      query: String,
                      window: Option<Window>,
                      expect: Expect| {
        recall_n += 1;
        let id = format!("R{recall_n}");
        ops.push(Op::Recall {
            id: id.clone(),
            query,
            k: cfg.k,
            window,
        });
        probes.push(Probe { id, family, expect });
    };

    // Retrieval: every question of every tested fact.
    for (i, f) in facts.iter().enumerate().filter(|(_, f)| f.tested) {
        let stale = polluted.contains(&i).then(|| format!("s-{}", f.key));
        for q in &f.questions {
            recall(
                &mut ops,
                &mut probes,
                Family::Retrieval,
                q.text.clone(),
                None,
                Expect::Gold {
                    gold: f.key.clone(),
                    phrasing: phrasing_name(q.phrasing).to_string(),
                    stale: stale.clone(),
                    answer: answer_of(f),
                },
            );
        }
    }

    // v2: the path-shaped read. Every file the world's code refs name is
    // asked once, as a path — what a caller about to edit it should see.
    // Gold is every live, truthful note bound to the file: the non-chain
    // facts (tested or not — a distractor bound to a file is still what an
    // editor should know) and the chain heads. A stale sibling shares its
    // fact's refs but is neither gold nor penalised.
    let mut path_n = 0usize;
    if v2 {
        let live: Vec<&Fact> = facts
            .iter()
            .copied()
            .chain(c.chains.iter().map(|ch| by_key[ch.head()]))
            .collect();
        let mut paths: Vec<&str> = Vec::new();
        let mut bound: HashMap<&str, Vec<&Fact>> = HashMap::new();
        for f in &live {
            for p in &f.code_refs {
                if !bound.contains_key(p.as_str()) {
                    paths.push(p);
                }
                let golds = bound.entry(p.as_str()).or_default();
                // A note may name the same file twice in its refs.
                if !golds.iter().any(|g| g.key == f.key) {
                    golds.push(f);
                }
            }
        }
        for p in paths {
            let golds = &bound[p];
            path_n += 1;
            let id = format!("P{path_n}");
            ops.push(Op::RecallPath {
                id: id.clone(),
                path: p.to_string(),
                k: cfg.k,
            });
            probes.push(Probe {
                id,
                family: Family::Retrieval,
                expect: Expect::Bound {
                    path: p.to_string(),
                    gold: golds.iter().map(|f| f.key.clone()).collect(),
                    answers: golds.iter().map(|f| answer_of(f)).collect(),
                },
            });
        }
    }
    let _ = path_n;

    // Contradiction plan first: transitive cases borrow phantom subjects
    // from the END of the control list, and those controls are dropped.
    let targets: Vec<(usize, &Fact)> = facts
        .iter()
        .enumerate()
        .filter(|(i, f)| f.tested && i % 3 == 0)
        .map(|(i, f)| (i, *f))
        .take(cap(cfg.size, 3, 6))
        .collect();
    let mut cases: Vec<(&Fact, Case)> = Vec::new();
    let mut ghosts_used = 0usize;
    for (n, (i, f)) in targets.iter().enumerate() {
        let comp = component_of(&f.subject);
        let other_subject = facts
            .iter()
            .find(|o| o.key != f.key && o.kind != f.kind && component_of(&o.subject) == comp)
            .map(|o| o.subject.as_str());
        // A claim from another kind and another component, re-attached to
        // this subject: unrelated by construction (no slot triple is shared).
        let other_predicate = facts
            .iter()
            .find(|o| o.key != f.key && o.kind != f.kind && component_of(&o.subject) != comp)
            .map(|o| o.predicate.as_str());
        let ghost_idx = c.phantom_subjects.len().checked_sub(ghosts_used + 1);
        let ghost = ghost_idx.map(|g| c.phantom_subjects[g].as_str());
        let mut made = None;
        let shapes: &[(&'static str, u8, bool)] = if v2 { &SHAPES_V2 } else { &SHAPES };
        for step in 0..shapes.len() {
            let (shape, tier, positive) = shapes[(n + step) % shapes.len()];
            let needs_ghost = shape == "transitive";
            if needs_ghost && ghost.is_none() {
                continue;
            }
            if let Some(case) = make_case(
                shape,
                tier,
                positive,
                f,
                *i + 17,
                other_subject,
                other_predicate,
                ghost,
            ) {
                if needs_ghost {
                    ghosts_used += 1;
                }
                made = Some(case);
                break;
            }
        }
        if let Some(case) = made {
            cases.push((f, case));
        }
    }

    // Abstention: every control whose phantom subject was not lent out.
    let keep = c.unanswerable.len().saturating_sub(ghosts_used);
    for q in c.unanswerable.iter().take(keep) {
        recall(
            &mut ops,
            &mut probes,
            Family::Abstention,
            q.text.clone(),
            None,
            Expect::Control { natural: false },
        );
    }
    // v2: natural nulls — a real subject, a kind of question it has no
    // note for. One per four tested facts, like the phantoms.
    if v2 {
        for (i, f) in facts
            .iter()
            .enumerate()
            .filter(|(i, f)| f.tested && i % 4 == 3)
            .take(cap(cfg.size, 4, 4))
        {
            recall(
                &mut ops,
                &mut probes,
                Family::Abstention,
                natural_null(f, i + 5),
                None,
                Expect::Control { natural: true },
            );
        }
    }

    // Currency: the current state of every re-decided subject, plus a
    // lineage walk per chain.
    for ch in &c.chains {
        let retired: Vec<String> = ch.retired().to_vec();
        for q in &ch.questions {
            recall(
                &mut ops,
                &mut probes,
                Family::Currency,
                q.text.clone(),
                None,
                Expect::Current {
                    head: ch.head().to_string(),
                    retired: retired.clone(),
                    answer: answer_of(by_key[ch.head()]),
                },
            );
        }
        lineage_n += 1;
        let id = format!("L{lineage_n}");
        ops.push(Op::Lineage {
            id: id.clone(),
            key: ch.head().to_string(),
        });
        probes.push(Probe {
            id,
            family: Family::Currency,
            expect: Expect::Lineage {
                head: ch.head().to_string(),
                retired,
            },
        });
    }

    // Rationale: questions answerable only across an edge.
    let tested_base: HashSet<&str> = facts
        .iter()
        .filter(|f| f.tested)
        .map(|f| f.key.as_str())
        .collect();
    let mut why = 0usize;
    let mut answers = 0usize;
    let rationale_cap = cap(cfg.size, 4, 4);
    for e in &c.edges {
        if !tested_base.contains(e.from.as_str()) || !tested_base.contains(e.to.as_str()) {
            continue;
        }
        let (from, to) = (by_key[e.from.as_str()], by_key[e.to.as_str()]);
        match e.verb {
            "because" if why < rationale_cap => {
                why += 1;
                recall(
                    &mut ops,
                    &mut probes,
                    Family::Rationale,
                    format!("why does the {} {}?", from.subject, from.predicate),
                    None,
                    Expect::Linked {
                        gold: to.key.clone(),
                        anchor: from.key.clone(),
                        verb: "because".into(),
                        answer: answer_of(to),
                    },
                );
            }
            "answers" if answers < rationale_cap => {
                answers += 1;
                recall(
                    &mut ops,
                    &mut probes,
                    Family::Rationale,
                    format!(
                        "what answers the open issue where the {} {}?",
                        to.subject, to.predicate
                    ),
                    None,
                    Expect::Linked {
                        gold: from.key.clone(),
                        anchor: to.key.clone(),
                        verb: "answers".into(),
                        answer: answer_of(from),
                    },
                );
            }
            _ => {}
        }
    }

    // Temporal: the paraphrase question, scoped to the days around capture.
    for (i, f) in facts
        .iter()
        .enumerate()
        .filter(|(i, f)| f.tested && i % 3 == 2)
        .take(cap(cfg.size, 4, 4))
    {
        let Some(q) = question(f, Phrasing::Paraphrase) else {
            continue;
        };
        let ts = ts_of(i);
        let window = Window {
            after: Some(ts - cfg.window_days * DAY),
            before: Some(ts + cfg.window_days * DAY),
        };
        recall(
            &mut ops,
            &mut probes,
            Family::Temporal,
            q,
            Some(window),
            Expect::Windowed {
                gold: f.key.clone(),
                window,
                answer: answer_of(f),
            },
        );
    }

    // ---- 4. plant the contradiction cases ---------------------------------
    for (n, (f, case)) in cases.iter().enumerate() {
        let mut keys = Vec::new();
        for (j, (title, body)) in case.planted.iter().enumerate() {
            write_n += 1;
            let key = format!("c{n}{}", (b'a' + j as u8) as char);
            keys.push(key.clone());
            ops.push(Op::Inscribe {
                id: Some(format!("W{write_n}")),
                record: Record {
                    key,
                    kind: kind_name(f.kind),
                    title: title.clone(),
                    body: body.clone(),
                    code_refs: vec![],
                    created_at: Some(base - (n as i64 + 1) * 3600),
                    open: false,
                },
                mode: WriteMode::Write,
            });
        }
        probes.push(Probe {
            id: format!("C{n}"),
            family: Family::Contradiction,
            expect: Expect::Case {
                gold: f.key.clone(),
                witness: keys[0].clone(),
                planted: keys,
                tier: case.tier,
                shape: case.shape.to_string(),
                positive: case.positive,
            },
        });
    }

    // ---- 5. settle, then the queue --------------------------------------
    ops.push(Op::Settle);
    ops.push(Op::Suspects { id: "S".into() });
    for i in polluted
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
    {
        let f = facts[i];
        probes.push(Probe {
            id: format!("D{i}"),
            family: Family::Drift,
            expect: Expect::Drifted {
                gold: f.key.clone(),
                stale: format!("s-{}", f.key),
            },
        });
    }

    // ---- 6. deletions -----------------------------------------------------
    let victims: Vec<(usize, &Fact)> = facts
        .iter()
        .enumerate()
        .filter(|(i, f)| f.tested && i % 3 == 1 && !polluted.contains(i))
        .map(|(i, f)| (i, *f))
        .take(cap(cfg.size, 6, 4))
        .collect();
    for (n, (i, f)) in victims.iter().enumerate() {
        let released = n % 2 == 0;
        if released {
            ops.push(Op::Release {
                key: f.key.clone(),
                reason: format!(
                    "no longer applies after the {} rework",
                    component_of(&f.subject)
                ),
            });
        } else {
            ops.push(Op::Purge { key: f.key.clone() });
        }
        let q = question(f, Phrasing::Lexical).unwrap_or_else(|| f.title.clone());
        recall(
            &mut ops,
            &mut probes,
            Family::Deletion,
            q,
            None,
            Expect::Absent {
                victim: f.key.clone(),
                released,
                answer: f.answer.clone(),
            },
        );
        write_n += 1;
        let id = format!("W{write_n}");
        let mut again = record(f, ts_of(*i));
        again.key = format!("z-{}", f.key);
        again.created_at = Some(base - 60);
        ops.push(Op::Inscribe {
            id: Some(id.clone()),
            record: again,
            mode: WriteMode::Write,
        });
        probes.push(Probe {
            id,
            family: Family::Deletion,
            expect: Expect::Resurrect {
                victim: f.key.clone(),
                released,
            },
        });
    }

    // ---- 7. authority -----------------------------------------------------
    if cfg.authority {
        let targets: Vec<(usize, &Fact)> = facts
            .iter()
            .enumerate()
            .filter(|(i, f)| f.tested && i % 3 == 2)
            .map(|(i, f)| (i, *f))
            .take(cap(cfg.size, 3, SCENARIOS.len()))
            .collect();
        let mut pending: Vec<(String, Family, String, Expect)> = Vec::new();
        for (n, (i, f)) in targets.iter().enumerate() {
            let sc = &SCENARIOS[n % SCENARIOS.len()];
            let ts = ts_of(*i);
            let other_kind = match f.kind {
                Kind::Insight => "Problem",
                _ => "Insight",
            };
            let mut keys: Vec<String> = Vec::new();
            let mut retired: Vec<String> = Vec::new();
            let mut needs: std::collections::BTreeSet<&'static str> =
                std::collections::BTreeSet::new();
            // Plant every twin first, in ladder order.
            for (j, tw) in sc.twins.iter().enumerate() {
                let key = format!("a{n}{}", (b'a' + j as u8) as char);
                let mut body = f.body.clone();
                if tw.claims {
                    body.push_str("\n\n");
                    body.push_str(CLAIMS_AUTHORITY);
                }
                ops.push(Op::Inscribe {
                    id: None,
                    record: Record {
                        key: key.clone(),
                        kind: if tw.other_kind {
                            other_kind.to_string()
                        } else {
                            kind_name(f.kind)
                        },
                        title: twin_title(f, j + 1, *i + 3),
                        body,
                        code_refs: f.code_refs.clone(),
                        created_at: Some(if tw.fresh {
                            (ts + 10 * DAY).min(base - 1800)
                        } else {
                            ts
                        }),
                        open: false,
                    },
                    mode: WriteMode::Import,
                });
                notes += 1;
                keys.push(key);
            }
            // Then the endorsements — winner first unless the scenario is
            // about which endorsement came last.
            let mut order: Vec<usize> = (0..sc.twins.len()).collect();
            if sc.reverse_bumps {
                order.reverse();
            }
            for j in order {
                for (by, count) in sc.twins[j].bumps {
                    for _ in 0..*count {
                        ops.push(Op::Endorse {
                            key: keys[j].clone(),
                            by: *by,
                        });
                    }
                }
            }
            // A superseded twin gets its unendorsed successor after its
            // endorsements: the pin must not keep it delivered.
            for (j, tw) in sc.twins.iter().enumerate() {
                if tw.superseded {
                    let successor = format!("a{n}x");
                    ops.push(Op::Supersede {
                        old: keys[j].clone(),
                        new: Record {
                            key: successor.clone(),
                            kind: kind_name(f.kind),
                            title: twin_title(f, 4, *i + 11),
                            body: f.body.clone(),
                            code_refs: f.code_refs.clone(),
                            created_at: Some((ts + 12 * DAY).min(base - 1800)),
                            open: false,
                        },
                    });
                    notes += 1;
                    retired.push(keys[j].clone());
                }
            }
            // What the winning signal needs: every rung the winner was
            // endorsed on; for a graded ladder, every rung in it.
            let winner = &sc.twins[0];
            for (by, _) in winner.bumps {
                needs.insert(by.capability());
            }
            if sc.order {
                for tw in sc.twins {
                    for (by, _) in tw.bumps {
                        needs.insert(by.capability());
                    }
                }
            }
            let mut losers: Vec<String> = keys[1..]
                .iter()
                .filter(|k| !retired.contains(k))
                .cloned()
                .collect();
            losers.push(f.key.clone());
            let q = question(f, Phrasing::Paraphrase).unwrap_or_else(|| f.title.clone());
            pending.push((
                keys[0].clone(),
                Family::Authority,
                q,
                Expect::Ranked {
                    winner: keys[0].clone(),
                    losers,
                    retired,
                    order: sc.order,
                    layer: sc.layer,
                    scenario: sc.name.to_string(),
                    needs: needs.into_iter().map(String::from).collect(),
                },
            ));
        }
        // One session boundary after every twin and endorsement, then the
        // questions.
        ops.push(Op::Settle);
        for (_, family, q, expect) in pending {
            recall(&mut ops, &mut probes, family, q, None, expect);
        }
    }

    Script {
        digest: String::new(),
        spec: WorldSpec {
            size: cfg.size,
            seed: cfg.seed,
            k: cfg.k,
            chains: c.chains.len(),
            chain_len: cfg.chain_len,
            pollution: cfg.pollution,
            pollution_shape: cfg.shape,
            spread_days: cfg.spread_days,
            window_days: cfg.window_days,
            base_ts: base,
            authority: cfg.authority,
            edition: cfg.edition,
            notes,
            edges,
        },
        ops,
        probes,
    }
    .seal()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> WorldConfig {
        WorldConfig {
            size: 30,
            seed: 7,
            ..WorldConfig::default()
        }
    }

    #[test]
    fn the_script_is_deterministic() {
        let a = build(&small());
        let b = build(&small());
        assert_eq!(a.digest(), b.digest());
        assert_eq!(a, b);
    }

    #[test]
    fn every_family_has_probes() {
        let s = build(&WorldConfig {
            authority: true,
            ..small()
        });
        for fam in Family::ALL {
            assert!(
                s.probes.iter().any(|p| p.family == fam),
                "no probes for {fam:?}"
            );
        }
    }

    #[test]
    fn every_probe_has_exactly_one_op_or_reply_source() {
        let s = build(&small());
        let ids: Vec<&str> = s
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Inscribe { id: Some(id), .. } => Some(id.as_str()),
                Op::Recall { id, .. }
                | Op::RecallPath { id, .. }
                | Op::Suspects { id }
                | Op::Lineage { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        for p in &s.probes {
            let sourced = match &p.expect {
                Expect::Case { .. } | Expect::Drifted { .. } => ids.contains(&"S"),
                _ => ids.contains(&p.id.as_str()),
            };
            assert!(sourced, "probe {} has no op to answer it", p.id);
        }
    }

    /// Every polluted subject's sibling under each shape: `twin` wears the
    /// truth's body and sits before it, `late` keeps the hint and sits
    /// after it, and the default world's digest does not move because the
    /// knob exists.
    #[test]
    fn pollution_shapes_move_only_what_they_claim() {
        let default = build(&small());
        let shaped = |shape| build(&WorldConfig { shape, ..small() });
        assert_eq!(shaped(PollutionShape::Stale).digest(), default.digest());
        assert!(
            !serde_json::to_string(&default.spec)
                .unwrap()
                .contains("pollution_shape")
        );
        for (shape, later, wears_body) in [
            (PollutionShape::Stale, false, false),
            (PollutionShape::Twin, false, true),
            (PollutionShape::Late, true, false),
        ] {
            let s = shaped(shape);
            assert_eq!(
                s.digest() == default.digest(),
                shape == PollutionShape::Stale,
                "{shape:?}: digest"
            );
            let records: HashMap<&str, &Record> = s
                .ops
                .iter()
                .filter_map(|op| match op {
                    Op::Inscribe { record, .. } => Some((record.key.as_str(), record)),
                    _ => None,
                })
                .collect();
            let mut seen = 0;
            for (key, stale) in records.iter().filter(|(k, _)| k.starts_with("s-")) {
                let gold = records[&key[2..]];
                let (st, gt) = (stale.created_at.unwrap(), gold.created_at.unwrap());
                assert_eq!(st > gt, later, "{shape:?}: {key} clock");
                assert!(st < s.spec.base_ts, "{shape:?}: {key} in the future");
                assert_eq!(stale.body == gold.body, wears_body, "{shape:?}: {key} body");
                assert_ne!(stale.title, gold.title);
                seen += 1;
            }
            assert!(seen > 0);
        }
    }

    /// The v1 worlds under `worlds/v1/` are the benchmark. This asserts the
    /// vendored generator still produces them byte for byte: every file's
    /// stamped digest equals the digest of the world rebuilt from its spec.
    #[test]
    fn the_v1_worlds_regenerate_from_this_crate() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/v1");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("worlds/v1 exists") {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let file: Script =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let spec = &file.spec;
            let rebuilt = build(&WorldConfig {
                size: spec.size,
                seed: spec.seed,
                k: spec.k,
                chain_len: spec.chain_len,
                pollution: spec.pollution,
                shape: spec.pollution_shape,
                spread_days: spec.spread_days,
                window_days: spec.window_days,
                authority: spec.authority,
                edition: spec.edition,
            });
            assert_eq!(
                rebuilt.digest(),
                file.digest,
                "{} no longer regenerates",
                path.display()
            );
            assert_eq!(rebuilt.compute_digest(), file.digest);
            seen += 1;
        }
        assert_eq!(seen, 10, "the ten v1 worlds");
    }

    /// The authority family is opt-in and lands after every other family's
    /// questions: the default digest does not move, every twin and every
    /// endorsement sits after the last deletion probe, all four layers and
    /// all twenty-one scenarios appear at sixty facts, and the winning
    /// rung is what a task needs.
    #[test]
    fn authority_is_opt_in_and_planted_last() {
        // Sixty-six facts: twenty-two ≡ 2 (mod 3) targets, one per scenario.
        let base = WorldConfig {
            size: 66,
            seed: 5,
            ..WorldConfig::default()
        };
        let plain = build(&base);
        let with = build(&WorldConfig {
            authority: true,
            ..base.clone()
        });
        assert_ne!(plain.digest(), with.digest());
        assert!(
            !serde_json::to_string(&plain.spec)
                .unwrap()
                .contains("authority")
        );
        assert!(with.spec.authority && !plain.spec.authority);
        assert_eq!(
            plain.probes.len(),
            with.probes
                .iter()
                .filter(|p| p.family != Family::Authority)
                .count()
        );
        // Everything before the first endorsement is the plain world.
        let first_twin = with
            .ops
            .iter()
            .position(|op| matches!(op, Op::Inscribe { record, .. } if record.key.starts_with('a')))
            .unwrap();
        assert_eq!(&with.ops[..first_twin], &plain.ops[..]);
        let mut layers = [false; 5];
        let mut scenarios = HashSet::new();
        for p in &with.probes {
            if let Expect::Ranked {
                winner,
                losers,
                layer,
                scenario,
                needs,
                order,
                ..
            } = &p.expect
            {
                layers[*layer as usize] = true;
                scenarios.insert(scenario.clone());
                assert!(winner.starts_with('a') && winner.ends_with('a'), "{winner}");
                // The original note is always the bottom rung.
                assert!(losers.last().unwrap().starts_with('f'));
                if scenario == "ladder3" || scenario == "ladder4" {
                    assert!(*order && needs.iter().any(|n| n == "endorse_retrieval"));
                }
                if scenario.starts_with("exposure") {
                    assert_eq!(needs, &["endorse_retrieval"]);
                }
                if scenario == "confirm_vs_exposure" {
                    assert_eq!(needs, &["endorse_assistant"]);
                }
            }
        }
        assert!(layers[1] && layers[2] && layers[3] && layers[4]);
        assert_eq!(scenarios.len(), SCENARIOS.len());
        let endorsements = with
            .ops
            .iter()
            .filter(|op| matches!(op, Op::Endorse { .. }))
            .count();
        assert!(endorsements > 40, "{endorsements}");
    }

    /// The v2 edition is a switch on the same generator: the default digest
    /// does not move, and the v2 world carries what v2 is about — two-word
    /// shared subjects, a fourth crossed phrasing on every tested fact,
    /// natural-null controls whose subject was written, an answer on every
    /// retrieval probe, and the three lexical-trap contradiction shapes.
    #[test]
    fn the_v2_edition_is_a_switch_on_the_same_generator() {
        let v1 = build(&WorldConfig {
            size: 60,
            seed: 3,
            ..WorldConfig::default()
        });
        let v2 = build(&WorldConfig {
            size: 60,
            seed: 3,
            edition: 2,
            ..WorldConfig::default()
        });
        assert_ne!(v1.digest(), v2.digest());
        assert!(!serde_json::to_string(&v1.spec).unwrap().contains("edition"));
        assert_eq!(v2.spec.edition, 2);
        let titles: Vec<&str> = v2
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Inscribe { record, .. } | Op::Supersede { new: record, .. } => {
                    Some(record.title.as_str())
                }
                _ => None,
            })
            .collect();
        // Every imported note's subject is three lowercase words before its
        // predicate (planted contradiction notes start however their shape
        // frames them).
        let imported = v2.ops.iter().filter_map(|op| match op {
            Op::Inscribe {
                record,
                mode: WriteMode::Import,
                ..
            } => Some(record.title.as_str()),
            _ => None,
        });
        for t in imported {
            let first = t.split_whitespace().next().unwrap();
            assert!(
                first.chars().all(|c| c.is_ascii_lowercase()),
                "v2 subject starts with a shared word: {t}"
            );
        }
        let mut crossed = 0;
        let mut natural = 0;
        let mut phantom = 0;
        let mut shapes = HashSet::new();
        for p in &v2.probes {
            match &p.expect {
                Expect::Gold {
                    phrasing, answer, ..
                } => {
                    assert!(!answer.is_empty());
                    if phrasing == "crossed" {
                        crossed += 1;
                    }
                }
                Expect::Control { natural: true } => {
                    natural += 1;
                    let Some(Op::Recall { query, .. }) = v2
                        .ops
                        .iter()
                        .find(|op| matches!(op, Op::Recall { id, .. } if *id == p.id))
                    else {
                        panic!("control has a recall")
                    };
                    // The subject named in the question WAS written …
                    let subject = titles
                        .iter()
                        .find(|t| {
                            let s = t.split_whitespace().take(4).collect::<Vec<_>>().join(" ");
                            query.contains(&s)
                        })
                        .is_some();
                    assert!(subject, "natural null names no written subject: {query}");
                }
                Expect::Control { natural: false } => phantom += 1,
                Expect::Case { shape, .. } => {
                    shapes.insert(shape.clone());
                }
                _ => {}
            }
        }
        assert_eq!(crossed, 60, "one crossed question per tested fact");
        assert_eq!(natural, 15);
        // The path reads: one per file the code refs name, every gold a
        // live record, every path an op of its own.
        let keys: HashSet<&str> = v2
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Inscribe { record, .. } | Op::Supersede { new: record, .. } => {
                    Some(record.key.as_str())
                }
                _ => None,
            })
            .collect();
        let mut bound = 0;
        for p in &v2.probes {
            if let Expect::Bound {
                path,
                gold,
                answers,
            } = &p.expect
            {
                bound += 1;
                assert!(path.starts_with("src/"), "{path}");
                assert!(!gold.is_empty() && gold.len() == answers.len());
                assert!(gold.iter().all(|g| keys.contains(g.as_str())));
                assert!(
                    gold.iter().all(|g| !g.starts_with("s-")),
                    "siblings are not gold"
                );
                let unique: HashSet<&str> = gold.iter().map(String::as_str).collect();
                assert_eq!(unique.len(), gold.len(), "a note is bound once: {path}");
                assert!(v2.ops.iter().any(
                    |op| matches!(op, Op::RecallPath { id, path: p2, .. } if *id == p.id && p2 == path)
                ));
            }
        }
        assert!(bound > 0, "v2 poses path reads");
        assert!(
            v1.probes
                .iter()
                .all(|p| !matches!(p.expect, Expect::Bound { .. })),
            "v1 has no path reads"
        );
        assert!(phantom > 0);
        for s in ["reworded", "clause", "synonym"] {
            assert!(shapes.contains(s), "v2 shape {s} missing");
        }
        for p in &v1.probes {
            if let Expect::Gold { answer, .. } = &p.expect {
                assert!(answer.is_empty(), "v1 probes stay key-graded");
            }
        }
    }

    /// The v2 worlds under `worlds/v2/` regenerate byte for byte, like v1's.
    #[test]
    fn the_v2_worlds_regenerate_from_this_crate() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("worlds/v2");
        let mut seen = 0;
        for entry in std::fs::read_dir(&dir).expect("worlds/v2 exists") {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let file: Script =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let spec = &file.spec;
            assert_eq!(spec.edition, 2, "{}", path.display());
            let rebuilt = build(&WorldConfig {
                size: spec.size,
                seed: spec.seed,
                k: spec.k,
                chain_len: spec.chain_len,
                pollution: spec.pollution,
                shape: spec.pollution_shape,
                spread_days: spec.spread_days,
                window_days: spec.window_days,
                authority: spec.authority,
                edition: spec.edition,
            });
            assert_eq!(
                rebuilt.digest(),
                file.digest,
                "{} no longer regenerates",
                path.display()
            );
            seen += 1;
        }
        assert_eq!(seen, 5, "the five v2 worlds");
    }

    #[test]
    fn the_world_clock_is_2026_09_01() {
        assert_eq!(day_to_unix(BASE_DAY), Some(1_788_220_800));
        assert_eq!(day_to_unix("1970-01-01"), Some(0));
        assert_eq!(day_to_unix("2000-03-01"), Some(951_868_800));
        assert_eq!(day_to_unix("2026-13-01"), None);
    }

    #[test]
    fn targets_are_disjoint_and_pollution_is_seeded() {
        let s = build(&small());
        let mut contradicted = HashSet::new();
        let mut deleted = HashSet::new();
        let mut polluted = HashSet::new();
        for p in &s.probes {
            match &p.expect {
                Expect::Case { gold, .. } => {
                    contradicted.insert(gold.clone());
                }
                Expect::Absent { victim, .. } => {
                    deleted.insert(victim.clone());
                }
                Expect::Drifted { gold, .. } => {
                    polluted.insert(gold.clone());
                }
                _ => {}
            }
        }
        assert!(contradicted.is_disjoint(&deleted));
        assert!(deleted.is_disjoint(&polluted));
        assert!(!polluted.is_empty(), "10% of 30 must pollute something");
        // Every stale sibling is imported before its gold.
        for p in &s.probes {
            if let Expect::Drifted { gold, stale } = &p.expect {
                let pos = |k: &str| {
                    s.ops
                        .iter()
                        .position(|op| matches!(op, Op::Inscribe { record, .. } if record.key == k))
                        .unwrap()
                };
                assert!(pos(stale) < pos(gold));
            }
        }
    }

    #[test]
    fn transitive_cases_borrow_controls_that_are_then_dropped() {
        let s = build(&WorldConfig {
            size: 60,
            seed: 3,
            ..WorldConfig::default()
        });
        let ghosts = s
            .probes
            .iter()
            .filter(|p| matches!(&p.expect, Expect::Case { shape, .. } if shape == "transitive"))
            .count();
        let controls = s
            .probes
            .iter()
            .filter(|p| p.family == Family::Abstention)
            .count();
        // controls() mints tested/4 phantoms; every ghost costs one.
        assert_eq!(controls, 60 / 4 - ghosts);
        // No control query may name a subject that any op wrote.
        let written: Vec<String> = s
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::Inscribe { record, .. } | Op::Supersede { new: record, .. } => {
                    Some(record.title.clone())
                }
                _ => None,
            })
            .collect();
        for p in &s.probes {
            if p.family != Family::Abstention {
                continue;
            }
            let Some(Op::Recall { query, .. }) = s
                .ops
                .iter()
                .find(|op| matches!(op, Op::Recall { id, .. } if *id == p.id))
            else {
                panic!("control has a recall")
            };
            let subject = query
                .split_whitespace()
                .find(|w| w.chars().next().is_some_and(char::is_uppercase))
                .unwrap_or("");
            assert!(
                !subject.is_empty() && !written.iter().any(|t| t.contains(subject)),
                "control subject {subject} was written"
            );
        }
    }

    #[test]
    fn shapes_cover_all_three_tiers_with_positives_and_negatives() {
        let s = build(&small());
        let mut tiers = [false; 4];
        let (mut pos, mut neg) = (0, 0);
        for p in &s.probes {
            if let Expect::Case { tier, positive, .. } = &p.expect {
                tiers[*tier as usize] = true;
                if *positive { pos += 1 } else { neg += 1 }
            }
        }
        assert!(tiers[1] && tiers[2] && tiers[3]);
        assert!(pos > 0 && neg > 0);
    }
}
