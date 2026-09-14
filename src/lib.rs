//! KnowledgeDrift — an offline, judge-free benchmark for AI memory in
//! software development.
//!
//! One seeded world of invented project knowledge is poured into a memory
//! system through a ten-operation protocol and then questioned, re-decided,
//! contradicted, retired and asked again. Every probe is a task with a
//! pass/fail rule the script fixed before the question was asked; the
//! headline is the mean success over every task posed,
//! beside a macro-averaged composite and an efficiency-multiplied score.
//!
//! * [`protocol`] — what a memory system has to implement.
//! * [`world`] — how the world and its probes are generated.
//! * [`script`] — the JSON both halves exchange.
//! * [`runner`] — replay a script, record a transcript.
//! * [`grade`] — the rules.
//! * [`corpus`] — the vendored corpus generator the worlds are built from.
//! * [`arms`] — the reference system and the in-process baselines
//!   (`--features arms`; `--features fastembed` for real models).

#[cfg(feature = "arms")]
pub mod arms;
pub mod corpus;
pub mod grade;
pub mod protocol;
pub mod report;
pub mod runner;
pub mod script;
pub mod world;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Every in-process arm by name, in the order the tables print them.
pub const ARM_NAMES: [&str; 6] = ["engram", "rag", "grep", "curated", "whole", "chance"];

#[cfg(all(test, feature = "arms"))]
mod tests {
    use super::*;
    use crate::arms::{EngramArm, FlatArm, Mode};
    use crate::protocol::Memory;
    use engram_core::{FakeEmbedder, FakeNli};

    fn world() -> script::Script {
        world::build(&world::WorldConfig {
            size: 30,
            seed: 11,
            ..world::WorldConfig::default()
        })
    }

    fn engram() -> EngramArm {
        EngramArm::build(
            Box::new(FakeEmbedder::default()),
            None,
            Some(Box::new(FakeNli)),
        )
        .unwrap()
    }

    fn col(g: &grade::Graded, fam: &str, name: &str) -> f64 {
        g.families
            .iter()
            .find(|f| f.family == fam)
            .and_then(|f| f.columns.get(name).copied())
            .unwrap_or_else(|| panic!("no column {fam}.{name}"))
    }

    fn rate(g: &grade::Graded, fam: &str) -> Option<f64> {
        g.families
            .iter()
            .find(|f| f.family == fam)
            .and_then(|f| f.pass_rate)
    }

    #[test]
    fn the_product_runs_every_family_and_the_mechanisms_hold() {
        let s = world();
        let mut arm = engram();
        let t = runner::run(&s, &mut arm).unwrap();
        let g = grade::grade(&s, &t).unwrap();
        for f in &g.families {
            assert!(f.na.is_none(), "{} is n/a for engram: {:?}", f.family, f.na);
        }
        // The lexical path works under the fake embedder, so titles are
        // findable at all — a harness check, not a quality claim.
        assert!(col(&g, "retrieval", "lexical_r@5") > 0.5);
        // Supersession is structural: the retired side never comes back and
        // the history walk always reaches it.
        assert_eq!(col(&g, "currency", "pollution"), 0.0);
        assert_eq!(col(&g, "currency", "lineage"), 1.0);
        // Deletion: released and purged notes are gone as live knowledge;
        // a release leaves a findable trace.
        assert_eq!(col(&g, "deletion", "released_gone"), 1.0);
        assert_eq!(col(&g, "deletion", "purged_gone"), 1.0);
        assert!(col(&g, "deletion", "trace") > 0.0);
        // A purged note written again warns nobody: there is nothing left
        // to warn about.
        assert_eq!(col(&g, "deletion", "purged_rewrite_warned"), 0.0);
        assert!(g.tasks > 100);
        assert!((0.0..=1.0).contains(&g.success));
        assert!(g.multiplier >= 0.1 && g.multiplier <= 10.0);
        assert!(g.standing_tokens_hint() > 0);
    }

    #[test]
    fn flat_stores_score_na_where_they_have_no_mechanism() {
        let s = world();
        let mut grep = FlatArm::new(Mode::Grep, None);
        let t = runner::run(&s, &mut grep).unwrap();
        let g = grade::grade(&s, &t).unwrap();
        assert!(rate(&g, "contradiction").is_none());
        assert!(rate(&g, "drift").is_none());
        assert!(rate(&g, "temporal").is_some(), "grep can filter by date");
        assert!(col(&g, "currency", "lineage_na") == 1.0);
        // A flat store's release IS a purge: gone, no trace, no warning.
        assert_eq!(col(&g, "deletion", "released_gone"), 1.0);
        assert_eq!(col(&g, "deletion", "resurrection_warned"), 0.0);
        assert!(!g.capabilities.trace);

        let mut whole = FlatArm::new(Mode::Whole, None);
        let t = runner::run(&s, &mut whole).unwrap();
        let g = grade::grade(&s, &t).unwrap();
        assert!(rate(&g, "temporal").is_none(), "a file has no clock");
        // Every system is posed the same tasks; a dump attempts fewer and
        // the headline charges it for every one it cannot.
        assert!(g.attempted < g.tasks);
        assert!(g.success < g.attempted_success);
        assert_eq!(
            col(&g, "retrieval", "r@5"),
            1.0,
            "a dump holds every answer"
        );
        // …and a dump holds the stale sibling beside it, so every polluted
        // question is ambiguous.
        assert_eq!(col(&g, "retrieval", "stale_above"), 1.0);
        assert!(g.signal_share < 0.1, "present is not readable");
        assert!(g.multiplier < 1.0, "a dump pays for its noise");
        assert!(g.score < g.composite * 100.0);
    }

    #[test]
    fn a_transcript_round_trips_through_json_and_grades_the_same() {
        let s = world();
        let mut arm = FlatArm::new(Mode::Rag, Some(Box::new(FakeEmbedder::default())));
        let t = runner::run(&s, &mut arm).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        let back: script::Transcript = serde_json::from_str(&json).unwrap();
        // Scores may differ by an ulp after a JSON parse (serde_json without
        // `float_roundtrip`); grading must not care.
        assert_eq!(back.replies.len(), t.replies.len());
        let sjson = serde_json::to_string(&s).unwrap();
        let s2: script::Script = serde_json::from_str(&sjson).unwrap();
        assert_eq!(s2.digest(), s.digest());
        let a = grade::grade(&s, &t).unwrap();
        let b = grade::grade(&s2, &back).unwrap();
        assert_eq!(a.success, b.success);
        assert_eq!(a.failed, b.failed);
        for (fa, fb) in a.families.iter().zip(&b.families) {
            assert_eq!(fa.pass_rate, fb.pass_rate, "{}", fa.family);
            assert_eq!(fa.columns, fb.columns, "{}", fa.family);
        }
    }

    #[test]
    fn a_transcript_for_another_world_is_refused() {
        let s = world();
        let other = world::build(&world::WorldConfig {
            size: 30,
            seed: 12,
            ..world::WorldConfig::default()
        });
        let mut arm = FlatArm::new(Mode::Chance, None);
        let t = runner::run(&other, &mut arm).unwrap();
        assert!(grade::grade(&s, &t).is_err());
    }

    impl grade::Graded {
        fn standing_tokens_hint(&self) -> usize {
            self.cost.get("standing_tokens").copied().unwrap_or(0.0) as usize
        }
    }

    #[test]
    fn the_engram_arm_reports_the_brief_as_standing_cost() {
        let s = world();
        let mut arm = engram();
        let _ = runner::run(&s, &mut arm).unwrap();
        assert!(arm.standing_tokens() > 0);
    }
}
