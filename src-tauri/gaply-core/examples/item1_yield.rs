//! **Item 1, measured before wiring: what do the shipping specialists and the
//! reporting-standard evaluators actually produce on 20 real manuscripts?**
//!
//! Both stages need ONLY `extraction` (+ manuscript text for standards), which
//! the pipeline already has — so wiring them is a call, not a new input. The
//! question this answers is whether the call is worth making: a stage that
//! yields almost nothing on real input is not worth three days of plumbing,
//! and §11 D165/D166 are two lanes already declined on exactly that test.
use gaply_core::extract::{self, docparse};
use gaply_core::journal_standards::Standard;
use gaply_core::specialist::{self, SpecialistInput};
use std::collections::BTreeMap;

const STANDARDS: &[Standard] = &[
    Standard::Consort,
    Standard::Prisma,
    Standard::Strobe,
    Standard::Arrive,
    Standard::Tripod,
];

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut by_spec: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // (ran, findings)
    let mut declined: BTreeMap<String, usize> = BTreeMap::new();
    let (mut docs, mut docs_with_spec_finding) = (0usize, 0usize);
    let mut std_rows: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    let mut examples: Vec<String> = Vec::new();

    println!("{:<46} {:>6} {:>6} {:>7}", "manuscript", "spec", "unmet", "stds");
    for p in &paths {
        let name = std::path::Path::new(p).file_name().unwrap().to_string_lossy().to_string();
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        docs += 1;
        let ex = extract::extract_from_text(&text);

        // --- stage 1: specialists -----------------------------------------
        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let mut doc_findings = 0usize;
        for s in specialist::shipped() {
            let r = specialist::run(s.as_ref(), &sin);
            let e = by_spec.entry(r.specialist.to_string()).or_default();
            if r.not_applicable.is_some() {
                *declined.entry(r.specialist.to_string()).or_default() += 1;
            } else {
                e.0 += 1;
                e.1 += r.admitted.len();
                doc_findings += r.admitted.len();
                for f in r.admitted.iter().take(1) {
                    if examples.len() < 6 {
                        examples.push(format!(
                            "  [{}] {} :: {}",
                            r.specialist,
                            f.code,
                            f.span.as_deref().unwrap_or("(no span)").chars().take(90).collect::<String>()
                        ));
                    }
                }
            }
        }
        if doc_findings > 0 {
            docs_with_spec_finding += 1;
        }

        // --- stage 2: reporting standards ---------------------------------
        let mut unmet = 0usize;
        for s in STANDARDS {
            let ev = gaply_core::report::evaluate(*s, &ex, &text);
            let e = std_rows.entry(format!("{s:?}")).or_default();
            for v in &ev.verdicts {
                *e.entry(format!("{:?}", v.status)).or_default() += 1;
            }
            unmet += ev
                .verdicts
                .iter()
                .filter(|v| matches!(v.status, gaply_core::report::ItemStatus::NotFound))
                .count();
        }
        println!("{:<46} {:>6} {:>6} {:>7}", name, doc_findings, unmet, STANDARDS.len());
    }

    println!("\n================= STAGE 1: SPECIALISTS =================");
    println!("{:<24} {:>8} {:>10} {:>10}", "specialist", "ran", "declined", "findings");
    for (s, (ran, f)) in &by_spec {
        println!("{:<24} {:>8} {:>10} {:>10}", s, ran, declined.get(s).copied().unwrap_or(0), f);
    }
    println!("\n  manuscripts                      {docs}");
    println!("  with >=1 specialist finding      {docs_with_spec_finding}");
    println!("\n  example findings (span, not count):");
    for e in &examples {
        println!("{e}");
    }

    println!("\n============= STAGE 2: REPORTING STANDARDS =============");
    for (s, counts) in &std_rows {
        let total: usize = counts.values().sum();
        print!("{s:<14} total={total:<5}");
        for (st, n) in counts {
            print!("  {st}={n}");
        }
        println!();
    }
}
