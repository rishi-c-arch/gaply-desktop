//! **`src/contracts/` and `src/stats_desc/` are pure, the contracts name no
//! domain, and only the types listed here may derive `Serialize`.**
//!
//! The Research OS's guard (Phase 2A), in the shape of
//! `stats_l1_is_llm_free_and_offline.rs`: a scanned directory, a forbidden
//! list, an import allowlist, a self-check, and a deletion test recorded in
//! the Phase 2A report. Six checks:
//!
//! 1. No route to a file, the network, a process, the environment, a
//!    thread, the clock, a database, Tauri, an async runtime, a model, the
//!    proxy, stdout or a log, in either directory.
//! 2. `use` roots: `std`, `serde`, `serde_json` and `sha2` from outside the
//!    crate. Inside it, `contracts` may name only itself; `stats_desc` may
//!    name `crate::contracts`, `crate::stats_l1::dataset`, and exactly
//!    `crate::stats_verify::{mean, sample_variance, RangeFault}` (the stable
//!    two-pass mean of part C and the variance over its range-checked sum of
//!    squares) and `crate::stats_l1::binding::suspect_codes_in` (the rule
//!    that uses `MISSING_CODE_CANDIDATES`). Every `crate::` path anywhere in
//!    the code is held to the same list, so an inline path cannot route
//!    around a `use` line.
//! 3. Layering: `contracts` names no module of the verifier or a capability
//!    and no type they declare. The type list is read from their source,
//!    not typed here.
//! 4. `Serialize`: in `stats_desc`, exactly the types listed below derive
//!    it; those that hold a header or a level label are listed apart; none
//!    of them holds a cell, the dataset or the tally. In `contracts`, no
//!    field holds a `String` except the two checked newtypes.
//! 5. A type in `stats_desc` with a `String` field does not derive `Debug`
//!    and prints counts through its own `impl Debug`.
//! 6. Every struct that derives `Deserialize` refuses unknown fields (or is
//!    a transparent newtype), and every named-field struct on the wire is
//!    camelCase.
//!
//! What it cannot see: a route through a helper in an allowed module (a
//! function of `stats_l1::dataset` that read a file would not be caught
//! here; `stats_l1_is_llm_free_and_offline.rs` guards that directory), and
//! a macro from another crate that expands to I/O.

use std::path::{Path, PathBuf};

const CONTRACTS: &str = "src/contracts";
const STATS_DESC: &str = "src/stats_desc";

const FORBIDDEN: &[(&str, &str)] = &[
    ("std::fs", "the filesystem"),
    ("File::", "the filesystem"),
    ("OpenOptions", "the filesystem"),
    ("std::path", "a path"),
    ("PathBuf", "a path"),
    ("std::io", "I/O"),
    ("std::net", "the network"),
    ("reqwest", "an HTTP client"),
    ("ureq", "an HTTP client"),
    ("hyper", "an HTTP client"),
    ("std::process", "a process"),
    ("Command::", "a process"),
    ("std::env", "the environment"),
    ("std::thread", "a thread"),
    ("std::time", "the clock"),
    ("SystemTime", "the clock"),
    ("Instant", "the clock"),
    ("tokio", "an async runtime"),
    ("tauri", "the app shell"),
    ("rusqlite", "the database"),
    ("Connection", "the database"),
    ("proxy", "the cloud proxy"),
    ("Proxy", "the cloud proxy"),
    ("stats_assist", "the helper layer"),
    ("candle", "a model runtime"),
    ("tokenizers", "a model runtime"),
    ("Generative", "a generative model"),
    ("Embedder", "an embedding model"),
    ("llm", "a language model"),
    ("print!", "stdout"),
    ("println!", "stdout"),
    ("dbg!", "stderr"),
    ("tracing", "a log"),
    ("log::", "a log"),
    ("unsafe", "unchecked code"),
];

const EXTERNAL_ROOTS: &[&str] = &["std", "serde", "serde_json", "sha2"];

const CONTRACTS_CRATE_PATHS: &[&str] = &["crate::contracts"];
const STATS_DESC_CRATE_PATHS: &[&str] = &[
    "crate::contracts",
    "crate::stats_l1::dataset",
    "crate::stats_verify",
    "crate::stats_l1::binding",
];

/// The verifier's items `stats_desc` may import, by exact name.
const VERIFIER_ITEMS: &[(&str, &[&str])] = &[
    (
        "crate::stats_verify",
        &["mean", "sample_variance", "RangeFault"],
    ),
    ("crate::stats_l1::binding", &["suspect_codes_in"]),
];

/// The modules `contracts` must never name.
const DOMAIN_MODULES: &[&str] = &[
    "stats_l0",
    "stats_l1",
    "stats_assist",
    "stats_desc",
    "stats_verify",
    "stats_verdict",
    "stats_chat",
];
/// Where their types are declared, read to build the type list.
const DOMAIN_SOURCES: &[&str] = &[
    "src/stats_l0",
    "src/stats_l1",
    "src/stats_assist",
    "src/stats_desc",
    "src/stats_verify.rs",
];

/// `stats_desc` types that hold a column header or a level label. They may
/// derive `Serialize` because they ARE the user's result, for the user's own
/// screen and the export the user saves.
const HOLDS_HEADERS_OR_LABELS: &[&str] = &[
    "DatasetProfile",
    "ColumnProfile",
    "LevelCount",
    "DescribeResult",
    "ColumnResult",
    "NumericSummary",
    "CategoricalSummary",
];
/// `stats_desc` types that derive `Serialize` and hold no text at all.
const HOLDS_NO_TEXT: &[&str] = &[
    "DescribeParams",
    "ColumnRequest",
    "TreatAs",
    "ColumnKind",
    "SuspectCode",
    "Stat",
];
/// What a serialisable type must never hold.
const CELL_TYPES: &[&str] = &[
    "Cell",
    "Dataset",
    "RawTable",
    "Tally",
    "SchemaSummary",
    "ColumnSummary",
];

/// The only `String`-holding types in `contracts`, each checked on the way
/// in. Sorted, as the scan's list is.
const CONTRACTS_STRING_TYPES: &[&str] = &["CapabilityId", "ContentHash"];

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    if dir.is_file() {
        return vec![dir.to_path_buf()];
    }
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

fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every file of a directory, comments removed, with its path.
fn sources(dir: &str, at_least: usize) -> Vec<(String, String)> {
    let files = rust_files(Path::new(dir));
    assert!(
        files.len() >= at_least,
        "{dir}: {} files — the scan would pass vacuously",
        files.len()
    );
    files
        .iter()
        .map(|f| {
            (
                f.display().to_string(),
                code_only(&std::fs::read_to_string(f).expect("read")),
            )
        })
        .collect()
}

fn forbidden_in(path: &str, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (needle, what) in FORBIDDEN {
        for (n, line) in code.lines().enumerate() {
            if line.contains(needle) {
                out.push(format!(
                    "{path}:{}: `{needle}` — {what}\n    {}",
                    n + 1,
                    line.trim()
                ));
            }
        }
    }
    out
}

/// Every `use` statement, joined onto one line when rustfmt wrapped it.
fn use_statements(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<String> = None;
    for line in code.lines() {
        let t = line.trim();
        if let Some(acc) = open.as_mut() {
            acc.push(' ');
            acc.push_str(t);
        } else if let Some(rest) = t
            .strip_prefix("pub(crate) use ")
            .or_else(|| t.strip_prefix("pub use "))
            .or_else(|| t.strip_prefix("use "))
        {
            open = Some(rest.to_string());
        }
        if open.as_ref().is_some_and(|acc| acc.ends_with(';')) {
            out.push(
                open.take()
                    .expect("open")
                    .trim_end_matches(';')
                    .trim()
                    .to_string(),
            );
        }
    }
    out
}

/// The imported names of `use <prefix>::{a, b}` or `use <prefix>::a`.
fn imported_names(statement: &str, prefix: &str) -> Vec<String> {
    let rest = statement
        .strip_prefix(prefix)
        .unwrap_or("")
        .trim_start_matches("::");
    rest.trim_matches(|c| c == '{' || c == '}')
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn import_violations(
    path: &str,
    code: &str,
    crate_paths: &[&str],
    items: &[(&str, &[&str])],
) -> Vec<String> {
    let mut out = Vec::new();
    for statement in use_statements(code) {
        if !statement.contains("::") {
            continue; // a local name re-exported, such as `pub(crate) use closed;`
        }
        let root = statement
            .split(['{', ':', ';', ' '])
            .next()
            .unwrap_or("")
            .trim();
        match root {
            "self" | "super" => {}
            "crate" => {
                let Some(allowed) = crate_paths.iter().find(|p| statement.starts_with(*p)) else {
                    out.push(format!(
                        "{path}: `use {statement}` — not an allowed crate path"
                    ));
                    continue;
                };
                if let Some((_, names)) = items.iter().find(|(p, _)| p == allowed) {
                    for name in imported_names(&statement, allowed) {
                        if !names.contains(&name.as_str()) {
                            out.push(format!("{path}: `use {statement}` — `{name}` is not on the list for {allowed}"));
                        }
                    }
                }
            }
            r if EXTERNAL_ROOTS.contains(&r) => {}
            other => out.push(format!(
                "{path}: `use {statement}` — `{other}` is not on the import allowlist"
            )),
        }
    }
    out
}

/// Every `crate::` path in the code, `use` lines or not, must start with an
/// allowed path.
fn crate_path_violations(path: &str, code: &str, crate_paths: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        let mut rest = line;
        while let Some(i) = rest.find("crate::") {
            let tail = &rest[i..];
            if !crate_paths.iter().any(|p| tail.starts_with(p)) {
                out.push(format!(
                    "{path}:{}: `{}` is not an allowed crate path",
                    n + 1,
                    tail.split([' ', ';', ',', '(']).next().unwrap_or(tail)
                ));
            }
            rest = &rest[i + "crate::".len()..];
        }
    }
    out
}

fn identifiers(code: &str) -> Vec<&str> {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect()
}

/// A `struct` or `enum` declaration with the attribute lines above it.
#[derive(Debug)]
struct Item {
    name: String,
    attrs: String,
    body: String,
}

fn items(code: &str) -> Vec<Item> {
    let lines: Vec<&str> = code.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        let Some(rest) = [
            "pub struct ",
            "pub enum ",
            "pub(crate) struct ",
            "pub(crate) enum ",
            "struct ",
            "enum ",
        ]
        .iter()
        .find_map(|p| t.strip_prefix(p)) else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let mut attrs = Vec::new();
        for above in lines[..i].iter().rev() {
            let a = above.trim();
            if a.starts_with("#[") || a.is_empty() {
                attrs.push(a);
            } else {
                break;
            }
        }
        let mut body = String::new();
        let mut depth = 0i32;
        for line in &lines[i..] {
            body.push_str(line);
            body.push('\n');
            depth += line.matches('{').count() as i32 - line.matches('}').count() as i32;
            if depth <= 0 && (line.contains('}') || line.trim_end().ends_with(';')) {
                break;
            }
        }
        out.push(Item {
            name,
            attrs: attrs.join("\n"),
            body,
        });
    }
    out
}

fn derives(item: &Item, what: &str) -> bool {
    item.attrs
        .lines()
        .any(|a| a.starts_with("#[derive(") && a.contains(what))
}

/// Field-type positions of `String`: `: String`, `<String`, `(String`.
fn holds_string(body: &str) -> bool {
    [": String", "<String", "(String"]
        .iter()
        .any(|p| body.contains(p))
}

#[test]
fn contracts_and_stats_desc_reach_no_file_network_process_clock_model_or_log() {
    let mut found = Vec::new();
    for (path, code) in sources(CONTRACTS, 2)
        .iter()
        .chain(sources(STATS_DESC, 2).iter())
    {
        found.extend(forbidden_in(path, code));
    }
    assert!(
        found.is_empty(),
        "contracts and stats_desc are pure. Found:\n{}",
        found.join("\n")
    );
}

#[test]
fn imports_are_std_serde_serde_json_sha2_and_the_named_crate_paths_only() {
    let mut found = Vec::new();
    let mut statements = 0;
    for (dir, crate_paths, verifier) in [
        (CONTRACTS, CONTRACTS_CRATE_PATHS, &[][..]),
        (STATS_DESC, STATS_DESC_CRATE_PATHS, VERIFIER_ITEMS),
    ] {
        for (path, code) in sources(dir, 2) {
            statements += use_statements(&code).len();
            found.extend(import_violations(&path, &code, crate_paths, verifier));
            found.extend(crate_path_violations(&path, &code, crate_paths));
        }
    }
    assert!(
        statements >= 8,
        "{statements} `use` statements found — the import scan is inert"
    );
    assert!(found.is_empty(), "{}", found.join("\n"));

    // stats_desc must still import each named verifier item, or the list
    // guards nothing.
    let desc: String = sources(STATS_DESC, 2).into_iter().map(|(_, c)| c).collect();
    for (prefix, names) in VERIFIER_ITEMS {
        let imported: Vec<String> = use_statements(&desc)
            .iter()
            .filter(|s| s.starts_with(prefix))
            .flat_map(|s| imported_names(s, prefix))
            .collect();
        for name in *names {
            assert!(
                imported.iter().any(|i| i == name),
                "stats_desc no longer imports {prefix}::{name}; update the list"
            );
        }
    }
}

#[test]
fn contracts_name_no_verifier_or_capability_module_or_type() {
    let contracts = sources(CONTRACTS, 2);
    let own: Vec<String> = contracts
        .iter()
        .flat_map(|(_, c)| items(c))
        .map(|i| i.name)
        .collect();
    assert!(
        own.len() >= 20,
        "{} types found in contracts — the item scan is inert",
        own.len()
    );

    let mut domain_types = Vec::new();
    for src in DOMAIN_SOURCES {
        for (_, code) in sources(src, 1) {
            for line in code.lines() {
                let t = line.trim();
                for p in [
                    "pub struct ",
                    "pub enum ",
                    "pub trait ",
                    "pub type ",
                    "pub(crate) struct ",
                    "pub(crate) enum ",
                ] {
                    if let Some(rest) = t.strip_prefix(p) {
                        let name: String = rest
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_')
                            .collect();
                        if !name.is_empty() {
                            domain_types.push(name);
                        }
                    }
                }
            }
        }
    }
    domain_types.sort();
    domain_types.dedup();
    assert!(
        domain_types.len() >= 100,
        "{} domain types found — the type list is inert",
        domain_types.len()
    );

    let mut found = Vec::new();
    for (path, code) in &contracts {
        for word in identifiers(code) {
            if DOMAIN_MODULES.contains(&word) {
                found.push(format!("{path}: names the module `{word}`"));
            }
            if domain_types.iter().any(|t| t == word) && !own.iter().any(|o| o == word) {
                found.push(format!("{path}: names the domain type `{word}`"));
            }
        }
    }
    let shared: Vec<&String> = domain_types.iter().filter(|t| own.contains(t)).collect();
    println!("names declared both in contracts and in a domain module (contracts' own, so exempt): {shared:?}");
    assert!(
        found.is_empty(),
        "contracts are domain-free:\n{}",
        found.join("\n")
    );
}

#[test]
fn only_the_listed_stats_desc_types_derive_serialize_and_none_holds_a_cell() {
    let desc: Vec<Item> = sources(STATS_DESC, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut serialised: Vec<&str> = desc
        .iter()
        .filter(|i| derives(i, "Serialize"))
        .map(|i| i.name.as_str())
        .collect();
    serialised.sort_unstable();
    let mut listed: Vec<&str> = HOLDS_HEADERS_OR_LABELS
        .iter()
        .chain(HOLDS_NO_TEXT)
        .copied()
        .collect();
    listed.sort_unstable();
    assert_eq!(
        serialised, listed,
        "the types deriving Serialize in {STATS_DESC} are not the listed ones"
    );

    for item in desc.iter().filter(|i| derives(i, "Serialize")) {
        for cell in CELL_TYPES {
            assert!(
                !identifiers(&item.body).contains(cell),
                "{} derives Serialize and names `{cell}`",
                item.name
            );
        }
        if HOLDS_NO_TEXT.contains(&item.name.as_str()) {
            assert!(
                !holds_string(&item.body),
                "{} is listed as holding no text but has a String field",
                item.name
            );
            for text in HOLDS_HEADERS_OR_LABELS {
                assert!(
                    !identifiers(&item.body).contains(text),
                    "{} is listed as holding no text but holds a {text}",
                    item.name
                );
            }
        }
    }
    let tally = desc
        .iter()
        .find(|i| i.name == "Tally")
        .expect("the tally exists");
    assert!(
        !tally.attrs.contains("#[derive("),
        "the tally holds the column's values and derives nothing"
    );

    let contracts: Vec<Item> = sources(CONTRACTS, 2)
        .iter()
        .flat_map(|(_, c)| items(c))
        .collect();
    let mut with_strings: Vec<&str> = contracts
        .iter()
        .filter(|i| holds_string(&i.body))
        .map(|i| i.name.as_str())
        .collect();
    with_strings.sort_unstable();
    assert_eq!(
        with_strings, CONTRACTS_STRING_TYPES,
        "only the checked newtypes in contracts may hold a String"
    );
}

#[test]
fn a_stats_desc_type_holding_text_prints_counts_through_its_own_debug() {
    let files = sources(STATS_DESC, 2);
    let all: String = files.iter().map(|(_, c)| c.as_str()).collect();
    let texty: Vec<Item> = files
        .iter()
        .flat_map(|(_, c)| items(c))
        .filter(|i| holds_string(&i.body))
        .collect();
    assert!(
        texty.len() >= 5,
        "{} text-holding types found — the scan is inert",
        texty.len()
    );
    for item in &texty {
        assert!(
            !derives(item, "Debug"),
            "{} holds a header or label and derives Debug",
            item.name
        );
        let printable = all.contains(&format!("impl std::fmt::Debug for {}", item.name));
        if derives(item, "Serialize") || printable {
            assert!(
                HOLDS_HEADERS_OR_LABELS.contains(&item.name.as_str()),
                "{} holds a String and is not listed",
                item.name
            );
            assert!(printable, "{} has no counts-only Debug", item.name);
        } else {
            // The tally: neither printed nor serialised, so nothing to list.
            assert!(
                item.attrs.is_empty() || !item.attrs.contains("#[derive("),
                "{} derives something",
                item.name
            );
        }
    }
}

#[test]
fn every_deserialised_struct_refuses_unknown_fields_and_wire_structs_are_camel_case() {
    let mut checked = 0;
    for dir in [CONTRACTS, STATS_DESC] {
        for (path, code) in sources(dir, 2) {
            // Every check here reads a derive from ONE line. A derive list
            // wrapped over several lines would be invisible to them, so it
            // fails here instead of passing there.
            for (n, line) in code.lines().enumerate() {
                let t = line.trim();
                assert!(
                    !t.starts_with("#[derive(") || t.ends_with(")]"),
                    "{path}:{}: a derive wrapped over lines; the guard reads one line",
                    n + 1
                );
            }
            for item in items(&code) {
                let is_struct = item.body.trim_start().starts_with("pub struct")
                    || item.body.trim_start().starts_with("struct");
                let named_fields = is_struct
                    && item
                        .body
                        .lines()
                        .next()
                        .is_some_and(|l| l.trim_end().ends_with('{'));
                if is_struct && derives(&item, "Deserialize") {
                    checked += 1;
                    assert!(
                        item.attrs.contains("deny_unknown_fields")
                            || item.attrs.contains("transparent"),
                        "{path}: {} derives Deserialize without deny_unknown_fields",
                        item.name
                    );
                }
                if named_fields && (derives(&item, "Serialize") || derives(&item, "Deserialize")) {
                    assert!(
                        item.attrs.contains("rename_all = \"camelCase\""),
                        "{path}: {} is on the wire without camelCase",
                        item.name
                    );
                }
            }
        }
    }
    assert!(
        checked >= 12,
        "{checked} deserialised structs found — the scan is inert"
    );
}

/// The predicates catch what they exist for, and a comment naming a route
/// does not trip them.
#[test]
fn the_guard_catches_each_kind_of_violation() {
    for offending in [
        "fn read(p: &str) -> String { std::fs::read_to_string(p).unwrap() }\n",
        "use std::net::TcpStream;\n",
        "fn now() -> u64 { std::time::SystemTime::now().elapsed().unwrap().as_secs() }\n",
        "fn show(x: f64) { println!(\"{x}\"); }\n",
        "fn log(x: f64) { tracing::info!(x); }\n",
        "use tauri::State;\n",
    ] {
        assert!(
            !forbidden_in("synthetic.rs", &code_only(offending)).is_empty(),
            "missed: {offending}"
        );
    }
    assert!(forbidden_in(
        "synthetic.rs",
        &code_only("//! never std::fs, never tracing\npub fn f() {}\n")
    )
    .is_empty());

    let bad_root = "use regex::Regex;\n";
    assert!(
        !import_violations("s.rs", bad_root, STATS_DESC_CRATE_PATHS, VERIFIER_ITEMS).is_empty()
    );
    let wrapped = "use crate::stats_verify::{\n    mean,\n    t_test_welch,\n};\n";
    assert!(
        !import_violations("s.rs", wrapped, STATS_DESC_CRATE_PATHS, VERIFIER_ITEMS).is_empty(),
        "a wrapped use with an unlisted item"
    );
    let ok = "use crate::stats_verify::{mean, sample_variance, RangeFault};\nuse crate::stats_l1::dataset::Cell;\n";
    assert!(import_violations("s.rs", ok, STATS_DESC_CRATE_PATHS, VERIFIER_ITEMS).is_empty());
    let l0 = "use crate::stats_l0::pvalue::p_t;\n";
    assert!(!import_violations("s.rs", l0, STATS_DESC_CRATE_PATHS, VERIFIER_ITEMS).is_empty());
    let inline = "fn p() -> f64 { crate::stats_l0::pvalue::p_t(1.0, 2.0) }\n";
    assert!(
        !crate_path_violations("s.rs", inline, STATS_DESC_CRATE_PATHS).is_empty(),
        "an inline crate path"
    );

    let leak = "#[derive(Debug, Clone, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct Leak {\n    pub rows: Vec<String>,\n}\n";
    let item = &items(leak)[0];
    assert!(
        item.name == "Leak"
            && derives(item, "Serialize")
            && derives(item, "Debug")
            && holds_string(&item.body)
    );
    let open = "#[derive(Deserialize)]\npub struct Open {\n    pub a: u32,\n}\n";
    assert!(!items(open)[0].attrs.contains("deny_unknown_fields"));
}
