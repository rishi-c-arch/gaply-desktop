//! **Is the self-plagiarism signal duplication, or is it English?**
//!
//! The shipped app embeds with `HashEmbedder` (`src/lib.rs`, the Tauri setup) —
//! signed-hash bag-of-words, no stopword removal, no IDF. 63% of all chunks in
//! the 20-manuscript corpus match something in their own document, which is the
//! uniform-result tell. The negative control it needs: chunks from DIFFERENT,
//! unrelated manuscripts, which share no content at all.
//!
//! If those score at or above the threshold that publishes a finding, the
//! detector is measuring prose style, not duplication.
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::extract::docparse;

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x * y) as f64).sum()
}

/// The same 512-token / 448-step windowing the plagiarism lane uses, by word.
fn chunks(text: &str) -> Vec<String> {
    let w: Vec<&str> = text.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < w.len() {
        out.push(w[i..(i + 512).min(w.len())].join(" "));
        if i + 512 >= w.len() {
            break;
        }
        i += 448;
    }
    out
}

fn main() {
    let e = HashEmbedder;
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut docs: Vec<(String, Vec<Vec<f32>>)> = Vec::new();
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(t) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let v: Vec<Vec<f32>> = chunks(&t).iter().filter_map(|c| e.embed(c).ok()).collect();
        if !v.is_empty() {
            docs.push((name, v));
        }
    }

    let mut cross: Vec<f64> = Vec::new();
    for i in 0..docs.len() {
        for j in (i + 1)..docs.len() {
            for a in docs[i].1.iter().take(12) {
                for b in docs[j].1.iter().take(12) {
                    cross.push(cosine(a, b));
                }
            }
        }
    }
    cross.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let pct = |p: f64| cross[((cross.len() - 1) as f64 * p) as usize];
    println!("CROSS-MANUSCRIPT chunk pairs (unrelated documents): {}", cross.len());
    println!("  min {:.3}   p25 {:.3}   MEDIAN {:.3}   p75 {:.3}   p95 {:.3}   max {:.3}",
        cross[0], pct(0.25), pct(0.50), pct(0.75), pct(0.95), cross[cross.len() - 1]);
    for t in [0.5, 0.6, 0.7, 0.8, 0.85, 0.9] {
        let n = cross.iter().filter(|x| **x >= t).count();
        println!("  >= {t:.2}: {n:>7} ({:.1}%)  <- would publish a finding at this threshold",
            100.0 * n as f64 / cross.len() as f64);
    }
}
