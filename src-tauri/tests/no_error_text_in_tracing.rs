//! **No tracing call logs an error's text (Research OS, decision 42, Phase
//! 4A).** It sits next to `no_path_in_tracing.rs` and uses the same lexer
//! rules (comments removed, string literals kept whole and masked for
//! matching).
//!
//! An error's `Display` or `Debug` is text nobody wrote for a log: a proxy
//! error carries the server's reason, a gate's refusal can quote the model's
//! reply, a serde error quotes the value it could not read, a parser's error
//! can name the sheet or the file. So an error is logged by its closed KIND,
//! under the key `error_kind` (`e.code()` for a `GaplyError`), never as
//! itself. In every `trace!` to `error!`, `event!` and `*span!` call, and in
//! every `#[instrument]` attribute, it is a violation to have:
//! * a field whose key is `error`, `err` or `e` (`error = %e`, `err = ?x`);
//! * a shorthand field `%e`, `?e`, `%err`, `?err`, `%error`, `?error`, or a
//!   bare `e`, `err` or `error`;
//! * a capture `{e}`, `{err}` or `{error}` (with any format spec) in the
//!   message, or a positional format argument whose leading name is one of
//!   those three;
//! * `err` or `err(…)` in an `#[instrument]`, which logs the returned error's
//!   `Display`.
//!
//! **`gaply-core/src` has no exemption** (decision 42: its seven cloud-agent
//! fields log `error_kind = e.code()` now). **`src-tauri/src` has a short
//! list**, by file and message: sites whose error cannot hold user text, a
//! path or model output, each with its reason. They are recorded for a later
//! step, not endorsed. An entry that matches nothing fails, so the list
//! cannot outlive its site.
//!
//! What it cannot see: an error held under another name (`let reason = e;
//! warn!(%reason)`). It constrains what was TYPED. The rendered side for the
//! Statistics paths is the sentinel sweeps (`workspace_privacy.rs`,
//! `stats_desc_privacy_matrix.rs`, `verifier_privacy.rs`).

use std::path::{Path, PathBuf};

const APP_DIR: &str = "src";
const CORE_DIR: &str = "gaply-core/src";

const MACROS: &[&str] = &[
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "event",
    "span",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
];

/// The names an error is held under.
const ERROR_NAMES: &[&str] = &["e", "err", "error"];

/// `src-tauri/src` sites whose error cannot hold user text, a path or model
/// output: (file, the call's message, why). Kept for a later step.
const APP_LISTED: &[(&str, &str, &str)] = &[
    (
        "src/lib.rs",
        "bundled journal snapshot did not load",
        "the bundled public journal seed and the database; no user data",
    ),
    (
        "src/lib.rs",
        "journal ownership reconcile did not run",
        "a database error over public journal rows",
    ),
    (
        "src/lib.rs",
        "journal requirement corrections did not run",
        "a database error over public journal rows",
    ),
    (
        "src/lib.rs",
        "journal misread-row reconcile did not run",
        "a database error over public journal rows",
    ),
    (
        "src/lib.rs",
        "journal article-type reconcile did not run",
        "a database error over public journal rows",
    ),
    (
        "src/lib.rs",
        "fatal: tauri runtime failed",
        "the runtime's own start-up error, before any user input",
    ),
    (
        "src/escalation.rs",
        "evidence_persist failed; verdict withheld",
        "a database error: rusqlite names the table and constraint, never a row's values",
    ),
    (
        "src/escalation.rs",
        "record_escalation failed",
        "a database error, as above",
    ),
    (
        "src/commands.rs",
        "oa fetch: target build failed",
        "a database lookup of stored ids",
    ),
    (
        "src/commands.rs",
        "oa fetch: http client build failed",
        "an HTTP client builder's error; no request yet",
    ),
    (
        "src/commands.rs",
        "oa fetch: paper fetcher build failed",
        "an HTTP client builder's error; no request yet",
    ),
    (
        "src/harness_log.rs",
        "box4: could not serialize comparison record",
        "serde_json's serialisation error names no value",
    ),
    (
        "src/harness_log.rs",
        "box4: could not create record dir",
        "std::io::Error's Display holds no path",
    ),
    (
        "src/harness_log.rs",
        "box4: could not append comparison record",
        "std::io::Error's Display holds no path",
    ),
    (
        "src/harness_log.rs",
        "box4: could not open record file",
        "std::io::Error's Display holds no path",
    ),
    (
        "src/harness_log.rs",
        "box4: could not hash manuscript",
        "std::io::Error's Display holds no path",
    ),
    (
        "src/guidelines.rs",
        "storing provenance failed",
        "a database write of public journal provenance",
    ),
    (
        "src/guidelines.rs",
        "storing requirements failed",
        "a database write of public journal requirements",
    ),
    (
        "src/models/mod.rs",
        "SLM-2: unload client build failed",
        "an HTTP client builder's error; no request yet",
    ),
    (
        "src/stats_ai.rs",
        "stats_ai: retryable failure",
        "TransportError's Debug: closed words and an HTTP status",
    ),
    (
        "src/ai/generative.rs",
        "model registry unreadable; falling back to the bundled model",
        "a database read of the model registry",
    ),
    (
        "src/models/candle_perplexity.rs",
        "perplexity: tensor build failed at {i}: {e}",
        "a candle tensor error: shapes and dtypes",
    ),
    (
        "src/models/candle_perplexity.rs",
        "perplexity: forward failed at {i}: {e}",
        "a candle tensor error: shapes and dtypes",
    ),
    (
        "src/models/candle_perplexity.rs",
        "perplexity: logprob read failed at {i}: {e}",
        "a candle tensor error: shapes and dtypes",
    ),
    (
        "src/models/candle_perplexity.rs",
        "perplexity: batched scoring failed: {e}",
        "a candle tensor error: shapes and dtypes",
    ),
    (
        "src/ai/device.rs",
        "Metal device creation failed, running on CPU: {e}",
        "the GPU driver's error, before any input",
    ),
    (
        "src/ai/job_runner.rs",
        "audit result accepted but its evidence card could not be written",
        "a database write of an evidence card",
    ),
    (
        "src/pipeline.rs",
        "build_checklist failed; empty checklist",
        "a database and embedder query, no network and no model reply",
    ),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has it moved?", dir.display());
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// `code`: comments blanked, strings kept. `masked`: string and char
/// literals' insides blanked too. The same rules as `no_path_in_tracing.rs`.
struct Source {
    code: Vec<char>,
    masked: Vec<char>,
}

fn ident_char(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn lex(src: &str) -> Source {
    let s: Vec<char> = src.chars().collect();
    let mut code = s.clone();
    let mut masked = s.clone();
    let blank = |v: &mut Vec<char>, i: usize| {
        if v[i] != '\n' {
            v[i] = ' ';
        }
    };
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        let next = s.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < s.len() && s[i] != '\n' {
                blank(&mut code, i);
                blank(&mut masked, i);
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < s.len() {
                if s[i] == '/' && s.get(i + 1) == Some(&'*') {
                    depth += 1;
                    for k in i..i + 2 {
                        blank(&mut code, k);
                        blank(&mut masked, k);
                    }
                    i += 2;
                } else if s[i] == '*' && s.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    for k in i..i + 2 {
                        blank(&mut code, k);
                        blank(&mut masked, k);
                    }
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    blank(&mut code, i);
                    blank(&mut masked, i);
                    i += 1;
                }
            }
        } else if c == 'r'
            && (next == Some('"') || next == Some('#'))
            && !ident_char(s.get(i.wrapping_sub(1)).copied())
        {
            let mut j = i + 1;
            let mut hashes = 0;
            while s.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if s.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            j += 1;
            let start = j;
            while j < s.len()
                && !(s[j] == '"' && (0..hashes).all(|h| s.get(j + 1 + h) == Some(&'#')))
            {
                j += 1;
            }
            for k in start..j.min(s.len()) {
                blank(&mut masked, k);
            }
            i = j + 1 + hashes;
        } else if c == '"' {
            let mut j = i + 1;
            while j < s.len() && s[j] != '"' {
                if s[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            for k in i + 1..j.min(s.len()) {
                blank(&mut masked, k);
            }
            i = j + 1;
        } else if c == '\'' {
            if next == Some('\\') {
                let mut j = i + 2;
                while j < s.len() && s[j] != '\'' {
                    j += 1;
                }
                for k in i + 1..j.min(s.len()) {
                    blank(&mut masked, k);
                }
                i = j + 1;
            } else if s.get(i + 2) == Some(&'\'') {
                blank(&mut masked, i + 1);
                i += 3;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    Source { code, masked }
}

fn line_of(s: &[char], at: usize) -> usize {
    s[..at.min(s.len())].iter().filter(|&&c| c == '\n').count() + 1
}

fn matching(m: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, &ch) in m.iter().enumerate().skip(open) {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_commas(m: &[char], from: usize, to: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, from);
    for i in from..to {
        match m[i] {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push((start, i));
                start = i + 1;
            }
            _ => {}
        }
    }
    if m[start..to].iter().any(|c| !c.is_whitespace()) {
        out.push((start, to));
    }
    out
}

fn text(v: &[char], (a, b): (usize, usize)) -> String {
    v[a..b].iter().collect::<String>().trim().to_string()
}

/// The leading name of an expression: `e` of `e.to_string()`, `err` of `&err`.
fn leading_name(t: &str) -> String {
    t.trim_start_matches(['&', '*', '(', ' '])
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

fn is_literal(t: &str) -> bool {
    t.starts_with('"') || t.starts_with("r\"") || t.starts_with("r#")
}

/// The literal's text without its quotes.
fn literal_text(t: &str) -> String {
    let t = t.trim_start_matches('r').trim_matches('#');
    t.strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(t)
        .to_string()
}

/// `{e}`, `{err:?}`, `{error:#}` in a format string.
fn captures_error(format: &str) -> bool {
    let mut rest = format;
    while let Some(i) = rest.find('{') {
        let after = &rest[i + 1..];
        if let Some(end) = after.find('}') {
            let name = after[..end].split(':').next().unwrap_or("").trim();
            if ERROR_NAMES.contains(&name) {
                return true;
            }
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    false
}

#[derive(Debug)]
struct Site {
    file: String,
    line: usize,
    message: String,
    why: String,
}

#[derive(Default)]
struct Scan {
    macros: usize,
    attributes: usize,
    sites: Vec<Site>,
}

fn scan_source(file: &str, src: &str, scan: &mut Scan) {
    let Source { code, masked } = lex(src);
    let n = masked.len();
    let mut i = 0;
    while i < n {
        if !ident_char(masked.get(i).copied())
            || ident_char(i.checked_sub(1).and_then(|k| masked.get(k)).copied())
        {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < n && ident_char(Some(masked[j])) {
            j += 1;
        }
        let word: String = masked[i..j].iter().collect();
        // `name!(…)`
        let mut k = j;
        let bang = k < n && masked[k] == '!';
        if bang {
            k += 1;
            while k < n && masked[k].is_whitespace() {
                k += 1;
            }
        }
        if bang && k < n && masked[k] == '(' && MACROS.contains(&word.as_str()) {
            let Some(close) = matching(&masked, k) else {
                i = j;
                continue;
            };
            scan.macros += 1;
            let pieces = split_commas(&masked, k + 1, close);
            let msg_at = pieces.iter().position(|p| is_literal(&text(&code, *p)));
            let message = msg_at
                .map(|m| literal_text(&text(&code, pieces[m])))
                .unwrap_or_default();
            for (idx, p) in pieces.iter().enumerate() {
                let piece = text(&code, *p);
                let masked_piece = text(&masked, *p);
                let why = if Some(idx) == msg_at {
                    captures_error(&message)
                        .then(|| "a capture of the error in the message".to_string())
                } else if msg_at.is_some_and(|m| idx > m) {
                    // A positional format argument.
                    ERROR_NAMES
                        .contains(&leading_name(&piece).as_str())
                        .then(|| format!("a format argument `{piece}`"))
                } else if let Some(eq) = masked_piece
                    .find('=')
                    .filter(|&q| !masked_piece[q..].starts_with("=="))
                {
                    let key = masked_piece[..eq].trim().trim_matches('"').to_string();
                    let key_last = key.rsplit('.').next().unwrap_or("").to_string();
                    ERROR_NAMES
                        .contains(&key_last.as_str())
                        .then(|| format!("the field `{piece}`"))
                } else {
                    let bare = piece.trim_start_matches(['%', '?']);
                    (ERROR_NAMES.contains(&bare) && !piece.contains(':'))
                        .then(|| format!("the field `{piece}`"))
                };
                if let Some(why) = why {
                    scan.sites.push(Site {
                        file: file.to_string(),
                        line: line_of(&code, i),
                        message: message.clone(),
                        why,
                    });
                }
            }
            i = close;
            continue;
        }
        // `#[instrument(…)]` or `#[tracing::instrument(…)]`
        if word == "instrument" && j < n && masked[j] == '(' {
            let before: String = masked[i.saturating_sub(12)..i].iter().collect();
            if before.contains("#[") || before.contains("tracing::") {
                if let Some(close) = matching(&masked, j) {
                    scan.attributes += 1;
                    for p in split_commas(&masked, j + 1, close) {
                        let t = text(&masked, p);
                        if t == "err" || t.starts_with("err(") {
                            scan.sites.push(Site {
                                file: file.to_string(),
                                line: line_of(&code, i),
                                message: String::new(),
                                why: format!("`{t}` in #[instrument] logs the returned error"),
                            });
                        }
                    }
                    i = close;
                    continue;
                }
            }
        }
        i = j;
    }
}

fn scan_dir(dir: &str) -> Scan {
    let base = root().join(dir);
    let mut scan = Scan::default();
    for f in rust_files(&base) {
        let rel = format!("{dir}/{}", f.strip_prefix(&base).unwrap().display()).replace('\\', "/");
        scan_source(&rel, &std::fs::read_to_string(&f).unwrap(), &mut scan);
    }
    scan
}

fn report(sites: &[&Site]) -> String {
    sites
        .iter()
        .map(|s| {
            format!(
                "{}:{}: {} (message: {:?})",
                s.file, s.line, s.why, s.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_tracing_call_in_gaply_core_logs_an_errors_text() {
    let scan = scan_dir(CORE_DIR);
    // Measured 18 Oct 2026: 35 calls and 14 attributes (grep agrees).
    assert!(
        scan.macros >= 30,
        "only {} tracing calls found in {CORE_DIR} — the scan is inert",
        scan.macros
    );
    assert!(
        scan.attributes > 10,
        "only {} #[instrument]s found in {CORE_DIR}",
        scan.attributes
    );
    let all: Vec<&Site> = scan.sites.iter().collect();
    assert!(
        all.is_empty(),
        "log the error's kind (`error_kind = e.code()`), never its text:\n{}",
        report(&all)
    );
}

#[test]
fn no_tracing_call_in_the_app_crate_logs_an_errors_text_outside_the_list() {
    let scan = scan_dir(APP_DIR);
    // Measured 18 Oct 2026: 140 calls and 120 attributes (grep agrees).
    assert!(
        scan.macros >= 120,
        "only {} tracing calls found in {APP_DIR} — the scan is inert",
        scan.macros
    );
    assert!(
        scan.attributes > 100,
        "only {} #[instrument]s found in {APP_DIR}",
        scan.attributes
    );
    let listed = |s: &Site| {
        APP_LISTED
            .iter()
            .any(|(f, m, _)| s.file == *f && s.message == *m)
    };
    let unlisted: Vec<&Site> = scan.sites.iter().filter(|s| !listed(s)).collect();
    // Each entry of the list matches exactly one site, or it is stale.
    let mut stale = Vec::new();
    for (f, m, _) in APP_LISTED {
        let hits = scan
            .sites
            .iter()
            .filter(|s| s.file == *f && s.message == *m)
            .count();
        if hits != 1 {
            stale.push(format!("{f}: {m:?} matches {hits} sites"));
        }
    }
    println!(
        "listed sites in {APP_DIR}: {} (each one line in APP_LISTED)",
        APP_LISTED.len()
    );
    assert!(
        unlisted.is_empty(),
        "log the error's kind, never its text:\n{}",
        report(&unlisted)
    );
    assert!(stale.is_empty(), "the list is stale:\n{}", stale.join("\n"));
}

/// Each rule on a snippet that must fail, and the fixed forms that must pass.
#[test]
fn the_guard_catches_each_way_of_logging_an_error_and_passes_its_kind() {
    let caught = |src: &str| {
        let mut s = Scan::default();
        scan_source("snippet.rs", src, &mut s);
        s.sites.len()
    };
    for bad in [
        "fn f() { tracing::warn!(error = %e, \"cloud call failed\"); }",
        "fn f() { tracing::warn!(\n    batch,\n    error = ?e,\n    \"aborted\"\n); }",
        "fn f() { warn!(err = %x, \"x\"); }",
        "fn f() { tracing::info!(%e, \"x\"); }",
        "fn f() { tracing::info!(?err, \"x\"); }",
        "fn f() { tracing::warn!(\"failed: {e}\"); }",
        "fn f() { tracing::warn!(\"failed: {err:?}\"); }",
        "fn f() { tracing::warn!(\"failed: {}\", e); }",
        "fn f() { tracing::warn!(\"failed: {}\", e.to_string()); }",
        "fn f() { let _ = r.inspect_err(|e| tracing::warn!(error = %e, \"x\")); }",
        "#[tracing::instrument(skip_all, err)]\nfn f() {}",
        "#[instrument(err(Display))]\nfn f() {}",
    ] {
        assert_eq!(caught(bad), 1, "missed: {bad}");
    }
    for good in [
        "fn f() { tracing::warn!(error_kind = e.code(), \"cloud call failed\"); }",
        "fn f() { tracing::warn!(error_kind = ?e.classify(), \"x\"); }",
        "fn f() { tracing::info!(run, code = c.wire(), \"refused\"); }",
        "fn f() { tracing::warn!(\"error = %e is never logged\"); }",
        "// tracing::warn!(error = %e, \"commented out\");\nfn f() {}",
        "fn f() { tracing::info!(errors = n, \"counted\"); }",
        "#[tracing::instrument(skip_all)]\nfn f() {}",
    ] {
        assert_eq!(caught(good), 0, "flagged: {good}");
    }
}
