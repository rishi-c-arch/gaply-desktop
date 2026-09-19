//! **SYNTHETIC PAIR. Read the label before the rows.**
//!
//! `tests_absent_from_the_record` is meant to run on a manuscript AND THE
//! ANALYSIS THAT PRODUCED IT. No such pair exists on this machine: the one real
//! analysis artefact is SPSS's journal, whose dataset is gone, and no manuscript
//! here belongs to that study. So this pairs UNRELATED files. Every finding is
//! an artefact of the pairing, not a discrepancy in anyone's work.
use std::path::Path;
use gaply_core::analysis::spss;
use gaply_core::extract;
use gaply_core::specialist::{self, SpecialistInput};

fn main() {
    let mut a = std::env::args().skip(1);
    let jnl = a.next().expect("usage: analysis_pair_probe <journal> <manuscript>...");
    let text = std::fs::read_to_string(&jnl).expect("read journal");
    let record = spss::parse("statistics.jnl", &text);
    println!("record: {} procedures, {} statistical, kinds {:?}\n",
             record.procedures.len(), record.statistical().count(), record.kinds());
    for p in a {
        let Ok(t) = extract::docparse::parse_path(Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&t);
        let tests: Vec<String> = ex.statistics.iter().filter_map(|s| match &s.stat {
            extract::stats::Stat::Test { name, raw } => Some(format!("{name} `{raw}`")),
            _ => None,
        }).collect();
        let sin = SpecialistInput { extraction: &ex, science: None, analysis: Some(&record) };
        let rep = specialist::run(
            specialist::shipped().iter().find(|s| s.id() == "frequentist_stats").unwrap().as_ref(),
            &sin,
        );
        let hits: Vec<_> = rep.admitted.iter()
            .filter(|f| f.code == "test_claimed_but_absent_from_analysis").collect();
        println!("=== {} — {} Test stat(s) in prose, {} finding(s)", base(&p), tests.len(), hits.len());
        for t in tests.iter().take(3) { println!("      prose: {}", trunc(t, 100)); }
        for f in hits.iter().take(3) {
            println!("      [{:?}] {}", f.severity, trunc(&f.summary, 160));
            match &f.span { Some(s) => println!("        span: {}", trunc(s, 140)), None => println!("        span: (none)") }
        }
        println!();
    }
}
fn base(p: &str) -> String { Path::new(p).file_name().unwrap().to_string_lossy().to_string() }
fn trunc(s: &str, n: usize) -> String { if s.chars().count()<=n {s.into()} else {s.chars().take(n).collect()} }
