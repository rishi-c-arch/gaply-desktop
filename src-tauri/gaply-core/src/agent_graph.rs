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
    /// Layers this agent may read. Reading outside them is the error the
    /// declaration exists to make visible.
    pub reads: Vec<Layer>,
    /// Artifacts it needs. See [`Artifact::is_derivable`].
    #[serde(default)]
    pub requires: Vec<Artifact>,
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
            reads: vec![Layer::ResearchState],
            requires: Vec::new(),
            after: Vec::new(),
            model: ModelRequirement::None,
            trust_tier: TrustTier::Deterministic,
            hard_constraint: false,
            may_revise: false,
            requires_consent: None,
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
        assert_eq!(g.agents.len(), 6, "the six existing lanes");
    }

    /// **The graph's order must be the order the pipeline actually runs.**
    ///
    /// `run_pipeline_inner` executes extraction, validation, ai, plagiarism,
    /// rag, verification — in that sequence, hardcoded. If the graph disagreed,
    /// it would be a description of something that does not happen, which is
    /// worse than no description: the next phase routes over it.
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
                "verification"
            ],
            "the graph must describe the order run_pipeline_inner really uses"
        );
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

    /// **No shipped agent requires the scientific layer, so the harness must
    /// not derive it.** That is the measured decision (2.8–150 ms for something
    /// nothing reads) expressed as data rather than as a flag someone has to
    /// remember. When the first specialist declares it, this test changes and
    /// the layer switches on for that graph — with no list to maintain.
    #[test]
    fn nothing_shipped_requires_the_scientific_layer_yet() {
        let g = shipped_graph();
        assert!(
            !g.requires_scientific_extraction(),
            "a shipped agent now declares ScientificExtraction — the harness will start \
             deriving it, which is correct, but the cost is 2.8-150 ms per manuscript \
             (examples/scientific_cost_probe.rs) and this test is where that becomes \
             deliberate"
        );
        assert!(g.derivable_requirements().is_empty());
    }
}
