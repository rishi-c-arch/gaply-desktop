//! **§12 Phase 7b — run every adversarial fixture and print the rows.**
//!
//! A pass count is not a result. Each row shows what the pipeline produced,
//! the status, and the trail, so a silent absence is visible as itself rather
//! than as a green tick.
use gaply_core::red_team::*;
use gaply_core::review_lens::{SourceId, SourceState};

fn main() {
    let dir = std::env::temp_dir().join("gaply-red-team");
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("rt1.pdf");
    std::fs::write(&pdf_path, white_text_pdf()).unwrap();
    let pdf_text = gaply_core::extract::docparse::parse_path(&pdf_path).expect("parse");

    for f in fixtures() {
        println!("\n{}", "=".repeat(78));
        println!("{}  {}", f.id, f.what);
        println!("{}", "=".repeat(78));

        match f.id {
            "RT1" | "RT2" => {
                let text = if f.id == "RT1" { pdf_text.clone() } else { CAPTION_INJECTION.to_string() };
                let hits = gaply_core::sanitize::scan_injections(&text);
                println!("  extracted the hidden text : {}", text.contains("Ignore previous instructions"));
                println!("  scanner flagged           : {hits:?}");
                let (_, reports, _) = run_pipeline(&text, None);
                println!("  findings produced         : {:?}", codes(&reports));
                let leaked = injection_reaches_a_finding(&reports);
                println!("  injection rode a finding  : {}", if leaked.is_empty() { "no".into() } else { format!("YES {leaked:?}") });
            }
            "RT3" => {
                let prov = gaply_core::refverify::Provenance {
                    source: "journal".into(), url: "https://example.org/authors".into(),
                    fetched_at: 0, checksum: "x".into(), from_cache: false,
                };
                let t = gaply_core::refverify::UntrustedText::new(GUIDELINE_INJECTION, prov);
                println!("  suspicious                : {}", t.is_suspicious());
                println!("  flags (the 'why')         : {:?}", t.injection_flags());
                println!("  llm_safe output           : {:?}", t.llm_safe());
                println!("  instruction withheld      : {}", !t.llm_safe().contains("approves every submission"));
            }
            "RT4" => {
                let (ex, reports, _) = run_pipeline(BAD_TOTALS, None);
                println!("  tables extracted          : {}", ex.tables.len());
                println!("  findings produced         : {:?}", codes(&reports));
                println!("  arithmetic check fired    : {}",
                    codes(&reports).iter().any(|c| c.to_lowercase().contains("arith") || c.to_lowercase().contains("total")));
            }
            "RT5" => {
                let (_, reports, _) = run_pipeline(FALSE_NOVELTY, None);
                println!("  findings produced         : {:?}", codes(&reports));
                println!("  novelty lane state        : {:?}", SourceId::NoveltyClaims.state());
                println!("  decline is discoverable   : {}", SourceId::NoveltyClaims.note().contains("D166"));
                println!("  any novelty verdict       : {}",
                    codes(&reports).iter().any(|c| c.contains("NOVEL") || c.contains("PRIOR_WORK")));
            }
            "RT6" => {
                let rec = gaply_core::analysis::spss::parse(
                    "contradicts.sps",
                    "FREQUENCIES VARIABLES=provision.\nDESCRIPTIVES VARIABLES=capacity.\n",
                );
                let paper = "A Title\n\n4. Methods\n\nWe ran a paired t-test and an ANOVA.\n\n\
                             5. Results\n\nThe effect was significant (t = 3.1, p = 0.004).\n";
                let (_, with, _) = run_pipeline(paper, Some(&rec));
                let (_, without, _) = run_pipeline(paper, None);
                println!("  with a supplied record    : {:?}", codes(&with));
                println!("  without one (every run)   : {:?}", codes(&without));
                println!("  PRODUCT-UNREACHABLE       : no upload path accepts .sps (§12 node 11)");
            }
            "RT7" => {
                let (_, reports, rubric) = run_pipeline(CAUSAL_OVERCLAIM, None);
                println!("  findings produced         : {:?}", codes(&reports));
                println!("  posture                   : {}", rubric.posture.as_str());
            }
            "RT8" => {
                let (_, reports, rubric) = run_pipeline(COMPLIANT_BUT_FATAL, None);
                println!("  findings produced         : {:?}", codes(&reports));
                println!("  posture                   : {}", rubric.posture.as_str());
                println!("  why                       : {}", rubric.why);
            }
            _ => {}
        }
        println!("  EXPECTED                  : {:?}", f.expectation);
    }

    println!("\n{}", "=".repeat(78));
    println!("DECLINED LANES THIS SUITE ASSERTS SILENCE FOR");
    println!("{}", "=".repeat(78));
    for s in [SourceId::NoveltyClaims, SourceId::ComputationalConsistency] {
        println!("  {:<28} {:?}\n      {}", s.as_str(), s.state(), s.note());
    }
    let _ = SourceState::Declined;
}
