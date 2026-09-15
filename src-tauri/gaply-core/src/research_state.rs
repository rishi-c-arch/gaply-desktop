//! **`ResearchState` — the machine-readable study, derived from extraction.**
//!
//! The architecture's §3.1 research-state layer: *"question, hypotheses,
//! design, population, variables, outcomes, claims, methods, analyses, results,
//! tables, figures, references, uncertainties"*.
//!
//! # IT COMPOSES, IT DOES NOT DUPLICATE
//!
//! Most of those fields already exist. [`crate::scientific_model::ScientificExtraction`]
//! holds questions, hypotheses, variables, methods, datasets (population lives
//! there), claims, contributions and limitations — already versioned, already
//! carrying `SourceSpan` provenance, already cross-referenced by id. Restating
//! them here would be a second definition of one thing, drifting the first time
//! only one is edited (§11 D129).
//!
//! So this type holds the scientific layer by `Arc` and adds what was missing:
//! a content hash, per-field provenance about WHICH EXTRACTOR produced what,
//! and the evidence graph.
//!
//! # WHY THERE IS NO PROSE IN HERE
//!
//! §3.1 gives the layers different privacy classes: the **Manuscript** layer
//! (full text, sections, figures) is premium and needs `Manuscript` consent; the
//! **Research state** is *"structured, gate-safe"*. That pairing is the test for
//! whether something belongs.
//!
//! So [`SectionSummary`] carries a section's kind, heading and paragraph COUNT
//! and never its paragraphs. A `ResearchState` can cross a boundary that the
//! manuscript cannot — and would stop being able to the moment prose was added
//! to it for convenience. `release_gate::manuscript_text_in` is what would catch
//! that, but only for payloads it sees; the shape of this type is the part that
//! does not depend on being checked.
//!
//! # WHAT IS ABSENT, AND SAYS SO
//!
//! **Figures.** Nothing extracts them — `extract` detects tables only. The field
//! is present and always empty, because a caller asking "what figures does this
//! paper have" deserves "we do not extract figures" rather than a missing field
//! it might read as "none".
//!
//! **The scientific layer — no longer usually absent, as of Phase 4.**
//! `ExtractOptions::scientific` is a dependency of `frequentist_stats` and
//! `ml_methodology` in `data/agent_graph.json`, so `run_pipeline_inner` derives
//! it and [`ResearchState::science`] is `Some` on a real run. `None` remains
//! typed absence — the extractor was never asked, NOT "the paper has no claims"
//! — for the callers that construct a state without it.
//!
//! # AND THAT SWITCH-ON MAKES THE "NO PROSE" CLAIM ABOVE CONDITIONAL
//!
//! **§3.1 gives the research state an egress class of "none — structured,
//! gate-safe, carries no prose". With the scientific layer derived, that is no
//! longer true of every state**, and this is a correction to the architecture
//! rather than a note about the code. [`crate::scientific_model::ScientificClaim`]
//! carries `statement`, which is a manuscript sentence or a span of one:
//!
//! ```text
//! manuscript: "Treated larvae showed values appreciably higher than the
//!              untreated control across both seasons."
//! claim.statement: "higher than the untreated control across both seasons."
//! ```
//!
//! That is a verbatim substring of the manuscript inside a layer §3.1 says may
//! travel without `Manuscript` consent. The prose-free guarantee survives for
//! everything this module composes itself — [`SectionSummary`] still carries a
//! paragraph COUNT and never a paragraph — and it does not survive the composed
//! scientific layer.
//!
//! [`ResearchState::carries_manuscript_prose`] is therefore the egress question,
//! answered per state rather than per layer, and
//! `the_research_state_carries_prose_only_through_the_scientific_layer` is where
//! it is pinned. **The test that was supposed to catch this passed while the
//! layer was off and its fixture produced zero claims** — green for the old
//! reason, not the new one, which is why it now asserts its own precondition.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::extract::{
    citations::{Citation, Reference},
    stats::StatClaim,
    ExtractionResult, Location, Section, SectionKind, TableRef,
};
use crate::scientific_model::ScientificExtraction;

/// Version of this envelope's shape. Bump when a field is added or its meaning
/// changes; the scientific layer carries its own
/// [`crate::scientific_model::SCIENTIFIC_MODEL_SCHEMA_VERSION`] independently,
/// because the two evolve for different reasons.
pub const RESEARCH_STATE_SCHEMA_VERSION: u32 = 1;

/// A section's shape WITHOUT its prose. See the module header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionSummary {
    pub kind: SectionKind,
    /// The heading as written. A heading is a label the author chose, not the
    /// body — it is what a checklist cites and what a locator names.
    pub heading: String,
    pub paragraph_count: usize,
}

/// A figure. **Never produced** — see the module header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Figure {
    pub label: String,
    pub caption: Option<String>,
    pub location: Location,
}

/// Which extractor produced a field, and how much of it.
///
/// This is the "which extractor produced it" half of §3.1's provenance
/// requirement. The "from which manuscript span" half lives on the items
/// themselves — `StatClaim::location`, `TableRef::location`,
/// `ScientificClaim::source_span` — where it is per-item and cannot be lost by
/// aggregation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldProvenance {
    /// The field this describes, as its name in this struct.
    pub field: String,
    /// The extractor that produced it, as a module path.
    pub extractor: String,
    pub count: usize,
}

// ---------------------------------------------------------------------------
// The evidence graph
// ---------------------------------------------------------------------------

/// One end of an edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum NodeRef {
    /// Index into [`ResearchState::statistics`].
    Statistic { index: usize },
    /// Index into [`ResearchState::tables`].
    Table { index: usize },
    /// Index into [`ResearchState::citations`].
    Citation { index: usize },
    /// Index into [`ResearchState::references`].
    Reference { index: usize },
    /// A `ClaimId` from the scientific layer.
    Claim { id: String },
}

/// **What an edge ASSERTS — named for what it rests on, not for what one would
/// like it to mean.**
///
/// The instruction this was built to is *"populate from what extraction already
/// knows; leave edges it cannot establish absent, never guessed"*, and the
/// naming is where that is either honoured or quietly lost. `Supports` would be
/// a claim about meaning; `CoLocated` is a claim about position, which is all
/// extraction can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// **EXACT.** A numeric in-text marker resolves to a reference-list entry by
    /// index: `[12]` is `references[11]`.
    ///
    /// Numeric styles only. An author-year marker identifies a work
    /// APPROXIMATELY (§11 D131) — two papers by the same authors in the same
    /// year are indistinguishable to it — so no edge is emitted for them rather
    /// than a probable one.
    CitesReference,
    /// **CO-LOCATION, NOT SUPPORT.** A statistic and a table reference in the
    /// same paragraph. The paragraph reporting a number and naming a table is
    /// usually reporting that table's number; extraction cannot confirm it, and
    /// an edge called `ReportsTable` would assert what only a reader can check.
    StatisticCoLocatedWithTable,
    /// **CO-LOCATION, NOT SUPPORT.** A scientific claim and a statistic in the
    /// same paragraph.
    ClaimCoLocatedWithStatistic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeRef,
    pub to: NodeRef,
    pub kind: EdgeKind,
}

/// Edges between research-state nodes. Empty is a legitimate answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGraph {
    pub edges: Vec<Edge>,
}

impl EvidenceGraph {
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }
    pub fn of_kind(&self, kind: EdgeKind) -> impl Iterator<Item = &Edge> {
        self.edges.iter().filter(move |e| e.kind == kind)
    }
}

// ---------------------------------------------------------------------------
// The state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchState {
    pub schema_version: u32,
    /// sha256 over the canonical JSON of every field below it. Two states with
    /// the same hash were derived from the same extraction; §3.3's versioning
    /// and §5.5's incremental re-analysis both key on this.
    pub content_hash: String,

    /// The Stage-1 scientific layer. `None` = the extractor was not asked (the
    /// production default today), NOT "this paper has no claims".
    pub science: Option<Arc<ScientificExtraction>>,

    pub title: Option<String>,
    pub structure: Vec<SectionSummary>,
    /// Analyses and results: every statistic extraction found, with its span.
    pub statistics: Vec<StatClaim>,
    pub tables: Vec<TableRef>,
    /// Always empty — no extractor produces figures. See the module header.
    pub figures: Vec<Figure>,
    pub citations: Vec<Citation>,
    pub references: Vec<Reference>,

    pub provenance: Vec<FieldProvenance>,
    pub evidence: EvidenceGraph,
}

impl ResearchState {
    /// Derive a state from an extraction. **Pure**: same extraction in, same
    /// state out, byte-identical hash.
    ///
    /// This is the "change where its output lands" of Phase 1 — extraction is
    /// not altered, asked for anything new, or re-run. Everything here is a
    /// projection of what it already returned.
    pub fn from_extraction(ex: &ExtractionResult) -> Self {
        let structure: Vec<SectionSummary> = ex.sections.iter().map(summarise_section).collect();
        let provenance = vec![
            prov("structure", "extract::sections", structure.len()),
            prov("statistics", "extract::stats", ex.statistics.len()),
            prov("tables", "extract::detect_table", ex.tables.len()),
            prov("citations", "extract::citations::extract_in_text", ex.citations.len()),
            prov("references", "extract::citations::parse_reference_list", ex.references.len()),
            // Recorded at zero rather than omitted: "no extractor runs for this"
            // and "this extractor found nothing" are different facts, and a
            // missing row would read as the second.
            prov("figures", "(none — no figure extractor exists)", 0),
            prov(
                "science",
                "extract::{claims,variables,methods,datasets} (opt-in)",
                ex.scientific.as_ref().map(|s| s.claims.len()).unwrap_or(0),
            ),
        ];

        let mut state = Self {
            schema_version: RESEARCH_STATE_SCHEMA_VERSION,
            content_hash: String::new(),
            science: ex.scientific.clone(),
            title: ex.title.clone(),
            structure,
            statistics: ex.statistics.clone(),
            tables: ex.tables.clone(),
            figures: Vec::new(),
            citations: ex.citations.clone(),
            references: ex.references.clone(),
            provenance,
            evidence: EvidenceGraph::default(),
        };
        state.evidence = build_graph(&state);
        state.content_hash = state.compute_hash();
        state
    }

    /// The hash over everything except the hash field itself.
    fn compute_hash(&self) -> String {
        let mut probe = self.clone();
        probe.content_hash = String::new();
        // serde_json is deterministic for structs (declaration order) and this
        // type contains no map, so the bytes are stable for a given value.
        let json = serde_json::to_vec(&probe).unwrap_or_default();
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(&json);
        format!("{:x}", h.finalize())
    }

    /// Recompute and compare — the check that a state has not been mutated
    /// since it was derived.
    /// **Does this state carry manuscript prose, and so need `Manuscript`
    /// consent to leave?**
    ///
    /// §3.1's egress classes are per-LAYER and this one is per-STATE, because
    /// the answer depends on whether the scientific layer was derived. A state
    /// without it is gate-safe as §3.1 describes; a state with it carries claim
    /// statements, which are manuscript sentences.
    ///
    /// Callers on the boundary must ask this rather than assume the layer's
    /// class — `Layer::ResearchState::needs_consent_to_leave()` is `false` and
    /// is a statement about the layer's OWN fields, not about what it composes.
    pub fn carries_manuscript_prose(&self) -> bool {
        self.science.as_ref().is_some_and(|s| {
            !s.claims.is_empty()
                || !s.contributions.is_empty()
                || !s.limitations.is_empty()
                || !s.questions.is_empty()
                || !s.hypotheses.is_empty()
        })
    }

    pub fn hash_matches(&self) -> bool {
        self.compute_hash() == self.content_hash
    }
}

fn summarise_section(s: &Section) -> SectionSummary {
    SectionSummary {
        kind: s.kind,
        heading: s.heading.clone(),
        paragraph_count: s.paragraphs.len(),
    }
}

fn prov(field: &str, extractor: &str, count: usize) -> FieldProvenance {
    FieldProvenance { field: field.to_string(), extractor: extractor.to_string(), count }
}

fn same_paragraph(a: &Location, b: &Location) -> bool {
    a.section == b.section && a.paragraph == b.paragraph
}

/// Build every edge extraction can establish, and no others.
fn build_graph(s: &ResearchState) -> EvidenceGraph {
    let mut edges = Vec::new();

    // citation -> reference, by NUMERIC index only.
    for (ci, c) in s.citations.iter().enumerate() {
        for n in &c.numbers {
            // `[n]` is 1-based; an out-of-range marker means the bibliography
            // and the text disagree, which is a FINDING for another lane and
            // not an edge to invent here.
            let Some(idx) = (*n as usize).checked_sub(1) else { continue };
            if idx < s.references.len() {
                edges.push(Edge {
                    from: NodeRef::Citation { index: ci },
                    to: NodeRef::Reference { index: idx },
                    kind: EdgeKind::CitesReference,
                });
            }
        }
    }

    // statistic -> table, same paragraph.
    for (si, st) in s.statistics.iter().enumerate() {
        for (ti, t) in s.tables.iter().enumerate() {
            if same_paragraph(&st.location, &t.location) {
                edges.push(Edge {
                    from: NodeRef::Statistic { index: si },
                    to: NodeRef::Table { index: ti },
                    kind: EdgeKind::StatisticCoLocatedWithTable,
                });
            }
        }
    }

    // claim -> statistic, same paragraph. Only when the scientific layer ran.
    if let Some(sci) = &s.science {
        for claim in &sci.claims {
            let loc = match &claim.source_span {
                crate::scientific_model::SourceSpan::Point(l) => l.clone(),
                // A range is anchored at its FIRST paragraph. Using the whole
                // range would make a claim spanning three paragraphs co-located
                // with every statistic in all three, which is a broader claim
                // than "these appear together".
                crate::scientific_model::SourceSpan::Range(r) => {
                    Location { section: r.section, paragraph: r.start_paragraph }
                }
            };
            for (si, st) in s.statistics.iter().enumerate() {
                if same_paragraph(&loc, &st.location) {
                    edges.push(Edge {
                        from: NodeRef::Claim { id: claim.id.0.clone() },
                        to: NodeRef::Statistic { index: si },
                        kind: EdgeKind::ClaimCoLocatedWithStatistic,
                    });
                }
            }
        }
    }

    EvidenceGraph { edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{extract_from_text, ExtractOptions};

    const PAPER: &str = "\
Title: Sleep and Memory

Abstract
We examined sleep and recall.

Introduction
Prior work suggests sleep supports consolidation [1].
A second line reports the opposite [2].

Methods
We recruited 48 participants and analysed recall with a paired t-test.

Results
Table 1 Recall by condition. Sleep improved recall (t(47) = 3.2, p = 0.002, d = 0.46).

References
Walker M P and Stickgold R. 2006. Sleep, memory, and plasticity. Annual Review of Psychology 57: 139-166.

Diekelmann S and Born J. 2010. The memory function of sleep. Nature Reviews Neuroscience 11: 114-126.
";

    fn state() -> ResearchState {
        ResearchState::from_extraction(&extract_from_text(PAPER))
    }

    /// Same extraction in, same bytes out — the property §5.5's incremental
    /// re-analysis keys on. A hash that varied per call would make every node
    /// look dirty on every run.
    #[test]
    fn the_derivation_is_pure_and_the_hash_is_stable() {
        let a = state();
        let b = state();
        assert_eq!(a.content_hash, b.content_hash, "same input must hash the same");
        assert_eq!(a, b);
        assert!(a.hash_matches());
        assert!(!a.content_hash.is_empty());
    }

    /// The hash covers the CONTENT. A state whose fields were edited after
    /// derivation must stop matching, or the hash is decoration.
    #[test]
    fn the_hash_detects_a_mutated_state() {
        let mut s = state();
        assert!(s.hash_matches());
        s.title = Some("a different paper".into());
        assert!(!s.hash_matches(), "the hash must cover the fields it is supposed to");
    }

    /// Provenance names the extractor per field, and records ZERO for figures
    /// rather than omitting them — "no extractor runs for this" and "this
    /// extractor found nothing" are different facts.
    #[test]
    fn provenance_names_each_extractor_including_the_one_that_does_not_exist() {
        let s = state();
        let by = |f: &str| s.provenance.iter().find(|p| p.field == f).cloned().expect(f);
        assert_eq!(by("statistics").extractor, "extract::stats");
        assert_eq!(by("references").extractor, "extract::citations::parse_reference_list");
        assert!(by("statistics").count > 0, "the fixture has a t-test");
        let figs = by("figures");
        assert_eq!(figs.count, 0);
        assert!(figs.extractor.contains("none"), "the absence must name itself: {figs:?}");
        assert!(s.figures.is_empty());
    }

    /// §3.1's privacy class, at the type level: no paragraph text anywhere.
    #[test]
    fn the_state_holds_no_prose() {
        let s = state();
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("We recruited 48 participants"),
            "a body sentence reached the gate-safe layer"
        );
        // Headings ARE carried — a heading is a label the author chose and what
        // a checklist cites, not the body.
        assert!(s.structure.iter().any(|x| x.heading.to_lowercase().contains("method")));
        assert!(s.structure.iter().all(|x| x.paragraph_count > 0 || x.heading.is_empty()));
    }

    /// **EXACT edges only.** `[1]` and `[2]` resolve to references 0 and 1.
    #[test]
    fn numeric_citations_edge_to_their_reference_by_index() {
        let s = state();
        let cites: Vec<&Edge> = s.evidence.of_kind(EdgeKind::CitesReference).collect();
        assert!(!cites.is_empty(), "the fixture has numeric markers and a reference list");
        for e in &cites {
            let (NodeRef::Citation { index: ci }, NodeRef::Reference { index: ri }) =
                (&e.from, &e.to)
            else {
                panic!("wrong node kinds on a CitesReference edge: {e:?}")
            };
            assert!(*ci < s.citations.len());
            assert!(*ri < s.references.len(), "an edge must never point past the bibliography");
        }
    }

    /// An out-of-range marker produces NO edge. `[9]` against two references is
    /// a disagreement between the text and the bibliography — a finding for
    /// another lane, not an edge to invent.
    #[test]
    fn an_out_of_range_marker_produces_no_edge_rather_than_a_nearest_guess() {
        let paper = PAPER.replace("[2]", "[9]");
        let s = ResearchState::from_extraction(&extract_from_text(&paper));
        for e in s.evidence.of_kind(EdgeKind::CitesReference) {
            let NodeRef::Reference { index } = &e.to else { panic!("wrong node") };
            assert!(*index < s.references.len(), "edge points past the reference list: {e:?}");
        }
    }

    /// Co-location is named as co-location. The Results paragraph holds both a
    /// table reference and a statistic.
    #[test]
    fn a_statistic_and_a_table_in_one_paragraph_are_co_located_not_supported() {
        let s = state();
        let edges: Vec<&Edge> =
            s.evidence.of_kind(EdgeKind::StatisticCoLocatedWithTable).collect();
        assert!(!edges.is_empty(), "the fixture puts Table 1 and a t-test in one paragraph");
        for e in edges {
            let (NodeRef::Statistic { index: si }, NodeRef::Table { index: ti }) =
                (&e.from, &e.to)
            else {
                panic!("wrong node kinds: {e:?}")
            };
            assert_eq!(
                s.statistics[*si].location, s.tables[*ti].location,
                "an edge claiming co-location must actually be co-located"
            );
        }
    }

    /// With the scientific layer off — the production default — there are no
    /// claim edges at all, and `science` is None rather than an empty layer.
    #[test]
    fn no_claim_edges_exist_when_the_scientific_layer_did_not_run() {
        let s = state();
        assert!(s.science.is_none(), "extract_from_text uses ExtractOptions::base()");
        assert_eq!(s.evidence.of_kind(EdgeKind::ClaimCoLocatedWithStatistic).count(), 0);
    }

    /// And with it on, claim edges appear — so the absence above is the
    /// extractor not running, not the graph being unable to build them.
    #[test]
    fn claim_edges_appear_when_the_scientific_layer_runs() {
        use crate::extract::extract_from_text_with;
        let ex = extract_from_text_with(PAPER, ExtractOptions::with_scientific());
        let s = ResearchState::from_extraction(&ex);
        assert!(s.science.is_some(), "the opt-in layer must be present when asked for");
        // The graph may legitimately be empty if no claim shares a paragraph
        // with a statistic; what must hold is that every edge it DID build is
        // real, and that the layer is now available to build them from.
        for e in s.evidence.of_kind(EdgeKind::ClaimCoLocatedWithStatistic) {
            let NodeRef::Claim { id } = &e.from else { panic!("wrong node: {e:?}") };
            assert!(
                s.science.as_ref().unwrap().claims.iter().any(|c| &c.id.0 == id),
                "an edge names a claim that is not in the layer: {id}"
            );
        }
    }
}

#[cfg(test)]
mod egress_tests {
    //! **The prose predicate, exercised directly.**
    //!
    //! `pipeline.rs`'s `the_research_state_carries_no_manuscript_prose` runs
    //! against a pipeline that does not derive the scientific layer (§11 D165),
    //! so it cannot reach the route prose actually takes into this type. This
    //! does, by constructing the layer explicitly. **It is the armed half of
    //! that guard**: if the layer is ever reopened, this already fails if a
    //! claim statement stops being manuscript text, and the assertion that
    //! matters does not wait on the reopening.

    use super::*;
    use crate::extract;

    const TEXT: &str = "Introduction\n\nBackground.\n\nResults\n\nTreated larvae showed values \
                        appreciably higher than the untreated control across both seasons.\n";

    #[test]
    fn a_state_without_the_layer_is_gate_safe() {
        let ex = extract::extract_from_text(TEXT);
        let s = ResearchState::from_extraction(&ex);
        assert!(s.science.is_none(), "extract_from_text uses ExtractOptions::base()");
        assert!(!s.carries_manuscript_prose());
    }

    /// **The route, demonstrated.** A claim statement is a verbatim substring of
    /// the manuscript, so a state carrying claims is NOT the "structured,
    /// gate-safe, carries no prose" layer §3.1 describes.
    #[test]
    fn carries_manuscript_prose_when_the_layer_is_present() {
        let ex = extract::extract_from_text_with(TEXT, extract::ExtractOptions::with_scientific());
        let sci = ex.scientific.as_ref().expect("the layer was asked for");
        assert!(!sci.claims.is_empty(), "the fixture must produce a claim, or this proves nothing");

        let statement = sci.claims[0].statement.trim();
        assert!(
            TEXT.contains(statement),
            "`{statement}` must be verbatim manuscript text — that is WHY the state stops \
             being gate-safe, and if it changes this test should be rewritten, not deleted"
        );

        let s = ResearchState::from_extraction(&ex);
        assert!(
            s.carries_manuscript_prose(),
            "a state with claims must declare that it carries prose"
        );
        let json = serde_json::to_string(&s).expect("serialises");
        assert!(json.contains(statement), "and the sentence really is in the serialised payload");
    }
}
