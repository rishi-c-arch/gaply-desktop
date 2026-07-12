//! The AI Check flow (Set 4): tiered detection then passage classification,
//! with the one-at-a-time model lifecycle.
//!
//! Three stages over one document, at most ONE model in memory at a time
//! (the proven PublishReady lifecycle — the 8GB hard requirement):
//!
//! 1. **Stage 1+2** — `analyze_tiered`: the heuristic pre-pass over the whole
//!    document, then SLM-1 (candle, in-process) deep re-scores the top
//!    candidates within the Set-3 budget. SLM-1 is scoped to this block and
//!    DROPS before anything else loads.
//! 2. **Stage 3** — `classify_passages`: SLM-2 (qwen3 over Ollama, separate
//!    process) classifies each deep-verified passage — leans-AI-generated vs
//!    leans-AI-paraphrased — within the Set-4 call cap. When Ollama isn't
//!    running this is the honest two-way fallback: no category is guessed.
//! 3. **Unload** — `unload_slm2` verifies via `/api/ps` that qwen3 actually
//!    left memory, so a follow-up analysis can load SLM-1 again safely.
//!
//! LOCAL-ONLY: AI Check is free and never touches the cloud proxy. Every
//! model arrives through a gaply-core trait seam; all honesty guarantees
//! (signal labels, cautions, gates, the deterministic %) live in the core.

use gaply_core::ai_detect::{
    self, ClassifiedAnalysis, HeuristicModel, DEFAULT_MAX_CLASSIFIED_PASSAGES,
    DEFAULT_MAX_DEEP_PASSAGES, DEFAULT_MAX_DEEP_TOKENS,
};
use gaply_core::extract::ExtractionResult;

/// Run the full AI Check over an extraction. Never fails: an absent SLM-1
/// means honest heuristic-only labels; an absent Ollama means the honest
/// two-way fallback — both are data in the result, not errors.
#[tracing::instrument(skip(extraction), fields(sections = extraction.sections.len()))]
pub fn run_aicheck_flow(extraction: &ExtractionResult) -> ClassifiedAnalysis {
    // Stage 1+2 — SLM-1 is scoped to this block: candle's memory is RELEASED
    // at the closing brace, BEFORE Ollama loads SLM-2 (one-at-a-time).
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

    // Stage 3 — SLM-2 classification; None = honest two-way fallback.
    let classifier = crate::models::aicheck_classifier();
    let out = ai_detect::classify_passages(
        classifier.as_deref(),
        &tiered,
        DEFAULT_MAX_CLASSIFIED_PASSAGES,
    );

    // The 8GB guard: verify SLM-2 actually left memory before returning, so
    // the next analysis can load SLM-1 without stacking models.
    if classifier.is_some() {
        crate::models::unload_slm2();
    }
    out
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

    /// BOTH degraded-environment scenarios in ONE test, sequentially — the
    /// two scenarios mutate the same process-wide env vars, so splitting them
    /// into parallel #[test]s would race. SLM-1 is forced absent throughout
    /// (the pipeline-test `force_heuristic` precedent): no candle load, no
    /// real network, fully deterministic.
    #[test]
    fn flow_stays_honest_with_scripted_ollama_and_without_it() {
        std::env::set_var("GAPLY_SLM1_GGUF", "/nonexistent/gaply-aicheck-test.gguf");
        let ex = extract_from_text(&format!(
            "Introduction\n\n{h} {a} {a} {a} {h}\n",
            a = AI_SENT,
            h = HUMAN_SENT
        ));

        // --- Scenario 1: scripted Ollama reachable, SLM-1 absent -----------
        // With no deep tier nothing is deep-verified, so the classifier must
        // receive ZERO calls (structural eligibility) while still being
        // reported as the available backend.
        std::env::set_var("GAPLY_SLM2_ENDPOINT", scripted_ollama("{}"));
        let out = run_aicheck_flow(&ex);
        assert!(out.deep_model.is_none(), "SLM-1 absent must be reported as None");
        assert!(!out.passages.is_empty(), "the AI-like run should flag");
        assert!(out
            .passages
            .iter()
            .all(|p| p.tiered.depth == AnalysisDepth::HeuristicOnly));
        assert!(out
            .passages
            .iter()
            .all(|p| p.category == PassageCategory::Unclassified));
        assert!(out.classifier_model.is_some(), "scripted Ollama probes as live");
        assert_eq!(out.classified, 0, "heuristic-only passages are never classified");
        assert!(out.classification_note.contains("0 deep-verified"));
        assert!(out.coverage_note.contains("heuristic-only"));

        // --- Scenario 2: Ollama absent → honest two-way fallback -----------
        // Bind-then-drop a listener so the port is guaranteed dead.
        let dead = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", l.local_addr().unwrap())
        };
        std::env::set_var("GAPLY_SLM2_ENDPOINT", dead);
        let out = run_aicheck_flow(&ex);
        assert!(out.classifier_model.is_none(), "unreachable Ollama must be None");
        assert_eq!(out.classified, 0);
        assert!(out.classification_note.contains("UNAVAILABLE"));
        assert!(out.classification_note.contains("nothing was guessed"));

        // The un-strippable cautions ride in BOTH scenarios.
        assert!(!out.disclaimer.is_empty());
        assert!(!out.paraphrase_caution.is_empty());
        for p in &out.passages {
            assert!(!p.category_note.is_empty());
            assert!(!p.tiered.depth_note.is_empty());
            assert!(!p.tiered.passage.uncertainty.is_empty());
        }
    }
}
