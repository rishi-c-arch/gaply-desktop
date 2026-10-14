//! The Level 1 reproduction path for one run, WITHOUT Tauri (Phase 2, Step 4).
//!
//! The same kind of seam as Level 0's `run_stats_consistency_measured`:
//! a plain function an example binary or a test can call, so an instrument
//! measures the path a command will take. It takes the claims from the run
//! state, the data file and the (claim revision, token) pairs the user
//! confirmed, and returns `gaply_core`'s `ReproductionOutcome`. No network,
//! no model, no helper task, no database.
//!
//! The file is opened only after at least one claim's token opens the gate,
//! and read once for the whole run (`stats_l1_ingest::load_dataset`, which
//! fingerprints the bytes again; `compute` refuses a changed file).

use std::path::Path;

use gaply_core::stats_l0::method::Provenance;
use gaply_core::stats_l1::claim::RevisionHash;
use gaply_core::stats_l1::codes::{self, WireCode};
use gaply_core::stats_l1::compute::InputCounts;
use gaply_core::stats_l1::reproduce::{
    reproduce, CandidateComparison, ClaimReproduction, DataRead, L1Record, L1Verdict, NumberComparison, Printed, ReproductionOutcome,
};
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_l1::token::ConfirmationToken;

use crate::stats_l1_ingest::load_dataset;

/// Level 1 for every claim in `state`, on the data file at `path`.
#[doc(hidden)]
#[tracing::instrument(skip(state, path, pairs))]
pub fn run_stats_reproduction_measured(
    state: &L1RunState,
    path: &Path,
    pairs: &[(RevisionHash, ConfirmationToken)],
) -> ReproductionOutcome {
    reproduce(state, pairs, || load_dataset(path))
}

/* ------------------------------------------------------------------------ *
 *  The view: what crosses to the webview, and nothing else.
 * ------------------------------------------------------------------------ */

// `ReproductionOutcome` is deliberately not `Serialize`: it holds every printed
// number. This view is its one serialisable form, built by one function
// (`reproduction_view`), in the APP crate so that no `gaply_core` code can name
// it. It carries manuscript text for display on this device: each printed
// number exactly as printed. Never data: no cell, no level label, no header.
//
// The provenance record (`L1Record`) crosses as `ProvenanceView`, WITHOUT its
// wrapped `MethodCheck`'s `outcome` or `computed`: the outcome is a p
// comparison, not a verdict on the claim, and the view has no field that
// could be read as one.
//
// Every reason, note, status, variant and number kind is a closed code from
// `gaply_core::stats_l1::codes` (since Step 7; before, the enums' `Debug`
// names), the one code set the screen's wording table is keyed by. No ratio,
// percentage or score anywhere in it: counts only
// (`tests/stats_l1_reproduction.rs` checks every key).

/// Level 1 for one run, as a screen renders it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReproductionView {
    pub counts: ReproductionCountsView,
    /// `not_needed`, `read`, or `unreadable` with the code the file was
    /// refused with.
    pub data: WireCode,
    pub unmatched_pairs: usize,
    pub claims: Vec<ClaimReproductionView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReproductionCountsView {
    pub reproduced: usize,
    pub not_reproduced: usize,
    pub could_not_check: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimReproductionView {
    pub claim_id: usize,
    pub section_index: usize,
    pub paragraph: usize,
    pub sentence: usize,
    /// The Level 1 method id, when a binding was confirmed.
    pub method: Option<&'static str>,
    /// The binding's variant and who set it, as codes.
    pub variant: Option<WireCode>,
    pub variant_set_by: Option<WireCode>,
    pub verdict: L1VerdictView,
    pub notes: Vec<WireCode>,
    /// Every candidate variant, each printed number beside the computed one.
    pub candidates: Vec<CandidateView>,
    pub n_used: Option<usize>,
    pub rows_read: Option<usize>,
    pub exclusions: Vec<ExclusionView>,
    /// Where the result came from; `None` when nothing was computed.
    pub provenance: Option<ProvenanceView>,
}

/// The run's provenance: what computed it and from which binding. Never the
/// method check's `outcome` or `computed`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceView {
    pub method_id: &'static str,
    /// The publication a method follows; `None` for Gaply's own arithmetic.
    pub reference: Option<&'static str>,
    pub method_version: &'static str,
    pub engine_version: &'static str,
    /// SHA-256, hex, of the method's typed inputs. Never of text.
    pub inputs_digest: String,
    pub binding_source: WireCode,
    pub variant_set_by: WireCode,
}

/// The closed verdict set. A `notReproduced` never says which number is wrong.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum L1VerdictView {
    Reproduced { notes: Vec<WireCode> },
    NotReproduced { conclusion_changes: Option<bool> },
    CouldNotCheck { reason: WireCode },
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateView {
    pub variant: WireCode,
    pub numbers: Vec<NumberView>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumberView {
    pub kind: WireCode,
    /// Exactly as printed; `ns` for a p printed only as not significant.
    pub printed: String,
    /// `=`, `<`, `≤`, `>` or `≥`.
    pub relation: &'static str,
    pub computed: f64,
    /// What the computed value had to fall in. `None` for an open end.
    pub accepted_lo: Option<f64>,
    pub accepted_hi: Option<f64>,
    pub status: WireCode,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExclusionView {
    pub reason: WireCode,
    pub rows: usize,
}

fn word(code: &'static str) -> WireCode {
    WireCode { code, params: Default::default() }
}

fn provenance_view(r: &L1Record) -> ProvenanceView {
    ProvenanceView {
        method_id: r.check.method_id,
        reference: match r.check.provenance {
            Provenance::Published { reference } => Some(reference),
            Provenance::GaplyArithmetic => None,
        },
        method_version: r.method_version,
        engine_version: r.check.engine_version,
        inputs_digest: r.check.inputs_digest.clone(),
        binding_source: codes::binding_source(r.binding_source),
        variant_set_by: codes::variant_set_by(r.variant_set_by),
    }
}

fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}

fn number_view(n: &NumberComparison) -> NumberView {
    use gaply_core::stats_l0::number::Relation;
    let (printed, relation) = match &n.printed {
        Printed::Number(r) => (
            r.raw().to_string(),
            match r.relation() {
                Relation::Eq => "=",
                Relation::Lt => "<",
                Relation::Le => "≤",
                Relation::Gt => ">",
                Relation::Ge => "≥",
            },
        ),
        Printed::NotSignificant => ("ns".to_string(), "="),
    };
    NumberView {
        kind: codes::number_kind(n.kind),
        printed,
        relation,
        computed: n.computed,
        accepted_lo: finite(n.accepted.0),
        accepted_hi: finite(n.accepted.1),
        status: codes::number_status(n.status),
    }
}

fn candidate_view(c: &CandidateComparison) -> CandidateView {
    CandidateView { variant: codes::computed_variant(c.variant), numbers: c.numbers.iter().map(number_view).collect() }
}

fn counts_view(counts: Option<&InputCounts>) -> (Option<usize>, Option<usize>, Vec<ExclusionView>) {
    match counts {
        Some(c) => (
            Some(c.n_used),
            Some(c.rows_read),
            c.excluded.iter().map(|e| ExclusionView { reason: codes::exclusion_reason(e.reason), rows: e.rows }).collect(),
        ),
        None => (None, None, Vec::new()),
    }
}

fn claim_view(c: &ClaimReproduction) -> ClaimReproductionView {
    let verdict = match &c.verdict {
        L1Verdict::Reproduced(notes) => L1VerdictView::Reproduced { notes: notes.iter().map(|n| codes::l1_note(*n)).collect() },
        L1Verdict::NotReproduced { conclusion_changes } => L1VerdictView::NotReproduced { conclusion_changes: *conclusion_changes },
        L1Verdict::CouldNotCheck(why) => L1VerdictView::CouldNotCheck { reason: codes::l1_could_not_check(*why) },
    };
    let (n_used, rows_read, exclusions) = counts_view(c.counts.as_ref());
    ClaimReproductionView {
        claim_id: c.claim.0,
        section_index: c.location.section_index,
        paragraph: c.location.paragraph,
        sentence: c.location.sentence,
        method: c.method.map(|m| m.id()),
        variant: c.variant.map(|v| codes::variant(v.variant())),
        variant_set_by: c.variant.map(|v| codes::variant_set_by(v.set_by())),
        verdict,
        notes: c.notes.iter().map(|n| codes::l1_note(*n)).collect(),
        candidates: c.candidates.iter().map(candidate_view).collect(),
        n_used,
        rows_read,
        exclusions,
        provenance: c.record.as_ref().map(provenance_view),
    }
}

/// The one function that builds the view.
pub fn reproduction_view(outcome: &ReproductionOutcome) -> ReproductionView {
    ReproductionView {
        counts: ReproductionCountsView {
            reproduced: outcome.counts.reproduced,
            not_reproduced: outcome.counts.not_reproduced,
            could_not_check: outcome.counts.could_not_check,
        },
        data: match outcome.data {
            DataRead::NotNeeded => word("not_needed"),
            DataRead::Read => word("read"),
            DataRead::Unreadable(e) => {
                let mut c = word("unreadable");
                let inner = codes::l1_ingest_refusal(e);
                // The codes' own convention: a nested code with no parameters
                // is its bare word.
                let p = if inner.params.is_empty() { codes::Param::Word(inner.code) } else { codes::Param::Code(Box::new(inner)) };
                c.params.insert("refusal", p);
                c
            }
        },
        unmatched_pairs: outcome.unmatched_pairs,
        claims: outcome.claims.iter().map(claim_view).collect(),
    }
}
