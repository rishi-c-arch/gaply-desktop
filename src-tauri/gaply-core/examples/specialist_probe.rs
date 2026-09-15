//! **The two Phase-4 specialists over the six real manuscripts.**
//!
//! Prints every finding with its span, whole, plus every rejection with its
//! reason. Rows, not totals: the journal phase produced "46 requirements, 3
//! conflicts" and both numbers were wrong — 29 and 1 once the rows were read,
//! and no count caught any of the four defects.
//!
//! This probe is the benchmark instrument. Precision is adjudicated by reading
//! each row against its span; recall is not measurable here and is not claimed.

use gaply_core::analysis::AnalysisRecord;
use gaply_core::extract::{self, docparse};
use gaply_core::specialist::{run, shipped, SpecialistInput};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Optional: an SPSS file, so the record-backed path can be exercised at all.
    let mut spss: Option<String> = None;
    if let Some(i) = args.iter().position(|a| a == "--spss") {
        args.remove(i);
        spss = Some(args.remove(i));
    }
    if args.is_empty() {
        eprintln!("usage: specialist_probe [--spss <file>] <manuscript>...");
        std::process::exit(2);
    }

    let record: Option<AnalysisRecord> = spss.as_ref().map(|p| {
        let name = std::path::Path::new(p)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.clone());
        gaply_core::analysis::spss::parse(&name, &std::fs::read_to_string(p).expect("read spss"))
    });
    if let Some(r) = &record {
        println!(
            "analysis record: {} procedure(s), {} statistical, kinds {:?}\n",
            r.procedures.len(),
            r.statistical().count(),
            r.kinds()
        );
    } else {
        println!("analysis record: NONE — which is the state of every real run today\n");
    }

    let specialists = shipped();
    let mut admitted_total = 0usize;
    let mut rejected_total = 0usize;

    for p in &args {
        let text = match docparse::parse_path(std::path::Path::new(p)) {
            Ok(t) => t,
            Err(e) => {
                println!("=== {} ===\n  PARSE FAILED: {e}\n", short(p));
                continue;
            }
        };
        let ex = extract::extract_from_text_with(&text, extract::ExtractOptions::with_scientific());
        let input = SpecialistInput {
            extraction: &ex,
            science: ex.scientific.as_deref(),
            analysis: record.as_ref(),
        };
        println!("=== {} ===", short(p));
        // The declaration the pipeline acts on, confirmed at the point of use.
        println!(
            "  scientific layer: {}",
            match &ex.scientific {
                Some(s) => format!(
                    "POPULATED — {} claims, {} variables, {} methods, {} datasets",
                    s.claims.len(),
                    s.variables.len(),
                    s.methods.len(),
                    s.datasets.len()
                ),
                None => "None".into(),
            }
        );

        for s in &specialists {
            let r = run(s.as_ref(), &input);
            if let Some(why) = &r.not_applicable {
                println!("  [{}] NOT APPLICABLE — {why}", r.specialist);
                continue;
            }
            println!(
                "  [{}] {} admitted, {} rejected",
                r.specialist,
                r.admitted.len(),
                r.rejected.len()
            );
            admitted_total += r.admitted.len();
            rejected_total += r.rejected.len();
            for f in &r.admitted {
                println!("    - {} [{:?}] {}", f.code, f.severity, f.summary);
                println!("      span: {}", f.span.as_deref().unwrap_or("<none>"));
                println!("      unknown: {}", f.uncertainty.as_deref().unwrap_or("<none>"));
            }
            for rej in &r.rejected {
                println!("    REJECTED {} — {}", rej.finding.code, rej.reason);
            }
        }
        println!();
    }

    println!("TOTAL admitted={admitted_total} rejected={rejected_total} over {} file(s)", args.len());
}

fn short(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.into())
}
