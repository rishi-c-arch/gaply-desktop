//! **What would a design precondition actually save?**
//!
//! Stage 2 evaluates every standard against every manuscript. This counts the
//! verdicts it produces today, by status, so the gate's payoff is a measured
//! reduction rather than a quoted one. `Unevaluable` is counted apart from
//! `NotFound` deliberately — §11 records that conflating them "invents a
//! compliance failure", and only `NotFound` is the noise a gate removes.
use gaply_core::extract::{self, docparse};
use gaply_core::journal_standards::Standard;
use gaply_core::report::{evaluate, ItemStatus};

const ALL: &[Standard] = &[
    Standard::Consort, Standard::Prisma, Standard::Strobe, Standard::Arrive,
    Standard::Tripod, Standard::Cheers, Standard::Spirit, Standard::Stard,
];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let (mut met, mut notfound, mut uneval) = (0usize, 0usize, 0usize);
    println!("{:<46} {:>5} {:>9} {:>11}", "manuscript", "met", "notfound", "unevaluable");
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let (mut m, mut n, mut u) = (0usize, 0usize, 0usize);
        for s in ALL {
            for v in evaluate(*s, &ex, &text).verdicts {
                match v.status {
                    ItemStatus::Met => m += 1,
                    ItemStatus::NotFound => n += 1,
                    ItemStatus::Unevaluable => u += 1,
                }
            }
        }
        println!("{:<46} {m:>5} {n:>9} {u:>11}", name.chars().take(46).collect::<String>());
        met += m;
        notfound += n;
        uneval += u;
    }
    let total = met + notfound + uneval;
    println!("\n  TOTAL verdicts over {} manuscripts x 8 standards: {total}", paths.len());
    println!("    Met         {met}");
    println!("    NotFound    {notfound}");
    println!("    Unevaluable {uneval}");
    println!("  per manuscript, all 8 standards: {:.1} verdicts", total as f64 / paths.len() as f64);
}
