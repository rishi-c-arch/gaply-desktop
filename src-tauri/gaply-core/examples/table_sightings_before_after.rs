//! §11 D213 before/after: table SIGHTINGS, the table COUNT a reader is shown,
//! the extraction lane's `examined` flag, and what AI detection drops as a
//! table — per manuscript, from the product's own functions.
//!
//! Built twice: at the pre-D213 commit (with the `AFTER-ONLY` lines deleted and
//! the field renamed back) and in the working tree. Every number comes from the
//! code under measurement: `detect_extraction` for the exclusions,
//! `ExtractionResult::table_count` and `ai_detect::is_caption_shaped` for the
//! new rules. The only rules written out here are the OLD ones, which are
//! one-liners: `report_build` showed `tables.len()`, and `pipeline.rs` set the
//! lane from `!tables.is_empty() || !references.is_empty()`.
//!
//! Output is JSON lines, one `paper` row and one `sighting` row per sighting,
//! so the two runs can be diffed by location in both directions. Captions are
//! printed WHOLE: a clipped span cannot be checked.
//!
//! cargo run -p gaply_core --example table_sightings_before_after -- FILE...
use gaply_core::ai_detect::{self, DeepKind, HeuristicModel, ProseExclusionReason};
use gaply_core::extract::{self, docparse};

fn main() {
    for p in std::env::args().skip(1) {
        let path = std::path::Path::new(&p);
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let text = match docparse::parse_path(path) {
            Ok(t) => t,
            Err(e) => {
                // Loud, never skipped: a missing row must not read as a clean one.
                println!("{}", serde_json::json!({"kind": "error", "paper": name, "error": e.to_string()}));
                continue;
            }
        };
        // Exactly the pipeline's construction (`pipeline.rs`, §11 D212).
        let mut ex = extract::extract_from_text(&text);
        ex.doc_tables = docparse::parse_path_tables(path).unwrap_or_default();
        let rep = ai_detect::detect_extraction(&HeuristicModel::gpt2_like(), DeepKind::Absent, &ex);
        let (ex_paras, ex_chars) = rep
            .prose
            .excluded
            .iter()
            .filter(|e| e.reason == ProseExclusionReason::Table)
            .fold((0usize, 0usize), |(p, c), e| (p + e.paragraphs, c + e.chars));

        let mut row = serde_json::json!({
            "kind": "paper",
            "paper": name,
            "sightings": ex.table_mentions.len(),
            "doc_tables": ex.doc_tables.len(),
            "references": ex.references.len(),
            "old_table_count": ex.table_mentions.len(),
            "old_extraction_examined": !ex.table_mentions.is_empty() || !ex.references.is_empty(),
            "ai_table_paras_excluded": ex_paras,
            "ai_table_chars_excluded": ex_chars,
        });
        row["new_table_count"] = serde_json::json!(ex.table_count()); // AFTER-ONLY
        row["new_extraction_examined"] = serde_json::json!(!ex.references.is_empty()); // AFTER-ONLY
        println!("{row}");

        for t in &ex.table_mentions {
            let mut s = serde_json::json!({
                "kind": "sighting",
                "paper": name,
                "section_index": t.location.section_index,
                "paragraph": t.location.paragraph,
                "label": t.label,
                "caption": t.caption,
            });
            s["caption_shaped"] = serde_json::json!(ai_detect::is_caption_shaped(t)); // AFTER-ONLY
            println!("{s}");
        }
    }
}
