//! THROWAWAY — measure WHY registry years disagree. Not a test.
use gaply_core::extract::{docparse, extract_from_text};
use app_lib::http_fetcher::RefVerifier;
use gaply_core::{now_epoch, Database, GaplyError};
use std::path::Path;

fn main() -> Result<(), GaplyError> {
    let path = std::env::args().nth(1).expect("usage: registry_probe <manuscript>");
    let text = docparse::parse_path(Path::new(&path))?;
    let ex = extract_from_text(&text);
    let db = Database::in_memory()?;
    let v = RefVerifier::new()?;
    let now = now_epoch();
    println!("references parsed: {}\n", ex.references.len());
    for r in &ex.references {
        let rv = match v.verify(&db, r, now) { Ok(x) => x, Err(e) => { println!("ERR {e}"); continue } };
        let ec = match rv.exists.as_ref() { Some(e) => e, None => continue };
        let my = match ec.matched_year { Some(y) => y, None => continue };
        let local = match r.year { Some(y) => y, None => continue };
        if local == my { continue; }
        println!("=== MISMATCH: cited {local} vs registry {my} ===");
        println!("  local authors : {}", r.authors);
        println!("  local title   : {:?}", r.title);
        println!("  local raw     : {}", r.raw.chars().take(150).collect::<String>());
        println!("  registry src  : {}", ec.source);
        println!("  registry title: {:?}", ec.title.as_ref().map(|t| t.display_raw().chars().take(110).collect::<String>()));
        println!("  registry auth : {:?}", ec.matched_authors.as_ref().map(|a| a.display_raw().chars().take(90).collect::<String>()));
        println!("  registry doi  : {:?}", ec.doi);
        println!("  local doi     : {:?}", r.doi);
        println!("  provenance    : {} {}", ec.provenance.source, ec.provenance.url);
        println!();
    }
    Ok(())
}
