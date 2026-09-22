//! Word-token chunking for RAG ingestion: ~512 tokens per chunk with a
//! 64-token overlap so no sentence context is lost at boundaries.

pub const CHUNK_TOKENS: usize = 512;
pub const CHUNK_OVERLAP: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// 0-based position of the chunk within its document.
    pub seq: i64,
    pub content: String,
    pub token_count: usize,
}

/// Split `text` into overlapping chunks. Tokens are whitespace-delimited
/// words (a fair proxy for model tokens at this granularity); chunk content
/// is the tokens re-joined with single spaces.
pub fn chunk_text(text: &str, chunk_tokens: usize, overlap: usize) -> Vec<Chunk> {
    assert!(overlap < chunk_tokens, "overlap must be smaller than chunk size");
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }

    let step = chunk_tokens - overlap;
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut seq = 0i64;
    loop {
        let end = (start + chunk_tokens).min(tokens.len());
        chunks.push(Chunk {
            seq,
            content: tokens[start..end].join(" "),
            token_count: end - start,
        });
        if end == tokens.len() {
            break;
        }
        start += step;
        seq += 1;
    }
    chunks
}

/// Default chunking for the ingestion pipeline.
pub fn chunk_default(text: &str) -> Vec<Chunk> {
    chunk_text(text, CHUNK_TOKENS, CHUNK_OVERLAP)
}

/* ===================== page-aware chunking (AI engine) ==================== *
 * The AI ingest needs three things `chunk_text` cannot express: the page a
 * chunk came from, its character span in the document text, and the section it
 * sits under. It also must never split a sentence — an evidence quote that
 * begins mid-clause is not quotable.
 *
 * This is the SAME 512/64 windowing budget, packed by SENTENCE instead of by
 * raw whitespace token. Sentence and heading knowledge are reused, not
 * re-implemented: `extract::sentence::sentences_in` owns sentence boundaries
 * and `extract::sections::detect_heading` owns the heading vocabulary
 * (Abstract / Introduction / Methods / Results / Discussion / Conclusion /
 * References, numbered variants included).
 *
 * `chunk_text` above is untouched and still serves the existing RAG pipeline. */

use crate::extract::docparse::PagedBlock;
use crate::extract::sections::{self, SectionKind};
use crate::extract::sentence;

/// A chunk carrying the provenance the AI layer requires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedChunk {
    pub seq: i64,
    pub content: String,
    pub token_estimate: usize,
    /// The recorded page this chunk STARTS on; `None` for page-less sources.
    /// Never inferred — see [`PagedBlock::page`].
    pub page: Option<u32>,
    /// Best-effort section label, `None` until the first recognised heading.
    pub section: Option<String>,
    /// Span into the document text produced by [`document_text`], as UTF-8
    /// BYTE offsets (Rust slice indices), not Unicode scalar counts — see plan
    /// §11 D4 for why the `char_*` column names were kept. `content` re-slices
    /// byte-identically from that text, which is what makes an evidence quote
    /// checkable rather than merely plausible.
    pub char_start: usize,
    pub char_end: usize,
}

/// The canonical document text these chunks index into: blocks joined by a
/// blank line, matching the non-paged parser's output contract.
pub fn document_text(blocks: &[PagedBlock]) -> String {
    blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n\n")
}

fn token_estimate(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Human label for a detected section kind; `Other` keeps the literal heading.
fn section_label(kind: SectionKind, heading: &str) -> String {
    match kind {
        SectionKind::Abstract => "Abstract".to_string(),
        SectionKind::Introduction => "Introduction".to_string(),
        SectionKind::Methods => "Methods".to_string(),
        SectionKind::Results => "Results".to_string(),
        SectionKind::Discussion => "Discussion".to_string(),
        SectionKind::Conclusion => "Conclusion".to_string(),
        SectionKind::References => "References".to_string(),
        SectionKind::Other => heading.trim().to_string(),
    }
}

/// Page-aware, sentence-safe chunking over reflowed blocks.
///
/// Sentences are packed up to `chunk_tokens`; a sentence longer than the budget
/// becomes its own chunk rather than being cut. `overlap` re-emits whole
/// trailing sentences worth roughly that many tokens at the head of the next
/// chunk, so context survives a boundary without a sentence ever being split.
///
/// A chunk never spans two blocks, so its page and section are unambiguous.
pub fn chunk_paged(blocks: &[PagedBlock], chunk_tokens: usize, overlap: usize) -> Vec<PagedChunk> {
    assert!(overlap < chunk_tokens, "overlap must be smaller than chunk size");
    let mut out: Vec<PagedChunk> = Vec::new();
    let mut seq = 0i64;
    let mut cursor = 0usize; // char offset of the current block in document_text
    let mut section: Option<String> = None;

    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            cursor += 2; // the "\n\n" joiner
        }
        let block_start = cursor;
        cursor += block.text.len(); // bytes, matching the offsets stored below

        // A heading defines the section for everything after it and is not
        // itself chunked as evidence.
        if let Some((kind, heading, _also)) = sections::detect_heading(&block.text) {
            section = Some(section_label(kind, &heading));
            continue;
        }

        // Sentence slices point INTO block.text, so their exact spans come from
        // pointer arithmetic rather than from re-measuring joined strings. That
        // is what makes `content` byte-identical to a re-slice of the document
        // text — the property `chunk_span_reslices_to_content` asserts, and the
        // one that lets an evidence quote be verified instead of trusted.
        let base = block.text.as_ptr() as usize;
        let spans: Vec<(usize, usize)> = sentence::sentences_in(&block.text)
            .iter()
            .map(|s| {
                let start = s.as_ptr() as usize - base;
                (start, start + s.len())
            })
            .collect();
        if spans.is_empty() {
            continue;
        }

        // Pack sentences into windows, carrying whole sentences as overlap.
        // `window` holds spans, never substrings, so nothing is ever cut.
        let mut window: Vec<(usize, usize)> = Vec::new();
        let mut window_tokens = 0usize;

        for &(s_start, s_end) in &spans {
            let s_tokens = token_estimate(&block.text[s_start..s_end]);
            if !window.is_empty() && window_tokens + s_tokens > chunk_tokens {
                push_chunk(
                    &mut out, &mut seq, block, block_start, &window, window_tokens, &section,
                );
                window = take_overlap(&block.text, &window, overlap);
                window_tokens = window
                    .iter()
                    .map(|&(a, b)| token_estimate(&block.text[a..b]))
                    .sum();
            }
            window.push((s_start, s_end));
            window_tokens += s_tokens;
        }
        push_chunk(&mut out, &mut seq, block, block_start, &window, window_tokens, &section);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn push_chunk(
    out: &mut Vec<PagedChunk>,
    seq: &mut i64,
    block: &PagedBlock,
    block_start: usize,
    window: &[(usize, usize)],
    window_tokens: usize,
    section: &Option<String>,
) {
    let (Some(&(first, _)), Some(&(_, last))) = (window.first(), window.last()) else {
        return;
    };
    out.push(PagedChunk {
        seq: *seq,
        content: block.text[first..last].to_string(),
        token_estimate: window_tokens,
        page: block.page,
        section: section.clone(),
        char_start: block_start + first,
        char_end: block_start + last,
    });
    *seq += 1;
}

/// The trailing whole sentences worth up to `overlap` tokens.
fn take_overlap(text: &str, window: &[(usize, usize)], overlap: usize) -> Vec<(usize, usize)> {
    let mut carry: Vec<(usize, usize)> = Vec::new();
    let mut tokens = 0usize;
    for &(a, b) in window.iter().rev() {
        let t = token_estimate(&text[a..b]);
        if tokens + t > overlap {
            break;
        }
        carry.push((a, b));
        tokens += t;
    }
    carry.reverse();
    carry
}

/// Default page-aware chunking, on the same budget as [`chunk_default`].
pub fn chunk_paged_default(blocks: &[PagedBlock]) -> Vec<PagedChunk> {
    chunk_paged(blocks, CHUNK_TOKENS, CHUNK_OVERLAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered_words(n: usize) -> String {
        (0..n).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn overlap_boundaries_are_exact() {
        let text = numbered_words(1000);
        let chunks = chunk_text(&text, 512, 64);
        // 1000 tokens, step 448: chunks at [0..512], [448..960], [896..1000]
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].token_count, 512);
        assert_eq!(chunks[1].token_count, 512);
        assert_eq!(chunks[2].token_count, 104);

        // the last 64 tokens of chunk N equal the first 64 tokens of chunk N+1
        for pair in chunks.windows(2) {
            let prev: Vec<&str> = pair[0].content.split(' ').collect();
            let next: Vec<&str> = pair[1].content.split(' ').collect();
            assert_eq!(prev[prev.len() - 64..], next[..64], "overlap mismatch");
        }

        // chunk 1 must start exactly at token 448
        assert!(chunks[1].content.starts_with("w448 "));
        // chunk 2 must start exactly at token 896
        assert!(chunks[2].content.starts_with("w896 "));
    }

    #[test]
    fn short_document_is_a_single_full_chunk() {
        let chunks = chunk_default(&numbered_words(100));
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].seq, 0);
        assert_eq!(chunks[0].token_count, 100);
    }

    #[test]
    fn exact_multiple_produces_no_empty_tail() {
        let chunks = chunk_text(&numbered_words(512), 512, 64);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn empty_text_yields_no_chunks() {
        assert!(chunk_default("   \n\t ").is_empty());
    }

    /* --------------------- page-aware chunking (AI engine) ----------------- */

    fn blk(page: Option<u32>, text: &str) -> PagedBlock {
        PagedBlock { page, style: None, text: text.to_string() }
    }

    /// N sentences, each ~10 tokens, so a small budget forces several windows.
    fn sentences(n: usize) -> String {
        (0..n)
            .map(|i| format!("Sentence {i} carries eight more filler words here for length."))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn sentence_boundaries_are_never_split() {
        let text = sentences(40);
        let blocks = vec![blk(Some(1), &text)];
        let chunks = chunk_paged(&blocks, 40, 10);
        assert!(chunks.len() > 3, "expected several windows, got {}", chunks.len());

        // Every sentence of the source, whole, must appear in some chunk, and no
        // chunk may begin or end mid-sentence.
        let originals = sentence::sentences_in(&text);
        for c in &chunks {
            let first = originals
                .iter()
                .find(|s| c.content.starts_with(**s))
                .unwrap_or_else(|| panic!("chunk starts mid-sentence: {:?}", &c.content[..40.min(c.content.len())]));
            assert!(!first.is_empty());
            assert!(
                originals.iter().any(|s| c.content.ends_with(*s)),
                "chunk ends mid-sentence: {:?}",
                &c.content[c.content.len().saturating_sub(40)..]
            );
        }
        for s in &originals {
            assert!(chunks.iter().any(|c| c.content.contains(*s)), "sentence lost: {s:?}");
        }
    }

    #[test]
    fn chunk_span_reslices_to_content() {
        let blocks = vec![
            blk(Some(1), &sentences(12)),
            blk(Some(2), "A second block. With two sentences."),
        ];
        let doc = document_text(&blocks);
        for c in chunk_paged(&blocks, 30, 8) {
            assert_eq!(
                &doc[c.char_start..c.char_end],
                c.content,
                "span does not re-slice to content — an evidence quote would be unverifiable"
            );
        }
    }

    #[test]
    fn page_is_the_page_the_block_started_on_across_a_page_break() {
        // reflow_pdf_lines already joins a paragraph that wraps a page break into
        // ONE block tagged with its starting page. The chunker must carry that
        // page verbatim rather than re-deriving it.
        let blocks = vec![
            blk(Some(2), &sentences(30)), // starts p2, would run onto p3
            blk(Some(3), "Next paragraph begins here. It has two sentences."),
        ];
        let chunks = chunk_paged(&blocks, 40, 10);
        let from_first: Vec<_> = chunks.iter().filter(|c| c.char_start < blocks[0].text.len()).collect();
        assert!(from_first.len() > 1, "the spanning paragraph should yield several chunks");
        for c in &from_first {
            assert_eq!(c.page, Some(2), "a chunk of the spanning paragraph lost its starting page");
        }
        assert_eq!(chunks.last().unwrap().page, Some(3));
    }

    #[test]
    fn pageless_sources_report_none_never_a_guess() {
        let blocks = vec![blk(None, "Only sentence here. And a second one.")];
        for c in chunk_paged(&blocks, 40, 10) {
            assert_eq!(c.page, None, "a page was invented for a page-less source");
        }
    }

    #[test]
    fn section_labels_follow_headings_and_are_none_before_the_first() {
        let blocks = vec![
            blk(Some(1), "Front matter sentence before any heading."),
            blk(Some(1), "Methods"),
            blk(Some(1), "We did the thing. Then we measured it."),
            blk(Some(2), "3. Results"),
            blk(Some(2), "The thing worked. Numbers followed."),
        ];
        let chunks = chunk_paged(&blocks, 100, 10);
        assert_eq!(chunks[0].section, None, "content before the first heading has no section");
        assert_eq!(chunks[1].section.as_deref(), Some("Methods"));
        // numbered variant — detect_heading strips the leading number
        assert_eq!(chunks[2].section.as_deref(), Some("Results"));
        // a heading is a label, not evidence: it is never emitted as a chunk
        assert!(chunks.iter().all(|c| c.content != "Methods" && c.content != "3. Results"));
    }

    #[test]
    fn a_sentence_longer_than_the_budget_becomes_its_own_chunk_uncut() {
        let long = format!("{}.", (0..80).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" "));
        let blocks = vec![blk(Some(1), &long)];
        let chunks = chunk_paged(&blocks, 20, 5);
        assert_eq!(chunks.len(), 1, "an over-budget sentence was split");
        assert_eq!(chunks[0].content, long);
    }

    #[test]
    fn empty_and_heading_only_input_yield_no_chunks() {
        assert!(chunk_paged(&[], 40, 10).is_empty());
        assert!(chunk_paged(&[blk(Some(1), "References")], 40, 10).is_empty());
    }
}
