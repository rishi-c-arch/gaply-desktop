//! **The workspace contract (Research OS, Phase 3), pure.** The
//! `WorkspaceState` edge table pair by pair, the descriptors this build
//! carries, the two checks every operation command relies on, and the wire.
//!
//! The edge table is the one in `contracts/workspace.rs`'s header and in the
//! Phase 3 D-entry: five edge kinds, fifteen edges over three workspaces. Every
//! other (state, event) pair of the 10 × 7 must be refused with
//! `workspace.illegal_transition`, naming the phase, the event and, when the
//! state names one, the workspace.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::workspace::{
    availability, check_spec, require, transition, ActivationPolicy, Availability, DisplayNameCode,
    WorkspaceCode, WorkspaceDescriptor, WorkspaceEvent, WorkspaceEventKind, WorkspacePhase,
    WorkspaceState,
};
use gaply_core::contracts::{CapabilityId, Os, ParamKey, ParamValue, WorkspaceId};
use gaply_core::stats_desc;
// Phase 4A: the descriptors carry the registry's capability ids (decision 47).
use gaply_core::stats_registry::{
    workspace_descriptor as descriptor, workspace_descriptors as descriptors, Registry,
};

const WS: [WorkspaceId; 3] = [
    WorkspaceId::Statistics,
    WorkspaceId::Compute,
    WorkspaceId::Design,
];

/// The listed edges, written out independently of `transition`.
fn listed(from: WorkspaceState, event: WorkspaceEvent) -> Option<WorkspaceState> {
    use WorkspaceEvent as E;
    use WorkspaceState as S;
    for w in WS {
        let edges = [
            (S::NoWorkspace, E::Activate(w), S::Activating(w)),
            (S::Activating(w), E::Activated, S::Active(w)),
            (S::Activating(w), E::ActivationRefused, S::NoWorkspace),
            (S::Active(w), E::Deactivate, S::Deactivating(w)),
            (S::Deactivating(w), E::Deactivated, S::NoWorkspace),
        ];
        for (f, e, t) in edges {
            if f == from && e == event {
                return Some(t);
            }
        }
    }
    None
}

#[test]
fn every_listed_edge_succeeds_and_every_other_pair_is_refused() {
    let states = WorkspaceState::all();
    let events = WorkspaceEvent::all();
    assert_eq!(states.len(), 10, "NoWorkspace and 3 phases × 3 workspaces");
    assert_eq!(events.len(), 7, "3 activations and 4 other events");

    let (mut ok, mut refused) = (0, 0);
    for &from in &states {
        for &event in &events {
            match (listed(from, event), transition(from, event)) {
                (Some(want), Ok(got)) => {
                    assert_eq!(got, want, "{from:?} –{event:?}→");
                    ok += 1;
                }
                (None, Err(d)) => {
                    assert_eq!(
                        d.code,
                        WorkspaceCode::IllegalTransition,
                        "{from:?} {event:?}"
                    );
                    assert_eq!(
                        d.params.get(&ParamKey::From),
                        Some(&ParamValue::Phase(from.phase()))
                    );
                    assert_eq!(
                        d.params.get(&ParamKey::Event),
                        Some(&ParamValue::WorkspaceEvent(event.kind()))
                    );
                    assert_eq!(
                        d.params.get(&ParamKey::Workspace),
                        from.workspace().map(ParamValue::Workspace).as_ref(),
                        "{from:?} {event:?}"
                    );
                    refused += 1;
                }
                (want, got) => panic!("{from:?} –{event:?}→ listed {want:?}, got {got:?}"),
            }
        }
    }
    assert_eq!((ok, refused), (15, 55), "15 edges, 55 refusals, 70 pairs");
}

#[test]
fn one_workspace_at_a_time_every_state_reachable_and_none_beside_another() {
    // Reachable from NoWorkspace through the table.
    let mut seen = BTreeSet::new();
    let mut frontier = vec![WorkspaceState::NoWorkspace];
    while let Some(s) = frontier.pop() {
        if !seen.insert(format!("{s:?}")) {
            continue;
        }
        for e in WorkspaceEvent::all() {
            if let Ok(t) = transition(s, e) {
                frontier.push(t);
            }
        }
    }
    assert_eq!(seen.len(), WorkspaceState::all().len(), "{seen:?}");

    // From a state that names a workspace, no event leads to a state naming
    // another one: the only way to a second workspace is through NoWorkspace.
    for s in WorkspaceState::all() {
        let Some(w) = s.workspace() else { continue };
        for e in WorkspaceEvent::all() {
            if let Ok(t) = transition(s, e) {
                assert!(
                    t.workspace().map_or(true, |x| x == w),
                    "{s:?} –{e:?}→ {t:?}"
                );
            }
        }
    }
}

#[test]
fn statistics_is_available_and_compute_and_design_are_missing_and_claim_nothing() {
    assert_eq!(
        availability(WorkspaceId::Statistics),
        Availability::Available
    );
    assert_eq!(availability(WorkspaceId::Compute), Availability::Missing);
    assert_eq!(availability(WorkspaceId::Design), Availability::Missing);

    assert_eq!(
        descriptor(WorkspaceId::Statistics),
        WorkspaceDescriptor {
            id: WorkspaceId::Statistics,
            version: 1,
            display_name_code: DisplayNameCode::Statistics,
            availability: Availability::Available,
            activation_policy: ActivationPolicy::SingleActive,
            capability_ids: Registry::statistics().ids(),
            supports_offline: true,
            supported_platforms: vec![Os::Macos, Os::Windows, Os::Linux],
        },
        "Statistics' capabilities are the registry's"
    );
    assert!(descriptor(WorkspaceId::Statistics)
        .capability_ids
        .contains(&stats_desc::capability_id()));
    for (id, name) in [
        (WorkspaceId::Compute, DisplayNameCode::Compute),
        (WorkspaceId::Design, DisplayNameCode::Design),
    ] {
        assert_eq!(
            descriptor(id),
            WorkspaceDescriptor {
                id,
                version: 0,
                display_name_code: name,
                availability: Availability::Missing,
                activation_policy: ActivationPolicy::SingleActive,
                capability_ids: vec![],
                supports_offline: false,
                supported_platforms: vec![],
            }
        );
    }
    assert_eq!(
        descriptors().iter().map(|d| d.id).collect::<Vec<_>>(),
        WorkspaceId::ALL
    );
    for d in descriptors() {
        assert_eq!(
            d.availability,
            availability(d.id),
            "one answer per workspace"
        );
        for c in &d.capability_ids {
            assert_eq!(c.workspace(), d.id, "{} is {:?}'s", c.as_str(), d.id);
        }
    }
}

#[test]
fn a_capability_id_must_name_a_workspace_this_build_has() {
    for w in WS {
        let id = format!("{}.solve", w.namespace());
        assert_eq!(
            CapabilityId::parse(&id).is_some(),
            availability(w) != Availability::Missing,
            "{id}"
        );
    }
    assert!(CapabilityId::parse("design.sketch").is_none());
}

#[test]
fn only_the_needed_active_workspace_passes_the_check() {
    for s in WorkspaceState::all() {
        for needed in WS {
            match require(s, needed) {
                Ok(()) => assert_eq!(s, WorkspaceState::Active(needed)),
                Err(d) => {
                    assert_ne!(s, WorkspaceState::Active(needed));
                    assert_eq!(d.code, WorkspaceCode::Inactive);
                    assert_eq!(
                        d.params.get(&ParamKey::Expected),
                        Some(&ParamValue::Workspace(needed))
                    );
                    assert_eq!(
                        d.params.get(&ParamKey::From),
                        Some(&ParamValue::Phase(s.phase()))
                    );
                }
            }
        }
    }
}

#[test]
fn a_spec_runs_only_in_the_active_workspace_for_its_own_capability() {
    let describe = stats_desc::capability_id();
    assert!(check_spec(WorkspaceId::Statistics, WorkspaceId::Statistics, &describe).is_ok());

    for forged in [WorkspaceId::Compute, WorkspaceId::Design] {
        let d = check_spec(WorkspaceId::Statistics, forged, &describe).expect_err("forged");
        assert_eq!(d.code, WorkspaceCode::WrongWorkspace);
        assert_eq!(
            d.params.get(&ParamKey::Expected),
            Some(&ParamValue::Workspace(WorkspaceId::Statistics))
        );
        assert_eq!(
            d.params.get(&ParamKey::Found),
            Some(&ParamValue::Workspace(forged))
        );
        // Active in another workspace, a Statistics capability is refused
        // even when the spec says that other workspace.
        let d = check_spec(forged, forged, &describe).expect_err("capability elsewhere");
        assert_eq!(d.code, WorkspaceCode::WrongWorkspace);
        assert_eq!(
            d.params.get(&ParamKey::Found),
            Some(&ParamValue::Workspace(WorkspaceId::Statistics))
        );
    }
}

#[test]
fn the_wire_words_and_shapes_are_pinned() {
    let words = |all: &[&str]| all.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    assert_eq!(
        WorkspaceCode::ALL
            .iter()
            .map(|c| c.wire())
            .collect::<Vec<_>>(),
        words(&[
            "workspace.unavailable",
            "workspace.not_ready",
            "workspace.inactive",
            "workspace.wrong_workspace",
            "workspace.run_active",
            "workspace.illegal_transition",
        ])
    );
    assert_eq!(
        WorkspacePhase::ALL
            .iter()
            .map(|c| c.wire())
            .collect::<Vec<_>>(),
        words(&["no_workspace", "activating", "active", "deactivating"])
    );
    assert_eq!(
        WorkspaceEventKind::ALL
            .iter()
            .map(|c| c.wire())
            .collect::<Vec<_>>(),
        words(&[
            "activate",
            "activated",
            "activation_refused",
            "deactivate",
            "deactivated"
        ])
    );
    assert_eq!(
        DisplayNameCode::ALL
            .iter()
            .map(|c| c.wire())
            .collect::<Vec<_>>(),
        words(&[
            "workspace.name.statistics",
            "workspace.name.compute",
            "workspace.name.design"
        ])
    );

    assert_eq!(
        serde_json::to_value(WorkspaceState::NoWorkspace).unwrap(),
        json!({ "phase": "no_workspace" })
    );
    assert_eq!(
        serde_json::to_value(WorkspaceState::Active(WorkspaceId::Statistics)).unwrap(),
        json!({ "phase": "active", "workspace": "statistics" })
    );
    for s in WorkspaceState::all() {
        let v = serde_json::to_value(s).unwrap();
        assert_eq!(serde_json::from_value::<WorkspaceState>(v).unwrap(), s);
    }
    for bad in [
        json!({ "phase": "active", "workspace": "statistics", "extra": 1 }),
        json!({ "phase": "active", "workspace": "geometry" }),
        json!({ "phase": "open", "workspace": "statistics" }),
        json!({ "phase": "active" }),
    ] {
        assert!(
            serde_json::from_value::<WorkspaceState>(bad.clone()).is_err(),
            "{bad}"
        );
    }

    let d = serde_json::to_value(descriptor(WorkspaceId::Compute)).unwrap();
    let keys: Vec<&String> = d.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        [
            "activationPolicy",
            "availability",
            "capabilityIds",
            "displayNameCode",
            "id",
            "supportedPlatforms",
            "supportsOffline",
            "version"
        ]
    );
    assert_eq!(d["availability"], "missing");
    assert_eq!(d["activationPolicy"], "single_active");
    let mut extra: Value = d.clone();
    extra["entitled"] = json!(true);
    assert!(
        serde_json::from_value::<WorkspaceDescriptor>(extra).is_err(),
        "an unknown field is refused"
    );
    for d in descriptors() {
        let back: WorkspaceDescriptor =
            serde_json::from_value(serde_json::to_value(&d).unwrap()).unwrap();
        assert_eq!(back, d);
    }

    // A refusal's parameters read back as what they were.
    let d = transition(
        WorkspaceState::Active(WorkspaceId::Statistics),
        WorkspaceEvent::Activate(WorkspaceId::Compute),
    )
    .unwrap_err();
    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(
        json,
        json!({
            "code": "workspace.illegal_transition",
            "params": { "from": "active", "event": "activate", "workspace": "statistics" },
            "severity": "error"
        })
    );
    assert_eq!(
        serde_json::from_value::<gaply_core::contracts::Diagnostic<WorkspaceCode>>(json).unwrap(),
        d
    );
}
