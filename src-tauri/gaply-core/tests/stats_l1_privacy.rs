//! **A cell value and a level label never appear in the outbound schema, in
//! any error, or in tracing output.**
//!
//! Two sentinels that cannot occur by accident — `CELL_SECRET_7d9` as a cell
//! value and `LEVEL_SECRET_b18` as a level of a group column — go through the
//! whole Step 2 flow: typing, the aliased schema, local proposals, every
//! rejection a binding can draw, confirmation, the gate, a stale token. The
//! test then reads what could leave: the serialised `AliasedSchema`, the
//! `Debug` form of every error, and every tracing event and span emitted
//! during the flow, captured by a subscriber written here (no
//! `tracing-subscriber` in this crate), and asserts neither sentinel is in
//! any of them.
//!
//! The capture asserts a POSITIVE count of events first: a flow that emits
//! nothing would pass the sentinel check vacuously.
//!
//! This binary holds ONE test on purpose. `tracing` caches callsite interest
//! process-wide, so a callsite first hit on another test's thread can be
//! skipped by this capture; alone, with `Interest::sometimes` and an interest
//! rebuild once the capture is set, nothing else can hit a callsite first.

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use gaply_core::extract::SectionKind;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::Family;
use gaply_core::stats_l1::binding::{BindingDraft, OlsTarget, ProposalSource, TransformationRequest};
use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, VariantChoice, VariantKind};
use gaply_core::stats_l1::claim::{ClaimId, ClaimLocation, ClaimOrigin, L1Claim, UserClaim, UserP};
use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, RawTable, RawVariable, TableRef};
use gaply_core::stats_l1::compute::compute;
use gaply_core::stats_l1::reproduce::reproduce;
use gaply_core::stats_l1::run_state::L1RunState;
use gaply_core::stats_l1::token::ConfirmationToken;

const CELL: &str = "CELL_SECRET_7d9";
const LEVEL: &str = "LEVEL_SECRET_b18";

/* ---- a tracing subscriber that keeps every field it is shown ------------ */

struct Capture {
    lines: Arc<Mutex<Vec<String>>>,
}

struct Visitor(String);

impl tracing::field::Visit for Visitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, "{}={:?} ", field.name(), value);
    }
}

impl tracing::Subscriber for Capture {
    /// `sometimes`, never a cached `always`/`never`, so a callsite first hit
    /// elsewhere is still asked about this capture.
    fn register_callsite(&self, _: &'static tracing::Metadata<'static>) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut v = Visitor(format!("span {} ", attrs.metadata().name()));
        attrs.record(&mut v);
        self.lines.lock().unwrap().push(v.0);
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        let mut v = Visitor(String::from("record "));
        values.record(&mut v);
        self.lines.lock().unwrap().push(v.0);
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut v = Visitor(format!("event {} ", event.metadata().target()));
        event.record(&mut v);
        self.lines.lock().unwrap().push(v.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/* ---- the flow ----------------------------------------------------------- */

fn claim(id: usize, sentence: &str) -> L1Claim {
    L1Claim::user_entered(
        ClaimId(id),
        &UserClaim {
            family: Family::T,
            statistic: "2.10",
            df: vec!["3"],
            n: None,
            p: UserP::Reported { raw: ".04", relation: Relation::Eq },
            location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: id },
            sentence: Some(sentence),
        },
        ClaimOrigin::UserAdded,
    )
    .unwrap()
}

/// Everything that could leave, as strings.
fn run_flow() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    // `group` has three levels (the local proposer will not pick it for a t);
    // `g2` has two, so the proposer has a candidate.
    let mut raw = RawTable::from_rows(
        vec!["score".into(), "group".into(), "note".into(), "g2".into()],
        vec![
            vec!["1".into(), LEVEL.into(), CELL.into(), "p".into()],
            vec!["2".into(), "b".into(), CELL.into(), "q".into()],
            vec!["3".into(), LEVEL.into(), "x".into(), "p".into()],
            vec!["4".into(), "b".into(), CELL.into(), "q".into()],
            vec!["".into(), LEVEL.into(), "y".into(), "p".into()],
            vec!["6".into(), "c".into(), "z".into(), "q".into()],
        ],
    );
    raw.variables = vec![
        RawVariable::default(),
        RawVariable { value_labels: vec![(LEVEL.into(), LEVEL.into())], ..Default::default() },
        RawVariable::default(),
        RawVariable::default(),
    ];
    let dataset = build_dataset(raw, DataFingerprint::of_bytes(CELL.as_bytes(), "p", TableRef::FirstTable)).unwrap();
    out.push(serde_json::to_string(&dataset.schema.aliased()).unwrap());

    let mut s = L1RunState::new();
    s.load_dataset(dataset);
    s.set_claims(vec![claim(0, &format!("The {LEVEL} group scored higher, t(3) = 2.10, p = .04.")), claim(1, "t(3) = 2.10, p = .04.")]);

    // Local proposals; then a manual binding whose filter and level selection
    // NAME the sentinel level (the group has three levels, so a t needs the
    // filter), confirmed, and the gate.
    let ids = s.propose_local(ClaimId(0)).unwrap();
    assert!(!ids.is_empty());
    out.push(format!("{:?}", s.confirm(ids[0]).map(|(b, _)| b.counts())));
    let manual = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
        .role(RoleName::Outcome, &[ColumnAlias(1)])
        .role(RoleName::Group, &[ColumnAlias(2)])
        .filter(ColumnAlias(2), &[LEVEL, "b"])
        .levels(RoleName::Group, &[LEVEL, "b"]);
    let id = s.add_proposal(ClaimId(0), manual, ProposalSource::Manual { edited_from: None }).unwrap();
    let (confirmed, token) = s.confirm(id).unwrap();
    assert_eq!(confirmed.counts().rows_filtered_out, 1, "{:?}", confirmed.counts());
    out.push(format!("{token:?}"));
    // The computation and the whole Level 1 outcome (Phase 2, Step 4; they
    // replaced Step 2's compute stub), on the data read again.
    let reread = s.dataset().expect("loaded").clone();
    out.push(format!("{:?}", compute(&s, &token, ClaimId(0), &reread)));
    out.push(format!("{:?}", compute(&s, &token, ClaimId(1), &reread)));
    out.push(format!("{:?}", compute(&s, &ConfirmationToken::presented("forged"), ClaimId(0), &reread)));
    let revision = s.claim(ClaimId(0)).expect("claim").revision.clone();
    out.push(format!("{:?}", reproduce(&s, &[(revision, token.clone())], || Ok(reread.clone()))));

    // Every rejection a draft can draw, each as an error string.
    let t = || BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T));
    let drafts = vec![
        t().role(RoleName::Outcome, &[ColumnAlias(1)]),
        t().role(RoleName::Outcome, &[ColumnAlias(3)]).role(RoleName::Group, &[ColumnAlias(2)]),
        t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(2)]).levels(RoleName::Group, &[LEVEL]),
        t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(2)]).levels(RoleName::Group, &[CELL]),
        t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(2)]).filter(ColumnAlias(2), &[CELL]),
        t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(1)]),
        t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(9)]),
        t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(2)]).target(OlsTarget::OverallF),
        BindingDraft { transformations: vec![TransformationRequest::Log], ..t().role(RoleName::Outcome, &[ColumnAlias(1)]).role(RoleName::Group, &[ColumnAlias(2)]) },
        BindingDraft { method_id: CELL.into(), ..t() },
    ];
    for d in drafts {
        let id = s.add_proposal(ClaimId(0), d, ProposalSource::Manual { edited_from: None }).unwrap();
        out.push(format!("{:?}", s.confirm(id).map(|(b, _)| b.counts())));
    }
    // A stale token after an edit.
    let edited = UserClaim {
        family: Family::T,
        statistic: "2.11",
        df: vec!["3"],
        n: None,
        p: UserP::Reported { raw: ".04", relation: Relation::Eq },
        location: ClaimLocation { section_index: 0, section: SectionKind::Results, paragraph: 0, sentence: 0 },
        sentence: Some(LEVEL),
    };
    out.push(format!("{:?}", s.edit_claim(ClaimId(0), &edited).map(|r| r.as_str().len())));
    out.push(format!("{:?}", compute(&s, &token, ClaimId(0), &reread)));
    out.push(format!("{:?}", s.propose_local(ClaimId(7))));
    out.push(format!("{:?}", s.edit_claim(ClaimId(7), &edited)));
    out
}

#[test]
fn no_cell_value_or_level_label_reaches_the_aliased_schema_an_error_or_a_trace() {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let outputs = tracing::subscriber::with_default(Capture { lines: Arc::clone(&lines) }, || {
        // Callsites cached as `never` before this subscriber existed are
        // asked again now that it does.
        tracing::callsite::rebuild_interest_cache();
        run_flow()
    });
    let traced: Vec<String> = lines.lock().unwrap().clone();

    assert!(outputs.len() >= 15, "the flow produced only {} outputs", outputs.len());
    assert!(traced.len() >= 3, "no tracing events were captured — the sentinel check would be vacuous");
    assert!(traced.iter().any(|l| l.contains("dataset typed")), "{traced:?}");
    assert!(traced.iter().any(|l| l.contains("binding confirmed")), "{traced:?}");

    for s in outputs.iter().chain(traced.iter()) {
        assert!(!s.contains(CELL), "a cell value leaked: {s}");
        assert!(!s.contains(LEVEL), "a level label leaked: {s}");
    }
    // The aliased schema is exactly the four fields.
    assert_eq!(
        outputs[0],
        r#"{"columns":[{"alias":"C1","type":"integer","has_missing":true,"levels":"three_to_ten"},{"alias":"C2","type":"categorical","has_missing":false,"levels":"three_to_ten"},{"alias":"C3","type":"categorical","has_missing":false,"levels":"three_to_ten"},{"alias":"C4","type":"categorical","has_missing":false,"levels":"two"}]}"#
    );

    // The check must be able to fail: a string that does carry the sentinel
    // is caught by the same assertion. (Folded in here so this binary holds
    // one test: see the header.)
    let leaked = format!("event stats_l1 levels=[{LEVEL:?}]");
    assert!(leaked.contains(LEVEL));
}
