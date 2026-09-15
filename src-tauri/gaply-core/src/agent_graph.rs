//! **The typed agent graph — data, with a validator that is the deliverable.**
//!
//! §4.3: the graph is data, validated at build time — no cycle, every `reads`
//! names a layer that exists, every cloud agent has a consent gate upstream,
//! every `hard_constraint` agent has `model: None`, every agent declares its
//! trust tier.
//!
//! # The validator came first, and each rule was broken on purpose
//!
//! A validator whose first run is green on a hand-written valid graph proves
//! nothing — the standing rule in this project. So every rule below has a test
//! that feeds it a graph violating exactly that rule and asserts the reason is
//! named. See `tests::` at the bottom.
//!
//! # Two kinds of `requires`, because §4.3's single kind is wrong for one of them
//!
//! §4.3 defines `requires` as *"e.g. `AnalysisRecord` — absent → agent is not
//! invoked"*. That is right for an artifact the USER supplies: they uploaded
//! code or they did not, and absence is a fact about the submission.
//!
//! It is wrong for [`Artifact::ScientificExtraction`], which the harness can
//! PRODUCE from the manuscript it already has. Treating it as supplied would
//! mean an agent that never runs because nobody set a flag; treating a supplied
//! artifact as derivable would mean a harness trying to conjure an analysis
//! record out of prose.
//!
//! So [`Artifact::is_derivable`] splits them, and the scheduling rule is:
//!
//! * derivable + required by a scheduled agent -> the harness computes it;
//! * supplied + absent -> the agent is not invoked.
//!
//! # A THIRD KIND, which Phase 4 forced and §4.3 does not have
//!
//! **Neither kind above fits the analysis record for a methodological
//! specialist, and writing the first two specialists is what made that visible.**
//! The frequentist specialist is BETTER with the record — it can then check that
//! a test the paper reports appears among the procedures actually run, which §1
//! calls the differentiator — and it is perfectly useful without it, because the
//! p-values, tests and intervals it reasons over are in the manuscript.
//!
//! Declared under `requires`, the record is SUPPLIED and absent on every run, so
//! the specialist is never invoked at all. Left out entirely, the evidence
//! policy may not permit `AnalysisRecordEntry`, so the one record-backed check
//! is rejected at the gate the moment a record does arrive. Both readings of a
//! two-kind rule are wrong, which is what says the rule needs a third kind.
//!
//! [`AgentSpec::optional`] is it: an artifact that ENRICHES an agent without
//! gating it. Scheduling ignores it; the evidence policy may name it. The
//! validator was the thing that surfaced this — it rejected the first graph
//! written, the same way it rejected `extraction` and exposed §3.1's egress
//! confusion. **A case the rule has to decide is what tests a premise.**
//!
//! This is what makes the scientific layer a DEPENDENCY rather than a flag.
//! Measured (`examples/scientific_cost_probe.rs`), that layer costs 2.8–150 ms;
//! a lane that declares no need for it pays none of that, and nobody maintains
//! a list of which lanes opt in.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Stable identifier for an agent within the graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// §3.1's six layers. An agent declares which it reads; reading outside its
/// declaration is what the harness refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Manuscript,
    Analysis,
    Journal,
    ExternalEvidence,
    ResearchState,
    Verdict,
}

impl Layer {
    /// **Does this layer's content need consent to LEAVE THE MACHINE?**
    ///
    /// §3.1 labels the Manuscript layer *"premium, `Manuscript` consent"*, which
    /// reads as though reading it required consent. **It does not, and the first
    /// graph written against that reading failed to validate** — `extraction`
    /// reads the whole manuscript on the free tier and sends nothing.
    ///
    /// The privacy class is about EGRESS. A local agent may read any layer; a
    /// CLOUD agent reading a premium layer needs a consent scope covering it.
    /// Conflating the two would mean the free tier required premium consent to
    /// parse a file the user just opened.
    ///
    /// TOTAL — no wildcard, so a seventh layer cannot compile until it is
    /// named here AND its egress class is decided (§3.1's pairing test).
    pub fn needs_consent_to_leave(self) -> bool {
        match self {
            Layer::Manuscript | Layer::Analysis => true,
            Layer::Journal
            | Layer::ExternalEvidence
            | Layer::ResearchState
            | Layer::Verdict => false,
        }
    }
}

/// Something an agent needs that is not a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Artifact {
    /// DERIVABLE. `claims`/`variables`/`methods`/`datasets`, computed from the
    /// manuscript the harness already parsed.
    ScientificExtraction,
    /// SUPPLIED. Parsed from code/SPSS/MATLAB the researcher uploaded. If they
    /// uploaded none there is nothing to derive it from.
    AnalysisRecord,
    /// SUPPLIED. The journal's ingested guidelines and corpus.
    JournalFingerprint,
}

impl Artifact {
    /// Can the harness produce this from what it already has?
    ///
    /// TOTAL on purpose: a new artifact forces this decision, because getting
    /// it wrong in either direction silently disables an agent or silently
    /// invents a dependency.
    pub fn is_derivable(self) -> bool {
        match self {
            Artifact::ScientificExtraction => true,
            Artifact::AnalysisRecord | Artifact::JournalFingerprint => false,
        }
    }
}

/// §4.4's trust tiers. Every agent declares one; it decides what may override
/// what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// Arithmetic, equivalence, recomputation. Overrides everything above it.
    Deterministic,
    /// Checklist evaluators over the research state.
    Rule,
    /// Does the evidence support the claim (LLM, evidence policy satisfied).
    EvidenceReasoning,
    /// Novelty, likely reviewer concern, contribution, fit.
    ExpertJudgement,
    /// The editorial reading.
    Synthesis,
}

/// §4.1's six clusters. An agent belongs to exactly one, and the cluster is
/// what §4.2 fans out inside.
///
/// This is DECLARATION, not dispatch: nothing routes on it yet, and saying so
/// is the point — `run_pipeline_inner` still executes a fixed sequence
/// (§12.1 item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cluster {
    Ingestion,
    MethodologicalSoundness,
    ReportingStandards,
    JournalFit,
    Integrity,
    Synthesis,
}

/// Where a piece of evidence can come from.
///
/// Deliberately NOT a free string: the validator's job below is to check that
/// an agent which permits a source kind has actually declared the artifact that
/// kind arrives in, and it cannot do that over prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    /// A span of the manuscript — a sentence, a table cell, a reported statistic.
    ManuscriptSpan,
    /// An item of the derived scientific layer: a claim, variable, method, dataset.
    ScientificItem,
    /// A procedure recovered from an uploaded analysis file.
    AnalysisRecordEntry,
    /// An ingested journal requirement or convention.
    JournalRequirement,
    /// A retrieved external work — a reference, an OA full text, a retraction record.
    ExternalWork,
}

impl EvidenceSource {
    /// The artifact this kind of evidence ARRIVES IN, if any. `None` means the
    /// layer declaration alone suffices.
    ///
    /// TOTAL on purpose: a new source kind must decide where it comes from, or
    /// the validator below silently stops checking it.
    pub fn arrives_in(self) -> Option<Artifact> {
        match self {
            EvidenceSource::ScientificItem => Some(Artifact::ScientificExtraction),
            EvidenceSource::AnalysisRecordEntry => Some(Artifact::AnalysisRecord),
            EvidenceSource::JournalRequirement => Some(Artifact::JournalFingerprint),
            EvidenceSource::ManuscriptSpan | EvidenceSource::ExternalWork => None,
        }
    }
}

/// §4.3: *"minimum sources, permitted source types, citation required,
/// uncertainty required"*.
///
/// **What it is for**: stopping a model producing a sophisticated conclusion
/// from insufficient evidence. An opinion emitted without its policy satisfied
/// is rejected at the gate with the reason recorded.
///
/// **What is checkable at BUILD time, and what is not.** The validator can only
/// check the policy is SATISFIABLE — that it does not demand evidence from an
/// artifact the agent never asked for, and does not demand a positive number of
/// sources while permitting no kind. Whether a given finding actually carried
/// its evidence is a runtime question, and [`EvidencePolicy::admits`] is the
/// one predicate both halves use so they cannot drift.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EvidencePolicy {
    /// How many distinct pieces of evidence a finding must carry. `0` means the
    /// agent may speak from the structure alone — correct for a deterministic
    /// pass whose finding IS the computation.
    ///
    /// The `Default` is the permissive one (0 / none / false / false), so the
    /// five existing lanes keep their behaviour when this field is added to a
    /// graph file that does not carry it. A lane that has always spoken from its
    /// own computation is not suddenly required to cite.
    #[serde(default)]
    pub min_sources: usize,
    /// The kinds that count towards `min_sources`. Empty with
    /// `min_sources > 0` is unsatisfiable and rejected.
    #[serde(default)]
    pub permitted_sources: Vec<EvidenceSource>,
    /// Every finding must quote the span or work it rests on.
    #[serde(default)]
    pub citation_required: bool,
    /// Every finding must state what it could not determine.
    #[serde(default)]
    pub uncertainty_required: bool,
}

impl EvidencePolicy {
    /// **Does this policy admit a finding carrying these sources?**
    ///
    /// ONE predicate, used by the build-time satisfiability check and by the
    /// runtime gate. §34.3's producer-and-checker shape: a gate that rejected
    /// what the validator had accepted would be two rules wearing one name.
    pub fn admits(&self, carried: &[EvidenceSource]) -> bool {
        carried.iter().filter(|s| self.permitted_sources.contains(s)).count() >= self.min_sources
    }
}

/// What an agent needs to run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelRequirement {
    /// Deterministic: rules, checklists, arithmetic. No model at all.
    None,
    /// A local model at the named tier.
    Local(String),
    /// A cloud model, reached through the proxy. **Requires a consent gate.**
    Cloud,
}

/// One node in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSpec {
    pub id: AgentId,
    /// §4.1's cluster. Defaults to `Ingestion` only so the six shipped lanes,
    /// which predate the field, keep parsing; every node added from Phase 4 on
    /// names it.
    #[serde(default = "default_cluster")]
    pub cluster: Cluster,
    /// Layers this agent may read. Reading outside them is the error the
    /// declaration exists to make visible.
    pub reads: Vec<Layer>,
    /// Artifacts it needs. Absent-and-supplied means the agent is not invoked;
    /// absent-and-derivable means the harness computes it. See
    /// [`Artifact::is_derivable`].
    #[serde(default)]
    pub requires: Vec<Artifact>,
    /// Artifacts that ENRICH this agent without gating it — see the module
    /// header for why a two-kind `requires` could not express this. Scheduling
    /// ignores these; [`EvidencePolicy`] may name them.
    #[serde(default)]
    pub optional: Vec<Artifact>,
    /// Agents that must run before it. The cycle check is over these edges.
    #[serde(default)]
    pub after: Vec<AgentId>,
    pub model: ModelRequirement,
    pub trust_tier: TrustTier,
    /// Deterministic verdicts that are never voted on (§4.4 Tier 0/1).
    #[serde(default)]
    pub hard_constraint: bool,
    /// Whether this agent may revise during the round-table (§5.1).
    #[serde(default)]
    pub may_revise: bool,
    /// The consent scope a cloud agent needs. `None` for non-cloud agents.
    #[serde(default)]
    pub requires_consent: Option<ConsentScopeName>,
    /// §4.3's evidence policy. Defaults permissive — see [`EvidencePolicy`].
    #[serde(default)]
    pub evidence_policy: EvidencePolicy,
}

fn default_cluster() -> Cluster {
    Cluster::Ingestion
}

/// The consent scope named in the graph, as a string the graph file can carry.
/// Deliberately NOT `gaply_core::consent::ConsentScope`: that is a bitset built
/// for the release gate, and a graph file should name what it needs in words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentScopeName {
    Manuscript,
    AnalysisCode,
    AnalysisData,
    ExternalEvidence,
    JournalResearch,
}

/// The whole graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentGraph {
    pub version: u32,
    pub agents: Vec<AgentSpec>,
}

/// One validation failure, naming the RULE rather than saying "invalid graph".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Two agents share an id.
    DuplicateId(AgentId),
    /// `after` names an agent that is not in the graph.
    UnknownDependency { agent: AgentId, missing: AgentId },
    /// The `after` edges contain a cycle. Carries the cycle in order.
    Cycle(Vec<AgentId>),
    /// A cloud agent with no consent scope declared.
    CloudWithoutConsentGate(AgentId),
    /// A non-cloud agent that declares a consent scope it cannot need.
    ConsentGateWithoutCloud(AgentId),
    /// A hard-constraint agent that requires a model.
    HardConstraintWithModel { agent: AgentId, model: ModelRequirement },
    /// An agent reading a premium-consented layer without declaring consent.
    PremiumLayerWithoutConsent { agent: AgentId, layer: Layer },
    /// An agent that declares no layers at all.
    ReadsNothing(AgentId),
    /// An evidence policy that demands sources while permitting no kind.
    EvidencePolicyUnsatisfiable(AgentId),
    /// An evidence policy permitting a source kind that arrives in an artifact
    /// the agent never declared.
    EvidencePolicyNeedsArtifact { agent: AgentId, source: EvidenceSource, artifact: Artifact },
}

impl ConsentScopeName {
    /// Does this scope authorise sending the content of `layer`?
    pub fn covers(self, layer: Layer) -> bool {
        matches!(
            (self, layer),
            (ConsentScopeName::Manuscript, Layer::Manuscript)
                | (ConsentScopeName::AnalysisCode, Layer::Analysis)
                | (ConsentScopeName::AnalysisData, Layer::Analysis)
                | (ConsentScopeName::ExternalEvidence, Layer::ExternalEvidence)
                | (ConsentScopeName::JournalResearch, Layer::Journal)
        )
    }
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::DuplicateId(id) => write!(f, "duplicate agent id `{id}`"),
            GraphError::UnknownDependency { agent, missing } => {
                write!(f, "agent `{agent}` runs after `{missing}`, which is not in the graph")
            }
            GraphError::Cycle(ids) => {
                let path: Vec<String> = ids.iter().map(|i| i.0.clone()).collect();
                write!(f, "cycle in the agent graph: {}", path.join(" -> "))
            }
            GraphError::CloudWithoutConsentGate(id) => write!(
                f,
                "agent `{id}` uses a cloud model but declares no consent scope — a cloud \
                 call with no gate upstream is the boundary this graph exists to make \
                 checkable"
            ),
            GraphError::ConsentGateWithoutCloud(id) => write!(
                f,
                "agent `{id}` declares a consent scope but uses no cloud model — a gate on \
                 a local agent is a claim that something leaves the machine when nothing does"
            ),
            GraphError::HardConstraintWithModel { agent, model } => write!(
                f,
                "agent `{agent}` is a hard constraint but requires {model:?} — a verdict that \
                 overrides consensus without being voted on must be deterministic, or the \
                 override is a model outranking the vote"
            ),
            GraphError::PremiumLayerWithoutConsent { agent, layer } => write!(
                f,
                "agent `{agent}` reads the {layer:?} layer, which is premium-consented, but \
                 declares no consent scope"
            ),
            GraphError::ReadsNothing(id) => {
                write!(f, "agent `{id}` declares no layers — it cannot read anything")
            }
            GraphError::EvidencePolicyUnsatisfiable(id) => write!(
                f,
                "agent `{id}` requires evidence but permits no source kind — a policy no \
                 finding can satisfy silently disables the agent instead of constraining it"
            ),
            GraphError::EvidencePolicyNeedsArtifact { agent, source, artifact } => write!(
                f,
                "agent `{agent}` permits {source:?} evidence, which arrives in {artifact:?}, \
                 but declares it in neither `requires` nor `optional` — the policy would admit \
                 evidence the harness was never asked to produce. Use `requires` if the agent \
                 cannot run without it, `optional` if it is an enrichment"
            ),
        }
    }
}

impl AgentGraph {
    /// **Validate every §4.3 rule. Returns EVERY violation, not the first.**
    ///
    /// Stopping at the first would make fixing a graph an N-round game, and
    /// would hide a second class of error behind the first.
    pub fn validate(&self) -> Result<(), Vec<GraphError>> {
        let mut errors = Vec::new();

        // --- ids are unique -------------------------------------------------
        let mut seen: BTreeSet<&AgentId> = BTreeSet::new();
        for a in &self.agents {
            if !seen.insert(&a.id) {
                errors.push(GraphError::DuplicateId(a.id.clone()));
            }
        }
        let known: BTreeSet<&AgentId> = self.agents.iter().map(|a| &a.id).collect();

        for a in &self.agents {
            // --- every `reads` is a layer that exists.
            //
            // The TYPE already guarantees this: `Layer` is a closed enum, so a
            // graph file naming `"quantum_layer"` fails to DESERIALIZE and
            // never reaches here. What remains checkable is the degenerate
            // case the type cannot express — an empty list.
            if a.reads.is_empty() {
                errors.push(GraphError::ReadsNothing(a.id.clone()));
            }

            // --- dependencies resolve
            for dep in &a.after {
                if !known.contains(dep) {
                    errors.push(GraphError::UnknownDependency {
                        agent: a.id.clone(),
                        missing: dep.clone(),
                    });
                }
            }

            // --- every cloud agent has a consent gate, and only cloud agents do
            match (&a.model, &a.requires_consent) {
                (ModelRequirement::Cloud, None) => {
                    errors.push(GraphError::CloudWithoutConsentGate(a.id.clone()))
                }
                (m, Some(_)) if *m != ModelRequirement::Cloud => {
                    errors.push(GraphError::ConsentGateWithoutCloud(a.id.clone()))
                }
                _ => {}
            }

            // --- hard constraints are deterministic
            if a.hard_constraint && a.model != ModelRequirement::None {
                errors.push(GraphError::HardConstraintWithModel {
                    agent: a.id.clone(),
                    model: a.model.clone(),
                });
            }

            // --- the evidence policy must be SATISFIABLE.
            //
            // Two ways it is not, and they fail in opposite directions. A policy
            // demanding sources while permitting no kind can never be met, so
            // the agent is disabled rather than constrained. A policy permitting
            // a kind that arrives in an undeclared artifact demands evidence the
            // harness was never asked to derive — the Phase-4 shape exactly:
            // a methodological specialist permitting `AnalysisRecordEntry` while
            // forgetting to require the record.
            let pol = &a.evidence_policy;
            if pol.min_sources > 0 && pol.permitted_sources.is_empty() {
                errors.push(GraphError::EvidencePolicyUnsatisfiable(a.id.clone()));
            }
            for src in &pol.permitted_sources {
                if let Some(art) = src.arrives_in() {
                    if !a.requires.contains(&art) && !a.optional.contains(&art) {
                        errors.push(GraphError::EvidencePolicyNeedsArtifact {
                            agent: a.id.clone(),
                            source: *src,
                            artifact: art,
                        });
                    }
                }
            }

            // --- a CLOUD agent reading a premium layer needs a consent that
            //     covers it. Local agents are unconstrained: they read, they do
            //     not transmit, and the free tier depends on that.
            if a.model == ModelRequirement::Cloud {
                for layer in &a.reads {
                    if layer.needs_consent_to_leave()
                        && !a.requires_consent.is_some_and(|c| c.covers(*layer))
                    {
                        errors.push(GraphError::PremiumLayerWithoutConsent {
                            agent: a.id.clone(),
                            layer: *layer,
                        });
                    }
                }
            }
        }

        // --- no cycles ------------------------------------------------------
        if let Some(cycle) = self.find_cycle() {
            errors.push(GraphError::Cycle(cycle));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Depth-first cycle detection over `after`, returning the cycle in order.
    /// Reporting the PATH rather than a bool is the difference between a
    /// message someone can act on and one they have to re-derive.
    fn find_cycle(&self) -> Option<Vec<AgentId>> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            Open,
            Done,
        }
        let deps: BTreeMap<&AgentId, &Vec<AgentId>> =
            self.agents.iter().map(|a| (&a.id, &a.after)).collect();
        let mut marks: BTreeMap<&AgentId, Mark> = BTreeMap::new();
        let mut stack: Vec<&AgentId> = Vec::new();

        fn walk<'a>(
            node: &'a AgentId,
            deps: &BTreeMap<&'a AgentId, &'a Vec<AgentId>>,
            marks: &mut BTreeMap<&'a AgentId, Mark>,
            stack: &mut Vec<&'a AgentId>,
        ) -> Option<Vec<AgentId>> {
            match marks.get(node) {
                Some(Mark::Done) => return None,
                Some(Mark::Open) => {
                    let at = stack.iter().position(|n| *n == node).unwrap_or(0);
                    let mut cycle: Vec<AgentId> = stack[at..].iter().map(|n| (*n).clone()).collect();
                    cycle.push(node.clone());
                    return Some(cycle);
                }
                None => {}
            }
            marks.insert(node, Mark::Open);
            stack.push(node);
            if let Some(after) = deps.get(node) {
                for dep in after.iter() {
                    // An unknown dependency is reported by its own rule; skip it
                    // here so a missing node cannot masquerade as a cycle.
                    if deps.contains_key(dep) {
                        if let Some(c) = walk(dep, deps, marks, stack) {
                            return Some(c);
                        }
                    }
                }
            }
            stack.pop();
            marks.insert(node, Mark::Done);
            None
        }

        for a in &self.agents {
            if let Some(c) = walk(&a.id, &deps, &mut marks, &mut stack) {
                return Some(c);
            }
        }
        None
    }

    /// Execution order: dependencies first, ties broken by declaration order so
    /// the same graph always produces the same order. Errors if the graph does
    /// not validate.
    pub fn execution_order(&self) -> Result<Vec<&AgentSpec>, Vec<GraphError>> {
        self.validate()?;
        let mut out: Vec<&AgentSpec> = Vec::new();
        let mut placed: BTreeSet<&AgentId> = BTreeSet::new();
        // Repeated passes in declaration order: deterministic, and O(n^2) on a
        // graph whose size is the number of agents in the product.
        while out.len() < self.agents.len() {
            let before = out.len();
            for a in &self.agents {
                if placed.contains(&a.id) {
                    continue;
                }
                if a.after.iter().all(|d| placed.contains(d)) {
                    out.push(a);
                    placed.insert(&a.id);
                }
            }
            if out.len() == before {
                // validate() proved acyclic, so this is unreachable.
                break;
            }
        }
        Ok(out)
    }

    /// **Does anything scheduled here need the scientific layer?**
    ///
    /// This is the per-lane opt-in the cost measurement pointed at, expressed
    /// as a dependency: the harness derives the layer when an agent declares it
    /// and not otherwise, so a lane that reads nothing pays nothing and nobody
    /// maintains a list.
    pub fn requires_scientific_extraction(&self) -> bool {
        self.agents
            .iter()
            .any(|a| a.requires.contains(&Artifact::ScientificExtraction))
    }

    /// Artifacts the harness must PRODUCE for this graph, deduplicated.
    pub fn derivable_requirements(&self) -> BTreeSet<Artifact> {
        self.agents
            .iter()
            .flat_map(|a| a.requires.iter().copied())
            .filter(|r| r.is_derivable())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! **Every rule is broken on purpose, one at a time.**
    //!
    //! A validator whose first run is green on a hand-written valid graph
    //! proves nothing about whether it validates — the standing rule here. So
    //! each rule gets a graph violating exactly it, and the assertion is on the
    //! REASON, not on "some error happened": a validator that reported
    //! `Cycle` for a missing consent gate would pass a bare `is_err()`.

    use super::*;

    fn agent(id: &str) -> AgentSpec {
        AgentSpec {
            id: AgentId(id.into()),
            cluster: Cluster::Ingestion,
            reads: vec![Layer::ResearchState],
            requires: Vec::new(),
            optional: Vec::new(),
            after: Vec::new(),
            model: ModelRequirement::None,
            trust_tier: TrustTier::Deterministic,
            hard_constraint: false,
            may_revise: false,
            requires_consent: None,
            evidence_policy: EvidencePolicy::default(),
        }
    }

    fn graph(agents: Vec<AgentSpec>) -> AgentGraph {
        AgentGraph { version: 1, agents }
    }

    /// The positive control. Without it, every test below could pass because
    /// the validator rejects everything.
    #[test]
    fn a_valid_graph_validates() {
        let g = graph(vec![agent("a"), {
            let mut b = agent("b");
            b.after = vec![AgentId("a".into())];
            b
        }]);
        assert_eq!(g.validate(), Ok(()), "the valid graph must pass, or the breaks below prove nothing");
        let order: Vec<String> =
            g.execution_order().unwrap().iter().map(|a| a.id.0.clone()).collect();
        assert_eq!(order, vec!["a", "b"], "dependencies come first");
    }

    // --- RULE: no cycles ----------------------------------------------------

    #[test]
    fn a_cyclic_graph_is_rejected_and_the_cycle_is_named() {
        let mut a = agent("a");
        let mut b = agent("b");
        a.after = vec![AgentId("b".into())];
        b.after = vec![AgentId("a".into())];
        let errs = graph(vec![a, b]).validate().expect_err("a cycle must be rejected");
        let cycle = errs
            .iter()
            .find_map(|e| match e {
                GraphError::Cycle(c) => Some(c.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected a Cycle error, got {errs:?}"));
        assert!(cycle.len() >= 2, "the cycle must be reported as a PATH, not a bool: {cycle:?}");
        let names: Vec<String> = cycle.iter().map(|i| i.0.clone()).collect();
        assert!(names.contains(&"a".to_string()) && names.contains(&"b".to_string()));
    }

    /// A three-node cycle, because a two-node one can be caught by a simpler
    /// check (mutual reference) that would not generalise.
    #[test]
    fn a_longer_cycle_is_also_found() {
        let mut a = agent("a");
        let mut b = agent("b");
        let mut c = agent("c");
        a.after = vec![AgentId("c".into())];
        b.after = vec![AgentId("a".into())];
        c.after = vec![AgentId("b".into())];
        let errs = graph(vec![a, b, c]).validate().expect_err("a 3-cycle must be rejected");
        assert!(
            errs.iter().any(|e| matches!(e, GraphError::Cycle(_))),
            "expected a Cycle error, got {errs:?}"
        );
    }

    /// A MISSING node must not masquerade as a cycle — they have different
    /// fixes, and reporting the wrong one sends the reader to the wrong place.
    #[test]
    fn an_unknown_dependency_is_named_as_missing_not_as_a_cycle() {
        let mut a = agent("a");
        a.after = vec![AgentId("nowhere".into())];
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                GraphError::UnknownDependency { missing, .. } if missing.0 == "nowhere"
            )),
            "expected UnknownDependency naming `nowhere`, got {errs:?}"
        );
        assert!(
            !errs.iter().any(|e| matches!(e, GraphError::Cycle(_))),
            "a missing dependency is not a cycle: {errs:?}"
        );
    }

    // --- RULE: every cloud agent has a consent gate upstream ----------------

    #[test]
    fn a_cloud_agent_without_a_consent_gate_is_rejected() {
        let mut a = agent("reviewer");
        a.model = ModelRequirement::Cloud;
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                GraphError::CloudWithoutConsentGate(id) if id.0 == "reviewer"
            )),
            "expected CloudWithoutConsentGate, got {errs:?}"
        );
    }

    /// And the inverse: a gate on a local agent claims something leaves the
    /// machine when nothing does.
    #[test]
    fn a_consent_gate_on_a_local_agent_is_rejected() {
        let mut a = agent("local");
        a.requires_consent = Some(ConsentScopeName::Manuscript);
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                GraphError::ConsentGateWithoutCloud(id) if id.0 == "local"
            )),
            "expected ConsentGateWithoutCloud, got {errs:?}"
        );
    }

    /// The cloud agent that IS gated passes — so the rule is not "no cloud
    /// agents", which would also have made the test above green.
    #[test]
    fn a_gated_cloud_agent_is_accepted() {
        let mut a = agent("reviewer");
        a.model = ModelRequirement::Cloud;
        a.requires_consent = Some(ConsentScopeName::Manuscript);
        a.trust_tier = TrustTier::EvidenceReasoning;
        assert_eq!(graph(vec![a]).validate(), Ok(()));
    }

    // --- RULE: every hard_constraint agent has model: None ------------------

    #[test]
    fn a_hard_constraint_agent_with_a_model_is_rejected() {
        for model in [ModelRequirement::Local("slm1".into()), ModelRequirement::Cloud] {
            let mut a = agent("validator");
            a.hard_constraint = true;
            a.model = model.clone();
            if model == ModelRequirement::Cloud {
                a.requires_consent = Some(ConsentScopeName::Manuscript);
            }
            let errs = graph(vec![a]).validate().expect_err("must be rejected");
            assert!(
                errs.iter().any(|e| matches!(
                    e,
                    GraphError::HardConstraintWithModel { agent, .. } if agent.0 == "validator"
                )),
                "expected HardConstraintWithModel for {model:?}, got {errs:?}"
            );
        }
    }

    #[test]
    fn a_deterministic_hard_constraint_is_accepted() {
        let mut a = agent("validator");
        a.hard_constraint = true;
        assert_eq!(graph(vec![a]).validate(), Ok(()));
    }

    // --- RULE: reads names layers that exist --------------------------------

    /// The closed `Layer` enum already makes an unknown layer a DESERIALIZATION
    /// failure, so it can never reach the validator. This asserts that — a rule
    /// enforced by the type is stronger than one enforced by a check, and the
    /// test records WHERE it is enforced so nobody adds a redundant check or
    /// removes the type guarantee believing a check covers it.
    #[test]
    fn an_unknown_layer_name_fails_to_deserialize_and_never_reaches_the_validator() {
        let json = r#"{"version":1,"agents":[{"id":"a","reads":["quantum_layer"],
                        "model":"none","trust_tier":"deterministic"}]}"#;
        let parsed: Result<AgentGraph, _> = serde_json::from_str(json);
        assert!(parsed.is_err(), "an unknown layer must not deserialize");
        let msg = parsed.unwrap_err().to_string();
        assert!(msg.contains("quantum_layer") || msg.contains("unknown variant"), "{msg}");
    }

    #[test]
    fn an_agent_that_reads_nothing_is_rejected() {
        let mut a = agent("blind");
        a.reads = Vec::new();
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| matches!(e, GraphError::ReadsNothing(id) if id.0 == "blind")),
            "expected ReadsNothing, got {errs:?}"
        );
    }

    // --- RULE: a premium layer needs a declared consent ---------------------

    /// **A LOCAL agent may read the manuscript with no consent at all.**
    ///
    /// This test asserted the opposite until the shipped graph failed to
    /// validate on `extraction`, which reads the whole manuscript on the free
    /// tier and transmits nothing. §3.1's *"premium, `Manuscript` consent"*
    /// label is about EGRESS; reading it as a read-permission would mean the
    /// free tier needed premium consent to parse a file the user just opened.
    #[test]
    fn a_local_agent_may_read_the_manuscript_without_consent() {
        let mut a = agent("extraction");
        a.reads = vec![Layer::Manuscript];
        assert_eq!(
            graph(vec![a]).validate(),
            Ok(()),
            "a local agent transmits nothing, so no consent is needed to read"
        );
    }

    /// A CLOUD agent reading the manuscript without a covering scope IS
    /// rejected — the rule that matters, on the case where content can leave.
    #[test]
    fn a_cloud_agent_reading_the_manuscript_without_a_covering_scope_is_rejected() {
        let mut a = agent("reviewer");
        a.reads = vec![Layer::Manuscript];
        a.model = ModelRequirement::Cloud;
        // A scope that exists but does NOT cover the manuscript: this is the
        // case a mere `is_some()` check would wave through.
        a.requires_consent = Some(ConsentScopeName::JournalResearch);
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.iter().any(|e| matches!(
                e,
                GraphError::PremiumLayerWithoutConsent { layer: Layer::Manuscript, .. }
            )),
            "expected PremiumLayerWithoutConsent, got {errs:?}"
        );
    }

    /// And with the right scope it passes, so the rule is not "no cloud agent
    /// may read the manuscript".
    #[test]
    fn a_cloud_agent_with_a_covering_scope_is_accepted() {
        let mut a = agent("reviewer");
        a.reads = vec![Layer::Manuscript];
        a.model = ModelRequirement::Cloud;
        a.requires_consent = Some(ConsentScopeName::Manuscript);
        a.trust_tier = TrustTier::EvidenceReasoning;
        assert_eq!(graph(vec![a]).validate(), Ok(()));
    }

    // --- EVERY violation is reported, not just the first --------------------

    #[test]
    fn all_violations_are_reported_together() {
        let mut a = agent("a");
        a.model = ModelRequirement::Cloud; // no gate
        a.hard_constraint = true; // and a model
        a.reads = Vec::new(); // and reads nothing
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.len() >= 3,
            "stopping at the first error makes fixing a graph an N-round game: {errs:?}"
        );
    }

    // --- the scientific layer, as a dependency ------------------------------

    #[test]
    fn the_scientific_layer_is_required_only_when_an_agent_declares_it() {
        let g = graph(vec![agent("a")]);
        assert!(!g.requires_scientific_extraction(), "nobody asked for it");
        assert!(g.derivable_requirements().is_empty());

        let mut b = agent("b");
        b.requires = vec![Artifact::ScientificExtraction];
        let g = graph(vec![agent("a"), b]);
        assert!(g.requires_scientific_extraction(), "one agent asking is enough");
        assert!(g.derivable_requirements().contains(&Artifact::ScientificExtraction));
    }

    /// A SUPPLIED artifact is never something the harness produces — the
    /// distinction §4.3's single `requires` kind cannot express.
    // --- RULE: the evidence policy must be satisfiable ----------------------

    /// A policy demanding evidence while permitting no kind can never be met.
    /// The failure mode is the quiet one: the agent is DISABLED rather than
    /// constrained, and nothing says so.
    #[test]
    fn a_policy_that_permits_no_source_but_demands_one_is_rejected() {
        let mut a = agent("a");
        a.evidence_policy =
            EvidencePolicy { min_sources: 2, permitted_sources: vec![], ..Default::default() };
        let errs = graph(vec![a]).validate().expect_err("unsatisfiable policy must be rejected");
        assert!(
            errs.contains(&GraphError::EvidencePolicyUnsatisfiable(AgentId("a".into()))),
            "expected EvidencePolicyUnsatisfiable, got {errs:?}"
        );
    }

    /// **The Phase-4 shape.** A methodological specialist that permits evidence
    /// from the analysis record and forgets to `require` it would demand
    /// evidence the harness was never asked to produce — and, because the
    /// record is SUPPLIED rather than derivable, would be silently starved on
    /// every real run rather than failing.
    #[test]
    fn a_policy_naming_an_undeclared_artifact_is_rejected_and_the_artifact_is_named() {
        let mut a = agent("a");
        a.evidence_policy = EvidencePolicy {
            min_sources: 1,
            permitted_sources: vec![EvidenceSource::AnalysisRecordEntry],
            ..Default::default()
        };
        let errs = graph(vec![a]).validate().expect_err("must be rejected");
        assert!(
            errs.contains(&GraphError::EvidencePolicyNeedsArtifact {
                agent: AgentId("a".into()),
                source: EvidenceSource::AnalysisRecordEntry,
                artifact: Artifact::AnalysisRecord,
            }),
            "the error must name the ARTIFACT, or the fix is a guess: {errs:?}"
        );
    }

    /// The positive control for the rule above: declaring the artifact makes the
    /// same policy legal. Without this, the rule could be rejecting everything.
    #[test]
    fn the_same_policy_is_accepted_once_the_artifact_is_declared() {
        let mut a = agent("a");
        a.requires = vec![Artifact::AnalysisRecord];
        a.evidence_policy = EvidencePolicy {
            min_sources: 1,
            permitted_sources: vec![EvidenceSource::AnalysisRecordEntry],
            ..Default::default()
        };
        assert_eq!(graph(vec![a]).validate(), Ok(()));
    }

    /// **The third kind, and the case that forced it.** A methodological
    /// specialist is better with the analysis record and useful without it.
    /// Under `requires` it would never be invoked (supplied + absent); left out
    /// it could not cite the record when one arrived. `optional` is the only
    /// declaration that is true of it.
    #[test]
    fn an_optional_artifact_satisfies_the_policy_without_gating_the_agent() {
        let mut a = agent("a");
        a.optional = vec![Artifact::AnalysisRecord];
        a.evidence_policy = EvidencePolicy {
            min_sources: 1,
            permitted_sources: vec![EvidenceSource::AnalysisRecordEntry],
            ..Default::default()
        };
        assert_eq!(graph(vec![a.clone()]).validate(), Ok(()), "optional satisfies the policy");
        assert!(
            !a.requires.contains(&Artifact::AnalysisRecord),
            "and does NOT gate: scheduling reads `requires`, which is empty"
        );
        // The negative control for the pair: moving it back out of both lists
        // must fail again, or `optional` is not what made it pass.
        let mut b = a.clone();
        b.optional.clear();
        assert!(graph(vec![b]).validate().is_err(), "with neither list, it must be rejected");
    }

    /// `ManuscriptSpan` and `ExternalWork` arrive in no artifact, so permitting
    /// them requires nothing. A rule that demanded an artifact for every source
    /// kind would reject every honest policy.
    #[test]
    fn a_source_kind_that_arrives_in_no_artifact_needs_no_declaration() {
        let mut a = agent("a");
        a.evidence_policy = EvidencePolicy {
            min_sources: 1,
            permitted_sources: vec![
                EvidenceSource::ManuscriptSpan,
                EvidenceSource::ExternalWork,
            ],
            ..Default::default()
        };
        assert_eq!(graph(vec![a]).validate(), Ok(()));
        assert_eq!(EvidenceSource::ManuscriptSpan.arrives_in(), None);
        assert_eq!(EvidenceSource::ExternalWork.arrives_in(), None);
    }

    /// **`admits` is ONE predicate, and this is the test that says so.** The
    /// build-time check and the runtime gate both call it; a second
    /// implementation at either end is the drift this exists to prevent.
    #[test]
    fn admits_counts_only_permitted_kinds() {
        let pol = EvidencePolicy {
            min_sources: 2,
            permitted_sources: vec![EvidenceSource::ManuscriptSpan, EvidenceSource::ScientificItem],
            ..Default::default()
        };
        assert!(pol.admits(&[EvidenceSource::ManuscriptSpan, EvidenceSource::ScientificItem]));
        assert!(
            !pol.admits(&[EvidenceSource::ManuscriptSpan, EvidenceSource::ExternalWork]),
            "an unpermitted kind must not count towards the minimum"
        );
        assert!(!pol.admits(&[EvidenceSource::ManuscriptSpan]), "one is fewer than two");
        assert!(
            EvidencePolicy::default().admits(&[]),
            "the permissive default admits a finding carrying nothing — that is what \
             keeps the five deterministic lanes working"
        );
    }

    #[test]
    fn a_supplied_artifact_is_not_derivable() {
        let mut a = agent("a");
        a.requires = vec![Artifact::AnalysisRecord];
        let g = graph(vec![a]);
        assert!(!g.requires_scientific_extraction());
        assert!(
            g.derivable_requirements().is_empty(),
            "the harness cannot conjure an analysis record from prose"
        );
        assert!(!Artifact::AnalysisRecord.is_derivable());
        assert!(Artifact::ScientificExtraction.is_derivable());
    }
}

// ---------------------------------------------------------------------------
// The shipped graph
// ---------------------------------------------------------------------------

/// The six existing lanes, as data.
///
/// `include_str!` rather than a runtime file read: the graph is part of the
/// binary's contract, not user configuration, so a malformed one should fail to
/// BUILD rather than to start. `shipped_graph` parses it once.
const SHIPPED_GRAPH_JSON: &str = include_str!("../data/agent_graph.json");

/// The validated shipped graph.
///
/// Parsed and validated on first use; both failures panic, because a graph that
/// does not validate is a programming error in committed data and there is no
/// sensible degraded behaviour — running an unvalidated agent graph is exactly
/// what the validator exists to prevent.
pub fn shipped_graph() -> &'static AgentGraph {
    static GRAPH: std::sync::OnceLock<AgentGraph> = std::sync::OnceLock::new();
    GRAPH.get_or_init(|| {
        let g: AgentGraph = serde_json::from_str(SHIPPED_GRAPH_JSON)
            .expect("the shipped agent graph must parse");
        if let Err(errors) = g.validate() {
            let detail: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            panic!("the shipped agent graph does not validate:\n  {}", detail.join("\n  "));
        }
        g
    })
}

#[cfg(test)]
mod shipped {
    use super::*;

    /// The graph in the repository validates. This is the rule the other tests
    /// exist to give meaning: they prove the validator rejects; this proves it
    /// accepts the thing that actually ships.
    #[test]
    fn the_shipped_graph_validates() {
        let g = shipped_graph();
        assert_eq!(
            g.agents.len(),
            9,
            "the six pipeline lanes plus the three Phase-4/4b specialists"
        );
    }

    /// **The graph's order must be the order the pipeline actually runs — for
    /// the SIX LANES. For the specialists it pins something weaker, and saying
    /// which is the point.**
    ///
    /// `run_pipeline_inner` executes extraction, validation, ai, plagiarism,
    /// rag, verification — in that sequence, hardcoded. If the graph disagreed
    /// about those it would describe something that does not happen, which is
    /// worse than no description: the next phase routes over it.
    ///
    /// `frequentist_stats`, `ml_methodology` and `claim_evidence_strength` are
    /// **not executed by `run_pipeline_inner` at all**. Checked 15 Sep 2026:
    /// the only callers of `specialist::run` anywhere in the tree are tests and
    /// `examples/`. So for those three this test pins the graph's declared
    /// topological order and nothing about execution — the same gap §12.1
    /// item 2 records for the lanes' DRIVING, one node further along. A reader
    /// who takes this test's name at face value would conclude the specialists
    /// run in production. They do not.
    #[test]
    fn the_graphs_order_matches_the_pipelines_lane_order() {
        let order: Vec<String> = shipped_graph()
            .execution_order()
            .expect("validates")
            .iter()
            .map(|a| a.id.0.clone())
            .collect();
        assert_eq!(
            order,
            vec![
                "extraction",
                "validation_maths",
                "ai_detection",
                "plagiarism",
                "rag",
                "verification",
                "frequentist_stats",
                "ml_methodology",
                "claim_evidence_strength",
            ],
            "the graph must describe the order run_pipeline_inner really uses"
        );
    }

    /// **The six PIPELINE lanes, still in the pipeline's order, before anything
    /// else.** The test above now mixes two populations: the lanes
    /// `run_pipeline_inner` executes, and the specialists it does not. This one
    /// keeps the original guarantee separately, so adding a ninth agent cannot
    /// quietly reorder the six that a hardcoded sequence actually runs.
    #[test]
    fn the_six_pipeline_lanes_keep_their_order_and_come_first() {
        let g = shipped_graph();
        let lanes = [
            "extraction",
            "validation_maths",
            "ai_detection",
            "plagiarism",
            "rag",
            "verification",
        ];
        let order: Vec<String> =
            g.execution_order().expect("validates").iter().map(|a| a.id.0.clone()).collect();
        assert_eq!(&order[..6], &lanes[..], "run_pipeline_inner's sequence, unchanged");
    }

    /// Verification is the ONLY cloud agent and the ONLY reviser — the two
    /// properties §5.1 and CLAUDE.md both state, now checkable from data.
    #[test]
    fn verification_is_the_only_cloud_agent_and_the_only_reviser() {
        let g = shipped_graph();
        let cloud: Vec<&str> = g
            .agents
            .iter()
            .filter(|a| a.model == ModelRequirement::Cloud)
            .map(|a| a.id.0.as_str())
            .collect();
        assert_eq!(cloud, vec!["verification"], "only verification touches the network");

        let revisers: Vec<&str> =
            g.agents.iter().filter(|a| a.may_revise).map(|a| a.id.0.as_str()).collect();
        assert_eq!(
            revisers,
            vec!["verification"],
            "the other five produce MEASUREMENTS; a validator that changed its answer under \
             peer pressure would be broken, not collaborative (revising.rs)"
        );
    }

    /// Validation/Maths is the hard constraint, and it is deterministic — the
    /// §4.4 rule, now enforced by the validator rather than by convention.
    #[test]
    fn the_hard_constraint_is_deterministic() {
        let g = shipped_graph();
        let hard: Vec<&AgentSpec> = g.agents.iter().filter(|a| a.hard_constraint).collect();
        assert_eq!(hard.len(), 1);
        assert_eq!(hard[0].id.0, "validation_maths");
        assert_eq!(hard[0].model, ModelRequirement::None);
        assert_eq!(hard[0].trust_tier, TrustTier::Deterministic);
    }

    /// **THE SCIENTIFIC LAYER IS DECLINED, NOT PENDING — §11 D165.**
    ///
    /// This test has now been written three ways, and the history is the point.
    /// It first asserted nothing required the layer because nothing had been
    /// built to read it. It then asserted `frequentist_stats` required it,
    /// because a specialist existed. It now asserts the requirement is ABSENT
    /// again, for a reason neither earlier version had: **the layer was
    /// measured, and it fabricates.**
    ///
    /// Hand-checked, not sampled — all 152 `Method` objects across the six real
    /// manuscripts (`examples/methods_precision_probe.rs`):
    ///
    /// ```text
    /// span is a genuine method statement          9 / 152   5.9%
    ///   ... and `design` is correct               3 / 152   2.0%
    ///   ... and `n` and `software` are too        1 / 152   0.66%
    /// NO-SKILL (first paragraph of each Methods
    ///           section, 12 guesses)              6 /  12   50%
    /// ```
    ///
    /// **A one-line heuristic outscores nine hand-written regexes by 8.5x.**
    /// That is D128's shape and a starker version of it: D128 withdrew a lane
    /// at 18.3% against an 18.0% baseline, a difference of noise. This is a
    /// difference of an order of magnitude, in the baseline's favour.
    ///
    /// Turning the layer on would put 0.85-confidence structured claims about a
    /// manuscript's methodology in front of a researcher, sourced from table
    /// cells, Turnitin page footers and an author's middle initial. The
    /// condition that reopens it is D128's: a labelled set, a measured
    /// precision, and a no-skill comparison it beats.
    #[test]
    fn the_scientific_layer_is_declined_and_nothing_requires_it() {
        let g = shipped_graph();
        assert!(
            !g.requires_scientific_extraction(),
            "a shipped agent declares ScientificExtraction. The layer is DECLINED (§11 D165) \
             at 5.9% precision against a 50% no-skill baseline, not merely unbuilt — \
             reopening it needs a labelled set and a measured precision, which is the bar \
             D128 applied to citation_need"
        );
        assert!(
            g.derivable_requirements().is_empty(),
            "the harness must derive nothing: there is no derivable artifact it has evidence for"
        );
    }

    /// **The analysis record gates nothing, and must not.** Declared under
    /// `requires` it is supplied-and-absent, so both specialists would be
    /// skipped on every real run — measured: no upload path in the app accepts
    /// an analysis file, and the researcher corpus contains none.
    #[test]
    fn the_analysis_record_is_optional_for_both_specialists_and_gates_neither() {
        let g = shipped_graph();
        for id in ["frequentist_stats", "ml_methodology"] {
            let a = g.agents.iter().find(|a| a.id.0 == id).expect("present");
            assert!(
                a.optional.contains(&Artifact::AnalysisRecord),
                "{id} must declare the record OPTIONAL so its evidence policy may cite it"
            );
            assert!(
                !a.requires.contains(&Artifact::AnalysisRecord),
                "{id} must NOT require it — supplied + absent means the agent is never invoked, \
                 and it is absent on every run"
            );
            // The scientific layer is optional for the same structural reason
            // and a different evidential one: the record has no INPUT (no
            // upload path), the layer has no CREDIBILITY (§11 D165). Both
            // belong in `optional`, and neither may gate.
            assert!(
                !a.requires.contains(&Artifact::ScientificExtraction),
                "{id} must not require the declined layer"
            );
        }
    }
}
