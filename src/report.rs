//! Terminal tables. The JSON receipt is the record; this is what a run
//! prints while it happens.

use crate::grade::Graded;

fn pct(x: f64) -> String {
    format!("{:>3.0}%", x * 100.0)
}

fn cols(g: &Graded, family: &str, names: &[&str]) -> String {
    let Some(f) = g.families.iter().find(|f| f.family == family) else {
        return String::new();
    };
    names
        .iter()
        .filter_map(|n| f.columns.get(*n).map(|v| format!("{n} {v:.2}")))
        .collect::<Vec<_>>()
        .join("  ")
}

/// One arm, every family, the columns worth reading at a glance.
pub fn print_arm(g: &Graded) {
    println!(
        "  {:<8} success {} = {} passed of {} posed (attempted {}/{}, {} of those passed)   composite {:.3}   S {:.2} ×{:.1}   score {:.0}   standing {} tok   {} tok/query",
        g.arm,
        pct(g.success),
        g.passed,
        g.tasks,
        g.attempted,
        g.tasks,
        pct(g.attempted_success),
        g.composite,
        g.signal_share,
        g.multiplier,
        g.score,
        g.cost.get("standing_tokens").copied().unwrap_or(0.0) as usize,
        g.cost.get("tokens_per_query").copied().unwrap_or(0.0) as usize,
    );
    if g.edition >= 2 {
        println!(
            "  {:<8} v2 score {:.0} = families {:.0} + signal {:.0} + tokens {:.0} (billed {:.0} tok/query)",
            "", g.score, g.family_points, g.signal_score, g.token_score, g.tokens_billed
        );
    }
    for flag in &g.flags {
        println!("  {:<8} ! {flag}", "");
    }
    let detail: &[(&str, &[&str])] = &[
        (
            "retrieval",
            &[
                "r@1",
                "r@5",
                "lexical_r@5",
                "paraphrase_r@5",
                "oblique_r@5",
                "crossed_r@5",
                "stale_above",
                "hedge",
                "noise",
            ],
        ),
        (
            "abstention",
            &[
                "fp",
                "phantom_fp",
                "natural_fp",
                "answered",
                "declined",
                "separation",
            ],
        ),
        (
            "currency",
            &["head_r@1", "head_r@5", "pollution", "lineage"],
        ),
        (
            "contradiction",
            &[
                "t1_recall",
                "t2_recall",
                "t3_recall",
                "t2_false_alarm",
                "t3_false_alarm",
                "queued",
                "flagged",
                "absorbed",
            ],
        ),
        ("drift", &["noticed", "flagged"]),
        (
            "deletion",
            &[
                "released_gone",
                "purged_gone",
                "trace",
                "marker_leak",
                "resurrection_warned",
                "purged_rewrite_warned",
            ],
        ),
        (
            "rationale",
            &["direct_r@5", "assisted_r@5", "structure_only"],
        ),
        ("temporal", &["in_window_r@5", "leak"]),
        (
            "authority",
            &[
                "l1_autonomous",
                "l2_governed",
                "l3_three_hands",
                "l4_supervised",
                "winner_top",
                "order_exact",
                "resurrected",
                "na_share",
            ],
        ),
    ];
    for (fam, names) in detail {
        let Some(f) = g.families.iter().find(|f| f.family == *fam) else {
            continue;
        };
        if f.posed == 0 {
            continue; // this world does not pose the family
        }
        match (&f.na, f.pass_rate) {
            (Some(why), _) => println!("    {fam:<14} {:>4}  n/a — {why}", "-"),
            (None, Some(rate)) => println!(
                "    {fam:<14} {:>4}  {}  {}",
                f.tasks,
                pct(rate),
                cols(g, fam, names)
            ),
            (None, None) => {}
        }
    }
}

/// The cross-arm table for one size.
pub fn print_summary(size: usize, notes: usize, arms: &[Graded]) {
    let v2 = arms.iter().any(|g| g.edition >= 2);
    println!(
        "\n== {size} tested facts, {notes} notes{} ==",
        if v2 { ", v2" } else { "" }
    );
    println!(
        "  {:<8} {:>6} {:>9} {:>6} {:>8} {:>9} {:>9} {:>5} {:>5} {:>7} {:>9} {:>8}",
        "arm",
        "posed",
        "attempted",
        "passed",
        "success",
        "of att.",
        "composite",
        "S",
        "mult",
        "score",
        "standing",
        "tok/q"
    );
    for g in arms {
        println!(
            "  {:<8} {:>6} {:>9} {:>6} {:>8} {:>9} {:>9.3} {:>5.2} {:>5.1} {:>7.0} {:>9} {:>8}",
            g.arm,
            g.tasks,
            g.attempted,
            g.passed,
            pct(g.success),
            pct(g.attempted_success),
            g.composite,
            g.signal_share,
            g.multiplier,
            g.score,
            g.cost.get("standing_tokens").copied().unwrap_or(0.0) as usize,
            g.cost.get("tokens_per_query").copied().unwrap_or(0.0) as usize,
        );
    }
    if v2 {
        println!(
            "  {:<8} {:>8} {:>8} {:>7} {:>7} {:>9}",
            "arm", "families", "signal", "tokens", "score", "billed"
        );
        for g in arms {
            println!(
                "  {:<8} {:>8.0} {:>8.0} {:>7.0} {:>7.0} {:>9.0}",
                g.arm, g.family_points, g.signal_score, g.token_score, g.score, g.tokens_billed
            );
        }
    }
}
