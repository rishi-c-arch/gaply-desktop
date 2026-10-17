//! Fixtures shared by the Level 1 command tests: real files in a folder named
//! `PATH_SECRET_f61` with `FILE_SECRET_9a3` in their names, a world with the
//! mock transport and the in-memory key store, and a tracing capture.
#![allow(dead_code)]

use std::fmt::Write as _;
use std::io::{Read, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use app_lib::stats_ai::{
    AiSettings, CredentialStore, MemoryCredentialStore, RequestTimeout, ResponsesTransport,
    StatsAi, TransportFactory,
};
use app_lib::stats_l1_commands::{
    self as l1, BindingEdits, L1OpenView, L1Runs, LevelEdit, RoleEdit,
};
use gaply_core::stats_assist::transport::{HelperTransport, MockTransport};
use gaply_core::stats_l0::pvalue;
use gaply_core::{Database, GaplyError};

pub const CELL: &str = "CELL_SECRET_7d9";
pub const LEVEL: &str = "LEVEL_SECRET_b18";
pub const HEADER: &str = "HEADER_SECRET_c42";
pub const MS: &str = "MS_SECRET_e55";
pub const PATH: &str = "PATH_SECRET_f61";
pub const FILE: &str = "FILE_SECRET_9a3";
pub const KEY: &str = "KEY_SECRET_a1b2";

pub const TREATED: [f64; 4] = [12.1, 14.3, 11.8, 13.6];
pub const CONTROL: [f64; 4] = [15.2, 16.9, 17.4, 16.0];

/* ---- files ---------------------------------------------------------------- */

pub static DIRS: AtomicUsize = AtomicUsize::new(0);

pub struct Files {
    pub root: PathBuf,
    pub manuscript: PathBuf,
    pub data: PathBuf,
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn printed() -> (String, String, String) {
    let t = gaply_core::stats_verify::t_test_student(&TREATED, &CONTROL).unwrap();
    let p = pvalue::t_two_tailed(t.t, t.df).unwrap();
    (
        format!("{:.2}", t.t.abs()),
        format!("{}", t.df),
        format!("{:.3}", p).trim_start_matches('0').to_string(),
    )
}

pub fn csv(treated: &[f64]) -> String {
    let mut s = format!("{HEADER},arm,note,solo\n");
    for (i, v) in treated.iter().enumerate() {
        let note = if i % 2 == 0 { CELL } else { "fine" };
        writeln!(s, "{v},{LEVEL},{note},{LEVEL}_solo").unwrap();
    }
    for v in CONTROL {
        writeln!(s, "{v},ctrl,{CELL},{LEVEL}_solo").unwrap();
    }
    s
}

pub fn files() -> Files {
    let n = DIRS.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("gaply-l1-cmd-{}-{n}", std::process::id()));
    let dir = root.join(PATH);
    std::fs::create_dir_all(&dir).unwrap();
    let (t, df, p) = printed();
    let manuscript = dir.join(format!("{FILE}_manuscript.txt"));
    std::fs::write(
        &manuscript,
        format!(
            "A Synthetic Study\n\nMethods\n\nParticipants were assigned to two arms at random. Groups were compared with a two-sample t-test assuming equal variances.\n\nResults\n\nBaseline readings of the cohort were stable across the whole study period and {MS} did not move. The treated arm scored lower than control, t({df}) = {t}, p = {p}.\n"
        ),
    )
    .unwrap();
    let data = dir.join(format!("{FILE}_data.csv"));
    std::fs::write(&data, csv(&TREATED)).unwrap();
    Files {
        root,
        manuscript,
        data,
    }
}

/* ---- the context ---------------------------------------------------------- */

pub struct World {
    pub files: Files,
    pub db: Database,
    pub runs: L1Runs,
    pub ai: StatsAi,
    pub store: Arc<MemoryCredentialStore>,
    pub mock: Arc<MockTransport>,
}

pub fn world_with(mock: MockTransport) -> World {
    let mock = Arc::new(mock);
    let m = mock.clone();
    let factory: TransportFactory = Arc::new(move |_| m.clone() as Arc<dyn HelperTransport>);
    let store = Arc::new(MemoryCredentialStore::default());
    let ai = StatsAi::for_tests(store.clone(), factory, "http://127.0.0.1:9/v1");
    World {
        files: files(),
        db: Database::in_memory().unwrap(),
        runs: L1Runs::default(),
        ai,
        store,
        mock,
    }
}

pub fn ai_on(w: &World, web: bool) {
    w.ai.set_settings(AiSettings {
        ai_help: true,
        web_context: web,
        model: "gpt-5.5".into(),
    })
    .unwrap();
    w.store.save_key(KEY).unwrap();
}

pub fn open(w: &World) -> L1OpenView {
    l1::open(&w.db, &w.runs, &w.ai, &w.files.manuscript, &w.files.data).expect("open")
}

pub fn envelope(text: &str, annotations: Value) -> String {
    json!({
        "id": "resp_mock", "status": "completed",
        "output": [{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": text, "annotations": annotations}]}],
        "usage": {"input_tokens": 300, "output_tokens": 40}
    })
    .to_string()
}

pub fn json_reply(v: Value) -> String {
    envelope(&v.to_string(), json!([]))
}

pub fn manual_edits() -> BindingEdits {
    BindingEdits {
        method_id: "independent_t".into(),
        roles: vec![
            RoleEdit {
                role: "outcome".into(),
                aliases: vec![1],
            },
            RoleEdit {
                role: "group".into(),
                aliases: vec![2],
            },
        ],
        levels: vec![LevelEdit {
            role: "group".into(),
            levels: vec![LEVEL.into(), "ctrl".into()],
        }],
        filters: vec![],
        variant_ticked: None,
        ols_overall_f: None,
        ols_predictor: None,
        exact_whole_numbers: vec![],
    }
}

/// Confirm claim 0 by hand, from the first proposal held.
pub fn confirm_by_hand(w: &World, run: u64) -> String {
    let proposals = l1::proposals(&w.runs, run).unwrap();
    let first = proposals
        .first()
        .map(|p| p.proposal)
        .expect("a local proposal");
    l1::confirm_binding(&w.runs, run, first, Some(&manual_edits()))
        .expect("confirmed")
        .token
}

pub fn code(e: &GaplyError) -> String {
    match e {
        GaplyError::Validation(m) => serde_json::from_str::<Value>(m)
            .map(|v| v["code"].as_str().unwrap_or("").to_string())
            .unwrap_or_default(),
        other => format!("not a validation error: {other:?}"),
    }
}

pub fn binding_reply() -> String {
    json_reply(
        json!({"roles": [{"role": "outcome", "alias": "C1", "levels_to_select": null}, {"role": "group", "alias": "C2", "levels_to_select": 2}], "reason": "C1 is numeric; C2 has two levels.", "relied_on": ["schema"]}),
    )
}

/* ---- a tracing capture -------------------------------------------------- */

pub struct Capture {
    pub lines: Arc<Mutex<Vec<String>>>,
}

pub struct Visitor(pub String);

impl tracing::field::Visit for Visitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.0, "{}={:?} ", field.name(), value);
    }
}

impl tracing::Subscriber for Capture {
    /// `sometimes`, never a cached `always`/`never`: tests run on parallel
    /// threads, and a callsite first hit where no subscriber is set would
    /// otherwise be cached as `never` and skip this capture.
    fn register_callsite(
        &self,
        _: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let mut v = Visitor(format!("span {} ", attrs.metadata().name()));
        attrs.record(&mut v);
        self.lines.lock().unwrap().push(v.0);
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, values: &tracing::span::Record<'_>) {
        let mut v = Visitor(String::from("record "));
        values.record(&mut v);
        self.lines.lock().unwrap().push(v.0);
    }
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut v = Visitor(format!("event {} ", event.metadata().target()));
        event.record(&mut v);
        self.lines.lock().unwrap().push(v.0);
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

/* ---- the real transport against a capture server -------------------------- */

/// One canned response per accepted connection; every raw request recorded.
pub struct CaptureServer {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Vec<u8>>>>,
}

pub fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).to_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .map(|v| v.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            if buf.len() >= end + 4 + len {
                break;
            }
        }
    }
    buf
}

/// A status that means "stall": read the request, hold the connection past
/// any client timeout, and close it without a reply.
pub const STALL: u16 = 0;

pub fn capture_server(responses: Vec<(u16, String)>) -> CaptureServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let r = requests.clone();
    std::thread::spawn(move || {
        // Each connection on its own thread, so a stall never holds up the
        // next attempt; the n-th connection gets the n-th response.
        for (n, conn) in listener.incoming().enumerate() {
            let Ok(mut s) = conn else { return };
            let r = r.clone();
            let reply = responses.get(n).cloned();
            std::thread::spawn(move || {
                r.lock().unwrap().push(read_request(&mut s));
                match reply {
                    Some((STALL, _)) => std::thread::sleep(Duration::from_millis(1500)),
                    Some((status, body)) => {
                        let reason = if status == 200 { "OK" } else { "Error" };
                        let resp = format!(
                            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\nx-request-id: req_capture\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = s.write_all(resp.as_bytes());
                    }
                    None => {}
                }
            });
        }
    });
    CaptureServer { url, requests }
}

pub fn world_with_real_transport(
    server: &CaptureServer,
    cancel_flag: Option<Arc<AtomicBool>>,
    timeout: RequestTimeout,
) -> World {
    let store = Arc::new(MemoryCredentialStore::default());
    let base = server.url.clone();
    let s2: Arc<dyn CredentialStore> = store.clone();
    let factory: TransportFactory = Arc::new(move |cancel| {
        let cancel = cancel_flag.clone().unwrap_or(cancel);
        Arc::new(
            ResponsesTransport::new(
                &base,
                s2.clone(),
                cancel,
                Duration::from_millis(10),
                timeout,
            )
            .unwrap(),
        ) as Arc<dyn HelperTransport>
    });
    let ai = StatsAi::for_tests(store.clone(), factory, &server.url);
    World {
        files: files(),
        db: Database::in_memory().unwrap(),
        runs: L1Runs::default(),
        ai,
        store,
        mock: Arc::new(MockTransport::new()),
    }
}

pub fn split(raw: &[u8]) -> (String, Vec<(String, String)>, Vec<u8>) {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a head");
    let head = String::from_utf8(raw[..end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap().to_string();
    let headers = lines.map(|l| {
        let (k, v) = l.split_once(':').unwrap();
        (k.trim().to_lowercase(), v.trim().to_string())
    });
    (request_line, headers.collect(), raw[end + 4..].to_vec())
}
