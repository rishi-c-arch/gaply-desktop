//! §11 D213: the table chain, measured through the app's OWN entry points.
//!
//! The earlier D213 probe (`gaply-core/examples/table_sightings_before_after.rs`,
//! now removed) called `docparse::parse_path` + `extract_from_text` itself and
//! attached `parse_path_tables` itself — a copy of the pipeline's construction,
//! not the pipeline. This one calls:
//!
//! * `pipeline::run_pipeline_measured` — the real `run_pipeline_inner`, for
//!   `doc_tables`, the sightings, the stored `extractions` row, the report's
//!   findings, `lanes.extraction_examined` and the composed report text;
//! * `commands::detect_ai` — the AI Check screen's own command, for what AI
//!   detection excludes as a table.
//!
//! Nothing here re-implements a rule. Built at the pre-D213 commit too, with
//! only the field renamed back, so both sides come from the same code path.
//!
//! GAPLY_DISABLE_DEEP=1 cargo run --release --example table_chain_probe -- --db-dir DIR FILE...
//! cargo run --release --example table_chain_probe -- --read-stored ROW.json...
use std::sync::Arc;

use app_lib::pipeline::{run_pipeline_measured, AnalysisEvent, NetworkConsent};
use gaply_core::ai_detect::ProseExclusionReason;
use gaply_core::db::Database;
use gaply_core::embed::{Embedder, HashEmbedder};
use gaply_core::extract::ExtractionResult;
use gaply_core::report_compose::{compose, Block};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Mode 2 — the deserialize arrow. `Database::conn` is crate-private, so the
    // stored row is pulled out with the sqlite3 CLI and read back HERE, by the
    // product's own `ExtractionResult`.
    if args.first().map(String::as_str) == Some("--read-stored") {
        for f in &args[1..] {
            let raw = std::fs::read_to_string(f).expect("stored row file");
            let back: Result<ExtractionResult, _> = serde_json::from_str(raw.trim_end());
            println!(
                "{}",
                match back {
                    Ok(b) => serde_json::json!({"stored_row": f, "doc_tables": b.doc_tables.len(), "sightings": b.table_mentions.len()}),
                    Err(e) => serde_json::json!({"stored_row": f, "error": e.to_string()}),
                }
            );
        }
        return;
    }
    // Mode 1 — `--db-dir DIR FILE...`: one on-disk database per manuscript.
    assert_eq!(args.first().map(String::as_str), Some("--db-dir"), "usage: --db-dir DIR FILE... | --read-stored JSON...");
    let dir = std::path::PathBuf::from(&args[1]);
    for (i, p) in args[2..].iter().cloned().enumerate() {
        let name = std::path::Path::new(&p).file_name().unwrap_or_default().to_string_lossy().to_string();
        let db_path = dir.join(format!("paper{i}.sqlite"));
        let _ = std::fs::remove_file(&db_path);
        let db = Arc::new(Database::open(&db_path).expect("on-disk db"));
        db.migrate().expect("migrate");
        let embedder: Arc<dyn Embedder> = Arc::new(HashEmbedder);
        let out = match run_pipeline_measured(
            db.clone(),
            embedder,
            p.clone(),
            Some(name.clone()),
            None,
            None,
            None,
            NetworkConsent::Denied,
            &|_e: AnalysisEvent| {},
        ) {
            Ok(o) => o,
            Err(e) => {
                // Loud: a missing row must never read as a clean one.
                println!("{}", serde_json::json!({"paper": name, "error": e.to_string()}));
                continue;
            }
        };

        let model = out.report_model(None, None, None);
        let blocks = compose(&model);
        let text_of = |prefix: &str| {
            blocks.iter().find_map(|b| match b {
                Block::Paragraph { text } | Block::Bullet { text, .. } if text.starts_with(prefix) => Some(text.clone()),
                _ => None,
            })
        };
        let table_findings = out
            .report
            .findings
            .iter()
            .filter(|f| f.provenance.iter().any(|s| s == "signal:tables"))
            .count();

        let ai = app_lib::commands::detect_ai(p.clone()).expect("detect_ai");
        let (ai_paras, ai_chars) = ai
            .prose
            .excluded
            .iter()
            .filter(|e| e.reason == ProseExclusionReason::Table)
            .fold((0usize, 0usize), |(a, b), e| (a + e.paragraphs, b + e.chars));

        println!(
            "{}",
            serde_json::json!({
                "paper": name,
                "doc_tables": out.extraction.doc_tables.len(),
                "db": db_path.to_string_lossy(),
                "sightings": out.extraction.table_mentions.len(),
                "labels": out.extraction.table_mentions.iter().map(|t| t.label.clone()).collect::<Vec<_>>(),
                "references": out.extraction.references.len(),
                "table_findings": table_findings,
                "extraction_examined": out.lanes.extraction_examined,
                "ai_check_table_paras_excluded": ai_paras,
                "ai_check_table_chars_excluded": ai_chars,
                "report_table_count": model.manuscript.table_count,
                "report_summary": text_of("This manuscript has"),
                "report_unexamined_line": text_of("Table and reference checks"),
            })
        );
    }
}
