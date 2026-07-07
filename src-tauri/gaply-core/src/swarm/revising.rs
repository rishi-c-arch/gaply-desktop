//! Peer-informed reconsideration for the Verification agent (ReConcile-style).
//!
//! DESIGN BOUNDARY (confirmed): only the Verification agent gets revision
//! capability. The other five agents stay [`PrecomputedAgent`]s — their outputs
//! are MEASUREMENTS (deterministic rules, parses, similarity scores, perplexity
//! statistics), not beliefs; a validator that changed its answer under peer
//! pressure would be broken, not collaborative. Verification's verdicts are
//! LLM-derived judgments, so peer-informed reconsideration is meaningful there
//! and only there.
//!
//! Budget: the reconsideration happens INSIDE `run_debate`'s round loop (it is
//! one of the ≤ [`MAX_ROUNDS`](super::MAX_ROUNDS) discussion rounds, not an
//! extra loop) and fires at most once per debate.

use crate::extract::citations::Reference;
use crate::refverify::ReferenceVerification;
use crate::verify_agent::{
    reconsider_citations, PeerFinding, ProxyClient, VerificationReport,
};
use crate::GaplyError;

use super::{adapters, AgentKind, Opinion, SwarmAgent};

/// A Verification agent that can reconsider its verdicts when the round-table
/// contradicts it. Wraps the proxy: reconsideration is a SECOND proxy round,
/// built by [`reconsider_citations`] and therefore harness-gated exactly like
/// the first call (gates 1+2) plus the revision-grounding gate (gate 3).
pub struct RevisingVerificationAgent<'a> {
    proxy: &'a dyn ProxyClient,
    /// The same (reference, evidence) pairs the initial report was built from —
    /// reused so the reconsideration bundle (and thus the gate) is identical.
    items: Vec<(Reference, ReferenceVerification)>,
    report: VerificationReport,
    reconsidered: bool,
}

impl<'a> RevisingVerificationAgent<'a> {
    pub fn new(
        proxy: &'a dyn ProxyClient,
        items: Vec<(Reference, ReferenceVerification)>,
        initial_report: VerificationReport,
    ) -> Self {
        Self { proxy, items, report: initial_report, reconsidered: false }
    }

    /// The current (possibly revised) verification report.
    pub fn report(&self) -> &VerificationReport {
        &self.report
    }
}

impl SwarmAgent for RevisingVerificationAgent<'_> {
    fn kind(&self) -> AgentKind {
        AgentKind::Verification
    }

    fn opine(&mut self) -> Result<Opinion, GaplyError> {
        Ok(adapters::from_verification_report(&self.report))
    }

    fn revise(&mut self, own: &Opinion, others: &[Opinion], round: usize) -> Option<Opinion> {
        // At most one reconsideration per debate — this spends one of the
        // (≤ 3) rounds; it is not an unbounded extra loop.
        if self.reconsidered {
            return None;
        }
        // Trigger: a peer whose stance contradicts ours (e.g. Plagiarism
        // surfacing a high-similarity match while we said pass).
        let contradicting: Vec<&Opinion> =
            others.iter().filter(|o| o.answer != own.answer).collect();
        if contradicting.is_empty() {
            return None;
        }
        self.reconsidered = true;

        let findings: Vec<PeerFinding> = contradicting
            .iter()
            .map(|o| PeerFinding {
                agent: format!("{:?}", o.agent),
                answer: o.answer.clone(),
                summary: o.explanation.clone(),
                confidence: o.confidence,
            })
            .collect();

        match reconsider_citations(self.proxy, &self.items, &self.report, &findings) {
            Ok(new_report) => {
                let new_op = adapters::from_verification_report(&new_report);
                self.report = new_report;
                tracing::info!(
                    round,
                    peers = findings.len(),
                    "verification agent reconsidered its verdicts"
                );
                // Hold if the gated reconsideration changed nothing observable.
                if new_op.answer == own.answer && (new_op.confidence - own.confidence).abs() < 1e-9
                {
                    None
                } else {
                    Some(new_op)
                }
            }
            Err(e) => {
                // A failed reconsideration must never lose the gated original.
                tracing::warn!(error = %e, "reconsideration failed; holding prior verdicts");
                None
            }
        }
    }
}
