//! **Equation findings on the report surface.**
//!
//! The Tier-0 engine ([`crate::equation`]) produced [`ArithmeticFinding`] and
//! [`DimensionVerdict`] that no surface displayed, which means a researcher got
//! none of it. This is the mapping onto [`Finding`], and it lives in its own
//! module so the whole translation is auditable in one place rather than spread
//! through `report.rs`'s constructors.
//!
//! # Why it is not inside `equation/`
//!
//! `tests/equation_is_llm_free.rs` pins the Tier-0 engine's imports to `std`
//! and `serde`. `report.rs` reaches the swarm, the evidence record and the
//! proxy-facing payload, and the engine must not acquire a path to any of them
//! just to be displayed. The engine states what it found; this decides how it
//! is shown.
//!
//! The pairing with an `EvidenceRecord` deliberately stays in `report.rs`:
//! `ReportFinding` is documented there as "never exported", so that these
//! findings cannot acquire a second, hand-set construction site.
//!
//! # The four decisions in the mapping, each with its reason
//!
//! 1. **`CertaintyTier::MathematicallyCertain`, always.** Everything the engine
//!    emits is Tier 0 by construction — enforced by the purity guard, not by a
//!    field — so there is no case where a lower tier would be right.
//!    §11 D156 settled that `CertaintyTier` is the shipping name for this axis.
//!
//! 2. **`EpistemicStatus` stays VISIBLE.** It is a different axis from tier and
//!    from severity (§11 D156), and collapsing it into severity would lose the
//!    distinction the whole decimal rule exists to preserve:
//!    `REQUIRES_AUTHOR_CONFIRMATION` is not a weaker `DETECTED`, it is a
//!    question. It appears in the title, in the provenance, and in the detail.
//!
//! 3. **Severity tracks the STATUS, not the tier.** `report.rs` already
//!    separates these two axes deliberately — a deterministic finding is not
//!    automatically Critical. `DETECTED` is `Major`: the arithmetic is wrong
//!    and no reading rescues it. `REQUIRES_AUTHOR_CONFIRMATION` is `Minor`: it
//!    may be nothing but the author has to say.
//!
//! 4. **The verification trail is carried, not summarised.** §6b requires a
//!    *"reviewer verification trail"*, and a trail that has been condensed is
//!    no longer one — the point is that a reader can re-do the arithmetic. It
//!    goes into `detail` verbatim, one step per line.

use crate::epistemic::EpistemicStatus;
use crate::equation::check::ArithmeticFinding;
use crate::equation::units::DimensionVerdict;
use crate::evidence::ClaimKind;
use crate::report::{CertaintyTier, Finding, FindingSeverity};
use crate::swarm::AgentKind;

/// Deterministic verdicts carry full confidence — the `k = 1.0` tier.
const DETERMINISTIC_CONF: f64 = 1.0;

/// Map an epistemic status to editorial urgency.
///
/// The two axes are kept apart on purpose: `Detected` and
/// `RequiresAuthorConfirmation` are equally CERTAIN (both are Tier 0) and not
/// equally urgent.
fn severity_for(status: EpistemicStatus) -> FindingSeverity {
    match status {
        EpistemicStatus::Detected | EpistemicStatus::Contradicted => FindingSeverity::Major,
        EpistemicStatus::RequiresAuthorConfirmation => FindingSeverity::Minor,
        _ => FindingSeverity::Info,
    }
}

/// Turn one Tier-0 arithmetic finding into a report finding.
///
/// Returns `None` for anything not reportable — `Confirmed` and `Unverified`
/// do not reach the author, because a report listing every check that passed
/// buries the ones that did not.
pub fn arithmetic_finding(
    f: &ArithmeticFinding,
    location: Option<crate::extract::Location>,
) -> Option<Finding> {
    if !f.is_reportable() {
        return None;
    }
    let mut provenance = vec![
        "signal:equation-arithmetic".to_string(),
        format!("epistemic:{}", status_slug(f.status)),
        format!("claim-index:{}", f.claim_index),
    ];
    for r in &f.readings {
        provenance.push(format!(
            "reading:{}={}",
            match r.reading {
                crate::epistemic::DecimalReading::AsWritten => "as_written",
                crate::epistemic::DecimalReading::AsRounded => "as_rounded",
            },
            if r.holds { "holds" } else { "fails" }
        ));
    }

    let mut detail = String::new();
    detail.push_str(&f.message);
    detail.push_str("\n\nLeft:  ");
    detail.push_str(&f.left.text);
    if let Some(v) = &f.left.value {
        detail.push_str(" = ");
        detail.push_str(v);
    }
    detail.push_str("\nRight: ");
    detail.push_str(&f.right.text);
    if let Some(v) = &f.right.value {
        detail.push_str(" = ");
        detail.push_str(v);
    }
    if !f.trail.is_empty() {
        detail.push_str("\n\nTo check this by hand:");
        for step in &f.trail {
            detail.push_str("\n  - ");
            detail.push_str(step);
        }
    }

    Some(Finding {
            // §12.1's GAP, CLOSED. The caller locates `source_line` with
            // `extract::locate_line`; `None` now means the line was not found
            // in exactly one paragraph, which is a decision rather than an
            // absence.
            location,
            claim: ClaimKind::ManuscriptDefect,
            severity: severity_for(f.status),
            // Tier 0 — see the module header. Never conditional.
            tier: CertaintyTier::MathematicallyCertain,
            certainty_label: CertaintyTier::MathematicallyCertain.label().into(),
            // The deterministic, LLM-free family, beside `validate.rs`.
            agent: AgentKind::ValidationMaths,
            title: format!(
                "Arithmetic {}: {}",
                f.status.label(),
                truncate(&f.source_line, EQUATION_LABEL_CHARS)
            ),
            detail,
        confidence: DETERMINISTIC_CONF,
        provenance,
    })
}

/// Turn a dimensional verdict into a report finding. Only `Inconsistent`
/// reports — a consistent or unverifiable one is not news.
pub fn dimension_finding(
    source_line: &str,
    v: &DimensionVerdict,
    location: Option<crate::extract::Location>,
) -> Option<Finding> {
    let DimensionVerdict::Inconsistent { left, right, detail } = v else {
        return None;
    };
    Some(Finding {
            // §12.1's GAP, CLOSED — see `arithmetic_finding`.
            location,
            claim: ClaimKind::ManuscriptDefect,
            severity: FindingSeverity::Major,
            tier: CertaintyTier::MathematicallyCertain,
            certainty_label: CertaintyTier::MathematicallyCertain.label().into(),
            agent: AgentKind::ValidationMaths,
            title: format!(
                "Units do not balance: {}",
                truncate(source_line, EQUATION_LABEL_CHARS)
            ),
            detail: format!(
                "{detail}\n\nLeft:  {left}\nRight: {right}\n\n\
                 A dimensional mismatch is arithmetic, not a matter of opinion — but it can \
                 also mean a constant's units were never stated in the text."
            ),
            confidence: DETERMINISTIC_CONF,
            provenance: vec![
                "signal:equation-dimensions".into(),
                format!("epistemic:{}", status_slug(EpistemicStatus::Detected)),
            ],
    })
}

/// How much of the equation to show in a title.
const EQUATION_LABEL_CHARS: usize = 90;

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n.saturating_sub(1)).collect();
    format!("{head}…")
}

fn status_slug(s: EpistemicStatus) -> &'static str {
    match s {
        EpistemicStatus::Detected => "detected",
        EpistemicStatus::Supported => "supported",
        EpistemicStatus::Confirmed => "confirmed",
        EpistemicStatus::Contradicted => "contradicted",
        EpistemicStatus::Unverified => "unverified",
        EpistemicStatus::RequiresAuthorConfirmation => "requires_author_confirmation",
    }
}
