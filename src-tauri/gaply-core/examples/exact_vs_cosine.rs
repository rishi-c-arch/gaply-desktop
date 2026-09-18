//! **The other plagiarism path, measured.** `plagiarism_exact` is deterministic
//! winnowing + verified verbatim matching, already wired as the
//! `check_plagiarism_exact` Tauri command. The embedding-cosine lane produced
//! 1879 self-matches across this corpus with zero verified real. This asks what
//! the exact matcher says about the same 20 documents, and prints the TEXT of
//! what it finds — the same reading standard, not a second count to trust.
use gaply_core::extract::docparse;
use gaply_core::plagiarism_exact::{analyze_exact, ExactConfig};

fn main() {
    let cfg = ExactConfig::default();
    let mut grand = 0usize;
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let r = analyze_exact(&text, &[], &cfg);
        grand += r.self_matches.len();
        println!(
            "{:<46} self_matches {:>3}   dup_ratio {:.3}   words {}",
            name.chars().take(46).collect::<String>(),
            r.self_matches.len(),
            r.duplication_ratio,
            r.total_source_words
        );
        // The rows, longest first — a count is what the cosine lane offered.
        let mut ms = r.self_matches.clone();
        ms.sort_by_key(|m| std::cmp::Reverse(m.word_count));
        let long = ms.iter().filter(|m| m.word_count >= 50).count();
        let vlong = ms.iter().filter(|m| m.word_count >= 150).count();
        println!("    >=50 words: {long}   >=150 words: {vlong}");
        for m in ms.iter().take(2) {
            println!("    {} words, jaccard {:.3}  A[{}..{}] B[{}..{}]  gap {}", m.word_count, m.similarity, m.source.start_char, m.source.end_char, m.matched.start_char, m.matched.end_char, (m.matched.start_char as i64 - m.source.start_char as i64).abs());
            println!("      A: {}", m.source.text.chars().take(190).collect::<String>());
            println!("      B: {}", m.matched.text.chars().take(190).collect::<String>());
        }
    }
    println!("\n  exact self-matches across the corpus: {grand}   (cosine lane: 1879)");
}
