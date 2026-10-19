//! **The provenance graph is acyclic whatever is offered to it (Research OS
//! Phase 5B-1, ADR-009 §7).**
//!
//! A property test (proptest, decision 3): random edges over six runs, eight
//! artifacts and the app as agent, offered in random order to
//! `ProvGraph::add`. After every offer the graph must be acyclic by two
//! checks that share no code with `add`: `ProvGraph::is_acyclic` (Kahn's
//! algorithm) and this file's own depth-first search. And every edge `add`
//! refuses as a cycle must really close one: the search finds a cycle in the
//! graph with that edge put in, so the refusal is never a false alarm.
//!
//! It lives here, not beside the module, because `tests/contracts_are_pure.rs`
//! holds `research_project`'s source, its tests included, to `std`, `serde`
//! and `crate::contracts`.

use std::collections::{BTreeMap, BTreeSet};

use gaply_core::research_project::{
    AgentVersion, ArtifactHash, ProjectCode, ProvEdge, ProvGraph, ProvNode, StoredRunId, UsedRole,
};
use proptest::prelude::*;

fn hash_of(i: u8) -> ArtifactHash {
    ArtifactHash::parse(&format!("{i:064x}")).expect("hash")
}

fn edge(kind: u8, run: u8, a: u8, b: u8) -> ProvEdge {
    let run = StoredRunId(u64::from(run % 6));
    match kind % 4 {
        0 => ProvEdge::Used {
            run,
            artifact: hash_of(a % 8),
            role: UsedRole::Data,
        },
        1 => ProvEdge::WasGeneratedBy {
            artifact: hash_of(a % 8),
            run,
        },
        2 => ProvEdge::WasDerivedFrom {
            artifact: hash_of(a % 8),
            from: hash_of(b % 8),
            via: run,
        },
        _ => ProvEdge::WasAssociatedWith {
            run,
            agent: AgentVersion::parse("0.1.0").expect("agent"),
        },
    }
}

/// A cycle search of this file's own: depth-first, three colours.
fn has_cycle<'a>(edges: impl Iterator<Item = &'a ProvEdge>) -> bool {
    let mut adj: BTreeMap<ProvNode, Vec<ProvNode>> = BTreeMap::new();
    for e in edges {
        let (a, b) = e.ends();
        adj.entry(b.clone()).or_default();
        adj.entry(a).or_default().push(b);
    }
    fn visit(
        n: &ProvNode,
        adj: &BTreeMap<ProvNode, Vec<ProvNode>>,
        grey: &mut BTreeSet<ProvNode>,
        black: &mut BTreeSet<ProvNode>,
    ) -> bool {
        if black.contains(n) {
            return false;
        }
        if !grey.insert(n.clone()) {
            return true;
        }
        for m in &adj[n] {
            if visit(m, adj, grey, black) {
                return true;
            }
        }
        grey.remove(n);
        black.insert(n.clone());
        false
    }
    let (mut grey, mut black) = (BTreeSet::new(), BTreeSet::new());
    adj.keys().any(|n| visit(n, &adj, &mut grey, &mut black))
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn every_graph_add_builds_is_acyclic_and_every_cycle_refusal_is_real(
        offered in proptest::collection::vec((any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>()), 0..60)
    ) {
        let mut g = ProvGraph::new();
        for (k, r, a, b) in offered {
            let e = edge(k, r, a, b);
            match g.add(e.clone()) {
                Ok(()) => {}
                Err(ProjectCode::ProvCycle) => {
                    prop_assert!(
                        has_cycle(g.edges().iter().chain(std::iter::once(&e))),
                        "refused an edge that closes no cycle"
                    );
                }
                Err(ProjectCode::ProvUnlinkedDerivation) => {}
                Err(other) => prop_assert!(false, "unexpected {other:?}"),
            }
            prop_assert!(g.is_acyclic());
            prop_assert!(!has_cycle(g.edges().iter()));
        }
    }
}

#[test]
fn the_search_finds_a_cycle_and_passes_a_chain() {
    let used = edge(0, 1, 2, 0);
    let generated = edge(1, 1, 2, 0);
    assert!(
        has_cycle([used.clone(), generated].iter()),
        "run 1 used and made artifact 2"
    );
    assert!(!has_cycle([used].iter()));
}
