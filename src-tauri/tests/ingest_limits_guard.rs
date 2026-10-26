//! **Each production caller of the ingest passes its own named limits
//! (Research OS, Phase 8B, decision 243), by source scan.** The ingest takes
//! its limits as a value, `supplementary::IngestLimits`, and three named
//! values exist: `LEVEL_1`, `SUPPLEMENTARY` and `METHODS`. A later step changes
//! one of them alone, so a caller passing another's is a limit changed
//! without anyone deciding it. This scans `src/**/*.rs`, outside
//! `stats_l1_ingest.rs` and `supplementary.rs` (which define the ingest), and:
//!
//! 1. finds every call of `load_dataset_with`, `load_dataset_keeping_bytes`,
//!    `load_dataset_from_bytes_with` and `parse_supplementary_with`, as a
//!    whole identifier (`load_dataset` is a prefix of two other names, and
//!    `parse_supplementary` of `parse_supplementary_bytes`), never a method
//!    call such as `state.load_dataset(`, read to its matching parenthesis
//!    since rustfmt may wrap it;
//! 2. checks each passes, as its last argument, the value [`CALLS`] assigns
//!    to its file and enclosing function, and that every row of [`CALLS`] is
//!    found: fewer call sites than the table lists fails, and so does one it
//!    does not list;
//! 3. fails if production source names one of the old wrappers (they pass a
//!    fixed value and are for tests and examples only);
//! 4. fails if production source anywhere, the two files included, builds an
//!    `IngestLimits` outside `impl IngestLimits`: a struct literal, or a `..`
//!    spread.
//!
//! Comments and `#[cfg(test)]` items are removed first, as
//! `verifier_facade_guards.rs` removes them: an item at a time, so the
//! `#[cfg(test)] use` lines above `project_commands.rs`'s `rerun_open` hide
//! nothing after them. Tests and examples are outside its scope, and may
//! build an `IngestLimits` of their own (`tests/ingest_limits.rs` does).
//!
//! It constrains what is TYPED: a value reaching the ingest through a
//! variable, a constant of another name or a macro is not seen, and the
//! check that a named value is passed then fails rather than passing.

use std::path::{Path, PathBuf};

/// `(file, enclosing function, callee, the value it passes)`, as decision 243
/// assigns them.
const CALLS: &[(&str, &str, &str, &str)] = &[
    (
        "src/stats_l1_commands.rs",
        "open",
        "load_dataset_with",
        "LEVEL_1",
    ),
    (
        "src/stats_l1_commands.rs",
        "compute",
        "load_dataset_with",
        "LEVEL_1",
    ),
    (
        "src/stats_reproduction.rs",
        "run_stats_reproduction_measured",
        "load_dataset_with",
        "LEVEL_1",
    ),
    (
        "src/stats_desc_commands.rs",
        "open",
        "load_dataset_keeping_bytes",
        "METHODS",
    ),
    (
        "src/project_commands.rs",
        "rerun_open",
        "load_dataset_from_bytes_with",
        "METHODS",
    ),
    (
        "src/commands.rs",
        "run_publishready_measured",
        "parse_supplementary_with",
        "SUPPLEMENTARY",
    ),
    (
        "src/commands.rs",
        "load_first_table",
        "parse_supplementary_with",
        "SUPPLEMENTARY",
    ),
];

const CALLEES: &[&str] = &[
    "load_dataset_with",
    "load_dataset_keeping_bytes",
    "load_dataset_from_bytes_with",
    "parse_supplementary_with",
];

/// The wrappers that keep the old names and pass one fixed value.
const OLD_NAMES: &[&str] = &[
    "load_dataset",
    "load_dataset_from_bytes",
    "check_file",
    "dataset_from_checked",
    "compute_from_path",
    "parse_supplementary",
    "parse_supplementary_bytes",
    "exceeds_row_cap",
    "exceeds_col_cap",
];

const INGEST_FILES: &[&str] = &["src/stats_l1_ingest.rs", "src/supplementary.rs"];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let p = e.unwrap().path();
        if p.is_dir() {
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `src/...` with `/`, on every platform.
fn rel(p: &Path) -> String {
    let r = p.strip_prefix(root()).unwrap();
    r.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The source with block and line comments removed.
fn code(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        rest = rest[at..]
            .find("*/")
            .map_or("", |end| &rest[at + end + 2..]);
    }
    out.push_str(rest);
    out.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source with each `#[cfg(test)]` item removed: up to its `;` when that
/// comes before its `{`, else its brace block.
fn without_test_items(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let semi = after.find(';').unwrap_or(after.len());
        let brace = after.find('{').unwrap_or(after.len());
        if semi < brace {
            rest = &after[(semi + 1).min(after.len())..];
            continue;
        }
        rest = &after[block_end(after, brace)..];
    }
    out.push_str(rest);
    out
}

/// The index just past the brace block opening at `open`.
fn block_end(s: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (i, c) in s[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + i + 1;
                }
            }
            _ => {}
        }
    }
    s.len()
}

fn production(p: &Path) -> String {
    let src = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    without_test_items(&code(&src))
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Each offset where `name` stands as a whole identifier, not after a `.`.
fn occurrences(src: &str, name: &str) -> Vec<usize> {
    src.match_indices(name)
        .filter(|(i, _)| {
            let before = src[..*i].chars().next_back();
            let after = src[i + name.len()..].chars().next();
            !before.is_some_and(|c| ident(c) || c == '.') && !after.is_some_and(ident)
        })
        .map(|(i, _)| i)
        .collect()
}

/// The arguments of the call whose name ends at `from`, to the matching
/// parenthesis; `None` when no `(` follows.
fn arguments(src: &str, from: usize) -> Option<&str> {
    let rest = &src[from..];
    let open = rest.find(|c: char| !c.is_whitespace())?;
    if !rest[open..].starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (i, c) in rest[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&rest[open + 1..open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// The last top-level argument, without whitespace.
fn last_argument(args: &str) -> String {
    let mut depth = 0i32;
    let mut start = 0;
    let mut parts = Vec::new();
    for (i, c) in args.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&args[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&args[start..]);
    let last = parts
        .into_iter()
        .rev()
        .find(|p| !p.trim().is_empty())
        .unwrap_or("");
    last.chars().filter(|c| !c.is_whitespace()).collect()
}

/// The name of the last `fn` before `at`.
fn enclosing_fn(src: &str, at: usize) -> String {
    let head = &src[..at];
    let fns = occurrences(head, "fn");
    let i = *fns
        .last()
        .unwrap_or_else(|| panic!("no fn before offset {at}"));
    head[i + 2..]
        .trim_start()
        .chars()
        .take_while(|c| ident(*c))
        .collect()
}

/// `IngestLimits::X`, perhaps with its module path, as `X`.
fn named_value(arg: &str) -> Option<&str> {
    let tail = arg.strip_prefix("crate::").unwrap_or(arg);
    let tail = tail.strip_prefix("supplementary::").unwrap_or(tail);
    let value = tail.strip_prefix("IngestLimits::")?;
    ["LEVEL_1", "SUPPLEMENTARY", "METHODS"]
        .into_iter()
        .find(|v| *v == value)
}

fn sources() -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_files(&root().join("src"), &mut files);
    files.sort();
    files.iter().map(|p| (rel(p), production(p))).collect()
}

#[test]
fn each_production_caller_of_the_ingest_passes_its_named_limits() {
    let sources = sources();
    assert!(
        sources.len() > 50,
        "only {} source files found: the scan is reading the wrong folder",
        sources.len()
    );
    let mut found: Vec<(String, String, String, String)> = Vec::new();
    let mut failures = Vec::new();
    for (file, src) in &sources {
        if INGEST_FILES.contains(&file.as_str()) {
            continue;
        }
        for callee in CALLEES {
            for at in occurrences(src, callee) {
                let Some(args) = arguments(src, at + callee.len()) else {
                    continue;
                };
                let f = enclosing_fn(src, at);
                let arg = last_argument(args);
                match named_value(&arg) {
                    Some(v) => found.push((file.clone(), f, callee.to_string(), v.to_string())),
                    None => failures.push(format!(
                        "{file}: {f} calls {callee} with `{arg}` last, not a named IngestLimits"
                    )),
                }
            }
        }
        for old in OLD_NAMES {
            if !occurrences(src, old).is_empty() {
                failures.push(format!("{file} names the old wrapper {old}, which passes a fixed value; production code passes its named limits to the _with form"));
            }
        }
    }
    for (file, f, callee, value) in CALLS {
        let here: Vec<_> = found
            .iter()
            .filter(|(fl, fu, c, _)| fl == file && fu == f && c == callee)
            .collect();
        match here.as_slice() {
            [] => failures.push(format!(
                "{file}: {f} no longer calls {callee}; the table lists it"
            )),
            [one] if one.3 != *value => failures.push(format!(
                "{file}: {f} passes IngestLimits::{} to {callee}, and decision 243 assigns {value}",
                one.3
            )),
            [_] => {}
            many => failures.push(format!(
                "{file}: {f} calls {callee} {} times; the table lists one",
                many.len()
            )),
        }
    }
    for (file, f, callee, value) in &found {
        if !CALLS
            .iter()
            .any(|(fl, fu, c, _)| fl == file && fu == f && c == callee)
        {
            failures.push(format!("{file}: {f} calls {callee} with IngestLimits::{value}, a call the table does not list"));
        }
    }
    assert!(
        found.len() >= CALLS.len(),
        "found {} call sites, fewer than the {} the table lists: {failures:#?}",
        found.len(),
        CALLS.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn no_production_source_builds_an_ingest_limits_outside_its_impl() {
    let mut impls = 0;
    let mut failures = Vec::new();
    for (file, mut src) in sources() {
        if let Some(at) = src.find("impl IngestLimits {") {
            impls += 1;
            let open = at + src[at..].find('{').unwrap();
            src.replace_range(at..block_end(&src, open), "");
        }
        let squashed: String = src.chars().filter(|c| !c.is_whitespace()).collect();
        for (i, _) in squashed.match_indices("IngestLimits") {
            let after = &squashed[i + "IngestLimits".len()..];
            let before = &squashed[..i];
            let path_start = before
                .trim_end_matches(|c: char| ident(c) || c == ':')
                .len();
            if before[..path_start].ends_with("..") {
                failures.push(format!("{file}: a `..` spread of an IngestLimits"));
            }
            if after.starts_with('{') && !before.ends_with("struct") && !before.ends_with("->") {
                failures.push(format!("{file}: an IngestLimits struct literal"));
            }
        }
    }
    assert_eq!(
        impls, 1,
        "impl IngestLimits was not found exactly once: the scan cannot tell its block apart"
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
