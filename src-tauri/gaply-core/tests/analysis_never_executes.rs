//! **The analysis parsers never execute anything, and this is the guard.**
//!
//! §11 and §12's Phase 8: running uploaded code *"reintroduces process execution
//! into a product whose security posture rests on its absence"* — a deferred
//! phase needing its own threat model and probably its own binary. The parsers
//! in `src/analysis/` are where that posture erodes first, because executing the
//! script is the obvious way to learn what it did.
//!
//! # WHY A SOURCE SCAN AND NOT A DOC COMMENT
//!
//! Counted 14 Sep 2026: `validate.rs`, `stats_verify.rs` and `stats_verdict.rs`
//! each state *"no model, no proxy, no network, no I/O"* in their headers —
//! three claims, zero tests, holding only because nobody has tried. This file
//! follows `tests/equation_is_llm_free.rs`, which was the first guard of this
//! kind in the repository; that one asserts a PURITY property, and this one
//! asserts a SECURITY property, which is why it also denies dynamic loading and
//! anything that reaches a shell.
//!
//! **Confirmed to gate by breaking the real tree**, as the standing rule
//! requires — a gate whose first run is green proves nothing about whether it
//! gates. See `the_denylist_matches_when_it_should` for the same predicate run
//! over a synthetic module, so the proof does not depend on anyone remembering
//! to re-break the tree.

use std::fs;
use std::path::{Path, PathBuf};

/// Every route out of the process. Each entry is a SUBSTRING of source text.
const DENIED: &[(&str, &str)] = &[
    ("std::process", "process spawning — the thing Phase 8 defers"),
    ("Command::new", "a subprocess is an execution"),
    ("Command ::", "a subprocess is an execution"),
    ("std::os::unix::process", "process control"),
    ("libloading", "loading a dynamic library executes its initialisers"),
    ("dlopen", "loading a dynamic library executes its initialisers"),
    ("reqwest", "the analysis record never leaves the machine on its own"),
    ("ureq", "network"),
    ("TcpStream", "network"),
    ("std::net", "network"),
    ("rusqlite", "a parser writes nothing"),
    ("std::fs", "a parser is handed text, it does not go looking for files"),
    ("File::open", "a parser is handed text"),
    ("include_str!", "a parser is handed text"),
    ("unsafe", "no reason for one in a text parser"),
    ("SystemTime", "a parser is deterministic"),
    ("Instant::now", "a parser is deterministic"),
    ("rand", "a parser is deterministic"),
];

/// Crates the module may import. Anything else is a decision, not an accident.
const ALLOWED_CRATES: &[&str] = &["std", "core", "serde", "crate", "super", "self"];

fn analysis_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("analysis")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).expect("src/analysis must exist") {
        let p = e.expect("dir entry").path();
        if p.is_dir() {
            out.extend(rust_files(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Source with `//` and `//!` comment bodies removed, so the module's own prose
/// about `std::process` does not trip its own guard.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn violations(code: &str) -> Vec<&'static str> {
    DENIED.iter().filter(|(pat, _)| code.contains(pat)).map(|(pat, _)| *pat).collect()
}

#[test]
fn no_analysis_parser_can_reach_a_process_a_file_or_the_network() {
    let files = rust_files(&analysis_dir());
    assert!(!files.is_empty(), "no files scanned — a guard over nothing passes trivially");

    let mut found: Vec<String> = Vec::new();
    for f in &files {
        let code = code_only(&fs::read_to_string(f).expect("read"));
        for pat in violations(&code) {
            let why = DENIED.iter().find(|(p, _)| *p == pat).map(|(_, w)| *w).unwrap_or("");
            found.push(format!("{}: `{pat}` — {why}", f.file_name().unwrap().to_string_lossy()));
        }
    }
    assert!(
        found.is_empty(),
        "the analysis parsers must never execute, read or transmit anything:\n  {}",
        found.join("\n  ")
    );
}

#[test]
fn the_import_list_is_pinned() {
    for f in rust_files(&analysis_dir()) {
        let code = code_only(&fs::read_to_string(&f).expect("read"));
        for line in code.lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("use ") else { continue };
            let root = rest
                .trim_start_matches("::")
                .split(|c: char| c == ':' || c == ' ' || c == '{' || c == ';')
                .next()
                .unwrap_or("");
            assert!(
                ALLOWED_CRATES.contains(&root),
                "{}: imports `{root}`, which is not on the allowlist. Adding a crate to a \
                 read-only parser is a decision — make it here.",
                f.file_name().unwrap().to_string_lossy()
            );
        }
    }
}

/// **The negative control. Predict the failure before claiming the gate.**
///
/// The two tests above are green on a tree that has never violated the rule,
/// which is exactly the shape that proves nothing. This runs the same predicate
/// over source that DOES violate it and asserts each is caught by name.
#[test]
fn the_denylist_matches_when_it_should() {
    let bad = "use std::process::Command;\nfn run() { Command::new(\"Rscript\").spawn(); }";
    let v = violations(&code_only(bad));
    assert!(v.contains(&"std::process"), "a subprocess must be caught: {v:?}");
    assert!(v.contains(&"Command::new"), "and by both patterns: {v:?}");

    let reading = "let s = std::fs::read_to_string(p);";
    assert!(violations(&code_only(reading)).contains(&"std::fs"), "file reads must be caught");

    // And a comment mentioning the denied thing must NOT trip it, or the guard
    // makes the module undocumentable and gets deleted the first time it bites.
    let prose = "// we must never call std::process::Command here\nfn f() {}";
    assert!(
        violations(&code_only(prose)).is_empty(),
        "a comment is not code — the guard must read past it"
    );
}
