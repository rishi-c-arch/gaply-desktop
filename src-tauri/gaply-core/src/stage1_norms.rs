//! Absolute human-academic perplexity norms for the Stage-1 real-LM signal —
//! the replacement for the convicted self-referential baseline (H2b).
//!
//! PROVISIONAL (Phase 1): seeded from a small discipline-spanning human sample
//! (see `calibration/human_academic/`). The measurement showed human academic
//! perplexity has a WIDE range that OVERLAPS AI-written text — so this norm
//! drives a SOFT, down-weighted signal, NEVER a standalone gate. Phase 3
//! replaces it with held-out-evaluated, non-native-inclusive norms.
//!
//! The table is bundled at compile time (`include_str!`) and evaluated in pure
//! Rust — no runtime file dependency, no network.

use std::collections::HashMap;

use serde::Deserialize;

const NORMS_JSON: &str = include_str!("../calibration/stage1_norms.json");

/// The human-academic perplexity distribution for one model.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PerplexityNorm {
    pub n: usize,
    pub median: f64,
    pub mean: f64,
    pub p10: f64,
    pub p90: f64,
    pub min: f64,
    pub max: f64,
}

/// Per-model norms + provenance note.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ModelNorm {
    pub human_academic_perplexity: PerplexityNorm,
    /// Provenance + the honest provisional caveat (surfaced in the report).
    pub notes: String,
}

/// The bundled norms table.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Stage1Norms {
    pub version: u32,
    /// True until Phase-3 held-out evaluation hardens the numbers. While true,
    /// the report labels the perplexity signal "preliminary" and the 0-100
    /// score stays absent (honesty gate).
    pub provisional: bool,
    models: HashMap<String, ModelNorm>,
}

impl Stage1Norms {
    /// Load the compile-time-bundled norms. Never fails at runtime (the JSON is
    /// validated by this crate's tests).
    pub fn bundled() -> Self {
        serde_json::from_str(NORMS_JSON).expect("bundled stage1_norms.json must parse")
    }

    /// Look up the norms for a model id (e.g. "qwen2.5-0.5b-instruct-q4_k_m").
    pub fn for_model(&self, model_id: &str) -> Option<&ModelNorm> {
        self.models.get(model_id)
    }
}

/// A document's real-LM perplexity placed against the human norm — a SOFT,
/// weighted signal, never a verdict (the human range overlaps AI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerplexitySignal {
    /// Below the human 10th percentile — unusually predictable (AI-leaning).
    UnusuallyPredictable,
    /// Between p10 and the median — mildly predictable.
    BelowHumanMedian,
    /// At or above the human median — human-leaning.
    WithinOrAboveHuman,
}

impl ModelNorm {
    /// Place a document perplexity in the human distribution. SOFT signal only.
    pub fn signal(&self, perplexity: f64) -> PerplexitySignal {
        let n = &self.human_academic_perplexity;
        if perplexity < n.p10 {
            PerplexitySignal::UnusuallyPredictable
        } else if perplexity < n.median {
            PerplexitySignal::BelowHumanMedian
        } else {
            PerplexitySignal::WithinOrAboveHuman
        }
    }
}

/// Derive the norms key from a GGUF path: the lowercased filename without the
/// `.gguf` extension (e.g. `.../Qwen2.5-0.5B-Instruct-Q4_K_M.gguf` ->
/// `qwen2.5-0.5b-instruct-q4_k_m`). Keeps norms tied to the exact model, so a
/// model swap can't silently reuse a mismatched calibration.
pub fn model_id_from_gguf(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.trim_end_matches(".gguf").trim_end_matches(".GGUF").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_norms_parse_and_are_provisional() {
        let norms = Stage1Norms::bundled();
        assert_eq!(norms.version, 1);
        assert!(norms.provisional, "Phase-1 seed must be flagged provisional");
    }

    #[test]
    fn norm_lookup_by_model_id_and_soft_signal_placement() {
        let norms = Stage1Norms::bundled();
        let m = norms
            .for_model("qwen2.5-0.5b-instruct-q4_k_m")
            .expect("0.5B norms present");
        assert_eq!(m.human_academic_perplexity.n, 20);
        assert!(m.notes.contains("PROVISIONAL"));
        assert!(m.notes.to_lowercase().contains("overlap"), "records the human/AI overlap caveat");

        // Soft placement (p10=8.6, median=17.5):
        assert_eq!(m.signal(5.0), PerplexitySignal::UnusuallyPredictable);
        // The dense-AI reference (11.46) lands in the HUMAN range, NOT below p10 —
        // the overlap finding: perplexity alone can't convict it.
        assert_eq!(m.signal(11.46), PerplexitySignal::BelowHumanMedian);
        assert_eq!(m.signal(25.0), PerplexitySignal::WithinOrAboveHuman);
        assert!(norms.for_model("no-such-model").is_none());
    }

    #[test]
    fn model_id_derives_from_gguf_filename() {
        assert_eq!(
            model_id_from_gguf("/Users/x/gaply-models/stage1-lm/Qwen2.5-0.5B-Instruct-Q4_K_M.gguf"),
            "qwen2.5-0.5b-instruct-q4_k_m"
        );
        // Windows separators too.
        assert_eq!(model_id_from_gguf(r"C:\models\Model-A.GGUF"), "model-a");
    }
}
