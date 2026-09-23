//! SPIKE — Layer 2 claim-extraction feasibility. NOT production code. Safe to delete.
//!
//! Question: can a locally-runnable model, inside the 8GB budget, extract a
//! manuscript's claims reliably enough to build Review Engines on top of?
//!
//! Design = Option 2 (deterministic candidates + surprisal classification), chosen
//! over generative extraction because it structurally cannot invent a claim the
//! paper never made: every candidate is a real sentence with a real anchor.
//!   1. Parse + extract with the REAL gaply-core path (docparse + extract_from_text).
//!   2. Split paragraphs into sentences DETERMINISTICALLY (no model).
//!   3. Apply a deliberately SHALLOW cue filter (no model) — shallow on purpose, so
//!      the failure analysis can tell whether cue filtering or the template set is
//!      the weak point.
//!   4. Classify each survivor by scoring label templates with the EXISTING
//!      `PerplexityModel::surprisals` — no decoder written, no generation.
//!
//! Shape: the Sentence is the atomic unit; annotations are a collection on it, so a
//! second annotation type later arrives as a new enum variant rather than a schema
//! change. Today `Annotation` has exactly one variant. This is a container choice,
//! NOT a framework — there is no registry, no dispatch, no extractor trait here.
//!
//! Provenance anchors on `sentence:sN`, which survives classifier changes; claim IDs
//! do not.
//!
//! Usage:
//!   cargo run --release --example claim_extraction_spike -- --dump <manuscript>
//!       Deterministic sentence inventory with anchors. NO model is loaded. Used to
//!       produce the hand-annotation worksheet BEFORE any classifier is run.
//!   cargo run --release --example claim_extraction_spike -- --classify <manuscript>
//!       Runs the classifier, prints claims in the typed shape, timing and peak RSS.
//!   cargo run --release --example claim_extraction_spike -- --score <manuscript> <annotations.tsv>
//!       Classify, then score against the hand annotations.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use gaply_core::ai_detect::PerplexityModel;
use gaply_core::extract::sections::SectionKind;
use gaply_core::extract::{extract_from_text, ExtractionResult};

// ---------------------------------------------------------------------------
// Typed shape (A3). Sentence is atomic; annotations are a collection on it.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum ClaimKind {
    Empirical,
    Causal,
    Comparative,
    Methodological,
}

impl ClaimKind {
    fn tag(&self) -> &'static str {
        match self {
            ClaimKind::Empirical => "empirical",
            ClaimKind::Causal => "causal",
            ClaimKind::Comparative => "comparative",
            ClaimKind::Methodological => "methodological",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "empirical" => Some(ClaimKind::Empirical),
            "causal" => Some(ClaimKind::Causal),
            "comparative" => Some(ClaimKind::Comparative),
            "methodological" => Some(ClaimKind::Methodological),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hedge {
    Asserted,
    Hedged,
    Speculative,
}

impl Hedge {
    fn tag(&self) -> &'static str {
        match self {
            Hedge::Asserted => "asserted",
            Hedge::Hedged => "hedged",
            Hedge::Speculative => "speculative",
        }
    }
}

/// One annotation on a sentence. TODAY there is exactly one variant. A second
/// (Limitation, Result, Method) arrives as a new variant, not a schema change —
/// and a sentence that is both a claim and a limitation is representable.
#[derive(Debug, Clone)]
enum Annotation {
    Claim {
        kind: ClaimKind,
        hedge: Hedge,
        /// Indices into `ExtractionResult.statistics`.
        supporting_stats: Vec<usize>,
        /// Indices into `ExtractionResult.citations`.
        adjacent_citations: Vec<usize>,
    },
}

/// The atomic unit. Anchored by (section_index, paragraph, index) — section INDEX,
/// not SectionKind, because SectionKind is not unique (`Other` is a catch-all and
/// nothing forbids two Results sections), so (kind, paragraph) does not resolve.
#[derive(Debug, Clone)]
struct Sentence {
    /// Stable document-order id. This is what provenance anchors on: `sentence:sN`.
    id: usize,
    section_index: usize,
    section_kind: SectionKind,
    paragraph: usize,
    /// All original paragraph indices this sentence spans (line unwrapping can
    /// make one sentence cover several source lines). Used for exact linkage.
    source_paragraphs: Vec<usize>,
    index: usize,
    text: String,
    annotations: Vec<Annotation>,
}

// ---------------------------------------------------------------------------
// 1. Deterministic sentence splitting — no model
// ---------------------------------------------------------------------------

/// Tokens that end in '.' but do not end a sentence.
const ABBREVIATIONS: &[&str] = &[
    "et al.", "e.g.", "i.e.", "cf.", "vs.", "approx.", "ca.", "etc.", "Fig.", "Figs.", "Tab.",
    "No.", "Dr.", "Prof.", "Mr.", "Mrs.", "Ms.", "St.", "Jr.", "Sr.", "Eq.", "Eqs.", "ref.",
    "refs.", "Ref.", "min.", "max.", "sec.", "wt.", "vol.", "conc.", "temp.", "spp.", "sp.",
    "subsp.", "var.", "p.", "pp.", "ed.", "eds.", "Inc.", "Ltd.", "Co.", "U.S.", "U.K.",
];

/// Split a paragraph into sentences. Boundary = [.!?] + whitespace + (uppercase |
/// digit | quote). Suppressed after a known abbreviation, after a single-capital
/// initial ("J. Smith"), and inside decimals ("p = 0.03").
fn split_sentences(paragraph: &str) -> Vec<String> {
    let chars: Vec<char> = paragraph.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];
        if c == '.' || c == '!' || c == '?' {
            // Look ahead: whitespace then a capital/digit/quote?
            let mut j = i + 1;
            // Allow a closing bracket/quote to ride with the terminator.
            while j < chars.len() && matches!(chars[j], ')' | ']' | '"' | '\'' | '’' | '”') {
                j += 1;
            }
            let mut k = j;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            let has_space = k > j;
            let next_starts = k < chars.len()
                && (chars[k].is_uppercase() || chars[k].is_ascii_digit() || chars[k] == '"');

            if c == '.' {
                // Decimal: digit '.' digit
                let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
                let next_digit = i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
                if prev_digit && next_digit {
                    i += 1;
                    continue;
                }
                // Single-capital initial: whitespace/start, capital, '.'
                if i >= 1
                    && chars[i - 1].is_uppercase()
                    && (i == 1 || !chars[i - 2].is_alphanumeric())
                {
                    i += 1;
                    continue;
                }
                // Known abbreviation ending here. Word-boundary aware: "unaffected."
                // must NOT match the "ed." abbreviation, so the character before
                // the match has to be a non-alphanumeric (or the string start).
                let upto: String = chars[start..=i].iter().collect();
                let tail = upto.trim_end();
                let abbrev_hit = ABBREVIATIONS.iter().any(|a| {
                    if !tail.ends_with(a) {
                        return false;
                    }
                    let before = tail.len() - a.len();
                    before == 0
                        || !tail[..before]
                            .chars()
                            .next_back()
                            .map(|c| c.is_alphanumeric())
                            .unwrap_or(false)
                });
                if abbrev_hit {
                    i += 1;
                    continue;
                }
            }

            if has_space && next_starts {
                let s: String = chars[start..j].iter().collect();
                let s = s.trim().to_string();
                if !s.is_empty() {
                    out.push(s);
                }
                start = k;
                i = k;
                continue;
            }
        }
        i += 1;
    }

    let tail: String = chars[start..].iter().collect();
    let tail = tail.trim().to_string();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// SPIKE FINDING, compensated here: `docparse::parse_path` on a PDF yields one
/// `Section.paragraph` per PHYSICAL LINE, not per paragraph. Sentences therefore
/// arrive split across several "paragraphs" ("All three analogues raised total" /
/// "free amino acids, ..."). Measuring claim extraction on that would measure PDF
/// line wrapping instead. Unwrapping is deterministic and separable, so the spike
/// does it here rather than abandoning the measurement — and reports it as a real
/// result. See the Step D report.
///
/// A block is a run of lines joined with single spaces, closed when the
/// accumulated text ends in terminal punctuation. Character spans back to the
/// ORIGINAL paragraph indices are retained so statistic/citation linkage stays
/// exact rather than approximate.
struct Block {
    text: String,
    /// (start, end, original paragraph index) in `text`.
    spans: Vec<(usize, usize, usize)>,
}

/// Running headers/footers repeated on every page ("Page 3 of 13", the journal
/// banner) are page furniture, not manuscript content. Dropped deterministically:
/// an exact line repeated 3+ times in the document, or a `Page N of M` line.
fn page_furniture(paragraphs: &[(usize, &String)]) -> std::collections::HashSet<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (_, p) in paragraphs {
        *counts.entry(p.trim().to_string()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .filter(|(t, n)| {
            if *n >= 3 && !t.is_empty() {
                return true;
            }
            let l = t.to_lowercase();
            l.starts_with("page ") && l.contains(" of ")
        })
        .map(|(t, _)| t)
        .collect()
}

fn unwrap_lines(paragraphs: &[String], furniture: &std::collections::HashSet<String>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();

    for (p_idx, line) in paragraphs.iter().enumerate() {
        let t = line.trim();
        if t.is_empty() || furniture.contains(t) {
            continue;
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        let start = cur.len();
        cur.push_str(t);
        spans.push((start, cur.len(), p_idx));

        // A line ending in a known abbreviation ("… Etebari et al.") is a wrapped
        // line, not a closed block — closing there would fabricate a sentence
        // boundary the splitter itself correctly refuses to make.
        let ends_abbrev = ABBREVIATIONS.iter().any(|a| {
            t.ends_with(a) && {
                let before = t.len() - a.len();
                before == 0
                    || !t[..before].chars().next_back().map(|c| c.is_alphanumeric()).unwrap_or(false)
            }
        });
        if !ends_abbrev && (t.ends_with('.') || t.ends_with('!') || t.ends_with('?')) {
            blocks.push(Block { text: std::mem::take(&mut cur), spans: std::mem::take(&mut spans) });
        }
    }
    if !cur.is_empty() {
        blocks.push(Block { text: cur, spans });
    }
    blocks
}

/// Original paragraph indices whose span overlaps `[a, b)` in the block text.
fn spans_overlapping(block: &Block, a: usize, b: usize) -> Vec<usize> {
    block
        .spans
        .iter()
        .filter(|(s, e, _)| *s < b && a < *e)
        .map(|(_, _, p)| *p)
        .collect()
}

/// Build the sentence inventory. `Other` is skipped — on this manuscript it holds
/// the AI-detection-report wrapper and front matter, neither of which the paper
/// asserts. `References` is skipped for the same reason `extract_from_text`
/// skips it (`extract/mod.rs:60`).
fn build_sentences(ex: &ExtractionResult) -> Vec<Sentence> {
    let mut out = Vec::new();
    let mut id = 0usize;

    let all_lines: Vec<(usize, &String)> = ex
        .sections
        .iter()
        .flat_map(|s| s.paragraphs.iter().enumerate())
        .collect();
    let furniture = page_furniture(&all_lines);

    for (section_index, section) in ex.sections.iter().enumerate() {
        if matches!(section.kind, SectionKind::References | SectionKind::Other) {
            continue;
        }
        for block in unwrap_lines(&section.paragraphs, &furniture) {
            let mut cursor = 0usize;
            for (s_idx, text) in split_sentences(&block.text).into_iter().enumerate() {
                // Locate this sentence in the block to recover its source paragraphs.
                let a = block.text[cursor..].find(&text).map(|o| cursor + o).unwrap_or(cursor);
                let b = a + text.len();
                cursor = b;
                let src = spans_overlapping(&block, a, b);
                out.push(Sentence {
                    id,
                    section_index,
                    section_kind: section.kind,
                    paragraph: *src.first().unwrap_or(&0),
                    source_paragraphs: src,
                    index: s_idx,
                    text,
                    annotations: Vec::new(),
                });
                id += 1;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 2. Deliberately shallow cue filter — no model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilterVerdict {
    Candidate,
    TooShort,
    Question,
    DocumentStructure,
    Intent,
    HeadingLike,
    Attributed,
}

impl FilterVerdict {
    fn tag(&self) -> &'static str {
        match self {
            FilterVerdict::Candidate => "candidate",
            FilterVerdict::TooShort => "too-short",
            FilterVerdict::Question => "question",
            FilterVerdict::DocumentStructure => "doc-structure",
            FilterVerdict::Intent => "intent",
            FilterVerdict::HeadingLike => "heading-like",
            FilterVerdict::Attributed => "attributed",
        }
    }
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Narrative attribution: "Smith et al. (2019) showed", "According to X".
/// Deliberately narrow — parenthetical citations do NOT trigger it, because
/// "X increased 12% (Smith, 2019)" is an authorial claim with a supporting cite.
fn is_attributed(s: &str) -> bool {
    let lower = s.to_lowercase();
    if lower.starts_with("according to") {
        return true;
    }
    // "<Author> et al. (YYYY) <verb>" — the citation appears BEFORE the verb, i.e.
    // the cited work is the grammatical subject.
    if let Some(pos) = s.find("et al.") {
        let after = &s[pos + "et al.".len()..];
        let after_trim = after.trim_start();
        if after_trim.starts_with('(') {
            if let Some(close) = after_trim.find(')') {
                let rest = after_trim[close + 1..].trim_start().to_lowercase();
                const REPORTING: &[&str] = &[
                    "showed", "reported", "found", "demonstrated", "observed", "suggested",
                    "described", "concluded", "noted", "proposed", "established", "documented",
                ];
                if REPORTING.iter().any(|v| rest.starts_with(v)) {
                    return true;
                }
            }
        }
    }
    false
}

fn cue_filter(s: &Sentence) -> FilterVerdict {
    let t = s.text.trim();
    let lower = t.to_lowercase();

    if t.ends_with('?') {
        return FilterVerdict::Question;
    }
    if word_count(t) < 5 {
        return FilterVerdict::TooShort;
    }
    // Heading-like: no terminal punctuation and short.
    if !t.ends_with('.') && !t.ends_with('!') && word_count(t) < 10 {
        return FilterVerdict::HeadingLike;
    }
    const DOC_STRUCTURE: &[&str] = &[
        "section ", "table ", "figure ", "fig. ", "in this section", "this paper is organized",
        "the remainder of this", "supplementary ", "appendix ",
    ];
    if DOC_STRUCTURE.iter().any(|p| lower.starts_with(p)) {
        return FilterVerdict::DocumentStructure;
    }
    const INTENT: &[&str] = &[
        "we aim to", "the aim of this study", "the aim of the present study", "we seek to",
        "this study aims", "the objective of this study", "the purpose of this study",
        "the present study aims", "here we investigate", "we set out to",
    ];
    if INTENT.iter().any(|p| lower.contains(p)) {
        return FilterVerdict::Intent;
    }
    if is_attributed(t) {
        return FilterVerdict::Attributed;
    }
    FilterVerdict::Candidate
}

// ---------------------------------------------------------------------------
// 3. Hedge detection — deterministic, surface form
// ---------------------------------------------------------------------------

fn detect_hedge(s: &str) -> Hedge {
    let l = s.to_lowercase();
    const SPECULATIVE: &[&str] = &[
        "could plausibly", "might conceivably", "it is conceivable", "we speculate",
        "may possibly", "could potentially", "hypothetically",
    ];
    const HEDGED: &[&str] = &[
        " may ", " might ", " could ", " appears to", " seems to", " suggests", " suggested that",
        " likely", " possibly", " probably", " tends to", " indicative of", " potential ",
    ];
    if SPECULATIVE.iter().any(|p| l.contains(p)) {
        return Hedge::Speculative;
    }
    if HEDGED.iter().any(|p| l.contains(p)) {
        return Hedge::Hedged;
    }
    Hedge::Asserted
}

// ---------------------------------------------------------------------------
// 4. Surprisal-template classifier — uses the EXISTING PerplexityModel trait
// ---------------------------------------------------------------------------

/// Labels scored per sentence. NotAClaim is a real label, not a threshold: the
/// classifier chooses it in competition with the four claim kinds.
const LABELS: &[(&str, &str)] = &[
    ("not-a-claim", " background information drawn from previous work."),
    ("empirical", " an observed result measured in this study."),
    ("causal", " that one factor causes or leads to another."),
    ("comparative", " that one condition differs from or exceeds another."),
    ("methodological", " a property of the method or materials used."),
];

const PROMPT_PREFIX: &str = "Sentence: ";
const PROMPT_MIDDLE: &str = "\nThe preceding sentence states";

struct ClassifyStats {
    prefix_mismatches: usize,
    scored: usize,
}

/// Mean surprisal (bits) of the continuation tokens, conditioned on the prefix.
/// Returns None if BPE prefix-consistency fails (recorded, not silently ignored).
fn score_continuation(
    model: &dyn PerplexityModel,
    prefix: &str,
    continuation: &str,
    stats: &mut ClassifyStats,
) -> Option<f32> {
    let prefix_toks = model.tokenize(prefix);
    let full = format!("{prefix}{continuation}");
    let full_toks = model.tokenize(&full);

    if full_toks.len() <= prefix_toks.len() {
        stats.prefix_mismatches += 1;
        return None;
    }
    // BPE prefix-consistency check — the boundary token can merge.
    if full_toks[..prefix_toks.len()] != prefix_toks[..] {
        stats.prefix_mismatches += 1;
        return None;
    }

    let bits = model.surprisals(&full_toks);
    if bits.len() != full_toks.len() {
        stats.prefix_mismatches += 1;
        return None;
    }
    let region = &bits[prefix_toks.len()..];
    if region.is_empty() {
        return None;
    }
    stats.scored += 1;
    Some(region.iter().sum::<f32>() / region.len() as f32)
}

/// Classify one sentence. Returns (label, per-label mean bits).
fn classify(
    model: &dyn PerplexityModel,
    sentence: &str,
    stats: &mut ClassifyStats,
) -> (String, Vec<(String, f32)>) {
    let prefix = format!("{PROMPT_PREFIX}{sentence}{PROMPT_MIDDLE}");
    let mut scores: Vec<(String, f32)> = Vec::new();
    for (label, continuation) in LABELS {
        match score_continuation(model, &prefix, continuation, stats) {
            Some(bits) => scores.push((label.to_string(), bits)),
            None => scores.push((label.to_string(), f32::INFINITY)),
        }
    }
    let best = scores
        .iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(l, _)| l.clone())
        .unwrap_or_else(|| "not-a-claim".to_string());
    (best, scores)
}

// ---------------------------------------------------------------------------
// Peak RSS sampler — `ps` polling, macOS RSS in KB
// ---------------------------------------------------------------------------

fn spawn_rss_sampler() -> (Arc<AtomicU64>, Arc<AtomicBool>) {
    let peak = Arc::new(AtomicU64::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let p = peak.clone();
    let s = stop.clone();
    let pid = std::process::id();
    std::thread::spawn(move || {
        while !s.load(Ordering::Relaxed) {
            if let Ok(out) = std::process::Command::new("ps")
                .args(["-o", "rss=", "-p", &pid.to_string()])
                .output()
            {
                if let Ok(txt) = String::from_utf8(out.stdout) {
                    if let Ok(kb) = txt.trim().parse::<u64>() {
                        p.fetch_max(kb, Ordering::Relaxed);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    (peak, stop)
}

// ---------------------------------------------------------------------------
// Statistic / citation linkage — paragraph granularity (A2)
// ---------------------------------------------------------------------------

/// Statistics and citations are located to (SectionKind, paragraph). A sentence
/// knows its section INDEX; map index -> kind to join. Where two sections share a
/// kind this over-links, which is exactly the A2 defect — recorded, not hidden.
fn link_paragraph(
    ex: &ExtractionResult,
    s: &Sentence,
) -> (Vec<usize>, Vec<usize>) {
    let stats: Vec<usize> = ex
        .statistics
        .iter()
        .enumerate()
        .filter(|(_, st)| st.location.section == s.section_kind && s.source_paragraphs.contains(&st.location.paragraph))
        .map(|(i, _)| i)
        .collect();
    let cites: Vec<usize> = ex
        .citations
        .iter()
        .enumerate()
        .filter(|(_, c)| c.location.section == s.section_kind && s.source_paragraphs.contains(&c.location.paragraph))
        .map(|(i, _)| i)
        .collect();
    (stats, cites)
}

// ---------------------------------------------------------------------------
// Scoring against hand annotations
// ---------------------------------------------------------------------------

/// Annotation file format (TSV, one line per CLAIM only):
///   <sentence_id>\t<kind>\t<note>
/// Lines starting with '#' are comments.
fn load_annotations(path: &Path) -> HashMap<usize, ClaimKind> {
    let text = std::fs::read_to_string(path).expect("annotation file unreadable");
    let mut out = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let id: usize = match parts.next().and_then(|s| s.trim().parse().ok()) {
            Some(v) => v,
            None => continue,
        };
        let kind = parts.next().and_then(ClaimKind::parse);
        if let Some(k) = kind {
            out.insert(id, k);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn load_model() -> (Box<dyn PerplexityModel>, String) {
    // Respect the production gate first, then fall back to the ungated Stage-1 LM.
    let sel = app_lib::models::select_deep_model();
    if let Some(m) = sel.model {
        let name = m.name().to_string();
        println!("[gate] select_deep_model -> {:?}, using: {name}", sel.kind);
        return (m, name);
    }
    println!(
        "[gate] select_deep_model -> {:?} (no deep model). Falling back to the UNGATED Stage-1 LM.",
        sel.kind
    );
    let m = app_lib::models::stage1_lm_model().expect("stage-1 LM unavailable — no model to run");
    let name = m.name().to_string();
    println!("[gate] using: {name}");
    (m, name)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("SPIKE usage:");
        eprintln!("  --dump <manuscript>");
        eprintln!("  --classify <manuscript>");
        eprintln!("  --score <manuscript> <annotations.tsv>");
        std::process::exit(2);
    }
    let mode = args[1].clone();
    let manuscript = args[2].clone();

    let text = gaply_core::extract::docparse::parse_path(Path::new(&manuscript))
        .expect("parse failed");
    let ex = extract_from_text(&text);
    let sentences = build_sentences(&ex);

    println!("=== SPIKE: claim extraction (Option 2 — deterministic candidates + surprisal classification) ===");
    println!("manuscript: {manuscript}");
    println!(
        "parsed: {} chars, {} sections, {} statistics, {} citations, {} references",
        text.len(),
        ex.sections.len(),
        ex.statistics.len(),
        ex.citations.len(),
        ex.references.len()
    );
    println!("sentences (non-References): {}", sentences.len());

    let verdicts: Vec<FilterVerdict> = sentences.iter().map(cue_filter).collect();
    let candidates: Vec<usize> = verdicts
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == FilterVerdict::Candidate)
        .map(|(i, _)| i)
        .collect();
    println!("candidates after cue filter: {}", candidates.len());
    {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for v in &verdicts {
            *counts.entry(v.tag()).or_insert(0) += 1;
        }
        let mut kv: Vec<_> = counts.into_iter().collect();
        kv.sort();
        println!("filter breakdown: {kv:?}");
    }

    if mode == "--validate-delta" {
        // Does line-per-paragraph change the DETERMINISTIC validator's outcome?
        // Rebuild the document text with paragraphs reconstructed, re-extract, and
        // diff the flags. No model involved; purely deterministic both sides.
        let all_lines: Vec<(usize, &String)> = ex.sections.iter()
            .flat_map(|s| s.paragraphs.iter().enumerate()).collect();
        let furniture = page_furniture(&all_lines);
        let mut rebuilt = String::new();
        if let Some(t) = &ex.title { rebuilt.push_str(t); rebuilt.push_str("\n\n"); }
        for sec in &ex.sections {
            if !sec.heading.is_empty() {
                rebuilt.push_str(&sec.heading);
                rebuilt.push_str("\n\n");
            }
            for b in unwrap_lines(&sec.paragraphs, &furniture) {
                rebuilt.push_str(&b.text);
                rebuilt.push_str("\n\n");
            }
        }
        let ex2 = extract_from_text(&rebuilt);
        let v1 = gaply_core::validate::validate(&ex);
        let v2 = gaply_core::validate::validate(&ex2);
        println!("\n--- AS PARSED (line-per-paragraph) ---");
        println!("sections={} stats={} citations={} refs={}",
            ex.sections.len(), ex.statistics.len(), ex.citations.len(), ex.references.len());
        println!("validation flags: {}", v1.flags.len());
        for f in &v1.flags {
            println!("  {:?} {:?} at {:?}/{}", f.severity, f.rule, f.location.section, f.location.paragraph);
        }
        println!("\n--- PARAGRAPHS RECONSTRUCTED ---");
        println!("sections={} stats={} citations={} refs={}",
            ex2.sections.len(), ex2.statistics.len(), ex2.citations.len(), ex2.references.len());
        println!("validation flags: {}", v2.flags.len());
        for f in &v2.flags {
            println!("  {:?} {:?} at {:?}/{}", f.severity, f.rule, f.location.section, f.location.paragraph);
        }
        // Statistics breakdown by Stat variant, both sides.
        fn stat_counts(ex: &ExtractionResult) -> (usize, usize, usize, usize) {
            use gaply_core::extract::stats::Stat;
            let mut p = 0; let mut ci = 0; let mut n = 0; let mut t = 0;
            for s in &ex.statistics {
                match &s.stat {
                    Stat::PValue { .. } => p += 1,
                    Stat::ConfidenceInterval { .. } => ci += 1,
                    Stat::SampleSize { .. } => n += 1,
                    Stat::Test { .. } => t += 1,
                    // Milestone 4 variants: counted with the test they belong to
                    // rather than given new columns, since this spike's output
                    // format is a checked-in comparison table.
                    Stat::TestStatistic { .. } => t += 1,
                    Stat::EffectSize { .. } => {}
                    // §42: a significance criterion is not a p-value. Counted
                    // nowhere rather than folded into `p`, which would make this
                    // spike's checked-in comparison table silently wrong.
                    Stat::SignificanceThreshold { .. } => {}
                }
            }
            (p, ci, n, t)
        }
        let a = stat_counts(&ex);
        let b = stat_counts(&ex2);
        println!("\n--- STATISTICS EXTRACTION DELTA ---");
        println!("{:<28} {:>10} {:>14}", "", "as-parsed", "reconstructed");
        println!("{:<28} {:>10} {:>14}", "p-values", a.0, b.0);
        println!("{:<28} {:>10} {:>14}", "confidence intervals", a.1, b.1);
        println!("{:<28} {:>10} {:>14}", "sample sizes", a.2, b.2);
        println!("{:<28} {:>10} {:>14}", "statistical test names", a.3, b.3);
        println!("{:<28} {:>10} {:>14}", "TOTAL", ex.statistics.len(), ex2.statistics.len());
        println!("{:<28} {:>10} {:>14}", "tables", ex.table_mentions.len(), ex2.table_mentions.len());
        println!("\n--- raw statistics, as-parsed ---");
        for s in &ex.statistics { println!("  {:?} @ {:?}/{}", s.stat, s.location.section, s.location.paragraph); }
        println!("--- raw statistics, reconstructed ---");
        for s in &ex2.statistics { println!("  {:?} @ {:?}/{}", s.stat, s.location.section, s.location.paragraph); }

        let refs_with_year = ex2.references.iter().filter(|r| r.year.is_some()).count();
        println!("\nreferences with a parsed year: as-parsed {} of {}, reconstructed {} of {}",
            ex.references.iter().filter(|r| r.year.is_some()).count(), ex.references.len(),
            refs_with_year, ex2.references.len());
        return;
    }

    if mode == "--refside" {
        use gaply_core::extract::citations::{classify_reference_use, CitationUse};
        let v = classify_reference_use(&ex.references, &ex.citations);
        println!("\n=== ALL {} PARSED REFERENCE ROWS ===", ex.references.len());
        for (i, r) in ex.references.iter().enumerate() {
            println!("[{:>2}] {:?}", i + 1, v.get(i));
            println!("     authors={:?}", r.authors);
            println!("     year={:?} doi={:?}", r.year, r.doi);
            println!("     raw={}", r.raw);
        }
        return;
    }

    if mode == "--score-citations" {
        // Scoring harness for CITATION_EVAL_PROTOCOL_V1 (sha256 e931b9e1...).
        // Ground truth frozen in CITATION_GROUND_TRUTH_V1.md (sha256 18dda429...):
        // 31 scored occurrences, 0 ambiguous, 26 bibliography entries, 0 uncited.
        // (surnames, year, occurrence count)
        let gt: Vec<(&[&str], i32, usize)> = vec![
            (&["slama", "williams"], 1966, 1),
            (&["trivedy"], 1993, 1),
            (&["kamimura", "kiuchi"], 1998, 2),
            (&["miranda"], 2002, 1),
            (&["mamatha"], 2006, 1),
            (&["bizhannia"], 2005, 2),
            (&["etebari"], 2007, 2),
            (&["begum"], 2011, 2),
            (&["nair"], 2009, 2),
            (&["dandin"], 2005, 1),
            (&["etebari"], 2005, 1),
            (&["moore", "stein"], 1954, 1),
            (&["dubois"], 1956, 1),
            (&["miller"], 1959, 1),
            (&["carroll"], 1956, 1),
            (&["schmidt", "platzer"], 1980, 1),
            (&["bradford"], 1976, 1),
            (&["reitman", "frankel"], 1957, 1),
            (&["srivastava", "upadhyay"], 2015, 1),
            (&["liu"], 2023, 1),
            (&["gordon", "burford"], 1984, 1),
            (&["suzuki"], 2023, 1),
            (&["mamatha"], 2008, 1),
            (&["göncü", "parlak"], 2011, 1),
            (&["nair"], 2003, 1),
            (&["rahmathulla", "suresh"], 2012, 1),
        ];
        let total_occ: usize = gt.iter().map(|(_, _, n)| n).sum();
        println!("\n=== PROTOCOL v1 SCORING ===");
        println!("GT: {} keys, {} occurrences", gt.len(), total_occ);

        // Extracted citation keys.
        let ex_keys: Vec<(Vec<String>, i32)> = ex
            .citations
            .iter()
            .filter_map(|c| {
                let y = c.year?;
                let ns: Vec<String> = c
                    .authors
                    .split(|ch: char| !(ch.is_alphabetic() || ch == '-' || ch == '\''))
                    .filter(|t| t.chars().count() >= 2 && t.chars().next().map(|x| x.is_uppercase()).unwrap_or(false))
                    .filter(|t| !["et", "al", "and", "the", "of", "for"].contains(&t.to_lowercase().as_str()))
                    .map(|t| t.to_lowercase())
                    .collect();
                (!ns.is_empty()).then_some((ns, y))
            })
            .collect();

        // --- LAYER 2: citation-side key coverage ---
        let mut hit_keys = 0usize;
        let mut missed: Vec<String> = Vec::new();
        let mut matched_occ = 0usize;
        for (names, year, n) in &gt {
            let found = ex_keys
                .iter()
                .filter(|(ns, y)| y == year && ns.iter().any(|a| names.contains(&a.as_str())))
                .count();
            if found > 0 {
                hit_keys += 1;
                matched_occ += found.min(*n);
            } else {
                missed.push(format!("{} {}", names[0], year));
            }
        }
        println!("\n-- LAYER 2: citation-side --");
        println!("key coverage      : {}/{} = {:.4}", hit_keys, gt.len(), hit_keys as f64 / gt.len() as f64);
        println!("occurrence recall : {}/{} = {:.4}", matched_occ, total_occ, matched_occ as f64 / total_occ as f64);
        if !missed.is_empty() {
            println!("MISSED keys ({}): {:?}", missed.len(), missed);
        }

        // spurious extracted citations (match no GT key)
        let spurious: Vec<String> = ex_keys
            .iter()
            .filter(|(ns, y)| !gt.iter().any(|(names, year, _)| y == year && ns.iter().any(|a| names.contains(&a.as_str()))))
            .map(|(ns, y)| format!("{:?} {}", ns, y))
            .collect();
        println!("extracted citations matching NO GT key: {} {:?}", spurious.len(), spurious);

        // --- LAYER 1: reference-side ---
        use gaply_core::extract::citations::{classify_reference_use, CitationUse};
        let verdicts = classify_reference_use(&ex.references, &ex.citations);
        let mut evaluable = 0usize;
        let mut ref_keys: Vec<(Vec<String>, i32)> = Vec::new();
        for (i, v) in verdicts.iter().enumerate() {
            if !matches!(v, CitationUse::NotEvaluated(_)) {
                evaluable += 1;
                let r = &ex.references[i];
                let ns: Vec<String> = r.authors
                    .split(|ch: char| !(ch.is_alphabetic() || ch == '-' || ch == '\''))
                    .filter(|t| t.chars().count() >= 2 && t.chars().next().map(|x| x.is_uppercase()).unwrap_or(false))
                    .filter(|t| !["et", "al", "and", "the", "of", "for"].contains(&t.to_lowercase().as_str()))
                    .map(|t| t.to_lowercase()).collect();
                if let Some(y) = r.year { ref_keys.push((ns, y)); }
            }
        }
        let mut gt_entry_evaluable = 0usize;
        let mut lost: Vec<String> = Vec::new();
        for (names, year, _) in &gt {
            if ref_keys.iter().any(|(ns, y)| y == year && ns.iter().any(|a| names.contains(&a.as_str()))) {
                gt_entry_evaluable += 1;
            } else {
                lost.push(format!("{} {}", names[0], year));
            }
        }
        println!("\n-- LAYER 1: reference-side --");
        println!("parsed reference rows        : {}", ex.references.len());
        println!("evaluable rows               : {}", evaluable);
        println!("GT entries that became evaluable: {}/{} = {:.4}", gt_entry_evaluable, gt.len(), gt_entry_evaluable as f64 / gt.len() as f64);
        if !lost.is_empty() { println!("LOST entries ({}): {:?}", lost.len(), lost); }

        // --- LAYER 3: join correctness ---
        let mut correct = 0usize; let mut wrong: Vec<String> = Vec::new();
        for (i, v) in verdicts.iter().enumerate() {
            let r = &ex.references[i];
            let ns: Vec<String> = r.authors
                .split(|ch: char| !(ch.is_alphabetic() || ch == '-' || ch == '\''))
                .filter(|t| t.chars().count() >= 2 && t.chars().next().map(|x| x.is_uppercase()).unwrap_or(false))
                .map(|t| t.to_lowercase()).collect();
            let truly_cited = r.year.map(|y| gt.iter().any(|(names, year, _)| *year == y && ns.iter().any(|a| names.contains(&a.as_str())))).unwrap_or(false);
            match v {
                CitationUse::Cited if truly_cited => correct += 1,
                CitationUse::Uncited if !truly_cited => correct += 1,
                CitationUse::NotEvaluated(_) => {}
                _ => wrong.push(format!("[{}] {:?} said {:?}, truth cited={}", i + 1, ns.first(), v, truly_cited)),
            }
        }
        println!("\n-- LAYER 3: join correctness --");
        println!("correct verdicts on evaluable rows: {}/{}", correct, evaluable);
        if !wrong.is_empty() { println!("WRONG ({}):", wrong.len()); for w in &wrong { println!("   {w}"); } }

        // --- the finding's actual output ---
        let unc = verdicts.iter().filter(|v| matches!(v, CitationUse::Uncited)).count();
        println!("\n-- FINDING OUTPUT --");
        println!("would report {} reference(s) as uncited; truth = 0 uncited", unc);
        println!("false accusations = {unc}");
        println!("coverage over evaluable rows = {:.4} (threshold 0.993)", if evaluable > 0 { (evaluable - unc) as f64 / evaluable as f64 } else { 0.0 });
        // Blast radius: citation_density feeds a stylometry finding with a 5.0 cut.
        let wc = text.split_whitespace().count();
        let n = ex.citations.len();
        println!("\n-- BLAST RADIUS: citation_density --");
        println!("word_count={wc} citations={n}");
        for (label, k) in [("BEFORE(28)", 28usize), ("AFTER", n)] {
            let d = k as f64 / wc as f64 * 1000.0;
            let band = if d == 0.0 { "unavailable" } else if d < 1.0 { "Notable (low)" } else if d < 5.0 { "Moderate" } else { "no finding" };
            println!("  {label}: density={d:.3} -> {band}");
        }
        println!("style consistency: {:?}", gaply_core::ai_signals::citation_style_consistency(&ex.citations));
        return;
    }

    if mode == "--uncited" {
        use gaply_core::extract::citations::{classify_reference_use, author_year_style_determined, CitationUse};
        println!("\nauthor-year style determined: {}", author_year_style_determined(&ex.citations));
        let v = classify_reference_use(&ex.references, &ex.citations);
        let mut cited = 0; let mut unc = 0; let mut ne = 0;
        for (i, u) in v.iter().enumerate() {
            let who = ex.references[i].authors.split_whitespace().next().unwrap_or("?");
            match u {
                CitationUse::Cited => { cited += 1; }
                CitationUse::Uncited => { unc += 1; println!("  UNCITED   [{:>2}] {} {:?}", i+1, who, ex.references[i].year); }
                CitationUse::NotEvaluated(r) => { ne += 1; println!("  not-eval  [{:>2}] {:?}  {}", i+1, r, &ex.references[i].raw.chars().take(60).collect::<String>()); }
            }
        }
        println!("\ncited={cited} uncited={unc} not-evaluated={ne} total={}", ex.references.len());
        println!("\n--- all extracted in-text citations ---");
        let mut ks: Vec<String> = ex.citations.iter().map(|c| format!("{:?} {:?} {:?} raw={}", c.style, c.authors, c.year, c.raw)).collect();
        ks.sort(); ks.dedup();
        for k in ks { println!("  {k}"); }
        return;
    }

    if mode == "--refs" {
        // Diagnostic for the uncited-reference finding: does one parsed Reference
        // correspond to one bibliography entry?
        println!("\nparsed references: {}", ex.references.len());
        let numeric = ex.citations.iter().filter(|c| !c.numbers.is_empty()).count();
        println!("citations: {} total, {} numeric-style", ex.citations.len(), numeric);
        println!("\n--- first 12 parsed Reference entries (raw) ---");
        for (i, r) in ex.references.iter().take(12).enumerate() {
            println!("[{:>2}] year={:?} doi={:?}", i + 1, r.year, r.doi);
            println!("     raw: {}", &r.raw.chars().take(130).collect::<String>());
        }
        let no_year = ex.references.iter().filter(|r| r.year.is_none()).count();
        println!("\nreferences with NO parsed year: {} of {}", no_year, ex.references.len());
        // Index-position correspondence + furniture check, post-reflow.
        println!("\n--- ordering / furniture audit ---");
        let mut surnames: Vec<String> = Vec::new();
        for r in &ex.references {
            let first = r.authors.split_whitespace().next().unwrap_or("").trim_end_matches(',').to_lowercase();
            surnames.push(first);
        }
        let sorted = { let mut v = surnames.clone(); v.sort(); v };
        println!("alphabetical by first surname: {}", if sorted == surnames { "YES" } else { "NO" });
        for (i, r) in ex.references.iter().enumerate() {
            let looks_furniture = r.raw.len() < 40 || r.raw.to_lowercase().starts_with("page ");
            if looks_furniture {
                println!("  [{:>2}] SUSPECT: {}", i + 1, r.raw);
            }
        }
        println!("shortest entry: {} chars", ex.references.iter().map(|r| r.raw.len()).min().unwrap_or(0));
        println!("all 20 first-surnames: {:?}", surnames);
        return;
    }

    if mode == "--rawtext" {
        // Diagnostic: what does docparse actually hand the section splitter?
        let nl = text.matches('\n').count();
        let blank = text.matches("\n\n").count();
        let lines: Vec<&str> = text.lines().collect();
        let empty = lines.iter().filter(|l| l.trim().is_empty()).count();
        println!("\nchars={} lines={} newlines={} '\\n\\n' occurrences={} blank lines={}",
            text.len(), lines.len(), nl, blank, empty);
        println!("\n--- first 30 lines, bracketed ---");
        for (i, l) in lines.iter().take(30).enumerate() {
            println!("{i:3}: [{l}]");
        }
        println!("\n--- paragraphs per section (as split_document produced) ---");
        for (i, sec) in ex.sections.iter().enumerate() {
            println!("  [{}] {:?} heading={:?} paragraphs={}", i, sec.kind, sec.heading, sec.paragraphs.len());
            for (j, p) in sec.paragraphs.iter().take(3).enumerate() {
                println!("      p{j}: {}", &p.chars().take(100).collect::<String>());
            }
        }
        return;
    }

    if mode == "--tokstats" {
        // Token accounting only — one model load, ZERO forward passes. Quantifies
        // how much of the measured cost is redundant prefix re-scoring.
        let (model, _) = load_model();
        let mut pl = Vec::new();
        let mut cl = Vec::new();
        for &si in candidates.iter() {
            let prefix = format!("{PROMPT_PREFIX}{}{PROMPT_MIDDLE}", sentences[si].text);
            pl.push(model.tokenize(&prefix).len());
        }
        for (_, cont) in LABELS {
            cl.push(model.tokenize(cont).len());
        }
        let mean_p: f64 = pl.iter().sum::<usize>() as f64 / pl.len() as f64;
        let mean_c: f64 = cl.iter().sum::<usize>() as f64 / cl.len() as f64;
        let actual = 5.0 * (mean_p + mean_c);
        let ideal = mean_p + 5.0 * mean_c;
        println!("\nmean prefix tokens   = {mean_p:.1}");
        println!("mean continuation    = {mean_c:.1}");
        println!("scored per sentence  = {actual:.1} (as built)  vs {ideal:.1} (prefix cached)");
        println!("redundancy factor    = {:.2}x", actual / ideal);
        println!("total positions      = {:.0}", actual * candidates.len() as f64);
        return;
    }

    if mode == "--dump" {
        // Deterministic inventory ONLY — no model is loaded. This is the worksheet
        // the hand annotation is written against, BEFORE any classifier runs.
        println!("\n--- SENTENCE INVENTORY (id \\t section \\t para \\t filter \\t text) ---");
        for (i, s) in sentences.iter().enumerate() {
            println!(
                "{}\t{:?}[{}]\t{}\t{}\t{}",
                s.id,
                s.section_kind,
                s.section_index,
                s.paragraph,
                verdicts[i].tag(),
                s.text.replace('\t', " ")
            );
        }
        return;
    }

    // ---- model phase ----
    let (peak_kb, stop) = spawn_rss_sampler();
    let t_load = Instant::now();
    let (model, model_name) = load_model();
    let load_secs = t_load.elapsed().as_secs_f64();
    println!("model load: {load_secs:.1}s  context={} stride={}", model.context_tokens(), model.stride());

    let t_classify = Instant::now();
    let mut stats = ClassifyStats { prefix_mismatches: 0, scored: 0 };
    let mut sentences = sentences;
    let mut predicted: HashMap<usize, ClaimKind> = HashMap::new();
    let mut all_scores: Vec<(usize, String, Vec<(String, f32)>)> = Vec::new();

    for (n, &si) in candidates.iter().enumerate() {
        let (label, scores) = classify(model.as_ref(), &sentences[si].text.clone(), &mut stats);
        all_scores.push((sentences[si].id, label.clone(), scores));
        if let Some(kind) = ClaimKind::parse(&label) {
            let (supporting_stats, adjacent_citations) = link_paragraph(&ex, &sentences[si]);
            let hedge = detect_hedge(&sentences[si].text);
            predicted.insert(sentences[si].id, kind);
            sentences[si].annotations.push(Annotation::Claim {
                kind,
                hedge,
                supporting_stats,
                adjacent_citations,
            });
        }
        if n % 10 == 0 {
            let el = t_classify.elapsed().as_secs_f64();
            eprintln!(
                "  [{}/{}] {:.1}s elapsed, {:.2}s/sentence",
                n + 1,
                candidates.len(),
                el,
                el / (n + 1) as f64
            );
        }
    }
    let classify_secs = t_classify.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let peak_mb = peak_kb.load(Ordering::Relaxed) as f64 / 1024.0;

    // ---- typed output ----
    println!("\n--- EXTRACTED CLAIMS (typed shape) ---");
    for s in &sentences {
        for a in &s.annotations {
            let Annotation::Claim { kind, hedge, supporting_stats, adjacent_citations } = a;
            println!(
                "sentence:s{}  section={:?}[{}] para={} idx={}",
                s.id, s.section_kind, s.section_index, s.paragraph, s.index
            );
            println!(
                "  claim:kind={}  claim:hedge={}  claim:has_supporting_stat={}  claim:has_adjacent_citation={}",
                kind.tag(),
                hedge.tag(),
                !supporting_stats.is_empty(),
                !adjacent_citations.is_empty()
            );
            println!("  text: {}", s.text);
        }
    }

    // ---- viability gate FIRST ----
    let total_secs = load_secs + classify_secs;
    let headroom_gb = (8.0 * 1024.0 - peak_mb) / 1024.0;
    println!("\n--- VIABILITY GATE (evaluated before accuracy) ---");
    println!("model: {model_name}");
    println!("sentences classified: {} ({} scored, {} prefix mismatches)", candidates.len(), stats.scored, stats.prefix_mismatches);
    println!("wall clock: load {load_secs:.1}s + classify {classify_secs:.1}s = {total_secs:.1}s ({:.1} min)", total_secs / 60.0);
    println!("peak RSS: {peak_mb:.0} MB  -> headroom on 8GB: {headroom_gb:.2} GB");
    let time_ok = total_secs <= 600.0;
    let mem_ok = headroom_gb >= 1.0;
    println!("wall clock <= 10 min : {}", if time_ok { "PASS" } else { "FAIL" });
    println!("headroom >= 1 GB     : {}", if mem_ok { "PASS" } else { "FAIL" });
    if !(time_ok && mem_ok) {
        println!("\nVIABILITY GATE FAILED — accuracy is NOT measured. A model that cannot run does not get graded.");
        return;
    }
    println!("VIABILITY GATE PASSED — proceeding to accuracy.");

    if mode != "--score" {
        println!("\n(no annotation file given; run --score <manuscript> <annotations.tsv> to measure accuracy)");
        return;
    }
    let ann_path = args.get(3).expect("--score needs an annotation file");
    let truth = load_annotations(Path::new(ann_path));

    // ---- metrics ----
    let pred_ids: Vec<usize> = predicted.keys().copied().collect();
    let truth_ids: Vec<usize> = truth.keys().copied().collect();
    let tp: Vec<usize> = pred_ids.iter().copied().filter(|i| truth.contains_key(i)).collect();
    let fp: Vec<usize> = pred_ids.iter().copied().filter(|i| !truth.contains_key(i)).collect();
    let fneg: Vec<usize> = truth_ids.iter().copied().filter(|i| !predicted.contains_key(i)).collect();

    let precision = if pred_ids.is_empty() { 0.0 } else { tp.len() as f64 / pred_ids.len() as f64 };
    let recall = if truth_ids.is_empty() { 0.0 } else { tp.len() as f64 / truth_ids.len() as f64 };
    let kind_agree = tp.iter().filter(|i| predicted.get(i) == truth.get(i)).count();
    let kind_acc = if tp.is_empty() { 0.0 } else { kind_agree as f64 / tp.len() as f64 };

    println!("\n--- ACCURACY vs {ann_path} ---");
    println!("annotated claims: {}   extracted: {}", truth_ids.len(), pred_ids.len());
    println!("TP={} FP={} FN={}", tp.len(), fp.len(), fneg.len());
    println!("precision    = {precision:.3}   (proceed >= 0.85, kill < 0.70)");
    println!("recall       = {recall:.3}   (proceed >= 0.60, kill < 0.40)");
    println!("kind accuracy= {kind_acc:.3}   (proceed >= 0.80; below -> drop ClaimKind, not fatal)");

    let by_id: HashMap<usize, &Sentence> = sentences.iter().map(|s| (s.id, s)).collect();
    let mut fp_sorted = fp.clone();
    fp_sorted.sort();
    println!("\n--- FALSE POSITIVES (predicted claim, not annotated) ---");
    for id in &fp_sorted {
        let s = by_id[id];
        let sc = all_scores.iter().find(|(i, _, _)| i == id);
        println!("sentence:s{id}  [{:?}] predicted={}", s.section_kind, predicted[id].tag());
        if let Some((_, _, scores)) = sc {
            let mut v = scores.clone();
            v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let top: Vec<String> = v.iter().take(3).map(|(l, b)| format!("{l}={b:.3}")).collect();
            println!("  scores: {}", top.join("  "));
        }
        println!("  text: {}", s.text);
    }

    let mut fn_sorted = fneg.clone();
    fn_sorted.sort();
    println!("\n--- FALSE NEGATIVES (annotated claim, not predicted) ---");
    for id in &fn_sorted {
        let s = by_id[id];
        let idx = sentences.iter().position(|x| x.id == *id).unwrap();
        let filt = verdicts[idx];
        let sc = all_scores.iter().find(|(i, _, _)| i == id);
        println!(
            "sentence:s{id}  [{:?}]  filter={}  {}",
            s.section_kind,
            filt.tag(),
            if filt == FilterVerdict::Candidate { "-> reached classifier" } else { "-> BLOCKED BY CUE FILTER" }
        );
        if let Some((_, label, scores)) = sc {
            let mut v = scores.clone();
            v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let top: Vec<String> = v.iter().take(3).map(|(l, b)| format!("{l}={b:.3}")).collect();
            println!("  classifier said: {label}   scores: {}", top.join("  "));
        }
        println!("  text: {}", s.text);
    }

    println!("\n--- KIND DISAGREEMENTS (on TP) ---");
    for id in &tp {
        if predicted.get(id) != truth.get(id) {
            println!(
                "sentence:s{id}  truth={}  predicted={}",
                truth[id].tag(),
                predicted[id].tag()
            );
            println!("  text: {}", by_id[id].text);
        }
    }

    println!("\nNOTE: a pass here means \"proceed to a 5-manuscript measurement\", NOT \"the Manuscript Understanding Layer is validated\".");
}
