//! THROWAWAY: render a finished audit job to PDF + HTML without the app.
//!
//!   cargo run --release --example audit_report_preview -- <job_id> <name> <outdir>
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let job: i64 = std::env::args().nth(1).unwrap_or_else(|| "7".into()).parse()?;
    let name = std::env::args().nth(2).unwrap_or_else(|| "R PAPER .docx".into());
    let out = std::path::PathBuf::from(
        std::env::args().nth(3).unwrap_or_else(|| "/tmp".into()),
    );
    let home = std::path::PathBuf::from(std::env::var("HOME")?);
    let app_data = home.join("Library/Application Support/ai.gaply.app");
    let db = gaply_core::Database::open(&app_data.join("gaply.db"))?;

    let installed =
        app_lib::ai::generative::resolve_generative_loader(&db, &app_data).map(|l| l.model_id());
    let today = app_lib::audit_export::today_label();
    let m = app_lib::audit_export::build_model_with(
        &db,
        job,
        &name,
        &today,
        installed.as_deref(),
    )?;
    // Optional 4th arg: recompute the consistency findings from the manuscript.
    //
    // They are PERSISTED in `ai_jobs.summary_json` at audit time, so a preview
    // of an old job replays the wording that shipped with it. That is correct
    // behaviour and it is also why a re-render cannot show a fix to those
    // strings: pass the file to see what a fresh run would say.
    let mut m_fix: Option<Vec<gaply_core::consistency::ConsistencyFinding>> = None;
    if let Some(src) = std::env::args().nth(4) {
        let p = std::path::Path::new(&src);
        let blocks = gaply_core::extract::docparse::parse_path_paged(p)?;
        let report = gaply_core::ai_engine::audit_prepass::prepass_blocks(&blocks);
        let c = gaply_core::consistency::check_consistency(&blocks, &report);
        println!("  recomputed consistency from {src}: {} findings", c.findings.len());
        m_fix = Some(c.findings);
    }
    println!(
        "job {job}: {} items, {} checked, has_pages={}, model={:?}",
        m.total_sentences, m.checked, m.has_pages, m.model_id
    );
    // §11 D108. `health_score` is gone: every input came from the support
    // verdict, which measured as a constant. The counts below are what remain
    // true, and the count of items carrying a QUOTED passage replaces it —
    // that is what the report now leads with.
    println!(
        "  supported={} needs={} unverifiable={} failed={} with_passages={}",
        m.supported.len(),
        m.needs_citation.len(),
        m.unverifiable.len(),
        m.failed.len(),
        m.supported.iter().filter(|i| !i.evidence.is_empty()).count()
    );

    // §11 D72: how many items carry a locator. A .docx has no pages, so this
    // is the number that says whether the paragraph locator reached the report.
    let located = m
        .supported
        .iter()
        .chain(m.needs_citation.iter())
        .chain(m.unverifiable.iter())
        .chain(m.failed.iter())
        .filter(|i| i.page.is_some() || i.paragraph.is_some())
        .count();
    let total = m.supported.len() + m.needs_citation.len() + m.unverifiable.len() + m.failed.len();
    println!("  items carrying a locator: {located}/{total}");

    let mut m = m;
    if let Some(f) = m_fix {
        m.consistency = f;
    }
    let blocks = gaply_core::audit_report::compose_audit(&m);
    let pdf = gaply_core::report_pdf::render_pdf(&blocks);
    let html = gaply_core::report_html::render_html(&blocks, "Thesis citation audit");
    std::fs::write(out.join("audit-preview.pdf"), &pdf)?;
    std::fs::write(out.join("audit-preview.html"), html.as_bytes())?;
    println!("wrote {}/audit-preview.{{pdf,html}} ({} KB pdf)", out.display(), pdf.len() / 1024);
    Ok(())
}
