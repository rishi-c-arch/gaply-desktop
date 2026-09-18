//! **Read the duplication pairs.** A count said 278; the rows are what decide.
//! Prints BOTH excerpts of self-matches at or above the shipped 0.80 threshold,
//! so "these are the same text" is checkable rather than believed.
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::extract::docparse;

fn cosine(a: &[f32], b: &[f32]) -> f64 { a.iter().zip(b).map(|(x, y)| (x * y) as f64).sum() }

fn chunks(text: &str) -> Vec<String> {
    let w: Vec<&str> = text.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < w.len() {
        out.push(w[i..(i + 512).min(w.len())].join(" "));
        if i + 512 >= w.len() { break }
        i += 448;
    }
    out
}

fn main() {
    let e = HashEmbedder;
    let path = std::env::args().nth(1).unwrap();
    let text = docparse::parse_path(std::path::Path::new(&path)).unwrap();
    let cs = chunks(&text);
    let vs: Vec<Vec<f32>> = cs.iter().map(|c| e.embed(c).unwrap()).collect();
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..cs.len() {
        for j in (i + 1)..cs.len() {
            let s = cosine(&vs[i], &vs[j]);
            if s >= 0.80 { pairs.push((s, i, j)) }
        }
    }
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    println!("{} self-match pairs at >= 0.80 over {} chunks\n", pairs.len(), cs.len());
    // The pairs JUST over the bar decide whether the bar is in the right place.
    for (s, i, j) in pairs.iter().take(3) {
        println!("=== similarity {s:.3}  chunk {i} vs chunk {j} ===");
        println!("  A: {}", cs[*i].chars().take(230).collect::<String>());
        println!("  B: {}\n", cs[*j].chars().take(230).collect::<String>());
    }
    if let Some((s, i, j)) = pairs.last() {
        println!("=== HIGHEST {s:.3}  chunk {i} vs chunk {j} ===");
        println!("  A: {}", cs[*i].chars().take(230).collect::<String>());
        println!("  B: {}", cs[*j].chars().take(230).collect::<String>());
    }
}
