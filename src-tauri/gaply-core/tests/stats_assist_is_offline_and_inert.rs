//! **`src/stats_assist/` has no HTTP, no network, no file, no clock, no model
//! runtime and no code execution; it serialises only its outbound structs and
//! the request body around them; and it names no path by which a `stats_l1`
//! decision is made.**
//!
//! `stats_assist/mod.rs` claims all of that in prose. This is the scan that
//! tests it, in the shape of `stats_l1_is_llm_free_and_offline.rs`.
//!
//! 1. Infrastructure: an HTTP client, the network, the filesystem, a path, a
//!    database, the clock, a process, a model runtime or an embedding. The
//!    transport is a trait here; Step 6 implements it in the app crate.
//! 2. Decisions: a confirmed binding, a token, the run state, a binding draft
//!    or proposal, a variant choice, the compute and comparison functions,
//!    and a Level 1 verdict outside a match arm. Together with private fields
//!    on `ConfirmedBinding` and the token store, and validators that return
//!    this module's own types, this is what makes "no model output can create,
//!    change or suppress a verdict" hold without anyone remembering it.
//! 3. `Serialize` only on the names in `SERIALIZABLE`, and serialisation
//!    calls only in `outbound.rs` (the request) and `response.rs` (reading
//!    back what was sent, to check an explanation's numbers).
//! 4. Imports only from `std`, `serde` and `serde_json`.
//! 5. No function tools: the string `"function"` does not occur.
//!
//! What it cannot see: a decision made through a helper in another module (a
//! `crate::` import is not followed), or a type renamed to dodge the list.
//! Only production code is read (up to each file's first `#[cfg(test)]`), and
//! `//` comments are stripped, so a doc comment naming a forbidden thing is
//! not a violation.

use std::path::{Path, PathBuf};

const SCANNED_DIR: &str = "src/stats_assist";

const FORBIDDEN: &[(&str, &str)] = &[
    // Infrastructure.
    ("reqwest", "an HTTP client"),
    ("ureq", "an HTTP client"),
    ("hyper", "an HTTP client"),
    ("HttpFetcher", "an HTTP fetcher"),
    ("ReqwestFetcher", "an HTTP fetcher"),
    ("ProxyClient", "the cloud proxy"),
    ("std::net", "the network"),
    ("TcpStream", "the network"),
    ("std::fs", "the filesystem"),
    ("File::", "the filesystem"),
    ("std::path", "a path"),
    ("PathBuf", "a path"),
    ("rusqlite", "the database"),
    ("Connection", "the database"),
    ("SystemTime", "the clock"),
    ("Instant::now", "the clock"),
    ("now_epoch", "the clock"),
    ("std::process", "a process"),
    ("Command::", "a process"),
    ("candle", "a model runtime"),
    ("tokenizers", "a model runtime"),
    ("PerplexityModel", "a language model"),
    ("GenerationBackend", "a generative model"),
    ("Embedder", "an embedding model"),
    ("keyring", "the keychain"),
    ("secrets::", "the keychain"),
    ("\"function\"", "a function tool"),
    // Decisions that belong to stats_l1 and the user.
    ("ConfirmedBinding", "a confirmed binding"),
    ("ConfirmationToken", "a confirmation token"),
    ("TokenStore", "the token store"),
    ("TokenRecord", "a token record"),
    // The run state by name. Its module path is NOT forbidden: `codes.rs`
    // imports `GateRejection` and `StaleField` from it to name reasons.
    ("L1RunState", "the run state"),
    ("BindingDraft", "a binding draft"),
    ("BindingProposal", "a binding proposal"),
    ("ProposalSource", "a proposal source"),
    ("VariantChoice", "a variant decision"),
    // The compute function itself. The module path is not forbidden: `codes.rs`
    // imports the refusal and variant enums from it.
    ("compute(", "a computation"),
    ("compute::compute", "a computation"),
    ("L1Computation", "a computation"),
    ("ComputedResult", "a computed value"),
    ("reproduce(", "a reproduction"),
    ("reproduce_claim", "a reproduction"),
    ("compare_claim", "a comparison"),
    ("compare_p", "a comparison"),
    ("decide(", "a verdict"),
];

/// Named only in match arms (`=>` on the line): reading a verdict is how an
/// explanation's input is built; making one is not allowed.
const READ_ONLY_IN_MATCH_ARMS: &[&str] = &["L1Verdict::", "ClaimReproduction {"];

const ALLOWED_EXTERNAL_IMPORTS: &[&str] = &["std", "serde", "serde_json"];

/// The outbound structs and their row types, and the request body types.
const SERIALIZABLE: &[&str] = &[
    "ResultFieldsOut",
    "CatalogueEntryOut",
    "ClaimMethodOutbound",
    "RoleSlotOut",
    "HeaderOut",
    "BindingSuggestionOutbound",
    "NumberOut",
    "ExclusionOut",
    "ResultExplanationOutbound",
    "WebContextOutbound",
    // `WireCode` and `Param` moved to `stats_l1::codes` in Step 7 and are
    // re-exported from `codes.rs` here; they are no longer declared here.
    "ReasoningParam",
    "JsonSchemaFormat",
    "TextParam",
    "WebSearchFilters",
    "WebSearchTool",
    "ResponsesBody",
];

const SERIALISING_FILES: &[&str] = &["outbound.rs", "response.rs"];

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        panic!("{} is not readable — has the helper layer moved?", dir.display());
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

/// Production code only, `//` comments stripped.
fn production_code(src: &str) -> String {
    let prod = match src.find("#[cfg(test)]") {
        Some(i) => &src[..i],
        None => src,
    };
    prod.lines()
        .map(|l| match l.find("//") {
            Some(i) if !l[..i].ends_with("https:") => &l[..i],
            _ => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A type-name needle (a bare identifier starting upper-case) matches only
/// as a whole identifier (`ComputedResult`, not `NoComputedResult`). Every
/// other needle is a substring, so `candle` still catches `candle_core`.
fn hits(line: &str, needle: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    if !needle.starts_with(|c: char| c.is_ascii_uppercase()) || !needle.chars().all(ident) {
        return line.contains(needle);
    }
    line.match_indices(needle).any(|(i, _)| {
        !line[..i].chars().next_back().is_some_and(ident) && !line[i + needle.len()..].chars().next().is_some_and(ident)
    })
}

fn violations(path: &str, code: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (n, line) in code.lines().enumerate() {
        for (needle, what) in FORBIDDEN {
            if hits(line, needle) {
                out.push(format!("{path}:{}: `{needle}` — {what}\n    {}", n + 1, line.trim()));
            }
        }
        for needle in READ_ONLY_IN_MATCH_ARMS {
            if line.contains(needle) && !line.contains("=>") {
                out.push(format!("{path}:{}: `{needle}` outside a match arm — only the engine makes one\n    {}", n + 1, line.trim()));
            }
        }
    }
    out
}

fn serialize_derives(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        if t.starts_with("#[derive(") && t.split(|c: char| !c.is_alphanumeric()).any(|w| w == "Serialize") {
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
    out.dedup();
    out
}

#[test]
fn stats_assist_is_offline_and_makes_no_decision() {
    let files = rust_files(Path::new(SCANNED_DIR));
    // 10 files on 14 Oct 2026. Fewer, and the scan is reading something else.
    assert!(files.len() >= 10, "the scan found {} files — it would pass vacuously", files.len());
    let mut found = Vec::new();
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read");
        found.extend(violations(&f.display().to_string(), &production_code(&src)));
    }
    assert!(
        found.is_empty(),
        "The helper layer builds requests and reads replies; it has no transport, no file, no clock, \
         no model, and it makes none of Level 1's decisions. Found:\n{}",
        found.join("\n")
    );
}

#[test]
fn only_the_outbound_structs_and_the_body_serialise() {
    let mut seen = Vec::new();
    for f in rust_files(Path::new(SCANNED_DIR)) {
        let src = std::fs::read_to_string(&f).expect("read");
        let code = production_code(&src);
        for name in serialize_derives(&code) {
            assert!(SERIALIZABLE.contains(&name.as_str()), "{}: `{name}` derives Serialize and is not an outbound struct", f.display());
            seen.push(name);
        }
        let file = f.file_name().unwrap().to_string_lossy().to_string();
        if !SERIALISING_FILES.contains(&file.as_str()) {
            assert!(!code.contains("serde_json::to_"), "{}: serialises, and only outbound.rs and response.rs may", f.display());
        }
    }
    seen.sort();
    let mut expect: Vec<String> = SERIALIZABLE.iter().map(|s| s.to_string()).collect();
    expect.sort();
    assert_eq!(seen, expect, "every allow-listed type is found exactly once — a shorter list means the scan stopped matching");
}

#[test]
fn stats_assist_imports_only_std_serde_and_serde_json() {
    let mut imports = 0;
    for f in rust_files(Path::new(SCANNED_DIR)) {
        let src = std::fs::read_to_string(&f).expect("read");
        for (n, line) in production_code(&src).lines().enumerate() {
            let Some(rest) = line.trim().strip_prefix("use ") else { continue };
            imports += 1;
            let root = rest.split(['{', ':', ';', ' ']).next().unwrap_or("").trim();
            if matches!(root, "" | "crate" | "super" | "self") {
                continue;
            }
            assert!(ALLOWED_EXTERNAL_IMPORTS.contains(&root), "{}:{}: `{root}` is not on the import allowlist", f.display(), n + 1);
        }
    }
    assert!(imports > 0, "no `use` line found — the import scan is inert");
}

/// The scan trips on what it is for, and not on a comment.
#[test]
fn the_scan_catches_what_it_is_for() {
    let bad = "let b = ConfirmedBinding::new();\nlet v = L1Verdict::Reproduced(vec![]);\nlet c = reqwest::Client::new();\nlet r = compute(&s, &t, id, &d);";
    let v = violations("x.rs", &production_code(bad));
    assert_eq!(v.len(), 4, "{v:?}");
    let import = "use crate::stats_l1::compute::{L1Refusal, OlsTerm};\nuse crate::stats_l1::run_state::{GateRejection, StaleField};";
    assert!(violations("x.rs", &production_code(import)).is_empty(), "importing reason enums is allowed");
    let gap = "        StatisticGap::NoComputedResult => c(\"no_computed_result\"),";
    assert!(violations("x.rs", &production_code(gap)).is_empty(), "an identifier containing a needle is not the needle");
    assert_eq!(violations("x.rs", "let r: ComputedResult = x;").len(), 1);
    assert_eq!(violations("x.rs", "let t = candle_core::Tensor::zeros(1);").len(), 1, "lower-case needles stay substrings");
    let arm = "        L1Verdict::Reproduced(_) => \"reproduced\",";
    assert!(violations("x.rs", &production_code(arm)).is_empty());
    let comment = "/// a ConfirmedBinding is made only by confirm\nlet x = 1; // reqwest lives in the app crate";
    assert!(violations("x.rs", &production_code(comment)).is_empty());
    let url = "let u = \"https://bmj.com\"; // fine";
    assert!(violations("x.rs", &production_code(url)).is_empty());
    let test_only = "fn f() {}\n#[cfg(test)]\nmod tests { use std::fs; }";
    assert!(violations("x.rs", &production_code(test_only)).is_empty());
    assert_eq!(serialize_derives("#[derive(Debug, Serialize)]\npub struct Leak {"), vec!["Leak"]);
    assert!(serialize_derives("#[derive(Debug, Deserialize)]\nstruct Wire {").is_empty());
}
