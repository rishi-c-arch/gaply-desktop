//! **Every model instruction must pass the proxy's prose guard.**
//!
//! `gaply-proxy/app/validation.py` rejects any string in a `/verify` payload
//! that is longer than `PROSE_MIN_FIELD_CHARS` and has more than
//! `MAX_SENTENCES_PER_FIELD` sentence ends, and any string longer than
//! `MAX_FIELD_CHARS`. The rule exists to keep manuscript prose off the wire,
//! and it applies to every string in the payload, **our own instruction
//! included**.
//!
//! The citation `INSTRUCTION` in `verify_agent.rs` grew to 10 sentence ends on
//! 93f3ca3 (11 Jul). Every citation call was then refused with a 422, hidden
//! until batching (6 Aug) brought requests under the size limit, which the
//! proxy checks first. Nothing on the client could see it: the client's budget
//! copied the size rule and not this one. The guard is not relaxed for our
//! prompt; the prompt conforms, and this test holds every instruction to it.
//!
//! **The rule is read from the proxy, not copied.** The thresholds and the
//! sentence regex are parsed out of `validation.py`, so a change there is a
//! change here. `grrb_context_pool_is_the_fixed_set.rs` reads the same file
//! for the GRRB context payload; this test covers the instructions.
//!
//! **Scope.** Every `const *_INSTRUCTION: &str` in `gaply-core/src` and the app
//! crate's `src`, which is every instruction this codebase sends to `/verify`
//! (and `CLASSIFY_INSTRUCTION`, sent to a local model, which must pass anyway).
//! The scan must find at least `MIN_INSTRUCTIONS` of them, so a matcher that
//! stops matching fails rather than passing on nothing.

use std::path::{Path, PathBuf};

/// Measured 8 Oct 2026: 12 in `gaply-core/src`, 1 in the app crate.
const MIN_INSTRUCTIONS: usize = 13;

struct ProxyRule {
    sentence: regex::Regex,
    max_sentences: usize,
    prose_min_chars: usize,
    max_field_chars: usize,
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is src-tauri/gaply-core.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn int_constant(py: &str, name: &str) -> usize {
    let re = regex::Regex::new(&format!(r"(?m)^{name}\s*=\s*(\d+)")).unwrap();
    re.captures(py)
        .unwrap_or_else(|| panic!("{name} not found in validation.py — the guard can no longer read the proxy's rule"))[1]
        .parse()
        .unwrap()
}

fn proxy_rule() -> ProxyRule {
    let path = repo_root().join("gaply-proxy/app/validation.py");
    let py = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let pattern = regex::Regex::new(r#"_SENTENCE\s*=\s*re\.compile\(r"([^"]+)"\)"#)
        .unwrap()
        .captures(&py)
        .expect("_SENTENCE regex not found in validation.py")[1]
        .to_string();
    ProxyRule {
        sentence: regex::Regex::new(&pattern).expect("the proxy's sentence regex compiles in Rust"),
        max_sentences: int_constant(&py, "MAX_SENTENCES_PER_FIELD"),
        prose_min_chars: int_constant(&py, "PROSE_MIN_FIELD_CHARS"),
        max_field_chars: int_constant(&py, "MAX_FIELD_CHARS"),
    }
}

/// The value of a Rust string literal whose body starts at `body` (just after
/// the opening quote). Handles the escapes these constants use; anything else
/// panics, so an unhandled escape is a loud failure and never a wrong count.
fn literal_value(body: &str) -> String {
    let mut out = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => return out,
            '\\' => match chars.next() {
                // Line continuation: the newline (CRLF on a Windows checkout)
                // and the next line's leading whitespace are removed.
                Some('\n') | Some('\r') => {
                    while matches!(chars.peek(), Some(w) if w.is_whitespace()) {
                        chars.next();
                    }
                }
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                other => panic!("unhandled escape \\{other:?} in an instruction literal"),
            },
            _ => out.push(c),
        }
    }
    panic!("unterminated string literal");
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// `(file, name, value)` for every `const *_INSTRUCTION: &str = "…";`.
fn instructions() -> Vec<(String, String, String)> {
    let decl = regex::Regex::new(r#"const (\w*INSTRUCTION\w*): &str = ""#).unwrap();
    let root = repo_root();
    let mut files = Vec::new();
    rust_files(&root.join("src-tauri/gaply-core/src"), &mut files);
    rust_files(&root.join("src-tauri/src"), &mut files);
    let mut found = Vec::new();
    for file in files {
        let src = std::fs::read_to_string(&file).unwrap();
        for m in decl.captures_iter(&src) {
            let whole = m.get(0).unwrap();
            let value = literal_value(&src[whole.end()..]);
            let name = file.strip_prefix(&root).unwrap_or(&file).display().to_string();
            found.push((name, m[1].to_string(), value));
        }
    }
    found
}

#[test]
fn every_instruction_passes_the_proxy_prose_guard() {
    let rule = proxy_rule();
    let found = instructions();
    assert!(
        found.len() >= MIN_INSTRUCTIONS,
        "found only {} instruction constants (expected at least {MIN_INSTRUCTIONS}) — \
         the scan has stopped matching and would pass on nothing",
        found.len()
    );

    let mut rejected = Vec::new();
    for (file, name, value) in &found {
        let chars = value.chars().count();
        let ends = rule.sentence.find_iter(value).count();
        if chars > rule.max_field_chars {
            rejected.push(format!("{file}: {name} is {chars} chars (limit {})", rule.max_field_chars));
        } else if chars > rule.prose_min_chars && ends > rule.max_sentences {
            rejected.push(format!(
                "{file}: {name} has {ends} sentence ends in {chars} chars (the proxy allows {} \
                 past {} chars) — every call carrying it gets a 422",
                rule.max_sentences, rule.prose_min_chars
            ));
        }
    }
    assert!(rejected.is_empty(), "instructions the proxy would refuse:\n{}", rejected.join("\n"));
}

/// The citation instruction specifically, since it is the one that failed.
#[test]
fn the_citation_instruction_is_found_and_passes() {
    let rule = proxy_rule();
    let found = instructions();
    let (_, _, value) = found
        .iter()
        .find(|(f, n, _)| f.ends_with("verify_agent.rs") && n == "INSTRUCTION")
        .expect("verify_agent.rs INSTRUCTION not found by the scan");
    let ends = rule.sentence.find_iter(value).count();
    assert!(
        ends <= rule.max_sentences,
        "the citation INSTRUCTION has {ends} sentence ends; the proxy allows {}",
        rule.max_sentences
    );
}
