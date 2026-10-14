//! Fixtures shared by `stats_assist.rs` and the three sentinel binaries
//! (`stats_assist_privacy_*.rs`): a dataset and a manuscript carrying Step 5's
//! sentinels, a world with a confirmed and an unbound result, every task's
//! outbound, and the tracing capture the sentinel tests read.
//!
//! Each test that asserts something is ABSENT from captured tracing lives
//! alone in its own binary: `tracing` caches callsite interest process-wide,
//! and a callsite first hit on another test's thread can be skipped by a
//! capture. Measured 14 Oct 2026 in the app crate's equivalent: 6 to 22
//! captured lines across eight runs of one flow while it shared a binary.
#![allow(dead_code, unused_imports)]

use std::fmt::Write as _;
pub use std::sync::{Arc, Mutex};

pub use serde_json::{json, Value};

pub use gaply_core::extract::{ExtractionResult, Section, SectionKind};
pub use gaply_core::stats_assist::citation::{parse_web_context, CitationDrop, IndexUnit, RetrievedAt, NO_CITED_SOURCES};
pub use gaply_core::stats_assist::consent::{AiHelpSetting, DataCategory, HelperRunId, OptIns};
pub use gaply_core::stats_assist::outbound::{
    build_binding_suggestion, build_claim_method, build_result_explanation, build_web_context, BuildRefusal, Outbound,
};
pub use gaply_core::stats_assist::query::{check_query, template_query, ContextTopic, QueryBlocked, QueryScan};
pub use gaply_core::stats_assist::response::{
    read_response, validate_binding_suggestion, validate_claim_method, validate_explanation, Rejection,
};
pub use gaply_core::stats_assist::session::{ExplanationGap, ExplanationOutcome, HelperSession, PreviewId, SendRefusal};
pub use gaply_core::stats_assist::spec::{allowed_web_domains, HelperTask, ModelId};
pub use gaply_core::stats_assist::transport::{MockTransport, TransportError};
pub use gaply_core::stats_l0::input::DocumentContext;
pub use gaply_core::stats_l0::pvalue;
pub use gaply_core::stats_l0::result::{read_results, TableStructure};
pub use gaply_core::stats_l1::binding::{BindingDraft, ProposalSource};
pub use gaply_core::stats_l1::catalogue::{L1MethodId, RoleName, Variant, VariantChoice, VariantKind};
pub use gaply_core::stats_l1::claim::{claims_from_reading, ClaimId, L1Claim};
pub use gaply_core::stats_l1::dataset::{build_dataset, ColumnAlias, DataFingerprint, Dataset, RawTable, TableRef};
pub use gaply_core::stats_l1::reproduce::{reproduce, ClaimReproduction, L1Verdict};
pub use gaply_core::stats_l1::run_state::L1RunState;
pub use gaply_core::stats_verify;

pub const CELL: &str = "CELL_SECRET_7d9";
pub const LEVEL: &str = "LEVEL_SECRET_b18";
pub const HEADER: &str = "HEADER_SECRET_c42";
pub const MS: &str = "MS_SECRET_e55";
pub const PATH: &str = "PATH_SECRET_f61";
pub const SENTINELS: [&str; 5] = [CELL, LEVEL, HEADER, MS, PATH];

pub const TREATED: [f64; 4] = [12.1, 14.3, 11.8, 13.6];
pub const CONTROL: [f64; 4] = [15.2, 16.9, 17.4, 16.0];

/* ---- fixtures ------------------------------------------------------------ */

pub fn dataset() -> Dataset {
    let mut rows = Vec::new();
    for (i, v) in TREATED.iter().enumerate() {
        rows.push(vec![v.to_string(), LEVEL.to_string(), if i % 2 == 0 { CELL.to_string() } else { "fine".into() }, format!("{LEVEL}_solo")]);
    }
    for v in CONTROL {
        rows.push(vec![v.to_string(), "ctrl".to_string(), CELL.to_string(), format!("{LEVEL}_solo")]);
    }
    let raw = RawTable::from_rows(vec![HEADER.into(), "arm".into(), "note".into(), "site".into()], rows);
    let table = TableRef::Sheet { index: 0, name: format!("/Users/someone/{PATH}/trial.xlsx") };
    build_dataset(raw, DataFingerprint::of_bytes(PATH.as_bytes(), "in-memory", table)).expect("dataset")
}

/// The printed t, df and p a reader would give for treated vs control.
pub fn printed() -> (String, String, String) {
    let t = stats_verify::t_test_student(&TREATED, &CONTROL).unwrap();
    let p = pvalue::t_two_tailed(t.t, t.df).unwrap();
    assert!(p > 0.0005 && p < 0.05, "a moderate p keeps the fixture readable: {p}");
    (format!("{:.2}", t.t.abs()), format!("{}", t.df), format!("{:.3}", p).trim_start_matches('0').to_string())
}

pub const METHODS_PASSAGE: &str = "Groups were compared with a two-sample t-test assuming equal variances.";
pub const COPIED: &str = "Baseline readings of the cohort were stable across the whole study period";

pub fn manuscript() -> ExtractionResult {
    let (t, df, p) = printed();
    ExtractionResult {
        sections: vec![
            Section {
                kind: SectionKind::Methods,
                heading: "Methods".into(),
                paragraphs: vec![format!("Participants were assigned to two arms at random. {METHODS_PASSAGE}")],
                also_covers: vec![],
            },
            Section {
                kind: SectionKind::Results,
                heading: "Results".into(),
                paragraphs: vec![format!("{COPIED} and {MS} did not move. The treated arm scored lower than control, t({df}) = {t}, p = {p}.")],
                also_covers: vec![],
            },
        ],
        ..Default::default()
    }
}

pub struct World {
    pub ex: ExtractionResult,
    pub ctx: DocumentContext,
    pub claim: L1Claim,
    pub dataset: Dataset,
    pub reproduced: ClaimReproduction,
    pub unbound: ClaimReproduction,
}

pub fn world() -> World {
    let ex = manuscript();
    let reading = read_results(&ex, TableStructure::NotAvailable);
    let ctx = DocumentContext::read(&ex, &reading);
    let (claims, not_located) = claims_from_reading(&ex, &reading);
    assert_eq!((claims.len(), not_located), (1, 0), "the fixture holds one result");
    let claim = claims[0].clone();
    assert!(!claim.sentence.contains(MS), "the sentinel is outside the result's sentence: {}", claim.sentence);

    let mut s = L1RunState::new();
    s.load_dataset(dataset());
    s.set_claims(claims);
    s.set_context(ctx.clone());
    let draft = BindingDraft::new(L1MethodId::IndependentT, VariantChoice::not_stated(VariantKind::T))
        .role(RoleName::Outcome, &[ColumnAlias(1)])
        .role(RoleName::Group, &[ColumnAlias(2)])
        .levels(RoleName::Group, &[LEVEL, "ctrl"]);
    let id = s.add_proposal(ClaimId(0), draft, ProposalSource::Manual { edited_from: None }).expect("proposal");
    let token = s.confirm(id).expect("confirmed").1;
    let rev = s.claim(ClaimId(0)).unwrap().revision.clone();
    let out = reproduce(&s, &[(rev, token)], || Ok(dataset()));
    let reproduced = out.claims[0].clone();
    assert!(matches!(reproduced.verdict, L1Verdict::Reproduced(_)), "{:?}", reproduced.verdict);
    let unbound = reproduce(&s, &[], || Ok(dataset())).claims[0].clone();
    assert!(matches!(unbound.verdict, L1Verdict::CouldNotCheck(_)));
    World { ex, ctx, claim, dataset: dataset(), reproduced, unbound }
}

pub fn on() -> AiHelpSetting {
    AiHelpSetting { ai_help: true, web_context: true }
}

pub fn session() -> HelperSession {
    HelperSession::new(HelperRunId(7), on(), ModelId::default_model())
}

/// A Responses API body with one output_text part.
pub fn envelope(text: &str, annotations: Value) -> String {
    json!({
        "id": "resp_mock",
        "object": "response",
        "status": "completed",
        "output": [
            {"type": "web_search_call", "id": "ws_1", "status": "completed"},
            {"type": "message", "role": "assistant", "content": [
                {"type": "output_text", "text": text, "annotations": annotations}
            ]}
        ],
        "usage": {"input_tokens": 321, "output_tokens": 54}
    })
    .to_string()
}

pub fn json_reply(v: Value) -> String {
    envelope(&v.to_string(), json!([]))
}

/// Every outbound, one per task, with the given opt-ins.
pub fn outbounds(w: &World, all_opt_ins: bool, headers: bool) -> Vec<Outbound> {
    let cm = if all_opt_ins {
        OptIns::new(HelperTask::ClaimMethod, &[DataCategory::ClaimSentence, DataCategory::MethodsPassage]).unwrap()
    } else {
        OptIns::none(HelperTask::ClaimMethod)
    };
    let mut bs_ticks = Vec::new();
    if headers || all_opt_ins {
        bs_ticks.push(DataCategory::ColumnHeaders);
    }
    if all_opt_ins {
        bs_ticks.push(DataCategory::ClaimSentence);
    }
    let bs = OptIns::new(HelperTask::BindingSuggestion, &bs_ticks).unwrap();
    let web = if all_opt_ins { OptIns::new(HelperTask::WebContext, &[DataCategory::PaperDoiOrTitle]).unwrap() } else { OptIns::none(HelperTask::WebContext) };

    let template = template_query(L1MethodId::IndependentT, Variant::None, ContextTopic::Assumptions);
    let scan = QueryScan::new(&w.dataset.schema, Some(&w.ex), &[]);
    let query = check_query(&template, &template, &scan).expect("the template passes");
    vec![
        Outbound::ClaimMethod(
            build_claim_method(&w.claim, &w.ctx, cm.claim_sentence(&w.claim).unwrap(), cm.methods_passage(&w.ex, METHODS_PASSAGE).unwrap()).unwrap(),
        ),
        Outbound::BindingSuggestion(
            build_binding_suggestion(L1MethodId::IndependentT, &w.dataset.schema, bs.column_headers(&w.dataset.schema), bs.claim_sentence(&w.claim).unwrap())
                .unwrap(),
        ),
        Outbound::ResultExplanation(build_result_explanation(&w.reproduced, &w.ctx)),
        Outbound::WebContext(build_web_context(&query, web.paper("10.5555/example.2026.01").unwrap()).unwrap()),
    ]
}

/* ---- a tracing subscriber that keeps every field it is shown ------------ */

pub struct Capture {
    pub lines: Arc<Mutex<Vec<String>>>,
}

pub struct Visitor(pub String);

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

/// Run the whole flow with the given opt-ins: preview and send every task,
/// read every reply, and provoke every refusal. Returns (bodies by task, error
/// and outcome Debug strings, tracing lines).
pub fn flow(all_opt_ins: bool, headers: bool) -> (Vec<(HelperTask, String)>, Vec<String>, Vec<String>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let capture = Capture { lines: lines.clone() };
    let mut errors: Vec<String> = Vec::new();
    let mut bodies: Vec<(HelperTask, String)> = Vec::new();
    tracing::subscriber::with_default(capture, || {
        // Callsites cached as `never` before this subscriber existed are
        // asked again now that it does.
        tracing::callsite::rebuild_interest_cache();
        let w = world();
        let replies = [
            json_reply(json!({"method_id": "independent_t", "variant": "student", "reason": "A t with one df and two groups.", "relied_on": ["statistic_symbol", "df"]})),
            json_reply(json!({"roles": [{"role": "outcome", "alias": "C1", "levels_to_select": null}, {"role": "group", "alias": "C2", "levels_to_select": 2}], "reason": "C1 is numeric; C2 has two levels.", "relied_on": ["schema"]})),
            json_reply(json!({"explanation": "The data reproduced the printed result under the pooled variant."})),
            envelope("Check normality of residuals before a t-test ([bmj.com](https://www.bmj.com/a)).", json!([])),
        ];
        let mut t = MockTransport::new();
        for r in &replies {
            t = t.reply(r);
        }
        let mut s = session();
        for o in outbounds(&w, all_opt_ins, headers) {
            let p = s.preview(o).expect("preview");
            bodies.push((p.task, p.body.clone()));
            let sent = s.send_clicked(p.id, &t).expect("sent");
            let out = read_response(&sent.reply.body);
            errors.push(format!("{:?}", out.as_ref().map(|o| o.citations.len())));
            errors.push(format!("{:?}", sent.diagnostic));
        }
        for (task, body) in t.sent() {
            bodies.push((task, String::from_utf8(body).unwrap()));
        }
        // Refusals, each printed.
        let mut off = HelperSession::new(HelperRunId(8), AiHelpSetting::default(), ModelId::default_model());
        errors.push(format!("{:?}", off.preview(Outbound::ResultExplanation(build_result_explanation(&w.unbound, &w.ctx)))));
        errors.push(format!("{:?}", s.send(PreviewId(999), &t)));
        errors.push(format!("{:?}", s.send_bytes(PreviewId(1), CELL.as_bytes(), &t)));
        errors.push(format!("{:?}", OptIns::new(HelperTask::ClaimMethod, &[DataCategory::ColumnHeaders])));
        let o = OptIns::new(HelperTask::ClaimMethod, &[DataCategory::MethodsPassage]).unwrap();
        errors.push(format!("{:?}", o.methods_passage(&w.ex, &format!("{MS} {CELL}"))));
        let scan = QueryScan::new(&w.dataset.schema, Some(&w.ex), &[]);
        for edit in [format!("t-test {HEADER}"), format!("t-test {LEVEL}"), format!("t-test {LEVEL}_solo"), format!("t-test {COPIED}")] {
            errors.push(format!("{:?}", check_query("t-test", &edit, &scan)));
        }
        errors.push(format!("{:?}", MockTransport::new().fail(TransportError::Http { status: 401 })));
        errors.push(format!("{:?}", s));
        let hostile = read_response(json_reply(json!({"roles": [{"role": "group", "alias": format!("/Users/x/{PATH}.csv"), "levels_to_select": 2}], "reason": CELL, "relied_on": []})).as_bytes()).unwrap();
        if let Some(Outbound::BindingSuggestion(b)) = s.outbound(PreviewId(2)).cloned() {
            errors.push(format!("{:?}", validate_binding_suggestion(&hostile, &b)));
        }
    });
    let lines = lines.lock().unwrap().clone();
    (bodies, errors, lines)
}

pub fn assert_absent(what: &str, haystacks: &[String], needles: &[&str]) {
    for h in haystacks {
        for n in needles {
            assert!(!h.contains(n), "{n} leaked into {what}:\n{h}");
        }
    }
}
