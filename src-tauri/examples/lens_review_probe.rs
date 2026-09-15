//! **Phase 4b's deliverable: reviewer reports on one real manuscript against
//! one real journal.**
//!
//! Crawl → extract → store → fingerprint → specialists → lenses. Nothing is
//! hand-fed: every journal row's span is the journal's own sentence and can be
//! checked against the live page, and every manuscript span is the manuscript's.
//!
//! **The surfaced row is the pipeline's, not one chosen here.** §11 D163: the
//! text-mining quota was found because the probe printed the NEWEST word limit
//! and that happened to be the bad one. So this prints the report's own first
//! major concern — highest severity, first in the lens's order — and says which
//! rule put it there, rather than a row picked for looking good.
//!
//! ```text
//! cargo run --release --example lens_review_probe -- <manuscript> [<journal-key> <entry-url>]
//! ```

use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use app_lib::journal_crawl::{crawl, CrawlBudget};
use gaply_core::extract::{self, docparse};
use gaply_core::journal_extract::extract_requirements;
use gaply_core::journal_standards::{bindings_from, Standard};
use gaply_core::journal_store::{requirements_for, store_requirements};
use gaply_core::novelty::{self, NoveltyAssessment};
use gaply_core::ratelimit::RateLimiter;
use gaply_core::refverify::{ApiRateLimiters, HttpFetcher, HttpRequest, VerifyContext};
use gaply_core::report::{checklist_from_requirements, evaluate, ChecklistItem, StandardEvaluation};
use gaply_core::editor::{decide, EditorInput};
use gaply_core::review_lens::{lenses, review, CriterionState, LensInput, ReviewerReport, DECLINED_LENSES};
use gaply_core::specialist::{self, SpecialistInput};
use gaply_core::Database;

const THIS_YEAR: i32 = 2026;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: lens_review_probe <manuscript> [<key> <entry-url>]");
    let journal = args.next().zip(args.next());

    let text = docparse::parse_path(std::path::Path::new(&path)).expect("parse");
    let ex = extract::extract_from_text(&text);
    let words = text.split_whitespace().count();

    // ---- the journal layer, from a live crawl
    let db = Database::in_memory().unwrap();
    let mut checklist: Option<Vec<ChecklistItem>> = None;
    let mut bound: Vec<Standard> = Vec::new();
    if let Some((key, entry)) = &journal {
        let budget = CrawlBudget::from_json(include_str!("../config/journal-crawl.json")).unwrap();
        let f = ReqwestFetcher::new().unwrap();
        let o = crawl(&f, &RateLimiter::new(4.0, 4.0), &budget, entry).expect("crawl");
        let mut bindings = Vec::new();
        for p in o.pages.iter().filter(|p| p.verdict == "guideline") {
            let Ok(r) = f.get(&HttpRequest::get(&p.url)) else { continue };
            let reqs = extract_requirements(&html_to_blocks(&r.body));
            bindings.extend(bindings_from(&reqs));
            let _ = store_requirements(&db, key, &p.url, &reqs, 1);
        }
        bindings.sort();
        bindings.dedup();
        let stored = requirements_for(&db, key).unwrap();
        println!(
            "JOURNAL: {key}\n  crawl fetched={} guideline={} -> {} stored requirement(s), \
             {} standard binding(s)",
            o.fetched,
            o.guideline,
            stored.len(),
            bindings.len()
        );
        bound = bindings.iter().map(|b| b.standard).collect();
        bound.sort();
        bound.dedup();
        checklist = Some(checklist_from_requirements(&ex, &text, words, &stored, &bindings));
    } else {
        println!("JOURNAL: none supplied — the compliance section must say so, not print empty");
    }

    // ---- producers
    let sinput = SpecialistInput { extraction: &ex, science: None, analysis: None };
    let reports: Vec<_> =
        specialist::shipped().iter().map(|s| specialist::run(s.as_ref(), &sinput)).collect();
    let validity = gaply_core::validate::validate(&ex);
    // **Only the standards the journal actually binds.** Evaluating all five
    // against a journal that binds two would report items no journal asked for.
    let standards: Vec<StandardEvaluation> = bound.iter().map(|s| evaluate(*s, &ex, &text)).collect();

    let dir = std::env::temp_dir().join("gaply-lens-review");
    std::fs::create_dir_all(&dir).unwrap();
    let ndb = Database::open(&dir.join("n.db")).unwrap();
    let http = ReqwestFetcher::new().unwrap();
    let limiters = ApiRateLimiters::with_polite_defaults();
    let ctx = VerifyContext { db: &ndb, http: &http, limiters: &limiters, contact_email: None };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let novelty_assessments: Vec<NoveltyAssessment> = novelty::extract_claims(&ex)
        .iter()
        .map(|c| {
            let r = novelty::retrieve(&ctx, c, 10, now).unwrap_or_default();
            novelty::assess(c, &ex.references, &r)
        })
        .collect();

    println!(
        "\nMANUSCRIPT: {}\n  {words} words, {} sections, {} statistics, {} references",
        short(&path),
        ex.sections.len(),
        ex.statistics.len(),
        ex.references.len()
    );
    for r in &reports {
        match &r.not_applicable {
            Some(w) => println!("  [{}] NOT APPLICABLE — {w}", r.specialist),
            None => println!(
                "  [{}] {} admitted, {} rejected",
                r.specialist,
                r.admitted.len(),
                r.rejected.len()
            ),
        }
    }
    println!(
        "  [validate.rs] {} flag(s)   [standards] {} bound   [novelty] {} claim(s)",
        validity.flags.len(),
        standards.len(),
        novelty_assessments.len()
    );

    let input = LensInput {
        extraction: &ex,
        full_text: Some(&text),
        this_year: THIS_YEAR,
        specialists: Some(&reports),
        validity: Some(&validity),
        standards: Some(&standards),
        checklist: checklist.as_deref(),
        novelty: Some(&novelty_assessments),
    };

    let mut first_surfaced: Option<(String, gaply_core::review_lens::Concern)> = None;
    let mut all_reports: Vec<ReviewerReport> = Vec::new();

    for l in lenses() {
        let r = review(&l, &input);
        all_reports.push(r.clone());
        println!("\n{}", "=".repeat(78));
        println!("REVIEWER REPORT — {} lens", r.lens.as_str().to_uppercase());
        println!("{}", "=".repeat(78));
        println!("\nOVERALL ASSESSMENT\n  {}", wrap(&r.overall_assessment));
        println!("\nCONTRIBUTION\n  {}", wrap(&r.contribution));
        println!("\nSTRENGTHS ({})", r.strengths.len());
        for s in &r.strengths {
            println!("  + {}", wrap(s));
        }
        println!("\nMAJOR CONCERNS ({})", r.major_concerns.len());
        for c in &r.major_concerns {
            print_concern(c);
        }
        println!("\nMINOR CONCERNS ({})", r.minor_concerns.len());
        for c in &r.minor_concerns {
            print_concern(c);
        }
        println!("\nJOURNAL-SPECIFIC COMPLIANCE ({})", r.journal_compliance.len());
        for line in &r.journal_compliance {
            println!("  {}", wrap(line));
        }
        println!("\nREQUIRED REVISIONS ({})", r.required_revisions.len());
        for (i, rev) in r.required_revisions.iter().enumerate() {
            println!("  {}. {}", i + 1, wrap(rev));
        }
        println!("\nREFUSED BY THE ABSENCE RULE ({})", r.absence_rejected.len());
        for a in &r.absence_rejected {
            println!("  x {} / {} — {}", a.criterion, a.code, wrap(&a.reason));
        }
        println!("\nNOT EVALUATED");
        let mut any = false;
        for c in &r.criteria {
            let (mark, detail) = match &c.state {
                CriterionState::Evaluated => continue,
                CriterionState::SourceNotRun { detail } => ("~", detail),
                CriterionState::NoShippingSource { detail } => ("!", detail),
                CriterionState::ReportedInComplianceSection { detail } => (">", detail),
            };
            any = true;
            println!("  {mark} {} — {}", c.criterion, wrap(detail));
        }
        if !any {
            println!("  (every criterion this lens declares was evaluated)");
        }

        if first_surfaced.is_none() {
            if let Some(c) = r.major_concerns.first() {
                first_surfaced = Some((r.lens.as_str().to_string(), c.clone()));
            }
        }
    }

    // ---- §5.7 the editor, over every reviewer report
    let rubric = decide(&EditorInput {
        reports: &all_reports,
        journal: journal.as_ref().map(|(k, _)| k.as_str()),
    });
    println!("\n{}", "=".repeat(78));
    println!("EDITORIAL RUBRIC  (§5.7 editor, §8 Stage 1)");
    println!("{}", "=".repeat(78));
    // **Causes first, posture after** — `rubric.display_order`. Measured: the
    // posture is MAJOR REVISION on 18 of 20 real manuscripts, so leading with
    // it opens with the same sentence for almost every researcher.
    println!("\n  PRIMARY CAUSES");
    if rubric.primary_causes.is_empty() {
        println!("    (none — no concern was raised by the lenses that ran)");
    }
    for (i, c) in rubric.primary_causes.iter().enumerate() {
        println!(
            "    {}. [{}] {} — {}  ({} lens)",
            i + 1,
            c.severity.as_str(),
            c.criterion,
            c.code,
            c.lens.as_str()
        );
        println!("       {} x {}", c.occurrences, c.occurrence_unit);
        if let Some(sp) = &c.span {
            println!("       {}", wrap(sp));
        }
    }

    println!("\n  Editorial posture:  {}", rubric.posture.as_str());
    println!("    Blocking {}  ·  Major {}  ·  Minor {}", rubric.blocking, rubric.major, rubric.minor);
    println!("    {}", wrap(&rubric.why));

    println!("\n  PRECEDENCE: {}", wrap(&rubric.precedence_note));
    println!("\n  WHAT THE EDITOR DID NOT READ:");
    for x in &rubric.not_read {
        println!("    - {}", wrap(x));
    }

    println!("\n{}", "=".repeat(78));
    println!("DECLINED LENSES");
    println!("{}", "=".repeat(78));
    for d in DECLINED_LENSES {
        println!("\n  {} — {}", d.id.as_str(), wrap(d.reason));
    }

    println!("\n{}", "=".repeat(78));
    println!("THE ROW THE PIPELINE SURFACES");
    println!("{}", "=".repeat(78));
    match first_surfaced {
        Some((lens, c)) => {
            println!(
                "\nNot chosen: this is the first major concern of the first lens that has \
                 one, in the order `lenses()` returns them.\n"
            );
            println!("  lens     : {lens}");
            print_concern(&c);
        }
        None => println!("\n  No major concern was raised by any lens on this manuscript."),
    }
}

fn print_concern(c: &gaply_core::review_lens::Concern) {
    println!(
        "\n  [{}] {} — {}{}",
        c.severity.as_str(),
        c.criterion,
        c.code,
        if c.occurrences > 1 { format!("  x{}", c.occurrences) } else { String::new() }
    );
    println!("    source: {}", c.source.as_str());
    println!("    {}", wrap(&c.summary));
    for (i, s) in c.spans.iter().enumerate() {
        println!("    SPAN {}/{}: {}", i + 1, c.spans.len(), s.replace('\n', "\n          "));
    }
    println!("    TRAIL:");
    for t in &c.trail {
        println!("      {}: {}", t.stage, wrap(&t.detail));
    }
    if let Some(u) = &c.uncertainty {
        println!("    UNCERTAINTY: {}", wrap(u));
    }
}

/// Re-flow for the terminal. **Never truncates** — a clipped span is not a span.
fn wrap(s: &str) -> String {
    let mut out = String::new();
    let mut col = 0;
    for w in s.split_whitespace() {
        if col + w.len() > 88 && col > 0 {
            out.push_str("\n      ");
            col = 6;
        } else if col > 0 {
            out.push(' ');
            col += 1;
        }
        out.push_str(w);
        col += w.len();
    }
    out
}

fn short(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
