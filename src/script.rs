//! The script: an ordered list of protocol operations with graded probes
//! attached, and the transcript a run of it produces.
//!
//! Both halves are plain JSON. A script is exported once per size and seed;
//! any adapter — in-process or external — replays it and returns a
//! transcript; the grader scores the pair. Nothing about grading lives in
//! the adapter, so an external system can be scored without trusting it.

use serde::{Deserialize, Serialize};

use crate::protocol::{
    Authority, Capabilities, Inscribed, Recalled, Record, SuspectPair, Window, WriteMode,
};

/// Which family a probe belongs to. Attention and cost are read off the
/// retrieval probes and have no probes of their own.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Retrieval,
    Abstention,
    Currency,
    Contradiction,
    Deletion,
    Rationale,
    Temporal,
    Drift,
    Authority,
}

impl Family {
    pub const ALL: [Family; 9] = [
        Family::Retrieval,
        Family::Abstention,
        Family::Currency,
        Family::Contradiction,
        Family::Deletion,
        Family::Rationale,
        Family::Temporal,
        Family::Drift,
        Family::Authority,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Family::Retrieval => "retrieval",
            Family::Abstention => "abstention",
            Family::Currency => "currency",
            Family::Contradiction => "contradiction",
            Family::Deletion => "deletion",
            Family::Rationale => "rationale",
            Family::Temporal => "temporal",
            Family::Drift => "drift",
            Family::Authority => "authority",
        }
    }
}

/// One protocol operation. `id` is set on the operations whose replies are
/// graded (recall, suspects, lineage, and `Write`-mode inscribes); the
/// transcript echoes it so replies and probes pair up without positions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum Op {
    Inscribe {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        record: Record,
        mode: WriteMode,
    },
    Link {
        from: String,
        to: String,
        verb: String,
    },
    Supersede {
        old: String,
        new: Record,
    },
    Release {
        key: String,
        reason: String,
    },
    Purge {
        key: String,
    },
    /// Someone vouches for a note on one rung of the authority ladder.
    Endorse {
        key: String,
        by: Authority,
    },
    Settle,
    Recall {
        id: String,
        query: String,
        k: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<Window>,
    },
    /// v2: the path-shaped read — what a caller about to touch `path`
    /// should see. Answered by `Reply::Recall` like any recall.
    #[serde(rename = "recall_path")]
    RecallPath {
        id: String,
        path: String,
        k: usize,
    },
    Suspects {
        id: String,
    },
    Lineage {
        id: String,
        key: String,
    },
}

/// What a graded reply is expected to show. Every variant is a pass/fail
/// rule plus the columns it feeds; the rules themselves live in `grade.rs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "expect", rename_all = "lowercase")]
pub enum Expect {
    /// A recall whose answer is `gold`. `stale` is the drifted sibling the
    /// world polluted the subject with, when it did — a pass needs the gold
    /// delivered AND ranked above its stale twin.
    Gold {
        gold: String,
        phrasing: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stale: Option<String>,
        /// v2: the substring the delivered text must carry for the hit to
        /// count — a title-only snippet that names the note but not its
        /// value is a miss. Empty on v1 worlds (key-based credit).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        answer: String,
    },
    /// v2: a path-shaped read. `gold` is every live, truthful note bound to
    /// `path` through its code refs (a stale sibling bound to the same file
    /// is neither gold nor penalised); `answers` is index-aligned with
    /// `gold`. Pass = at least one gold delivered readably in the top five;
    /// `path_cover` is the share of the bound notes the top-k delivered.
    Bound {
        path: String,
        gold: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        answers: Vec<String>,
    },
    /// A recall about a subject that was never written. Pass = nothing
    /// delivered, or delivered under the system's own decline signal.
    /// `natural` (v2): the subject IS written, the predicate asked about is
    /// not — every word of the question exists in memory, the answer does
    /// not. Absent = a phantom subject nobody ever wrote.
    Control {
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        natural: bool,
    },
    /// A recall about a re-decided subject: `head` is current, `retired`
    /// are the generations it replaced. Pass = head in the top five and no
    /// retired generation delivered at all.
    Current {
        head: String,
        retired: Vec<String>,
        /// v2: the head's answer substring (see `Gold::answer`).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        answer: String,
    },
    /// A lineage walk from `head` must reach every retired generation.
    Lineage { head: String, retired: Vec<String> },
    /// A recall reachable only through structure: `gold` sits at the far
    /// end of a `verb` edge from `anchor`. Pass = gold in the top five
    /// directly, or carried as a neighbour by a top-five hit.
    Linked {
        gold: String,
        anchor: String,
        verb: String,
        /// v2: the gold's answer substring (see `Gold::answer`).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        answer: String,
    },
    /// A windowed recall. Pass = gold in the top five and no delivered hit
    /// captured outside the window.
    Windowed {
        gold: String,
        window: Window,
        /// v2: the gold's answer substring (see `Gold::answer`).
        #[serde(default, skip_serializing_if = "String::is_empty")]
        answer: String,
    },
    /// A planted contradiction case, graded off the planted notes' write
    /// replies and the suspects reply. `positive` = the planted note really
    /// contradicts `gold`; a negative is a trap that must NOT be flagged.
    Case {
        gold: String,
        planted: Vec<String>,
        /// The planted note whose nomination against `gold` counts (the
        /// others are scaffolding, e.g. the bridge note of a transitive
        /// case).
        witness: String,
        tier: u8,
        shape: String,
        positive: bool,
    },
    /// A drifted sibling the world imported beside `gold` — never
    /// superseded, never flagged by anyone. Pass = the system nominated the
    /// pair on its own by the end of the run.
    Drifted { gold: String, stale: String },
    /// A recall about a retired note. Pass = the victim is gone as live
    /// knowledge. `released` = retired with a trace (else purged).
    Absent {
        victim: String,
        released: bool,
        /// Substring the victim's text carried, for the role-blind leak
        /// column.
        answer: String,
    },
    /// The victim written again after its release: pass = the system warned
    /// that this knowledge was deliberately removed.
    Resurrect { victim: String, released: bool },
    /// An authority scenario: `twins` are near-identical notes about one
    /// subject (same body, a different value in the title) that were
    /// endorsed on different rungs; `winner` is the one the ladder says
    /// should come first, `losers` the rest in the order the ladder ranks
    /// them. Pass = the winner is in the top five and ranked above every
    /// other twin; when `order` is set, every delivered twin must also sit
    /// in the ladder's order; `retired` twins were superseded after their
    /// endorsement and must not be delivered at all. `needs` lists the
    /// capabilities the winning signal requires — a system without one is
    /// not attempting the task (N/A, still charged in the headline).
    Ranked {
        winner: String,
        losers: Vec<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        retired: Vec<String>,
        order: bool,
        layer: u8,
        scenario: String,
        needs: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Probe {
    pub id: String,
    pub family: Family,
    #[serde(flatten)]
    pub expect: Expect,
}

/// The stated assumptions a script was built under — printed in every
/// receipt so no number travels without them.
/// How a polluted subject's stale sibling is shaped. Each shape removes one
/// signal the default leaves in, so a family's pollution number can be
/// attributed to a mechanism rather than assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PollutionShape {
    /// The default: the flipped restatement in the generator's own wording,
    /// dated 20–40 days BEFORE the truth, whose body says it was recorded
    /// before the re-tune — content, clock and hint all agree it is old.
    #[default]
    Stale,
    /// The unlinked successor's victim: the flipped restatement wearing the
    /// truth's own body and code refs, dated 20–40 days before — told apart
    /// from the truth by nothing but the value and the clock.
    Twin,
    /// The re-imported past: the default's wording and hint, dated 20–40
    /// days AFTER the truth (a migration stamps import time as capture
    /// time) — the clock now points the wrong way.
    Late,
}

impl PollutionShape {
    pub const NAMES: [&'static str; 3] = ["stale", "twin", "late"];

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "stale" => Some(Self::Stale),
            "twin" => Some(Self::Twin),
            "late" => Some(Self::Late),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Stale => "stale",
            Self::Twin => "twin",
            Self::Late => "late",
        }
    }

    fn is_stale(&self) -> bool {
        *self == Self::Stale
    }
}

fn edition_one() -> u8 {
    1
}

fn is_edition_one(e: &u8) -> bool {
    *e == 1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorldSpec {
    pub size: usize,
    pub seed: u64,
    pub k: usize,
    pub chains: usize,
    pub chain_len: usize,
    /// Share of subjects imported with a stale, never-superseded sibling.
    pub pollution: f64,
    /// The sibling's shape. Absent from the file (and from the digest)
    /// when it is the default, so scripts exported before the knob existed
    /// keep their digest.
    #[serde(default, skip_serializing_if = "PollutionShape::is_stale")]
    pub pollution_shape: PollutionShape,
    /// Days the world's capture times are spread over, and the half-width
    /// of a temporal probe's window.
    pub spread_days: i64,
    pub window_days: i64,
    /// The world's "now" — every capture time is before it.
    pub base_ts: i64,
    /// The authority family was generated (`--authority`). Absent from the
    /// file when off, so the v1 worlds keep their digests.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub authority: bool,
    /// The benchmark edition the world was built for: 1 (absent from the
    /// file, so every v1 digest holds) or 2 — shared-vocabulary subjects, a
    /// fourth crossed phrasing, natural-null controls, reworded contradiction
    /// shapes, answer-bearing probes, and the additive score.
    #[serde(default = "edition_one", skip_serializing_if = "is_edition_one")]
    pub edition: u8,
    /// Notes in the world at the end of the import (before plantings).
    pub notes: usize,
    pub edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Script {
    pub spec: WorldSpec,
    /// A stable digest of spec + ops + probes, stamped by the generator and
    /// carried in the file, so an external runner copies it into its
    /// transcript instead of re-deriving a serialization byte for byte.
    #[serde(default)]
    pub digest: String,
    pub ops: Vec<Op>,
    pub probes: Vec<Probe>,
}

impl Script {
    pub fn probe(&self, id: &str) -> Option<&Probe> {
        self.probes.iter().find(|p| p.id == id)
    }

    /// Compute the digest over spec, ops and probes (FNV-1a over the
    /// compact JSON), the value `build` stamps into `digest`.
    pub fn compute_digest(&self) -> String {
        let bytes =
            serde_json::to_vec(&(&self.spec, &self.ops, &self.probes)).expect("script serializes");
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        format!("{h:016x}")
    }

    /// The stamped digest — what transcripts and receipts quote.
    pub fn digest(&self) -> String {
        if self.digest.is_empty() {
            self.compute_digest()
        } else {
            self.digest.clone()
        }
    }

    /// Stamp (or re-stamp) the digest after the ops and probes are final.
    pub fn seal(mut self) -> Self {
        self.digest = String::new();
        self.digest = self.compute_digest();
        self
    }
}

/// One reply, tagged with the probe id it answers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "reply", rename_all = "lowercase")]
pub enum Reply {
    Inscribe {
        id: String,
        result: Inscribed,
    },
    Recall {
        id: String,
        result: Recalled,
    },
    Suspects {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pairs: Option<Vec<SuspectPair>>,
    },
    Lineage {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keys: Option<Vec<String>>,
    },
}

impl Reply {
    pub fn id(&self) -> &str {
        match self {
            Reply::Inscribe { id, .. }
            | Reply::Recall { id, .. }
            | Reply::Suspects { id, .. }
            | Reply::Lineage { id, .. } => id,
        }
    }
}

/// Wall-clock per operation kind — the cost family's latency column.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Timing {
    pub count: usize,
    pub total_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transcript {
    pub arm: String,
    pub capabilities: Capabilities,
    pub standing_tokens: usize,
    /// The script this transcript answers, so a mismatch is caught before
    /// grading rather than scored as zeros.
    pub script_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settle_note: Option<String>,
    pub replies: Vec<Reply>,
    #[serde(default)]
    pub timing: std::collections::BTreeMap<String, Timing>,
}

impl Transcript {
    pub fn reply(&self, id: &str) -> Option<&Reply> {
        self.replies.iter().find(|r| r.id() == id)
    }
}
