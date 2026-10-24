//! **The project's two exports from a synthetic project: golden files, the
//! same bytes in fresh processes, the title only as the report's title, and
//! a cycle refused (Research OS, Phase 5B-2; decisions 111 and 125).**
//!
//! The project holds a succeeded summary, a succeeded t-test and a failed
//! t-test over two data files, with exclusions, limitations, notes, a
//! damaged artifact and a title full of Markdown and HTML. Its report and
//! PROV JSON are pinned byte for byte in `tests/fixtures/research_project/`.
//! Ten fresh processes of this test binary render both and must print the
//! goldens' SHA-256: a `HashMap`'s order is seeded per process, so a renderer
//! that leaned on one would differ between processes, never within one.

use std::process::Command;

use gaply_core::contracts::codes::Closed;
use gaply_core::contracts::{
    Arch, CapabilityId, ContentHash, Diagnostic, Exclusion, Os, ParamKey, ParamValue, Platform,
    Reproducibility, RunState,
};
use gaply_core::research_project::{
    render_prov_json, render_report_markdown, AgentVersion, ArtifactHash, ArtifactKind,
    ArtifactRef, ColumnRole, DataFormat, ExportCode, ExportCount, ExportManifest, ExportParameter,
    ExportRun, ProjectCode, ProjectExportInput, ProjectId, ProjectTitle, ProvEdge, StoredRun,
    StoredRunId, TestVariant, UsedRole,
};
use gaply_core::stats_desc::DescCode;
use gaply_core::stats_ttest::TTestCode;

const TITLE: &str = "Quokka <3> & *zebra* [draft](x) `#1`";

fn h(c: char) -> ArtifactHash {
    ArtifactHash::parse(&c.to_string().repeat(64)).unwrap()
}

fn ch(c: char) -> ContentHash {
    ContentHash::from_hex(&c.to_string().repeat(64)).unwrap()
}

fn art(c: char, kind: ArtifactKind, format: DataFormat, bytes: u64) -> ArtifactRef {
    ArtifactRef {
        hash: h(c),
        bytes,
        kind,
        format,
        schema_version: 1,
        damage: None,
    }
}

fn run(id: u64, cap: &str, state: RunState, t: i64) -> StoredRun {
    StoredRun {
        run_id: StoredRunId(id),
        capability_id: CapabilityId::parse(cap).unwrap(),
        capability_version: 1,
        state,
        end_code: None,
        agent: AgentVersion::parse("0.1.0").unwrap(),
        started_at: t,
        ended_at: Some(t + 1),
        saved_at: t + 60,
        rerun_of: None,
    }
}

fn manifest(input: char, params: char, output: char, read: u64, used: u64) -> ExportManifest {
    ExportManifest {
        engine_version: 1,
        inputs: vec![ch(input)],
        parameters_hash: ch(params),
        output_hash: ch(output),
        rows_read: read,
        rows_used: used,
        elapsed_ms: 3,
        platform: Platform {
            os: Os::Macos,
            arch: Arch::Aarch64,
        },
        reproducibility: Reproducibility::Bitwise,
    }
}

fn used(run: u64, c: char, role: UsedRole) -> ProvEdge {
    ProvEdge::Used {
        run: StoredRunId(run),
        artifact: h(c),
        role,
    }
}

fn generated(run: u64, c: char) -> ProvEdge {
    ProvEdge::WasGeneratedBy {
        artifact: h(c),
        run: StoredRunId(run),
    }
}

fn associated(run: u64) -> ProvEdge {
    ProvEdge::WasAssociatedWith {
        run: StoredRunId(run),
        agent: AgentVersion::parse("0.1.0").unwrap(),
    }
}

/// A synthetic project: three runs, two data files, every kind of artifact.
fn project() -> ProjectExportInput {
    let t0 = 1_792_396_351;
    let mut damaged = art('9', ArtifactKind::RunFailure, DataFormat::Json, 120);
    damaged.damage = Some(ProjectCode::ArtifactWrongHash);
    let note = Diagnostic::warning(DescCode::SuspectMissingCode)
        .with(ParamKey::Column, ParamValue::Int(2))
        .int(ParamKey::Value, 99)
        .int(ParamKey::Count, 3);
    let excluded = Exclusion {
        code: DescCode::MissingCell,
        count: 4,
        params: [(ParamKey::Column, ParamValue::Int(2))]
            .into_iter()
            .collect(),
    };
    let limitations = [
        TTestCode::NormalityNotAssessed,
        TTestCode::CohensDHasNoInterval,
    ]
    .map(|c| ExportCode::of(&Diagnostic::note(c)))
    .to_vec();
    ProjectExportInput {
        title: ProjectTitle::from_typed(TITLE).unwrap(),
        project: ProjectId::parse("0123456789abcdef0123456789abcdef").unwrap(),
        created_at: t0 - 3600,
        created_by: AgentVersion::parse("0.1.0").unwrap(),
        artifacts: vec![
            art('1', ArtifactKind::DataFile, DataFormat::Csv, 812),
            art('2', ArtifactKind::OperationSpec, DataFormat::Json, 301),
            art('3', ArtifactKind::ResultBundle, DataFormat::Json, 2048),
            art('4', ArtifactKind::RunManifest, DataFormat::Json, 640),
            art('5', ArtifactKind::OperationSpec, DataFormat::Json, 290),
            art('6', ArtifactKind::ResultBundle, DataFormat::Json, 1500),
            art('7', ArtifactKind::RunManifest, DataFormat::Json, 650),
            art('8', ArtifactKind::DataFile, DataFormat::Xlsx, 9001),
            art('a', ArtifactKind::OperationSpec, DataFormat::Json, 288),
            damaged,
        ],
        runs: vec![
            ExportRun {
                run: run(3, "stats.analyze.independent_t", RunState::Failed, t0 + 20),
                parameters: vec![
                    ExportParameter::InputShape {
                        rows: 4,
                        columns: 2,
                    },
                    ExportParameter::Column {
                        position: 1,
                        role: ColumnRole::Outcome,
                    },
                    ExportParameter::Column {
                        position: 2,
                        role: ColumnRole::Group,
                    },
                    ExportParameter::Levels(None),
                    ExportParameter::Variant(Some(TestVariant::Student)),
                    ExportParameter::MissingCodes { count: 0 },
                    ExportParameter::Seed(None),
                ],
                manifest: None,
                exclusions: vec![],
                limitations: vec![],
                notes: vec![],
                failure: Some(ExportCode::of(&Diagnostic::error(
                    TTestCode::BothGroupsConstant,
                ))),
            },
            ExportRun {
                run: run(1, "stats.describe", RunState::Succeeded, t0),
                parameters: vec![
                    ExportParameter::InputShape {
                        rows: 12,
                        columns: 3,
                    },
                    ExportParameter::Column {
                        position: 1,
                        role: ColumnRole::SummarisedNumeric,
                    },
                    ExportParameter::Column {
                        position: 2,
                        role: ColumnRole::SummarisedCategorical,
                    },
                    ExportParameter::MissingCodes { count: 1 },
                    ExportParameter::Seed(None),
                ],
                manifest: Some(manifest('1', 'b', 'c', 12, 12)),
                exclusions: vec![ExportCount::of(&excluded)],
                limitations: vec![],
                notes: vec![ExportCode::of(&note)],
                failure: None,
            },
            ExportRun {
                run: run(
                    2,
                    "stats.analyze.independent_t",
                    RunState::Succeeded,
                    t0 + 10,
                ),
                parameters: vec![
                    ExportParameter::InputShape {
                        rows: 12,
                        columns: 3,
                    },
                    ExportParameter::Column {
                        position: 1,
                        role: ColumnRole::Outcome,
                    },
                    ExportParameter::Column {
                        position: 3,
                        role: ColumnRole::Group,
                    },
                    ExportParameter::Levels(Some([0, 2])),
                    ExportParameter::Variant(Some(TestVariant::Welch)),
                    ExportParameter::MissingCodes { count: 0 },
                    ExportParameter::Seed(Some(42)),
                ],
                manifest: Some(manifest('1', 'd', 'e', 12, 10)),
                exclusions: vec![],
                limitations,
                notes: vec![],
                failure: None,
            },
        ],
        edges: vec![
            used(1, '1', UsedRole::Data),
            used(1, '2', UsedRole::Spec),
            generated(1, '3'),
            generated(1, '4'),
            associated(1),
            used(2, '1', UsedRole::Data),
            used(2, '5', UsedRole::Spec),
            generated(2, '6'),
            generated(2, '7'),
            associated(2),
            used(3, '8', UsedRole::Data),
            used(3, 'a', UsedRole::Spec),
            generated(3, '9'),
            associated(3),
        ],
    }
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/research_project")
        .join(name)
}

fn golden(name: &str, got: &[u8]) {
    let p = fixture(name);
    if std::env::var_os("GAPLY_WRITE_RENDER_GOLDEN").is_some() {
        std::fs::write(&p, got).unwrap();
    }
    let want = std::fs::read(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(
        String::from_utf8_lossy(got),
        String::from_utf8_lossy(&want),
        "{name}: the render and its golden file differ"
    );
}

fn sha(b: &[u8]) -> String {
    ContentHash::of_bytes(b).as_hex().to_string()
}

#[test]
fn the_report_and_the_prov_json_match_their_golden_files() {
    let p = project();
    golden("report.md", render_report_markdown(&p).as_bytes());
    golden("provenance.json", &render_prov_json(&p).unwrap());
}

#[test]
fn the_title_is_the_reports_first_line_escaped_and_nowhere_else() {
    let p = project();
    let report = render_report_markdown(&p);
    assert_eq!(
        report.lines().next().unwrap(),
        "# Quokka &lt;3&gt; &amp; \\*zebra\\* \\[draft\\]\\(x\\) \\`\\#1\\`"
    );
    assert_eq!(report.matches("Quokka").count(), 1);
    assert_eq!(report.matches("zebra").count(), 1);
    let prov = String::from_utf8(render_prov_json(&p).unwrap()).unwrap();
    assert!(!prov.contains("Quokka") && !prov.contains("zebra"));
}

#[test]
fn a_cycle_in_the_edges_is_refused_with_its_code() {
    let mut p = project();
    // Run 1 used what it generated.
    p.edges.push(used(1, '3', UsedRole::Data));
    assert_eq!(render_prov_json(&p), Err(ProjectCode::ProvCycle));
    let mut p = project();
    p.edges.push(ProvEdge::WasDerivedFrom {
        artifact: h('3'),
        from: h('8'),
        via: StoredRunId(1),
    });
    assert_eq!(
        render_prov_json(&p),
        Err(ProjectCode::ProvUnlinkedDerivation),
        "a stored derivation whose run did not use its source"
    );
}

#[test]
fn every_kind_and_code_in_the_project_is_a_closed_word() {
    let p = project();
    let report = render_report_markdown(&p);
    for k in ArtifactKind::ALL {
        if p.artifacts.iter().any(|a| a.kind == *k) {
            assert!(report.contains(k.wire()), "{}", k.wire());
        }
    }
    assert!(report.contains("`stats.ttest.both_groups_constant`"));
    assert!(report.contains("`stats.desc.missing_cell` (column 2): 4"));
    assert!(report.contains("damaged: project.artifact_wrong_hash"));
}

/// The child half of the fresh-process check: prints the two renders'
/// hashes when the parent asks, and does nothing otherwise.
#[test]
fn render_child_prints_hashes() {
    if std::env::var_os("GAPLY_RENDER_CHILD").is_none() {
        return;
    }
    let p = project();
    println!(
        "RENDER_HASHES report={} prov={}",
        sha(render_report_markdown(&p).as_bytes()),
        sha(&render_prov_json(&p).unwrap())
    );
}

#[test]
fn ten_fresh_processes_render_the_same_bytes_as_the_golden_files() {
    let want = format!(
        "RENDER_HASHES report={} prov={}",
        sha(&std::fs::read(fixture("report.md")).unwrap()),
        sha(&std::fs::read(fixture("provenance.json")).unwrap())
    );
    let exe = std::env::current_exe().unwrap();
    let mut seen = Vec::new();
    for _ in 0..10 {
        let out = Command::new(&exe)
            .args([
                "render_child_prints_hashes",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("GAPLY_RENDER_CHILD", "1")
            .output()
            .expect("the test binary runs again");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        // The harness prints the test's name first, on the same line.
        let line = text
            .lines()
            .find_map(|l| {
                l.find("RENDER_HASHES ")
                    .map(|i| l[i..].trim_end().to_string())
            })
            .unwrap_or_else(|| panic!("the child printed no hashes:\n{text}"));
        seen.push(line);
    }
    assert_eq!(seen.len(), 10);
    for line in &seen {
        assert_eq!(line, &want, "a fresh process rendered other bytes");
    }
}
