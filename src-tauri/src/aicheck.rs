//! The AI Check flow (Set 5): TWO-WAY tiered detection — human-written vs
//! AI-associated — with the one-at-a-time model lifecycle.
//!
//! # Why two-way (the Set-4 live-probe decision)
//!
//! Set 4 built the full generated-vs-paraphrased classification path (seam,
//! gates, cap) and the live probe then showed qwen3:4b CANNOT make that
//! distinction usably: fast mode (think:false) blurs every passage into one
//! category; reasoning mode (think:true) separates them but at 5–14 MINUTES
//! per call and still forces a category onto clearly-human text. So the
//! shipped flow runs `classify_passages(None, …)` — the honest two-way
//! fallback that was built and tested for exactly this: every deep-verified
//! passage keeps its AI-associated signal, the paraphrase lane reports
//! "unavailable", and nothing is ever guessed. The Set-4 machinery stays
//! correct (schema-as-format fixed) for probes and for a future local model
//! that can actually make the distinction.
//!
//! # Stages (at most ONE model in memory at any point)
//!
//! 1. **Stage 1** — heuristic pre-pass over the whole document (seconds).
//! 2. **Stage 2** — SLM-1 (candle, in-process) deep re-scores the top
//!    candidates within the Set-3 budget, self-calibrated against the
//!    document's own baseline. SLM-1 is scoped to this block and DROPS at its
//!    end. No other model is ever loaded by this flow.
//!
//! LOCAL-ONLY: AI Check is free and never touches the cloud proxy. All
//! honesty guarantees (signal labels, tiers, cautions, the deterministic %,
//! the Set-5 language downgrade) live in gaply-core.

use gaply_core::ai_detect::{
    self, ClassifiedAnalysis, HeuristicModel, DEFAULT_MAX_CLASSIFIED_PASSAGES,
    DEFAULT_MAX_DEEP_PASSAGES, DEFAULT_MAX_DEEP_TOKENS,
};
use gaply_core::extract::ExtractionResult;

/// Run the full AI Check over an extraction. Never fails: an absent SLM-1
/// means honest heuristic-only labels — degraded tiers are data in the
/// result, not errors.
#[tracing::instrument(skip(extraction), fields(sections = extraction.sections.len()))]
pub fn run_aicheck_flow(extraction: &ExtractionResult) -> ClassifiedAnalysis {
    // Stage 1+2 — SLM-1 is scoped to this block: candle's memory is RELEASED
    // at the closing brace (one-at-a-time).
    let tiered = {
        let fast = HeuristicModel::default();
        let deep = crate::models::slm1_model();
        if deep.is_none() {
            tracing::warn!("AI Check: SLM-1 absent — all flags will be heuristic-only");
        }
        ai_detect::analyze_tiered(
            &fast,
            deep.as_deref(),
            extraction,
            DEFAULT_MAX_DEEP_PASSAGES,
            DEFAULT_MAX_DEEP_TOKENS,
        )
    }; // ← SLM-1 dropped HERE

    // TWO-WAY collapse (probe decision): `None` is deliberate, even when
    // Ollama is running — no supported local model makes the
    // generated-vs-paraphrased distinction reliably, so it is never asked.
    // The result honestly reports the distinction as unavailable.
    ai_detect::classify_passages(None, &tiered, DEFAULT_MAX_CLASSIFIED_PASSAGES)
}

#[cfg(test)]
mod tests {
    use gaply_core::ai_detect::{AnalysisDepth, PassageCategory};
    use gaply_core::extract::extract_from_text;

    use crate::models::ollama_verify::scripted::scripted_ollama;

    use super::*;

    // Common-word AI-like prose the heuristic pre-pass reliably flags.
    const AI_SENT: &str = "The results show that the model can do the work well.";
    const HUMAN_SENT: &str =
        "Cerulean contraptions wheezed, magnificently preposterous, unfathomable.";

    /// The two-way collapse holds with AND without a live (scripted) Ollama —
    /// both scenarios in ONE test because they mutate process-wide env vars.
    /// SLM-1 is forced absent throughout (the pipeline-test `force_heuristic`
    /// precedent): no candle load, no real network, fully deterministic.
    #[test]
    fn flow_is_two_way_and_honest_with_or_without_ollama() {
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/gaply-aicheck-test.gguf");
        let ex = extract_from_text(&format!(
            "Introduction\n\n{h} {a} {a} {a} {h}\n",
            a = AI_SENT,
            h = HUMAN_SENT
        ));

        // --- Scenario 1: a live (scripted) Ollama must make NO difference —
        // the probe decision is that the classifier is never consulted.
        std::env::set_var("GAPLY_SLM2_ENDPOINT", scripted_ollama("{}"));
        let out = run_aicheck_flow(&ex);
        assert!(out.deep_model.is_none(), "SLM-1 absent must be reported as None");
        assert!(!out.passages.is_empty(), "the AI-like run should flag");
        assert!(out
            .passages
            .iter()
            .all(|p| p.tiered.depth == AnalysisDepth::HeuristicOnly));
        assert!(
            out.classifier_model.is_none(),
            "TWO-WAY collapse: the classifier is never consulted, even with Ollama live"
        );
        assert!(out
            .passages
            .iter()
            .all(|p| p.category == PassageCategory::Unclassified));
        assert_eq!(out.classified, 0);
        assert!(out.classification_note.contains("UNAVAILABLE"));
        assert!(out.classification_note.contains("nothing was guessed"));
        assert!(out.classification_note.contains("two-way"));
        assert!(out.coverage_note.contains("heuristic-only"));

        // --- Scenario 2: Ollama absent — identical two-way result shape.
        let dead = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", l.local_addr().unwrap())
        };
        std::env::set_var("GAPLY_SLM2_ENDPOINT", dead);
        let out2 = run_aicheck_flow(&ex);
        assert!(out2.classifier_model.is_none());
        assert_eq!(out2.classified, 0);
        assert!(out2.classification_note.contains("UNAVAILABLE"));

        // The un-strippable cautions + Set-5 language assessment ride always.
        for out in [&out, &out2] {
            assert!(!out.disclaimer.is_empty());
            assert!(!out.paraphrase_caution.is_empty());
            assert_eq!(out.language.detected, "english");
            assert!(out.language.calibration_reliable);
            assert!(!out.language.note.is_empty(), "language note is REQUIRED");
            for p in &out.passages {
                assert!(!p.category_note.is_empty());
                assert!(!p.tiered.depth_note.is_empty());
                assert!(!p.tiered.passage.uncertainty.is_empty());
            }
        }
    }

    /// Non-English input downgrades the whole report, un-strippably.
    #[test]
    fn non_english_document_is_downgraded() {
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/gaply-aicheck-test.gguf");
        let ex = extract_from_text(
            "Introducción\n\nLos resultados de este estudio muestran que el método es eficaz y \
             los datos son consistentes con la interpretación de los hallazgos en la muestra.\n",
        );
        let out = run_aicheck_flow(&ex);
        assert_eq!(out.language.detected, "spanish");
        assert!(!out.language.calibration_reliable);
        assert!(out.language.note.contains("LOW-CONFIDENCE"));
    }
}
