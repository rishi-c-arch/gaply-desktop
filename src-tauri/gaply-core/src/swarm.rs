//! Round-table (mesh) swarm orchestration for the six Gaply agents.
//!
//! INTEGRATION DECISION (Prompt 19). ruv-swarm's Rust core (`ruv-swarm-core`
//! v1.0.6, MIT/Apache-2.0) was evaluated for direct embedding. Findings:
//!   * its `Agent` trait is `async fn process(...)` and the `std` feature pulls
//!     tokio + futures + dashmap — an async runtime into a deliberately
//!     synchronous crate (gaply_core agents are all sync functions);
//!   * it raises our MSRV 1.77.2 → 1.85;
//!   * it provides agent lifecycle / topology / task-dispatch primitives, but
//!     NONE of what this feature actually specifies — debate rounds, confidence
//!     rescaling, weighted voting, hard constraints, or gate filtering. All of
//!     that would be our code regardless.
//! Per the dependency-light principle (Prompts 16–17), we therefore implement
//! ruv-swarm's mesh/round-table SEMANTICS natively and keep [`SwarmAgent`] as
//! the seam: it maps 1:1 onto `ruv_swarm_core::Agent` (kind→id, opine→process,
//! mesh round-table→`Topology::mesh`) if the app later goes multi-process and
//! wants the full runtime. CPU-native, no GPU, fully offline for the five local
//! agents; only the Verification agent involves the network, and only through
//! the existing proxy seam ([`crate::verify_agent::ProxyClient`]).
//!
//! Protocol per debate:
//!   1. every agent produces an [`Opinion`] (answer + explanation + confidence);
//!   2. opinions whose OWN internal gate failed (e.g. the Prompt-17 harness
//!      gate) are rejected before entering the round-table;
//!   3. up to [`MAX_ROUNDS`] discussion rounds: each agent sees all other
//!      current opinions (mesh) and may revise; a round with zero revisions
//!      terminates the debate early (converged);
//!   4. consensus: confidences are RESCALED per agent kind (heuristic/LLM
//!      agents shrink toward 0.5 — they run overconfident), then votes are
//!      weighted by rescaled confidence;
//!   5. HARD CONSTRAINTS: the Validation/Maths agent's deterministic verdicts
//!      are never subject to the vote — a hard-constraint opinion overrides any
//!      soft consensus, and the override is recorded.

use serde::{Deserialize, Serialize};

use crate::GaplyError;

/// Absolute ceiling on discussion rounds — forced termination.
pub const MAX_ROUNDS: usize = 3;

// ============================================================================
// Agents + opinions
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Extraction,
    ValidationMaths,
    AiDetection,
    Plagiarism,
    Rag,
    Verification,
}

impl AgentKind {
    /// Only the Verification agent ever touches the network (via the proxy).
    pub fn requires_network(&self) -> bool {
        matches!(self, AgentKind::Verification)
    }

    /// Confidence calibration factor: how much of an agent's self-reported
    /// deviation from 0.5 we believe. Deterministic agents keep it all;
    /// heuristic/LLM-backed agents (AI-Detection, Verification) are shrunk
    /// hardest because such scores run overconfident.
    fn calibration(&self) -> f64 {
        match self {
            AgentKind::ValidationMaths => 1.0, // deterministic (hard-constrained anyway)
            AgentKind::Extraction => 0.9,      // mechanical parsing
            AgentKind::Plagiarism => 0.8,      // embedding similarity
            AgentKind::Rag => 0.8,             // embedding retrieval
            AgentKind::AiDetection => 0.6,     // statistical proxy, explicit low confidence
            AgentKind::Verification => 0.6,    // LLM-derived confidences
        }
    }
}

/// One agent's position in the debate.
#[derive(Debug, Clone, Serialize)]
pub struct Opinion {
    pub agent: AgentKind,
    /// Categorical stance; equal strings vote together (e.g. "pass"/"concern").
    pub answer: String,
    pub explanation: String,
    /// Raw self-reported confidence in [0, 1] (rescaled before voting).
    pub confidence: f64,
    /// Deterministic verdict — overrides consensus, never voted on.
    pub hard_constraint: bool,
    /// Result of the agent's OWN internal gate (e.g. Prompt-17 harness gate).
    /// `false` ⇒ the opinion is rejected before the debate.
    pub gate_passed: bool,
}

/// A debate participant. Constructed pre-bound to its input; `opine` yields the
/// initial position, `revise` may update it after seeing the round-table.
pub trait SwarmAgent {
    fn kind(&self) -> AgentKind;
    fn opine(&mut self) -> Result<Opinion, GaplyError>;
    /// Mesh step: sees every other current opinion. Return `Some(new)` to
    /// change position, `None` to hold. Default: hold.
    fn revise(&mut self, _own: &Opinion, _others: &[Opinion], _round: usize) -> Option<Opinion> {
        None
    }
}

/// **A `&mut` to an agent is an agent.**
///
/// `run_debate` takes `&mut [Box<dyn SwarmAgent>]`, which means every
/// participant is normally MOVED into the vector and unreachable afterwards.
/// That is fine for [`PrecomputedAgent`], whose opinion never changes — but
/// [`RevisingVerificationAgent`] holds the verification report and REVISES it
/// during the debate, and the caller needs the revised one: `compile_report`
/// builds the per-citation findings from that report, so a caller that kept the
/// pre-debate copy would render findings that contradict its own
/// `revised_agents` summary.
///
/// With this impl the caller keeps ownership, boxes a `&mut`, and reads
/// `reviser.report()` once the debate returns. The alternative was a second
/// `reconsider_citations` call to rebuild the report — which is what the test
/// in `report/tests.rs` does and is acceptable there, but in production would
/// mean paying for the same cloud round-trip twice and hoping the two agreed.
impl<T: SwarmAgent + ?Sized> SwarmAgent for &mut T {
    fn kind(&self) -> AgentKind {
        (**self).kind()
    }
    fn opine(&mut self) -> Result<Opinion, GaplyError> {
        (**self).opine()
    }
    fn revise(&mut self, own: &Opinion, others: &[Opinion], round: usize) -> Option<Opinion> {
        (**self).revise(own, others, round)
    }
}

/// The simplest agent: a fixed, precomputed opinion (adapters below produce
/// these from the real agent reports). Never revises.
pub struct PrecomputedAgent {
    opinion: Opinion,
}

impl PrecomputedAgent {
    pub fn new(opinion: Opinion) -> Self {
        Self { opinion }
    }
}

impl SwarmAgent for PrecomputedAgent {
    fn kind(&self) -> AgentKind {
        self.opinion.agent
    }
    fn opine(&mut self) -> Result<Opinion, GaplyError> {
        Ok(self.opinion.clone())
    }
}

// ============================================================================
// Confidence rescaling + voting
// ============================================================================

/// Shrink a raw confidence toward 0.5 by the agent's calibration factor:
/// `0.5 + (raw - 0.5) * k`. 0.5 is a fixed point; deterministic agents (k=1)
/// pass through unchanged; heuristic agents lose most of their overconfidence.
pub fn rescale_confidence(kind: AgentKind, raw: f64) -> f64 {
    let raw = raw.clamp(0.0, 1.0);
    0.5 + (raw - 0.5) * kind.calibration()
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsensusResult {
    /// The winning answer (hard constraint if present, else weighted vote).
    pub answer: String,
    /// Winner's share of total rescaled weight (1.0 for a hard constraint).
    pub combined_confidence: f64,
    /// Rescaled weight each (non-hard) agent contributed.
    pub weights: Vec<(AgentKind, f64)>,
    /// True when a hard constraint displaced a different soft-vote winner.
    pub overridden_by_constraint: bool,
    pub constraint_note: Option<String>,
}

/// Confidence-weighted vote with hard-constraint override.
fn consensus(opinions: &[Opinion]) -> Result<ConsensusResult, GaplyError> {
    // --- soft vote over non-hard opinions -----------------------------------
    let soft: Vec<&Opinion> = opinions.iter().filter(|o| !o.hard_constraint).collect();
    let weights: Vec<(AgentKind, f64)> = soft
        .iter()
        .map(|o| (o.agent, rescale_confidence(o.agent, o.confidence)))
        .collect();
    let mut tally: Vec<(String, f64)> = Vec::new();
    for (o, (_, w)) in soft.iter().zip(&weights) {
        match tally.iter_mut().find(|(a, _)| *a == o.answer) {
            Some((_, sum)) => *sum += w,
            None => tally.push((o.answer.clone(), *w)),
        }
    }
    let total: f64 = tally.iter().map(|(_, w)| w).sum();
    let soft_winner = tally
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).expect("weights are finite"))
        .cloned();

    // --- hard constraints override everything --------------------------------
    let hard: Vec<&Opinion> = opinions.iter().filter(|o| o.hard_constraint).collect();
    if !hard.is_empty() {
        let answer = hard[0].answer.clone();
        if hard.iter().any(|h| h.answer != answer) {
            return Err(GaplyError::Conflict(
                "conflicting hard-constraint verdicts in the same debate".into(),
            ));
        }
        let overridden = soft_winner.as_ref().map(|(a, _)| *a != answer).unwrap_or(false);
        return Ok(ConsensusResult {
            answer,
            combined_confidence: 1.0,
            weights,
            overridden_by_constraint: overridden,
            constraint_note: Some(format!(
                "deterministic {:?} verdict is a hard constraint; not subject to voting{}",
                hard[0].agent,
                if overridden { " (soft majority disagreed and was overridden)" } else { "" }
            )),
        });
    }

    let (answer, winner_weight) = soft_winner
        .ok_or_else(|| GaplyError::Validation("consensus over zero admitted opinions".into()))?;
    Ok(ConsensusResult {
        answer,
        combined_confidence: if total > 0.0 { winner_weight / total } else { 0.0 },
        weights,
        overridden_by_constraint: false,
        constraint_note: None,
    })
}

// ============================================================================
// The debate
// ============================================================================

#[derive(Debug, Clone)]
pub struct DebateConfig {
    /// Requested rounds; always clamped to [`MAX_ROUNDS`].
    pub max_rounds: usize,
}

impl Default for DebateConfig {
    fn default() -> Self {
        Self { max_rounds: MAX_ROUNDS }
    }
}

#[derive(Debug, Serialize)]
pub struct DebateOutcome {
    /// Final (post-discussion) opinions that were admitted to the round-table.
    pub opinions: Vec<Opinion>,
    /// Opinions rejected before the debate because their own gate failed.
    pub rejected: Vec<Opinion>,
    /// Discussion rounds actually executed (0 if only one agent, ≤ MAX_ROUNDS).
    pub rounds_run: usize,
    /// True when a round completed with zero revisions (early termination).
    pub converged: bool,
    /// Agents that revised their opinion during the debate (e.g. a
    /// RevisingVerificationAgent reconsideration) — the report compiler uses
    /// this to assign the "reconsidered after peer review" certainty tier.
    pub revised_agents: Vec<AgentKind>,
    pub result: ConsensusResult,
}

/// Run the round-table debate to consensus. (Agents may borrow — e.g. the
/// RevisingVerificationAgent holds a proxy reference — hence the `+ 'a`.)
pub fn run_debate<'a>(
    agents: &mut [Box<dyn SwarmAgent + 'a>],
    config: &DebateConfig,
) -> Result<DebateOutcome, GaplyError> {
    if agents.is_empty() {
        return Err(GaplyError::Validation("debate needs at least one agent".into()));
    }
    let max_rounds = config.max_rounds.min(MAX_ROUNDS);

    // (1) initial opinions, (2) gate filter BEFORE the round-table.
    let mut admitted: Vec<(usize, Opinion)> = Vec::new();
    let mut rejected: Vec<Opinion> = Vec::new();
    for (i, agent) in agents.iter_mut().enumerate() {
        let op = agent.opine()?;
        if op.gate_passed {
            admitted.push((i, op));
        } else {
            tracing::warn!(agent = ?op.agent, "opinion rejected: internal gate failed");
            rejected.push(op);
        }
    }
    if admitted.is_empty() {
        return Err(GaplyError::Validation(
            "no opinion survived its internal gate; nothing to debate".into(),
        ));
    }

    // (3) mesh discussion rounds — everyone sees everyone; ≤ MAX_ROUNDS.
    let mut rounds_run = 0;
    let mut converged = false;
    let mut revised_agents: Vec<AgentKind> = Vec::new();
    while rounds_run < max_rounds {
        rounds_run += 1;
        let snapshot: Vec<Opinion> = admitted.iter().map(|(_, o)| o.clone()).collect();
        let mut any_revision = false;
        for (slot, (agent_idx, own)) in admitted.iter_mut().enumerate() {
            let others: Vec<Opinion> = snapshot
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != slot)
                .map(|(_, o)| o.clone())
                .collect();
            if let Some(mut new) = agents[*agent_idx].revise(own, &others, rounds_run) {
                // Revisions cannot smuggle in privileges the original lacked.
                new.agent = own.agent;
                new.hard_constraint = own.hard_constraint;
                new.gate_passed = own.gate_passed;
                if !revised_agents.contains(&new.agent) {
                    revised_agents.push(new.agent);
                }
                *own = new;
                any_revision = true;
            }
        }
        if !any_revision {
            converged = true;
            break;
        }
    }

    // (4)+(5) consensus with rescaled weights and hard-constraint override.
    let opinions: Vec<Opinion> = admitted.into_iter().map(|(_, o)| o).collect();
    let result = consensus(&opinions)?;
    Ok(DebateOutcome { opinions, rejected, rounds_run, converged, revised_agents, result })
}

// ============================================================================
// Adapters: real agent reports → opinions
// ============================================================================

/// Shared answer vocabulary for the integrity round-table.
pub const ANSWER_PASS: &str = "pass";
pub const ANSWER_CONCERN: &str = "concern";

pub mod adapters {
    use super::*;
    use crate::ai_detect::{AiDetectionReport, AiSignal};
    use crate::extract::ExtractionResult;
    use crate::plagiarism::PlagiarismReport;
    use crate::rag::RagHit;
    use crate::validate::StatsValidityReport;
    use crate::verify_agent::{VerificationReport, Verdict};

    /// Extraction: mechanical; high confidence that its own output is faithful.
    pub fn from_extraction(r: &ExtractionResult) -> Opinion {
        let n_stats = r.statistics.len();
        let n_refs = r.references.len();
        Opinion {
            agent: AgentKind::Extraction,
            answer: ANSWER_PASS.into(),
            explanation: format!(
                "extracted {n_stats} statistical claim(s) and {n_refs} reference(s) without parse anomalies"
            ),
            confidence: 0.9,
            hard_constraint: false,
            gate_passed: true,
        }
    }

    /// Validation/Maths: DETERMINISTIC — always a hard constraint.
    pub fn from_validation(r: &StatsValidityReport) -> Opinion {
        Opinion {
            agent: AgentKind::ValidationMaths,
            answer: if r.passed { ANSWER_PASS.into() } else { ANSWER_CONCERN.into() },
            explanation: if r.passed {
                "all deterministic statistical rules passed".into()
            } else {
                format!("{} deterministic rule flag(s): mathematically certain", r.flags.len())
            },
            confidence: 1.0,
            hard_constraint: true, // never subject to voting
            gate_passed: true,
        }
    }

    /// AI-Detection: statistical proxy; its report says "low" confidence — honor that.
    ///
    /// The explanation now names WHICH TIER produced the numbers and states that
    /// the lane has no measured accuracy (§11 D153). Severity is capped
    /// elsewhere, in `report.rs`, because it is a property of the CLAIM rather
    /// than of this adapter.
    pub fn from_ai_detection(r: &AiDetectionReport) -> Opinion {
        let (answer, conf) = match r.signal {
            AiSignal::LeansHumanLike => (ANSWER_PASS, 0.6),
            AiSignal::Inconclusive => (ANSWER_PASS, 0.5),
            AiSignal::LeansAiLike => (ANSWER_CONCERN, 0.6),
        };
        Opinion {
            agent: AgentKind::AiDetection,
            answer: answer.into(),
            explanation: format!(
                "signal {:?} (mean perplexity {:.1}, burstiness {:.1}); {} {}",
                r.signal,
                r.overall_mean_perplexity,
                r.overall_burstiness,
                crate::ai_detect::tier_provenance(r.deep_kind),
                r.disclaimer
            ),
            confidence: conf,
            hard_constraint: false,
            gate_passed: true,
        }
    }

    /// Plagiarism: similarity evidence; confidence follows the strongest match.
    pub fn from_plagiarism(r: &PlagiarismReport) -> Opinion {
        let strongest = r
            .corpus_matches
            .iter()
            .chain(&r.self_matches)
            .map(|m| m.similarity)
            .fold(0.0_f64, f64::max);
        let matched = strongest >= r.threshold;
        Opinion {
            agent: AgentKind::Plagiarism,
            answer: if matched { ANSWER_CONCERN.into() } else { ANSWER_PASS.into() },
            explanation: format!(
                "{} corpus / {} self match(es) at threshold {:.2}; {}",
                r.corpus_matches.len(),
                r.self_matches.len(),
                r.threshold,
                r.note
            ),
            confidence: if matched { strongest } else { 0.7 },
            hard_constraint: false,
            gate_passed: true,
        }
    }

    /// RAG: retrieval context. Quarantined-only or empty retrieval lowers confidence.
    pub fn from_rag_hits(hits: &[RagHit]) -> Opinion {
        Opinion {
            agent: AgentKind::Rag,
            answer: ANSWER_PASS.into(),
            explanation: format!("{} provenance-tagged context hit(s) retrieved", hits.len()),
            // Real, distance-ordered confidence (Step 0a) — was a hardcoded 0.75.
            // Reflects actual retrieval quality; empty retrieval → 0.5 (unchanged).
            confidence: crate::rag::rag_confidence(hits),
            hard_constraint: false,
            gate_passed: true,
        }
    }

    /// Verification: verdicts already passed the Prompt-17 harness gate — but if
    /// any verdict carries gate flags (potential hallucination / downgrade), the
    /// OPINION fails its internal gate here and is rejected from the debate.
    pub fn from_verification_report(r: &VerificationReport) -> Opinion {
        let gate_passed = r.verdicts.iter().all(|v| v.gate_flags.is_empty());
        let refuted = r.verdicts.iter().filter(|v| v.verdict == Verdict::Refuted).count();
        let unknown = r.verdicts.iter().filter(|v| v.verdict == Verdict::Unknown).count();
        let mean_conf = if r.verdicts.is_empty() {
            0.0
        } else {
            r.verdicts.iter().map(|v| v.confidence).sum::<f64>() / r.verdicts.len() as f64
        };
        Opinion {
            agent: AgentKind::Verification,
            answer: if refuted > 0 { ANSWER_CONCERN.into() } else { ANSWER_PASS.into() },
            explanation: format!(
                "{} citation verdict(s): {refuted} refuted, {unknown} unknown; harness-gated",
                r.verdicts.len()
            ),
            confidence: mean_conf,
            hard_constraint: false,
            gate_passed,
        }
    }
}

mod revising;
pub use revising::RevisingVerificationAgent;

#[cfg(test)]
mod tests;
