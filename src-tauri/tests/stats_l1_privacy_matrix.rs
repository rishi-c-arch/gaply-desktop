//! **Phase 2, Step 8: Step 5's sentinels under EVERY combination of opt-ins,
//! across all four tasks, through the Level 1 commands and the real transport
//! against a capture server on localhost.** What is checked for each
//! combination: the raw request (request line, every header, the body), the
//! preview, the result view, every refusal, and every tracing line.
//!
//! - A cell, a level label, the unclaimed manuscript sentence, the path, the
//!   file name: in nothing that leaves, nothing logged, no error.
//! - The key: once per request, in the Authorization header, nowhere else.
//! - The column header: only in a `binding_suggestion` body whose
//!   `column_headers` opt-in is ticked.
//! - The claim sentence, the methods passage, the paper's DOI: only in a body
//!   whose task and opt-in allow it, and in each such body (so the test is
//!   not vacuous).
//! - No number in any body equals a cell value.
//!
//! Alone in its binary on purpose: it captures tracing, and `tracing` caches
//! callsite interest process-wide (see `stats_l1_commands_privacy.rs`).

mod l1_support;

use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use app_lib::stats_ai::RequestTimeout;
use app_lib::stats_l1_commands::{self as l1, HelperTaskInput};
use l1_support::*;

/// Text that each opt-in lets out, and only it.
const SENTENCE_WORDS: &str = "scored lower than control";
const PASSAGE: &str = "Groups were compared with a two-sample t-test assuming equal variances.";
const PASSAGE_WORDS: &str = "assuming equal variances";
const DOI: &str = "10.5555/example.2026.01";

fn reply_for(task: &str) -> String {
    match task {
        "claim_method" => json_reply(json!({"method_id": "independent_t", "variant": "student", "reason": "A t with two groups.", "relied_on": ["df"]})),
        "binding_suggestion" => binding_reply(),
        "result_explanation" => json_reply(json!({"explanation": "The data reproduced the printed result."})),
        "web_context" => envelope(
            "Check normality first ([bmj.com](https://www.bmj.com/a)).",
            json!([{"type": "url_citation", "url": "https://www.bmj.com/a", "title": "BMJ", "start_index": 22, "end_index": 57}]),
        ),
        other => panic!("{other}"),
    }
}

fn input(task: &str, cats: &[&str]) -> HelperTaskInput {
    match task {
        "claim_method" => HelperTaskInput::ClaimMethod {
            claim_id: 0,
            methods_passage: cats.contains(&"methods_passage").then(|| PASSAGE.to_string()),
        },
        "binding_suggestion" => HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() },
        "result_explanation" => HelperTaskInput::ResultExplanation { claim_id: 0 },
        "web_context" => HelperTaskInput::WebContext {
            claim_id: 0,
            method_id: "independent_t".into(),
            topic: "assumptions".into(),
            edited_query: None,
            paper: cats.contains(&"paper_doi_or_title").then(|| DOI.to_string()),
        },
        other => panic!("{other}"),
    }
}

/// Every subset of each task's opt-ins.
fn combinations() -> Vec<(&'static str, Vec<&'static str>)> {
    let subsets = |opts: &[&'static str]| -> Vec<Vec<&'static str>> {
        (0..1u32 << opts.len()).map(|m| opts.iter().enumerate().filter(|(i, _)| m & (1 << i) != 0).map(|(_, o)| *o).collect()).collect()
    };
    let mut out = Vec::new();
    for (task, opts) in [
        ("claim_method", vec!["claim_sentence", "methods_passage"]),
        ("binding_suggestion", vec!["column_headers", "claim_sentence"]),
        ("result_explanation", vec![]),
        ("web_context", vec!["paper_doi_or_title"]),
    ] {
        for s in subsets(&opts) {
            out.push((task, s));
        }
    }
    out
}

/// Every number anywhere in a JSON value, strings excluded.
fn numbers(v: &Value, out: &mut Vec<f64>) {
    match v {
        Value::Number(n) => out.extend(n.as_f64()),
        Value::Array(a) => a.iter().for_each(|x| numbers(x, out)),
        Value::Object(o) => o.values().for_each(|x| numbers(x, out)),
        _ => {}
    }
}

#[test]
fn every_opt_in_combination_of_every_task_leaks_no_sentinel_anywhere() {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    // (task, ticked, raw request, preview body)
    let mut sent: Vec<(&str, Vec<&str>, Vec<u8>, String)> = Vec::new();
    let mut views: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    tracing::subscriber::with_default(Capture { lines: lines.clone() }, || {
        tracing::callsite::rebuild_interest_cache();
        for (task, cats) in combinations() {
            let server = capture_server(vec![(200, reply_for(task))]);
            let w = world_with_real_transport(&server, None, RequestTimeout::FromSpec);
            let v = open(&w);
            views.push(serde_json::to_string(&v).unwrap());
            if task == "result_explanation" {
                let token = confirm_by_hand(&w, v.run);
                views.push(serde_json::to_string(&l1::compute(&w.runs, v.run, &[token]).unwrap()).unwrap());
            }
            let ticked: Vec<String> = cats.iter().map(|c| c.to_string()).collect();
            // With AI help off first: the refusal.
            errors.push(format!("{:?}", l1::helper_preview(&w.runs, &w.ai, v.run, &input(task, &cats), &ticked).unwrap_err()));
            ai_on(&w, true);
            let p = l1::helper_preview(&w.runs, &w.ai, v.run, &input(task, &cats), &ticked).unwrap();
            views.push(serde_json::to_string(&p).unwrap());
            let r = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 1_792_000_000).unwrap();
            views.push(serde_json::to_string(&r).unwrap());
            views.push(serde_json::to_string(&l1::usage(&w.runs, v.run).unwrap()).unwrap());
            // Refusals that could carry a path, a header, a level or a cell.
            for edit in [format!("t-test {HEADER}"), format!("t-test {LEVEL}"), format!("t-test {CELL}")] {
                let q = HelperTaskInput::WebContext { claim_id: 0, method_id: "independent_t".into(), topic: "assumptions".into(), edited_query: Some(edit), paper: None };
                if let Err(e) = l1::helper_preview(&w.runs, &w.ai, v.run, &q, &[]) {
                    errors.push(format!("{e:?}"));
                }
            }
            let mut bad = manual_edits();
            bad.levels[0].levels = vec![LEVEL.into(), CELL.into()];
            let prop = l1::proposals(&w.runs, v.run).unwrap()[0].proposal;
            errors.push(format!("{:?}", l1::confirm_binding(&w.runs, v.run, prop, Some(&bad)).unwrap_err()));
            let reqs = server.requests.lock().unwrap().clone();
            assert_eq!(reqs.len(), 1, "{task} {cats:?}: one request");
            sent.push((task, cats.clone(), reqs[0].clone(), p.body.clone()));
        }
    });
    let lines = lines.lock().unwrap().clone();

    // Positive counts first: every combination sent, and tracing ran.
    assert_eq!(sent.len(), 11, "4 + 4 + 1 + 2 combinations");
    // Per combination: AI help off, three blocked query edits (a header, a
    // level label, a text cell), one refused level selection.
    assert_eq!(errors.len(), 11 * 5, "every refusal happened");
    assert!(lines.iter().filter(|l| l.contains("stats_")).count() >= 50, "tracing ran: {} lines", lines.len());

    let cells: Vec<f64> = TREATED.iter().chain(CONTROL.iter()).copied().collect();
    let mut checks = 0usize;
    for (task, cats, raw, preview) in &sent {
        let label = format!("{task} {cats:?}");
        let (line, headers, body) = split(raw);
        let body_s = String::from_utf8(body.clone()).unwrap();
        let raw_s = String::from_utf8_lossy(raw).to_string();
        assert!(line == "POST /v1/responses HTTP/1.1", "{label}: the request line");
        let mut names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["accept", "accept-encoding", "authorization", "content-length", "content-type", "host"], "{label}");
        // The key: once, in its header. Values are never printed.
        assert_eq!(raw_s.matches(KEY).count(), 1, "{label}: the key appears once");
        assert!(headers.iter().any(|(k, v)| k == "authorization" && v == &format!("Bearer {KEY}")), "{label}: the key's header");
        assert!(headers.iter().filter(|(k, _)| k != "authorization").all(|(_, v)| !v.contains(KEY)), "{label}");
        assert_eq!(body_s, *preview, "{label}: the body is the preview");
        for n in [PATH, FILE, CELL, LEVEL, MS] {
            assert!(!raw_s.contains(n), "{label}: {n} in the request");
        }
        let header_allowed = *task == "binding_suggestion" && cats.contains(&"column_headers");
        assert_eq!(raw_s.contains(HEADER), header_allowed, "{label}: the column header");
        let sentence_allowed = cats.contains(&"claim_sentence");
        assert_eq!(body_s.contains(SENTENCE_WORDS), sentence_allowed, "{label}: the claim sentence");
        assert_eq!(body_s.contains(PASSAGE_WORDS), cats.contains(&"methods_passage"), "{label}: the methods passage");
        assert_eq!(body_s.contains(DOI), cats.contains(&"paper_doi_or_title"), "{label}: the paper");
        let mut nums = Vec::new();
        numbers(&serde_json::from_str(&body_s).unwrap(), &mut nums);
        for x in &nums {
            assert!(!cells.iter().any(|c| (c - x).abs() < 1e-12), "{label}: a cell value {x} in the body");
        }
        checks += 1;
    }
    assert_eq!(checks, 11);
    for s in errors.iter().chain(lines.iter()) {
        for n in [PATH, FILE, CELL, LEVEL, MS, KEY, HEADER] {
            assert!(!s.contains(n), "{n} leaked into:\n{s}");
        }
    }
    // Views are for display on this device: a header or a level label may be
    // there; a path, a file name or the key never.
    assert!(views.len() >= 44, "{} views", views.len());
    for s in &views {
        for n in [PATH, FILE, KEY] {
            assert!(!s.contains(n), "{n} leaked into a view");
        }
    }
    println!("combinations: {}; refusals: {}; views: {}; tracing lines: {}", sent.len(), errors.len(), views.len(), lines.len());
}
