//! **The project store's guards (Research OS Phase 5B-1; decisions 99 to
//! 112; ADR-009 §1, §2, §16).** Four source scans, each with a self-check
//! that its matcher still matches and a snippet it must catch:
//!
//! 1. **No caller.** No non-test code outside `src/project_store.rs` names the
//!    store or any item it declares, in either crate, so nothing reaches it
//!    until 5B-2 wires a Save click (decision 105). The item list is read from
//!    the store's source. The one allowed mention is its `pub mod` line in
//!    `src/lib.rs`. `#[cfg(test)]` modules elsewhere are not scanned.
//! 2. **One resolver.** `projects_root` resolves the folder through
//!    `app_local_data_dir()` (decision 99), and the store's non-test code
//!    names no other path resolver, home folder, temp folder or environment
//!    variable.
//! 3. **Paths from parsed values.** Every `.join(…)` in the store's non-test
//!    code joins a fixed name, a parsed id's `as_str()`, a parsed hash's
//!    `as_hex()`, or a fixed prefix formatted with one of them. The list is
//!    exact, so a new join fails here until it is read and added.
//! 4. **The AI boundary.** No outbound or transport file of either crate
//!    (`stats_assist/`, `stats_ai.rs`, `models/proxy_client.rs`,
//!    `http_fetcher.rs`, `verify_agent`, `refverify`) and no file of the
//!    frontend's Supabase service names the project modules or any type they
//!    declare (read from their source); and the project modules name no HTTP
//!    client, transport, proxy, helper layer or Supabase.
//!
//! **What it cannot see.** It constrains what was TYPED. A third module that
//! reads a project's artifact into a plain `String` or `Vec<u8>` and copies
//! it into an outbound field names no project type and passes; so does a
//! future screen that hands a project's values to the Supabase client at run
//! time through a variable. A macro from another crate that expands to a
//! call is invisible. The rendered side for the store is its sentinel sweep
//! (`project_store_sentinels.rs`), which reads every file, error and tracing
//! line a store session produces; 5B-2's privacy matrices add the helper
//! previews.

use std::path::{Path, PathBuf};

const ROOT: &str = env!("CARGO_MANIFEST_DIR");
const STORE: &str = "src/project_store.rs";
const MODEL: &str = "gaply-core/src/research_project";

/// Outbound structs, transports and HTTP seams, in both crates.
const OUTBOUND: &[&str] = &[
    "gaply-core/src/stats_assist",
    "src/stats_ai.rs",
    "src/models/proxy_client.rs",
    "src/http_fetcher.rs",
    "gaply-core/src/verify_agent.rs",
    "gaply-core/src/verify_agent",
    "gaply-core/src/refverify.rs",
    "gaply-core/src/refverify",
];

/// The frontend's Supabase service, from the repository root.
const SUPABASE: &str = "../src/services/supabase";

/// What the project modules may never name.
const NETWORK_WORDS: &[&str] = &[
    "reqwest",
    "ureq",
    "hyper",
    "HttpFetcher",
    "ReqwestFetcher",
    "ProxyClient",
    "HelperTransport",
    "OutgoingRequest",
    "ResponsesTransport",
    "stats_assist",
    "stats_ai",
    "proxy_client",
    "http_fetcher",
    "supabase",
    "Supabase",
];

/// Path resolvers and environment reads the store may not use; only
/// `app_local_data_dir` is allowed (decision 99).
const OTHER_RESOLVERS: &[&str] = &[
    "app_data_dir",
    "data_dir",
    "local_data_dir",
    "home_dir",
    "document_dir",
    "desktop_dir",
    "download_dir",
    "app_config_dir",
    "config_dir",
    "app_cache_dir",
    "cache_dir",
    "app_log_dir",
    "resource_dir",
    "temp_dir",
    "var_os",
];

/// Every argument a `.join(` in the store's non-test code may have.
const JOIN_ARGUMENTS: &[&str] = &[
    "PROJECTS_DIR",
    "DB_FILE",
    "ARTIFACTS_DIR",
    "id.as_str()",
    "h.as_hex()",
    "hash.as_hex()",
    "a.expected.as_hex()",
    "format!(\"{CREATING_PREFIX}{}\", id.as_str())",
    "format!(\"{DELETING_PREFIX}{}\", id.as_str())",
    "format!(\"{TMP_PREFIX}{}\", random_hex32()?)",
];

fn rust_files(p: &Path, out: &mut Vec<PathBuf>) {
    if p.is_file() {
        out.push(p.to_path_buf());
        return;
    }
    for e in std::fs::read_dir(p)
        .unwrap_or_else(|e| panic!("{}: {e} — has it moved?", p.display()))
        .flatten()
    {
        let q = e.path();
        if q.is_dir() {
            rust_files(&q, out);
        } else if q.extension().is_some_and(|x| x == "rs") {
            out.push(q);
        }
    }
}

/// The code with `//` comments removed, a `//` inside a string kept.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| {
            let mut in_str = false;
            let b = l.as_bytes();
            for i in 0..b.len() {
                match b[i] {
                    b'"' if i == 0 || b[i - 1] != b'\\' => in_str = !in_str,
                    b'/' if !in_str && b.get(i + 1) == Some(&b'/') => return &l[..i],
                    _ => {}
                }
            }
            l
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The code with every `#[cfg(test)]` module removed, by brace count.
fn without_test_modules(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        let next = lines.get(i + 1).map(|l| l.trim()).unwrap_or("");
        if t == "#[cfg(test)]"
            && (next.starts_with("mod ") || next.starts_with("pub(crate) mod "))
            && next.ends_with('{')
        {
            let mut depth = 0i32;
            let mut j = i + 1;
            loop {
                depth +=
                    lines[j].matches('{').count() as i32 - lines[j].matches('}').count() as i32;
                j += 1;
                if depth <= 0 || j >= lines.len() {
                    break;
                }
            }
            i = j;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    out.join("\n")
}

fn identifiers(code: &str) -> Vec<&str> {
    code.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect()
}

/// The names a source declares as `pub`: structs, enums, traits and types at
/// any depth (those written through `closed!` are indented), and functions
/// at the top level only, since a method is reached through its type.
fn declared_pub_names(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in code.lines() {
        let t = line.trim();
        let top = !line.starts_with(char::is_whitespace);
        for p in [
            "pub struct ",
            "pub enum ",
            "pub trait ",
            "pub type ",
            "pub fn ",
        ] {
            if p == "pub fn " && !top {
                continue;
            }
            if let Some(rest) = t.strip_prefix(p) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn store_items() -> Vec<String> {
    let src = std::fs::read_to_string(Path::new(ROOT).join(STORE)).expect("the store");
    declared_pub_names(&without_test_modules(&code_only(&src)))
}

fn model_items() -> Vec<String> {
    let mut files = Vec::new();
    rust_files(&Path::new(ROOT).join(MODEL), &mut files);
    let mut names = Vec::new();
    for f in &files {
        names.extend(declared_pub_names(&code_only(
            &std::fs::read_to_string(f).unwrap(),
        )));
    }
    names.sort();
    names.dedup();
    names
}

/// Mentions of `forbidden` words in non-test code, by file and line.
fn mentions(file: &str, src: &str, forbidden: &[String]) -> Vec<String> {
    let code = without_test_modules(&code_only(src));
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        for w in identifiers(line) {
            if forbidden.iter().any(|f| f == w) {
                out.push(format!("{file}:{}: `{w}`", n + 1));
            }
        }
    }
    out
}

#[test]
fn no_non_test_code_outside_the_store_calls_it() {
    let items = store_items();
    for expected in [
        "ProjectRoot",
        "ProjectHandle",
        "SaveRequest",
        "projects_root",
    ] {
        assert!(
            items.iter().any(|i| i == expected),
            "the item scan lost `{expected}`: {items:?}"
        );
    }
    let mut forbidden = items.clone();
    forbidden.push("project_store".to_string());

    let root = Path::new(ROOT);
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("gaply-core/src"), &mut files);
    assert!(
        files.len() > 200,
        "{} files scanned — the scan would pass vacuously",
        files.len()
    );
    let mut found = Vec::new();
    let mut declarations = 0;
    for f in &files {
        let rel = f
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if rel == STORE {
            continue;
        }
        let src = std::fs::read_to_string(f).unwrap();
        for m in mentions(&rel, &src, &forbidden) {
            let line_no: usize = m
                .rsplit(':')
                .nth(1)
                .and_then(|n| n.trim().parse().ok())
                .unwrap_or(0);
            let code = without_test_modules(&code_only(&src));
            let line = code
                .lines()
                .nth(line_no.saturating_sub(1))
                .unwrap_or("")
                .trim();
            if rel == "src/lib.rs" && line == "pub mod project_store;" {
                declarations += 1;
                continue;
            }
            found.push(format!("{m}\n    {line}"));
        }
    }
    assert_eq!(
        declarations, 1,
        "src/lib.rs declares the store exactly once"
    );
    assert!(
        found.is_empty(),
        "the project store is called from outside it before 5B-2:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_root_resolver_is_the_local_app_data_dir_and_no_other() {
    let src = std::fs::read_to_string(Path::new(ROOT).join(STORE)).unwrap();
    let code = without_test_modules(&code_only(&src));
    let words = identifiers(&code);
    assert_eq!(
        words.iter().filter(|w| **w == "app_local_data_dir").count(),
        1,
        "one call to app_local_data_dir"
    );
    let start = code
        .find("pub fn projects_root")
        .expect("projects_root exists");
    let body: String = code[start..]
        .lines()
        .take_while(|l| *l != "}")
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains(".app_local_data_dir()"),
        "projects_root calls it:\n{body}"
    );
    let found: Vec<&&str> = OTHER_RESOLVERS
        .iter()
        .filter(|r| words.contains(r))
        .collect();
    assert!(
        found.is_empty(),
        "the store names another resolver: {found:?}"
    );
    for text in ["std::env", "env::var", "dirs::"] {
        assert!(!code.contains(text), "the store reads `{text}`");
    }
}

/// The argument of each `.join(` call, with nested parentheses kept.
fn join_arguments(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = code;
    while let Some(i) = rest.find(".join(") {
        let tail = &rest[i + ".join(".len()..];
        let mut depth = 1;
        let mut end = 0;
        for (k, c) in tail.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = k;
                        break;
                    }
                }
                _ => {}
            }
        }
        out.push(tail[..end].trim().to_string());
        rest = &tail[end..];
    }
    out
}

#[test]
fn every_path_the_store_joins_is_a_parsed_id_a_hash_or_a_fixed_name() {
    let src = std::fs::read_to_string(Path::new(ROOT).join(STORE)).unwrap();
    let code = without_test_modules(&code_only(&src));
    let args = join_arguments(&code);
    assert!(
        args.len() >= 12,
        "{} joins found — the scan is inert",
        args.len()
    );
    let unlisted: Vec<&String> = args
        .iter()
        .filter(|a| !JOIN_ARGUMENTS.contains(&a.as_str()))
        .collect();
    assert!(
        unlisted.is_empty(),
        "a path joined from something that is not a parsed id, a hash or a fixed name: {unlisted:?}"
    );
    for listed in JOIN_ARGUMENTS {
        assert!(
            args.iter().any(|a| a == listed),
            "`{listed}` is listed but no longer joined; drop it"
        );
    }
}

#[test]
fn no_project_type_reaches_an_outbound_struct_a_transport_or_supabase() {
    let mut types = model_items();
    types.extend(store_items());
    for expected in [
        "ProjectId",
        "ArtifactHash",
        "ArtifactRef",
        "StoredRun",
        "ProvGraph",
        "ProjectCode",
        "ProjectRoot",
        "ArtifactInput",
    ] {
        assert!(
            types.iter().any(|t| t == expected),
            "the type scan lost `{expected}`"
        );
    }
    assert!(
        types.len() >= 25,
        "{} project names found — the type list is inert",
        types.len()
    );
    let mut forbidden = types;
    forbidden.push("research_project".to_string());
    forbidden.push("project_store".to_string());

    let root = Path::new(ROOT);
    let mut files = Vec::new();
    for o in OUTBOUND {
        rust_files(&root.join(o), &mut files);
    }
    assert!(
        files.len() >= 15,
        "{} outbound files — the scan would pass vacuously",
        files.len()
    );
    let mut found = Vec::new();
    for f in &files {
        let rel = f
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        found.extend(mentions(
            &rel,
            &std::fs::read_to_string(f).unwrap(),
            &forbidden,
        ));
    }

    let sb = root.join(SUPABASE);
    let mut ts = 0;
    for e in std::fs::read_dir(&sb)
        .unwrap_or_else(|e| panic!("{}: {e} — has it moved?", sb.display()))
        .flatten()
    {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "ts" || x == "tsx") {
            ts += 1;
            let text = std::fs::read_to_string(&p).unwrap();
            for w in identifiers(&text) {
                if forbidden.iter().any(|f| f == w) {
                    found.push(format!("{}: `{w}`", p.display()));
                }
            }
        }
    }
    assert!(
        ts >= 5,
        "{ts} Supabase files — the scan would pass vacuously"
    );
    assert!(
        found.is_empty(),
        "a project type outside the device boundary:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_project_modules_name_no_client_transport_proxy_or_helper() {
    let root = Path::new(ROOT);
    let mut files = vec![root.join(STORE)];
    rust_files(&root.join(MODEL), &mut files);
    assert!(files.len() >= 3);
    let words: Vec<String> = NETWORK_WORDS.iter().map(|w| w.to_string()).collect();
    let mut found = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        // Test modules too: a test that reached the network would be a hole.
        for (n, line) in code_only(&src).lines().enumerate() {
            for w in identifiers(line) {
                if words.iter().any(|f| f == w) {
                    found.push(format!("{}:{}: `{w}`", f.display(), n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn the_guards_catch_what_they_exist_for() {
    let forbidden = vec!["ProjectRoot".to_string(), "project_store".to_string()];
    assert_eq!(
        mentions(
            "x.rs",
            "fn f() { let _ = crate::project_store::ProjectRoot::open_root(p); }\n",
            &forbidden
        )
        .len(),
        2
    );
    assert!(mentions("x.rs", "// crate::project_store in a comment\n", &forbidden).is_empty());
    assert!(
        mentions(
            "x.rs",
            "#[cfg(test)]\nmod tests {\n    use crate::project_store::ProjectRoot;\n}\n",
            &forbidden
        )
        .is_empty(),
        "a test module elsewhere may use the store"
    );
    let planted = "use crate::research_project::ProjectId;\npub struct Out { id: ProjectId }\n";
    assert_eq!(
        mentions(
            "outbound.rs",
            planted,
            &["research_project".to_string(), "ProjectId".to_string()]
        )
        .len(),
        3
    );
    assert_eq!(
        join_arguments("a.join(raw_id); b.join(format!(\"{X}{}\", id.as_str()));"),
        ["raw_id", "format!(\"{X}{}\", id.as_str())"]
    );
    assert_eq!(
        declared_pub_names(
            "pub struct A {\n    pub enum B in \"x.\" {\npub fn c() {}\n    pub fn method() {}"
        ),
        ["A", "B", "c"]
    );
}
