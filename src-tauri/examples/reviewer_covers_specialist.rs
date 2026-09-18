//! **Does the report a user already opens cover the two specialist findings?**
//!
//! `claim_evidence_strength` produced 2 Major causal-overclaim findings on
//! `Disha Correction .docx`, and they live in `PipelineResult.specialists`,
//! which nothing reads. The design question is whether a NEW surface is needed
//! or whether the existing report already says the same thing by another route —
//! so this prints every finding the report DOES carry, and the reviewer letter's
//! own text, before anything is designed.
use app_lib::pipeline::{run_pipeline_measured, NetworkConsent};
use gaply_core::db::Database;
use gaply_core::embed::{Embedder, HashEmbedder};
use std::sync::Arc;

fn main() {
    std::env::set_var("GAPLY_DISABLE_DEEP", "1");
    for path in std::env::args().skip(1) {
        let name =
            std::path::Path::new(&path).file_name().unwrap().to_string_lossy().to_string();
        let db = Arc::new(Database::in_memory().expect("in-memory db"));
        let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
        let out = match run_pipeline_measured(
            db, embedder, path.clone(), None, None, None, None, NetworkConsent::Denied, &|_e| {},
        ) {
            Ok(o) => o,
            Err(e) => { println!("\n######## {name}\n  ERROR {e:?}"); continue }
        };
        println!("\n######## {name}");
        println!("  verdict  : {}", out.report.verdict);
        println!("  findings in the REPORT (what a user opens): {}", out.report.findings.len());
        for f in &out.report.findings {
            println!("    [{:?}] {:?} — {}", f.severity, f.agent, f.title);
        }
        // The specialist findings, for side-by-side comparison.
        let spec: Vec<&str> = out
            .specialists
            .iter()
            .flat_map(|r| r.admitted.iter())
            .map(|f| f.code.as_str())
            .collect();
        println!("  codes in SPECIALISTS (what nothing reads): {spec:?}");
        // Does anything in the report speak to causal overclaiming at all?
        let hits: Vec<&str> = out
            .report
            .findings
            .iter()
            .filter(|f| {
                let t = format!("{} {}", f.title, f.detail).to_lowercase();
                t.contains("caus") || t.contains("cross-sectional") || t.contains("design")
            })
            .map(|f| f.title.as_str())
            .collect();
        println!("  report findings mentioning caus/design/cross-sectional: {}", hits.len());
        // The ONE report finding that is genuinely about a causal claim, in full,
        // beside the specialist's — the comparison the design decision needs.
        for f in out.report.findings.iter().filter(|f| f.title.contains("causal")) {
            println!("\n  == REPORT's causal finding ==");
            println!("     severity : {:?}", f.severity);
            println!("     agent    : {:?}", f.agent);
            println!("     title    : {}", f.title);
            println!("     detail   : {}", f.detail);
            println!("     location : {:?}", f.location);
        }
        // And how much of the report is one repeated kind, since that decides
        // whether a new surface would even be seen.
        let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
        for f in &out.report.findings {
            let k = f.title.split(" — ").next().unwrap_or(&f.title).to_string();
            *kinds.entry(k).or_default() += 1;
        }
        // **How many DISTINCT duplication facts are in those rows?** A count is
        // not a fact: 278 rows saying "this thesis repeats its methodology" is
        // one fact with a multiplicity, and the honest report is the fact.
        let dups: Vec<&_> = out
            .report
            .findings
            .iter()
            .filter(|f| f.title.starts_with("internal duplication"))
            .collect();
        if !dups.is_empty() {
            let mut by_loc: std::collections::BTreeMap<String, usize> = Default::default();
            for f in &dups {
                let k = match &f.location {
                    Some(l) => format!("{:?} sec{:?}", l.section, l.section_index),
                    None => "<no location>".into(),
                };
                *by_loc.entry(k).or_default() += 1;
            }
            println!("\n  {} duplication rows, by SOURCE location:", dups.len());
            let mut v: Vec<_> = by_loc.into_iter().collect();
            v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            for (k, n) in v.iter().take(10) { println!("    {n:>4}  {k}"); }
            println!("  distinct source locations: {}", v.len());
            // And the excerpts themselves: how many distinct texts are being matched?
            let mut texts: std::collections::BTreeSet<String> = Default::default();
            for f in &dups {
                texts.insert(f.detail.chars().take(90).collect::<String>());
            }
            println!("  distinct excerpts: {}", texts.len());
            // **Is this prose duplication, or tables matching tables?** A
            // correlation matrix shares most of its TOKENS with any other
            // correlation matrix, and word-overlap over digits means nothing.
            // Decidable: what fraction of each excerpt's tokens are numeric?
            let mut numeric_heavy = 0usize;
            for t in &texts {
                let toks: Vec<&str> = t.split_whitespace().collect();
                if toks.is_empty() { continue }
                let nums = toks
                    .iter()
                    .filter(|w| w.chars().any(|c| c.is_ascii_digit())
                        && w.chars().all(|c| c.is_ascii_digit() || ".,-()%±".contains(c)))
                    .count();
                if nums * 2 > toks.len() { numeric_heavy += 1; }
            }
            println!("  of those, MAJORITY-NUMERIC (tables, matrices): {numeric_heavy}");
            println!("  predominantly prose: {}", texts.len() - numeric_heavy);
        }

        // **The grouping key the Finding drops.** `MatchSpan.manuscript_chunk_seq`
        // survives into `PipelineResult.plagiarism`, so the clusters can be
        // measured even though a Finding cannot be grouped by location.
        // Contiguous runs of chunk seq = one repeated passage.
        {
            let mut seqs: Vec<i64> =
                out.plagiarism.self_matches.iter().map(|m| m.manuscript_chunk_seq).collect();
            seqs.sort_unstable();
            seqs.dedup();
            let mut runs = 0usize;
            let mut prev: Option<i64> = None;
            let mut lens: Vec<i64> = Vec::new();
            let mut cur = 0i64;
            for s in &seqs {
                match prev {
                    Some(p) if *s == p + 1 => cur += 1,
                    _ => { if cur > 0 { lens.push(cur) } runs += 1; cur = 1 }
                }
                prev = Some(*s);
            }
            if cur > 0 { lens.push(cur) }
            println!("\n  plagiarism: {} matches over {} chunks ({} corpus chunks available)",
                out.plagiarism.self_matches.len(), out.plagiarism.chunk_count,
                out.plagiarism.corpus_chunks_available);
            println!("  distinct manuscript chunks matched : {}", seqs.len());
            println!("  CONTIGUOUS RUNS (= repeated passages): {runs}");
            lens.sort_unstable_by(|a, b| b.cmp(a));
            println!("  longest runs (chunks): {:?}", lens.iter().take(8).collect::<Vec<_>>());
        }

        println!("\n  report findings by kind:");
        let mut v: Vec<_> = kinds.into_iter().collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (k, n) in v.iter().take(8) {
            println!("    {n:>4}  {k}");
        }
    }
}
