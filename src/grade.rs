//! Grading: a script and a transcript in, one pass/fail per task and the
//! family columns out. Nothing here calls a model; every rule is arithmetic
//! over keys the script knew before the question was asked.
//!
//! Three numbers head every arm's row:
//!
//! * **success** — the micro-average over EVERY task in the script: tasks
//!   passed over tasks posed, every family pooled. A task the system cannot
//!   attempt (an N/A family: no suspect queue, no clock, no history) counts
//!   as failed — a memory that cannot notice drift has not noticed it. The
//!   family row says why, and `attempted_success` (passed over attempted)
//!   is printed beside it for the capability-aware reading.
//! * **composite** — the macro-average: the mean of every family's pass
//!   rate, each family weighing the same however many tasks it has, an N/A
//!   family scoring zero.
//! * **score** — composite × an efficiency multiplier read off the signal
//!   share `S` (mean share of delivered tokens that were the answer, over
//!   the retrieval tasks that delivered it): `m = clamp(10·S, 0.1, 10)`.
//!   Ten percent signal is the ×1 baseline; a dump that holds everything
//!   scores S ≈ 0.01 and keeps a tenth of its composite; a memory that
//!   answers in a few relevant tokens can earn up to ×10. The multiplier is
//!   a stated sketch, printed beside the unmultiplied number. The score is
//!   scaled ×100 so it reads as a whole number: 0.87 × 6.4 → 550.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::protocol::{Capabilities, Recalled, SuspectPair, tokens};
use crate::script::{Expect, Family, Reply, Script, Timing, Transcript};

#[derive(Debug, Clone, Serialize)]
pub struct FamilyReport {
    pub family: String,
    /// Tasks the script poses in this family (the same for every system).
    pub posed: usize,
    /// Tasks this system attempted.
    pub tasks: usize,
    pub passed: usize,
    /// `None` when the family is N/A for this system.
    pub pass_rate: Option<f64>,
    /// Why the family is N/A, when it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub na: Option<String>,
    pub columns: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Graded {
    pub arm: String,
    pub capabilities: Capabilities,
    pub script_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settle_note: Option<String>,
    /// Tasks the script poses — the same number for every system.
    pub tasks: usize,
    /// Tasks this system could attempt (its capabilities cover them).
    pub attempted: usize,
    pub passed: usize,
    /// passed / tasks — N/A counts as failed.
    pub success: f64,
    /// passed / attempted — the capability-aware reading.
    pub attempted_success: f64,
    pub composite: f64,
    pub signal_share: f64,
    pub multiplier: f64,
    /// 100 × composite × multiplier.
    pub score: f64,
    pub families: Vec<FamilyReport>,
    pub attention: BTreeMap<String, f64>,
    pub cost: BTreeMap<String, f64>,
    pub timing: BTreeMap<String, Timing>,
    /// Ids of every failed task, so a receipt can be read back to the case.
    pub failed: Vec<String>,
}

/// Per-task verdict plus whatever columns the task contributes.
struct Task {
    id: String,
    family: Family,
    pass: bool,
}

struct Acc {
    tasks: Vec<Task>,
    /// (family, column) -> (sum, count); reported as means.
    cols: BTreeMap<(Family, &'static str), (f64, usize)>,
    na: BTreeMap<Family, String>,
}

impl Acc {
    fn task(&mut self, id: &str, family: Family, pass: bool) {
        self.tasks.push(Task {
            id: id.to_string(),
            family,
            pass,
        });
    }
    fn col(&mut self, family: Family, name: &'static str, v: f64) {
        let e = self.cols.entry((family, name)).or_insert((0.0, 0));
        e.0 += v;
        e.1 += 1;
    }
}

fn b(x: bool) -> f64 {
    if x { 1.0 } else { 0.0 }
}

/// Which probes are tasks — the same set for every system. The one probe
/// that is a column and never a task: writing a PURGED note again, which
/// nobody can be expected to warn about.
pub fn is_task(p: &crate::script::Probe) -> bool {
    !matches!(
        p.expect,
        Expect::Resurrect {
            released: false,
            ..
        }
    )
}

/// 1-based rank of `key` among live (non-tombstone) hits; a dump delivers
/// everything it holds at rank 1.
fn rank_of(r: &Recalled, key: &str) -> Option<usize> {
    let pos = r
        .hits
        .iter()
        .filter(|h| !h.tombstone)
        .position(|h| h.key.as_deref() == Some(key));
    match (r.dump, pos) {
        (true, Some(_)) => Some(1),
        (_, p) => p.map(|p| p + 1),
    }
}

fn delivered_tokens(r: &Recalled) -> usize {
    r.hits.iter().map(|h| tokens(&h.text)).sum()
}

fn top_score(r: &Recalled) -> Option<f64> {
    r.hits
        .iter()
        .filter_map(|h| h.score)
        .fold(None, |m, s| Some(m.map_or(s, |m: f64| m.max(s))))
}

fn recall_reply<'a>(t: &'a Transcript, id: &str) -> anyhow::Result<&'a Recalled> {
    match t.reply(id) {
        Some(Reply::Recall { result, .. }) => Ok(result),
        _ => anyhow::bail!("transcript has no recall reply for probe {id}"),
    }
}

pub fn grade(script: &Script, t: &Transcript) -> anyhow::Result<Graded> {
    anyhow::ensure!(
        t.script_digest == script.digest(),
        "transcript answers script {} but this script is {} (a loaded script must carry the digest its file was exported with)",
        t.script_digest,
        script.digest()
    );
    let caps = t.capabilities;
    let mut acc = Acc {
        tasks: Vec::new(),
        cols: BTreeMap::new(),
        na: BTreeMap::new(),
    };
    if !caps.suspects {
        acc.na
            .insert(Family::Contradiction, "no suspect nomination".into());
        acc.na.insert(Family::Drift, "no suspect nomination".into());
    }
    if !caps.temporal {
        acc.na
            .insert(Family::Temporal, "no capture-time scoping".into());
    }
    if !caps.endorses_any() {
        acc.na
            .insert(Family::Authority, "no endorsement rung".into());
    }

    // The suspect queue at the end of the run, plus every write-time
    // nomination, pooled: a pair counts wherever the system raised it.
    let mut queue: Vec<SuspectPair> = Vec::new();
    let mut inscribed: BTreeMap<&str, &crate::protocol::Inscribed> = BTreeMap::new();
    for r in &t.replies {
        match r {
            Reply::Suspects { pairs: Some(p), .. } => queue.extend(p.iter().cloned()),
            Reply::Inscribe { id, result } => {
                queue.extend(result.suspects.iter().cloned());
                inscribed.insert(id.as_str(), result);
            }
            _ => {}
        }
    }
    // Which script key each Write-mode inscribe wrote — a Case is graded by
    // the planted key, and the reply is filed under the op id.
    let key_to_write: BTreeMap<&str, &str> = script
        .ops
        .iter()
        .filter_map(|op| match op {
            crate::script::Op::Inscribe {
                id: Some(id),
                record,
                ..
            } => Some((record.key.as_str(), id.as_str())),
            _ => None,
        })
        .collect();
    let write_of = |key: &str| {
        key_to_write
            .get(key)
            .and_then(|id| inscribed.get(id).copied())
    };
    // A pair "raised" between two keys, with the strongest hint the system
    // attached: a near-duplicate match counts as raising the pair too.
    let raised = |x: &str, y: &str| -> (bool, Option<String>, bool) {
        let mut hit = false;
        let mut hint: Option<String> = None;
        let mut absorbed = false;
        for p in &queue {
            if p.joins(x, y) {
                hit = true;
                if p.hint.as_deref() == Some("contradiction") || hint.is_none() {
                    hint = p.hint.clone();
                }
            }
        }
        if let Some(w) = write_of(x)
            && w.matched.as_deref() == Some(y)
        {
            hit = true;
            if w.nli_label.as_deref() == Some("contradiction") {
                hint = w.nli_label.clone();
            } else {
                absorbed = true;
                if hint.is_none() {
                    hint = w.nli_label.clone();
                }
            }
        }
        (hit, hint, absorbed)
    };

    let mut answerable_scores: Vec<Option<f64>> = Vec::new();
    let mut control_scores: Vec<Option<f64>> = Vec::new();
    let mut focus_sum = 0.0;
    let mut focus_n = 0usize;
    let mut phrasing: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for p in &script.probes {
        if acc.na.contains_key(&p.family) {
            continue;
        }
        match &p.expect {
            Expect::Gold {
                gold,
                phrasing: ph,
                stale,
            } => {
                let r = recall_reply(t, &p.id)?;
                let rank = rank_of(r, gold);
                let stale_rank = stale.as_deref().and_then(|s| rank_of(r, s));
                let found5 = rank.is_some_and(|k| k <= 5);
                // A stale twin ranked above (or beside, in a dump) the
                // answer hands the reader the wrong value.
                let stale_above = match (rank, stale_rank) {
                    (Some(g), Some(s)) => r.dump || s < g,
                    (None, Some(_)) => true,
                    _ => false,
                };
                let pass = found5 && !stale_above;
                acc.task(&p.id, Family::Retrieval, pass);
                acc.col(Family::Retrieval, "r@1", b(rank == Some(1)));
                acc.col(Family::Retrieval, "r@5", b(found5));
                acc.col(
                    Family::Retrieval,
                    "mrr",
                    rank.map_or(0.0, |k| 1.0 / k as f64),
                );
                let returned = r.hits.len();
                let noise = if returned == 0 {
                    0.0
                } else {
                    (returned - usize::from(rank.is_some())) as f64 / returned as f64
                };
                acc.col(Family::Retrieval, "noise", noise);
                acc.col(Family::Retrieval, "tokens", delivered_tokens(r) as f64);
                if let Some(h) = r.hits.iter().find(|h| h.key.as_deref() == Some(gold)) {
                    let total = delivered_tokens(r).max(1);
                    let f = tokens(&h.text) as f64 / total as f64;
                    focus_sum += f;
                    focus_n += 1;
                    acc.col(Family::Retrieval, "hedge", b(r.declined));
                }
                if stale.is_some() {
                    acc.col(Family::Retrieval, "stale_above", b(stale_above));
                }
                let e = phrasing.entry(ph.clone()).or_insert((0, 0));
                e.0 += usize::from(found5);
                e.1 += 1;
                answerable_scores.push(top_score(r));
            }
            Expect::Control => {
                let r = recall_reply(t, &p.id)?;
                let answered = !r.hits.is_empty();
                let pass = !answered || r.declined;
                acc.task(&p.id, Family::Abstention, pass);
                acc.col(Family::Abstention, "answered", b(answered));
                acc.col(Family::Abstention, "fp", b(!pass));
                acc.col(Family::Abstention, "declined", b(answered && r.declined));
                control_scores.push(top_score(r));
            }
            Expect::Current { head, retired } => {
                let r = recall_reply(t, &p.id)?;
                let rank = rank_of(r, head);
                let polluted = retired.iter().any(|k| rank_of(r, k).is_some());
                let pass = rank.is_some_and(|k| k <= 5) && !polluted;
                acc.task(&p.id, Family::Currency, pass);
                acc.col(Family::Currency, "head_r@1", b(rank == Some(1)));
                acc.col(
                    Family::Currency,
                    "head_r@5",
                    b(rank.is_some_and(|k| k <= 5)),
                );
                acc.col(Family::Currency, "pollution", b(polluted));
            }
            Expect::Lineage { retired, .. } => match t.reply(&p.id) {
                Some(Reply::Lineage {
                    keys: Some(keys), ..
                }) => {
                    let pass = retired.iter().all(|k| keys.contains(k));
                    acc.task(&p.id, Family::Currency, pass);
                    acc.col(Family::Currency, "lineage", b(pass));
                }
                _ => {
                    // No history: the walk is not attempted, not failed.
                    acc.col(Family::Currency, "lineage_na", 1.0);
                }
            },
            Expect::Linked { gold, .. } => {
                let r = recall_reply(t, &p.id)?;
                let direct = rank_of(r, gold).is_some_and(|k| k <= 5);
                let assisted = r
                    .hits
                    .iter()
                    .filter(|h| !h.tombstone)
                    .take(5)
                    .any(|h| h.neighbors.iter().any(|n| n == gold));
                let pass = direct || assisted;
                acc.task(&p.id, Family::Rationale, pass);
                acc.col(Family::Rationale, "direct_r@5", b(direct));
                acc.col(Family::Rationale, "assisted_r@5", b(pass));
                acc.col(Family::Rationale, "structure_only", b(!direct && assisted));
            }
            Expect::Windowed { gold, window } => {
                let r = recall_reply(t, &p.id)?;
                let found = rank_of(r, gold).is_some_and(|k| k <= 5);
                let leak = r
                    .hits
                    .iter()
                    .any(|h| h.created_at.is_some_and(|ts| !window.contains(ts)));
                let pass = found && !leak;
                acc.task(&p.id, Family::Temporal, pass);
                acc.col(Family::Temporal, "in_window_r@5", b(found));
                acc.col(Family::Temporal, "leak", b(leak));
            }
            Expect::Case {
                gold,
                witness,
                tier,
                positive,
                ..
            } => {
                let (queued, hint, absorbed) = raised(witness, gold);
                let flagged = hint.as_deref() == Some("contradiction");
                let cleared = matches!(hint.as_deref(), Some("entailment") | Some("neutral"));
                let pass = if *positive {
                    // The pair reached the judge, was not merged away, and
                    // the hint did not tell the judge it agrees.
                    queued && !absorbed && hint.as_deref() != Some("entailment")
                } else {
                    // Never raised, raised-and-cleared, or merged as the
                    // duplicate it is. Raised with a contradiction hint or
                    // with no hint at all = a wasted judgment.
                    !queued || cleared || (absorbed && !flagged)
                };
                acc.task(&p.id, Family::Contradiction, pass);
                let tier_col: &'static str = match (*tier, *positive) {
                    (1, true) => "t1_recall",
                    (2, true) => "t2_recall",
                    (3, true) => "t3_recall",
                    (1, false) => "t1_false_alarm",
                    (2, false) => "t2_false_alarm",
                    _ => "t3_false_alarm",
                };
                acc.col(
                    Family::Contradiction,
                    tier_col,
                    b(if *positive { pass } else { !pass }),
                );
                if *positive {
                    acc.col(Family::Contradiction, "queued", b(queued));
                    acc.col(Family::Contradiction, "flagged", b(flagged));
                    acc.col(Family::Contradiction, "absorbed", b(absorbed));
                }
            }
            Expect::Drifted { gold, stale } => {
                let (queued, hint, _) = raised(stale, gold);
                let pass = queued && hint.as_deref() != Some("entailment");
                acc.task(&p.id, Family::Drift, pass);
                acc.col(Family::Drift, "noticed", b(pass));
                acc.col(
                    Family::Drift,
                    "flagged",
                    b(hint.as_deref() == Some("contradiction")),
                );
            }
            Expect::Absent {
                victim,
                released,
                answer,
            } => {
                let r = recall_reply(t, &p.id)?;
                let gone = rank_of(r, victim).is_none();
                acc.task(&p.id, Family::Deletion, gone);
                if *released {
                    acc.col(Family::Deletion, "released_gone", b(gone));
                    if caps.trace {
                        acc.col(
                            Family::Deletion,
                            "trace",
                            b(r.hits.iter().any(|h| h.tombstone)),
                        );
                        let needle = answer.to_lowercase();
                        acc.col(
                            Family::Deletion,
                            "marker_leak",
                            b(r.hits
                                .iter()
                                .any(|h| h.tombstone && h.text.to_lowercase().contains(&needle))),
                        );
                    }
                } else {
                    acc.col(Family::Deletion, "purged_gone", b(gone));
                }
            }
            Expect::Ranked {
                winner,
                losers,
                retired,
                order,
                layer,
                scenario,
                needs,
            } => {
                // The winning signal needs a rung (or history) the system
                // did not declare: not attempted, still posed.
                let na_col: &'static str = "na_share";
                if needs.iter().any(|n| caps.has(n) != Some(true)) {
                    acc.col(Family::Authority, na_col, 1.0);
                    continue;
                }
                acc.col(Family::Authority, na_col, 0.0);
                let r = recall_reply(t, &p.id)?;
                let wr = rank_of(r, winner);
                let top5 = wr.is_some_and(|k| k <= 5);
                // Above every other twin — a dump holding both is ambiguous.
                let above_all = losers.iter().all(|l| match (wr, rank_of(r, l)) {
                    (Some(w), Some(l)) => !r.dump && w < l,
                    (Some(_), None) => true,
                    (None, _) => false,
                });
                let resurrected = retired.iter().any(|k| rank_of(r, k).is_some());
                let ordered = if *order {
                    let mut last = wr;
                    let mut ok = wr.is_some() && !r.dump;
                    for l in losers {
                        if let Some(lr) = rank_of(r, l) {
                            if last.is_some_and(|p| lr <= p) {
                                ok = false;
                            }
                            last = Some(lr);
                        }
                    }
                    ok
                } else {
                    true
                };
                let pass = top5 && above_all && !resurrected && ordered;
                acc.task(&p.id, Family::Authority, pass);
                acc.col(Family::Authority, "winner_top", b(top5 && above_all));
                acc.col(Family::Authority, "winner_r@5", b(top5));
                let layer_col: &'static str = match layer {
                    1 => "l1_autonomous",
                    2 => "l2_governed",
                    3 => "l3_three_hands",
                    _ => "l4_supervised",
                };
                acc.col(Family::Authority, layer_col, b(pass));
                if *order {
                    acc.col(Family::Authority, "order_exact", b(ordered));
                }
                if !retired.is_empty() {
                    acc.col(Family::Authority, "resurrected", b(resurrected));
                }
                // Per-scenario pass rate, for reading a receipt back to the
                // case; interned so the column table can stay &'static.
                let name: &'static str = Box::leak(format!("s_{scenario}").into_boxed_str());
                acc.col(Family::Authority, name, b(pass));
            }
            Expect::Resurrect { released, .. } => {
                let warned = inscribed
                    .get(p.id.as_str())
                    .is_some_and(|w| w.warnings.iter().any(|x| x.reason == "tombstoned"));
                if *released {
                    if caps.trace {
                        acc.task(&p.id, Family::Deletion, warned);
                    }
                    acc.col(Family::Deletion, "resurrection_warned", b(warned));
                } else {
                    acc.col(Family::Deletion, "purged_rewrite_warned", b(warned));
                }
            }
        }
    }

    // ---- roll up ----------------------------------------------------------
    let mut families = Vec::new();
    for fam in Family::ALL {
        let tasks: Vec<&Task> = acc.tasks.iter().filter(|t| t.family == fam).collect();
        let passed = tasks.iter().filter(|t| t.pass).count();
        let mut columns: BTreeMap<String, f64> = acc
            .cols
            .iter()
            .filter(|((f, _), _)| *f == fam)
            .map(|((_, name), (sum, n))| (name.to_string(), sum / (*n).max(1) as f64))
            .collect();
        if fam == Family::Retrieval {
            let r5 = |k: &str| {
                phrasing
                    .get(k)
                    .map_or(0.0, |(h, n)| *h as f64 / (*n).max(1) as f64)
            };
            columns.insert("lexical_r@5".into(), r5("lexical"));
            columns.insert("paraphrase_r@5".into(), r5("paraphrase"));
            columns.insert("oblique_r@5".into(), r5("oblique"));
            columns.insert(
                "weighted_r@5".into(),
                0.45 * r5("lexical") + 0.45 * r5("paraphrase") + 0.10 * r5("oblique"),
            );
        }
        if fam == Family::Abstention {
            columns.insert(
                "separation".into(),
                separation(&answerable_scores, &control_scores),
            );
        }
        let posed = script
            .probes
            .iter()
            .filter(|p| p.family == fam && is_task(p))
            .count();
        let na = acc
            .na
            .get(&fam)
            .cloned()
            .or_else(|| (posed == 0).then(|| "no tasks in this world".to_string()));
        families.push(FamilyReport {
            family: fam.as_str().to_string(),
            posed,
            tasks: tasks.len(),
            passed,
            pass_rate: (na.is_none()).then(|| passed as f64 / tasks.len().max(1) as f64),
            na,
            columns,
        });
    }

    let tasks = script.probes.iter().filter(|p| is_task(p)).count();
    let attempted = acc.tasks.len();
    let passed = acc.tasks.iter().filter(|t| t.pass).count();
    let success = passed as f64 / tasks.max(1) as f64;
    let attempted_success = passed as f64 / attempted.max(1) as f64;
    // The macro mean over the families this WORLD poses: a world built
    // without the authority family averages eight, one built with it nine.
    // An N/A family the world does pose still scores zero.
    let posed_families: Vec<&FamilyReport> = families.iter().filter(|f| f.posed > 0).collect();
    let composite = posed_families
        .iter()
        .map(|f| f.pass_rate.unwrap_or(0.0))
        .sum::<f64>()
        / posed_families.len().max(1) as f64;
    let signal_share = if focus_n == 0 {
        0.0
    } else {
        focus_sum / focus_n as f64
    };
    let multiplier = (10.0 * signal_share).clamp(0.1, 10.0);

    let retrieval = families
        .iter()
        .find(|f| f.family == "retrieval")
        .map(|f| f.columns.clone())
        .unwrap_or_default();
    let mut attention = BTreeMap::new();
    attention.insert("focus".into(), signal_share);
    attention.insert(
        "noise".into(),
        retrieval.get("noise").copied().unwrap_or(0.0),
    );
    attention.insert(
        "tokens_per_query".into(),
        retrieval.get("tokens").copied().unwrap_or(0.0),
    );
    let mut cost = BTreeMap::new();
    cost.insert("standing_tokens".into(), t.standing_tokens as f64);
    cost.insert(
        "tokens_per_query".into(),
        retrieval.get("tokens").copied().unwrap_or(0.0),
    );
    for (op, tm) in &t.timing {
        cost.insert(format!("{op}_ms"), tm.total_ms / tm.count.max(1) as f64);
    }

    Ok(Graded {
        arm: t.arm.clone(),
        capabilities: caps,
        script_digest: t.script_digest.clone(),
        settle_note: t.settle_note.clone(),
        tasks,
        attempted,
        passed,
        success,
        attempted_success,
        composite,
        signal_share,
        multiplier,
        score: composite * multiplier * 100.0,
        families,
        attention,
        cost,
        timing: t.timing.clone(),
        failed: acc
            .tasks
            .iter()
            .filter(|t| !t.pass)
            .map(|t| t.id.clone())
            .collect(),
    })
}

/// Balanced accuracy of the best single threshold between the top scores of
/// answerable questions and of controls — the threshold-free abstention
/// number a system with no decline rule can still be read on. No hit at all
/// is the correct answer to a control, so it scores zero confidence rather
/// than being dropped.
fn separation(answerable: &[Option<f64>], controls: &[Option<f64>]) -> f64 {
    let pos: Vec<f64> = answerable.iter().map(|s| s.unwrap_or(0.0)).collect();
    let neg: Vec<f64> = controls.iter().map(|s| s.unwrap_or(0.0)).collect();
    let ratio = |hits: usize, total: usize| {
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    };
    let mut candidates: Vec<f64> = pos.iter().chain(neg.iter()).copied().collect();
    candidates.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    candidates.dedup();
    let mut best = 0.0f64;
    for &t in &candidates {
        let tpr = ratio(pos.iter().filter(|s| **s >= t).count(), pos.len());
        let tnr = ratio(neg.iter().filter(|s| **s < t).count(), neg.len());
        best = best.max((tpr + tnr) / 2.0);
    }
    best
}
