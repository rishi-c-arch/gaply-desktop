//! **The project's two exports, rendered from one input: the project report
//! (Markdown) and the PROV JSON (Research OS, Phase 5B-2; decisions 111 and
//! 125; ADR-009 §7).**
//!
//! Pure, like the rest of `research_project`: no file, clock, log or random
//! source. Both renderers take one [`ProjectExportInput`], whose only free
//! string is the project's title. The report prints it once, escaped, as its
//! title; the PROV JSON does not print it. Every other field is an id, a
//! hash, a closed word, an enum or a number: a run's settings as
//! [`ExportParameter`]s, which name a column by its position and its role and
//! never by its header, and its codes as [`ClosedWord`]s, which can only be
//! made from a member of a closed set. No row, cell, level label, header,
//! path or file name can be put into the input, because no field of it holds
//! free text.
//!
//! Both are deterministic: the same input gives the same bytes. Runs are
//! printed by id, artifacts and edges in sorted order, and the PROV JSON is
//! built from ordered maps. `wasDerivedFrom` is derived from the stored
//! `used` and `wasGeneratedBy` edges ([`ProvGraph::derivations`]). The PROV
//! JSON is refused with `project.prov_cycle` when the graph is not acyclic.
//!
//! The file names are fixed ([`REPORT_FILE_NAME`], [`PROV_FILE_NAME`]): the
//! save dialog suggests them, never the project's name (decision 125).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{
    AgentVersion, ArtifactHash, ArtifactRef, ClosedWord, ProjectCode, ProjectId, ProjectTitle,
    ProvEdge, ProvGraph, StoredRun, StoredRunId, UsedRole,
};
use crate::contracts::codes::{closed, Closed};
use crate::contracts::{
    Arch, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform, Reproducibility,
};

/// The report's file name in the save dialog (decision 125).
pub const REPORT_FILE_NAME: &str = "gaply_project_report.md";

/// The PROV JSON's file name in the save dialog (decision 125).
pub const PROV_FILE_NAME: &str = "gaply_project_provenance.json";

/// The PROV JSON's one namespace. A URN, so no reader resolves it over the
/// network.
pub const PROV_NAMESPACE: &str = "urn:gaply:prov:";

closed! {
    /// What a column was to a run. A column is named by its position and this
    /// role, never by its header.
    pub enum ColumnRole in "" {
        /// Summarised, read as numbers.
        SummarisedNumeric => "summarised_numeric",
        /// Summarised, read as categories.
        SummarisedCategorical => "summarised_categorical",
        /// The t-test's outcome.
        Outcome => "outcome",
        /// The t-test's group.
        Group => "group",
    }
}

closed! {
    /// Which t-test the user chose.
    pub enum TestVariant in "" {
        Student => "student",
        Welch => "welch",
    }
}

/// One setting of a run, as the report shows it: positions, indexes, closed
/// words and counts only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportParameter {
    /// The data file's shape as the run read it.
    InputShape { rows: u64, columns: u64 },
    /// A column the run used, by its 1-based position, and its role.
    Column { position: u32, role: ColumnRole },
    /// The two levels compared, by their index in code-point order, group 1
    /// first. `None`: the group's only two levels, in that order.
    Levels(Option<[u32; 2]>),
    /// The t-test's variant. `None` is never saved: a run needs it chosen.
    Variant(Option<TestVariant>),
    /// How many missing-value codes the user ticked. Their values are in the
    /// settings' fingerprint, not here.
    MissingCodes { count: u64 },
    /// The random seed, if the run used one.
    Seed(Option<u64>),
}

/// A closed code with its integer parameters. A closed-word parameter is
/// left out: the code names what it is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCode {
    pub code: ClosedWord,
    pub params: Vec<(ParamKey, i64)>,
}

impl ExportCode {
    pub fn of<C: Closed>(d: &Diagnostic<C>) -> ExportCode {
        ExportCode {
            code: ClosedWord::of(d.code),
            params: ints(&d.params),
        }
    }
}

/// Rows or cells a run left out, by reason, with a count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCount {
    pub code: ClosedWord,
    pub count: u64,
    pub params: Vec<(ParamKey, i64)>,
}

impl ExportCount {
    pub fn of<C: Closed>(e: &Exclusion<C>) -> ExportCount {
        ExportCount {
            code: ClosedWord::of(e.code),
            count: e.count,
            params: ints(&e.params),
        }
    }
}

fn ints<C>(params: &BTreeMap<ParamKey, ParamValue<C>>) -> Vec<(ParamKey, i64)> {
    params
        .iter()
        .filter_map(|(k, v)| match v {
            ParamValue::Int(n) => Some((*k, *n)),
            _ => None,
        })
        .collect()
}

/// A run's manifest, as the report shows it: versions, hashes, counts, the
/// platform and the reproducibility claim.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportManifest {
    pub engine_version: u32,
    pub inputs: Vec<ContentHash>,
    pub parameters_hash: ContentHash,
    pub output_hash: ContentHash,
    pub rows_read: u64,
    pub rows_used: u64,
    pub elapsed_ms: u64,
    pub platform: Platform,
    pub reproducibility: Reproducibility,
}

/// One saved run, as both exports show it.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportRun {
    pub run: StoredRun,
    pub parameters: Vec<ExportParameter>,
    /// A succeeded run's manifest; `None` for a failed one.
    pub manifest: Option<ExportManifest>,
    pub exclusions: Vec<ExportCount>,
    pub limitations: Vec<ExportCode>,
    /// The validation's warnings that travelled with the result.
    pub notes: Vec<ExportCode>,
    /// A failed run's code.
    pub failure: Option<ExportCode>,
}

/// Everything both exports are rendered from. The title is the one free
/// string (decision 125: the project's name appears only as the report's
/// title).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectExportInput {
    pub title: ProjectTitle,
    pub project: ProjectId,
    pub created_at: i64,
    pub created_by: AgentVersion,
    pub artifacts: Vec<ArtifactRef>,
    pub runs: Vec<ExportRun>,
    /// The stored edges: `used`, `wasGeneratedBy`, `wasAssociatedWith`, and
    /// any stored `wasDerivedFrom`.
    pub edges: Vec<ProvEdge>,
}

/* ------------------------------------------------------------------------ *
 *  Words
 * ------------------------------------------------------------------------ */

fn param_word(k: ParamKey) -> &'static str {
    match k {
        ParamKey::Column => "column",
        ParamKey::Count => "count",
        ParamKey::Index => "index",
        ParamKey::Value => "value",
        ParamKey::Limit => "limit",
        ParamKey::Expected => "expected",
        ParamKey::Found => "found",
        ParamKey::From => "from",
        ParamKey::Event => "event",
        ParamKey::Workspace => "workspace",
        ParamKey::Run => "run",
        ParamKey::Lifecycle => "lifecycle",
    }
}

fn os_word(os: Os) -> &'static str {
    match os {
        Os::Macos => "macos",
        Os::Windows => "windows",
        Os::Linux => "linux",
        Os::Other => "other",
    }
}

fn arch_word(arch: Arch) -> &'static str {
    match arch {
        Arch::Aarch64 => "aarch64",
        Arch::X86_64 => "x86_64",
        Arch::Other => "other",
    }
}

fn reproducibility_words(r: Reproducibility) -> String {
    match r {
        Reproducibility::Bitwise => {
            "bitwise: the same inputs, engine version and platform give the same bytes".into()
        }
        Reproducibility::WithinTolerance { relative } => {
            format!("within a relative tolerance of {relative:e}")
        }
        Reproducibility::NotReproducible => "not reproducible".into(),
    }
}

fn params_words(params: &[(ParamKey, i64)]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let inner: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{} {v}", param_word(*k)))
        .collect();
    format!(" ({})", inner.join(", "))
}

/// The title, safe in Markdown: `&`, `<`, `>` and `"` as HTML entities, so
/// no tag or entity of the user's survives, and every other ASCII
/// punctuation mark backslash-escaped, which CommonMark reads as the literal
/// mark (spec §2.4). A title holds no control character (its parser refuses
/// them), so it stays on its one line.
pub fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_ascii_punctuation() => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

/// Days since 1970-01-01 to a civil date (H. Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Seconds since the epoch as `YYYY-MM-DDTHH:MM:SSZ`, UTC.
pub fn iso_utc(secs: i64) -> String {
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3_600,
        (s % 3_600) / 60,
        s % 60
    )
}

/* ------------------------------------------------------------------------ *
 *  The project report
 * ------------------------------------------------------------------------ */

/// What the report says about itself, under its title (decision 125).
pub const REPORT_PREAMBLE: &str = "A Gaply project report. It holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. A column is named by its position and its role, never by its header.";

fn artifact_of<'a>(input: &'a ProjectExportInput, h: &ArtifactHash) -> Option<&'a ArtifactRef> {
    input.artifacts.iter().find(|a| &a.hash == h)
}

fn used_of(input: &ProjectExportInput, run: StoredRunId) -> Vec<(UsedRole, ArtifactHash)> {
    let mut v: Vec<(UsedRole, ArtifactHash)> = input
        .edges
        .iter()
        .filter_map(|e| match e {
            ProvEdge::Used {
                run: r,
                artifact,
                role,
            } if *r == run => Some((*role, artifact.clone())),
            _ => None,
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn generated_of(input: &ProjectExportInput, run: StoredRunId) -> Vec<ArtifactHash> {
    let mut v: Vec<ArtifactHash> = input
        .edges
        .iter()
        .filter_map(|e| match e {
            ProvEdge::WasGeneratedBy { artifact, run: r } if *r == run => Some(artifact.clone()),
            _ => None,
        })
        .collect();
    v.sort_by_key(|h| {
        (
            artifact_of(input, h).map(|a| a.kind.wire()).unwrap_or(""),
            h.clone(),
        )
    });
    v.dedup();
    v
}

fn artifact_row(out: &mut String, label: &str, input: &ProjectExportInput, h: &ArtifactHash) {
    match artifact_of(input, h) {
        Some(a) => {
            let damage = a
                .damage
                .map(|c| format!(" damaged: {}", c.wire()))
                .unwrap_or_default();
            out.push_str(&format!(
                "| {label} | {} | {} | {} | `{}`{damage} |\n",
                a.kind.wire(),
                a.format.wire(),
                a.bytes,
                h.as_hex()
            ));
        }
        None => out.push_str(&format!(
            "| {label} | not recorded | not recorded | not recorded | `{}` |\n",
            h.as_hex()
        )),
    }
}

fn parameter_line(p: &ExportParameter) -> String {
    match p {
        ExportParameter::InputShape { rows, columns } => {
            format!("Data file as read: {rows} rows, {columns} columns")
        }
        ExportParameter::Column { position, role } => {
            format!("Column {position}: {}", role.wire())
        }
        ExportParameter::Levels(Some([a, b])) => {
            format!(
                "Levels compared: index {a} (group 1) and index {b} (group 2), in code-point order"
            )
        }
        ExportParameter::Levels(None) => {
            "Levels compared: the group's two levels, in code-point order".into()
        }
        ExportParameter::Variant(Some(v)) => format!("Variant: {}", v.wire()),
        ExportParameter::Variant(None) => "Variant: not chosen".into(),
        ExportParameter::MissingCodes { count } => {
            format!("Missing-value codes: {count} (their values are in the settings fingerprint)")
        }
        ExportParameter::Seed(Some(s)) => format!("Random seed: {s}"),
        ExportParameter::Seed(None) => "Random seed: none".into(),
    }
}

fn run_section(out: &mut String, input: &ProjectExportInput, r: &ExportRun) {
    let s = &r.run;
    out.push_str(&format!(
        "\n## Run {}: `{}`, version {}\n\n",
        s.run_id.0,
        s.capability_id.as_str(),
        s.capability_version
    ));
    out.push_str(&format!("- Status: {}\n", s.state.wire()));
    if let Some(f) = &r.failure {
        out.push_str(&format!(
            "- Failure code: `{}`{}\n",
            f.code.as_str(),
            params_words(&f.params)
        ));
    }
    if let Some(c) = s.end_code {
        out.push_str(&format!("- End code: `{}`\n", c.wire()));
    }
    out.push_str(&format!("- Started: {}\n", iso_utc(s.started_at)));
    match s.ended_at {
        Some(t) => out.push_str(&format!("- Ended: {}\n", iso_utc(t))),
        None => out.push_str("- Ended: not recorded\n"),
    }
    out.push_str(&format!("- Saved: {}\n", iso_utc(s.saved_at)));
    out.push_str(&format!("- Carried out by: Gaply {}\n", s.agent.as_str()));
    if let Some(m) = &r.manifest {
        out.push_str(&format!("- Engine version: {}\n", m.engine_version));
        out.push_str(&format!(
            "- Platform: {} {}\n",
            os_word(m.platform.os),
            arch_word(m.platform.arch)
        ));
        out.push_str(&format!(
            "- Reproducibility: {}\n",
            reproducibility_words(m.reproducibility)
        ));
    }

    out.push_str("\n### Settings\n\n");
    if r.parameters.is_empty() {
        out.push_str("- None recorded\n");
    }
    for p in &r.parameters {
        out.push_str(&format!("- {}\n", parameter_line(p)));
    }

    out.push_str("\n### Inputs and outputs\n\n");
    out.push_str("| | kind | format | bytes | SHA-256 |\n|---|---|---|---|---|\n");
    for (role, h) in used_of(input, s.run_id) {
        artifact_row(out, &format!("used, {}", role.wire()), input, &h);
    }
    for h in generated_of(input, s.run_id) {
        artifact_row(out, "generated", input, &h);
    }

    if let Some(m) = &r.manifest {
        out.push_str("\n### Fingerprints and counts\n\n");
        for (i, h) in m.inputs.iter().enumerate() {
            out.push_str(&format!("- Input {} SHA-256: `{}`\n", i + 1, h.as_hex()));
        }
        out.push_str(&format!(
            "- Analysis settings SHA-256: `{}`\n",
            m.parameters_hash.as_hex()
        ));
        out.push_str(&format!("- Result SHA-256: `{}`\n", m.output_hash.as_hex()));
        out.push_str(&format!(
            "- Rows read: {}; rows used: {}\n",
            m.rows_read, m.rows_used
        ));
        out.push_str(&format!("- Time taken: {} ms\n", m.elapsed_ms));
    }

    let lists: [(&str, Vec<String>); 3] = [
        (
            "Left out",
            r.exclusions
                .iter()
                .map(|e| {
                    format!(
                        "`{}`{}: {}",
                        e.code.as_str(),
                        params_words(&e.params),
                        e.count
                    )
                })
                .collect(),
        ),
        (
            "Limitations",
            r.limitations
                .iter()
                .map(|c| format!("`{}`{}", c.code.as_str(), params_words(&c.params)))
                .collect(),
        ),
        (
            "Notes",
            r.notes
                .iter()
                .map(|c| format!("`{}`{}", c.code.as_str(), params_words(&c.params)))
                .collect(),
        ),
    ];
    for (heading, lines) in lists {
        out.push_str(&format!("\n### {heading}\n\n"));
        if lines.is_empty() {
            out.push_str("- None\n");
        }
        for l in lines {
            out.push_str(&format!("- {l}\n"));
        }
    }
}

/// The project report, as Markdown. The title, escaped, is the first line
/// and the only place the project's name appears.
pub fn render_report_markdown(input: &ProjectExportInput) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", escape_markdown(input.title.as_str())));
    out.push_str(REPORT_PREAMBLE);
    out.push_str("\n\n");
    out.push_str(&format!("- Project: `{}`\n", input.project.as_str()));
    out.push_str(&format!("- Created: {}\n", iso_utc(input.created_at)));
    out.push_str(&format!(
        "- Created by: Gaply {}\n",
        input.created_by.as_str()
    ));
    out.push_str(&format!("- Saved runs: {}\n", input.runs.len()));
    let mut runs: Vec<&ExportRun> = input.runs.iter().collect();
    runs.sort_by_key(|r| r.run.run_id);
    for r in runs {
        run_section(&mut out, input, r);
    }
    out
}

/* ------------------------------------------------------------------------ *
 *  The PROV JSON
 * ------------------------------------------------------------------------ */

fn entity_id(h: &ArtifactHash) -> String {
    format!("gaply:artifact-{}", h.as_hex())
}

fn activity_id(run: StoredRunId) -> String {
    format!("gaply:run-{}", run.0)
}

fn agent_id(a: &AgentVersion) -> String {
    format!("gaply:app-{}", a.as_str())
}

/// The provenance graph as PROV-JSON (the W3C member submission's layout:
/// `prefix`, `entity`, `activity`, `agent`, and one object per relation).
/// Deterministic; refused with `project.prov_cycle` when the edges do not
/// form an acyclic graph, or with `project.prov_unlinked_derivation` when a
/// stored derivation's run did not link both its ends.
pub fn render_prov_json(input: &ProjectExportInput) -> Result<Vec<u8>, ProjectCode> {
    // `used` and `wasGeneratedBy` first, so a stored derivation finds its
    // links, as the store loads them.
    let mut ordered: Vec<ProvEdge> = input.edges.clone();
    ordered.sort_by_key(|e| match e {
        ProvEdge::Used { .. } | ProvEdge::WasGeneratedBy { .. } => 0,
        ProvEdge::WasAssociatedWith { .. } => 1,
        ProvEdge::WasDerivedFrom { .. } => 2,
    });
    let mut graph = ProvGraph::from_edges(ordered)?;
    for e in graph.derivations() {
        graph.add(e)?;
    }
    if !graph.is_acyclic() {
        return Err(ProjectCode::ProvCycle);
    }

    let mut entity: BTreeMap<String, Value> = BTreeMap::new();
    let mut activity: BTreeMap<String, Value> = BTreeMap::new();
    let mut agent: BTreeMap<String, Value> = BTreeMap::new();
    let mut used: BTreeMap<String, Value> = BTreeMap::new();
    let mut generated: BTreeMap<String, Value> = BTreeMap::new();
    let mut derived: BTreeMap<String, Value> = BTreeMap::new();
    let mut associated: BTreeMap<String, Value> = BTreeMap::new();

    for a in &input.artifacts {
        let mut attrs: BTreeMap<&str, Value> = BTreeMap::new();
        attrs.insert("prov:type", json!(format!("gaply:{}", a.kind.wire())));
        attrs.insert("gaply:sha256", json!(a.hash.as_hex()));
        attrs.insert("gaply:bytes", json!(a.bytes));
        attrs.insert("gaply:format", json!(a.format.wire()));
        attrs.insert("gaply:schemaVersion", json!(a.schema_version));
        if let Some(d) = a.damage {
            attrs.insert("gaply:damage", json!(d.wire()));
        }
        entity.insert(entity_id(&a.hash), json!(attrs));
    }
    for r in &input.runs {
        let s = &r.run;
        let mut attrs: BTreeMap<&str, Value> = BTreeMap::new();
        attrs.insert("prov:startTime", json!(iso_utc(s.started_at)));
        if let Some(t) = s.ended_at {
            attrs.insert("prov:endTime", json!(iso_utc(t)));
        }
        attrs.insert("gaply:capability", json!(s.capability_id.as_str()));
        attrs.insert("gaply:capabilityVersion", json!(s.capability_version));
        attrs.insert("gaply:state", json!(s.state.wire()));
        attrs.insert("gaply:savedAt", json!(iso_utc(s.saved_at)));
        if let Some(c) = s.end_code {
            attrs.insert("gaply:endCode", json!(c.wire()));
        }
        if let Some(f) = &r.failure {
            attrs.insert("gaply:failureCode", json!(f.code.as_str()));
        }
        if let Some(m) = &r.manifest {
            attrs.insert("gaply:engineVersion", json!(m.engine_version));
            attrs.insert("gaply:parametersSha256", json!(m.parameters_hash.as_hex()));
            attrs.insert("gaply:outputSha256", json!(m.output_hash.as_hex()));
        }
        activity.insert(activity_id(s.run_id), json!(attrs));
    }

    for e in graph.edges() {
        match e {
            ProvEdge::Used {
                run,
                artifact,
                role,
            } => {
                used.insert(
                    format!("_:used-{}-{}-{}", run.0, artifact.as_hex(), role.wire()),
                    json!({
                        "prov:activity": activity_id(*run),
                        "prov:entity": entity_id(artifact),
                        "prov:role": format!("gaply:{}", role.wire()),
                    }),
                );
            }
            ProvEdge::WasGeneratedBy { artifact, run } => {
                generated.insert(
                    format!("_:generated-{}-{}", run.0, artifact.as_hex()),
                    json!({
                        "prov:entity": entity_id(artifact),
                        "prov:activity": activity_id(*run),
                    }),
                );
            }
            ProvEdge::WasDerivedFrom {
                artifact,
                from,
                via,
            } => {
                derived.insert(
                    format!(
                        "_:derived-{}-{}-{}",
                        via.0,
                        artifact.as_hex(),
                        from.as_hex()
                    ),
                    json!({
                        "prov:generatedEntity": entity_id(artifact),
                        "prov:usedEntity": entity_id(from),
                        "prov:activity": activity_id(*via),
                    }),
                );
            }
            ProvEdge::WasAssociatedWith { run, agent: a } => {
                agent.insert(
                    agent_id(a),
                    json!({
                        "prov:type": "prov:SoftwareAgent",
                        "gaply:application": "Gaply",
                        "gaply:version": a.as_str(),
                    }),
                );
                associated.insert(
                    format!("_:associated-{}-{}", run.0, a.as_str()),
                    json!({
                        "prov:activity": activity_id(*run),
                        "prov:agent": agent_id(a),
                    }),
                );
            }
        }
    }

    let mut doc: BTreeMap<&str, Value> = BTreeMap::new();
    doc.insert("prefix", json!({ "gaply": PROV_NAMESPACE }));
    doc.insert("entity", json!(entity));
    doc.insert("activity", json!(activity));
    doc.insert("agent", json!(agent));
    doc.insert("used", json!(used));
    doc.insert("wasGeneratedBy", json!(generated));
    doc.insert("wasDerivedFrom", json!(derived));
    doc.insert("wasAssociatedWith", json!(associated));
    let mut bytes =
        serde_json::to_vec_pretty(&doc).expect("a document of words, hashes and numbers");
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_leaves_no_tag_entity_or_markdown_mark_live() {
        assert_eq!(
            escape_markdown("<b>x</b> & *y* [z](u) # `c` \"q\""),
            "&lt;b&gt;x&lt;\\/b&gt; &amp; \\*y\\* \\[z\\]\\(u\\) \\# \\`c\\` &quot;q&quot;"
        );
        assert_eq!(escape_markdown("Thesis chapter 3"), "Thesis chapter 3");
        assert_eq!(escape_markdown("é ü 中文"), "é ü 中文");
    }

    #[test]
    fn dates_are_utc_and_known_values_match() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(1_792_396_351), "2026-10-19T07:52:31Z");
        assert_eq!(iso_utc(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn every_param_key_has_a_word() {
        use std::collections::BTreeSet;
        let keys = [
            ParamKey::Column,
            ParamKey::Count,
            ParamKey::Index,
            ParamKey::Value,
            ParamKey::Limit,
            ParamKey::Expected,
            ParamKey::Found,
            ParamKey::From,
            ParamKey::Event,
            ParamKey::Workspace,
            ParamKey::Run,
            ParamKey::Lifecycle,
        ];
        let words: BTreeSet<&str> = keys.iter().map(|k| param_word(*k)).collect();
        assert_eq!(words.len(), keys.len());
        for k in keys {
            let wire = serde_json::to_value(k).expect("a key serialises");
            assert_eq!(wire, json!(param_word(k)), "{k:?}");
        }
    }

    #[test]
    fn the_word_lists_are_pinned() {
        let roles: Vec<&str> = ColumnRole::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(
            roles,
            [
                "summarised_numeric",
                "summarised_categorical",
                "outcome",
                "group"
            ]
        );
        let variants: Vec<&str> = TestVariant::ALL.iter().map(|c| c.wire()).collect();
        assert_eq!(variants, ["student", "welch"]);
        assert_eq!(REPORT_FILE_NAME, "gaply_project_report.md");
        assert_eq!(PROV_FILE_NAME, "gaply_project_provenance.json");
    }

    #[test]
    fn a_kind_word_is_what_the_entity_type_carries() {
        use super::super::ArtifactKind;
        // Each artifact kind the store writes has a PROV type.
        for k in ArtifactKind::ALL {
            assert!(!k.wire().is_empty());
        }
    }
}
