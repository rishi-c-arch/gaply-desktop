//! **What does the ingest store TODAY?**
//!
//! The 368-character Nature Medicine document came from a DB snapshot dated
//! 15 Sep. The ingest has changed since, so that is evidence about then. This
//! runs the CURRENT `GuidelinesIngestor` into a fresh database and reports what
//! actually lands, per URL — fetch, blocks and store in one place, so the layer
//! that loses text is named by measurement rather than inferred from a stale row.
use app_lib::guidelines::GuidelinesIngestor;
use gaply_core::embed::HashEmbedder;
use gaply_core::Database;

fn main() {
    let tmp = std::env::temp_dir().join(format!("ingest_now_{}.db", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let db = Database::open(&tmp).expect("db");
    let embedder = HashEmbedder;
    let ingestor = GuidelinesIngestor::new().expect("ingestor");

    for url in std::env::args().skip(1) {
        let report = ingestor.ingest(&db, &embedder, None, Some(&url));
        println!("\n{url}");
        println!("  any_ingested {}   note: {}", report.any_ingested, report.note);
    }
    println!("\n(db: {})", tmp.display());
}
