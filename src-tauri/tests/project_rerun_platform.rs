//! **A rerun on another kind of computer (Research OS, Phase 8I, decision
//! 366).** A saved result rerun on another platform can differ in its last
//! digits, so its output hash differs and decision 189 says "not identical".
//! The runs view now says which platform each run's manifest names, whether
//! they differ, and, for a different result on a different platform,
//! whether every number agrees to within 4 units in the last place: facts,
//! never a verdict.
//!
//! The rows are built through the store as `project_rerun.rs` builds them:
//! a saved summary of this Mac's, its manifest copied with Linux x86_64 as
//! its platform for a "saved elsewhere" run, then reruns of it whose
//! manifests name this Mac with another output hash, and whose bundles move
//! one number by 1 or 5 units in the last place.

mod project_support;

use serde_json::{json, Value};

use app_lib::project_commands::{self as pc, SaveTarget, StoredRunView};
use app_lib::project_store::{ArtifactInput, ProjectRoot, RunToSave, SaveRequest};
use gaply_core::contracts::{Arch, CapabilityId, ContentHash, Os, Platform, RunState};
use gaply_core::research_project::{
    ArtifactHash, ArtifactKind, DataFormat, RerunComparison, StoredRunId, UsedRole,
};

use project_support::*;

fn hash(bytes: &[u8]) -> ArtifactHash {
    ArtifactHash::parse(ContentHash::of_bytes(bytes).as_hex()).unwrap()
}

fn json_art(kind: ArtifactKind, bytes: &[u8]) -> ArtifactInput {
    ArtifactInput {
        kind,
        format: DataFormat::Json,
        schema_version: 1,
        expected: hash(bytes),
        bytes: bytes.to_vec(),
    }
}

/// A world whose open project holds one saved summary of `CSV`, run on this
/// computer: the project's id.
fn with_a_summary(w: &World) -> String {
    let slot = w.open("a.csv", CSV);
    let run = w.describe(slot);
    pc::save_run(
        &w.root(),
        &w.ws,
        run,
        SaveTarget::NewProject {
            name: "Platforms".into(),
        },
    )
    .expect("saved")
    .project_id
}

/// Saves a row on stored run 1's data file and spec, with `bundle` and
/// `manifest`, as a rerun of `of` when given one. Its stored id.
fn store(w: &World, id: &str, of: Option<u64>, bundle: &[u8], manifest: &[u8]) -> u64 {
    let first = pc::run_result(&w.root(), &w.ws, id, 1).unwrap();
    let spec = first.spec_json.into_bytes();
    let data = ArtifactHash::parse(&first.data_file.sha256).unwrap();
    let store = ProjectRoot::open_root_with(&w.root(), SMALL).unwrap();
    let mut h = store.open(id).unwrap();
    let req = SaveRequest {
        run: RunToSave {
            capability_id: CapabilityId::parse(&first.capability_id).unwrap(),
            capability_version: 1,
            state: RunState::Succeeded,
            started_at: 1_792_396_340,
            ended_at: 1_792_396_341,
            used: vec![(data, UsedRole::Data), (hash(&spec), UsedRole::Spec)],
            generated: vec![hash(bundle), hash(manifest)],
            derived: Vec::new(),
        },
        artifacts: vec![
            json_art(ArtifactKind::OperationSpec, &spec),
            json_art(ArtifactKind::ResultBundle, bundle),
            json_art(ArtifactKind::RunManifest, manifest),
        ],
    };
    match of {
        Some(of) => h.save_rerun(req, StoredRunId(of)).expect("a rerun row").0,
        None => h.save(req).expect("a row").0,
    }
}

/// The first number of `v` that is not a whole number, moved by `ulps`.
fn move_first_fraction(v: &mut Value, ulps: i64) -> bool {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(x) if x.fract() != 0.0 => {
                *v = json!(f64::from_bits((x.to_bits() as i64 + ulps) as u64));
                true
            }
            _ => false,
        },
        Value::Array(a) => a.iter_mut().any(|x| move_first_fraction(x, ulps)),
        Value::Object(o) => o.values_mut().any(|x| move_first_fraction(x, ulps)),
        _ => false,
    }
}

fn view(w: &World, id: &str, n: u64) -> StoredRunView {
    pc::runs(&w.root(), &w.ws, id)
        .unwrap()
        .into_iter()
        .find(|r| r.stored_run_id == n)
        .unwrap()
}

const MAC: Platform = Platform {
    os: Os::Macos,
    arch: Arch::Aarch64,
};
const LINUX: Platform = Platform {
    os: Os::Linux,
    arch: Arch::X86_64,
};

#[test]
fn a_rerun_on_another_kind_of_computer_says_so_and_whether_its_numbers_agree() {
    let w = World::new("rr-platform");
    let id = with_a_summary(&w);
    let first = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap();
    let bundle: Value = serde_json::from_str(first.bundle_json.as_deref().unwrap()).unwrap();
    let manifest: Value = serde_json::from_str(first.manifest_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        manifest["platform"],
        serde_json::to_value(Platform::current()).unwrap()
    );
    // The run saved elsewhere: this one's result, its manifest naming Linux.
    let mut elsewhere = manifest.clone();
    elsewhere["platform"] = json!({"os": "linux", "arch": "x86_64"});
    let saved = store(
        &w,
        &id,
        None,
        first.bundle_json.as_deref().unwrap().as_bytes(),
        &serde_json::to_vec(&elsewhere).unwrap(),
    );
    // Its reruns here: another output hash, one number moved.
    let mut here = manifest.clone();
    here["platform"] = json!({"os": "macos", "arch": "aarch64"});
    here["outputHash"] = json!("1".repeat(64));
    let moved = |ulps: i64| {
        let mut b = bundle.clone();
        assert!(
            move_first_fraction(&mut b["primaryOutputs"], ulps),
            "a fraction to move"
        );
        serde_json::to_vec(&b).unwrap()
    };
    let here_bytes = serde_json::to_vec(&here).unwrap();
    let one = store(&w, &id, Some(saved), &moved(1), &here_bytes);
    let five = store(&w, &id, Some(saved), &moved(5), &here_bytes);

    let v = view(&w, &id, one);
    assert_eq!(v.comparison, Some(RerunComparison::Different));
    assert_eq!((v.platform, v.repeated_platform), (Some(MAC), Some(LINUX)));
    assert_eq!(v.platform_differs, Some(true));
    assert_eq!(v.numbers_agree_to_last_digits, Some(true), "one ulp apart");
    let wire = serde_json::to_value(&v).unwrap();
    assert_eq!(wire["platform"], json!({"os": "macos", "arch": "aarch64"}));
    assert_eq!(
        wire["repeatedPlatform"],
        json!({"os": "linux", "arch": "x86_64"})
    );
    assert_eq!(wire["platformDiffers"], json!(true));
    assert_eq!(wire["numbersAgreeToLastDigits"], json!(true));

    let v = view(&w, &id, five);
    assert_eq!(v.platform_differs, Some(true));
    assert_eq!(
        v.numbers_agree_to_last_digits,
        Some(false),
        "five ulps apart"
    );
}

#[test]
fn the_same_platform_says_nothing_new_and_an_older_manifest_still_loads() {
    let w = World::new("rr-platform-same");
    let id = with_a_summary(&w);
    let first = pc::run_result(&w.root(), &w.ws, &id, 1).unwrap();
    let bundle: Value = serde_json::from_str(first.bundle_json.as_deref().unwrap()).unwrap();
    let mut manifest: Value =
        serde_json::from_str(first.manifest_json.as_deref().unwrap()).unwrap();
    manifest["outputHash"] = json!("1".repeat(64));
    let mut b = bundle.clone();
    assert!(move_first_fraction(&mut b["primaryOutputs"], 1));
    // The same platform, a different hash: no platform line, no comparison.
    let same = store(
        &w,
        &id,
        Some(1),
        &serde_json::to_vec(&b).unwrap(),
        &serde_json::to_vec(&manifest).unwrap(),
    );
    let v = view(&w, &id, same);
    assert_eq!(v.comparison, Some(RerunComparison::Different));
    assert_eq!(v.platform_differs, Some(false));
    assert_eq!(v.numbers_agree_to_last_digits, None);
    // A manifest and a bundle from before these fields: the run loads, its
    // platform unknown, nothing compared.
    let older = store(
        &w,
        &id,
        Some(1),
        b"{\"test\":\"bundle\"}",
        format!(
            "{{\"outputHash\":\"{}\",\"engineVersion\":1}}",
            "2".repeat(64)
        )
        .as_bytes(),
    );
    let v = view(&w, &id, older);
    assert_eq!(v.comparison, Some(RerunComparison::Different));
    assert_eq!(v.platform, None);
    assert_eq!(v.repeated_platform, Some(Platform::current()));
    assert_eq!(
        (v.platform_differs, v.numbers_agree_to_last_digits),
        (None, None)
    );
    let wire = serde_json::to_value(&v).unwrap();
    for key in ["platform", "platformDiffers", "numbersAgreeToLastDigits"] {
        assert!(wire.get(key).is_none(), "{key}: {wire}");
    }
    // The run saved first, not a rerun, has none of the four.
    let wire = serde_json::to_value(view(&w, &id, 1)).unwrap();
    for key in [
        "platform",
        "repeatedPlatform",
        "platformDiffers",
        "numbersAgreeToLastDigits",
    ] {
        assert!(wire.get(key).is_none(), "{key}: {wire}");
    }
}
