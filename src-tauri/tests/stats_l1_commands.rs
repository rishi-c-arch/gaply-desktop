//! The Level 1 commands at their boundary (Phase 2, Step 6), with the mock
//! transport and the in-memory key store, and one real transport against a
//! capture server on localhost. No real service, no real key, no keychain.
//!
//! The sentinel test (paths, cells, labels, the key) is in its own binary,
//! `stats_l1_commands_privacy.rs`: a tracing capture shares callsite state
//! with every test thread in its binary, and alone it is deterministic.

mod l1_support;

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use app_lib::stats_ai::{AiSettings, CredentialStore, RequestTimeout};
use app_lib::stats_l1_commands::{self as l1, HelperTaskInput, L1ClaimInput, L1Handle, L1HelperResultView};
use gaply_core::stats_assist::transport::MockTransport;
use gaply_core::GaplyError;
use l1_support::*;

/* ---- the manual path ------------------------------------------------------ */

#[test]
fn the_manual_path_works_with_ai_help_off_no_key_and_no_network() {
    let w = world_with(MockTransport::new());
    let v = open(&w);
    assert_eq!(v.claims.len(), 1);
    assert!(v.claims[0].level0.is_some(), "Level 0 ran");
    assert_eq!(v.schema.columns.len(), 4);
    assert_eq!(v.schema.columns[0].header, HEADER, "the local view carries headers for display");
    assert!(!v.proposals.is_empty(), "local suggestions");
    let token = confirm_by_hand(&w, v.run);
    let out = l1::compute(&w.runs, v.run, &[token]).unwrap();
    assert_eq!(out.counts.reproduced, 1, "{out:?}");
    assert_eq!(w.mock.sent_count(), 0, "nothing reached a transport");
    assert!(!w.store.has_key().unwrap());
    // A helper preview with AI help off is refused, and the run still works.
    let e = l1::helper_preview(&w.runs, &w.ai, v.run, &HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: None }, &[]).unwrap_err();
    assert_eq!(code(&e), "ai_help_off");
    assert_eq!(l1::compute(&w.runs, v.run, &[]).unwrap().claims.len(), 1);
    assert_eq!(l1::key_status(&w.ai).unwrap().saved, false);
}

#[test]
fn level_0s_verdict_in_the_open_view_is_level_0s_own() {
    let w = world_with(MockTransport::new());
    let v = open(&w);
    let outcome = app_lib::stats_consistency::run_stats_consistency_measured(&w.db, &w.files.manuscript).unwrap();
    assert_eq!(outcome.claims.len(), 1);
    let l0 = serde_json::to_value(&v.claims[0].level0).unwrap();
    let expected = match &outcome.claims[0].verdict {
        gaply_core::stats_l0::method::L0Verdict::Consistent(_) => "consistent",
        gaply_core::stats_l0::method::L0Verdict::Inconsistent { .. } => "inconsistent",
        gaply_core::stats_l0::method::L0Verdict::Concern(_) => "concern",
        gaply_core::stats_l0::method::L0Verdict::CouldNotCheck(_) => "could_not_check",
    };
    assert_eq!(l0["code"], expected);
}

/* ---- helper calls --------------------------------------------------------- */


#[test]
fn preview_then_send_sends_the_previewed_bytes_and_makes_a_proposal() {
    let w = world_with(MockTransport::new().reply(&binding_reply()));
    ai_on(&w, false);
    let v = open(&w);
    let input = HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() };
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[]).unwrap();
    assert_eq!(p.task, "binding_suggestion");
    let r = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 1_792_000_000).unwrap();
    assert_eq!(w.mock.sent()[0].1, p.body.as_bytes(), "byte for byte");
    let L1HelperResultView::BindingSuggestion { proposal, .. } = r else { panic!("{r:?}") };
    let held = l1::proposals(&w.runs, v.run).unwrap();
    let ai = held.iter().find(|x| x.proposal == proposal).expect("the suggestion is a proposal");
    assert_eq!(ai.source, "ai_suggestion");
    assert_eq!(ai.variant, "not_stated", "a model never sets a variant");
    // It is still only a proposal: confirming needs the user's level choice.
    let token = l1::confirm_binding(&w.runs, v.run, proposal, Some(&manual_edits())).unwrap().token;
    assert_eq!(l1::compute(&w.runs, v.run, &[token]).unwrap().counts.reproduced, 1);
    let u = l1::usage(&w.runs, v.run).unwrap();
    let b = u.tasks.iter().find(|t| t.task == "binding_suggestion").unwrap();
    assert_eq!((b.requests, b.input_tokens, b.output_tokens), (1, 300, 40));
}

#[test]
fn send_with_ai_help_off_or_no_key_is_refused_before_any_network() {
    let w = world_with(MockTransport::new().reply(&binding_reply()));
    ai_on(&w, false);
    let v = open(&w);
    let input = HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() };
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[]).unwrap();
    w.ai.set_settings(AiSettings::default()).unwrap();
    assert_eq!(code(&l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap_err()), "ai_help_off");
    ai_on(&w, false);
    w.store.remove_key().unwrap();
    assert_eq!(code(&l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap_err()), "no_key");
    // Web context needs its own switch.
    w.store.save_key(KEY).unwrap();
    let web = HelperTaskInput::WebContext { claim_id: 0, method_id: "independent_t".into(), topic: "assumptions".into(), edited_query: None, paper: None };
    assert_eq!(code(&l1::helper_preview(&w.runs, &w.ai, v.run, &web, &[]).unwrap_err()), "web_context_off");
    assert_eq!(w.mock.sent_count(), 0);
    // Every other command still works.
    assert_eq!(l1::compute(&w.runs, v.run, &[]).unwrap().claims.len(), 1);
}

#[test]
fn a_grant_is_per_task_and_revocable() {
    let w = world_with(
        MockTransport::new()
            .reply(&json_reply(json!({"method_id": "independent_t", "variant": "not_stated", "reason": "r", "relied_on": []})))
            .reply(&json_reply(json!({"method_id": "independent_t", "variant": "not_stated", "reason": "r", "relied_on": []}))),
    );
    ai_on(&w, false);
    let v = open(&w);
    let input = HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: None };
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[]).unwrap();
    l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap();
    assert!(!l1::helper_revoke(&w.runs, v.run, "binding_suggestion").unwrap(), "no grant for another task");
    assert!(l1::helper_revoke(&w.runs, v.run, "claim_method").unwrap());
    assert!(!l1::helper_revoke(&w.runs, v.run, "claim_method").unwrap(), "revoked");
    // Clicking Send again is a new consent.
    l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap();
    assert!(l1::helper_revoke(&w.runs, v.run, "claim_method").unwrap());
    assert_eq!(code(&l1::helper_revoke(&w.runs, v.run, "agent_loop").unwrap_err()), "unknown_task");
}

#[test]
fn a_preview_or_proposal_from_another_run_is_refused() {
    let w = world_with(MockTransport::new().reply(&binding_reply()));
    ai_on(&w, false);
    let a = open(&w);
    let b = open(&w);
    let input = HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() };
    let pb = l1::helper_preview(&w.runs, &w.ai, b.run, &input, &[]).unwrap();
    assert_eq!(code(&l1::helper_send(&w.runs, &w.ai, a.run, pb.preview, 0).unwrap_err()), "preview_from_another_run");
    let forged = L1Handle { run: a.run, id: pb.preview.id };
    assert_eq!(code(&l1::helper_send(&w.runs, &w.ai, a.run, forged, 0).unwrap_err()), "unknown_preview");
    let prop_b = l1::proposals(&w.runs, b.run).unwrap()[0].proposal;
    assert_eq!(code(&l1::confirm_binding(&w.runs, a.run, prop_b, None).unwrap_err()), "proposal_from_another_run");
    assert_eq!(code(&l1::compute(&w.runs, 999, &[]).unwrap_err()), "unknown_run");
    assert_eq!(w.mock.sent_count(), 0);
}

#[test]
fn an_explanation_is_retried_once_then_refused_with_a_closed_code() {
    let w = world_with(
        MockTransport::new()
            .reply(&json_reply(json!({"explanation": "It was reproduced with t = 99.99."})))
            .reply(&json_reply(json!({"explanation": "It was not reproduced."}))),
    );
    ai_on(&w, false);
    let v = open(&w);
    let token = confirm_by_hand(&w, v.run);
    l1::compute(&w.runs, v.run, &[token]).unwrap();
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &HelperTaskInput::ResultExplanation { claim_id: 0 }, &[]).unwrap();
    let e = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap_err();
    assert_eq!(code(&e), "reply_rejected");
    assert!(e.to_string().contains("wrong_verdict"), "{e}");
    assert_eq!(w.mock.sent_count(), 2, "one retry, no more");
    assert_eq!(l1::usage(&w.runs, v.run).unwrap().tasks.iter().find(|t| t.task == "result_explanation").unwrap().requests, 2);
}

/// The premise that makes `open`'s extraction-error branch unreachable:
/// preflight parses the file first and stops one that does not parse. If this
/// goes red, that branch is reachable and its own code must be checked for
/// leaks like the others.
#[test]
fn a_document_that_does_not_parse_is_stopped_by_the_preflight() {
    let w = world_with(MockTransport::new());
    for ext in ["pdf", "docx"] {
        let junk = w.files.manuscript.with_file_name(format!("{FILE}_junk.{ext}"));
        std::fs::write(&junk, b"not a document at all").unwrap();
        let c = code(&l1::open(&w.db, &w.runs, &w.ai, &junk, &w.files.data).err().expect("refused"));
        assert!(c == "manuscript_unreadable" || c == "manuscript_refused", "{ext}: {c}");
    }
}

/* ---- tokens --------------------------------------------------------------- */

#[test]
fn an_edited_claim_kills_its_token() {
    let w = world_with(MockTransport::new());
    let v = open(&w);
    let token = confirm_by_hand(&w, v.run);
    let mut edit = L1ClaimInput { family: "t".into(), statistic: "9.99".into(), df: vec!["6".into()], n: None, p: Some(".04".into()), p_relation: None, sentence: None };
    let rev = l1::edit_claim(&w.runs, v.run, 0, &edit).unwrap();
    assert_ne!(rev.revision, v.claims[0].revision);
    let out = l1::compute(&w.runs, v.run, &[token]).unwrap();
    assert_eq!(out.counts.could_not_check, 1, "{out:?}");
    // The closed code (Step 7), not a Debug name.
    let reason = serde_json::to_value(&out.claims[0].verdict).unwrap();
    assert_eq!(reason, json!({"kind": "couldNotCheck", "reason": {"code": "token_stale", "rejection": {"code": "revoked", "because": "claim_revised"}}}));
    edit.family = "w".into();
    assert_eq!(code(&l1::edit_claim(&w.runs, v.run, 0, &edit).unwrap_err()), "unknown_family");
}

#[test]
fn changed_file_bytes_give_a_stale_could_not_check() {
    let w = world_with(MockTransport::new());
    let v = open(&w);
    let token = confirm_by_hand(&w, v.run);
    let mut moved = TREATED;
    moved[0] += 1.0;
    std::fs::write(&w.files.data, csv(&moved)).unwrap();
    let out = l1::compute(&w.runs, v.run, &[token]).unwrap();
    assert_eq!(out.counts.could_not_check, 1, "{out:?}");
    assert_eq!(out.counts.reproduced + out.counts.not_reproduced, 0);
    let reason = serde_json::to_value(&out.claims[0].verdict).unwrap();
    assert_eq!(reason, json!({"kind": "couldNotCheck", "reason": {"code": "refused", "refusal": "fingerprint_changed"}}));
}

#[test]
fn compute_with_a_forged_handle_is_refused() {
    let w = world_with(MockTransport::new());
    let v = open(&w);
    let real = confirm_by_hand(&w, v.run);
    // One hex digit changed: always a different digit, never the real one.
    let flipped = format!("{}{}", &real[..63], if real.ends_with('0') { '1' } else { '0' });
    assert_ne!(flipped, real);
    for forged in ["", "abc", &"0".repeat(64), &flipped] {
        assert_eq!(code(&l1::compute(&w.runs, v.run, &[forged.to_string()]).unwrap_err()), "unknown_token_handle", "{forged}");
    }
    // A real handle from another run is forged here.
    let other = open(&w);
    assert_eq!(code(&l1::compute(&w.runs, other.run, &[real]).unwrap_err()), "unknown_token_handle");
}

/* ---- the key -------------------------------------------------------------- */

#[test]
fn no_key_command_ever_returns_the_key() {
    let w = world_with(MockTransport::new());
    let mut seen = Vec::new();
    seen.push(serde_json::to_string(&l1::key_status(&w.ai).unwrap()).unwrap());
    seen.push(serde_json::to_string(&l1::key_save(&w.ai, &format!("  {KEY}  ")).unwrap()).unwrap());
    seen.push(serde_json::to_string(&l1::key_status(&w.ai).unwrap()).unwrap());
    seen.push(serde_json::to_string(&l1::key_test(&w.ai)).unwrap());
    seen.push(format!("{:?}", l1::key_save(&w.ai, "two words").unwrap_err()));
    seen.push(serde_json::to_string(&l1::key_remove(&w.ai).unwrap()).unwrap());
    assert!(seen[1].contains("\"saved\":true") && seen[1].contains("openai"), "{}", seen[1]);
    assert!(seen[5].contains("\"saved\":false"));
    for s in &seen {
        assert!(!s.contains(KEY), "{s}");
    }
}

#[test]
fn the_generic_secret_commands_refuse_the_openai_entry() {
    use app_lib::commands::{delete_secret, has_secret, store_secret};
    let name = app_lib::stats_ai::OPENAI_KEY_ENTRY.to_string();
    // Refused before the keychain is touched: the test never reaches it.
    assert!(store_secret(name.clone(), KEY.into()).unwrap_err().to_string().contains("reserved_secret"));
    assert!(has_secret(name.clone()).unwrap_err().to_string().contains("reserved_secret"));
    assert!(delete_secret(format!(" {name} ")).unwrap_err().to_string().contains("reserved_secret"));
}

/* ---- links ---------------------------------------------------------------- */

#[test]
fn only_https_links_to_allowed_domains_are_opened() {
    for bad in [
        "http://www.bmj.com/content/1",
        "https://bmj.com.evil.io/x",
        "https://evilbmj.com/x",
        "https://bmj.com@evil.io/x",
        "javascript:alert(1)",
        "JAVASCRIPT:alert(1)",
        "file:///etc/passwd",
        "https://www.bmj.com/x y",
        "https://www.bmj.com/\u{7}",
        "",
    ] {
        assert!(l1::checked_source_link(bad).is_err(), "{bad} was accepted");
    }
    assert_eq!(l1::checked_source_link("https://www.bmj.com/content/1").unwrap(), "https://www.bmj.com/content/1");
    assert!(l1::checked_source_link("https://pmc.ncbi.nlm.nih.gov/articles/2").is_ok());
}

/* ---- the real transport against a capture server (in `l1_support`) ------- */

#[test]
fn the_real_transport_sends_exactly_the_previewed_request_and_the_key_only_in_its_header() {
    let server = capture_server(vec![(200, binding_reply())]);
    let w = world_with_real_transport(&server, None, RequestTimeout::FromSpec);
    ai_on(&w, false);
    let v = open(&w);
    let input = HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() };
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[]).unwrap();
    let r = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap();
    assert!(matches!(r, L1HelperResultView::BindingSuggestion { .. }));

    let reqs = server.requests.lock().unwrap().clone();
    assert_eq!(reqs.len(), 1);
    let (line, headers, body) = split(&reqs[0]);
    // No value printed on failure: a broken build could put the key here.
    assert!(line == "POST /v1/responses HTTP/1.1", "the request line is not POST /v1/responses");
    let mut names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
    names.sort_unstable();
    // Names only in the message: the Authorization value is never printed.
    assert_eq!(names, ["accept", "accept-encoding", "authorization", "content-length", "content-type", "host"]);
    let get = |k: &str| headers.iter().find(|(n, _)| n == k).unwrap().1.clone();
    assert!(get("authorization") == format!("Bearer {KEY}"), "the Authorization header is the saved key");
    assert_eq!(get("content-type"), "application/json");
    assert_eq!(get("accept"), "*/*");
    // The app's reqwest is built with gzip and brotli.
    assert_eq!(get("accept-encoding"), "gzip,br");
    assert_eq!(get("content-length"), p.body.len().to_string());
    assert_eq!(body, p.body.as_bytes(), "the body is the preview, byte for byte");
    let raw = String::from_utf8_lossy(&reqs[0]).to_string();
    assert_eq!(raw.matches(KEY).count(), 1, "the key appears once, in the Authorization header");
    for n in [PATH, FILE, CELL, LEVEL, MS, HEADER] {
        assert!(!raw.contains(n), "{n} in the request");
    }
}

#[test]
fn the_transport_retries_once_on_a_5xx_or_429_and_never_on_a_4xx() {
    for (first, expect_requests, ok) in [(500u16, 2usize, true), (429, 2, true), (400, 1, false), (401, 1, false)] {
        let server = capture_server(vec![(first, "{}".into()), (200, binding_reply())]);
        let w = world_with_real_transport(&server, None, RequestTimeout::FromSpec);
        ai_on(&w, false);
        let v = open(&w);
        let p = l1::helper_preview(&w.runs, &w.ai, v.run, &HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() }, &[]).unwrap();
        let r = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0);
        assert_eq!(r.is_ok(), ok, "{first}: {r:?}");
        if !ok {
            assert_eq!(code(&r.unwrap_err()), "transport");
        }
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(server.requests.lock().unwrap().len(), expect_requests, "{first}");
    }
}

/// The first attempt stalls past the timeout: one retry, which succeeds.
#[test]
fn a_stalled_attempt_is_retried_once() {
    let server = capture_server(vec![(STALL, String::new()), (200, binding_reply())]);
    let w = world_with_real_transport(&server, None, RequestTimeout::Fixed(Duration::from_millis(300)));
    ai_on(&w, false);
    let v = open(&w);
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() }, &[]).unwrap();
    let r = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap();
    assert!(matches!(r, L1HelperResultView::BindingSuggestion { .. }));
    assert_eq!(server.requests.lock().unwrap().len(), 2, "the stalled attempt and one retry");
}

/// Both attempts stall: the closed timeout code, and no third attempt, though
/// the server would have answered it.
#[test]
fn a_second_stall_is_not_retried_again_and_gives_the_timeout_code() {
    let server = capture_server(vec![(STALL, String::new()), (STALL, String::new()), (200, binding_reply())]);
    let w = world_with_real_transport(&server, None, RequestTimeout::Fixed(Duration::from_millis(300)));
    ai_on(&w, false);
    let run = open(&w).run;
    let p = l1::helper_preview(&w.runs, &w.ai, run, &HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() }, &[]).unwrap();
    let e = l1::helper_send(&w.runs, &w.ai, run, p.preview, 0).unwrap_err();
    let GaplyError::Validation(m) = &e else { panic!("{e:?}") };
    let reason: serde_json::Value = serde_json::from_str(m).unwrap();
    assert_eq!(reason, json!({"code": "transport", "error": "timeout"}));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(server.requests.lock().unwrap().len(), 2, "one retry, never a second");
    // Both stalled attempts left the device and count as requests; neither
    // reply came back, so no tokens were reported.
    let u = l1::usage(&w.runs, run).unwrap();
    let b = u.tasks.iter().find(|t| t.task == "binding_suggestion").unwrap();
    assert_eq!((b.requests, b.input_tokens, b.output_tokens), (2, 0, 0));
    assert_eq!(u.tasks.iter().map(|t| t.requests).sum::<u32>(), 2, "no other task recorded a request");
}

#[test]
fn a_cancelled_run_sends_nothing_and_no_key_sends_nothing() {
    let server = capture_server(vec![(200, binding_reply())]);
    let cancel = Arc::new(AtomicBool::new(true));
    let w = world_with_real_transport(&server, Some(cancel), RequestTimeout::FromSpec);
    ai_on(&w, false);
    let v = open(&w);
    let p = l1::helper_preview(&w.runs, &w.ai, v.run, &HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() }, &[]).unwrap();
    let e = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 0).unwrap_err();
    assert!(e.to_string().contains("cancelled"), "{e}");
    assert!(server.requests.lock().unwrap().is_empty());
}

#[test]
fn the_key_test_sends_no_user_data() {
    let server = capture_server(vec![(200, "{\"data\":[]}".into()), (401, "{}".into())]);
    let w = world_with_real_transport(&server, None, RequestTimeout::FromSpec);
    assert_eq!(l1::key_test(&w.ai).result, "no_key");
    w.store.save_key(KEY).unwrap();
    assert_eq!(l1::key_test(&w.ai).result, "accepted");
    assert_eq!(l1::key_test(&w.ai).result, "rejected");
    let reqs = server.requests.lock().unwrap().clone();
    assert_eq!(reqs.len(), 2);
    let (line, headers, body) = split(&reqs[0]);
    assert_eq!(line, "GET /v1/models HTTP/1.1");
    assert!(body.is_empty());
    let mut names: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["accept", "accept-encoding", "authorization", "host"]);
}

#[test]
fn runs_are_bounded_and_closed() {
    let w = world_with(MockTransport::new());
    let ids: Vec<u64> = (0..app_lib::stats_l1_commands::MAX_RUNS + 2).map(|_| open(&w).run).collect();
    assert_eq!(w.runs.len(), app_lib::stats_l1_commands::MAX_RUNS);
    assert_eq!(code(&l1::compute(&w.runs, ids[0], &[]).unwrap_err()), "unknown_run", "the oldest was dropped");
    let last = *ids.last().unwrap();
    assert!(w.runs.close(last));
    assert_eq!(code(&l1::usage(&w.runs, last).unwrap_err()), "unknown_run");
    let _: &Path = &w.files.root;
}

/* ---- Step 7: what the screens read from the open view --------------------- */

#[test]
fn the_open_view_carries_the_catalogue_and_who_set_each_variant() {
    let w = world_with(MockTransport::new());
    let v = open(&w);
    let cat = serde_json::to_value(&v.catalogue).unwrap();
    assert_eq!(
        cat,
        json!([
            {"method": "independent_t", "slots": [
                {"role": "outcome", "kind": "numeric", "levels": null, "min": null},
                {"role": "group", "kind": "group", "levels": {"code": "exactly", "levels": 2}, "min": null}],
             "variants": ["student", "welch"], "needsTarget": false},
            {"method": "paired_t", "slots": [
                {"role": "first", "kind": "numeric", "levels": null, "min": null},
                {"role": "second", "kind": "numeric", "levels": null, "min": null}],
             "variants": [], "needsTarget": false},
            {"method": "one_way_anova", "slots": [
                {"role": "outcome", "kind": "numeric", "levels": null, "min": null},
                {"role": "group", "kind": "group", "levels": {"code": "at_least", "levels": 2}, "min": null}],
             "variants": [], "needsTarget": false},
            {"method": "pearson", "slots": [
                {"role": "x", "kind": "numeric", "levels": null, "min": null},
                {"role": "y", "kind": "numeric", "levels": null, "min": null}],
             "variants": [], "needsTarget": false},
            {"method": "spearman", "slots": [
                {"role": "x", "kind": "numeric", "levels": null, "min": null},
                {"role": "y", "kind": "numeric", "levels": null, "min": null}],
             "variants": ["t_approximation", "exact"], "needsTarget": false},
            {"method": "chi_square_independence", "slots": [
                {"role": "row_variable", "kind": "group", "levels": {"code": "at_least", "levels": 2}, "min": null},
                {"role": "column_variable", "kind": "group", "levels": {"code": "at_least", "levels": 2}, "min": null}],
             "variants": ["continuity_correction", "no_continuity_correction"], "needsTarget": false},
            {"method": "ols", "slots": [
                {"role": "outcome", "kind": "numeric", "levels": null, "min": null},
                {"role": "predictors", "kind": "numeric_list", "levels": null, "min": 1}],
             "variants": [], "needsTarget": true}
        ])
    );
    // Every proposal says who set its variant, as a closed code; a local
    // proposal on a sentence that names no variant is not stated.
    assert!(!v.proposals.is_empty());
    for p in &v.proposals {
        let set_by = serde_json::to_value(&p.variant_set_by).unwrap();
        let code = set_by["code"].as_str().unwrap();
        assert!(["not_stated", "stated_in_paper", "user_ticked"].contains(&code), "{set_by}");
        assert_eq!(code == "not_stated", p.variant == "not_stated" || p.variant == "none", "{set_by} vs {}", p.variant);
    }
}

/// An edit drops the claim's proposals (Step 2's rule), so the screen needs a
/// way to turn its form into one: `draft_proposal`. Without a starting
/// proposal it is Manual; from one, Manual edited from that source.
#[test]
fn a_draft_after_an_edit_is_the_only_way_back_to_a_confirmation_and_it_computes() {
    let w = world_with(MockTransport::new().reply(&binding_reply()));
    ai_on(&w, false);
    let v = open(&w);
    let c0 = &v.claims[0];
    let same = L1ClaimInput {
        family: c0.family.into(),
        statistic: c0.statistic.clone(),
        df: c0.df.clone(),
        n: c0.n.clone(),
        p: Some(c0.p.trim_start_matches(|c: char| "=<>".contains(c)).trim().to_string()),
        p_relation: Some(c0.p.split_whitespace().next().unwrap().to_string()),
        sentence: None,
    };
    l1::edit_claim(&w.runs, v.run, 0, &same).unwrap();
    assert!(l1::proposals(&w.runs, v.run).unwrap().iter().all(|p| p.claim_id != 0), "an edit drops the claim's proposals");

    let p = l1::draft_proposal(&w.runs, v.run, 0, None, &manual_edits()).unwrap();
    assert_eq!((p.claim_id, p.source, p.method.as_str()), (0, "manual", "independent_t"));
    assert_eq!(serde_json::to_value(&p.variant_set_by).unwrap()["code"], "not_stated");
    let token = l1::confirm_binding(&w.runs, v.run, p.proposal, None).unwrap().token;
    let out = l1::compute(&w.runs, v.run, &[token]).unwrap();
    assert_eq!(out.counts.reproduced, 1, "{out:?}");
    let prov = out.claims[0].provenance.as_ref().expect("a computed claim carries provenance");
    assert_eq!(serde_json::to_value(&prov.binding_source).unwrap(), json!({"code": "manual"}));

    // From an AI suggestion: recorded as edited from AI.
    let input = HelperTaskInput::BindingSuggestion { claim_id: 0, method_id: "independent_t".into() };
    let pv = l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[]).unwrap();
    let L1HelperResultView::BindingSuggestion { proposal, .. } = l1::helper_send(&w.runs, &w.ai, v.run, pv.preview, 0).unwrap() else { panic!() };
    let edited = l1::draft_proposal(&w.runs, v.run, 0, Some(proposal), &manual_edits()).unwrap();
    let token = l1::confirm_binding(&w.runs, v.run, edited.proposal, None).unwrap().token;
    let out = l1::compute(&w.runs, v.run, &[token]).unwrap();
    let prov = out.claims[0].provenance.as_ref().unwrap();
    assert_eq!(serde_json::to_value(&prov.binding_source).unwrap(), json!({"code": "manual_edited_from_ai"}));

    // A handle for another claim, or from another run, is refused.
    let other = l1::add_claim(&w.runs, v.run, &same).unwrap().claim_id;
    assert_eq!(code(&l1::draft_proposal(&w.runs, v.run, other, Some(edited.proposal), &manual_edits()).unwrap_err()), "proposal_for_another_claim");
    let forged = L1Handle { run: v.run + 7, id: edited.proposal.id };
    assert_eq!(code(&l1::draft_proposal(&w.runs, v.run, 0, Some(forged), &manual_edits()).unwrap_err()), "proposal_from_another_run");
}
