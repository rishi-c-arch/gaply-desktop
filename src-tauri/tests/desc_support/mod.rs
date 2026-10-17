//! Shared by the descriptive summary's command tests (Research OS, Phase
//! 2B): temporary folders the test creates and removes, a workbook built in
//! the test, and an app over Tauri's mock runtime with the eight
//! `stats_desc_*` commands, so a test calls them through IPC with the
//! webview's argument names and reads the JSON the webview reads.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

use app_lib::runs::Runs;
use app_lib::state::AppState;
use app_lib::stats_desc_commands as desc;
use gaply_core::{AppConfig, Database};

/* ---- folders ---------------------------------------------------------------- */

static DIRS: AtomicUsize = AtomicUsize::new(0);

/// A folder under the OS temp dir, removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let n = DIRS.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("gaply-desc-{}-{n}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("temp dir");
        TempDir(p)
    }
    pub fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).expect("write fixture");
        p
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/* ---- tables ----------------------------------------------------------------- */

/// A cell of a table written in the test.
#[derive(Clone, Copy)]
pub enum C<'a> {
    N(&'a str),
    T(&'a str),
    Empty,
}

impl C<'_> {
    fn text(&self) -> &str {
        match self {
            C::N(s) | C::T(s) => s,
            C::Empty => "",
        }
    }
}

/// The table as delimited text: no quoting is needed for these fixtures.
pub fn delimited(headers: &[&str], rows: &[Vec<C>], sep: &str, eol: &str) -> Vec<u8> {
    let mut out = headers.join(sep);
    out.push_str(eol);
    for r in rows {
        out.push_str(&r.iter().map(C::text).collect::<Vec<_>>().join(sep));
        out.push_str(eol);
    }
    out.into_bytes()
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A zip of stored (uncompressed) parts.
fn stored_zip(parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        let offset = out.len() as u32;
        let (crc, size) = (crc32(data), data.len() as u32);
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0]);
        for v in [crc, size, size] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend_from_slice(&[
            0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0x21, 0,
        ]);
        for v in [crc, size, size] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0; 12]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
    out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn column_letter(i: usize) -> String {
    let mut n = i + 1;
    let mut s = String::new();
    while n > 0 {
        let r = (n - 1) % 26;
        s.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    s
}

/// The table as a minimal .xlsx: one sheet, numbers as numbers, text as
/// inline strings, an empty cell left out.
pub fn xlsx(headers: &[&str], rows: &[Vec<C>]) -> Vec<u8> {
    let cell = |r: usize, c: usize, v: C| -> String {
        let at = format!("{}{}", column_letter(c), r + 1);
        match v {
            C::N(s) => format!(r#"<c r="{at}"><v>{s}</v></c>"#),
            C::T(s) => format!(r#"<c r="{at}" t="inlineStr"><is><t>{s}</t></is></c>"#),
            C::Empty => String::new(),
        }
    };
    let mut sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    let all: Vec<Vec<C>> = std::iter::once(headers.iter().map(|h| C::T(h)).collect())
        .chain(rows.iter().cloned())
        .collect();
    for (r, row) in all.iter().enumerate() {
        sheet.push_str(&format!(r#"<row r="{}">"#, r + 1));
        for (c, v) in row.iter().enumerate() {
            sheet.push_str(&cell(r, c, *v));
        }
        sheet.push_str("</row>");
    }
    sheet.push_str("</sheetData></worksheet>");
    let parts = [
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.to_vec(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_vec(),
        ),
        (
            "xl/workbook.xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_vec(),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.to_vec(),
        ),
        ("xl/worksheets/sheet1.xml", sheet.into_bytes()),
    ];
    stored_zip(&parts)
}

/* ---- the app ---------------------------------------------------------------- */

/// An app over the mock runtime, its state built by `AppState::new` on
/// `app_data` and `db`, with `runs` swapped in when given.
pub fn app(
    app_data: &Path,
    db: Arc<Database>,
    runs: Option<Arc<Runs>>,
) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let mut state = AppState::new(
        AppConfig::new(app_data.join("gaply.db").to_string_lossy().as_ref()),
        db.clone(),
        db,
        Arc::new(gaply_core::embed::HashEmbedder),
        app_data.to_path_buf(),
    );
    if let Some(r) = runs {
        state.runs = r;
    }
    let app = mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            desc::stats_desc_open,
            desc::stats_desc_validate,
            desc::stats_desc_run,
            desc::stats_desc_status,
            desc::stats_desc_result,
            desc::stats_desc_cancel,
            desc::stats_desc_export,
            desc::stats_desc_close,
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let n = DIRS.fetch_add(1, Ordering::SeqCst);
    let webview = WebviewWindowBuilder::new(&app, format!("w{n}"), Default::default())
        .build()
        .expect("webview");
    (app, webview)
}

/// One IPC call. `Ok` is the view's JSON; `Err` is the refusal's JSON, as the
/// webview receives it (`{"code":"validation","message":"…"}`).
pub fn call(webview: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|r| r.deserialize::<Value>().expect("a JSON view"))
}

/// The closed code inside a refusal, with its parameters.
pub fn refusal(err: &Value) -> Value {
    assert_eq!(
        err["code"], "validation",
        "a refusal is a validation error: {err}"
    );
    serde_json::from_str(err["message"].as_str().expect("a message"))
        .expect("the message is a closed code in JSON")
}

/// A spec for `stats.describe` on the opened dataset: the input as the open
/// view gave it, the columns as (alias, kind) pairs.
pub fn spec(
    open: &Value,
    operation: u64,
    columns: &[(&str, &str)],
    missing_codes: &[f64],
) -> Value {
    json!({
        "schemaVersion": 1,
        "operationId": operation,
        "workspaceId": "statistics",
        "capabilityId": "stats.describe",
        "capabilityVersion": 1,
        "inputs": [open["inputRef"]],
        "parameters": {
            "columns": columns.iter().map(|(a, t)| json!({"alias": a, "treatAs": t})).collect::<Vec<_>>(),
            "missingCodes": missing_codes,
        },
        "randomSeed": null,
    })
}

/// Polls `stats_desc_status` until the run is terminal, for at most 10 s.
pub fn until_terminal(webview: &WebviewWindow<MockRuntime>, run_id: u64) -> Value {
    for _ in 0..1000 {
        let s = call(webview, "stats_desc_status", json!({ "runId": run_id })).expect("status");
        if s["terminal"] == true {
            return s;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("run {run_id} did not end within 10 s");
}
