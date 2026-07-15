//! Multi-signal feature vector + document-score scaffolding for the AI-Signal
//! ensemble (Phase 1). ADDITIVE — these types are not yet wired into
//! `analyze_tiered`; later sets populate the fields (real-LM perplexity, cheap
//! signals, the verifier feature) and surface the Evidence Summary.
//!
//! HONESTY GATE: the 0-100 `DocumentScore::value` stays `None` until a trained,
//! calibrated ensemble exists (Phase 3). Phases 1-2 ship the qualitative
//! Evidence Summary WITHOUT a headline number; the existing honest proportion
//! continues to carry the report.

use serde::{Deserialize, Serialize};

use crate::ai_detect::DeepKind;

/// Qualitative evidence level for one signal (Phase 1 — no numeric score yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalLevel {
    High,
    Moderate,
    Low,
    /// The signal could not be computed (e.g. citation verification offline).
    Unavailable,
}

/// Ensemble bias tier. Fairness is the load-bearing constraint: factual,
/// world-verifiable signals (citation existence/DOI match) are trusted; the
/// proficiency-correlated stylometric signals (perplexity, burstiness, lexical
/// diversity) are DOWN-WEIGHTED because they provably over-flag non-native
/// English writers. This mirrors `swarm.rs`: deterministic facts are near
/// hard-constraints; heuristic signals are rescaled (k≈0.6). Weights are
/// advisory until Phase-3 training replaces them with learned coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiasTier {
    /// World-verifiable (citation existence/DOI/fabrication). Highest trust.
    Factual,
    /// Structural/format (citation density, placement, n-gram templating). Medium.
    Structural,
    /// Proficiency-correlated (perplexity, burstiness, lexical diversity). Soft.
    Stylometric,
}

impl BiasTier {
    /// Default pre-training ensemble weight (swarm.rs philosophy).
    pub fn default_weight(self) -> f64 {
        match self {
            BiasTier::Factual => 1.0,
            BiasTier::Structural => 0.8,
            BiasTier::Stylometric => 0.6,
        }
    }
}

/// One signal's evidence, surfaced in the Evidence Summary. Never a verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalEvidence {
    /// Generic, model-agnostic label (e.g. "real language-model perplexity") so
    /// the underlying model can be upgraded without renaming.
    pub signal: String,
    pub level: SignalLevel,
    pub bias_tier: BiasTier,
    /// Short human-readable detail — never empty (e.g. "2 of 14 references
    /// could not be verified").
    pub detail: String,
}

/// The deep verifier as ONE ensemble feature — NEVER the gatekeeper. Tier
/// identity is an input the calibration accounts for.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VerifierFeature {
    pub confirmed: bool,
    pub tier: DeepKind,
}

/// Per-passage feature vector. `None` = the signal was not computed for this
/// passage. Fields populate across later sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PassageFeatures {
    /// Real language-model perplexity on the smart sample. Low = more AI-like.
    pub lm_perplexity: Option<f64>,
    /// Sentence perplexity/length burstiness. Low = more AI-like.
    pub burstiness: Option<f64>,
    /// MTLD lexical diversity as a TWO-SIDED deviation from the human norm.
    pub mtld_deviation: Option<f64>,
    /// Template-phrase density (regionally-loaded tokens stripped).
    pub template_density: Option<f64>,
    /// Long-n-gram novelty vs the reference corpus.
    pub ngram_novelty: Option<f64>,
    /// Consistency vs the document's own baseline — the former self-baseline,
    /// now a FEATURE (valuable for AI-inserted-into-human text), never a gate.
    pub self_consistency: Option<f64>,
    /// The deep verifier's opinion, if it ran.
    pub verifier: Option<VerifierFeature>,
}

/// Document-level score slot. `value` (0-100) is `None` until the trained
/// calibrated ensemble ships (Phase 3 honesty gate); `evidence` ships from
/// Phase 1; `band` is the confidence interval (Phase 3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DocumentScore {
    /// The 0-100 AI Signal Score — Phase 3 ONLY. `None` = not yet trained.
    pub value: Option<u8>,
    /// Confidence band (low, high) — Phase 3 ONLY.
    pub band: Option<(u8, u8)>,
    /// The per-signal Evidence Summary — ships from Phase 1.
    pub evidence: Vec<SignalEvidence>,
}

impl DocumentScore {
    /// Honesty gate: true only when a real trained score is present. Until then
    /// the report shows the Evidence Summary and the existing honest proportion.
    pub fn has_score(&self) -> bool {
        self.value.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bias_tier_weights_downweight_stylometric_signals() {
        assert_eq!(BiasTier::Factual.default_weight(), 1.0);
        assert_eq!(BiasTier::Structural.default_weight(), 0.8);
        assert_eq!(BiasTier::Stylometric.default_weight(), 0.6);
        assert!(BiasTier::Factual.default_weight() > BiasTier::Stylometric.default_weight());
    }

    #[test]
    fn honesty_gate_no_score_by_default_but_evidence_ships() {
        let mut doc = DocumentScore::default();
        assert!(!doc.has_score(), "no headline number until Phase 3");
        assert!(doc.band.is_none());
        // Evidence Summary ships in Phase 1 without any numeric score.
        doc.evidence.push(SignalEvidence {
            signal: "real language-model perplexity".into(),
            level: SignalLevel::High,
            bias_tier: BiasTier::Stylometric,
            detail: "sample perplexity below the human academic norm".into(),
        });
        assert!(!doc.has_score(), "evidence present, still no score");
        assert_eq!(doc.evidence.len(), 1);
    }

    #[test]
    fn features_and_score_round_trip_through_json() {
        let feats = PassageFeatures {
            lm_perplexity: Some(9.3),
            burstiness: Some(0.24),
            verifier: Some(VerifierFeature { confirmed: true, tier: DeepKind::Compact }),
            ..Default::default()
        };
        let wire = serde_json::to_string(&feats).unwrap();
        let back: PassageFeatures = serde_json::from_str(&wire).unwrap();
        assert_eq!(feats, back);
        assert!(wire.contains("lm_perplexity"));
        // un-computed signals serialize as null, not fabricated zeros.
        assert!(wire.contains("\"mtld_deviation\":null"));

        let doc = DocumentScore::default();
        let w2 = serde_json::to_string(&doc).unwrap();
        assert!(w2.contains("\"value\":null"), "honesty gate visible on the wire");
    }
}
