//! **Step 5's sentinels, the paths and the key appear in no request body, no
//! error, no view and no trace, through the Level 1 commands** (Phase 2,
//! Step 6). The command layer is where a path first exists.
//!
//! Alone in its binary on purpose: `tracing` caches each callsite's interest
//! process-wide, so a callsite first hit on another test's thread (with no
//! subscriber) can be skipped by this test's capture. Measured: in the shared
//! binary the capture saw between 6 and 22 lines across eight runs of the same
//! flow. Alone, nothing else can hit a callsite first.

mod l1_support;

use std::sync::{Arc, Mutex};

use serde_json::json;

use app_lib::stats_l1_commands::{self as l1, HelperTaskInput};
use gaply_core::stats_assist::transport::MockTransport;
use l1_support::*;

#[test]
fn step_5s_sentinels_and_the_paths_appear_in_no_body_error_view_or_trace() {
    let replies = [
        json_reply(json!({"method_id": "independent_t", "variant": "student", "reason": "A t with two groups.", "relied_on": ["df"]})),
        binding_reply(),
        json_reply(json!({"explanation": "The data reproduced the printed result."})),
        envelope("Check normality first ([bmj.com](https://www.bmj.com/a)).", json!([{"type": "url_citation", "url": "https://www.bmj.com/a", "title": "BMJ", "start_index": 22, "end_index": 57}])),
    ];
    let mut mock = MockTransport::new();
    for r in &replies {
        mock = mock.reply(r);
    }
    let w = world_with(mock);
    let lines = Arc::new(Mutex::new(Vec::new()));
    let mut errors: Vec<String> = Vec::new();
    let mut views: Vec<String> = Vec::new();
    tracing::subscriber::with_default(Capture { lines: lines.clone() }, || {
        // Callsites cached as `never` before this subscriber existed are
        // asked again now that it does.
        tracing::callsite::rebuild_interest_cache();
        // With AI off first: refusals.
        let v = open(&w);
        views.push(serde_json::to_string(&v).unwrap());
        let cm = HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: Some("Groups were compared with a two-sample t-test assuming equal variances.".into()) };
        errors.push(format!("{:?}", l1::helper_preview(&w.runs, &w.ai, v.run, &cm, &["methods_passage".into(), "claim_sentence".into()]).unwrap_err()));
        ai_on(&w, true);
        // Every opt-in on, every task.
        let p1 = l1::helper_preview(&w.runs, &w.ai, v.run, &cm, &["methods_passage".into(), "claim_sentence".into()]).unwrap();
        let bs = HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() };
        let p2 = l1::helper_preview(&w.runs, &w.ai, v.run, &bs, &["column_headers".into(), "claim_sentence".into()]).unwrap();
        for p in [&p1, &p2] {
            views.push(serde_json::to_string(&l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 1).unwrap()).unwrap());
        }
        let token = confirm_by_hand(&w, v.run);
        views.push(serde_json::to_string(&l1::compute(&w.runs, v.run, &[token]).unwrap()).unwrap());
        let p3 = l1::helper_preview(&w.runs, &w.ai, v.run, &HelperTaskInput::ResultExplanation { claim_id: 0 }, &[]).unwrap();
        views.push(serde_json::to_string(&l1::helper_send(&w.runs, &w.ai, v.run, p3.preview, 1).unwrap()).unwrap());
        let web = HelperTaskInput::WebContext {
            claim_id: 0,
            method_id: "independent_t".into(),
            topic: "assumptions".into(),
            edited_query: None,
            paper: Some("10.5555/example.2026.01".into()),
        };
        let p4 = l1::helper_preview(&w.runs, &w.ai, v.run, &web, &["paper_doi_or_title".into()]).unwrap();
        views.push(serde_json::to_string(&l1::helper_send(&w.runs, &w.ai, v.run, p4.preview, 1).unwrap()).unwrap());
        for p in [&p1, &p2, &p3, &p4] {
            views.push(serde_json::to_string(p).unwrap());
        }
        views.push(serde_json::to_string(&l1::usage(&w.runs, v.run).unwrap()).unwrap());
        // Refusals that could carry a path, a header, a level or a cell.
        let missing = w.files.manuscript.with_file_name(format!("{FILE}_absent.txt"));
        errors.push(format!("{:?}", l1::open(&w.db, &w.runs, &w.ai, &missing, &w.files.data).err()));
        // Files that exist and do not parse: they reach the preflight's and
        // the extractor's own error paths, which a missing file does not.
        for ext in ["pdf", "docx"] {
            let junk = w.files.manuscript.with_file_name(format!("{FILE}_junk.{ext}"));
            std::fs::write(&junk, b"not a document at all").unwrap();
            let e = l1::open(&w.db, &w.runs, &w.ai, &junk, &w.files.data).err();
            assert!(e.is_some(), "a junk .{ext} is refused");
            errors.push(format!("{e:?}"));
        }
        let missing_data = w.files.data.with_file_name(format!("{FILE}_absent.csv"));
        errors.push(format!("{:?}", l1::open(&w.db, &w.runs, &w.ai, &w.files.manuscript, &missing_data).err()));
        let bad_ext = w.files.data.with_extension("exe");
        std::fs::write(&bad_ext, b"x").unwrap();
        errors.push(format!("{:?}", l1::open(&w.db, &w.runs, &w.ai, &w.files.manuscript, &bad_ext).err()));
        for edit in [format!("t-test {HEADER}"), format!("t-test {LEVEL}"), format!("t-test {LEVEL}_solo")] {
            let q = HelperTaskInput::WebContext { claim_id: 0, method_id: "independent_t".into(), topic: "assumptions".into(), edited_query: Some(edit), paper: None };
            errors.push(format!("{:?}", l1::helper_preview(&w.runs, &w.ai, v.run, &q, &[]).unwrap_err()));
        }
        let mut bad = manual_edits();
        bad.levels[0].levels = vec![LEVEL.into(), CELL.into()];
        let prop = l1::proposals(&w.runs, v.run).unwrap()[0].proposal;
        errors.push(format!("{:?}", l1::confirm_binding(&w.runs, v.run, prop, Some(&bad)).unwrap_err()));
        errors.push(format!("{:?}", l1::compute(&w.runs, v.run, &[KEY.into()]).unwrap_err()));
        errors.push(format!("{:?}", l1::key_save(&w.ai, &format!("{KEY} {KEY}")).unwrap_err()));
    });
    let lines = lines.lock().unwrap().clone();
    let bodies: Vec<(gaply_core::stats_assist::spec::HelperTask, String)> =
        w.mock.sent().into_iter().map(|(t, b)| (t, String::from_utf8(b).unwrap())).collect();

    // Positive counts first, so nothing passes on empty input.
    assert_eq!(bodies.len(), 4, "every task sent once");
    assert!(errors.len() >= 10, "{errors:?}");
    assert!(lines.iter().filter(|l| l.contains("stats_")).count() >= 10, "tracing ran: {} lines", lines.len());
    assert!(views.len() >= 10);

    let everything: Vec<&String> = bodies.iter().map(|(_, b)| b).chain(errors.iter()).chain(lines.iter()).collect();
    for s in &everything {
        for n in [PATH, FILE, CELL, LEVEL, MS, KEY] {
            assert!(!s.contains(n), "{n} leaked into:\n{s}");
        }
    }
    // Views are for display on this device: headers and level labels may be
    // there; a path, a file name or the key never.
    for s in &views {
        for n in [PATH, FILE, KEY] {
            assert!(!s.contains(n), "{n} leaked into a view:\n{s}");
        }
    }
    // The header leaves only in binding_suggestion, where it was opted in.
    for (task, b) in &bodies {
        let has = b.contains(HEADER);
        assert_eq!(has, *task == gaply_core::stats_assist::spec::HelperTask::BindingSuggestion, "{task:?}");
        assert!(!lines.iter().any(|l| l.contains(HEADER)), "a header in tracing");
    }
    // What the opt-ins let through, so the test is not vacuous.
    let cm = &bodies[0].1;
    assert!(cm.contains("assuming equal variances") && cm.contains("scored lower than control"));
}
