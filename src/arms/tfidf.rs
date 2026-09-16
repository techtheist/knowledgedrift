//! `tfidf` — the lexical arm that cracked v1, rebuilt in-process so the v2
//! changes can be measured against the thing they were made for.
//!
//! What the field receipt did (2026-09-15, TF-IDF in SQL, ~138 tokens per
//! answer, score 823 on the v1 1500 world) and what this arm does:
//!
//! * **ranking** — cosine over TF-IDF vectors of `title + body`;
//! * **snippets** — a hit shows its title only, and only hits within half
//!   the best cosine are shown at all, so most answers are one or two
//!   titles: the bill is a tenth of a whole-note store's and the v1 signal
//!   share climbs with it;
//! * **abstention by idf oracle** — a capitalised query token the corpus has
//!   never seen (a coined phantom subject) declines the query; a best cosine
//!   under a floor declines it too;
//! * **contradiction by token differencing** — a written note whose title
//!   shares most of its tokens with a stored title is nominated when the
//!   tokens that differ carry a number or a negation, and reported as a
//!   near-duplicate match when nothing differs;
//! * **drift by the same rule** at every settle, over every pair of stored
//!   titles;
//! * a native capture-time window.
//!
//! No model anywhere. It is not a straw man: on a v1 world it out-scores the
//! reference system, which is the point of keeping it.

use std::collections::{BTreeSet, HashMap};

use crate::protocol::{
    Authority, Capabilities, Hit, Inscribed, Memory, Recalled, Record, SuspectPair, Window,
    WriteMode,
};

const STOP: [&str; 40] = [
    "the", "a", "an", "is", "are", "was", "were", "it", "its", "in", "on", "of", "for", "to",
    "and", "or", "that", "this", "what", "which", "who", "how", "did", "does", "do", "we", "our",
    "be", "been", "with", "from", "by", "at", "as", "if", "so", "up", "one", "once", "around",
];

const NEGATION: [&str; 9] = [
    "not", "never", "no", "wrong", "other", "than", "nobody", "stopped", "contrary",
];

/// Best cosine under this and the arm says "not in memory".
const FLOOR: f64 = 0.08;
/// Hits under this share of the best cosine are not shown.
const SHOW: f64 = 0.7;

fn toks(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}

fn is_number(t: &str) -> bool {
    t.chars().all(|c| c.is_ascii_digit())
}

#[derive(Debug, Clone)]
struct Doc {
    key: String,
    title: String,
    tf: HashMap<String, f64>,
    title_toks: BTreeSet<String>,
    created_at: Option<i64>,
}

#[derive(Default)]
pub struct TfidfArm {
    docs: Vec<Doc>,
    df: HashMap<String, usize>,
    queue: Vec<SuspectPair>,
}

impl TfidfArm {
    fn idf(&self, t: &str) -> f64 {
        let n = self.docs.len().max(1) as f64;
        let d = self.df.get(t).copied().unwrap_or(0) as f64;
        ((1.0 + n) / (1.0 + d)).ln() + 1.0
    }

    fn vec_of(&self, tf: &HashMap<String, f64>) -> (HashMap<String, f64>, f64) {
        let mut v = HashMap::new();
        let mut norm = 0.0;
        for (t, c) in tf {
            let w = (1.0 + c.ln()) * self.idf(t);
            norm += w * w;
            v.insert(t.clone(), w);
        }
        (v, norm.sqrt())
    }

    fn push(&mut self, r: &Record) {
        let mut tf: HashMap<String, f64> = HashMap::new();
        for t in toks(&format!("{}\n{}", r.title, r.body)) {
            *tf.entry(t).or_insert(0.0) += 1.0;
        }
        for t in tf.keys() {
            *self.df.entry(t.clone()).or_insert(0) += 1;
        }
        self.docs.push(Doc {
            key: r.key.clone(),
            title: r.title.clone(),
            tf,
            title_toks: toks(&r.title).into_iter().collect(),
            created_at: r.created_at,
        });
    }

    fn remove(&mut self, key: &str) {
        if let Some(i) = self.docs.iter().position(|d| d.key == key) {
            let d = self.docs.remove(i);
            for t in d.tf.keys() {
                if let Some(n) = self.df.get_mut(t) {
                    *n = n.saturating_sub(1);
                }
            }
        }
    }

    /// Token differencing: `Some(hint)` when most of `a`'s tokens appear in
    /// `b` (containment, so a stored title's trailing clause costs nothing)
    /// — "contradiction" if what `a` adds carries a number or a negation,
    /// "entailment" if it adds nothing of the sort.
    fn differ(a: &BTreeSet<String>, b: &BTreeSet<String>) -> Option<&'static str> {
        let shared = a.intersection(b).count();
        if shared < 4 || (shared as f64) / (a.len().max(1) as f64) < 0.6 {
            return None;
        }
        let loaded = a
            .difference(b)
            .any(|t| is_number(t) || NEGATION.contains(&t.as_str()));
        Some(if loaded {
            "contradiction"
        } else {
            "entailment"
        })
    }

    fn nominate(&mut self, a: &str, b: &str, hint: &str) {
        if self.queue.iter().any(|p| p.joins(a, b)) {
            return;
        }
        self.queue.push(SuspectPair {
            a: a.to_string(),
            b: b.to_string(),
            hint: Some(hint.to_string()),
        });
    }
}

impl Memory for TfidfArm {
    fn name(&self) -> String {
        "tfidf".to_string()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            suspects: true,
            temporal: true,
            verdict: true,
            write_check: true,
            ..Capabilities::default()
        }
    }

    fn inscribe(&mut self, record: &Record, mode: WriteMode) -> anyhow::Result<Inscribed> {
        let mut out = Inscribed::default();
        if mode == WriteMode::Write {
            let mine: BTreeSet<String> = toks(&record.title).into_iter().collect();
            let mut best: Option<(usize, String, &'static str)> = None;
            for d in &self.docs {
                if let Some(hint) = Self::differ(&mine, &d.title_toks) {
                    let shared = mine.intersection(&d.title_toks).count();
                    if best.as_ref().is_none_or(|(s, _, _)| shared > *s) {
                        best = Some((shared, d.key.clone(), hint));
                    }
                }
            }
            if let Some((_, key, hint)) = best {
                let stored = self
                    .docs
                    .iter()
                    .find(|d| d.key == key)
                    .map(|d| d.title_toks.clone())
                    .unwrap_or_default();
                if hint == "entailment" && mine.is_subset(&stored) {
                    out.matched = Some(key);
                    out.nli_label = Some("entailment".into());
                } else if hint == "contradiction" {
                    out.suspects.push(SuspectPair {
                        a: record.key.clone(),
                        b: key,
                        hint: Some("contradiction".into()),
                    });
                }
            }
        }
        self.push(record);
        Ok(out)
    }

    fn link(&mut self, _from: &str, _to: &str, _verb: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn supersede(&mut self, old: &str, new: &Record) -> anyhow::Result<()> {
        self.remove(old);
        self.push(new);
        Ok(())
    }

    fn release(&mut self, key: &str, _reason: &str) -> anyhow::Result<()> {
        self.remove(key);
        Ok(())
    }

    fn purge(&mut self, key: &str) -> anyhow::Result<()> {
        self.remove(key);
        Ok(())
    }

    fn endorse(&mut self, _key: &str, _by: Authority) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Drift scan: every pair of stored titles under the differencing rule.
    fn settle(&mut self) -> anyhow::Result<Option<String>> {
        let mut found = Vec::new();
        for (i, a) in self.docs.iter().enumerate() {
            for b in &self.docs[i + 1..] {
                if Self::differ(&a.title_toks, &b.title_toks) == Some("contradiction")
                    || Self::differ(&b.title_toks, &a.title_toks) == Some("contradiction")
                {
                    found.push((a.key.clone(), b.key.clone()));
                }
            }
        }
        let n = found.len();
        for (a, b) in found {
            self.nominate(&a, &b, "contradiction");
        }
        Ok(Some(format!("token differencing nominated {n} pairs")))
    }

    fn recall(&self, query: &str, k: usize, window: Option<Window>) -> anyhow::Result<Recalled> {
        let qt = toks(query);
        // The idf oracle: a name nobody ever wrote (a capitalised token the
        // corpus has no document for) means nobody wrote the subject;
        // decline. Lower-case unseen words are just wording.
        let unseen = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2 && w.chars().next().is_some_and(char::is_uppercase))
            .any(|w| self.df.get(&w.to_lowercase()).copied().unwrap_or(0) == 0);
        let mut qtf: HashMap<String, f64> = HashMap::new();
        for t in &qt {
            *qtf.entry(t.clone()).or_insert(0.0) += 1.0;
        }
        let (qv, qn) = self.vec_of(&qtf);
        let mut scored: Vec<(f64, usize)> = self
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| match (window, d.created_at) {
                (Some(w), Some(ts)) => w.contains(ts),
                (Some(_), None) => false,
                (None, _) => true,
            })
            .filter_map(|(i, d)| {
                let (dv, dn) = self.vec_of(&d.tf);
                if qn == 0.0 || dn == 0.0 {
                    return None;
                }
                let dot: f64 = qv
                    .iter()
                    .filter_map(|(t, w)| dv.get(t).map(|x| w * x))
                    .sum();
                let c = dot / (qn * dn);
                (c > 0.0).then_some((c, i))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        scored.truncate(k);
        let best = scored.first().map_or(0.0, |s| s.0);
        scored.retain(|(c, _)| *c >= SHOW * best);
        let declined = unseen || best < FLOOR;
        let hits = scored
            .into_iter()
            .map(|(c, i)| {
                let d = &self.docs[i];
                Hit {
                    key: Some(d.key.clone()),
                    text: d.title.clone(),
                    score: Some(c),
                    tombstone: false,
                    created_at: d.created_at,
                    neighbors: vec![],
                }
            })
            .collect();
        Ok(Recalled {
            hits,
            declined,
            dump: false,
        })
    }

    fn suspects(&mut self) -> anyhow::Result<Option<Vec<SuspectPair>>> {
        Ok(Some(self.queue.clone()))
    }

    fn lineage(&self, _key: &str) -> anyhow::Result<Option<Vec<String>>> {
        Ok(None)
    }

    fn standing_tokens(&self) -> usize {
        0
    }
}
