//! **Nothing in `src/equation/` may reach a model, a proxy, the network, the
//! database or the filesystem.**
//!
//! # Why this is a test and not a sentence in a doc comment
//!
//! §4.4 puts Tier 0 above every model in the system: a deterministic finding
//! overrides eight agents agreeing. That authority is only earned if the path
//! producing it is deterministic, and "it is deterministic" is currently a
//! claim made in prose by three modules — `validate`, `stats_verify`,
//! `stats_verdict` — none of which has anything checking it. The claim has held
//! because nobody has tried, and §11 D128's lesson is that knowing a rule is
//! not the same as having guarded it.
//!
//! The instruction this was written to was *"assert structurally that no LLM is
//! on this path, the way the startup module asserts no HTTP client."*
//! **There is no such assertion to copy.** `gaply_core` genuinely has no HTTP
//! client — `refverify.rs`'s header records the injected-`HttpFetcher` seam and
//! `Cargo.toml` has no `reqwest`/`ureq` — but that is a property of the
//! dependency list that nothing tests, and nothing anywhere in the workspace
//! asserts the absence of a model either. So this is the first of its kind
//! rather than a second copy of one; that is worth stating plainly, because a
//! reader told it follows an existing pattern would go looking for the pattern.
//!
//! # What it cannot catch, stated because the gap is real
//!
//! 1. **A model reached through a helper in another module.** The scan sees
//!    `crate::foo::bar()`, not what `bar` does. It catches the direct route,
//!    which is the one an edit actually takes.
//! 2. **Non-determinism that is not a model.** `SystemTime::now`, a
//!    `HashMap` iteration order leaking into output, a thread race. The first
//!    is banned below; the others are not visible to a text scan.
//! 3. **A dependency that becomes model-backed later.** This module uses only
//!    `serde` and `std`.

use std::path::Path;

/// Every file that makes up the Tier-0 engine.
const SCANNED_DIR: &str = "src/equation";

/// Substrings that mean this file can reach something outside pure computation.
///
/// Deliberately a BLACKLIST of routes, paired with the whitelist of imports
/// below — a blacklist alone would miss a new crate, and a whitelist alone
/// would miss a `crate::`-internal route to a model.
const FORBIDDEN: &[(&str, &str)] = &[
    ("reqwest", "an HTTP client"),
    ("ureq", "an HTTP client"),
    ("ProxyClient", "the cloud proxy"),
    ("proxy", "the cloud proxy"),
    ("PerplexityModel", "a model"),
    ("Embedder", "an embedding model"),
    ("embed(", "an embedding model"),
    ("llm", "a model"),
    ("Llm", "a model"),
    ("generative", "a model"),
    ("candle", "a model runtime"),
    ("tokenizers", "a model runtime"),
    ("rusqlite", "the database"),
    ("Connection", "the database"),
    ("std::fs", "the filesystem"),
    ("File::", "the filesystem"),
    ("std::net", "the network"),
    ("SystemTime", "the clock — a Tier-0 result must not depend on when it ran"),
    ("Instant::now", "the clock"),
    ("thread_rng", "an entropy source — Tier 0 is reproducible"),
    ("random", "an entropy source"),
];

/// The only crates the engine may import. Anything else has to be argued for
/// in this list, which is the point: the decision becomes visible.
const ALLOWED_EXTERNAL_IMPORTS: &[&str] = &["serde", "std"];

fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has the engine moved?", dir.display());
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

/// Strip `//` line comments so a doc comment NAMING a forbidden thing — this
/// module's own headers do, repeatedly — is not mistaken for using it.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_model_proxy_network_or_io_is_reachable_from_the_tier_zero_engine() {
    let files = rust_files(Path::new(SCANNED_DIR));
    assert!(!files.is_empty(), "the scan found no files — it would pass vacuously");

    let mut violations = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read");
        let code = code_only(&src);
        for (needle, what) in FORBIDDEN {
            for (n, line) in code.lines().enumerate() {
                if line.contains(needle) {
                    violations.push(format!(
                        "{}:{}: `{needle}` — {what}\n    {}",
                        f.display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "Tier 0 must be pure computation (§4.4, §6b). Found:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_engine_imports_nothing_but_std_and_serde() {
    for f in rust_files(Path::new(SCANNED_DIR)) {
        let src = std::fs::read_to_string(&f).expect("read");
        for (n, line) in code_only(&src).lines().enumerate() {
            let t = line.trim();
            let Some(rest) = t.strip_prefix("use ") else { continue };
            let root = rest
                .split(['{', ':', ';', ' '])
                .next()
                .unwrap_or("")
                .trim();
            if root.is_empty() || root == "crate" || root == "super" || root == "self" {
                continue;
            }
            assert!(
                ALLOWED_EXTERNAL_IMPORTS.contains(&root),
                "{}:{}: `{root}` is not on the Tier-0 import allowlist\n    {t}",
                f.display(),
                n + 1
            );
        }
    }
}

/// The guard must fail on the edit it exists to catch. A guard whose first run
/// is green proves nothing about whether it gates (CLAUDE.md's standing rule),
/// so this drives the same predicate over a synthetic file that commits the
/// violation, rather than requiring someone to break the tree by hand.
#[test]
fn the_guard_fails_on_a_file_that_reaches_a_model() {
    let offending = "use crate::perplexity::PerplexityModel;\n\
                     fn judge(m: &dyn PerplexityModel) -> bool { m.score(\"x\") > 1.0 }\n";
    let code = code_only(offending);
    let hits: Vec<&str> = FORBIDDEN
        .iter()
        .filter(|(n, _)| code.contains(n))
        .map(|(n, _)| *n)
        .collect();
    assert!(!hits.is_empty(), "the scan would not have caught a model import");

    // And a doc comment that merely NAMES one must not trip it — otherwise the
    // guard would forbid this module from explaining itself.
    let documented = "//! This module never calls reqwest or a PerplexityModel.\n\
                      pub fn pure(x: i32) -> i32 { x + 1 }\n";
    let code = code_only(documented);
    assert!(
        !FORBIDDEN.iter().any(|(n, _)| code.contains(n)),
        "a doc comment naming a forbidden route must not be a violation"
    );
}
