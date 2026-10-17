//! **Phase 4A's new surfaces carry no user data (Research OS).** A sentinel
//! sweep: a manuscript with a sentinel sentence, a data file with a sentinel
//! header, level label and text cell, each in a file and folder with sentinel
//! names. Then every new surface is collected:
//!
//! * the `workspace.inactive` refusal of all twelve verifier operation
//!   commands while no workspace is active, given the sentinel paths;
//! * every capability refusal the façades give for a forged spec;
//! * the `workspace_list` view, with the registry's capability ids;
//! * every tracing span and event of the process, from every thread, while
//!   those refusals were given AND while Statistics was active and Level 0,
//!   the legacy lane and Level 1's open read the sentinel files.
//!
//! No sentinel may appear in any of them. Each surface must have been
//! collected (counted), and the data must really have been read (the header
//! sentinel is in Level 1's open view), so the sweep is not vacuous. Alone
//! in its binary on purpose: its subscriber is the process's global default.

mod desc_support;
mod l1_support;
mod verifier_support;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use app_lib::workspace as ws;
use desc_support::call;
use gaply_core::contracts::capability::{Build, ConfirmationId};
use gaply_core::contracts::CapabilityId;
use gaply_core::extract::extract_from_text;
use gaply_core::stats_adapters;
use gaply_core::stats_l0::result::TableStructure;
use verifier_support::*;

const HDR: &str = "HDR_SENTINEL_v41";
const LVL: &str = "LVL_SENTINEL_v52";
const TXT: &str = "TXT_SENTINEL_v63";
const MS: &str = "MS_SENTINEL_v74";
const FILE: &str = "FILE_SENTINEL_v85";
const PATH: &str = "PATH_SENTINEL_v96";
const SENTINELS: [&str; 6] = [HDR, LVL, TXT, MS, FILE, PATH];

fn clean(surface: &str, text: &str) {
    for s in SENTINELS {
        assert!(!text.contains(s), "{surface} holds {s}: {text}");
    }
}

#[test]
fn no_header_label_cell_manuscript_text_file_name_or_path_reaches_a_new_surface() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    tracing::subscriber::set_global_default(l1_support::Capture {
        lines: lines.clone(),
    })
    .expect("the only subscriber");
    tracing::callsite::rebuild_interest_cache();

    let w = world(Arc::new(ws::readiness));
    let folder = w.dir.0.join(PATH);
    std::fs::create_dir_all(&folder).unwrap();
    let ms = folder.join(format!("{FILE}_ms.txt"));
    std::fs::write(
        &ms,
        format!("A Title\n\nResults\nScores rose in the {MS} group, t(4) = 1.20, p = .29.\n"),
    )
    .unwrap();
    let data = folder.join(format!("{FILE}_data.csv"));
    std::fs::write(
        &data,
        format!("score,{HDR},note\n1.5,a,t\n2.5,{LVL},t\n3.5,a,{TXT}\n4.5,{LVL},t\n5.5,a,t\n6.5,{LVL},t\n"),
    )
    .unwrap();
    let (ms, data) = (
        ms.to_string_lossy().into_owned(),
        data.to_string_lossy().into_owned(),
    );

    // 1. Twelve refusals while no workspace is active, given the sentinel paths.
    let mut refusals = 0;
    for (name, r) in operation_calls_on(&w, &ms, &data) {
        let e = r.expect_err(name);
        assert!(is_workspace_refusal(&e), "{name}: {e}");
        clean(name, &e.to_string());
        refusals += 1;
    }
    assert_eq!(refusals, 12);

    // 2. The façades' capability refusals for forged specs.
    let ex = extract_from_text(&format!(
        "A Title\n\nResults\nScores rose in the {MS} group, t(4) = 1.20, p = .29.\n"
    ));
    let mut forged = Vec::new();
    for (id, version, refs) in [
        ("stats.verify.kruskal", 1, vec![]),
        ("stats.check", 2, vec![]),
        (
            "stats.check",
            1,
            vec![ConfirmationId::parse(&"d".repeat(64)).unwrap()],
        ),
        ("stats.describe", 1, vec![]),
    ] {
        let mut s = stats_adapters::check_spec();
        s.capability_id = CapabilityId::parse(id).unwrap();
        s.capability_version = version;
        s.confirmation_refs = refs;
        let d = stats_adapters::check(&s, Build::Standard, &ex, TableStructure::NotAvailable)
            .expect_err(id);
        let text = ws::refuse_capability(d).to_string();
        clean(id, &text);
        assert!(
            !text.contains(id) || id == "stats.check",
            "a refusal does not echo the id it was given: {text}"
        );
        forged.push(text);
    }
    assert_eq!(forged.len(), 4);

    // 3. The list view.
    let list = call(&w.w, "workspace_list", json!({})).unwrap();
    clean("workspace_list", &list.to_string());
    assert_eq!(list[0]["capabilityIds"].as_array().unwrap().len(), 9);

    // 4. Active: the operation commands read the sentinel files.
    call(&w.w, "workspace_activate", json!({ "id": "statistics" })).expect("Statistics activates");
    let mut read = 0;
    for (name, r) in operation_calls_on(&w, &ms, &data) {
        if let Err(e) = &r {
            assert!(!is_workspace_refusal(e), "{name}: {e}");
        }
        if name == "stats_l1_open" {
            let v: Value = r.clone().expect("Level 1 opens the sentinel files");
            assert!(
                v.to_string().contains(HDR),
                "positive control: the header was read"
            );
            read += 1;
        }
        if name == "run_stats_consistency" || name == "run_stats_preview" {
            assert!(r.is_ok(), "{name}: {r:?}");
            read += 1;
        }
    }
    assert_eq!(read, 3);

    let lines = lines.lock().unwrap().clone();
    assert!(
        lines.len() >= 30,
        "only {} tracing lines captured",
        lines.len()
    );
    assert!(
        lines.iter().any(|l| l.contains("workspace: refused")),
        "the refusals were traced"
    );
    assert!(
        lines.iter().any(|l| l.contains("capability: refused")),
        "the capability refusals were traced"
    );
    for l in &lines {
        clean("tracing", l);
    }
    println!(
        "swept {refusals} refusals, {} capability refusals, 1 list view, {} tracing lines",
        forged.len(),
        lines.len()
    );
}
