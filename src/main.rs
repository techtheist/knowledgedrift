use std::process::ExitCode;

use knowledgedrift::ARM_NAMES;
use knowledgedrift::grade::grade;
use knowledgedrift::report::{print_arm, print_summary};
use knowledgedrift::script::{PollutionShape, Script, Transcript};
use knowledgedrift::world::{WorldConfig, build};

const USAGE: &str = "\
knowledgedrift — an offline, judge-free benchmark for AI memory in software development

USAGE:
    knowledgedrift [OPTIONS]

OPTIONS:
    --sizes 500,1500      tested facts per world; every fact is questioned
                          and the rest of the world is the noise. The official
                          ladder is 500 and 1500              [default: 500,1500]
    --seed N              world seed                          [default: 1]
    --k N                 results a recall may return         [default: 10]
    --pollution R         share of subjects imported with a stale sibling
                          nobody superseded                   [default: 0.10]
    --pollution-shape S   how the stale sibling is shaped: stale (older,
                          hinted body — the default), twin (older, the
                          truth's own body: only value and clock differ),
                          late (hinted, dated AFTER the truth — a migration
                          re-import)                          [default: stale]
    --chain-len N         generations per re-decided subject  [default: 3]
    --v2                  build the v2 edition of the world: shared-vocabulary
                          subjects, a fourth crossed phrasing, natural-null
                          controls, reworded contradiction shapes, answer-bearing
                          probes, and the additive score (families + signal +
                          tokens)
    --authority           add the authority family: near-identical twins
                          endorsed on different rungs (retrieval use, the
                          assistant's confirm, the owner's approve, a
                          supervisor's pin), asked which comes first. Off
                          by default; the v1 worlds do not carry it
    --arms a,b,...        in-process arms to run (needs --features arms or
                          fastembed) [default: engram,rag,grep,curated,whole,chance]
    --no-rerank           drop the cross-encoder from the engram arm (a
                          diagnostic, not a product option)
    --nli-dir DIR         load the engram arm's contradiction judge from DIR
                          (a three-label NLI export or a Laya directory)
                          instead of the default tasksource model; the
                          receipt names the directory
    --json PATH           write the receipt
    --export DIR          write each world's script as JSON (for external
                          adapters) and exit; the v1 worlds are already
                          under worlds/v1/ and regenerate byte-identical
    --grade TRANSCRIPT    grade an external transcript; needs --script
    --script SCRIPT       the script the transcript answers
    --sample              print a handful of ops and probes and exit

Grading a transcript and exporting worlds need no models and no feature.
The in-process arms need --features fastembed for real models; with plain
--features arms every model is a deterministic fake: the harness runs and
the lexical path is exercised, but no semantic number is worth quoting, and
the receipt says so.
";

fn main() -> ExitCode {
    match cli() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("knowledgedrift: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "arms")]
#[derive(serde::Serialize)]
struct SizeReceipt {
    spec: knowledgedrift::script::WorldSpec,
    digest: String,
    probes: usize,
    ops: usize,
    arms: Vec<knowledgedrift::grade::Graded>,
}

#[cfg(feature = "arms")]
#[derive(serde::Serialize)]
struct Receipt {
    generator: String,
    /// Diagnostics the run was started with, so a receipt says which stack
    /// it measured.
    flags: Vec<String>,
    embedder: String,
    reranker: String,
    nli: String,
    embeddings_are_fake: bool,
    sizes: Vec<SizeReceipt>,
}

fn cli() -> anyhow::Result<()> {
    let mut cfg = WorldConfig::default();
    let mut sizes: Vec<usize> = vec![500, 1500];
    let mut arms: Vec<String> = ARM_NAMES.iter().map(|s| s.to_string()).collect();
    let mut no_rerank = false;
    let mut json_out: Option<String> = None;
    let mut export: Option<String> = None;
    let mut grade_path: Option<String> = None;
    let mut script_path: Option<String> = None;
    let mut sample = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || -> anyhow::Result<String> {
            args.next()
                .ok_or_else(|| anyhow::anyhow!("{arg} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "--sizes" => {
                sizes = value()?
                    .split(',')
                    .map(|s| s.trim().parse::<usize>())
                    .collect::<Result<_, _>>()?;
            }
            "--seed" => cfg.seed = value()?.parse()?,
            "--k" => cfg.k = value()?.parse()?,
            "--pollution" => cfg.pollution = value()?.parse()?,
            "--pollution-shape" => {
                let v = value()?;
                cfg.shape = PollutionShape::parse(&v).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown pollution shape {v}; shapes are {}",
                        PollutionShape::NAMES.join(", ")
                    )
                })?;
            }
            "--chain-len" => cfg.chain_len = value()?.parse()?,
            "--authority" => cfg.authority = true,
            "--v2" => cfg.edition = 2,
            "--arms" => arms = value()?.split(',').map(|s| s.trim().to_string()).collect(),
            "--no-rerank" => no_rerank = true,
            #[cfg(feature = "arms")]
            "--nli-dir" => {
                let _ = knowledgedrift::arms::NLI_DIR.set(value()?.into());
            }
            "--json" => json_out = Some(value()?),
            "--export" => export = Some(value()?),
            "--grade" => grade_path = Some(value()?),
            "--script" => script_path = Some(value()?),
            "--sample" => sample = true,
            other => anyhow::bail!("unknown option {other} (try --help)"),
        }
    }
    anyhow::ensure!(!sizes.is_empty(), "--sizes needs at least one size");
    anyhow::ensure!(
        (0.0..=1.0).contains(&cfg.pollution),
        "--pollution is a share in 0..1"
    );
    anyhow::ensure!(
        cfg.chain_len >= 2,
        "--chain-len needs something to supersede"
    );
    for a in &arms {
        anyhow::ensure!(
            ARM_NAMES.contains(&a.as_str()),
            "unknown arm {a}; in-process arms are {}",
            ARM_NAMES.join(", ")
        );
    }

    if let Some(path) = grade_path {
        let script_path = script_path.ok_or_else(|| anyhow::anyhow!("--grade needs --script"))?;
        let script: Script = serde_json::from_str(&std::fs::read_to_string(&script_path)?)?;
        let transcript: Transcript = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        let g = grade(&script, &transcript)?;
        print_summary(
            script.spec.size,
            script.spec.notes,
            std::slice::from_ref(&g),
        );
        print_arm(&g);
        if let Some(out) = json_out {
            std::fs::write(&out, serde_json::to_string_pretty(&g)?)?;
            println!("\nwrote {out}");
        }
        return Ok(());
    }

    if sample {
        cfg.size = sizes[0];
        let s = build(&cfg);
        println!(
            "world {} — {} ops, {} probes, digest {}",
            cfg.size,
            s.ops.len(),
            s.probes.len(),
            s.digest()
        );
        for op in s.ops.iter().take(6) {
            println!("{}", serde_json::to_string(op)?);
        }
        println!("…");
        for p in s.probes.iter().step_by(s.probes.len().max(1) / 12).take(12) {
            println!("{}", serde_json::to_string(p)?);
        }
        return Ok(());
    }

    if let Some(dir) = export {
        std::fs::create_dir_all(&dir)?;
        for &size in &sizes {
            cfg.size = size;
            let s = build(&cfg);
            let shape = match cfg.shape {
                PollutionShape::Stale => String::new(),
                other => format!("-{}", other.name()),
            };
            let authority = if cfg.authority { "-authority" } else { "" };
            let edition = if cfg.edition >= 2 { "-v2" } else { "" };
            let path = format!(
                "{dir}/knowledgedrift-{size}-seed{}{shape}{authority}{edition}.json",
                cfg.seed
            );
            std::fs::write(&path, serde_json::to_string_pretty(&s)?)?;
            println!(
                "wrote {path} ({} ops, {} probes, digest {})",
                s.ops.len(),
                s.probes.len(),
                s.digest()
            );
        }
        return Ok(());
    }

    ladder(cfg, &sizes, &arms, no_rerank, json_out)
}

#[cfg(not(feature = "arms"))]
fn ladder(
    _cfg: WorldConfig,
    _sizes: &[usize],
    _arms: &[String],
    _no_rerank: bool,
    _json_out: Option<String>,
) -> anyhow::Result<()> {
    anyhow::bail!(
        "this build has no in-process arms — rebuild with `--features fastembed` \
         (or `--features arms` for the fake models); --grade, --export and --sample \
         work as is"
    )
}

#[cfg(feature = "arms")]
fn ladder(
    mut cfg: WorldConfig,
    sizes: &[usize],
    arms: &[String],
    no_rerank: bool,
    json_out: Option<String>,
) -> anyhow::Result<()> {
    use knowledgedrift::arms::{EngramArm, FlatArm, Mode, TfidfArm, embedder, nli, reranker};
    use knowledgedrift::protocol::Memory;
    use knowledgedrift::{VERSION, runner};

    let (_, embedder_name) = embedder();
    let (_, reranker_name) = if no_rerank {
        (None, "disabled (--no-rerank)".to_string())
    } else {
        reranker()
    };
    let (_, nli_name) = nli();
    let fake = embedder_name.contains("(fake)");
    if fake {
        eprintln!("! fake embedder: the lexical path is measured, the semantic numbers are noise");
    }
    println!(
        "knowledgedrift {VERSION} — embedder {embedder_name}, reranker {reranker_name}, nli {nli_name}, seed {}, pollution {:.0}% ({}){}",
        cfg.seed,
        cfg.pollution * 100.0,
        cfg.shape.name(),
        if cfg.authority { ", authority" } else { "" }
    );
    if cfg.edition >= 2 {
        println!("edition 2: additive score = families (800) + signal (100) + tokens (100)");
    }

    let mut flags = Vec::new();
    if no_rerank {
        flags.push("--no-rerank".to_string());
    }
    if cfg.authority {
        flags.push("--authority".to_string());
    }
    if cfg.edition >= 2 {
        flags.push("--v2".to_string());
    }
    let mut receipt = Receipt {
        generator: format!("knowledgedrift {VERSION}"),
        flags,
        embedder: embedder_name,
        reranker: reranker_name,
        nli: nli_name,
        embeddings_are_fake: fake,
        sizes: Vec::new(),
    };

    for &size in sizes {
        cfg.size = size;
        let s = build(&cfg);
        eprintln!(
            "world {size}: {} notes, {} edges, {} ops, {} probes",
            s.spec.notes,
            s.spec.edges,
            s.ops.len(),
            s.probes.len()
        );
        let mut graded = Vec::new();
        for name in arms {
            let started = std::time::Instant::now();
            let mut arm: Box<dyn Memory> = match name.as_str() {
                "engram" => Box::new(EngramArm::build(
                    embedder().0,
                    if no_rerank { None } else { reranker().0 },
                    Some(nli().0),
                )?),
                "rag" => Box::new(FlatArm::new(Mode::Rag, Some(embedder().0))),
                "grep" => Box::new(FlatArm::new(Mode::Grep, None)),
                "curated" => Box::new(FlatArm::new(
                    Mode::Curated(knowledgedrift::arms::flat::DEFAULT_CURATED_BUDGET),
                    None,
                )),
                "whole" => Box::new(FlatArm::new(Mode::Whole, None)),
                "chance" => Box::new(FlatArm::new(Mode::Chance, None)),
                "tfidf" => Box::new(TfidfArm::default()),
                other => anyhow::bail!("unknown arm {other}"),
            };
            let t = runner::run(&s, arm.as_mut())?;
            let g = grade(&s, &t)?;
            eprintln!(
                "  {name}: {} tasks, success {:.3}, {:.0}s",
                g.tasks,
                g.success,
                started.elapsed().as_secs_f64()
            );
            if let Some(note) = &g.settle_note {
                eprintln!("    settle: {note}");
            }
            graded.push(g);
        }
        print_summary(size, s.spec.notes, &graded);
        for g in &graded {
            print_arm(g);
        }
        receipt.sizes.push(SizeReceipt {
            spec: s.spec.clone(),
            digest: s.digest(),
            probes: s.probes.len(),
            ops: s.ops.len(),
            arms: graded,
        });
    }

    if let Some(path) = json_out {
        std::fs::write(&path, serde_json::to_string_pretty(&receipt)?)?;
        println!("\nwrote {path}");
    }
    Ok(())
}
