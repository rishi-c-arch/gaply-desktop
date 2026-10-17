//! **Every ignored test in the workspace is listed, and an ignored test that
//! guards precision, privacy, provenance or cancellation carries a waiver
//! (Research OS, Prompt 0; Phase 2B).**
//!
//! An ignored test is not a pass. One that guards precision, privacy,
//! provenance or cancellation may be ignored only with an issue, an owner and
//! a review date, written into its reason string as `issue=…`, `owner=…` and
//! `review=YYYY-MM-DD`, so the waiver is in the one place a reader of the
//! test, a reader of `cargo test`'s output and this guard all see.
//!
//! What it scans: every `.rs` file of both workspace members (the app crate's
//! `src/`, `tests/` and `examples/`, and `gaply-core`'s), comments removed,
//! for every attribute that ignores a test: `#[ignore]`, `#[ignore = "…"]`,
//! and `#[cfg_attr(…, ignore …)]` (the app's Windows skips), however many
//! lines each spans. A test is in scope when its function name or its file's
//! path mentions `precision`, `privacy`, `provenance` or `cancel`, in any
//! case. The waiver is required of an unconditional `#[ignore]`, a test that
//! runs nowhere; a `cfg_attr(<platform>, ignore)` runs on the other
//! platforms and is listed with its condition.
//!
//! It runs from `gaply-core` so that the clean-checkout (Linux) and Windows
//! jobs run it; it reads the app crate's files from the checkout beside it.
//! It prints the full list, and it requires a positive count, so a matcher
//! that stops matching fails rather than passing on nothing.

use std::path::{Path, PathBuf};

const IN_SCOPE: &[&str] = &["precision", "privacy", "provenance", "cancel"];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("gaply-core sits inside the workspace")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

fn scanned_files() -> Vec<PathBuf> {
    let root = workspace_root();
    let mut out = Vec::new();
    for dir in [
        "src",
        "tests",
        "examples",
        "gaply-core/src",
        "gaply-core/tests",
        "gaply-core/examples",
    ] {
        rust_files(&root.join(dir), &mut out);
    }
    out.sort();
    out
}

/// The source with comments blanked (strings kept), and a copy with the
/// inside of every string literal blanked too, for finding attributes.
fn without_comments(src: &str) -> (Vec<char>, Vec<char>) {
    let s: Vec<char> = src.chars().collect();
    let mut out = s.clone();
    let mut masked = s.clone();
    let mut i = 0;
    while i < s.len() {
        match (s[i], s.get(i + 1).copied()) {
            ('/', Some('/')) => {
                while i < s.len() && s[i] != '\n' {
                    out[i] = ' ';
                    i += 1;
                }
            }
            ('/', Some('*')) => {
                let mut depth = 0;
                while i < s.len() {
                    if s[i] == '/' && s.get(i + 1) == Some(&'*') {
                        depth += 1;
                        out[i] = ' ';
                        out[i + 1] = ' ';
                        i += 2;
                    } else if s[i] == '*' && s.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        out[i] = ' ';
                        out[i + 1] = ' ';
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        if s[i] != '\n' {
                            out[i] = ' ';
                        }
                        i += 1;
                    }
                }
            }
            ('"', _) => {
                i += 1;
                while i < s.len() && s[i] != '"' {
                    if s[i] == '\\' {
                        masked[i] = ' ';
                        i += 1;
                    }
                    if i < s.len() && s[i] != '\n' {
                        masked[i] = ' ';
                    }
                    i += 1;
                }
                i += 1;
            }
            ('\'', Some('\\')) => {
                i += 2;
                while i < s.len() && s[i] != '\'' {
                    i += 1;
                }
                i += 1;
            }
            ('\'', _) if s.get(i + 2) == Some(&'\'') => i += 3,
            _ => i += 1,
        }
    }
    for (k, ch) in out.iter().enumerate() {
        if *ch == ' ' {
            masked[k] = ' ';
        }
    }
    (out, masked)
}

/// The index of the `]` closing the `[` at `open`, skipping string literals.
fn closing_bracket(c: &[char], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut i = open;
    while i < c.len() {
        match c[i] {
            '"' => {
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The text of the first string literal in `t`, with `\`-newline
/// continuations joined the way Rust joins them.
fn first_string(t: &str) -> Option<String> {
    let start = t.find('"')? + 1;
    let mut out = String::new();
    let chars: Vec<char> = t[start..].chars().collect();
    let mut i = 0;
    while i < chars.len() && chars[i] != '"' {
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'\n') {
            i += 2;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }
        if chars[i] == '\\' {
            i += 1;
        }
        if let Some(ch) = chars.get(i) {
            out.push(*ch);
        }
        i += 1;
    }
    Some(out)
}

#[derive(Debug)]
struct Ignored {
    file: String,
    line: usize,
    test: String,
    /// `None` for a bare `#[ignore]`.
    reason: Option<String>,
    /// `cfg_attr(<condition>, ignore …)`: skipped only where the condition
    /// holds, and run everywhere else.
    condition: Option<String>,
}

fn ignored_in(file: &str, src: &str) -> Vec<Ignored> {
    let (c, m) = without_comments(src);
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < m.len() {
        if !(m[i] == '#' && m[i + 1] == '[') {
            i += 1;
            continue;
        }
        let Some(close) = closing_bracket(&m, i + 1) else {
            break;
        };
        let inner: String = c[i + 2..close].iter().collect();
        let compact: String = inner.split_whitespace().collect::<Vec<_>>().join(" ");
        let ignores = compact == "ignore"
            || compact.starts_with("ignore =")
            || compact.starts_with("ignore=")
            || (compact.starts_with("cfg_attr") && compact.contains(", ignore"));
        if ignores {
            let after: String = c[close + 1..].iter().collect();
            let test = after
                .split("fn ")
                .nth(1)
                .map(|r| {
                    r.chars()
                        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
                        .collect()
                })
                .unwrap_or_default();
            let reason = inner.find("ignore").and_then(|k| first_string(&inner[k..]));
            let condition = compact.starts_with("cfg_attr").then(|| {
                compact
                    .trim_start_matches("cfg_attr")
                    .trim_start_matches(['(', ' '])
                    .split(", ignore")
                    .next()
                    .unwrap_or_default()
                    .to_string()
            });
            out.push(Ignored {
                file: file.to_string(),
                line: c[..i].iter().filter(|&&ch| ch == '\n').count() + 1,
                test,
                reason,
                condition,
            });
        }
        i = close + 1;
    }
    out
}

fn in_scope(file: &str, test: &str) -> bool {
    let (f, t) = (file.to_ascii_lowercase(), test.to_ascii_lowercase());
    IN_SCOPE.iter().any(|w| f.contains(w) || t.contains(w))
}

/// What a waiver lacks: each of `issue=`, `owner=` and `review=` must be
/// present with a value, and the review date must read YYYY-MM-DD.
fn waiver_gaps(reason: Option<&str>) -> Vec<&'static str> {
    let Some(r) = reason else {
        return vec!["a reason string"];
    };
    let value = |key: &str| -> Option<String> {
        let at = r.find(key)? + key.len();
        let v: String = r[at..]
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ',' && *c != ';' && *c != ')')
            .collect();
        (!v.is_empty()).then_some(v)
    };
    let mut gaps = Vec::new();
    if value("issue=").is_none() {
        gaps.push("issue=");
    }
    if value("owner=").is_none() {
        gaps.push("owner=");
    }
    match value("review=") {
        None => gaps.push("review="),
        Some(d) => {
            let b = d.as_bytes();
            let dated = b.len() == 10
                && b[4] == b'-'
                && b[7] == b'-'
                && b.iter()
                    .enumerate()
                    .all(|(k, ch)| k == 4 || k == 7 || ch.is_ascii_digit());
            if !dated {
                gaps.push("review=YYYY-MM-DD");
            }
        }
    }
    gaps
}

fn relative(p: &Path) -> String {
    p.strip_prefix(workspace_root())
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn every_ignored_test_is_listed_and_one_guarding_precision_privacy_provenance_or_cancel_carries_a_waiver(
) {
    let files = scanned_files();
    assert!(
        files.len() >= 200,
        "{} files scanned — the scan would pass vacuously",
        files.len()
    );
    let mut all = Vec::new();
    for f in &files {
        all.extend(ignored_in(
            &relative(f),
            &std::fs::read_to_string(f).expect("read"),
        ));
    }
    println!("{} ignored test(s) in the workspace:", all.len());
    for t in &all {
        let scope = if in_scope(&t.file, &t.test) {
            "in scope"
        } else {
            "-"
        };
        let when = t
            .condition
            .as_deref()
            .map_or("always".to_string(), |c| format!("only on {c}"));
        println!(
            "  {}:{} {} [{scope}] [{when}] {:?}",
            t.file, t.line, t.test, t.reason
        );
    }
    // 12 `#[ignore]` and 5 `cfg_attr(windows, ignore)` when this was written.
    assert!(
        all.len() >= 15,
        "only {} ignored tests found — the matcher has stopped matching",
        all.len()
    );
    assert!(
        all.iter().all(|t| !t.test.is_empty()),
        "an ignore attribute with no test after it: {all:?}"
    );
    // The waiver is for a test that runs nowhere: `#[ignore]`. A
    // `cfg_attr(<platform>, ignore)` still runs on the other platforms, and is
    // listed above with its condition.
    let missing: Vec<String> = all
        .iter()
        .filter(|t| t.condition.is_none() && in_scope(&t.file, &t.test))
        .filter_map(|t| {
            let gaps = waiver_gaps(t.reason.as_deref());
            (!gaps.is_empty())
                .then(|| format!("{}:{} {} lacks {}", t.file, t.line, t.test, gaps.join(", ")))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "an ignored test that guards precision, privacy, provenance or cancellation needs issue=, owner= and review= in its reason:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_guard_reads_each_form_of_ignore_and_each_gap() {
    let src = "#[test]\n#[ignore]\nfn a_bare_one() {}\n\
               #[test]\n#[ignore = \"slow; issue=X-1 owner=me review=2027-01-31\"]\nfn a_waived_one() {}\n\
               #[test]\n#[cfg_attr(\n    windows,\n    ignore = \"Windows: \\\n              loader\"\n)]\nfn a_windows_one() {}\n\
               // #[ignore]\n// fn commented_out() {}\n\
               fn f() { let s = \"#[ignore] fn in_a_string() {}\"; }\n";
    let found = ignored_in("x.rs", src);
    let names: Vec<&str> = found.iter().map(|t| t.test.as_str()).collect();
    assert_eq!(names, ["a_bare_one", "a_waived_one", "a_windows_one"]);
    assert_eq!(found[0].reason, None);
    assert_eq!(found[2].reason.as_deref(), Some("Windows: loader"));
    assert_eq!(found[2].condition.as_deref(), Some("windows"));
    assert_eq!(found[1].condition, None);
    assert!(waiver_gaps(found[1].reason.as_deref()).is_empty());
    assert_eq!(waiver_gaps(None), ["a reason string"]);
    assert_eq!(waiver_gaps(Some("slow")), ["issue=", "owner=", "review="]);
    assert_eq!(
        waiver_gaps(Some("issue=A owner=b review=soon")),
        ["review=YYYY-MM-DD"]
    );
    assert!(in_scope("tests/l1_precision_sweep.rs", "measure"));
    assert!(in_scope("tests/x.rs", "a_run_is_cancelled"));
    assert!(!in_scope("tests/x.rs", "needle_a"));
}
