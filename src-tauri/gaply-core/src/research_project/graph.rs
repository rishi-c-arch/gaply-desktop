//! **The provenance graph: artifact versions, runs and the app as agent,
//! with the edges `used`, `wasGeneratedBy`, `wasDerivedFrom` and
//! `wasAssociatedWith` (blueprint §10.4; ADR-009 §7).**
//!
//! An artifact version is its hash: a new version of a file is a new hash.
//! The agent is the app and its version, never a person.
//!
//! Acyclic by construction: [`ProvGraph::add`] refuses an edge that would
//! close a cycle (`project.prov_cycle`), so every graph this type holds is a
//! DAG, and [`ProvGraph::is_acyclic`] checks it independently with Kahn's
//! algorithm. Each edge points from what depends to what it depends on: a run
//! to what it used and to its agent, an artifact to the run that generated it
//! and to what it was derived from. The PROV JSON export is 5B-2's
//! (decision 111).

use std::collections::{BTreeMap, BTreeSet};

use super::{AgentVersion, ArtifactHash, ProjectCode, StoredRunId, UsedRole};

/// A node of the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProvNode {
    Artifact(ArtifactHash),
    Run(StoredRunId),
    Agent(AgentVersion),
}

/// An edge of the graph, in PROV's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvEdge {
    /// The run read the artifact, in a role.
    Used {
        run: StoredRunId,
        artifact: ArtifactHash,
        role: UsedRole,
    },
    /// The run produced the artifact.
    WasGeneratedBy {
        artifact: ArtifactHash,
        run: StoredRunId,
    },
    /// The artifact was derived from another, by a run that used the one and
    /// generated the other.
    WasDerivedFrom {
        artifact: ArtifactHash,
        from: ArtifactHash,
        via: StoredRunId,
    },
    /// The run was carried out by the app at a version.
    WasAssociatedWith {
        run: StoredRunId,
        agent: AgentVersion,
    },
}

impl ProvEdge {
    /// The edge as (dependent, dependency).
    pub fn ends(&self) -> (ProvNode, ProvNode) {
        match self {
            ProvEdge::Used { run, artifact, .. } => {
                (ProvNode::Run(*run), ProvNode::Artifact(artifact.clone()))
            }
            ProvEdge::WasGeneratedBy { artifact, run } => {
                (ProvNode::Artifact(artifact.clone()), ProvNode::Run(*run))
            }
            ProvEdge::WasDerivedFrom { artifact, from, .. } => (
                ProvNode::Artifact(artifact.clone()),
                ProvNode::Artifact(from.clone()),
            ),
            ProvEdge::WasAssociatedWith { run, agent } => {
                (ProvNode::Run(*run), ProvNode::Agent(agent.clone()))
            }
        }
    }
}

/// A provenance graph that never holds a cycle.
#[derive(Clone, Default)]
pub struct ProvGraph {
    edges: Vec<ProvEdge>,
    /// One adjacency map (dependent -> its dependencies), kept in step with
    /// `edges` by [`ProvGraph::add`] so a cycle check never rebuilds it
    /// (5E-fix-1, F3). [`ProvGraph::push_unchecked`] adds to `edges` only, so
    /// `adj` can lack an edge `edges` holds; only [`ProvGraph::is_acyclic`],
    /// which rebuilds its own adjacency from `edges`, reads such a graph.
    adj: BTreeMap<ProvNode, Vec<ProvNode>>,
}

// How many times `ProvGraph::adjacency` rebuilt the whole map on this thread,
// for the test that pins `add` not to rebuild it (5E-fix-1, F3).
#[cfg(test)]
thread_local! {
    static ADJACENCY_REBUILDS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn adjacency_rebuilds() -> u32 {
    ADJACENCY_REBUILDS.with(|c| c.get())
}

#[cfg(test)]
pub(crate) fn reset_adjacency_rebuilds() {
    ADJACENCY_REBUILDS.with(|c| c.set(0));
}

impl std::fmt::Debug for ProvGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ProvGraph({} edges)", self.edges.len())
    }
}

impl ProvGraph {
    pub fn new() -> ProvGraph {
        ProvGraph::default()
    }

    /// A graph of these edges, in order; the first that would close a cycle
    /// or break a derivation's link is refused with its code.
    pub fn from_edges(edges: impl IntoIterator<Item = ProvEdge>) -> Result<ProvGraph, ProjectCode> {
        let mut g = ProvGraph::new();
        for e in edges {
            g.add(e)?;
        }
        Ok(g)
    }

    pub fn edges(&self) -> &[ProvEdge] {
        &self.edges
    }

    /// Adds an edge. An edge already present is a no-op. Refused:
    /// a self-loop or an edge whose dependency already reaches its dependent
    /// (`prov_cycle`); a derivation whose run did not use its source and
    /// generate its artifact (`prov_unlinked_derivation`).
    pub fn add(&mut self, edge: ProvEdge) -> Result<(), ProjectCode> {
        if self.edges.contains(&edge) {
            return Ok(());
        }
        let (from, to) = edge.ends();
        if from == to || self.reaches(&to, &from) {
            return Err(ProjectCode::ProvCycle);
        }
        if let ProvEdge::WasDerivedFrom {
            artifact,
            from: source,
            via,
        } = &edge
        {
            let used = self.edges.iter().any(
                |e| matches!(e, ProvEdge::Used { run, artifact: a, .. } if run == via && a == source),
            );
            let generated = self.edges.iter().any(
                |e| matches!(e, ProvEdge::WasGeneratedBy { artifact: a, run } if run == via && a == artifact),
            );
            if !(used && generated) {
                return Err(ProjectCode::ProvUnlinkedDerivation);
            }
        }
        // F3: keep the adjacency in step with the edges, so no later `add`
        // rebuilds it. `to` is both a key of its own and a value under `from`.
        self.adj.entry(to.clone()).or_default();
        self.adj.entry(from).or_default().push(to);
        self.edges.push(edge);
        Ok(())
    }

    fn adjacency(&self) -> BTreeMap<ProvNode, Vec<ProvNode>> {
        #[cfg(test)]
        ADJACENCY_REBUILDS.with(|c| c.set(c.get() + 1));
        let mut adj: BTreeMap<ProvNode, Vec<ProvNode>> = BTreeMap::new();
        for e in &self.edges {
            let (a, b) = e.ends();
            adj.entry(b.clone()).or_default();
            adj.entry(a).or_default().push(b);
        }
        adj
    }

    /// Whether `goal` is reachable from `start`, over the maintained
    /// adjacency (F3: never a rebuild).
    fn reaches(&self, start: &ProvNode, goal: &ProvNode) -> bool {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start.clone()];
        while let Some(n) = stack.pop() {
            if &n == goal {
                return true;
            }
            if seen.insert(n.clone()) {
                if let Some(next) = self.adj.get(&n) {
                    stack.extend(next.iter().cloned());
                }
            }
        }
        false
    }

    /// Kahn's algorithm: true when every node can be removed in dependency
    /// order, so no cycle exists. Independent of [`ProvGraph::add`]'s check.
    pub fn is_acyclic(&self) -> bool {
        let adj = self.adjacency();
        let mut indegree: BTreeMap<&ProvNode, usize> = adj.keys().map(|k| (k, 0)).collect();
        for targets in adj.values() {
            for t in targets {
                *indegree.get_mut(t).expect("every target is a key") += 1;
            }
        }
        let mut ready: Vec<&ProvNode> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut removed = 0;
        while let Some(n) = ready.pop() {
            removed += 1;
            for t in &adj[n] {
                let d = indegree.get_mut(t).expect("every target is a key");
                *d -= 1;
                if *d == 0 {
                    ready.push(t);
                }
            }
        }
        removed == adj.len()
    }

    /// The `wasDerivedFrom` edges the `used` and `wasGeneratedBy` edges
    /// imply (Phase 5B-2): every artifact a run generated was derived, by that
    /// run, from every artifact it used. Sorted, and without any already
    /// present. The store keeps no derivation for a Save; the PROV JSON is
    /// given these.
    pub fn derivations(&self) -> Vec<ProvEdge> {
        let mut found: BTreeSet<(StoredRunId, ArtifactHash, ArtifactHash)> = BTreeSet::new();
        for g in &self.edges {
            let ProvEdge::WasGeneratedBy { artifact, run } = g else {
                continue;
            };
            for u in &self.edges {
                if let ProvEdge::Used {
                    run: r,
                    artifact: from,
                    ..
                } = u
                {
                    if r == run && from != artifact {
                        found.insert((*run, artifact.clone(), from.clone()));
                    }
                }
            }
        }
        found
            .into_iter()
            .map(|(via, artifact, from)| ProvEdge::WasDerivedFrom {
                artifact,
                from,
                via,
            })
            .filter(|e| !self.edges.contains(e))
            .collect()
    }

    /// For tests of [`ProvGraph::is_acyclic`]: pushes an edge with no check,
    /// as a forged or corrupted store could hold one.
    #[cfg(test)]
    pub(crate) fn push_unchecked(&mut self, edge: ProvEdge) {
        self.edges.push(edge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(c: char) -> ArtifactHash {
        ArtifactHash::parse(&c.to_string().repeat(64)).expect("hash")
    }

    fn agent() -> AgentVersion {
        AgentVersion::parse("0.1.0").expect("agent")
    }

    #[test]
    fn a_run_that_used_its_own_output_is_refused_as_a_cycle() {
        let mut g = ProvGraph::new();
        let r = StoredRunId(1);
        g.add(ProvEdge::Used {
            run: r,
            artifact: h('a'),
            role: UsedRole::Data,
        })
        .expect("used");
        assert_eq!(
            g.add(ProvEdge::WasGeneratedBy {
                artifact: h('a'),
                run: r
            }),
            Err(ProjectCode::ProvCycle)
        );
        assert_eq!(
            g.add(ProvEdge::WasDerivedFrom {
                artifact: h('b'),
                from: h('b'),
                via: r
            }),
            Err(ProjectCode::ProvCycle),
            "a self-loop"
        );
        assert!(g.is_acyclic());
    }

    #[test]
    fn a_chain_of_runs_with_derivations_and_an_agent_is_accepted() {
        let g = ProvGraph::from_edges([
            ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('a'),
                role: UsedRole::Data,
            },
            ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('e'),
                role: UsedRole::Spec,
            },
            ProvEdge::WasGeneratedBy {
                artifact: h('b'),
                run: StoredRunId(1),
            },
            ProvEdge::WasDerivedFrom {
                artifact: h('b'),
                from: h('a'),
                via: StoredRunId(1),
            },
            ProvEdge::WasAssociatedWith {
                run: StoredRunId(1),
                agent: agent(),
            },
            ProvEdge::Used {
                run: StoredRunId(2),
                artifact: h('b'),
                role: UsedRole::Data,
            },
            ProvEdge::WasGeneratedBy {
                artifact: h('c'),
                run: StoredRunId(2),
            },
        ])
        .expect("a DAG");
        assert!(g.is_acyclic());
        assert_eq!(g.edges().len(), 7);
        let mut g2 = g.clone();
        assert_eq!(
            g2.add(ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('c'),
                role: UsedRole::Data
            }),
            Err(ProjectCode::ProvCycle),
            "run 1 using what run 2 made from run 1's output"
        );
    }

    #[test]
    fn a_derivation_needs_its_run_to_link_both_ends() {
        let mut g = ProvGraph::new();
        assert_eq!(
            g.add(ProvEdge::WasDerivedFrom {
                artifact: h('b'),
                from: h('a'),
                via: StoredRunId(1)
            }),
            Err(ProjectCode::ProvUnlinkedDerivation)
        );
    }

    #[test]
    fn derivations_follow_from_used_and_generated_and_add_no_cycle() {
        let mut g = ProvGraph::from_edges([
            ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('a'),
                role: UsedRole::Data,
            },
            ProvEdge::Used {
                run: StoredRunId(1),
                artifact: h('e'),
                role: UsedRole::Spec,
            },
            ProvEdge::WasGeneratedBy {
                artifact: h('b'),
                run: StoredRunId(1),
            },
            ProvEdge::WasGeneratedBy {
                artifact: h('c'),
                run: StoredRunId(1),
            },
            ProvEdge::Used {
                run: StoredRunId(2),
                artifact: h('a'),
                role: UsedRole::Data,
            },
        ])
        .expect("a DAG");
        let d = g.derivations();
        let pairs: Vec<(char, char)> = d
            .iter()
            .map(|e| match e {
                ProvEdge::WasDerivedFrom {
                    artifact,
                    from,
                    via,
                } => {
                    assert_eq!(*via, StoredRunId(1));
                    (
                        artifact.as_hex().chars().next().unwrap(),
                        from.as_hex().chars().next().unwrap(),
                    )
                }
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(pairs, [('b', 'a'), ('b', 'e'), ('c', 'a'), ('c', 'e')]);
        for e in d {
            g.add(e).expect("each links its run's two ends");
        }
        assert!(g.is_acyclic());
        assert!(g.derivations().is_empty(), "none left to add");
    }

    #[test]
    fn kahn_finds_a_forged_back_edge() {
        let mut g = ProvGraph::new();
        g.add(ProvEdge::Used {
            run: StoredRunId(1),
            artifact: h('a'),
            role: UsedRole::Data,
        })
        .expect("used");
        assert!(g.is_acyclic());
        g.push_unchecked(ProvEdge::WasGeneratedBy {
            artifact: h('a'),
            run: StoredRunId(1),
        });
        assert!(!g.is_acyclic());
    }

    #[test]
    fn add_maintains_the_adjacency_without_rebuilding_it() {
        reset_adjacency_rebuilds();
        let mut g = ProvGraph::new();
        g.add(ProvEdge::Used {
            run: StoredRunId(1),
            artifact: h('a'),
            role: UsedRole::Data,
        })
        .unwrap();
        g.add(ProvEdge::WasGeneratedBy {
            artifact: h('b'),
            run: StoredRunId(1),
        })
        .unwrap();
        g.add(ProvEdge::WasAssociatedWith {
            run: StoredRunId(1),
            agent: agent(),
        })
        .unwrap();
        g.add(ProvEdge::Used {
            run: StoredRunId(2),
            artifact: h('b'),
            role: UsedRole::Data,
        })
        .unwrap();
        g.add(ProvEdge::WasGeneratedBy {
            artifact: h('c'),
            run: StoredRunId(2),
        })
        .unwrap();
        assert_eq!(
            adjacency_rebuilds(),
            0,
            "F3: add rebuilt the whole adjacency"
        );
        // The maintained map is exactly a rebuild from the edges.
        assert_eq!(g.adj, g.adjacency());
    }
}
