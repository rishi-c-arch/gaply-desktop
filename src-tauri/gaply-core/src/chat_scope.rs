//! **§10's chat, scoped to what the verdict layer can answer.**
//!
//! §10's rule is the whole design: *"Every number comes from a query. Counts,
//! locators, severities are retrieved, never generated. The model phrases; it
//! does not compute."* So a mode ships only if the ANSWER is already a field of
//! what the run stored.
//!
//! # MEASURED FIRST — 137 findings over 20 real manuscripts
//!
//! `examples/chat_scope_audit.rs`:
//!
//! | mode | answerable | by |
//! |---|---:|---|
//! | **Explain** | **137 of 137** | `summary` + `trail[severity]` + `uncertainty` |
//! | **Evidence** | **137 of 137** | `spans`, stored whole |
//! | Correction | **0 of 137** | nothing stores a suggested edit |
//! | Challenge | **0 of 137** | no `Decision` row, and no graph-driven re-run |
//!
//! Explain and evidence are not answerable by luck: they rest on fields the
//! lens layer already enforces — `citation_required` refuses a span-less
//! concern, and every concern's trail carries a `severity` step naming the
//! risk-table cell it came from.
//!
//! # THE FIREWALL IS [`crate::chat_agent`]'S, NOT A SECOND ONE
//!
//! This module builds the CONTEXT a turn is answered from. The pre-filter, the
//! system instruction and the post-filter live in `chat_agent` and are reused
//! unchanged. A second firewall would be a second thing to keep correct, and
//! §11 D129's shape.
//!
//! **The `llm_safe` half was CLAIMED here before it was implemented, and the
//! Phase 7b red team found it.** The first version of this module copied
//! `summary` and `spans` straight off the concern while this header said the
//! `llm_safe` discipline was "reused unchanged" — a documented safety property
//! the code did not have. The fixture that exposed it is
//! `red_team::INJECTION_INSIDE_A_QUOTED_SPAN`: an instruction placed inside a
//! sentence that a finding QUOTES rides three findings' spans into this
//! context. Two earlier injection fixtures passed only because they sat where
//! no finding cited them, which proved nothing.
//!
//! Every manuscript-derived string now goes through [`UntrustedText::llm_safe`]
//! — THE mechanism, not a parallel one — with the same synthetic provenance
//! `chat_agent::safe_clamp` uses. A span flagged as injection is WITHHELD, and
//! [`ChatFinding::quarantined`] says so, because a silently-empty span reads as
//! a finding that quotes nothing.
//!
//! # WHAT IS DECLINED, AND WHY EACH
//!
//! See [`ChatMode::blocker`]. Both declines are recorded rather than left as
//! absent features, so a later session meets the reason instead of
//! rediscovering it.

use serde::{Deserialize, Serialize};

use crate::refverify::{Provenance, UntrustedText};
use crate::review_lens::{Concern, ReviewerReport};

/// §10's *"four modes, one scope"*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatMode {
    /// *Why was this flagged.*
    Explain,
    /// *Show me.*
    Evidence,
    /// *What would fix it* — mechanical findings only. **DECLINED.**
    Correction,
    /// *I disagree.* **DECLINED.**
    Challenge,
}

impl ChatMode {
    pub const ALL: [ChatMode; 4] =
        [ChatMode::Explain, ChatMode::Evidence, ChatMode::Correction, ChatMode::Challenge];

    pub fn as_str(self) -> &'static str {
        match self {
            ChatMode::Explain => "explain",
            ChatMode::Evidence => "evidence",
            ChatMode::Correction => "correction",
            ChatMode::Challenge => "challenge",
        }
    }

    /// Does this mode ship? TOTAL, no wildcard: a fifth mode must decide.
    pub fn ships(self) -> bool {
        match self {
            ChatMode::Explain | ChatMode::Evidence => true,
            ChatMode::Correction | ChatMode::Challenge => false,
        }
    }

    /// **Why a declined mode is declined.** `None` for a mode that ships.
    ///
    /// These are the sentences a user sees when they ask, so they say what is
    /// missing rather than "not supported".
    pub fn blocker(self) -> Option<&'static str> {
        match self {
            ChatMode::Explain | ChatMode::Evidence => None,
            ChatMode::Correction => Some(CORRECTION_BLOCKER),
            ChatMode::Challenge => Some(CHALLENGE_BLOCKER),
        }
    }
}

/// **Correction needs §9's suggested edits, which were never built.**
///
/// **`required_revision` exists and is NOT an answer to "what would fix it".**
/// It carries the reviewer document's generic **Action:** text — *"Use
/// reference manager tools (EndNote, Zotero) with the journal's style file"* —
/// which is advice about the criterion, not a fix for THIS manuscript. Offering
/// it as a computed correction would be a well-formed answer to a question the
/// system cannot answer, which is precisely the fabrication §11 D166 declined
/// the scientific layer over. That is why the audit counted correction at
/// **0 of 137** rather than 137 of 137, and the distinction is the entire
/// content of this decline.
pub const CORRECTION_BLOCKER: &str =
    "Correction mode is not built. §9 lists a suggested edit for mechanical findings and \
     nothing produces one. The `required_revision` this finding carries is the reviewer \
     document's generic advice for the criterion — not a fix for this manuscript — and \
     presenting it as a computed correction would be a well-formed answer to a question \
     the system cannot answer. What CAN be shown is the finding's own span and the \
     reviewer document's action text, both labelled as what they are.";

/// **Challenge needs four layers and the bottom one is unbuilt.**
///
/// Stated as a chain so a session proposing challenge mode meets it rather than
/// reassembling it:
///
/// ```text
/// §10 challenge mode          "I disagree; the 24 were excluded per protocol"
///   -> a Decision ledger      a row: what, why, user-confirmed, evidence
///                             pointer, which research-state fields it affects
///      -> §5.5 incremental    compute which fields changed, which agents read
///         re-analysis         them, re-run only that subgraph
///         -> §12.1 item 2     the graph DESCRIBES the lanes; it does not
///                             DRIVE them. `run_pipeline_inner` executes a
///                             hardcoded sequence, so there is no subgraph to
///                             re-run.
/// ```
///
/// Measured over the tree 15 Sep 2026 by grep, not asserted: `struct Decision`
/// 0 hits, `CREATE TABLE decision` 0, `revision_history` 0.
///
/// **Note the order.** A Decision ledger could be built tomorrow and challenge
/// mode would still not work, because §10's promise is that *"the affected
/// subgraph re-runs"* and nothing can re-run a subgraph. Building the ledger
/// first would produce a mode that records disagreement and changes nothing —
/// worse than declining it, because the user would reasonably expect the
/// finding to be re-evaluated.
pub const CHALLENGE_BLOCKER: &str =
    "Challenge mode is not built, and it depends on four layers of which the bottom is \
     unbuilt: §10's challenge -> a Decision ledger (no `Decision` type, no decisions table) \
     -> §5.5's incremental re-analysis -> §12.1 item 2, the graph-driven executor. \
     `run_pipeline_inner` runs a hardcoded sequence, so there is no affected subgraph to \
     re-run. A ledger alone would record the disagreement and change nothing, which is \
     worse than declining: the finding would visibly not be re-evaluated.";

/// One finding, as the chat may see it. **Stored fields only** — every value
/// here is copied from the run, never derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatFinding {
    /// Stable within a turn, so an answer can cite `f3`.
    pub id: String,
    pub lens: String,
    pub criterion: String,
    pub code: String,
    pub severity: String,
    /// EXPLAIN: what was found.
    pub summary: String,
    /// EXPLAIN: why this severity, naming the risk-table cell.
    pub severity_reason: String,
    /// EXPLAIN: what the check could not determine.
    pub uncertainty: Option<String>,
    /// EVIDENCE: the manuscript's own sentences, whole.
    pub spans: Vec<String>,
    /// EVIDENCE: how a reviewer would check it independently (§9).
    pub how_to_check: Vec<String>,
    /// The reviewer document's action text for this criterion. **Labelled, and
    /// never presented as a correction** — see [`CORRECTION_BLOCKER`].
    pub criterion_action: String,
    /// **Set when a manuscript-derived string was withheld as injection.**
    /// Never silent: an empty span and a quarantined one mean opposite things
    /// to a reader, and to a model.
    pub quarantined: Vec<String>,
}

/// The context a turn is answered from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatContext {
    pub findings: Vec<ChatFinding>,
    /// Modes that do not ship, with the sentence the user is shown.
    pub declined_modes: Vec<(String, String)>,
}

/// **Build the chat context from a stored run.** Pure: no model, no network,
/// and it never reads the manuscript — §10's first rule.
pub fn context_from(reports: &[ReviewerReport]) -> ChatContext {
    let mut findings = Vec::new();
    for r in reports {
        for c in r.major_concerns.iter().chain(r.minor_concerns.iter()) {
            let mut quarantined = Vec::new();
            let summary = safe(&c.summary, "report", &mut quarantined, "summary");
            let spans: Vec<String> = c
                .spans
                .iter()
                .enumerate()
                .map(|(i, s)| safe(s, "manuscript", &mut quarantined, &format!("span {}", i + 1)))
                .collect();
            findings.push(ChatFinding {
                id: format!("f{}", findings.len() + 1),
                lens: r.lens.as_str().to_string(),
                criterion: c.criterion.clone(),
                code: c.code.clone(),
                severity: c.severity.as_str().to_string(),
                summary,
                severity_reason: severity_reason(c),
                uncertainty: c.uncertainty.clone(),
                spans,
                // **The trail carries the spans too.** Sanitising `spans` and
                // not this left the instruction reaching the context through
                // the `evidence` step, which quotes every span back. The red
                // team found the first leak; re-running it found this one.
                how_to_check: c
                    .trail
                    .iter()
                    .map(|t| {
                        let d = safe(&t.detail, "report", &mut quarantined, &t.stage);
                        format!("{}: {}", t.stage, d)
                    })
                    .collect(),
                criterion_action: c.required_revision.clone(),
                quarantined,
            });
        }
    }
    ChatContext {
        findings,
        declined_modes: ChatMode::ALL
            .iter()
            .filter(|m| !m.ships())
            .map(|m| (m.as_str().to_string(), m.blocker().unwrap().to_string()))
            .collect(),
    }
}

/// **Run one manuscript-derived string through the firewall.**
///
/// `UntrustedText::llm_safe` withholds the whole string when the injection
/// scanner flags it, so a flagged span never reaches a model. The synthetic
/// provenance matches `chat_agent::safe_clamp`'s: this text came from a parsed
/// document, not a fetch, and there is no URL to record.
fn safe(raw: &str, source: &str, quarantined: &mut Vec<String>, what: &str) -> String {
    let prov = Provenance {
        source: source.to_string(),
        url: String::new(),
        fetched_at: 0,
        checksum: String::new(),
        from_cache: false,
    };
    let t = UntrustedText::new(raw, prov);
    if t.is_suspicious() {
        // **The reason names a COUNT, not the matched pattern.** The first
        // version interpolated `injection_flags()`, which put the trigger
        // string back into the payload the withholding exists to protect — the
        // quarantine notice re-introducing what it quarantined. Caught by
        // scanning the whole serialised context rather than field by field.
        // The flags remain available on the `UntrustedText` for anything
        // server-side that needs them; they do not travel to a model.
        quarantined.push(format!(
            "{what} withheld: {} injection pattern(s) detected in manuscript-derived text. \
             The pattern itself is deliberately not repeated here.",
            t.injection_flags().len()
        ));
    }
    t.llm_safe()
}

/// The trail step that explains the severity, which is what EXPLAIN turns on.
fn severity_reason(c: &Concern) -> String {
    c.trail
        .iter()
        .find(|t| t.stage == "severity")
        .map(|t| t.detail.clone())
        .unwrap_or_else(|| {
            // Unreachable while the lens always emits a severity step, and a
            // sentence rather than an empty string so a renderer never shows a
            // blank where a reason belongs.
            "No severity reason was recorded for this finding.".to_string()
        })
}

/// **Answer one question about one finding, from stored fields only.**
///
/// Returns the material a model may phrase. `Err` for a declined mode, carrying
/// the sentence the user sees.
pub fn answer(
    ctx: &ChatContext,
    mode: ChatMode,
    finding_id: &str,
) -> Result<Vec<String>, &'static str> {
    if let Some(blocker) = mode.blocker() {
        return Err(blocker);
    }
    let Some(f) = ctx.findings.iter().find(|f| f.id == finding_id) else {
        return Ok(vec![format!("No finding `{finding_id}` is in this run.")]);
    };
    Ok(match mode {
        ChatMode::Explain => {
            let mut out = vec![
                format!("[{}] {} — {}", f.severity, f.criterion, f.code),
                f.summary.clone(),
                f.severity_reason.clone(),
            ];
            if let Some(u) = &f.uncertainty {
                out.push(format!("What this check could not determine: {u}"));
            }
            out
        }
        ChatMode::Evidence => {
            let mut out = vec![format!(
                "{} span(s) from the manuscript, stored whole:",
                f.spans.len()
            )];
            out.extend(f.spans.iter().cloned());
            if !f.how_to_check.is_empty() {
                out.push("How a reviewer would check this independently:".to_string());
                out.extend(f.how_to_check.iter().cloned());
            }
            out
        }
        // Unreachable: both declined modes returned above.
        ChatMode::Correction | ChatMode::Challenge => unreachable!("declined modes return early"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review_lens::{lenses, review, LensInput};
    use crate::specialist::{self, SpecialistInput};

    const PAPER: &str = "A Title\n\n1. Background\n\nEmployer mandates matter.\n\n\
                         5. Results\n\nProvision was 36.5% (t = 2.1, p = 0.04).\n";

    fn ctx() -> ChatContext {
        let ex = crate::extract::extract_from_text(PAPER);
        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let specs: Vec<_> =
            specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &sin)).collect();
        let validity = crate::validate::validate(&ex);
        let standards =
            vec![crate::report::evaluate(crate::journal_standards::Standard::Strobe, &ex, PAPER)];
        let input = LensInput {
            extraction: &ex,
            full_text: Some(PAPER),
            this_year: 2026,
            specialists: Some(&specs),
            validity: Some(&validity),
            standards: Some(&standards),
            checklist: None,
            novelty: None,
        };
        let reports: Vec<_> = lenses().iter().map(|l| review(l, &input)).collect();
        context_from(&reports)
    }

    /// **Two modes ship and two are declined.** TOTAL over the enum, so a fifth
    /// mode has to decide.
    #[test]
    fn explain_and_evidence_ship_and_the_other_two_are_declined() {
        let ships: Vec<&str> =
            ChatMode::ALL.iter().filter(|m| m.ships()).map(|m| m.as_str()).collect();
        assert_eq!(ships, vec!["explain", "evidence"]);
        for m in ChatMode::ALL.iter().filter(|m| !m.ships()) {
            assert!(m.blocker().is_some(), "{m:?} is declined and must say why");
        }
    }

    /// **Every finding can be explained and shown.** The measurement was
    /// 137 of 137 on real manuscripts; this holds it.
    #[test]
    fn every_finding_answers_explain_and_evidence() {
        let c = ctx();
        assert!(!c.findings.is_empty());
        for f in &c.findings {
            let e = answer(&c, ChatMode::Explain, &f.id).unwrap();
            assert!(e.len() >= 3, "explain needs summary and severity reason: {e:?}");
            assert!(
                e.iter().any(|x| x.contains("Risk Assessment Table")),
                "the severity reason names the risk-table cell: {e:?}"
            );
            let v = answer(&c, ChatMode::Evidence, &f.id).unwrap();
            assert!(v.len() > 1, "evidence needs at least one span: {v:?}");
        }
    }

    /// **`required_revision` is carried but never offered as a correction.**
    /// Presenting the reviewer document's generic advice as a computed fix is
    /// the fabrication §11 D166 declined the scientific layer over.
    #[test]
    fn correction_is_refused_and_the_reason_names_required_revision() {
        let c = ctx();
        let id = c.findings[0].id.clone();
        let err = answer(&c, ChatMode::Correction, &id).unwrap_err();
        assert!(err.contains("required_revision"), "{err}");
        assert!(err.contains("not a fix for this manuscript"), "{err}");
        // The field IS carried — the decline is about how it may be used.
        assert!(!c.findings[0].criterion_action.is_empty());
    }

    /// **Challenge names its whole dependency chain**, so a session proposing
    /// it meets the chain rather than reassembling it.
    #[test]
    fn challenge_is_refused_and_names_every_layer_it_depends_on() {
        let c = ctx();
        let id = c.findings[0].id.clone();
        let err = answer(&c, ChatMode::Challenge, &id).unwrap_err();
        for layer in ["Decision ledger", "§5.5", "§12.1 item 2", "hardcoded sequence"] {
            assert!(err.contains(layer), "the chain must name `{layer}`: {err}");
        }
        assert!(
            err.contains("worse than declining"),
            "and why a ledger alone is not enough: {err}"
        );
    }

    /// **The WHOLE serialised context is scanned, not a field at a time.**
    ///
    /// Sanitising `summary` and `spans` still leaked, because the trail's
    /// `evidence` step quotes every span back. Checking one field at a time is
    /// how that survived the first fix; this asserts the property over the
    /// whole artefact, so the next field added is covered by construction.
    #[test]
    fn no_injection_pattern_survives_anywhere_in_the_serialised_context() {
        for text in [
            crate::red_team::INJECTION_INSIDE_A_QUOTED_SPAN,
            crate::red_team::CAPTION_INJECTION,
        ] {
            let (_, reports, _) = crate::red_team::run_pipeline(text, None);
            let c = context_from(&reports);
            let json = serde_json::to_string(&c).unwrap();
            let hits = crate::sanitize::scan_injections(&json);
            assert!(
                hits.is_empty(),
                "an injection pattern survived into the context: {hits:?}"
            );
        }
    }

    /// **An instruction inside a quoted span never reaches the chat context.**
    ///
    /// The Phase 7b fixture that exposed the original defect: this module
    /// claimed the `llm_safe` discipline was reused and copied spans raw. Two
    /// earlier injection fixtures passed only because they sat where no finding
    /// cited them.
    #[test]
    fn an_injection_inside_a_quoted_span_is_withheld_from_the_context() {
        let text = crate::red_team::INJECTION_INSIDE_A_QUOTED_SPAN;
        let (_, reports, _) = crate::red_team::run_pipeline(text, None);
        let leaked = crate::red_team::injection_reaches_a_finding(&reports);
        assert!(!leaked.is_empty(), "precondition: the finding spans DO carry it: {leaked:?}");

        let c = context_from(&reports);
        let json = serde_json::to_string(&c).unwrap();
        assert!(
            !json.to_lowercase().contains("ignore previous instructions"),
            "the instruction reached the chat context"
        );
        assert!(
            c.findings.iter().any(|f| !f.quarantined.is_empty()),
            "and the withholding is stated, not silent: {:?}",
            c.findings.iter().map(|f| &f.quarantined).collect::<Vec<_>>()
        );
    }

    /// **Nothing in the context is computed.** §10: the model phrases, it does
    /// not compute. Every field is a copy of a stored one — passed through the
    /// firewall, which returns the text unchanged unless the injection scanner
    /// flags it. This fixture carries no injection, so copy-equality is the
    /// right assertion here; `no_injection_pattern_survives_anywhere_in_the_serialised_context`
    /// covers the flagged case.
    #[test]
    fn every_context_field_is_copied_from_the_stored_run() {
        let ex = crate::extract::extract_from_text(PAPER);
        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let specs: Vec<_> =
            specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &sin)).collect();
        let validity = crate::validate::validate(&ex);
        let standards =
            vec![crate::report::evaluate(crate::journal_standards::Standard::Strobe, &ex, PAPER)];
        let input = LensInput {
            extraction: &ex,
            full_text: Some(PAPER),
            this_year: 2026,
            specialists: Some(&specs),
            validity: Some(&validity),
            standards: Some(&standards),
            checklist: None,
            novelty: None,
        };
        let reports: Vec<_> = lenses().iter().map(|l| review(l, &input)).collect();
        let c = context_from(&reports);

        let stored: Vec<&Concern> = reports
            .iter()
            .flat_map(|r| r.major_concerns.iter().chain(r.minor_concerns.iter()))
            .collect();
        assert_eq!(c.findings.len(), stored.len());
        for (f, s) in c.findings.iter().zip(stored.iter()) {
            assert_eq!(f.summary, s.summary);
            assert_eq!(f.spans, s.spans);
            assert_eq!(f.code, s.code);
            assert_eq!(f.severity, s.severity.as_str());
        }
    }

    /// The context never carries the manuscript — §10's first rule. It carries
    /// SPANS, which are sentences the findings already quote, and nothing else
    /// from the paper.
    #[test]
    fn the_context_carries_no_manuscript_text_beyond_the_spans_findings_quote() {
        let c = ctx();
        let json = serde_json::to_string(&c).unwrap();
        // A sentence present in the manuscript but quoted by no finding must
        // not appear anywhere in the context.
        assert!(
            !json.contains("Employer mandates matter"),
            "prose no finding cites leaked into the chat context"
        );
    }
}
