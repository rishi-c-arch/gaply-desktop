//! **Nothing in `src/stats_l1/` may name the cloud proxy, an HTTP fetcher, a
//! generative or embedding model, the helper layer, the filesystem, the
//! network, a database or a clock; and only the one outbound struct may
//! derive `Serialize`.**
//!
//! `stats_l1/mod.rs` claims all of that in prose. This is the scan that tests
//! it, in the shape of `stats_l0_is_llm_free_and_offline.rs`, with three
//! additions Level 1 needs and Level 0 did not:
//!
//! 1. `stats_assist` is forbidden by name: the prompt says `stats_l1` never
//!    references the helper layer.
//! 2. Filesystem, database and clock routes are forbidden: Level 1 receives a
//!    [`RawTable`] from the app crate's ingest and reads no file itself, and
//!    its tokens must not be persisted.
//! 3. A `Serialize` scan: types that hold rows, cell values, level labels,
//!    manuscript text or a key do not derive `Serialize`. The one exception
//!    in this module is `AliasedSchema` (with its row type `AliasedColumn`),
//!    the per-task outbound struct, and the scan names exactly those two.
//!
//! The same gaps as the Level 0 guard apply: a route through a helper in
//! another module, or a model type under a new name, is not caught.
//!
//! [`RawTable`]: gaply_core::stats_l1::dataset::RawTable

use std::path::{Path, PathBuf};

const SCANNED_DIR: &str = "src/stats_l1";

const FORBIDDEN: &[(&str, &str)] = &[
    // The helper layer.
    ("stats_assist", "the helper layer"),
    // The cloud proxy.
    ("ProxyClient", "the cloud proxy"),
    ("ProxyReqwestClient", "the cloud proxy"),
    ("proxy", "the cloud proxy"),
    ("Proxy", "the cloud proxy"),
    // HTTP.
    ("HttpFetcher", "an HTTP fetcher"),
    ("ReqwestFetcher", "an HTTP fetcher"),
    ("PaperFetch", "an HTTP fetcher"),
    ("Fetch", "an HTTP fetcher"),
    ("fetch", "an HTTP fetcher"),
    ("refverify", "the web connectors"),
    ("reqwest", "an HTTP client"),
    ("ureq", "an HTTP client"),
    ("std::net", "the network"),
    // Generative models.
    ("GenerationBackend", "a generative model"),
    ("QwenGenerativeBackend", "a generative model"),
    ("Generative", "a generative model"),
    ("generative", "a generative model"),
    ("PerplexityModel", "a language model"),
    ("CandlePerplexityModel", "a language model"),
    ("ModelManager", "a model"),
    ("ModelLease", "a model"),
    ("llm", "a language model"),
    ("Llm", "a language model"),
    ("LLM", "a language model"),
    ("candle", "a model runtime"),
    ("tokenizers", "a model runtime"),
    // Embedding models.
    ("Embedder", "an embedding model"),
    ("HashEmbedder", "an embedding model"),
    ("EmbeddingEngine", "an embedding model"),
    ("Embedding", "an embedding model"),
    ("embed", "an embedding model"),
    // Persistence, the filesystem and the clock.
    ("rusqlite", "the database"),
    ("Database", "the database"),
    ("Connection", "the database"),
    ("std::fs", "the filesystem"),
    ("File::", "the filesystem"),
    ("std::path", "a path"),
    ("PathBuf", "a path"),
    ("SystemTime", "the clock"),
    ("Instant::now", "the clock"),
    ("now_epoch", "the clock"),
];

/// `std` for everything, `serde` for the one outbound struct, `sha2` for the
/// fingerprint, digests and tokens.
const ALLOWED_EXTERNAL_IMPORTS: &[&str] = &["std", "serde", "sha2"];

/// The only types in the module that may derive `Serialize`.
const SERIALIZABLE: &[&str] = &["AliasedSchema", "AliasedColumn"];

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has Level 1 moved?", dir.display());
    };
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

fn violations(path: &str, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (needle, what) in FORBIDDEN {
        for (n, line) in code.lines().enumerate() {
            if line.contains(needle) {
                out.push(format!("{path}:{}: `{needle}` — {what}\n    {}", n + 1, line.trim()));
            }
        }
    }
    out
}

/// The name declared on the first `struct`/`enum` line after a `derive` that
/// names `Serialize`.
fn serialize_derives(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if t.starts_with("#[derive(") && t.contains("Serialize") {
            let name = lines[i + 1..]
                .iter()
                .map(|l| l.trim())
                .find(|l| !l.starts_with("#["))
                .and_then(|l| {
                    l.strip_prefix("pub struct ")
                        .or_else(|| l.strip_prefix("pub enum "))
                        .or_else(|| l.strip_prefix("struct "))
                        .or_else(|| l.strip_prefix("enum "))
                })
                .map(|rest| rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>())
                .unwrap_or_else(|| format!("<unnamed at line {}>", i + 1));
            out.push(name);
        }
    }
    out
}

#[test]
fn stats_l1_is_llm_free_and_offline() {
    let files = rust_files(Path::new(SCANNED_DIR));
    // 8 files on 13 Oct 2026. Fewer, and the scan is reading something else.
    assert!(files.len() >= 8, "the scan found {} files — it would pass vacuously", files.len());

    let mut found = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read");
        found.extend(violations(&f.display().to_string(), &code_only(&src)));
    }
    assert!(
        found.is_empty(),
        "Level 1 computes from a table it is handed: no proxy, no HTTP, no model, no helper layer, \
         no file, no database, no clock. Found:\n{}",
        found.join("\n")
    );
}

#[test]
fn stats_l1_imports_only_std_serde_and_sha2() {
    let mut imports = 0;
    for f in rust_files(Path::new(SCANNED_DIR)) {
        let src = std::fs::read_to_string(&f).expect("read");
        for (n, line) in code_only(&src).lines().enumerate() {
            let t = line.trim();
            let Some(rest) = t.strip_prefix("use ") else { continue };
            imports += 1;
            let root = rest.split(['{', ':', ';', ' ']).next().unwrap_or("").trim();
            if root.is_empty() || root == "crate" || root == "super" || root == "self" {
                continue;
            }
            assert!(
                ALLOWED_EXTERNAL_IMPORTS.contains(&root),
                "{}:{}: `{root}` is not on the Level 1 import allowlist\n    {t}",
                f.display(),
                n + 1
            );
        }
    }
    assert!(imports > 0, "no `use` line found — the import scan is inert");
}

#[test]
fn only_the_aliased_schema_derives_serialize() {
    let mut derived = Vec::new();
    for f in rust_files(Path::new(SCANNED_DIR)) {
        let src = std::fs::read_to_string(&f).expect("read");
        for name in serialize_derives(&code_only(&src)) {
            derived.push((f.display().to_string(), name));
        }
    }
    // A positive count: the outbound struct must exist, or the scan has
    // stopped seeing derives.
    assert_eq!(
        derived.len(),
        SERIALIZABLE.len(),
        "expected exactly {} `Serialize` derives in {SCANNED_DIR} ({SERIALIZABLE:?}), found {derived:?}",
        SERIALIZABLE.len()
    );
    for (file, name) in &derived {
        assert!(
            SERIALIZABLE.contains(&name.as_str()),
            "{file}: `{name}` derives Serialize. Types holding rows, cells, level labels, manuscript text or a key \
             must not; only the outbound struct may"
        );
        assert!(file.ends_with("dataset.rs"), "{file}: the outbound struct lives in dataset.rs");
    }
}

/// The predicates must catch the edits they exist for, and must not trip on
/// a comment that names a route.
#[test]
fn the_guard_fails_on_a_file_that_reaches_a_model_the_proxy_the_helper_layer_or_a_file() {
    for offending in [
        "use crate::verify_agent::ProxyClient;\nfn ask(c: &dyn ProxyClient) {}\n",
        "use crate::embed::Embedder;\nfn near(e: &dyn Embedder) {}\n",
        "fn judge(b: &dyn crate::GenerationBackend) {}\n",
        "fn get(f: &dyn crate::refverify::HttpFetcher) {}\n",
        "use crate::stats_assist::TaskSpec;\n",
        "fn read(p: &str) -> String { std::fs::read_to_string(p).unwrap() }\n",
        "fn when() -> u64 { std::time::SystemTime::now().elapsed().unwrap().as_secs() }\n",
    ] {
        assert!(!violations("synthetic.rs", &code_only(offending)).is_empty(), "missed: {offending}");
    }
    let documented = "//! Level 1 never calls a proxy, an Embedder, std::fs or stats_assist.\npub fn pure(x: i32) -> i32 { x + 1 }\n";
    assert!(violations("synthetic.rs", &code_only(documented)).is_empty());

    let serialising = "#[derive(Debug, Clone, Serialize)]\npub struct Leak { pub rows: Vec<String> }\n";
    assert_eq!(serialize_derives(&code_only(serialising)), vec!["Leak".to_string()]);
    let attributed = "#[derive(Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct Named;\n";
    assert_eq!(serialize_derives(&code_only(attributed)), vec!["Named".to_string()]);
}
