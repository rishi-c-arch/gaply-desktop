//! **What differs per method at the seam, pinned before it (Research OS,
//! Phase 8C).** Each test here pins one difference of the step's list that
//! no earlier test pinned, through the commands' plain bodies, as the Tauri
//! wrappers call them.
//!
//! * Difference 4, the registry's refusal: the t-test's commands ask the
//!   registry before they read the slot, and refuse with its `capability.*`
//!   code (decision 74). The summary's validate reads the slot first, and a
//!   registry refusal is a validation report, `Unsupported`, in its own code;
//!   its run asks no registry first, so the refusal is a refused record.
//! * Difference 12, the export's order: `export_input` reads each run's spec,
//!   checking its hash, before it looks the run's capability up. So a run whose
//!   capability no row names gives `project.db_unreadable`, and the same run
//!   with its spec damaged gives the damage's own code.
//!
//! Every file is synthetic and in a folder the test creates and removes. No
//! capability id but registered ones is written: the registry refusal is a
//! version this build does not have.

mod project_support;

use gaply_core::contracts::{CapabilityId, ContentHash, OperationId, RunId, RunState};
use gaply_core::research_project::{
    ArtifactHash, ArtifactKind, DataFormat, ProjectTitle, UsedRole,
};
use gaply_core::stats_desc::{self, Alias, DescribeParams};
use gaply_core::stats_l1::dataset::ColumnAlias;
use gaply_core::stats_ttest::{self, TTestParams, Variant};

use app_lib::project_commands as pc;
use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use app_lib::stats_desc_commands as desc;
use app_lib::stats_ttest_commands as ttest;

use project_support::{code, refusal, World, CSV, SMALL};

/// A slot number no open dataset has.
const NO_SLOT: u64 = 999;

/// The refusal of a command whose views are never `Debug`.
fn refused<T>(r: Result<T, gaply_core::GaplyError>) -> gaply_core::GaplyError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("the command was expected to refuse"),
    }
}

#[test]
fn the_ttest_asks_the_registry_before_the_slot_and_the_summary_after() {
    let w = World::new("seam-refusal");
    let slot = w.open("a.csv", CSV);
    let e = w.engine();
    let held = e.datasets.get(slot).expect("the slot");
    let profile = stats_desc::profile(held.dataset());
    let mut summary = stats_desc::spec(
        OperationId(1),
        held.dataset(),
        DescribeParams::defaults_for(&profile),
    );
    summary.capability_version = 2;
    let mut t = stats_ttest::spec(
        OperationId(2),
        held.dataset(),
        TTestParams {
            outcome: Alias(ColumnAlias(1)),
            group: Alias(ColumnAlias(2)),
            levels: None,
            variant: Some(Variant::Welch),
            missing_codes: Vec::new(),
        },
    );
    t.capability_version = 2;

    // The t-test: the registry first, whatever the slot.
    for s in [slot, NO_SLOT] {
        let r = refusal(&refused(ttest::validate(&e.datasets, s, &t)));
        assert_eq!(r["code"], "capability.version_mismatch", "slot {s}: {r}");
        assert_eq!(
            r["params"],
            serde_json::json!({ "expected": 1, "found": 2 })
        );
    }
    let held_before = e.runs.held();
    let r = refusal(&refused(ttest::run(&e.runs, &e.datasets, slot, t.clone())));
    assert_eq!(r["code"], "capability.version_mismatch", "{r}");
    assert_eq!(
        e.runs.held(),
        held_before,
        "a refused t-test makes no record"
    );

    // The summary: the slot first, then the registry inside the validation.
    let r = refusal(&refused(desc::validate(&e.datasets, NO_SLOT, &summary)));
    assert_eq!(r["code"], "run.unknown_input", "{r}");
    let v = desc::validate(&e.datasets, slot, &summary).expect("a report, not an error");
    let v = serde_json::to_value(&v).unwrap();
    assert_eq!(v["status"], "unsupported", "{v}");
    assert_eq!(
        v["errors"][0]["code"], "stats.desc.capability_version_mismatch",
        "{v}"
    );
    // Its run makes a record, refused with the same code.
    let run = desc::run(&e.runs, &e.datasets, slot, summary)
        .expect("the run is accepted")
        .run_id;
    assert_eq!(w.wait(run), RunState::Refused);
    let s = e.runs.snapshot(RunId(run)).unwrap();
    let codes: Vec<String> = s
        .diagnostics
        .iter()
        .map(|d| {
            serde_json::to_value(d).unwrap()["code"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(codes, ["stats.desc.capability_version_mismatch"]);
}

fn hash(bytes: &[u8]) -> ArtifactHash {
    ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap()
}

#[test]
fn the_export_reads_a_runs_spec_before_it_looks_its_capability_up() {
    let w = World::new("seam-order");
    // A run of `stats.check`: registered, but no method a project runs again
    // or exports, so the project table has no row for it.
    let spec: &[u8] = br#"{"schemaVersion":1,"seam":"8C"}"#;
    let bundle: &[u8] = br#"{"schemaVersion":1,"bundle":1}"#;
    let manifest: &[u8] = br#"{"schemaVersion":1,"manifest":1}"#;
    let (dh, sh, bh, mh) = (hash(CSV), hash(spec), hash(bundle), hash(manifest));
    let artifact = |kind, format, expected: &ArtifactHash, bytes: &[u8]| ArtifactInput {
        kind,
        format,
        schema_version: 1,
        expected: expected.clone(),
        bytes: bytes.to_vec(),
    };
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse("stats.check").unwrap(),
            capability_version: 1,
            state: RunState::Succeeded,
            started_at: 1_700_000_000,
            ended_at: 1_700_000_001,
            used: vec![(dh.clone(), UsedRole::Data), (sh.clone(), UsedRole::Spec)],
            generated: vec![bh.clone(), mh.clone()],
            derived: vec![],
        },
        artifacts: vec![
            artifact(ArtifactKind::DataFile, DataFormat::Csv, &dh, CSV),
            artifact(ArtifactKind::OperationSpec, DataFormat::Json, &sh, spec),
            artifact(ArtifactKind::ResultBundle, DataFormat::Json, &bh, bundle),
            artifact(ArtifactKind::RunManifest, DataFormat::Json, &mh, manifest),
        ],
    };
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).expect("the store opens");
    // By its path, not `store.create_with_save(`, as the store's own tests
    // call it, so the lock-codes guard's count of callers is unchanged.
    let (handle, _) =
        ProjectRoot::create_with_save(&store, &ProjectTitle::parse("seam").unwrap(), req)
            .expect("the project is made");
    let id = handle.id().as_str().to_string();
    handle.close();
    pc::open(&w.root(), &w.ws, &id).expect("the project opens");

    // Intact: the spec reads back, then no row names the capability.
    let e = pc::report(&w.root(), &w.ws, &id).unwrap_err();
    assert_eq!(code(&e), "project.db_unreadable");

    // Damaged: the read of the spec is what refuses.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = w.root().join("projects").join(&id).join("artifacts");
        let file = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| std::fs::read(p).unwrap() == spec)
            .expect("the spec's file");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut bytes = std::fs::read(&file).unwrap();
        bytes[3] ^= 0x01;
        std::fs::write(&file, &bytes).unwrap();
        let e = pc::report(&w.root(), &w.ws, &id).unwrap_err();
        assert_eq!(code(&e), "project.artifact_wrong_hash");
    }
}
