//! **Phase 2, Step 8 guards over the app crate and the frontend source.**
//!
//! 1. No test, fixture or screen in this phase holds a fake key shaped like a
//!    real OpenAI key. Fix 1 of Step 8: the settings vitest typed
//!    `sk-test-SECRET`; the phase's sentinel is `KEY_SECRET_a1b2`, which a key
//!    scanner cannot mistake for a real key.
//! 2. In the app crate's Level 1 files, only webview views and two value
//!    types derive `Serialize`. `gaply_core`'s `stats_l1` and `stats_assist`
//!    have their own allowlists (`stats_l1_is_llm_free_and_offline.rs`,
//!    `stats_assist_is_offline_and_inert.rs`).
//! 3. No frontend source names OpenAI's host or the Responses path: only the
//!    app crate's transport talks to OpenAI (T-BYOK), and no proxy route was
//!    added in Phase 2.
//!
//! These scan what was TYPED, in source. The built bundle is checked by the
//! Step 8 report's commands over `build/`, not here.

use std::path::{Path, PathBuf};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");

fn files_under(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let p = e.unwrap().path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if name != "node_modules" && name != "target" && name != "build" {
                files_under(&p, exts, out);
            }
        } else if exts.iter().any(|x| name.ends_with(x)) {
            out.push(p);
        }
    }
}

fn rel(p: &Path) -> String {
    p.strip_prefix(ROOT).unwrap_or(p).display().to_string().replace('\\', "/")
}

/// `s-k-` followed by a key character, spelled so this file does not match
/// itself.
fn key_shaped(text: &str) -> Vec<usize> {
    let needle = ["s", "k", "-"].concat();
    let mut at = Vec::new();
    let mut from = 0;
    while let Some(i) = text[from..].find(&needle) {
        let i = from + i;
        let next = text[i + 3..].chars().next();
        let before = text[..i].chars().next_back();
        let starts_word = !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if starts_word && next.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            at.push(i);
        }
        from = i + 3;
    }
    at
}

#[test]
fn no_phase_two_test_fixture_or_screen_holds_a_key_shaped_like_an_openai_key() {
    let root = Path::new(ROOT);
    let mut files = Vec::new();
    files_under(&root.join("src/screens/statsverifier"), &[".ts", ".tsx", ".json"], &mut files);
    files_under(&root.join("src-tauri/tests"), &[".rs"], &mut files);
    files_under(&root.join("src-tauri/gaply-core/tests"), &[".rs", ".csv", ".txt", ".R"], &mut files);
    for f in ["stats_ai.rs", "stats_l1_commands.rs", "stats_l1_ingest.rs", "stats_reproduction.rs"] {
        files.push(root.join("src-tauri/src").join(f));
    }
    files_under(&root.join("src-tauri/gaply-core/src/stats_assist"), &[".rs", ".json"], &mut files);
    assert!(files.len() > 80, "only {} files scanned: the guard would pass vacuously", files.len());
    let mut sentinel_users = 0;
    let mut hits = Vec::new();
    for f in &files {
        // This file quotes the defect it guards against in its header.
        if f.ends_with("tests/stats_l1_guards.rs") {
            continue;
        }
        let text = std::fs::read_to_string(f).unwrap_or_default();
        if text.contains("KEY_SECRET_a1b2") {
            sentinel_users += 1;
        }
        for at in key_shaped(&text) {
            let line = text[..at].matches('\n').count() + 1;
            // The location only: the match itself is never printed.
            hits.push(format!("{}:{line}", rel(f)));
        }
    }
    // The sentinel is in use where keys are typed: the shared Rust fixtures
    // and the settings vitest.
    assert!(sentinel_users >= 2, "the fake key sentinel is used in only {sentinel_users} file(s)");
    assert!(hits.is_empty(), "a key-shaped string (s-k- and a key character) at:\n{}", hits.join("\n"));
}

#[test]
fn the_key_scan_finds_what_it_is_for() {
    let real_shaped = ["s", "k", "-proj-abc123"].concat();
    assert_eq!(key_shaped(&format!("let k = \"{real_shaped}\";")).len(), 1);
    assert_eq!(key_shaped(&["value: '", "s", "k", "-test-SECRET'"].concat()).len(), 1);
    // A prose mention of the prefix, a word that ends in it, the sentinel.
    assert!(key_shaped(&["does not start with `", "s", "k", "-`"].concat()).is_empty());
    assert!(key_shaped(&["ask", "-a-question"].concat()).is_empty());
    assert!(key_shaped("KEY_SECRET_a1b2").is_empty());
}

/// The name of each type a `#[derive(..Serialize..)]` applies to.
fn serialize_derives(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = false;
    for line in src.lines() {
        let t = line.trim();
        if let Some(list) = t.strip_prefix("#[derive(").and_then(|r| r.strip_suffix(")]")) {
            // `Serialize` or `serde::Serialize` as one item; `Deserialize` is not it.
            if list.split(',').map(|x| x.trim().trim_start_matches("serde::")).any(|x| x == "Serialize") {
                pending = true;
            }
            continue;
        }
        if pending {
            if t.starts_with("#[") || t.starts_with("///") || t.starts_with("//") {
                continue;
            }
            let name = t
                .trim_start_matches("pub(crate) ")
                .trim_start_matches("pub ")
                .trim_start_matches("struct ")
                .trim_start_matches("enum ")
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("")
                .to_string();
            out.push(name);
            pending = false;
        }
    }
    out
}

#[test]
fn in_the_app_crates_level_1_files_only_views_and_two_value_types_serialise() {
    const ALLOWED: [&str; 3] = ["L1Handle", "AiSettings", "TaskUsage"];
    let mut seen = 0;
    for f in ["stats_ai.rs", "stats_l1_commands.rs", "stats_l1_ingest.rs", "stats_reproduction.rs"] {
        let path = Path::new(ROOT).join("src-tauri/src").join(f);
        // The whole file, test modules included: cutting at the first
        // `#[cfg(test)]` hid an item placed after the test module (measured:
        // the mutation check went green). No test module here derives
        // Serialize.
        let src = std::fs::read_to_string(&path).expect("source");
        for name in serialize_derives(&src) {
            seen += 1;
            assert!(
                name.ends_with("View") || ALLOWED.contains(&name.as_str()),
                "{f}: `{name}` derives Serialize. Only webview views (`*View`), the handle, the setting and the usage counts may: \
                 a type that holds rows, cell values, level labels, manuscript text or a key must not."
            );
        }
    }
    // 34 measured on 15 Oct 2026; far fewer means the scan stopped matching.
    assert!(seen >= 30, "only {seen} Serialize derives found: the scan is inert");
}

#[test]
fn the_serialize_scan_finds_what_it_is_for() {
    let src = "#[derive(Debug, Clone, serde::Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct RowsHolder {\n}\n#[derive(Deserialize)]\npub struct Input {}\n";
    assert_eq!(serialize_derives(src), vec!["RowsHolder".to_string()]);
}

/// The body of `fn name` or `pub fn name` declared at column 0 in `src`, up
/// to the next item at column 0.
fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
    let start = [format!("\npub fn {name}("), format!("\nfn {name}(")]
        .iter()
        .find_map(|m| src.find(m.as_str()))
        .map(|i| i + 1)
        .unwrap_or_else(|| panic!("no fn {name}"));
    let rest = &src[start + 1..];
    let end = ["\npub fn ", "\nfn ", "\n#[tauri::command]"].iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
    &src[start..start + 1 + end]
}

/// **The helper path cannot confirm, edit, withdraw or decide.** The attack
/// table (`stats_l1_attacks.rs`) shows that no reply changes the result of a
/// hand-confirmed claim; it cannot show that a reply did not confirm a
/// binding of its own, because a second confirmation does not revoke the
/// first and its token never reaches the webview (measured: a mutation that
/// confirmed every AI binding suggestion inside `helper_send` left all of
/// that table green). So the commands that send and read a helper reply are
/// held, by scan, to calls that make proposals and text only.
#[test]
fn the_helper_commands_make_proposals_and_text_only() {
    let src = std::fs::read_to_string(Path::new(ROOT).join("src-tauri/src/stats_l1_commands.rs")).expect("source");
    const FORBIDDEN: [(&str, &str); 9] = [
        (".confirm(", "a confirmation"),
        ("tokens.insert", "a token handle"),
        ("tokens.remove", "a token handle"),
        (".withdraw(", "a withdrawal"),
        (".edit_claim(", "a claim edit"),
        (".edit_proposal(", "a proposal edit (only the user's form edits one)"),
        ("reproduction =", "a Level 1 result"),
        ("run_stats_reproduction_measured", "a Level 1 computation"),
        ("user_ticked", "a stated variant"),
    ];
    let mut scanned = 0;
    // `helper_send` delegates to `helper_send_in`, which holds the body
    // (decision 96); both are scanned.
    for f in ["helper_preview", "helper_send", "helper_send_in", "transmit", "record_usage", "helper_revoke", "helper_cancel"] {
        let body = fn_body(&src, f);
        scanned += body.lines().count();
        for (needle, what) in FORBIDDEN {
            assert!(!body.contains(needle), "`{f}` contains `{needle}`: {what} must never come from the helper path");
        }
    }
    // helper_send_in alone is over 100 lines; far fewer and the cut is wrong.
    assert!(scanned > 180, "only {scanned} lines scanned: the body cut is wrong");
    // The scan itself: a body with a confirmation is caught.
    assert!(fn_body("\npub fn helper_send(x) {\n    s.confirm(p);\n}\nfn other() {}\n", "helper_send").contains(".confirm("));
    assert!(!fn_body("\nfn transmit() {}\npub fn after() { s.confirm(p); }\n", "transmit").contains(".confirm("));
}

#[test]
fn no_frontend_source_names_openais_host_or_the_responses_path() {
    let mut files = Vec::new();
    files_under(&Path::new(ROOT).join("src"), &[".ts", ".tsx", ".js", ".jsx", ".json"], &mut files);
    assert!(files.len() > 300, "only {} frontend files scanned", files.len());
    let needles = ["api.openai.com", "/v1/responses", "openai.com/v1"];
    let mut hits = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for n in needles {
            if text.contains(n) {
                hits.push(format!("{}: {n}", rel(f)));
            }
        }
    }
    assert!(hits.is_empty(), "the frontend names the OpenAI endpoint:\n{}", hits.join("\n"));
}
