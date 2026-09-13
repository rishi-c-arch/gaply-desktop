//! **Regexes in the extraction passes are compiled once, not per sentence.**
//!
//! # What this guards, and why it is a source scan
//!
//! Measured 13 Sep 2026 (`examples/scientific_cost_probe.rs`): the opt-in
//! scientific layer cost **0.8–14.5 s per manuscript** against 1–70 ms for base
//! extraction. The whole of it was `Regex::new` called inside functions that run
//! **once per sentence** — `datasets.rs` compiled 21 patterns per sentence,
//! `methods.rs` 35 in two loops plus 8 one-off. The pattern LISTS were
//! `OnceLock`-cached; the compiled regexes were not.
//!
//! The cost was linear in sentences and completely independent of what the
//! passes found — 4.2–7.5 ms/sentence across a 22x range of inputs while match
//! counts varied 2 to 79. That signature is work done before matching.
//!
//! # WHY A SOURCE SCAN AND NOT A TIMING TEST
//!
//! A timing test measures the thing that actually matters and is the obvious
//! instrument. It was rejected because **a per-sentence threshold is flaky on
//! shared CI runners** — the Windows job and the clean-checkout job run on
//! whatever hardware GitHub gives them, and a test that fails on a noisy
//! neighbour trains people to re-run it, which is worse than no test. It would
//! also need a committed manuscript large enough to time, and the honest ones
//! are a researcher's unpublished work.
//!
//! So this asserts the STRUCTURAL cause instead: no `Regex` is constructed
//! outside a cache initialiser in these files. That is checkable, deterministic,
//! and fails on the exact edit that caused the regression.
//!
//! # WHAT THIS CANNOT CATCH — stated plainly, because the gap is real
//!
//! 1. **Expensive per-sentence work that is not a regex.** The same files
//!    rebuilt a `HashSet` of 21 lowercased names on every call
//!    (`known_dataset_names`), and nothing here would have noticed. Allocation,
//!    `to_lowercase()` on every sentence, and repeated `sentences_in` are all
//!    invisible to it.
//! 2. **A regex compiled in a helper this scan exempts.** `compile_word_patterns`
//!    carries a `REGEX-CACHE-OK` marker; if someone calls it per sentence the
//!    marker still passes.
//! 3. **Any regression outside these four files.** The scan is scoped to the
//!    passes that were measured.
//! 4. **A slow regex.** Compiling once says nothing about a pattern that
//!    backtracks badly at match time.
//!
//! The probe is the instrument for all four. This is the cheap guard that fails
//! fast on the specific mistake that was actually made; re-running
//! `scientific_cost_probe` is what confirms the cost.

use std::path::Path;

/// The passes the probe measured.
const SCANNED: &[&str] = &[
    "src/extract/datasets.rs",
    "src/extract/methods.rs",
    "src/extract/variables.rs",
    "src/extract/claims.rs",
];

/// A line carrying this marker may construct a `Regex` outside a cache. Each
/// use must state why on the same line, so an exemption is a deliberate,
/// readable decision rather than a silent one.
const EXEMPT: &str = "REGEX-CACHE-OK";

/// Byte ranges of every `get_or_init(...)` closure body, found by brace
/// counting from the opening brace that follows it.
fn cache_init_spans(src: &str) -> Vec<(usize, usize)> {
    let bytes = src.as_bytes();
    let mut spans = Vec::new();
    let mut from = 0;
    while let Some(rel) = src[from..].find("get_or_init") {
        let at = from + rel;
        // Find the closure's opening brace, then match it.
        if let Some(open_rel) = src[at..].find('{') {
            let open = at + open_rel;
            let mut depth = 0usize;
            let mut i = open;
            while i < bytes.len() {
                match bytes[i] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            spans.push((open, i));
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
        }
        from = at + "get_or_init".len();
    }
    spans
}

fn line_of(src: &str, idx: usize) -> (usize, &str) {
    let line_no = src[..idx].matches('\n').count() + 1;
    let start = src[..idx].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = src[idx..].find('\n').map(|i| idx + i).unwrap_or(src.len());
    (line_no, &src[start..end])
}

#[test]
fn no_regex_is_compiled_outside_a_cache_in_the_extraction_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offences: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for rel in SCANNED {
        let path = root.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{rel} must be readable: {e}"));
        let spans = cache_init_spans(&src);

        for ctor in ["Regex::new(", "RegexBuilder::new("] {
            let mut from = 0;
            while let Some(r) = src[from..].find(ctor) {
                let at = from + r;
                from = at + ctor.len();
                checked += 1;
                let (line_no, line) = line_of(&src, at);
                // A mention inside a comment is prose, not a construction.
                if line.trim_start().starts_with("//") {
                    checked -= 1;
                    continue;
                }
                if line.contains(EXEMPT) {
                    continue;
                }
                // Single-expression initialiser: `RE.get_or_init(|| Regex::new(..))`
                // has no braces, so the span finder cannot see it. Checking the
                // line itself covers that form without parsing parentheses —
                // which would need string-literal awareness, since the patterns
                // themselves are full of them.
                if line.contains("get_or_init") {
                    continue;
                }
                if spans.iter().any(|(a, b)| at > *a && at < *b) {
                    continue;
                }
                offences.push(format!("{rel}:{line_no}: {}", line.trim()));
            }
        }
    }

    // The scan must have found the constructors it is guarding. A guard that
    // silently matched nothing would pass forever — the failure mode this
    // project keeps meeting (a uniformly clean result from a broken
    // instrument).
    assert!(
        checked >= 20,
        "the scan found only {checked} regex constructors across {} files — it is not \
         looking at what it thinks it is",
        SCANNED.len()
    );

    assert!(
        offences.is_empty(),
        "Regex constructed outside a cache initialiser in an extraction pass.\n\
         These functions run ONCE PER SENTENCE; compiling there cost 0.8-14.5 s per \
         manuscript before it was fixed (see examples/scientific_cost_probe.rs).\n\
         Move the pattern into the `OnceLock` beside its list, or mark the line \
         `{EXEMPT}: <reason>` if it genuinely cannot be cached.\n\n  {}",
        offences.join("\n  ")
    );
}

/// The spans finder is itself a small parser, so it gets a test: if it silently
/// returned nothing, every `Regex::new` would read as an offence and the guard
/// above would fail loudly — but if it returned one enormous span, every
/// offence would be swallowed and the guard would pass forever. The second
/// failure is the dangerous one.
#[test]
fn the_cache_span_finder_brackets_only_the_initialiser() {
    let src = r#"
fn a() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| { Regex::new("x").unwrap() })
}
fn b(s: &str) -> bool {
    let re = Regex::new("y").unwrap();
    re.is_match(s)
}
"#;
    let spans = cache_init_spans(src);
    assert_eq!(spans.len(), 1, "exactly one get_or_init closure: {spans:?}");
    let inside = src.find("Regex::new(\"x\")").unwrap();
    let outside = src.find("Regex::new(\"y\")").unwrap();
    let (a, b) = spans[0];
    assert!(inside > a && inside < b, "the cached one must be inside the span");
    assert!(
        !(outside > a && outside < b),
        "the uncached one must be OUTSIDE — a span that swallowed it would make the \
         guard pass on every future offence"
    );
}
