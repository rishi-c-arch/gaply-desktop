//! THE canonical user-facing vocabulary — one mapping, one place, consumed by
//! every surface: desktop, PDF, email, API, anything later.
//!
//! # Why it exists
//!
//! `synthesize.ts:45` drifted from the Rust aggregator, `adapters.ts` carried
//! five hardcoded `certainty_label` strings, and `matchTypeLabel` is a hand
//! mirror of `report.rs`. Four drifts, all the same shape: a concept named in
//! two places that were kept in step by hand (ARCHITECTURE_TRACE §31.8).
//!
//! # What belongs here, and what does not
//!
//! **VOCABULARY owns CLOSED CONCEPTS** — the enums below.
//! **PRODUCERS own GENERATED findings** — a finding `title` is a formatted
//! sentence with interpolated data, and centralising those would replace typed
//! producers with string templates, which is a regression.
//! **PRESENTATION maps concepts and never generates evidence** — casing,
//! ordering and emphasis are the renderer's, not this module's.
//!
//! # Totality
//!
//! Every function is TOTAL over its enum with NO WILDCARD ARM, following
//! `evidence.rs`'s discipline: a new variant must BREAK COMPILATION rather than
//! fall through to a raw internal name. The severity leak (§31.6) is exactly
//! what a wildcard would recreate — the product exposed `FindingSeverity` as
//! `.toUpperCase()` for its whole life because no presentation layer existed.
//!
//! # The rule these words follow (ONTOLOGY §4.21)
//!
//! **A new scholar should understand the label without learning Gaply's
//! internals.**

use crate::evidence::ClaimKind;
use crate::report::{CertaintyTier, FindingSeverity};
use crate::reviewer_agent::Recommendation;

/// How serious a finding is, in a reader's terms.
///
/// # Two families, not a severity ladder
///
/// *Important issue*, *Smaller issue* and *Additional note* all describe
/// **things to fix**. **`Critical` is categorically different** — it describes a
/// claim that cannot currently be supported, which is a statement about KIND
/// rather than DEGREE.
///
/// The engine's own reasoning agrees: `validate.rs`'s comment on the two rules
/// that emit `Critical` says they *"invalidate the analysis"*. The internal name
/// communicated severity and hid kind; *Unsupported conclusion* communicates
/// kind, and severity follows from it.
pub fn severity_label(s: FindingSeverity) -> &'static str {
    match s {
        FindingSeverity::Critical => "Unsupported conclusion",
        FindingSeverity::Major => "Important issue",
        FindingSeverity::Minor => "Smaller issue",
        FindingSeverity::Info => "Additional note",
    }
}

/// What KIND of statement a finding makes — `None` when it needs no label.
///
/// **The label marks the EXCEPTIONS.** A finding about the manuscript is just a
/// finding; only the carve-outs need to say why they did not count, so
/// `ManuscriptDefect` returns `None` rather than an invented word. Typed absence
/// (§4.12) rather than a name nobody asked for.
pub fn claim_label(c: ClaimKind) -> Option<&'static str> {
    match c {
        ClaimKind::ProcessState => Some("Technical check"),
        ClaimKind::AuthorshipSignal => Some("AI writing signal"),
        ClaimKind::ManuscriptDefect => None,
    }
}

/// The editorial recommendation, in SENTENCE CASE.
///
/// **Caps are presentation, not vocabulary.** `RECOMMENDATION_LABEL` rendered
/// `"MAJOR REVISION"`, which is unusable in a sentence — and the PDF needs it in
/// one. A header wanting uppercase applies `text-transform` in CSS.
pub fn recommendation_label(r: Recommendation) -> &'static str {
    match r {
        Recommendation::Accept => "Accept",
        Recommendation::MinorRevision => "Minor revision",
        Recommendation::MajorRevision => "Major revision",
        Recommendation::Reject => "Reject",
        Recommendation::Unknown => "Not determined",
    }
}

/// How firmly a finding is established. Moved verbatim from
/// `CertaintyTier::label`; the wording was already chosen deliberately and is
/// unchanged, only relocated so every label lives in one module.
pub fn tier_label(t: CertaintyTier) -> &'static str {
    match t {
        CertaintyTier::MathematicallyCertain => "mathematically certain",
        CertaintyTier::AiAssessedModerate => "AI-assessed, moderate confidence",
        CertaintyTier::ReconsideredAfterPeerReview => "reconsidered after peer review",
    }
}

/// The checked-in mirror artifact, relative to the `gaply_core` crate root.
///
/// TypeScript cannot call the functions above, and two surfaces genuinely need a
/// copy: `adapters.ts` builds SYNTHETIC reports client-side from raw agent
/// output, so there is no wire for it to read, and `matchTypeLabel` mirrors
/// `report.rs`'s similarity bands for the same reason.
///
/// A comment asking future editors to keep them in step is what `83f192c` left
/// behind, and it is §21's level 1. This artifact is the instrument: Rust
/// asserts it matches the functions, vitest asserts the TS tables match it, and
/// neither can drift without one of the two failing.
#[cfg(test)]
const MIRROR_ARTIFACT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../src/generated/vocabulary.json");

#[cfg(test)]
mod tests {
    use super::*;

    /// Both families are represented and distinguishable — the point of
    /// *Unsupported conclusion* is that it does not read as "worse than Major".
    #[test]
    fn severity_labels_split_into_two_families() {
        assert_eq!(severity_label(FindingSeverity::Critical), "Unsupported conclusion");
        for s in [FindingSeverity::Major, FindingSeverity::Minor, FindingSeverity::Info] {
            let l = severity_label(s);
            assert!(l == "Important issue" || l == "Smaller issue" || l == "Additional note");
            assert_ne!(l, severity_label(FindingSeverity::Critical));
        }
    }

    /// §4.21: no label may leak an internal identifier.
    #[test]
    fn no_label_exposes_an_internal_name() {
        let internal = ["Critical", "Major", "Minor", "Info", "ProcessState",
                        "AuthorshipSignal", "ManuscriptDefect", "MajorRevision",
                        "MinorRevision", "Unknown"];
        let mut all: Vec<&str> = vec![];
        for s in [FindingSeverity::Critical, FindingSeverity::Major,
                  FindingSeverity::Minor, FindingSeverity::Info] {
            all.push(severity_label(s));
        }
        for c in [ClaimKind::ProcessState, ClaimKind::AuthorshipSignal,
                  ClaimKind::ManuscriptDefect] {
            if let Some(l) = claim_label(c) { all.push(l); }
        }
        for r in [Recommendation::Accept, Recommendation::MinorRevision,
                  Recommendation::MajorRevision, Recommendation::Reject,
                  Recommendation::Unknown] {
            all.push(recommendation_label(r));
        }
        for label in all {
            for name in internal {
                assert_ne!(label, name, "a label must not be an internal identifier");
            }
            assert!(!label.is_empty(), "every label carries text");
        }
    }

    /// The label marks the EXCEPTIONS — the default claim has none.
    #[test]
    fn the_default_claim_has_no_label() {
        assert_eq!(claim_label(ClaimKind::ManuscriptDefect), None);
        assert!(claim_label(ClaimKind::ProcessState).is_some());
        assert!(claim_label(ClaimKind::AuthorshipSignal).is_some());
    }

    /// THE MIRROR PIN. Rust owns the vocabulary; the artifact is how TypeScript
    /// learns it. This asserts the checked-in file still matches the functions —
    /// so a wording edit in Rust that is not regenerated fails HERE, and the
    /// vitest side then tells the TS table it is stale.
    ///
    /// Regenerate with: `UPDATE_VOCABULARY=1 cargo test -p gaply_core mirror`
    #[test]
    fn the_mirror_artifact_matches_the_vocabulary() {
        use serde_json::json;
        let expected = json!({
            "_comment": "GENERATED from gaply-core/src/vocabulary.rs. Do not hand-edit.",
            "severity": {
                "critical": severity_label(FindingSeverity::Critical),
                "major": severity_label(FindingSeverity::Major),
                "minor": severity_label(FindingSeverity::Minor),
                "info": severity_label(FindingSeverity::Info),
            },
            "claim": {
                "process_state": claim_label(ClaimKind::ProcessState),
                "authorship_signal": claim_label(ClaimKind::AuthorshipSignal),
                "manuscript_defect": claim_label(ClaimKind::ManuscriptDefect),
            },
            "recommendation": {
                "accept": recommendation_label(Recommendation::Accept),
                "minor_revision": recommendation_label(Recommendation::MinorRevision),
                "major_revision": recommendation_label(Recommendation::MajorRevision),
                "reject": recommendation_label(Recommendation::Reject),
                "unknown": recommendation_label(Recommendation::Unknown),
            },
            "tier": {
                "mathematically_certain": tier_label(CertaintyTier::MathematicallyCertain),
                "ai_assessed_moderate": tier_label(CertaintyTier::AiAssessedModerate),
                "reconsidered_after_peer_review": tier_label(CertaintyTier::ReconsideredAfterPeerReview),
            },
        });
        let pretty = serde_json::to_string_pretty(&expected).unwrap() + "\n";
        if std::env::var("UPDATE_VOCABULARY").is_ok() {
            std::fs::write(MIRROR_ARTIFACT, &pretty).expect("write the mirror artifact");
            return;
        }
        let on_disk = std::fs::read_to_string(MIRROR_ARTIFACT).unwrap_or_default();
        assert_eq!(
            on_disk.trim(),
            pretty.trim(),
            "\n\nThe checked-in vocabulary artifact is stale. Rust owns these words; \
             TypeScript reads them from this file because it cannot call the functions. \
             Regenerate with `UPDATE_VOCABULARY=1 cargo test -p gaply_core mirror`, and \
             expect the vitest mirror test to then flag any TS table that still disagrees.\n"
        );
    }

    /// Enrichment attaches labels WITHOUT touching the engine's own fields, and
    /// gives the default claim an EXPLICIT null so "no label" is distinguishable
    /// from "not enriched".
    #[test]
    fn enrichment_adds_labels_and_leaves_the_engine_fields_alone() {
        use serde_json::json;
        let mut r = json!({"findings": [
            {"severity": "major", "claim": "process_state", "title": "t"},
            {"severity": "minor", "claim": "manuscript_defect", "title": "u"},
            {"severity": "not_a_severity", "claim": "manuscript_defect"}
        ]});
        enrich_report_labels(&mut r);
        let f = &r["findings"];
        assert_eq!(f[0]["severity_label"], "Important issue");
        assert_eq!(f[0]["claim_label"], "Technical check");
        assert_eq!(f[1]["severity_label"], "Smaller issue");
        assert!(f[1]["claim_label"].is_null(), "the default claim has an EXPLICIT null");
        // Unparseable severity gets NO label rather than a guessed one.
        assert!(f[2].get("severity_label").is_none());
        // The engine's own fields are untouched.
        assert_eq!(f[0]["severity"], "major");
        assert_eq!(f[0]["title"], "t");
        // Idempotent.
        let before = r.clone();
        enrich_report_labels(&mut r);
        assert_eq!(before, r);
    }

    /// Caps are presentation. A label must be usable mid-sentence, which the PDF
    /// requires and `"MAJOR REVISION"` was not.
    #[test]
    fn recommendation_labels_are_sentence_case() {
        for r in [Recommendation::Accept, Recommendation::MinorRevision,
                  Recommendation::MajorRevision, Recommendation::Reject,
                  Recommendation::Unknown] {
            let l = recommendation_label(r);
            assert_ne!(l, l.to_uppercase(), "{l} is shouting; caps belong in CSS");
        }
    }
}

/// Attach the user-facing labels to a serialized report, for a consumer that
/// cannot call these functions — the desktop UI, and later the PDF composer.
///
/// # Why this is a BOUNDARY function and not a field on `Finding`
///
/// The first attempt stored `severity_label` and `claim_label` on `Finding`,
/// following `certainty_label`'s precedent. **ONTOLOGY §4.19 forbids it:** the
/// ENGINE emits a report that knows nothing about presentation, and a
/// user-facing label baked into the engine's persisted output is *presentation
/// leaking backwards* — the exact failure the three-layer rule names.
///
/// Enriching at the boundary is better on every axis that mattered:
///
/// * **no schema change**, so `CACHED_REPORT_SCHEMA_VERSION` does not move and
///   §26.4's discipline is not invoked for a presentation edit;
/// * **labels stay fresh** — a copy edit applies to reports cached before it,
///   because nothing stale was written;
/// * **`build_review_payload` is untouched**, so a label edit cannot move
///   `summary_digest` (§31.7). The payload is an explicit projection of
///   `{id, agent, tier, severity, title, confidence, evidence}`; labels are for
///   humans and the model already receives the severity;
/// * **twelve construction sites stay unchanged**, and none can forget a label
///   or supply one inconsistent with its own severity.
///
/// Idempotent and additive: it only inserts keys, so calling it twice is safe.
pub fn enrich_report_labels(report: &mut serde_json::Value) {
    let Some(findings) = report.get_mut("findings").and_then(|f| f.as_array_mut()) else {
        return;
    };
    for f in findings {
        let severity: Option<FindingSeverity> =
            f.get("severity").and_then(|v| serde_json::from_value(v.clone()).ok());
        let claim: Option<ClaimKind> =
            f.get("claim").and_then(|v| serde_json::from_value(v.clone()).ok());
        let Some(obj) = f.as_object_mut() else { continue };
        // TYPED ABSENCE, not a fallback: a severity that does not parse gets NO
        // label rather than a guessed one (§4.12). It cannot happen on a report
        // this binary produced — the field is now strictly typed — so the arm
        // exists for a hand-edited or corrupted entry.
        if let Some(s) = severity {
            obj.insert("severity_label".into(), severity_label(s).into());
        }
        if let Some(c) = claim {
            match claim_label(c) {
                Some(l) => obj.insert("claim_label".into(), l.into()),
                // EXPLICIT null, not an absent key: the default claim genuinely
                // has no label, and a missing key would be indistinguishable
                // from "this build forgot to enrich".
                None => obj.insert("claim_label".into(), serde_json::Value::Null),
            };
        }
    }
}
