//! How many CRITICAL "small sample with causal claim" findings does the corpus
//! actually produce? The audit shows the READS are wrong; this shows how many
//! reach a report, which is the number that decides how urgent the fix is.
use gaply_core::extract::{self, docparse};
use gaply_core::validate::{validate, RuleId};

fn main() {
    let mut total = 0usize;
    let mut affected = 0usize;
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let r = validate(&ex);
        let flags: Vec<_> =
            r.flags.iter().filter(|f| f.rule == RuleId::SmallSampleCausalClaim).collect();
        if flags.is_empty() {
            continue;
        }
        affected += 1;
        total += flags.len();
        println!("\n######## {name}  — {} CRITICAL finding(s)", flags.len());
        for f in &flags {
            println!("    {:?}", f.location);
            println!("    {}", f.explanation);
        }
    }
    println!("\n  CRITICAL small-sample findings: {total} across {affected} manuscripts");
}
