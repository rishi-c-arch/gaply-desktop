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
}
