//! **Phase 2 evaluation fixes, part B: the AI helper layer through the
//! commands.** Written before the fixes, from the evaluations' own inputs:
//! eval2 B-4 (typed text into requests), M-1 (a retry after AI help is off),
//! m-1 (redirects), m-4 and eval3 (the key test); eval3 B-3 (a cell value and
//! a short header in the edited query).
//!
//! Mock transports, the in-memory credential store and localhost servers
//! only: never a real key, never a real host. Run with `--nocapture` for the
//! attack table.

mod l1_support;

use std::collections::VecDeque;
use std::io::Write as _;
use std::net::TcpListener;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use app_lib::stats_ai::{AiSettings, CredentialStore, MemoryCredentialStore, RequestTimeout, ResponsesTransport, StatsAi, TransportFactory};
use app_lib::stats_l1_commands::{self as l1, HelperTaskInput, L1ClaimInput, L1Runs};
use gaply_core::stats_assist::transport::{HelperTransport, MockTransport, OutgoingRequest, TransportError, TransportReply};
use gaply_core::Database;
use l1_support::*;

/* ------------------------------------------------------------------------ *
 *  1. Typed text into requests (eval2 B-4, eval3 B-3)
 * ------------------------------------------------------------------------ */

/// Build a web_context input from JSON, as the webview would send it. A
/// field the command does not know is refused at the boundary.
fn web_input(extra: Value) -> Result<HelperTaskInput, String> {
    let mut v = json!({"task": "web_context", "claimId": 0, "methodId": "independent_t", "topic": "assumptions"});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    serde_json::from_value::<HelperTaskInput>(v).map_err(|_| "refused at the command boundary".to_string())
}

/// Every lower-cased fragment a leak would show.
const NEEDLES: [&str; 6] = ["secret", "12.1", "14.3", "file_secret", "_solo", "ms_secret"];

fn leaked(body: &str) -> Vec<&'static str> {
    let b = body.to_lowercase();
    NEEDLES.iter().copied().filter(|n| b.contains(n)).collect()
}

#[test]
fn no_typed_text_reaches_a_web_context_request() {
    let w = world_with(MockTransport::new().reply(&envelope("Nothing cited.", json!([]))));
    ai_on(&w, true);
    let v = open(&w);
    let zw = |s: &str| s.chars().flat_map(|c| [c, '\u{200b}']).collect::<String>();
    let full_width: String = HEADER.chars().map(|c| if c.is_ascii_alphanumeric() { char::from_u32(c as u32 + 0xFEE0).unwrap() } else { c }).collect();
    // (attack, the input's extra fields, the categories ticked)
    let attacks: Vec<(String, Value, Vec<&str>)> = vec![
        ("edited query: a cell value and a 3-letter header (eval3 B-3)".into(), json!({"editedQuery": "t-test ab3 12.1"}), vec![]),
        ("edited query: header inside a word (eval2 B-4)".into(), json!({"editedQuery": format!("t-test x{HEADER}")}), vec![]),
        ("edited query: header split by spaces".into(), json!({"editedQuery": "t-test HEADER SECRET c42"}), vec![]),
        ("edited query: header with zero-width spaces".into(), json!({"editedQuery": format!("t-test {}", zw(HEADER))}), vec![]),
        ("edited query: level label split".into(), json!({"editedQuery": "t-test LEVEL SECRET b18"}), vec![]),
        ("edited query: one manuscript word".into(), json!({"editedQuery": format!("t-test {MS}")}), vec![]),
        ("edited query: a text cell".into(), json!({"editedQuery": format!("t-test {CELL}")}), vec![]),
        ("DOI/title field: a manuscript sentence (eval2 B-4)".into(), json!({"paper": format!("Baseline readings {MS} did not move")}), vec!["paper_doi_or_title", "paper_doi"]),
        ("DOI/title field: a header and a cell".into(), json!({"paper": format!("{HEADER} {CELL}")}), vec!["paper_doi_or_title", "paper_doi"]),
        ("DOI field (new name): a manuscript sentence".into(), json!({"doi": format!("Baseline readings {MS} did not move")}), vec!["paper_doi"]),
        ("DOI field: a DOI holding a header".into(), json!({"doi": format!("10.1234/{HEADER}")}), vec!["paper_doi"]),
        ("DOI field: a DOI holding a full-width header".into(), json!({"doi": format!("10.1234/{full_width}")}), vec!["paper_doi"]),
        ("DOI field: a DOI holding a zero-width header".into(), json!({"doi": format!("10.1234/{}", zw(HEADER))}), vec!["paper_doi"]),
        ("DOI field: a DOI holding a level label".into(), json!({"doi": format!("10.1234/{LEVEL}")}), vec!["paper_doi"]),
        ("DOI field: a DOI holding a text cell".into(), json!({"doi": format!("10.1234/x{CELL}")}), vec!["paper_doi"]),
        ("DOI field: a DOI holding the file name".into(), json!({"doi": format!("10.1234/{FILE}_data.csv")}), vec!["paper_doi"]),
        ("DOI field: a DOI holding a level label case-folded".into(), json!({"doi": format!("10.1234/{}", LEVEL.to_lowercase())}), vec!["paper_doi"]),
    ];
    println!("| attack | expected | actual |\n|---|---|---|");
    let mut failures = Vec::new();
    for (name, extra, cats) in attacks {
        let actual = match web_input(extra) {
            Err(e) => e,
            Ok(input) => {
                // Try each category word: the old one is refused once removed.
                let mut outcome = String::new();
                for c in &cats {
                    match l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[c.to_string()]) {
                        Ok(p) => {
                            let found = leaked(&p.body);
                            let sent = l1::helper_send(&w.runs, &w.ai, v.run, p.preview, 1_792_000_000);
                            outcome = if found.is_empty() { format!("previewed without it ({:?})", sent.is_ok()) } else { format!("LEAKED {found:?}") };
                            break;
                        }
                        Err(e) => outcome = format!("blocked before preview: {}", code(&e)),
                    }
                }
                if cats.is_empty() {
                    outcome = match l1::helper_preview(&w.runs, &w.ai, v.run, &input, &[]) {
                        Ok(p) => {
                            let found = leaked(&p.body);
                            if found.is_empty() { "previewed without it".into() } else { format!("LEAKED {found:?}") }
                        }
                        Err(e) => format!("blocked before preview: {}", code(&e)),
                    };
                }
                outcome
            }
        };
        println!("| {name} | blocked, or nothing typed sent | {actual} |");
        if actual.contains("LEAKED") {
            failures.push(format!("{name}: {actual}"));
        }
    }
    let on_wire: Vec<Vec<&str>> = w.mock.sent().iter().map(|(_, b)| leaked(&String::from_utf8_lossy(b))).collect();
    assert!(on_wire.iter().all(Vec::is_empty), "a typed value went on the wire: {on_wire:?}");
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn the_doi_field_takes_only_a_doi_and_the_title_option_is_gone() {
    let w = world_with(MockTransport::new().reply(&envelope("x.", json!([]))));
    ai_on(&w, true);
    let v = open(&w);
    let preview = |doi: &str| {
        let input = web_input(json!({"doi": doi}))?;
        l1::helper_preview(&w.runs, &w.ai, v.run, &input, &["paper_doi".to_string()]).map_err(|e| code(&e))
    };
    let p = preview("  https://doi.org/10.1371/journal.pone.0106616 ").expect("a DOI with its resolver prefix");
    let body: Value = serde_json::from_str(&p.body).unwrap();
    let input: Value = serde_json::from_str(body["input"].as_str().unwrap()).unwrap();
    assert_eq!(input["doi"], "10.1371/journal.pone.0106616");
    assert!(input.get("paper").is_none());
    assert_eq!(preview("doi:10.1000/xyz123").map(|_| ()), Ok(()));
    for bad in ["An attempt to target anxiety", "10.12/short", "10.1234/", "11.1234/x", "10.1234/x y", "10.1234/é", &format!("10.1234/{}", "x".repeat(200))] {
        assert_eq!(preview(bad).map(|_| ()), Err("doi_malformed".to_string()), "{bad:?}");
    }
    let input = web_input(json!({"doi": "10.1000/xyz123"})).unwrap();
    assert_eq!(code(&l1::helper_preview(&w.runs, &w.ai, v.run, &input, &["paper_doi_or_title".to_string()]).unwrap_err()), "unknown_category", "the title option is gone");
}

#[test]
fn the_claim_sentence_opt_in_carries_only_a_sentence_of_the_manuscript() {
    let w = world_with(MockTransport::new());
    ai_on(&w, true);
    let v = open(&w);
    let (t, df, p) = printed();
    let typed = L1ClaimInput {
        family: "t".into(),
        statistic: t,
        df: vec![df],
        n: None,
        p: Some(p),
        p_relation: Some("=".into()),
        sentence: Some(format!("A sentence I typed with {MS} in it.")),
    };
    let added = l1::add_claim(&w.runs, v.run, &typed).expect("added");
    let input = HelperTaskInput::ClaimMethod { claim_id: added.claim_id, methods_passage: None };
    let r = l1::helper_preview(&w.runs, &w.ai, v.run, &input, &["claim_sentence".to_string()]);
    match r {
        Ok(p) => panic!("a typed sentence was previewed as the manuscript's: leaked {:?}", leaked(&p.body)),
        Err(e) => assert_eq!(code(&e), "sentence_not_in_manuscript"),
    }
    // The manuscript's own claim still sends its sentence.
    let own = HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: None };
    assert!(l1::helper_preview(&w.runs, &w.ai, v.run, &own, &["claim_sentence".to_string()]).is_ok());
    // A Methods passage must lie inside a Methods paragraph.
    let results_text = HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: Some("The treated arm scored lower than control".into()) };
    assert_eq!(code(&l1::helper_preview(&w.runs, &w.ai, v.run, &results_text, &["methods_passage".to_string()]).unwrap_err()), "passage_not_in_methods");
}

/* ------------------------------------------------------------------------ *
 *  2. Every request checked on the wire (eval2 M-1, eval3 M-8)
 * ------------------------------------------------------------------------ */

/// Holds its first call until the test releases it, then answers in order.
struct Holding {
    arrived: Mutex<Option<mpsc::Sender<()>>>,
    release: Mutex<Option<mpsc::Receiver<()>>>,
    replies: Mutex<VecDeque<String>>,
    n: AtomicU32,
}

impl HelperTransport for Holding {
    fn send(&self, _request: &OutgoingRequest<'_>) -> Result<TransportReply, TransportError> {
        let i = self.n.fetch_add(1, Ordering::SeqCst);
        if i == 0 {
            if let Some(tx) = self.arrived.lock().unwrap().take() {
                let _ = tx.send(());
            }
            if let Some(rx) = self.release.lock().unwrap().take() {
                let _ = rx.recv();
            }
        }
        match self.replies.lock().unwrap().pop_front() {
            Some(b) => Ok(TransportReply { body: b.into_bytes(), request_id: Some("req_hold".into()), latency_ms: 1 }),
            None => Err(TransportError::Failed),
        }
    }
    fn requests_sent(&self) -> u32 {
        self.n.load(Ordering::SeqCst)
    }
}

struct Ctx {
    _files: Files,
    db: Database,
    runs: L1Runs,
    ai: StatsAi,
    store: Arc<MemoryCredentialStore>,
}

fn ctx_with(factory: TransportFactory, base: &str) -> Ctx {
    let store = Arc::new(MemoryCredentialStore::default());
    let ai = StatsAi::for_tests(store.clone(), factory, base);
    Ctx { _files: files(), db: Database::in_memory().unwrap(), runs: L1Runs::default(), ai, store }
}

fn turn_on(c: &Ctx) {
    c.ai.set_settings(AiSettings { ai_help: true, web_context: true, model: "gpt-5.5".into() }).unwrap();
    c.store.save_key(KEY).unwrap();
}

fn explanation_ready(c: &Ctx) -> u64 {
    let v = l1::open(&c.db, &c.runs, &c.ai, &c._files.manuscript, &c._files.data).expect("open");
    let first = l1::proposals(&c.runs, v.run).unwrap()[0].proposal;
    let tok = l1::confirm_binding(&c.runs, v.run, first, Some(&manual_edits())).expect("confirmed").token;
    l1::compute(&c.runs, v.run, &[tok]).unwrap();
    v.run
}

fn usage_requests(c: &Ctx, run: u64, task: &str) -> u32 {
    let u = serde_json::to_value(l1::usage(&c.runs, run).unwrap()).unwrap();
    u["tasks"].as_array().unwrap().iter().find(|t| t["task"] == task).map_or(0, |t| t["requests"].as_u64().unwrap() as u32)
}

#[derive(Debug, Clone, Copy)]
enum During {
    AiHelpOff,
    Revoke,
    Cancel,
}

fn act(c: &Ctx, run: u64, task: &str, d: During) {
    match d {
        During::AiHelpOff => {
            c.ai.set_settings(AiSettings { ai_help: false, web_context: false, model: "gpt-5.5".into() }).unwrap();
        }
        During::Revoke => {
            l1::helper_revoke(&c.runs, run, task).unwrap();
        }
        During::Cancel => l1::helper_cancel(&c.runs, run).unwrap(),
    }
}

fn want_code(d: During) -> &'static str {
    match d {
        During::AiHelpOff => "ai_help_off",
        During::Revoke => "no_grant",
        During::Cancel => "cancelled",
    }
}

#[test]
fn an_explanation_retry_is_gated_on_the_wire() {
    for d in [During::AiHelpOff, During::Revoke, During::Cancel] {
        let (atx, arx) = mpsc::channel();
        let (rtx, rrx) = mpsc::channel();
        // The first reply fails validation (a number not in the input), so
        // the command would retry.
        let bad = json_reply(json!({"explanation": "The printed t of 9.87 does not fit."}));
        let good = json_reply(json!({"explanation": "The result was reproduced."}));
        let hold = Arc::new(Holding { arrived: Mutex::new(Some(atx)), release: Mutex::new(Some(rrx)), replies: Mutex::new(VecDeque::from([bad, good])), n: AtomicU32::new(0) });
        let h2 = hold.clone();
        let factory: TransportFactory = Arc::new(move |_| h2.clone() as Arc<dyn HelperTransport>);
        let c = ctx_with(factory, "http://127.0.0.1:9/v1");
        turn_on(&c);
        let run = explanation_ready(&c);
        let p = l1::helper_preview(&c.runs, &c.ai, run, &HelperTaskInput::ResultExplanation { claim_id: 0 }, &[]).unwrap();
        let result = std::thread::scope(|s| {
            let job = s.spawn(|| l1::helper_send(&c.runs, &c.ai, run, p.preview, 1_792_000_000));
            arx.recv_timeout(Duration::from_secs(10)).expect("the first call arrived");
            act(&c, run, "result_explanation", d);
            rtx.send(()).unwrap();
            job.join().unwrap()
        });
        let sent = hold.requests_sent();
        let got = result.as_ref().map(|_| "shown".to_string()).unwrap_or_else(|e| code(&e));
        println!("explanation, {d:?} during the first call: requests={sent} usage={} result={got}", usage_requests(&c, run, "result_explanation"));
        assert_eq!(sent, 1, "{d:?}: exactly one request went out");
        assert_eq!(got, want_code(d), "{d:?}: the gate's closed reason");
        assert_eq!(usage_requests(&c, run, "result_explanation"), 1, "{d:?}: usage counts the one request");
    }
}

/// A localhost server: the n-th connection gets the n-th (status, extra
/// headers, body); `None` stalls past any client timeout. Records each request.
struct Server {
    url: String,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
}

fn server(responses: Vec<Option<(u16, String, String)>>) -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let r = requests.clone();
    std::thread::spawn(move || {
        for (n, conn) in listener.incoming().enumerate() {
            let Ok(mut s) = conn else { return };
            let r = r.clone();
            let reply = responses.get(n).cloned().flatten();
            std::thread::spawn(move || {
                r.lock().unwrap().push(read_request(&mut s));
                match reply {
                    None => std::thread::sleep(Duration::from_millis(1500)),
                    Some((status, headers, body)) => {
                        let resp = format!(
                            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = s.write_all(resp.as_bytes());
                    }
                }
            });
        }
    });
    Server { url, requests }
}

fn real_ctx(srv: &Server, timeout: RequestTimeout) -> Ctx {
    let store = Arc::new(MemoryCredentialStore::default());
    let base = srv.url.clone();
    let s2: Arc<dyn CredentialStore> = store.clone();
    let factory: TransportFactory =
        Arc::new(move |cancel| Arc::new(ResponsesTransport::new(&base, s2.clone(), cancel, Duration::from_millis(10), timeout).unwrap()) as Arc<dyn HelperTransport>);
    let ai = StatsAi::for_tests(store.clone(), factory, &srv.url);
    Ctx { _files: files(), db: Database::in_memory().unwrap(), runs: L1Runs::default(), ai, store }
}

#[test]
fn a_transport_retry_after_a_timeout_is_gated_on_the_wire() {
    for d in [During::AiHelpOff, During::Revoke, During::Cancel] {
        let srv = server(vec![None, None, None]);
        let c = real_ctx(&srv, RequestTimeout::Fixed(Duration::from_millis(400)));
        turn_on(&c);
        let v = l1::open(&c.db, &c.runs, &c.ai, &c._files.manuscript, &c._files.data).expect("open");
        let input = HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: None };
        let p = l1::helper_preview(&c.runs, &c.ai, v.run, &input, &[]).unwrap();
        let result = std::thread::scope(|s| {
            let job = s.spawn(|| l1::helper_send(&c.runs, &c.ai, v.run, p.preview, 1_792_000_000));
            let t0 = Instant::now();
            while srv.requests.lock().unwrap().is_empty() {
                assert!(t0.elapsed() < Duration::from_secs(10), "the first attempt never arrived");
                std::thread::sleep(Duration::from_millis(2));
            }
            act(&c, v.run, "claim_method", d);
            job.join().unwrap()
        });
        std::thread::sleep(Duration::from_millis(100));
        let n = srv.requests.lock().unwrap().len();
        let got = result.as_ref().map(|_| "ok".to_string()).unwrap_or_else(|e| code(&e));
        println!("claim_method timeout, {d:?} during the first attempt: requests={n} usage={} result={got}", usage_requests(&c, v.run, "claim_method"));
        assert_eq!(n, 1, "{d:?}: the retry did not go out");
        assert_eq!(got, want_code(d), "{d:?}");
        assert_eq!(usage_requests(&c, v.run, "claim_method"), 1, "{d:?}");
    }
}

/* ------------------------------------------------------------------------ *
 *  Part C follow-up 2: closing a run mid-send
 * ------------------------------------------------------------------------ */

/// The run is closed (the screen's "close", or eviction) while its first
/// attempt has timed out and the transport is waiting to retry. Closing sets
/// the run's cancel, and the wire gate refuses every later attempt of that
/// run with `RunClosed`: the retry never reaches the server, and the command
/// returns the closed run's code instead of any result.
#[test]
fn closing_a_run_while_its_send_waits_to_retry_sends_nothing_more_and_drops_the_result() {
    let srv = server(vec![None, None, None]);
    let c = real_ctx(&srv, RequestTimeout::Fixed(Duration::from_millis(400)));
    turn_on(&c);
    let v = l1::open(&c.db, &c.runs, &c.ai, &c._files.manuscript, &c._files.data).expect("open");
    let input = HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: None };
    let p = l1::helper_preview(&c.runs, &c.ai, v.run, &input, &[]).unwrap();
    let result = std::thread::scope(|s| {
        let job = s.spawn(|| l1::helper_send(&c.runs, &c.ai, v.run, p.preview, 1_792_000_000));
        let t0 = Instant::now();
        while srv.requests.lock().unwrap().is_empty() {
            assert!(t0.elapsed() < Duration::from_secs(10), "the first attempt never arrived");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(c.runs.close(v.run), "the run was held");
        job.join().unwrap()
    });
    std::thread::sleep(Duration::from_millis(600));
    let n = srv.requests.lock().unwrap().len();
    let got = result.as_ref().map(|_| "shown".to_string()).unwrap_or_else(|e| code(&e));
    println!("claim_method, run closed while waiting to retry: requests={n} result={got}");
    assert_eq!(n, 1, "no attempt after the close reached the server");
    assert_eq!(got, "unknown_run", "the result is dropped as the closed run's");
}

/// A reply that arrives after its run was closed is dropped, never shown;
/// and an explanation whose first reply fails validation does not retry.
#[test]
fn a_reply_that_arrives_after_its_run_is_closed_is_dropped_and_nothing_retries() {
    let good = json_reply(json!({"explanation": "The result was reproduced."}));
    let bad = json_reply(json!({"explanation": "The printed t of 9.87 does not fit."}));
    for (name, replies) in [("a valid reply", vec![good.clone()]), ("a reply that would retry", vec![bad, good])] {
        let (atx, arx) = mpsc::channel();
        let (rtx, rrx) = mpsc::channel();
        let hold = Arc::new(Holding { arrived: Mutex::new(Some(atx)), release: Mutex::new(Some(rrx)), replies: Mutex::new(VecDeque::from(replies)), n: AtomicU32::new(0) });
        let h2 = hold.clone();
        let factory: TransportFactory = Arc::new(move |_| h2.clone() as Arc<dyn HelperTransport>);
        let c = ctx_with(factory, "http://127.0.0.1:9/v1");
        turn_on(&c);
        let run = explanation_ready(&c);
        let p = l1::helper_preview(&c.runs, &c.ai, run, &HelperTaskInput::ResultExplanation { claim_id: 0 }, &[]).unwrap();
        let result = std::thread::scope(|s| {
            let job = s.spawn(|| l1::helper_send(&c.runs, &c.ai, run, p.preview, 1_792_000_000));
            arx.recv_timeout(Duration::from_secs(10)).expect("the first call arrived");
            assert!(c.runs.close(run), "the run was held");
            rtx.send(()).unwrap();
            job.join().unwrap()
        });
        let sent = hold.requests_sent();
        let got = result.as_ref().map(|_| "shown".to_string()).unwrap_or_else(|e| code(&e));
        println!("explanation, {name}, run closed during the first call: requests={sent} result={got}");
        assert_eq!(sent, 1, "{name}: exactly one request went out");
        assert_eq!(got, "unknown_run", "{name}: the result is dropped, not shown");
    }
}

/* ------------------------------------------------------------------------ *
 *  5. Redirects, the key test
 * ------------------------------------------------------------------------ */

#[test]
fn a_redirect_is_a_failed_call_and_nothing_goes_to_its_location() {
    for status in [301u16, 302, 307, 308] {
        let second = server(vec![Some((200, String::new(), json_reply(json!({"method_id": "none", "variant": "not_stated", "reason": "x", "relied_on": []}))))]);
        let first = server(vec![Some((status, format!("location: {}/responses\r\n", second.url), String::new()))]);
        let c = real_ctx(&first, RequestTimeout::Fixed(Duration::from_secs(5)));
        turn_on(&c);
        let v = l1::open(&c.db, &c.runs, &c.ai, &c._files.manuscript, &c._files.data).expect("open");
        let p = l1::helper_preview(&c.runs, &c.ai, v.run, &HelperTaskInput::ClaimMethod { claim_id: 0, methods_passage: None }, &[]).unwrap();
        let r = l1::helper_send(&c.runs, &c.ai, v.run, p.preview, 1_792_000_000);
        std::thread::sleep(Duration::from_millis(100));
        let (a, b) = (first.requests.lock().unwrap().len(), second.requests.lock().unwrap().len());
        let got = r.as_ref().map(|_| "ok".to_string()).unwrap_or_else(|e| code(&e));
        let usage = usage_requests(&c, v.run, "claim_method");
        println!("{status}: first host {a}, second host {b}, result {got}, usage {usage}");
        assert_eq!((a, b), (1, 0), "{status}: one request, none to the Location");
        assert_eq!(got, "redirect_refused", "{status}");
        assert_eq!(usage, 1, "{status}: counted");
    }
}

fn key_result(responses: Vec<Option<(u16, String, String)>>, ai_help: bool) -> (String, usize) {
    let srv = server(responses);
    let c = real_ctx(&srv, RequestTimeout::Fixed(Duration::from_secs(5)));
    turn_on(&c);
    if !ai_help {
        c.ai.set_settings(AiSettings::default()).unwrap();
    }
    let r = serde_json::to_value(l1::key_test(&c.ai)).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let n = srv.requests.lock().unwrap().len();
    (r["result"].as_str().unwrap().to_string(), n)
}

#[test]
fn the_key_test_reports_each_answer_truthfully() {
    let list = json!({"object": "list", "data": [{"id": "gpt-5.5", "object": "model"}]}).to_string();
    let cases: Vec<(&str, Option<(u16, String, String)>, &str)> = vec![
        ("2xx, a model list", Some((200, String::new(), list.clone())), "accepted"),
        ("2xx, not a model list", Some((200, String::new(), "<html>hello</html>".into())), "unexpected_reply"),
        ("2xx, JSON of another shape", Some((200, String::new(), "{\"ok\":true}".into())), "unexpected_reply"),
        ("301", Some((301, "location: https://example.invalid/\r\n".into(), String::new())), "redirect_refused"),
        ("307", Some((307, "location: https://example.invalid/\r\n".into(), String::new())), "redirect_refused"),
        ("401", Some((401, String::new(), "{}".into())), "rejected"),
        ("403", Some((403, String::new(), "{}".into())), "forbidden"),
        ("404", Some((404, String::new(), "{}".into())), "client_error"),
        ("429", Some((429, String::new(), "{}".into())), "client_error"),
        ("500", Some((500, String::new(), "{}".into())), "server_error"),
        ("503", Some((503, String::new(), "{}".into())), "server_error"),
    ];
    let mut wrong = Vec::new();
    for (name, resp, want) in cases {
        let (got, n) = key_result(vec![resp], true);
        println!("key test, {name}: {got} ({n} request)");
        if got != want || n != 1 {
            wrong.push(format!("{name}: want {want} with 1 request, got {got} with {n}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn with_ai_help_off_the_key_test_sends_nothing() {
    let (got, n) = key_result(vec![Some((200, String::new(), "{\"object\":\"list\",\"data\":[]}".into()))], false);
    println!("key test with AI help off: {got}, {n} requests");
    assert_eq!((got.as_str(), n), ("ai_help_off", 0));
}
