//! Replay a script against a [`Memory`] and record the transcript.
//!
//! The runner is deliberately dumb: it does not know what a probe expects,
//! it times every operation, and it PANICS on an adapter error rather than
//! scoring it — an error that empties a recall is indistinguishable from a
//! silent graph, and the 0.8.7 window bench learned that the hard way.

use std::time::Instant;

use crate::protocol::Memory;
use crate::script::{Op, Reply, Script, Timing, Transcript};

pub fn run(script: &Script, mem: &mut dyn Memory) -> anyhow::Result<Transcript> {
    let mut replies = Vec::new();
    let mut timing = std::collections::BTreeMap::<String, Timing>::new();
    let mut settle_note = None;

    for op in &script.ops {
        let started = Instant::now();
        let kind: &'static str = match op {
            Op::Inscribe { id, record, mode } => {
                let result = mem
                    .inscribe(record, *mode)
                    .map_err(|e| anyhow::anyhow!("inscribe {}: {e}", record.key))?;
                if let Some(id) = id {
                    replies.push(Reply::Inscribe {
                        id: id.clone(),
                        result,
                    });
                }
                "inscribe"
            }
            Op::Link { from, to, verb } => {
                mem.link(from, to, verb)
                    .map_err(|e| anyhow::anyhow!("link {from} {verb} {to}: {e}"))?;
                "link"
            }
            Op::Supersede { old, new } => {
                mem.supersede(old, new)
                    .map_err(|e| anyhow::anyhow!("supersede {old} -> {}: {e}", new.key))?;
                "supersede"
            }
            Op::Release { key, reason } => {
                mem.release(key, reason)
                    .map_err(|e| anyhow::anyhow!("release {key}: {e}"))?;
                "release"
            }
            Op::Purge { key } => {
                mem.purge(key)
                    .map_err(|e| anyhow::anyhow!("purge {key}: {e}"))?;
                "purge"
            }
            Op::Endorse { key, by } => {
                mem.endorse(key, *by)
                    .map_err(|e| anyhow::anyhow!("endorse {key} by {}: {e}", by.as_str()))?;
                "endorse"
            }
            Op::Settle => {
                if let Some(note) = mem.settle()? {
                    settle_note = Some(note);
                }
                "settle"
            }
            Op::Recall {
                id,
                query,
                k,
                window,
            } => {
                let result = mem
                    .recall(query, *k, *window)
                    .map_err(|e| anyhow::anyhow!("recall {id} {query:?}: {e}"))?;
                replies.push(Reply::Recall {
                    id: id.clone(),
                    result,
                });
                "recall"
            }
            Op::Suspects { id } => {
                let pairs = mem.suspects()?;
                replies.push(Reply::Suspects {
                    id: id.clone(),
                    pairs,
                });
                "suspects"
            }
            Op::Lineage { id, key } => {
                let keys = mem.lineage(key)?;
                replies.push(Reply::Lineage {
                    id: id.clone(),
                    keys,
                });
                "lineage"
            }
        };
        let t = timing.entry(kind.to_string()).or_default();
        t.count += 1;
        t.total_ms += started.elapsed().as_secs_f64() * 1000.0;
    }

    Ok(Transcript {
        arm: mem.name(),
        capabilities: mem.capabilities(),
        standing_tokens: mem.standing_tokens(),
        script_digest: script.digest(),
        settle_note,
        replies,
        timing,
    })
}
