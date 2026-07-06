//! Embedding abstraction for semantic memory.
//!
//! The pipeline depends on the [`Embedder`] trait, not a model. The default
//! [`HashEmbedder`] is a deterministic 384-dim feature-hashing bag-of-words
//! encoder: dependency-free, offline, and stable across runs — a drop-in
//! placeholder until a real all-MiniLM-L6-v2 runtime (ONNX) is wired in.
//! Swapping models means implementing this trait; nothing else changes.

use crate::error::GaplyError;
use crate::vector::EMBEDDING_DIM;

pub trait Embedder: Send + Sync {
    fn dim(&self) -> usize {
        EMBEDDING_DIM
    }
    fn embed(&self, text: &str) -> Result<Vec<f32>, GaplyError>;
}

/// FNV-1a, implemented locally because std's DefaultHasher is not guaranteed
/// stable across Rust releases — embeddings must be reproducible.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Default)]
pub struct HashEmbedder;

impl Embedder for HashEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, GaplyError> {
        let mut v = vec![0.0f32; EMBEDDING_DIM];
        for word in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
            if word.is_empty() {
                continue;
            }
            let h = fnv1a(word.as_bytes());
            let idx = (h % EMBEDDING_DIM as u64) as usize;
            let sign = if (h >> 63) == 0 { 1.0 } else { -1.0 };
            v[idx] += sign;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut v {
                *x /= norm;
            }
        }
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_normalized() {
        let e = HashEmbedder;
        let a = e.embed("reference style citations").unwrap();
        let b = e.embed("reference style citations").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), EMBEDDING_DIM);
        let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm: {norm}");
    }

    #[test]
    fn similar_texts_are_closer_than_unrelated() {
        let e = HashEmbedder;
        let dist = |a: &[f32], b: &[f32]| -> f32 {
            a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f32>().sqrt()
        };
        let q = e.embed("citation reference style for journals").unwrap();
        let close = e.embed("journal reference style and citation format").unwrap();
        let far = e.embed("mitochondrial dna sequencing protocol").unwrap();
        assert!(dist(&q, &close) < dist(&q, &far));
    }
}
