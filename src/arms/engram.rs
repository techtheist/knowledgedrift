//! The product under test: `engram_core::Engine` over an in-memory store,
//! driven exactly the way the daemon drives it — checked writes for
//! assistant-style notes, `replaces` edges for re-decisions, tombstoned
//! deletes for releases, auto-tune and the conflict sweep at every session
//! boundary, the calibrated verdict on every recall.

use std::collections::HashMap;

use engram_core::{
    Durability, EdgeType, Embedder, Engine, NewEdge, NewNode, Nli, NodeStatus, NodeType, Reranker,
    SearchFilter, Source, SqliteStore, TimeWindow, WriteOutcome,
};

use crate::protocol::{
    Authority, Capabilities, Hit, Inscribed, Memory, Recalled, Record, SuspectPair, Warning,
    Window, WriteMode, tokens,
};

pub struct EngramArm {
    engine: Engine,
    /// script key -> node id
    ids: HashMap<String, String>,
    /// node id -> script key
    keys: HashMap<String, String>,
}

impl EngramArm {
    pub fn build(
        embedder: Box<dyn Embedder>,
        reranker: Option<Box<dyn Reranker>>,
        nli: Option<Box<dyn Nli>>,
    ) -> anyhow::Result<Self> {
        let store = SqliteStore::open_in_memory()?;
        {
            use engram_core::Store as _;
            let dim = embedder.embed_one("dimension probe")?.len();
            store.reset_vectors(dim)?;
        }
        let mut engine = Engine::new(store, embedder);
        if let Some(r) = reranker {
            engine.set_reranker(r);
        }
        if let Some(n) = nli {
            engine.set_nli(n);
        }
        Ok(Self {
            engine,
            ids: HashMap::new(),
            keys: HashMap::new(),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The node id a script key was written as, for diagnostics that need
    /// to reach past the protocol (diagnostics).
    pub fn id_of(&self, key: &str) -> Option<&str> {
        self.ids.get(key).map(String::as_str)
    }

    /// Re-point any policy value before the script runs — the ablation
    /// handle: one built engine, the knob under test switched, everything
    /// else identical.
    pub fn tune(
        &self,
        f: impl FnOnce(&mut engram_core::config::PolicyConfig),
    ) -> anyhow::Result<()> {
        let mut cfg = self.engine.graph_config();
        f(&mut cfg.policy);
        self.engine.set_graph_config(&cfg)?;
        Ok(())
    }

    fn new_node(&self, r: &Record) -> anyhow::Result<NewNode> {
        Ok(NewNode {
            node_type: NodeType::parse(&r.kind)?,
            title: r.title.clone(),
            body: Some(r.body.clone()),
            created_at: r.created_at,
            durability: Durability::Stable,
            source: Source::Claude,
            session_id: Some("knowledgedrift".to_string()),
            status: r.open.then_some(NodeStatus::Open),
            code_refs: r.code_refs.clone(),
            tags: vec![],
            version: None,
            props: None,
            fields: None,
        })
    }

    fn remember(&mut self, key: &str, id: &str) {
        self.ids.insert(key.to_string(), id.to_string());
        self.keys.insert(id.to_string(), key.to_string());
    }

    fn id(&self, key: &str) -> anyhow::Result<&str> {
        self.ids
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| anyhow::anyhow!("unknown key {key}"))
    }

    fn pair(&self, a: &str, b: &str, hint: Option<String>) -> Option<SuspectPair> {
        Some(SuspectPair {
            a: self.keys.get(a)?.clone(),
            b: self.keys.get(b)?.clone(),
            hint,
        })
    }
}

impl Memory for EngramArm {
    fn name(&self) -> String {
        "engram".to_string()
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            link: true,
            history: true,
            trace: true,
            suspects: true,
            temporal: true,
            verdict: self.engine.has_reranker(),
            write_check: true,
            // Trust v2's ladder: confirm (the assistant's deliberate
            // "still true"), approve (the user), pin (a constant override).
            // Retrieval is deliberately NOT a rung — "retrieval is
            // observability, not evidence": a delivered note's last_seen
            // is stamped and nothing reads it for trust.
            endorse_retrieval: false,
            endorse_assistant: true,
            endorse_user: true,
            endorse_supervisor: true,
        }
    }

    fn inscribe(&mut self, record: &Record, mode: WriteMode) -> anyhow::Result<Inscribed> {
        let n = self.new_node(record)?;
        match mode {
            WriteMode::Import => {
                let node = self.engine.add_node(n)?;
                self.remember(&record.key, &node.id);
                Ok(Inscribed::default())
            }
            WriteMode::Write => match self.engine.add_node_checked(n)? {
                WriteOutcome::Created {
                    node,
                    warnings,
                    suspects,
                    ..
                } => {
                    self.remember(&record.key, &node.id);
                    Ok(Inscribed {
                        matched: None,
                        nli_label: None,
                        warnings: warnings
                            .into_iter()
                            .map(|w| Warning {
                                reason: w.reason,
                                key: self.keys.get(&w.id).cloned(),
                            })
                            .collect(),
                        suspects: suspects
                            .iter()
                            .filter_map(|s| self.pair(&s.a.id, &s.b.id, s.nli_label.clone()))
                            .collect(),
                    })
                }
                WriteOutcome::Matched {
                    node, nli_label, ..
                } => Ok(Inscribed {
                    matched: self.keys.get(&node.id).cloned(),
                    nli_label,
                    warnings: vec![],
                    suspects: vec![],
                }),
            },
        }
    }

    fn link(&mut self, from: &str, to: &str, verb: &str) -> anyhow::Result<bool> {
        let (from_id, to_id) = (self.id(from)?.to_string(), self.id(to)?.to_string());
        self.engine.add_edge(NewEdge {
            edge_type: EdgeType::parse(verb)?,
            from_id,
            to_id,
            source: Source::Claude,
            note: None,
            confidence: None,
            strength: None,
            status: None,
        })?;
        Ok(true)
    }

    fn supersede(&mut self, old: &str, new: &Record) -> anyhow::Result<()> {
        let old_id = self.id(old)?.to_string();
        let node = self.engine.add_node(self.new_node(new)?)?;
        self.remember(&new.key, &node.id);
        self.engine.add_edge(NewEdge {
            edge_type: EdgeType::parse("replaces")?,
            from_id: node.id,
            to_id: old_id,
            source: Source::Claude,
            note: None,
            confidence: None,
            strength: None,
            status: None,
        })?;
        Ok(())
    }

    fn release(&mut self, key: &str, reason: &str) -> anyhow::Result<()> {
        let id = self.id(key)?.to_string();
        // keep_text = false: the purge shape of the pane's delete, the one
        // ForgetEval's release maps to. The marker still names its victim.
        self.engine
            .delete_node_with_tombstone(&id, Some(reason), false)?;
        Ok(())
    }

    fn purge(&mut self, key: &str) -> anyhow::Result<()> {
        let id = self.id(key)?.to_string();
        self.engine.delete_node(&id)?;
        Ok(())
    }

    fn endorse(&mut self, key: &str, by: Authority) -> anyhow::Result<bool> {
        let id = self.id(key)?.to_string();
        match by {
            // Exposure never validates: the engine stamps last_seen on
            // delivery and reads it for nothing. Declared unsupported.
            Authority::Retrieval => Ok(false),
            // The pane's "Confirm still true" / an update_node patch:
            // stamps confirmed_at, trust restarts on the confirmed curve.
            Authority::Assistant => {
                self.engine.reconfirm(&id)?;
                Ok(true)
            }
            // The pane's Approve: trust restarts at its ceiling.
            Authority::User => {
                self.engine.approve(&id)?;
                Ok(true)
            }
            // The pane's Pin: a constant trust of 1.0 that no evidence moves.
            Authority::Supervisor => {
                self.engine.set_trust_override(&id, Some(1.0))?;
                Ok(true)
            }
        }
    }

    fn settle(&mut self) -> anyhow::Result<Option<String>> {
        // Auto-tune is damped; a deployment runs it at every session
        // boundary, so the measured stack is the converged line.
        let mut note = None;
        for _ in 0..16 {
            match self.engine.auto_tune()? {
                Some(n) => note = Some(n),
                None => break,
            }
        }
        let queued = self.engine.scan_conflicts()?;
        let line = self.engine.graph_config().policy.weak_evidence_top;
        Ok(Some(format!(
            "{}; weak line {line:.3}; sweep queued {queued}",
            note.unwrap_or_else(|| "auto-tune left the defaults".into())
        )))
    }

    fn recall(&self, query: &str, k: usize, window: Option<Window>) -> anyhow::Result<Recalled> {
        let hits = match window {
            None => self.engine.search(query, &[], k)?,
            Some(w) => {
                let filter = SearchFilter {
                    window: TimeWindow {
                        after: w.after,
                        before: w.before,
                    },
                    ..SearchFilter::default()
                };
                self.engine.search_filtered(query, &[], k, &filter)?
            }
        };
        let declined = matches!(
            self.engine.search_confidence(&hits),
            Some("weak") | Some("none")
        );
        let hits = hits
            .iter()
            .map(|h| Hit {
                key: self.keys.get(&h.id).cloned(),
                text: format!(
                    "## {}\n{}",
                    h.title,
                    h.snippet.replace(['\u{e000}', '\u{e001}'], "")
                ),
                score: Some(h.score),
                tombstone: h.tombstone,
                created_at: Some(h.created_at),
                neighbors: h
                    .neighbors
                    .iter()
                    .filter_map(|n| self.keys.get(&n.id).cloned())
                    .collect(),
            })
            .collect();
        Ok(Recalled {
            hits,
            declined,
            dump: false,
        })
    }

    /// The file channel: the same code-ref match the file-read hook
    /// serves — every live note whose refs cover the path, trust first,
    /// the note delivered whole (no query, so no snippet window).
    fn recall_path(&self, path: &str, k: usize) -> anyhow::Result<Recalled> {
        let cfg = self.engine.graph_config();
        let tombstones: Vec<String> = cfg
            .tombstone_types()
            .iter()
            .map(|t| t.to_string())
            .collect();
        let hits = self
            .engine
            .match_code_refs(path, k)?
            .into_iter()
            .map(|n| Hit {
                key: self.keys.get(&n.id).cloned(),
                text: format!("## {}\n{}", n.title, n.body.as_deref().unwrap_or("")),
                score: Some(n.trust),
                tombstone: tombstones.iter().any(|t| t == n.node_type.as_str()),
                created_at: Some(n.created_at),
                neighbors: Vec::new(),
            })
            .collect();
        Ok(Recalled {
            hits,
            declined: false,
            dump: false,
        })
    }

    fn suspects(&mut self) -> anyhow::Result<Option<Vec<SuspectPair>>> {
        self.engine.scan_conflicts()?;
        let pairs = self
            .engine
            .suspects()?
            .iter()
            .filter_map(|s| self.pair(&s.a.id, &s.b.id, s.nli_label.clone()))
            .collect();
        Ok(Some(pairs))
    }

    fn lineage(&self, key: &str) -> anyhow::Result<Option<Vec<String>>> {
        let mut out = Vec::new();
        let mut cur = self.id(key)?.to_string();
        loop {
            let next = self
                .engine
                .edges_out(&cur)?
                .into_iter()
                .find(|e| e.edge_type.as_str() == "replaces")
                .map(|e| e.to_id);
            match next {
                Some(id) if !out.contains(&id) => {
                    out.push(id.clone());
                    cur = id;
                }
                _ => break,
            }
        }
        Ok(Some(
            out.iter()
                .filter_map(|id| self.keys.get(id).cloned())
                .collect(),
        ))
    }

    fn standing_tokens(&self) -> usize {
        self.engine
            .brief(self.engine.brief_chars(None))
            .map(|b| tokens(&b))
            .unwrap_or(0)
    }
}
