//! The baselines: flat stores with no verbs, no history, no trace, no
//! suspects. One record store, five ways of reading it.
//!
//! * `grep` — keyword overlap over the records, whole records shown.
//! * `rag` — pure vector top-k over the same embedder the product uses; no
//!   keyword channel, no priors, no reranker, no graph.
//! * `chance` — records picked by hashing the query: the floor any real
//!   retrieval has to clear.
//! * `whole` — every record in context, every time (a memory file nobody
//!   prunes).
//! * `curated` — a hand-maintained file: durable kinds first, filled to a
//!   token budget by a question-blind hash order, entries trimmed.
//!
//! Supersede replaces the record in place (what an `update` does in a flat
//! store); release and purge both delete — a flat store has no trace to
//! leave. Temporal scoping is a filter on capture time for the ranking arms
//! and unsupported for the dumps (a file has no clock).

use std::collections::HashMap;

use engram_core::Embedder;

use crate::protocol::{
    Authority, Capabilities, Hit, Inscribed, Memory, Recalled, Record, SuspectPair, Window,
    WriteMode, tokens,
};

const STOPWORDS: [&str; 46] = [
    "the", "a", "an", "is", "are", "was", "were", "it", "its", "in", "on", "of", "for", "to",
    "and", "or", "that", "this", "what", "which", "who", "how", "did", "does", "do", "we", "our",
    "us", "you", "be", "been", "with", "from", "by", "at", "as", "if", "not", "no", "yes", "about",
    "into", "than", "then", "so", "up",
];

pub fn terms(query: &str) -> Vec<String> {
    let mut out: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_lowercase())
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Order-destroying hash of a key (FNV + splitmix finaliser), so the
/// curated file's tie-break is blind to write order.
fn scramble(key: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

#[derive(Debug, Clone)]
struct Rec {
    key: String,
    kind: String,
    title: String,
    body: String,
    created_at: Option<i64>,
}

impl Rec {
    fn text(&self) -> String {
        format!("## {}\n{}", self.title, self.body)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Grep,
    Rag,
    Chance,
    Whole,
    /// Token budget.
    Curated(usize),
}

/// A believable hand-maintained memory file: a few thousand tokens.
pub const DEFAULT_CURATED_BUDGET: usize = 3000;

pub struct FlatArm {
    mode: Mode,
    recs: Vec<Rec>,
    vecs: HashMap<String, Vec<f32>>,
    embedder: Option<Box<dyn Embedder>>,
}

impl FlatArm {
    pub fn new(mode: Mode, embedder: Option<Box<dyn Embedder>>) -> Self {
        assert!(
            mode != Mode::Rag || embedder.is_some(),
            "the rag arm needs an embedder"
        );
        Self {
            mode,
            recs: Vec::new(),
            vecs: HashMap::new(),
            embedder,
        }
    }

    fn push(&mut self, r: &Record) -> anyhow::Result<()> {
        if let Some(e) = &self.embedder {
            let v = e.embed_one(&format!("{}\n{}", r.title, r.body))?;
            self.vecs.insert(r.key.clone(), v);
        }
        self.recs.push(Rec {
            key: r.key.clone(),
            kind: r.kind.clone(),
            title: r.title.clone(),
            body: r.body.clone(),
            created_at: r.created_at,
        });
        Ok(())
    }

    fn remove(&mut self, key: &str) {
        self.recs.retain(|r| r.key != key);
        self.vecs.remove(key);
    }

    fn is_dump(&self) -> bool {
        matches!(self.mode, Mode::Whole | Mode::Curated(_))
    }

    /// The curated file's contents, recomputed from the live records: a
    /// maintainer's ordering (durable kinds first, then a hash) filled to
    /// the budget with trimmed entries.
    fn curated(&self, budget: usize) -> Vec<(&Rec, String)> {
        let rank = |kind: &str| match kind {
            "Principle" => 0,
            "Decision" => 1,
            "Caution" => 2,
            "Insight" => 3,
            _ => 4,
        };
        let mut order: Vec<&Rec> = self.recs.iter().collect();
        order.sort_by_key(|r| (rank(&r.kind), scramble(&r.key)));
        let mut out = Vec::new();
        let mut used = 0;
        for r in order {
            let body: String = r.body.chars().take(200).collect();
            let entry = format!("## {}\n{}", r.title, body);
            let cost = tokens(&entry) + 1;
            if used + cost > budget {
                continue;
            }
            used += cost;
            out.push((r, entry));
        }
        out
    }

    fn hit(r: &Rec, text: String, score: Option<f64>) -> Hit {
        Hit {
            key: Some(r.key.clone()),
            text,
            score,
            tombstone: false,
            created_at: r.created_at,
            neighbors: vec![],
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        dot += f64::from(*x) * f64::from(*y);
        na += f64::from(*x) * f64::from(*x);
        nb += f64::from(*y) * f64::from(*y);
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

impl Memory for FlatArm {
    fn name(&self) -> String {
        match self.mode {
            Mode::Grep => "grep",
            Mode::Rag => "rag",
            Mode::Chance => "chance",
            Mode::Whole => "whole",
            Mode::Curated(_) => "curated",
        }
        .to_string()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            temporal: !self.is_dump(),
            ..Capabilities::default()
        }
    }

    fn inscribe(&mut self, record: &Record, _mode: WriteMode) -> anyhow::Result<Inscribed> {
        self.push(record)?;
        Ok(Inscribed::default())
    }

    fn link(&mut self, _from: &str, _to: &str, _verb: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn supersede(&mut self, old: &str, new: &Record) -> anyhow::Result<()> {
        self.remove(old);
        self.push(new)
    }

    fn release(&mut self, key: &str, _reason: &str) -> anyhow::Result<()> {
        self.remove(key);
        Ok(())
    }

    fn purge(&mut self, key: &str) -> anyhow::Result<()> {
        self.remove(key);
        Ok(())
    }

    /// A flat store has no rung to put an endorsement on.
    fn endorse(&mut self, _key: &str, _by: Authority) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn settle(&mut self) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn recall(&self, query: &str, k: usize, window: Option<Window>) -> anyhow::Result<Recalled> {
        if let Mode::Curated(budget) = self.mode {
            let hits = self
                .curated(budget)
                .into_iter()
                .map(|(r, entry)| Self::hit(r, entry, None))
                .collect();
            return Ok(Recalled {
                hits,
                declined: false,
                dump: true,
            });
        }
        if self.mode == Mode::Whole {
            let hits = self
                .recs
                .iter()
                .map(|r| Self::hit(r, r.text(), None))
                .collect();
            return Ok(Recalled {
                hits,
                declined: false,
                dump: true,
            });
        }
        let pool: Vec<(usize, &Rec)> = self
            .recs
            .iter()
            .enumerate()
            .filter(|(_, r)| match (window, r.created_at) {
                (Some(w), Some(ts)) => w.contains(ts),
                (Some(_), None) => false,
                (None, _) => true,
            })
            .collect();
        let hits: Vec<Hit> = match self.mode {
            Mode::Grep => {
                let terms = terms(query);
                let mut scored: Vec<(usize, usize, &Rec)> = pool
                    .iter()
                    .filter_map(|(i, r)| {
                        let hay = r.text().to_lowercase();
                        let n = terms.iter().filter(|t| hay.contains(*t)).count();
                        (n > 0).then_some((n, *i, *r))
                    })
                    .collect();
                scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                scored.truncate(k);
                scored
                    .into_iter()
                    .map(|(n, _, r)| {
                        Self::hit(r, r.text(), Some(n as f64 / terms.len().max(1) as f64))
                    })
                    .collect()
            }
            Mode::Rag => {
                let qv = self
                    .embedder
                    .as_ref()
                    .expect("rag has an embedder")
                    .embed_one(query)?;
                let mut scored: Vec<(f64, usize, &Rec)> = pool
                    .iter()
                    .filter_map(|(i, r)| self.vecs.get(&r.key).map(|v| (cosine(&qv, v), *i, *r)))
                    .collect();
                scored.sort_by(|a, b| {
                    b.0.partial_cmp(&a.0)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.1.cmp(&b.1))
                });
                scored.truncate(k);
                scored
                    .into_iter()
                    .map(|(s, _, r)| Self::hit(r, r.text(), Some(s.clamp(0.0, 1.0))))
                    .collect()
            }
            Mode::Chance => {
                let seed = query
                    .bytes()
                    .fold(0u64, |a, b| a.wrapping_mul(0x0100_0000_01b3) ^ u64::from(b));
                let mut rng = crate::corpus::rng::Rng::new(seed);
                let mut picked: Vec<&Rec> = Vec::new();
                while picked.len() < k.min(pool.len()) {
                    let r = pool[rng.below(pool.len())].1;
                    if !picked.iter().any(|p| p.key == r.key) {
                        picked.push(r);
                    }
                }
                picked
                    .into_iter()
                    .map(|r| Self::hit(r, r.text(), Some(0.5)))
                    .collect()
            }
            Mode::Whole | Mode::Curated(_) => unreachable!("dumps returned above"),
        };
        Ok(Recalled {
            hits,
            declined: false,
            dump: false,
        })
    }

    fn suspects(&mut self) -> anyhow::Result<Option<Vec<SuspectPair>>> {
        Ok(None)
    }

    fn lineage(&self, _key: &str) -> anyhow::Result<Option<Vec<String>>> {
        Ok(None)
    }

    fn standing_tokens(&self) -> usize {
        match self.mode {
            Mode::Whole => self.recs.iter().map(|r| tokens(&r.text()) + 1).sum(),
            Mode::Curated(budget) => self
                .curated(budget)
                .iter()
                .map(|(_, e)| tokens(e) + 1)
                .sum(),
            _ => 0,
        }
    }
}
