//! In-process arms: the product, and the baselines every claim is measured
//! against. External systems do not live here — they replay the exported
//! script and are graded from their transcript.

pub mod engram;
pub mod flat;
pub mod tfidf;

pub use engram::EngramArm;
pub use flat::{FlatArm, Mode};
pub use tfidf::TfidfArm;

use engram_core::{Embedder, Nli, Reranker};

/// The embedder every in-process arm shares, and the name of whatever
/// actually loaded — real under `--features fastembed`, the deterministic
/// fake otherwise.
pub fn embedder() -> (Box<dyn Embedder>, String) {
    #[cfg(feature = "fastembed")]
    {
        match engram_core::FastEmbedder::new() {
            Ok(e) => {
                let name = e.name().to_string();
                return (Box::new(e), name);
            }
            Err(err) => eprintln!("! real embedder unavailable ({err}); falling back to fake"),
        }
    }
    let e = engram_core::FakeEmbedder::default();
    let name = format!("{} (fake)", e.name());
    (Box::new(e), name)
}

/// The reference system's precision layer. If it is missing the engram arm
/// is not the shipped stack, so the report names it either way.
pub fn reranker() -> (Option<Box<dyn Reranker>>, String) {
    #[cfg(feature = "fastembed")]
    {
        match engram_core::FastReranker::new() {
            Ok(r) => return (Some(Box::new(r)), "jina-reranker-v1-turbo-en".to_string()),
            Err(err) => eprintln!("! reranker unavailable ({err}); engram arm runs hybrid-only"),
        }
    }
    (None, "none".to_string())
}

/// The directory `--nli-dir` names, when a run swaps the reference system's
/// judge for another export it can load (a three-label NLI or a Laya
/// directory). Unset, the shipped default loads.
pub static NLI_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// The logic layer, and the name of whatever actually loaded — real under
/// `--features fastembed`, the deterministic fake otherwise.
pub fn nli() -> (Box<dyn Nli>, String) {
    #[cfg(feature = "fastembed")]
    {
        if let Some(dir) = NLI_DIR.get() {
            let name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.display().to_string());
            match engram_core::nli::load_dir(dir) {
                Ok(n) => return (n, name),
                Err(err) => eprintln!(
                    "! NLI at {} unavailable ({err}); falling back to fake",
                    dir.display()
                ),
            }
        } else {
            match engram_core::FastNli::new() {
                Ok(n) => return (Box::new(n), engram_core::nli::NLI_MODEL_NAME.to_string()),
                Err(err) => eprintln!("! real NLI unavailable ({err}); falling back to fake"),
            }
        }
    }
    (Box::new(engram_core::FakeNli), "fake".to_string())
}

pub use crate::ARM_NAMES;
