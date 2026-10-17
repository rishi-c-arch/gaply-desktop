//! **No external link opens around the app's own recheck** (Phase 2
//! evaluation fixes, part C follow-up, item 7; the deep evaluation's Minor
//! "the opener plugin can bypass the Rust recheck").
//!
//! Until this step the opener plugin was registered with `init()` and the
//! window held `opener:default`. That set allows `open_url` for every
//! `http(s)`, `mailto` and `tel` URL, and the plugin injects a click handler
//! (`init-iife.js`) that opens any `<a target="_blank">` through it. So any
//! script in the webview could open any URL with no check in Rust, around
//! `stats_open_source_link`'s allowlist.
//!
//! Now: the window may only reveal a file in its folder; the plugin's click
//! handler is off; links go through `open_external_link` (https only, no
//! credentials, bounded), and Level 1's model-supplied sources keep
//! `stats_open_source_link` with its domain list. These scans pin all three,
//! over what was TYPED: the capability files, `lib.rs`, and the frontend.

use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");

/// The only opener permission the webview keeps (`DocumentRow`'s "show in
/// folder").
const OPENER_ALLOWED: &[&str] = &["opener:allow-reveal-item-in-dir"];

fn strings_in(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(a) => a.iter().for_each(|x| strings_in(x, out)),
        serde_json::Value::Object(m) => m.values().for_each(|x| strings_in(x, out)),
        _ => {}
    }
}

#[test]
fn the_webview_holds_no_permission_to_open_a_url() {
    let dir = Path::new(ROOT).join("capabilities");
    let mut files = 0;
    let mut permissions = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "json") {
            files += 1;
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            strings_in(&v["permissions"], &mut permissions);
        }
    }
    assert!(
        files >= 1 && permissions.len() >= 5,
        "{files} capability files, {} permission strings: the scan is inert",
        permissions.len()
    );
    let opener: Vec<&String> = permissions
        .iter()
        .filter(|p| p.starts_with("opener:"))
        .collect();
    let extra: Vec<&&String> = opener
        .iter()
        .filter(|p| !OPENER_ALLOWED.contains(&p.as_str()))
        .collect();
    assert!(
        extra.is_empty(),
        "the webview may open URLs without the Rust recheck: {extra:?}"
    );
}

#[test]
fn the_plugins_click_handler_is_off_and_the_checked_command_is_registered() {
    let lib = std::fs::read_to_string(Path::new(ROOT).join("src/lib.rs")).unwrap();
    let code: String = lib
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("tauri_plugin_opener::init()"),
        "init() injects the plugin's own click handler"
    );
    assert!(
        code.contains("tauri_plugin_opener::Builder::new().open_js_links_on_click(false).build()"),
        "the opener must be built with its click handler off"
    );
    assert!(
        code.contains("external_links::open_external_link"),
        "open_external_link is not registered"
    );
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if p.is_dir() {
            files_under(&p, out);
        } else if (name.ends_with(".ts") || name.ends_with(".tsx"))
            && !name.contains(".vitest.")
            && !name.contains(".test.")
        {
            out.push(p);
        }
    }
}

#[test]
fn no_frontend_code_opens_a_url_through_the_plugin() {
    let mut files = Vec::new();
    files_under(&Path::new(ROOT).join("../src"), &mut files);
    assert!(
        files.len() > 100,
        "only {} frontend files: the scan is inert",
        files.len()
    );
    let mut bad = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for (i, line) in text.lines().enumerate() {
            // What is CALLED, not what a comment explains was removed.
            let t = line.trim_start();
            if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
                continue;
            }
            if line.contains("plugin:opener|open_url") || line.contains("openUrl") {
                bad.push(format!(
                    "{}:{}",
                    f.strip_prefix(ROOT).unwrap_or(f).display(),
                    i + 1
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "open links through `openExternal` (src/services/externalLinks.ts), never the plugin:\n{}",
        bad.join("\n")
    );
}
