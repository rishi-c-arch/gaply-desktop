//! Deterministic EXACT-match plagiarism (Set 2) — verbatim / recycled-text
//! detection you can point to in both places.
//!
//! This is a NEW, self-contained module that does not touch the existing
//! [`plagiarism`](crate::plagiarism) agent. That one is embedding-cosine
//! (semantic "similar meaning") and stays intact as a separate lane; THIS one
//! is pure deterministic text matching:
//!
//!   1. word-shingle the text (k-word shingles) over the ORIGINAL string,
//!      PRESERVING byte offsets (unlike [`chunk::chunk_text`](crate::chunk)
//!      which normalizes whitespace away and loses spans);
//!   2. rolling-hash each shingle (Rabin-Karp over per-token FNV hashes);
//!   3. WINNOWING — keep the minimum hash in each sliding window, the standard
//!      near-linear fingerprint-density technique (Schleimer/Wilkins/Aiken);
//!   4. a hash→postings index; CANDIDATE GENERATION only compares passages
//!      that SHARE a fingerprint (index lookup, never all-pairs — this is what
//!      keeps 500 pages near-linear);
//!   5. verify + greedily extend each seed into a MATCHED PASSAGE with real
//!      character spans in BOTH locations (the `flag_passages_in_block`
//!      forward-scan span discipline, applied to exact matches).
//!
//! Two modes: SELF (recycled text within one document) and LIBRARY (against
//! caller-provided comparison documents — the durable "my papers" store is a
//! later set; here the comparison set is an explicit input).
//!
//! HONEST by construction: a match is REAL overlapping text (verified verbatim,
//! not a hash-collision guess); the similarity is a COMPUTED Jaccard over the
//! shared shingles, never a model output; nothing is fabricated — no shared
//! shingles means no match. NO LLM, NO proxy, NO network, NO I/O.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Shingle size: how many consecutive words form one fingerprinting unit.
/// 5 is the common choice — long enough that incidental short overlaps
/// ("in this paper we") don't fingerprint, short enough to catch a recycled
/// sentence.
pub const DEFAULT_SHINGLE_SIZE: usize = 5;
/// Winnowing window: the guarantee is that any verbatim run of at least
/// `window_size + shingle_size - 1` words is detected. w=4 ⇒ any ~8-word run.
pub const DEFAULT_WINDOW_SIZE: usize = 4;
/// Minimum matched run (in words) to report — filters trivial overlaps.
pub const DEFAULT_MIN_MATCH_WORDS: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct ExactConfig {
    pub shingle_size: usize,
    pub window_size: usize,
    pub min_match_words: usize,
}

impl Default for ExactConfig {
    fn default() -> Self {
        Self {
            shingle_size: DEFAULT_SHINGLE_SIZE,
            window_size: DEFAULT_WINDOW_SIZE,
            min_match_words: DEFAULT_MIN_MATCH_WORDS,
        }
    }
}

/// The fixed limitation notice — the un-strippable content the existing
/// [`plagiarism::ISOLATION_NOTE`](crate::plagiarism::ISOLATION_NOTE) does NOT
/// state. Always part of every report's `disclosure`.
pub const NOT_TURNITIN_NOTICE: &str = "This is NOT a comprehensive plagiarism check and does \
NOT replace Turnitin or iThenticate. It only compares against the documents listed here, and \
cannot detect matches against journals, the web, subscription databases, or any work not \
provided to it. The absence of matches here does NOT mean the text is original.";

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// A span of the ORIGINAL text. Offsets are BYTE offsets into that document
/// (the same convention as [`ai_detect::FlaggedPassage`](crate::ai_detect::FlaggedPassage);
/// the UI text-anchors rather than indexing JS strings with them). `text`
/// slices back to exactly this range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    pub start_char: usize,
    pub end_char: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// The matched text recurs elsewhere in the SAME document (recycled text).
    SelfRepeat,
    /// The matched text also appears in a caller-provided comparison document.
    LibraryMatch,
}

/// One verbatim overlap, shown in BOTH places.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedPassage {
    /// The span in the document under test.
    pub source: TextSpan,
    /// The span in the OTHER location (elsewhere in this doc for `SelfRepeat`,
    /// or in the comparison document for `LibraryMatch`).
    pub matched: TextSpan,
    /// Jaccard over the two regions' shingle sets. A COMPUTED number from the
    /// text overlap — 1.0 for a fully verbatim run. Never a model guess.
    pub similarity: f64,
    /// Length of the overlap in words — longer runs are more significant.
    pub word_count: usize,
    pub match_kind: MatchKind,
    /// Which document the match is in: "this document" for `SelfRepeat`, or
    /// the comparison document's reference (title/filename) for `LibraryMatch`.
    pub source_ref: String,
}

/// Telemetry proving the postings-index path (candidate generation), not an
/// O(n²) all-pairs scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchStats {
    pub source_fingerprints: usize,
    pub comparison_fingerprints: usize,
    /// Number of (source-fingerprint, target-position) candidate pairs the
    /// postings index actually surfaced — ≪ source_fp × comparison_fp.
    pub candidate_seeds: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExactPlagiarismReport {
    pub shingle_size: usize,
    pub window_size: usize,
    pub self_matches: Vec<MatchedPassage>,
    pub library_matches: Vec<MatchedPassage>,
    /// The comparison documents' references (scope transparency).
    pub compared_against: Vec<String>,
    pub total_source_words: usize,
    /// Fraction of the source document's bytes covered by ANY match — the
    /// honest "how much overlaps" headline. Deterministic count.
    pub duplication_ratio: f64,
    pub stats: MatchStats,
    /// REQUIRED, never empty: the exact comparison scope + the Turnitin
    /// limitation ([`NOT_TURNITIN_NOTICE`]).
    pub disclosure: String,
}

/// A caller-provided comparison document (LIBRARY mode). The durable store is
/// a later set; this set takes the comparison set as an explicit input.
pub struct CompareDoc {
    pub reference: String,
    pub text: String,
}

// ---------------------------------------------------------------------------
// Tokenization with PRESERVED byte offsets
// ---------------------------------------------------------------------------

/// A word with its normalized form (for matching) and its ORIGINAL byte span
/// (for reporting the exact text). The span covers the raw word including any
/// attached punctuation — so a passage's `text` is the exact original slice.
#[derive(Clone)]
struct Token {
    norm: String,
    start: usize,
    end: usize,
}

/// Split into words on whitespace, keeping each word's byte span. The
/// normalized form is lowercased and stripped of leading/trailing
/// non-alphanumerics (so "Cat." and "cat" match); words that normalize to
/// nothing (pure punctuation) are dropped.
fn tokenize_with_spans(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut word_start: Option<usize> = None;
    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = word_start.take() {
                push_token(text, start, i, &mut tokens);
            }
        } else if word_start.is_none() {
            word_start = Some(i);
        }
    }
    if let Some(start) = word_start.take() {
        push_token(text, start, text.len(), &mut tokens);
    }
    tokens
}

fn push_token(text: &str, start: usize, end: usize, out: &mut Vec<Token>) {
    let raw = &text[start..end];
    let lowered = raw.to_lowercase();
    let trimmed = lowered.trim_matches(|c: char| !c.is_alphanumeric());
    if !trimmed.is_empty() {
        out.push(Token { norm: trimmed.to_string(), start, end });
    }
}

// ---------------------------------------------------------------------------
// Rolling-hash shingles + winnowing
// ---------------------------------------------------------------------------

/// FNV-1a 64-bit of a normalized token.
fn token_fnv(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// splitmix64 finalizer — spreads the polynomial's bits so winnowing's minima
/// are well distributed (biased hashes wreck fingerprint density).
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Rolling (Rabin-Karp) hash of every k-word shingle. Returns one hash per
/// shingle; shingle `i` covers tokens `[i, i+k)`. Empty if fewer than k tokens.
fn shingle_hashes(tokens: &[Token], k: usize) -> Vec<u64> {
    if k == 0 || tokens.len() < k {
        return Vec::new();
    }
    const BASE: u64 = 1_000_003;
    let th: Vec<u64> = tokens.iter().map(|t| token_fnv(&t.norm)).collect();
    let mut pk = 1u64; // BASE^(k-1)
    for _ in 0..k - 1 {
        pk = pk.wrapping_mul(BASE);
    }
    let mut out = Vec::with_capacity(th.len() - k + 1);
    let mut h = 0u64;
    for &t in th.iter().take(k) {
        h = h.wrapping_mul(BASE).wrapping_add(t);
    }
    out.push(mix64(h));
    for i in 1..=th.len() - k {
        h = h
            .wrapping_sub(th[i - 1].wrapping_mul(pk))
            .wrapping_mul(BASE)
            .wrapping_add(th[i + k - 1]);
        out.push(mix64(h));
    }
    out
}

/// Winnowing: in each window of `w` consecutive shingle hashes select the
/// minimum (rightmost on ties, the standard robust rule) as a fingerprint,
/// deduping consecutive identical positions. Bounds fingerprint density while
/// guaranteeing any run ≥ `w + k - 1` words is caught. Returns `(hash, shingle_index)`.
fn winnow(hashes: &[u64], w: usize) -> Vec<(u64, usize)> {
    let mut fps = Vec::new();
    if hashes.is_empty() {
        return fps;
    }
    let w = w.max(1).min(hashes.len());
    let mut last_pos = usize::MAX;
    for start in 0..=hashes.len() - w {
        // rightmost minimum in hashes[start..start+w]
        let mut min = hashes[start];
        let mut min_pos = start;
        for (j, &val) in hashes.iter().enumerate().take(start + w).skip(start) {
            if val <= min {
                min = val;
                min_pos = j;
            }
        }
        if min_pos != last_pos {
            fps.push((min, min_pos));
            last_pos = min_pos;
        }
    }
    fps
}

// ---------------------------------------------------------------------------
// Candidate generation + verify + extend
// ---------------------------------------------------------------------------

struct Doc {
    tokens: Vec<Token>,
    shingles: Vec<u64>,
    fingerprints: Vec<(u64, usize)>,
}

fn build_doc(text: &str, cfg: &ExactConfig) -> Doc {
    let tokens = tokenize_with_spans(text);
    let shingles = shingle_hashes(&tokens, cfg.shingle_size);
    let fingerprints = winnow(&shingles, cfg.window_size);
    Doc { tokens, shingles, fingerprints }
}

/// A verified+extended verbatim run expressed as word ranges in both docs.
struct Run {
    s_lo: usize,
    s_hi: usize, // source word range [s_lo, s_hi)
    t_lo: usize,
    t_hi: usize, // target word range [t_lo, t_hi)
}

/// Postings index over the target's fingerprints: hash → shingle positions.
fn postings(fps: &[(u64, usize)]) -> HashMap<u64, Vec<usize>> {
    let mut m: HashMap<u64, Vec<usize>> = HashMap::new();
    for &(h, pos) in fps {
        m.entry(h).or_default().push(pos);
    }
    m
}

/// For each source fingerprint, look up target positions sharing that
/// fingerprint (candidate generation — index lookup, NOT all-pairs), verify
/// the shingle is truly verbatim, and greedily extend it. `self_mode` excludes
/// a passage matching itself and keeps the two regions disjoint. Returns the
/// deduped runs and the number of candidate seeds surfaced.
fn find_runs(src: &Doc, tgt: &Doc, k: usize, min_words: usize, self_mode: bool) -> (Vec<Run>, usize) {
    let index = postings(&tgt.fingerprints);
    let mut seen: HashSet<(usize, usize, usize, usize)> = HashSet::new();
    let mut runs = Vec::new();
    let mut candidate_seeds = 0usize;

    for &(h, s_idx) in &src.fingerprints {
        let Some(t_positions) = index.get(&h) else { continue };
        for &t_idx in t_positions {
            candidate_seeds += 1;
            if self_mode && s_idx == t_idx {
                continue; // a shingle matching itself
            }
            // VERIFY verbatim (guards against hash collisions — never a
            // fabricated match).
            if !shingle_eq(&src.tokens, s_idx, &tgt.tokens, t_idx, k) {
                continue;
            }
            let mut s_lo = s_idx;
            let mut s_hi = s_idx + k;
            let mut t_lo = t_idx;
            let mut t_hi = t_idx + k;
            // extend left
            while s_lo > 0 && t_lo > 0 && src.tokens[s_lo - 1].norm == tgt.tokens[t_lo - 1].norm {
                s_lo -= 1;
                t_lo -= 1;
            }
            // extend right
            while s_hi < src.tokens.len()
                && t_hi < tgt.tokens.len()
                && src.tokens[s_hi].norm == tgt.tokens[t_hi].norm
            {
                s_hi += 1;
                t_hi += 1;
            }
            if s_hi - s_lo < min_words {
                continue;
            }
            if self_mode {
                // regions must be disjoint, and report each symmetric pair once
                // (source = the earlier occurrence).
                let disjoint = s_hi <= t_lo || t_hi <= s_lo;
                if !disjoint || s_lo >= t_lo {
                    continue;
                }
            }
            if seen.insert((s_lo, s_hi, t_lo, t_hi)) {
                runs.push(Run { s_lo, s_hi, t_lo, t_hi });
            }
        }
    }
    (runs, candidate_seeds)
}

fn shingle_eq(a: &[Token], ai: usize, b: &[Token], bi: usize, k: usize) -> bool {
    (0..k).all(|j| a[ai + j].norm == b[bi + j].norm)
}

/// Jaccard over the two regions' shingle-hash sets — a real computed measure.
/// 1.0 for a fully verbatim run (identical shingles).
fn region_jaccard(a: &[u64], a_lo: usize, a_hi: usize, b: &[u64], b_lo: usize, b_hi: usize, k: usize) -> f64 {
    let sa: HashSet<u64> = shingle_range(a, a_lo, a_hi, k);
    let sb: HashSet<u64> = shingle_range(b, b_lo, b_hi, k);
    if sa.is_empty() && sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    if union == 0 {
        0.0
    } else {
        inter as f64 / union as f64
    }
}

/// The shingle hashes whose window falls entirely inside word range [lo, hi).
fn shingle_range(shingles: &[u64], lo: usize, hi: usize, k: usize) -> HashSet<u64> {
    let mut set = HashSet::new();
    if hi < lo + k {
        return set;
    }
    for i in lo..=hi - k {
        if i < shingles.len() {
            set.insert(shingles[i]);
        }
    }
    set
}

fn span_of(tokens: &[Token], text: &str, lo: usize, hi: usize) -> TextSpan {
    let start = tokens[lo].start;
    let end = tokens[hi - 1].end;
    TextSpan { start_char: start, end_char: end, text: text[start..end].to_string() }
}

// ---------------------------------------------------------------------------
// The public analysis
// ---------------------------------------------------------------------------

fn build_disclosure(library_refs: &[String]) -> String {
    let scope = if library_refs.is_empty() {
        "Checked this document against ITSELF only — no other documents were provided for comparison.".to_string()
    } else {
        format!(
            "Checked this document against itself and {} provided document(s): {}.",
            library_refs.len(),
            library_refs.join("; ")
        )
    };
    format!("{scope} {NOT_TURNITIN_NOTICE}")
}

/// Deterministic exact-match plagiarism over `document`, in both SELF mode
/// (recycled text within it) and LIBRARY mode (against each `library` doc).
/// PURE: no I/O, no network, no model. Same input → same output.
pub fn analyze_exact(document: &str, library: &[CompareDoc], config: &ExactConfig) -> ExactPlagiarismReport {
    let k = config.shingle_size.max(1);
    let src = build_doc(document, config);

    // SELF mode: source against itself.
    let (self_runs, self_seeds) = find_runs(&src, &src, k, config.min_match_words, true);
    let self_matches: Vec<MatchedPassage> = self_runs
        .iter()
        .map(|r| MatchedPassage {
            source: span_of(&src.tokens, document, r.s_lo, r.s_hi),
            matched: span_of(&src.tokens, document, r.t_lo, r.t_hi),
            similarity: region_jaccard(&src.shingles, r.s_lo, r.s_hi, &src.shingles, r.t_lo, r.t_hi, k),
            word_count: r.s_hi - r.s_lo,
            match_kind: MatchKind::SelfRepeat,
            source_ref: "this document".to_string(),
        })
        .collect();

    // LIBRARY mode: source against each provided document.
    let mut library_matches = Vec::new();
    let mut comparison_fingerprints = 0usize;
    let mut library_seeds = 0usize;
    let mut compared_against = Vec::with_capacity(library.len());
    for doc in library {
        compared_against.push(doc.reference.clone());
        let tgt = build_doc(&doc.text, config);
        comparison_fingerprints += tgt.fingerprints.len();
        let (runs, seeds) = find_runs(&src, &tgt, k, config.min_match_words, false);
        library_seeds += seeds;
        for r in runs {
            library_matches.push(MatchedPassage {
                source: span_of(&src.tokens, document, r.s_lo, r.s_hi),
                matched: span_of(&tgt.tokens, &doc.text, r.t_lo, r.t_hi),
                similarity: region_jaccard(&src.shingles, r.s_lo, r.s_hi, &tgt.shingles, r.t_lo, r.t_hi, k),
                word_count: r.s_hi - r.s_lo,
                match_kind: MatchKind::LibraryMatch,
                source_ref: doc.reference.clone(),
            });
        }
    }

    // duplication ratio: union of source-side byte ranges covered by any match
    // (self counts BOTH of its in-document regions).
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for m in &self_matches {
        ranges.push((m.source.start_char, m.source.end_char));
        ranges.push((m.matched.start_char, m.matched.end_char));
    }
    for m in &library_matches {
        ranges.push((m.source.start_char, m.source.end_char));
    }
    let covered = covered_bytes(&mut ranges);
    let total = document.len();
    let duplication_ratio = if total == 0 { 0.0 } else { covered as f64 / total as f64 };

    ExactPlagiarismReport {
        shingle_size: config.shingle_size,
        window_size: config.window_size,
        self_matches,
        library_matches,
        compared_against: compared_against.clone(),
        total_source_words: src.tokens.len(),
        duplication_ratio,
        stats: MatchStats {
            source_fingerprints: src.fingerprints.len(),
            comparison_fingerprints,
            candidate_seeds: self_seeds + library_seeds,
        },
        disclosure: build_disclosure(&compared_against),
    }
}

/// Merge overlapping [start, end) ranges and sum the covered bytes.
fn covered_bytes(ranges: &mut [(usize, usize)]) -> usize {
    if ranges.is_empty() {
        return 0;
    }
    ranges.sort_unstable();
    let mut total = 0usize;
    let (mut cs, mut ce) = ranges[0];
    for &(s, e) in &ranges[1..] {
        if s <= ce {
            ce = ce.max(e);
        } else {
            total += ce - cs;
            cs = s;
            ce = e;
        }
    }
    total + (ce - cs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECYCLED: &str =
        "The mitochondrial membrane potential collapses during the early phase of apoptosis and \
         triggers the release of cytochrome c into the cytosol.";
    const FILLER_A: &str =
        "In the introduction we describe the general background of the study area and its scope.";
    const FILLER_B: &str =
        "The discussion revisits every finding against the wider literature and prior reviews.";

    fn cfg() -> ExactConfig {
        ExactConfig::default()
    }

    // ------------------------------- SELF mode ------------------------------

    #[test]
    fn self_mode_flags_a_recycled_paragraph_with_spans_in_both_places() {
        // same paragraph appears in two distant sections
        let doc = format!("Section one. {FILLER_A} {RECYCLED}\n\nSection two. {FILLER_B} {RECYCLED}");
        let out = analyze_exact(&doc, &[], &cfg());

        assert_eq!(out.self_matches.len(), 1, "one recycled passage: {:#?}", out.self_matches);
        let m = &out.self_matches[0];
        // spans slice back to the exact recycled text in BOTH locations
        assert_eq!(&doc[m.source.start_char..m.source.end_char], m.source.text);
        assert_eq!(&doc[m.matched.start_char..m.matched.end_char], m.matched.text);
        assert!(m.source.text.contains("mitochondrial membrane potential"));
        assert!(m.matched.text.contains("mitochondrial membrane potential"));
        // the two occurrences are at DIFFERENT offsets (real recycling)
        assert!(m.source.start_char < m.matched.start_char);
        assert_eq!(m.match_kind, MatchKind::SelfRepeat);
        assert_eq!(m.source_ref, "this document");
    }

    #[test]
    fn no_repeats_no_matches() {
        let doc = format!("{FILLER_A} {RECYCLED} {FILLER_B}");
        let out = analyze_exact(&doc, &[], &cfg());
        assert!(out.self_matches.is_empty(), "no recycling: {:#?}", out.self_matches);
        assert_eq!(out.duplication_ratio, 0.0);
    }

    #[test]
    fn trivial_self_match_of_a_passage_against_itself_is_excluded() {
        // a single occurrence must never match itself
        let doc = format!("Only once. {RECYCLED} And nothing else here at all really.");
        let out = analyze_exact(&doc, &[], &cfg());
        assert!(out.self_matches.is_empty(), "a passage cannot plagiarise itself: {:#?}", out.self_matches);
    }

    // ------------------------------ LIBRARY mode ----------------------------

    #[test]
    fn library_mode_flags_a_shared_paragraph_with_spans_in_both_docs() {
        let doc = format!("My introduction. {FILLER_A} {RECYCLED}");
        let other = format!("Their earlier paper. {FILLER_B} {RECYCLED} plus more of their own text.");
        let lib = vec![CompareDoc { reference: "Prior paper (2019)".into(), text: other.clone() }];
        let out = analyze_exact(&doc, &lib, &cfg());

        assert!(out.self_matches.is_empty());
        assert_eq!(out.library_matches.len(), 1, "{:#?}", out.library_matches);
        let m = &out.library_matches[0];
        assert_eq!(&doc[m.source.start_char..m.source.end_char], m.source.text);
        assert_eq!(&other[m.matched.start_char..m.matched.end_char], m.matched.text);
        assert!(m.source.text.contains("cytochrome c"));
        assert!(m.matched.text.contains("cytochrome c"));
        assert_eq!(m.match_kind, MatchKind::LibraryMatch);
        assert_eq!(m.source_ref, "Prior paper (2019)");
        assert_eq!(out.compared_against, vec!["Prior paper (2019)".to_string()]);
    }

    #[test]
    fn no_shared_text_no_library_match() {
        let doc = format!("{FILLER_A} {RECYCLED}");
        let other = format!("{FILLER_B} entirely unrelated prose about geology and sediment cores instead.");
        let lib = vec![CompareDoc { reference: "Unrelated".into(), text: other }];
        let out = analyze_exact(&doc, &lib, &cfg());
        assert!(out.library_matches.is_empty(), "{:#?}", out.library_matches);
    }

    // ---------------------------- honest similarity -------------------------

    #[test]
    fn similarity_is_deterministic_and_verbatim_runs_are_one() {
        let doc = format!("A. {FILLER_A} {RECYCLED} B. {FILLER_B} {RECYCLED}");
        let a = analyze_exact(&doc, &[], &cfg());
        let b = analyze_exact(&doc, &[], &cfg());
        assert_eq!(a, b, "same input → identical report");
        // a fully verbatim overlap is 100% similar — an honest 1.0, computed
        // (not fabricated) from the shared shingles.
        assert!((a.self_matches[0].similarity - 1.0).abs() < 1e-9);
    }

    // -------------------- near-linear: postings, not all-pairs --------------

    #[test]
    fn candidate_generation_uses_the_postings_index_not_all_pairs() {
        // two sizable docs with NO shared text: an all-pairs scan would be
        // source_fp × comparison_fp candidate comparisons; the postings index
        // surfaces (near) zero because no fingerprint is shared.
        let big_a: String = (0..400).map(|i| format!("alpha{i} beta{i} gamma{i} delta{i} epsilon{i}. ")).collect();
        let big_b: String = (0..400).map(|i| format!("uno{i} dos{i} tres{i} cuatro{i} cinco{i}. ")).collect();
        let lib = vec![CompareDoc { reference: "B".into(), text: big_b }];
        let out = analyze_exact(&big_a, &lib, &cfg());
        let s = out.stats;
        assert!(s.source_fingerprints > 100 && s.comparison_fingerprints > 100, "both docs fingerprinted");
        let all_pairs = s.source_fingerprints * s.comparison_fingerprints;
        assert!(
            s.candidate_seeds * 50 < all_pairs,
            "candidate_seeds {} must be ≪ all-pairs {}",
            s.candidate_seeds,
            all_pairs
        );
        assert!(out.library_matches.is_empty(), "no shared text → no matches");
    }

    // ----------------------- un-strippable disclosure -----------------------

    #[test]
    fn disclosure_is_required_and_states_scope_plus_the_turnitin_limit() {
        let self_only = analyze_exact(&format!("{FILLER_A} {RECYCLED}"), &[], &cfg());
        assert!(!self_only.disclosure.is_empty());
        assert!(self_only.disclosure.contains("against ITSELF only"));
        assert!(self_only.disclosure.contains("does NOT replace Turnitin"));
        assert!(self_only.disclosure.contains("cannot detect matches against journals"));

        let lib = vec![
            CompareDoc { reference: "Paper A".into(), text: "x".into() },
            CompareDoc { reference: "Paper B".into(), text: "y".into() },
        ];
        let with_lib = analyze_exact(RECYCLED, &lib, &cfg());
        assert!(with_lib.disclosure.contains("2 provided document(s)"));
        assert!(with_lib.disclosure.contains("Paper A"));
        assert!(with_lib.disclosure.contains("Paper B"));
        assert!(with_lib.disclosure.contains(NOT_TURNITIN_NOTICE));
        // survives serialization to the UI
        let wire = serde_json::to_string(&with_lib).unwrap();
        assert!(wire.contains("does NOT replace Turnitin"));
    }

    // ------------------------------ real spans ------------------------------

    #[test]
    fn every_reported_span_slices_back_to_the_exact_text() {
        let doc = format!("Intro. {FILLER_A} {RECYCLED} mid. {FILLER_B} {RECYCLED} end.");
        let other = format!("Other. {RECYCLED} tail.");
        let lib = vec![CompareDoc { reference: "Other".into(), text: other.clone() }];
        let out = analyze_exact(&doc, &lib, &cfg());
        for m in out.self_matches.iter().chain(out.library_matches.iter()) {
            assert_eq!(&doc[m.source.start_char..m.source.end_char], m.source.text);
            let tgt = if m.match_kind == MatchKind::SelfRepeat { &doc } else { &other };
            assert_eq!(&tgt[m.matched.start_char..m.matched.end_char], m.matched.text);
            assert!(m.word_count >= DEFAULT_MIN_MATCH_WORDS);
        }
        assert!(out.duplication_ratio > 0.0 && out.duplication_ratio <= 1.0);
    }

    // ------------------------- offsets on non-ASCII -------------------------

    #[test]
    fn byte_spans_are_correct_through_multibyte_text() {
        // é, α, — are multibyte; the spans must still slice validly.
        let passage = "café résumé naïve α β γ coördinate façade jalapeño piñata";
        let doc = format!("Über section — {passage}. Later — {passage}.");
        let out = analyze_exact(&doc, &[], &cfg());
        assert_eq!(out.self_matches.len(), 1);
        let m = &out.self_matches[0];
        // no panic slicing at multibyte boundaries; text round-trips
        assert_eq!(&doc[m.source.start_char..m.source.end_char], m.source.text);
        assert!(m.source.text.contains("jalapeño"));
    }
}
