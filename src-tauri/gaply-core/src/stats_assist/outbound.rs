//! **The four outbound structs, their builders, and the request body.**
//!
//! Each task has one allow-listed outbound struct, made by one builder from
//! typed inputs. These are the only types in `stats_assist` that serialise
//! into a request, and a request body serialises only them (as the `input`
//! string) beside the spec's fixed fields. Opt-in data enters only through an
//! [`Allowed`] argument.
//!
//! What each carries is exactly what its consent table row in
//! [`super::consent`] says. In particular: no row, cell, level code or label,
//! no header unless opted in, no manuscript text beyond the claim's printed
//! numbers unless opted in, no path, file name or fingerprint, and no
//! statistic computed from the data except `result_explanation`'s enumerated
//! fields.

use serde::Serialize;
use serde_json::Value;

use super::codes::{self, WireCode};
use super::consent::{base_category, Allowed, CategorySet, ClaimSentence, ColumnHeaders, DataCategory, MethodsPassage, PaperRef};
use super::query::CheckedQuery;
use super::spec::{allowed_web_domains, method_description, variant_names, HelperTask, ModelId, OutputFormat};
use crate::stats_l0::input::DocumentContext;
use crate::stats_l0::result::{Family, PValue};
use crate::stats_l1::catalogue::{
    contains_word, ContinuityVariant, L1MethodId, LevelRule, MethodCatalogue, RoleName, SlotKind, SpearmanPVariant, TVariant,
    Variant, METHOD_WORDS,
};
use crate::stats_l1::claim::L1Claim;
use crate::stats_l1::dataset::{AliasedSchema, SchemaSummary};
use crate::equation::rational::Rational;
use crate::stats_l0::number::{Relation, ReportedNumber};
use crate::stats_l1::reproduce::{ClaimReproduction, L1Verdict, NumberKind, Printed};

/// Why a builder refused. Closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRefusal {
    /// An opt-in ticked for another task.
    OptInForAnotherTask { ticked_for: HelperTask, used_for: HelperTask },
}

fn check_task<T>(a: &Option<Allowed<T>>, task: HelperTask) -> Result<(), BuildRefusal> {
    match a {
        Some(a) if a.task() != task => Err(BuildRefusal::OptInForAnotherTask { ticked_for: a.task(), used_for: task }),
        _ => Ok(()),
    }
}

/* ------------------------------------------------------------------------ *
 *  Shared wire words
 * ------------------------------------------------------------------------ */

pub fn role_name(r: RoleName) -> &'static str {
    match r {
        RoleName::Outcome => "outcome",
        RoleName::Group => "group",
        RoleName::First => "first",
        RoleName::Second => "second",
        RoleName::X => "x",
        RoleName::Y => "y",
        RoleName::RowVariable => "row_variable",
        RoleName::ColumnVariable => "column_variable",
        RoleName::Predictors => "predictors",
    }
}

pub fn role_from_name(s: &str) -> Option<RoleName> {
    [
        RoleName::Outcome,
        RoleName::Group,
        RoleName::First,
        RoleName::Second,
        RoleName::X,
        RoleName::Y,
        RoleName::RowVariable,
        RoleName::ColumnVariable,
        RoleName::Predictors,
    ]
    .into_iter()
    .find(|r| role_name(*r) == s)
}

fn family_symbol(f: Family) -> &'static str {
    match f {
        Family::T => "t",
        Family::F => "F",
        Family::ChiSquare => "chi_square",
        Family::R => "r",
        Family::Z => "z",
    }
}

fn tails(ctx: &DocumentContext) -> &'static str {
    if ctx.one_tailed_at.is_some() {
        "one_tailed_wording_in_paper"
    } else {
        "not_stated"
    }
}

fn variant_wire(v: Variant) -> Option<&'static str> {
    match v {
        Variant::T(TVariant::Student) => Some("student"),
        Variant::T(TVariant::Welch) => Some("welch"),
        Variant::Continuity(ContinuityVariant::Yes) => Some("continuity_correction"),
        Variant::Continuity(ContinuityVariant::No) => Some("no_continuity_correction"),
        Variant::SpearmanP(SpearmanPVariant::TApproximation) => Some("t_approximation"),
        Variant::SpearmanP(SpearmanPVariant::Exact) => Some("exact"),
        Variant::None => None,
        Variant::T(TVariant::NotStated)
        | Variant::Continuity(ContinuityVariant::NotStated)
        | Variant::SpearmanP(SpearmanPVariant::NotStated) => Some("not_stated"),
    }
}

/// A computed value at 6 significant figures: enough to explain any printed
/// number, and no more of the data's arithmetic than that.
pub fn six_significant(v: f64) -> f64 {
    if !v.is_finite() || v == 0.0 {
        return v;
    }
    format!("{v:.5e}").parse().unwrap_or(v)
}

/// A bound written exactly from the printed digits: at the printed number's
/// places, or one more when the bound needs it (4.44 -> 4.435 and 4.445).
fn exact_bound(r: Rational, decimals: u32) -> String {
    for places in decimals..=decimals + 2 {
        if r.round_to(places) == Some(r) {
            return r.to_decimal_string(places);
        }
    }
    r.to_decimal_string(decimals + 2)
}

/// The printed number's own rounding bounds (a count or a printed threshold
/// is a point), never the gate-widened acceptance range.
fn printed_bounds(n: &ReportedNumber) -> [String; 2] {
    let i = n.interval();
    [exact_bound(i.lo, n.decimals()), exact_bound(i.hi, n.decimals())]
}

fn relation_word(r: Relation) -> &'static str {
    match r {
        Relation::Eq => "=",
        Relation::Lt => "<",
        Relation::Le => "<=",
        Relation::Gt => ">",
        Relation::Ge => ">=",
    }
}

/* ------------------------------------------------------------------------ *
 *  claim_method
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultFieldsOut {
    pub statistic_symbol: &'static str,
    pub statistic: String,
    pub df: Vec<String>,
    pub p: String,
    pub tails: &'static str,
    pub stated_alpha: Vec<String>,
    pub alpha_defaulted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CatalogueEntryOut {
    pub method_id: &'static str,
    pub description: &'static str,
    pub variants: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClaimMethodOutbound {
    pub task: &'static str,
    pub result: ResultFieldsOut,
    /// Words from the closed `METHOD_WORDS` list found in the claim's
    /// sentence: the list's own strings, never the sentence's.
    pub method_words: Vec<&'static str>,
    pub catalogue: Vec<CatalogueEntryOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub methods_passage: Option<String>,
}

fn printed_p(p: &PValue) -> String {
    match p {
        PValue::Reported(n) => format!("{} {}", relation_word(n.relation()), n.raw()),
        PValue::NotSignificant => "ns".to_string(),
    }
}

pub fn build_claim_method(
    claim: &L1Claim,
    ctx: &DocumentContext,
    sentence: Option<Allowed<ClaimSentence>>,
    passage: Option<Allowed<MethodsPassage>>,
) -> Result<ClaimMethodOutbound, BuildRefusal> {
    check_task(&sentence, HelperTask::ClaimMethod)?;
    check_task(&passage, HelperTask::ClaimMethod)?;
    let r = &claim.result;
    let (_, defaulted) = ctx.alpha();
    let mut method_words: Vec<&'static str> =
        METHOD_WORDS.iter().filter(|(w, _)| contains_word(&claim.sentence, w)).map(|(w, _)| *w).collect();
    method_words.dedup();
    Ok(ClaimMethodOutbound {
        task: HelperTask::ClaimMethod.id(),
        result: ResultFieldsOut {
            statistic_symbol: family_symbol(r.family),
            statistic: r.statistic.raw().to_string(),
            df: r.df.iter().map(|d| d.raw().to_string()).collect(),
            p: printed_p(&r.p),
            tails: tails(ctx),
            stated_alpha: ctx.alpha_printed(),
            alpha_defaulted: defaulted,
        },
        method_words,
        catalogue: MethodCatalogue::ENTRIES
            .iter()
            .map(|e| CatalogueEntryOut {
                method_id: e.id.id(),
                description: method_description(e.id),
                variants: variant_names(e.variant_kind).to_vec(),
            })
            .collect(),
        sentence: sentence.map(|s| s.value().0.clone()),
        methods_passage: passage.map(|p| p.value().0.clone()),
    })
}

/* ------------------------------------------------------------------------ *
 *  binding_suggestion
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RoleSlotOut {
    pub role: &'static str,
    pub kind: &'static str,
    /// `exactly_2`, `at_least_2`, `at_least_1_column`, or null.
    pub levels: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HeaderOut {
    pub alias: String,
    pub header: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BindingSuggestionOutbound {
    pub task: &'static str,
    pub method_id: &'static str,
    pub role_slots: Vec<RoleSlotOut>,
    /// The one outbound form of the schema, reused from `stats_l1`.
    pub schema: AliasedSchema,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<HeaderOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
}

pub fn build_binding_suggestion(
    method: L1MethodId,
    schema: &SchemaSummary,
    headers: Option<Allowed<ColumnHeaders>>,
    sentence: Option<Allowed<ClaimSentence>>,
) -> Result<BindingSuggestionOutbound, BuildRefusal> {
    check_task(&headers, HelperTask::BindingSuggestion)?;
    check_task(&sentence, HelperTask::BindingSuggestion)?;
    let entry = MethodCatalogue::entry(method);
    Ok(BindingSuggestionOutbound {
        task: HelperTask::BindingSuggestion.id(),
        method_id: method.id(),
        role_slots: entry
            .slots
            .iter()
            .map(|s| {
                let (kind, levels) = match s.kind {
                    SlotKind::Numeric => ("numeric", None),
                    SlotKind::Group { levels: LevelRule::Exactly(k) } => ("group", Some(format!("exactly_{k}"))),
                    SlotKind::Group { levels: LevelRule::AtLeast(k) } => ("group", Some(format!("at_least_{k}"))),
                    SlotKind::NumericList { min } => ("numeric_list", Some(format!("at_least_{min}_column"))),
                };
                RoleSlotOut { role: role_name(s.role), kind, levels }
            })
            .collect(),
        schema: schema.aliased(),
        headers: headers.map(|h| h.value().0.iter().map(|(a, h)| HeaderOut { alias: a.clone(), header: h.clone() }).collect()),
        sentence: sentence.map(|s| s.value().0.clone()),
    })
}

/* ------------------------------------------------------------------------ *
 *  result_explanation
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NumberOut {
    pub kind: WireCode,
    /// As printed; a p carries its relation (`< .001`).
    pub printed: String,
    /// The printed number's own rounding bounds, written exactly; `None` for
    /// a p printed only as `ns`.
    pub interval: Option<[String; 2]>,
    /// At 6 significant figures.
    pub computed: f64,
    pub status: WireCode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExclusionOut {
    pub reason: WireCode,
    pub rows: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultExplanationOutbound {
    pub task: &'static str,
    pub method_id: Option<&'static str>,
    pub variant: Option<&'static str>,
    pub variant_stated: bool,
    pub computed_variant: Option<WireCode>,
    pub verdict: &'static str,
    pub conclusion_changes: Option<bool>,
    pub could_not_check_reason: Option<WireCode>,
    pub stated_alpha: Vec<String>,
    pub tails: &'static str,
    pub numbers: Vec<NumberOut>,
    pub n_used: Option<usize>,
    pub exclusions: Vec<ExclusionOut>,
    pub notes: Vec<WireCode>,
}

/// The engine's verdict, as the explanation must state it.
pub fn verdict_wire(v: &L1Verdict) -> &'static str {
    match v {
        L1Verdict::Reproduced(_) => "reproduced",
        L1Verdict::NotReproduced { .. } => "not_reproduced",
        L1Verdict::CouldNotCheck(_) => "could_not_check",
    }
}

/// Everything a finished result may send: the shown candidate's numbers, and
/// no group mean or other statistic.
pub fn build_result_explanation(rep: &ClaimReproduction, ctx: &DocumentContext) -> ResultExplanationOutbound {
    let shown = rep.shown();
    let (conclusion_changes, could_not_check_reason, verdict_notes) = match &rep.verdict {
        L1Verdict::Reproduced(notes) => (None, None, notes.clone()),
        L1Verdict::NotReproduced { conclusion_changes } => (*conclusion_changes, None, Vec::new()),
        L1Verdict::CouldNotCheck(why) => (None, Some(codes::l1_could_not_check(*why)), Vec::new()),
    };
    let numbers = shown
        .map(|c| {
            c.numbers
                .iter()
                .map(|n| {
                    let (printed, interval) = match &n.printed {
                        Printed::Number(r) if n.kind == NumberKind::P => {
                            (format!("{} {}", relation_word(r.relation()), r.raw()), Some(printed_bounds(r)))
                        }
                        Printed::Number(r) => (r.raw().to_string(), Some(printed_bounds(r))),
                        Printed::NotSignificant => ("ns".to_string(), None),
                    };
                    NumberOut {
                        kind: codes::number_kind(n.kind),
                        printed,
                        interval,
                        computed: six_significant(n.computed),
                        status: codes::number_status(n.status),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    ResultExplanationOutbound {
        task: HelperTask::ResultExplanation.id(),
        method_id: rep.method.map(L1MethodId::id),
        variant: rep.variant.and_then(|v| variant_wire(v.variant())),
        variant_stated: rep.variant.is_some_and(|v| v.variant().is_stated()),
        computed_variant: shown.map(|c| codes::computed_variant(c.variant)),
        verdict: verdict_wire(&rep.verdict),
        conclusion_changes,
        could_not_check_reason,
        stated_alpha: ctx.alpha_printed(),
        tails: tails(ctx),
        numbers,
        n_used: rep.counts.as_ref().map(|c| c.n_used),
        exclusions: rep
            .counts
            .as_ref()
            .map(|c| c.excluded.iter().map(|e| ExclusionOut { reason: codes::exclusion_reason(e.reason), rows: e.rows }).collect())
            .unwrap_or_default(),
        notes: verdict_notes.iter().chain(rep.notes.iter()).map(|n| codes::l1_note(*n)).collect(),
    }
}

/* ------------------------------------------------------------------------ *
 *  web_context
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WebContextOutbound {
    pub task: &'static str,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper: Option<String>,
}

/// The query must have passed the local scan; the DOI or title must have been
/// ticked. Nothing else enters: never the schema, headers, rows, results or
/// manuscript text.
pub fn build_web_context(query: &CheckedQuery, paper: Option<Allowed<PaperRef>>) -> Result<WebContextOutbound, BuildRefusal> {
    check_task(&paper, HelperTask::WebContext)?;
    Ok(WebContextOutbound {
        task: HelperTask::WebContext.id(),
        query: query.as_str().to_string(),
        paper: paper.map(|p| p.value().0.clone()),
    })
}

/* ------------------------------------------------------------------------ *
 *  Any outbound, and the categories it carries
 * ------------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq)]
pub enum Outbound {
    ClaimMethod(ClaimMethodOutbound),
    BindingSuggestion(BindingSuggestionOutbound),
    ResultExplanation(ResultExplanationOutbound),
    WebContext(WebContextOutbound),
}

impl Outbound {
    pub fn task(&self) -> HelperTask {
        match self {
            Outbound::ClaimMethod(_) => HelperTask::ClaimMethod,
            Outbound::BindingSuggestion(_) => HelperTask::BindingSuggestion,
            Outbound::ResultExplanation(_) => HelperTask::ResultExplanation,
            Outbound::WebContext(_) => HelperTask::WebContext,
        }
    }

    /// The categories this request carries, read from its content, not from
    /// the ticks: an opt-in field that is present is a category sent.
    pub fn categories(&self) -> CategorySet {
        let mut cats = vec![base_category(self.task())];
        match self {
            Outbound::ClaimMethod(o) => {
                if o.sentence.is_some() {
                    cats.push(DataCategory::ClaimSentence);
                }
                if o.methods_passage.is_some() {
                    cats.push(DataCategory::MethodsPassage);
                }
            }
            Outbound::BindingSuggestion(o) => {
                if o.headers.is_some() {
                    cats.push(DataCategory::ColumnHeaders);
                }
                if o.sentence.is_some() {
                    cats.push(DataCategory::ClaimSentence);
                }
            }
            Outbound::ResultExplanation(_) => {}
            Outbound::WebContext(o) => {
                if o.paper.is_some() {
                    cats.push(DataCategory::PaperDoiOrTitle);
                }
            }
        }
        CategorySet::of(cats)
    }

    fn input_json(&self) -> String {
        let r = match self {
            Outbound::ClaimMethod(o) => serde_json::to_string(o),
            Outbound::BindingSuggestion(o) => serde_json::to_string(o),
            Outbound::ResultExplanation(o) => serde_json::to_string(o),
            Outbound::WebContext(o) => serde_json::to_string(o),
        };
        r.expect("an outbound struct always serialises")
    }

    /// The input fields the model may say it relied on: the names of the
    /// fields actually sent.
    pub fn sent_fields(&self) -> Vec<&'static str> {
        match self {
            Outbound::ClaimMethod(o) => {
                let mut f = vec!["statistic_symbol", "statistic", "df", "p", "tails", "stated_alpha", "method_words", "catalogue"];
                if o.sentence.is_some() {
                    f.push("sentence");
                }
                if o.methods_passage.is_some() {
                    f.push("methods_passage");
                }
                f
            }
            Outbound::BindingSuggestion(o) => {
                let mut f = vec!["method_id", "role_slots", "schema"];
                if o.headers.is_some() {
                    f.push("headers");
                }
                if o.sentence.is_some() {
                    f.push("sentence");
                }
                f
            }
            Outbound::ResultExplanation(_) | Outbound::WebContext(_) => Vec::new(),
        }
    }
}

/* ------------------------------------------------------------------------ *
 *  The Responses API body
 * ------------------------------------------------------------------------ */

#[derive(Serialize)]
struct JsonSchemaFormat<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'static str,
    strict: bool,
    schema: &'a Value,
}

#[derive(Serialize)]
struct TextParam<'a> {
    format: JsonSchemaFormat<'a>,
}

#[derive(Serialize)]
struct WebSearchFilters<'a> {
    allowed_domains: &'a [String],
}

#[derive(Serialize)]
struct WebSearchTool<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    filters: WebSearchFilters<'a>,
}

#[derive(Serialize)]
struct ReasoningParam<'a> {
    effort: &'a str,
}

/// The request body. Its fields are the spec's and the outbound struct's,
/// nothing else: no background mode, no files, no previous_response_id, no
/// function tools, and never `user_location`.
#[derive(Serialize)]
struct ResponsesBody<'a> {
    model: &'a str,
    instructions: &'a str,
    /// The outbound struct, as JSON.
    input: String,
    max_output_tokens: u32,
    store: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningParam<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<TextParam<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<WebSearchTool<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<Vec<&'static str>>,
}

/// The exact bytes a request would send: pretty JSON, so the preview a user
/// reads is the request itself and not a rendering of it.
pub fn request_body(outbound: &Outbound, model: &ModelId) -> String {
    let task = outbound.task();
    let spec = task.spec();
    let (text, tools, tool_choice, include) = match spec.output_format {
        OutputFormat::JsonSchema => {
            let schema = spec.output_schema.as_ref().expect("a json_schema task has an output schema (pinned)");
            (Some(TextParam { format: JsonSchemaFormat { kind: "json_schema", name: task.id(), strict: true, schema } }), None, None, None)
        }
        OutputFormat::Text => (
            None,
            Some(vec![WebSearchTool { kind: "web_search", filters: WebSearchFilters { allowed_domains: allowed_web_domains() } }]),
            // The documented way to make the search run (the guide's
            // "required"); with one tool it can only be web_search.
            Some("required"),
            Some(vec!["web_search_call.action.sources"]),
        ),
    };
    let body = ResponsesBody {
        model: model.as_str(),
        instructions: &spec.instructions,
        input: outbound.input_json(),
        max_output_tokens: spec.max_output_tokens,
        store: false,
        reasoning: spec.reasoning_effort.as_deref().map(|effort| ReasoningParam { effort }),
        text,
        tools,
        tool_choice,
        include,
    };
    serde_json::to_string_pretty(&body).expect("a request body always serialises")
}
