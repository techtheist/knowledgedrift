//! The adapter protocol — the only thing a memory system has to implement to
//! be scored by KnowledgeDrift.
//!
//! Ten operations, each one thing a coding agent's memory is asked to do in
//! the field: write a note, link two notes, re-decide something, retire a
//! note with a trace, destroy a note, sit through a session boundary, recall,
//! nominate disagreements, walk a decision's history, and price itself. A
//! system that lacks an operation says so through [`Capabilities`] and the
//! families that need it score N/A — never zero.
//!
//! In-process arms implement [`Memory`] directly. External systems (Python
//! adapters for Mem0, LangGraph stores, …) replay the same script from its
//! JSON export and hand back a transcript in the shapes defined in
//! [`crate::script`]; the grader never knows which path a transcript took.

use serde::{Deserialize, Serialize};

/// One note as the benchmark writes it. `key` is the harness's own handle —
/// every later operation addresses the note by key, so adapters keep a
/// key → native-id map and the harness never has to resolve a target by
/// searching for it (ForgetEval resolves by query, which measures the
/// resolver as much as the mutation; this protocol keeps the two apart).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    pub key: String,
    /// Node type name — Decision, Caution, Principle, Problem, Insight. A
    /// system without types stores it as a tag, or drops it.
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub code_refs: Vec<String>,
    /// Unix seconds. The world backdates every note so temporal questions
    /// have something to scope; `None` = now.
    #[serde(default)]
    pub created_at: Option<i64>,
    /// Problems open; everything else carries no status.
    #[serde(default)]
    pub open: bool,
}

/// How a write reaches the system. `Import` is a bulk load — the world being
/// poured in, no verdict expected. `Write` is an assistant-style note with the
/// system's full write-time checks: near-duplicate matching, warnings,
/// suspect nomination. Only `Write` replies are graded.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WriteMode {
    Import,
    Write,
}

/// What a write came back with. Every field is optional behaviour: a system
/// with no write-time checks returns the default and the grader reads that
/// as "created, silently".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Inscribed {
    /// The system refused to create and pointed at an existing note instead
    /// (a near-duplicate match). Key of that note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched: Option<String>,
    /// The system's own label on the (new text, matched note) pair —
    /// "contradiction" | "entailment" | "neutral" — when it has a logic layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nli_label: Option<String>,
    /// Write-time warnings: the new note landed near canon that is
    /// contradicted, superseded or deliberately removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    /// Look-alike pairs the write queued for judgment, as (newer, older) keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suspects: Vec<SuspectPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Warning {
    /// "in-active-conflict" | "superseded" | "tombstoned" — or the system's
    /// own vocabulary; only `tombstoned` is graded.
    pub reason: String,
    /// The note the warning is about, when it maps to a script key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// A nominated disagreement: two notes the system thinks a person should look
/// at together, with its hint about what it saw.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuspectPair {
    pub a: String,
    pub b: String,
    /// "contradiction" | "entailment" | "neutral" | absent (no logic layer).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl SuspectPair {
    pub fn joins(&self, x: &str, y: &str) -> bool {
        (self.a == x && self.b == y) || (self.a == y && self.b == x)
    }
}

/// One delivered record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hit {
    /// Script key of the delivered note — `None` for anything the system
    /// minted itself (a deletion marker, a summary).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Exactly the text the system put in front of the caller. This is what
    /// the attention columns bill, so it must be what the caller would read:
    /// a snippet if the system shows snippets, a whole note if it shows notes.
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// The hit is a record that knowledge was removed, not live knowledge.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tombstone: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    /// Keys of notes the hit carries as 1-hop context (titles + ids the
    /// caller could follow) — the graph layer's delivery, credited as
    /// "assisted", never as a rank.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neighbors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Recalled {
    pub hits: Vec<Hit>,
    /// The system's own "I am not sure this is in memory" signal fired for
    /// this query. Hits may still be delivered beside it (nearest candidates);
    /// the grader counts a declined answer as honest, never as a false
    /// positive, and charges it as a hedge when the answer was in fact there.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub declined: bool,
    /// The system does not rank: it put a fixed body of text in context
    /// (a memory file). Presence counts as delivery at rank 1; position is
    /// not a rank.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dump: bool,
}

/// Capture-time window, unix seconds, half-open `[after, before)`.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Window {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<i64>,
}

impl Window {
    pub fn contains(&self, ts: i64) -> bool {
        self.after.is_none_or(|a| ts >= a) && self.before.is_none_or(|b| ts < b)
    }
}

/// What a system can do. Read by the grader to decide N/A, printed in every
/// receipt so a row's blanks are explained rather than mysterious.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capabilities {
    /// Sentence-shaped links between notes are stored and delivered as
    /// neighbours.
    pub link: bool,
    /// A superseded note stays reachable from its successor (history is
    /// kept, not destroyed).
    pub history: bool,
    /// A released note leaves a findable trace that says it was removed.
    pub trace: bool,
    /// The system nominates disagreements between stored notes.
    pub suspects: bool,
    /// Recall can be scoped to a capture-time window.
    pub temporal: bool,
    /// Recall carries a calibrated "not in memory" signal.
    pub verdict: bool,
    /// Writes come back with a verdict (near-duplicate match, warnings).
    pub write_check: bool,
}

/// The protocol. Every method takes the harness key; the adapter owns the
/// mapping to its native ids.
pub trait Memory {
    fn name(&self) -> String;
    fn capabilities(&self) -> Capabilities;

    fn inscribe(&mut self, record: &Record, mode: WriteMode) -> anyhow::Result<Inscribed>;

    /// Store a sentence-shaped link. `Ok(false)` = unsupported (no-op).
    fn link(&mut self, from: &str, to: &str, verb: &str) -> anyhow::Result<bool>;

    /// `new` re-decides `old`: from now on `old` is not current. A system
    /// with history keeps `old` reachable from `new`; a flat system replaces
    /// it in place.
    fn supersede(&mut self, old: &str, new: &Record) -> anyhow::Result<()>;

    /// Retire a note deliberately, with a reason, leaving whatever trace the
    /// system leaves. A system without traces deletes.
    fn release(&mut self, key: &str, reason: &str) -> anyhow::Result<()>;

    /// Destroy a note. Nothing should remain.
    fn purge(&mut self, key: &str) -> anyhow::Result<()>;

    /// A session boundary: run whatever maintenance the system runs between
    /// sessions (calibration, sweeps, consolidation). Returns a note for the
    /// receipt.
    fn settle(&mut self) -> anyhow::Result<Option<String>>;

    fn recall(&self, query: &str, k: usize, window: Option<Window>) -> anyhow::Result<Recalled>;

    /// Every disagreement the system currently wants a person to judge.
    /// `None` = the system has no such concept.
    fn suspects(&mut self) -> anyhow::Result<Option<Vec<SuspectPair>>>;

    /// Keys reachable from `key` by walking its supersession history, oldest
    /// last. `None` = the system keeps no history.
    fn lineage(&self, key: &str) -> anyhow::Result<Option<Vec<String>>>;

    /// Tokens the system costs every session before a question is asked.
    fn standing_tokens(&self) -> usize;
}

/// Rough token estimate — the same crude rule for every arm, so the
/// comparison is a ratio and no tokenizer dependency exists.
pub fn tokens(s: &str) -> usize {
    s.chars().count().div_ceil(4)
}
