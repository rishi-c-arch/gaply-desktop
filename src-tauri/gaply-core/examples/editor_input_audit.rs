//! **Does the editor layer have anything to order?** §5.7 gives it severity
//! precedence over the reviewer reports; this measures what those reports
//! actually contain across the corpus, before a posture rule is written.
//!
//! §5.7's inputs are *"all reviewer reports, the disagreement map, the journal
//! fingerprint, and Tier 0/1 findings"*. This probe covers the reports and the
//! Tier 0/1 findings. The journal layer is included as **all five reporting
//! standards evaluated**, which is what Nature Medicine actually binds; a
//! journal binding fewer produces fewer rows. The disagreement map is not
//! covered because nothing produces one.

use gaply_core::extract::{self, docparse};
use gaply_core::journal_standards::Standard;
use gaply_core::report::{evaluate, StandardEvaluation};
use gaply_core::editor::{decide, EditorInput, Posture};
use gaply_core::review_lens::{lenses, review, LensInput, ReviewSeverity, ReviewerReport};
use gaply_core::specialist::{self, SpecialistInput};
use std::collections::BTreeMap;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let n = paths.len();
    let (mut blocking, mut major, mut minor, mut info) = (0usize, 0usize, 0usize, 0usize);
    let mut by_code: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_code_sev: BTreeMap<String, ReviewSeverity> = BTreeMap::new();
    let mut postures: BTreeMap<Posture, usize> = BTreeMap::new();
    let mut docs_with_blocking = 0usize;
    let mut docs_with_any = 0usize;
    let mut strengths = 0usize;

    println!(
        "{:<44} {:>5} {:>5} {:>5} {:>5}  {:<15} {}",
        "manuscript", "BLOCK", "MAJOR", "MINOR", "str", "POSTURE", "surfaced cause"
    );
    for p in &paths {
        let Ok(text) = docparse::parse_path(std::path::Path::new(p)) else { continue };
        let ex = extract::extract_from_text(&text);
        let sin = SpecialistInput { extraction: &ex, science: None, analysis: None };
        let reports: Vec<_> =
            specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &sin)).collect();
        let validity = gaply_core::validate::validate(&ex);
        let standards: Vec<StandardEvaluation> = [
            Standard::Consort, Standard::Prisma, Standard::Strobe,
            Standard::Arrive, Standard::Tripod,
        ]
        .iter()
        .map(|s| evaluate(*s, &ex, &text))
        .collect();

        let input = LensInput {
            extraction: &ex,
            full_text: Some(&text),
            this_year: 2026,
            specialists: Some(&reports),
            validity: Some(&validity),
            standards: Some(&standards),
            checklist: None,
            novelty: None,
        };

        let (mut b, mut m, mut mi, mut st) = (0usize, 0usize, 0usize, 0usize);
        let mut reports: Vec<ReviewerReport> = Vec::new();
        for l in lenses() {
            let r = review(&l, &input);
            reports.push(r.clone());
            st += r.strengths.len();
            for c in r.major_concerns.iter().chain(r.minor_concerns.iter()) {
                *by_code.entry(c.code.clone()).or_default() += 1;
                by_code_sev.insert(c.code.clone(), c.severity);
                match c.severity {
                    ReviewSeverity::Blocking => b += 1,
                    ReviewSeverity::Major => m += 1,
                    ReviewSeverity::Minor => mi += 1,
                    ReviewSeverity::Informational => info += 1,
                }
            }
        }
        blocking += b;
        major += m;
        minor += mi;
        strengths += st;
        if b > 0 { docs_with_blocking += 1; }
        if b + m + mi > 0 { docs_with_any += 1; }
        let rubric = decide(&EditorInput { reports: &reports, journal: Some("nature-medicine") });
        *postures.entry(rubric.posture).or_default() += 1;
        let cause = rubric
            .primary_causes
            .first()
            .map(|c| format!("{} x{}", c.code, c.occurrences))
            .unwrap_or_else(|| "-".into());
        println!(
            "{:<44} {b:>5} {m:>5} {mi:>5} {st:>5}  {:<15} {cause}",
            short(p),
            rubric.posture.as_str()
        );
    }

    println!("\n=== WHAT THE EDITOR WOULD RECEIVE over {n} manuscript(s) ===");
    println!("  BLOCKING      {blocking}");
    println!("  MAJOR         {major}");
    println!("  MINOR         {minor}");
    println!("  INFORMATIONAL {info}");
    println!("  strengths     {strengths}");
    println!("  manuscripts with >=1 concern  {docs_with_any} of {n}");
    println!("  manuscripts with >=1 BLOCKING {docs_with_blocking} of {n}");

    println!("\n=== BY CODE (with severity) ===");
    let mut rs_total = 0usize;
    for (c, v) in &by_code {
        let sev = by_code_sev.get(c).copied().unwrap();
        let is_rs = matches!(
            c.as_str(),
            "AbstractPresent" | "ResultsSectionPresent" | "SampleSizeReported"
                | "StatisticalTestReported" | "EffectSizeReported" | "ConfidenceIntervalReported"
        );
        if is_rs && sev == ReviewSeverity::Major {
            rs_total += v;
        }
        println!("  {v:>4}  [{}] {c}{}", sev.as_str(), if is_rs { "   (reporting standard)" } else { "" });
    }
    println!("\n  of {major} MAJOR, {rs_total} are reporting-standard items");

    println!("\n=== POSTURE DISTRIBUTION ===");
    for (p, n) in &postures {
        println!("  {:<16} {n} of {}", p.as_str(), paths.len());
    }

    println!("\n=== SEVERITY PRECEDENCE: is there anything to order? ===");
    if blocking == 0 {
        println!("  NO. Zero BLOCKING findings across the corpus, so the rule §5.7 calls");
        println!("  the editor's job — a blocking finding outranks any number of strengths —");
        println!("  has never had a case to decide on real input.");
    } else {
        println!("  Yes: {blocking} blocking finding(s) to place ahead of {major} major.");
    }
}

fn short(p: &str) -> String {
    std::path::Path::new(p).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}
