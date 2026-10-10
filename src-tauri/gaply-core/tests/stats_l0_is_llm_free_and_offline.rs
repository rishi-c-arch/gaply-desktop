//! **Nothing in `src/stats_l0/` may name the cloud proxy, an HTTP fetcher, or
//! a generative or embedding model.**
//!
//! Level 0 judges a manuscript from the manuscript alone (`stats_l0/mod.rs`:
//! "no model, no proxy, no network"). That sentence is a claim in a doc
//! comment, and CLAUDE.md's purity-claim entry is about exactly that: a claim
//! nobody tests holds only until somebody tries. This is the scan that tests
//! it, in the shape of `equation_is_llm_free.rs`.
//!
//! The list names the TYPES this workspace actually has for each route,
//! enumerated from the source on 10 Oct 2026 (`pub trait|struct` matching
//! Proxy, Fetch, Http, Generat, Embed, Model, Judge), plus the crates behind
//! them. A new route under a new name is not caught; see below.
//!
//! # What it cannot catch, stated because the gap is real
//!
//! 1. **A route through a helper in another module.** The scan sees
//!    `crate::foo::bar()`, not what `bar` does.
//! 2. **A model type added later under a name this list does not hold.** The
//!    import allowlist below is the backstop for a new CRATE; a new
//!    `crate::`-internal type is not covered.
//! 3. **The app seam.** `src-tauri/src/stats_consistency.rs` runs Level 0 for
//!    the command and is outside this crate. Its own test
//!    `the_view_is_named_only_by_the_files_that_send_it_to_the_webview` ends
//!    by asserting that its code names no proxy identifier.

use std::path::{Path, PathBuf};

/// Every file that makes up Level 0.
const SCANNED_DIR: &str = "src/stats_l0";

/// Substrings that mean this file can reach the cloud, the network or a model.
const FORBIDDEN: &[(&str, &str)] = &[
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
];

/// The only crates Level 0 may import. `regex` reads the units, `sha2` digests
/// a method's typed inputs, and `statrs` gives `pvalue.rs` its distribution
/// tails (pure numerics; measured against R in `l0_p_reference.rs`). Anything
/// else has to be argued for here. `statrs` was missing from the first draft of
/// this list, and the first run said so.
const ALLOWED_EXTERNAL_IMPORTS: &[&str] = &["std", "regex", "sha2", "statrs"];

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has Level 0 moved?", dir.display());
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

/// Strip `//` line comments, so a doc comment NAMING a route ("no proxy") is
/// not mistaken for taking it.
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

#[test]
fn stats_l0_is_llm_free_and_offline() {
    let files = rust_files(Path::new(SCANNED_DIR));
    // 13 modules on 10 Oct 2026. A scan that silently finds fewer passes
    // vacuously on what it no longer reads.
    assert!(
        files.len() >= 13,
        "the scan found {} files — it would pass vacuously",
        files.len()
    );

    let mut found = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read");
        found.extend(violations(&f.display().to_string(), &code_only(&src)));
    }
    assert!(
        found.is_empty(),
        "Level 0 judges from the manuscript alone: no proxy, no HTTP, no model. Found:\n{}",
        found.join("\n")
    );
}

#[test]
fn stats_l0_imports_only_std_regex_sha2_and_statrs() {
    let mut imports = 0;
    for f in rust_files(Path::new(SCANNED_DIR)) {
        let src = std::fs::read_to_string(&f).expect("read");
        for (n, line) in code_only(&src).lines().enumerate() {
            let t = line.trim();
            let Some(rest) = t.strip_prefix("use ") else {
                continue;
            };
            imports += 1;
            let root = rest.split(['{', ':', ';', ' ']).next().unwrap_or("").trim();
            if root.is_empty() || root == "crate" || root == "super" || root == "self" {
                continue;
            }
            assert!(
                ALLOWED_EXTERNAL_IMPORTS.contains(&root),
                "{}:{}: `{root}` is not on the Level 0 import allowlist\n    {t}",
                f.display(),
                n + 1
            );
        }
    }
    assert!(
        imports > 0,
        "no `use` line found — the import scan is inert"
    );
}

/// The predicate must catch the edit it exists for, and must not trip on a
/// comment that names a route.
#[test]
fn the_guard_fails_on_a_file_that_reaches_a_model_or_the_proxy() {
    for offending in [
        "use crate::verify_agent::ProxyClient;\nfn ask(c: &dyn ProxyClient) {}\n",
        "use crate::embed::Embedder;\nfn near(e: &dyn Embedder) {}\n",
        "fn judge(b: &dyn crate::GenerationBackend) {}\n",
        "fn get(f: &dyn crate::refverify::HttpFetcher) {}\n",
    ] {
        assert!(
            !violations("synthetic.rs", &code_only(offending)).is_empty(),
            "missed: {offending}"
        );
    }
    let documented = "//! Level 0 never calls a proxy, an Embedder or an LLM.\npub fn pure(x: i32) -> i32 { x + 1 }\n";
    assert!(violations("synthetic.rs", &code_only(documented)).is_empty());
}
