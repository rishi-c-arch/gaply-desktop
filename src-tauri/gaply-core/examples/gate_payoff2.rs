//! Per-standard decomposition of the 500, and what a correctly-bound STROBE
//! alone produces for the three manuscripts whose design read holds.
use gaply_core::extract::{self, docparse};
use gaply_core::journal_standards::{items_for, ItemCheck, Standard};
use gaply_core::report::{evaluate, ItemStatus};

const ALL: &[Standard] = &[
    Standard::Consort, Standard::Prisma, Standard::Strobe, Standard::Arrive,
    Standard::Tripod, Standard::Cheers, Standard::Spirit, Standard::Stard,
];

fn main() {
    println!("standard   items  of which NotImplemented (unevaluable for EVERY manuscript)");
    let mut tot = 0;
    let mut ni_tot = 0;
    for s in ALL {
        let items = items_for(*s);
        let ni = items.iter().filter(|i| matches!(i.check, ItemCheck::NotImplemented)).count();
        println!("{:<10} {:>5}  {:>3}", s.as_str(), items.len(), ni);
        tot += items.len();
        ni_tot += ni;
    }
    println!("{:<10} {:>5}  {:>3}   <- x20 manuscripts = {} / {}", "TOTAL", tot, ni_tot, ni_tot * 20, tot * 20);

    println!("\nSTROBE ALONE, per manuscript:");
    println!("{:<46} {:>5} {:>9} {:>11}", "manuscript", "met", "notfound", "unevaluable");
    for p in std::env::args().skip(1) {
        let name = std::path::Path::new(&p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(&p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let (mut m, mut n, mut u) = (0usize, 0usize, 0usize);
        for v in evaluate(Standard::Strobe, &ex, &text).verdicts {
            match v.status {
                ItemStatus::Met => m += 1,
                ItemStatus::NotFound => n += 1,
                ItemStatus::Unevaluable => u += 1,
            }
        }
        println!("{:<46} {m:>5} {n:>9} {u:>11}", name.chars().take(46).collect::<String>());
    }
}
