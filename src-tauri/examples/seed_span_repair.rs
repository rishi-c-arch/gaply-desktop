//! **Restore the 19 journal-seed spans cut by the old 400-character cap.**
//!
//! docs/PROBLEM_DOSSIER.md A5. `journal_extract.rs` stored
//! `span.chars().take(400)`, and `journal_guidelines` retains no fetched page,
//! so the cut is unrecoverable locally — the pages must be fetched again.
//!
//! # It can only RESTORE, never substitute
//!
//! For each truncated record it fetches ONLY that record's own `source_url`,
//! re-extracts, and accepts a replacement only when the stored 400-character
//! span is a **prefix** of the candidate. That is the identity test: a prefix
//! match means the same sentence, continued. A sentence from another page, or a
//! different sentence on the same page, cannot satisfy it.
//!
//! Anything not matched is LEFT UNCHANGED and reported as unrecovered with a
//! reason. `--apply` writes; without it the run is a dry report.
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let path = std::env::args().nth(1).expect("usage: seed_span_repair <seed.json> [--apply]");
    let apply = std::env::args().any(|a| a == "--apply");
    let raw = std::fs::read_to_string(&path).expect("read seed");
    let mut seed: serde_json::Value = serde_json::from_str(&raw).expect("parse seed");

    // Every record carrying a 400-char span, with the collection it lives in.
    let mut targets: Vec<(String, usize, String, String)> = Vec::new(); // (coll, idx, url, span)
    for coll in ["requirements", "bindings"] {
        let Some(arr) = seed.get(coll).and_then(|v| v.as_array()) else { continue };
        for (i, rec) in arr.iter().enumerate() {
            let span = rec.get("source_span").and_then(|v| v.as_str()).unwrap_or("");
            if span.chars().count() == 400 {
                let url = rec.get("source_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
                targets.push((coll.to_string(), i, url, span.to_string()));
            }
        }
    }
    eprintln!("truncated records: {}", targets.len());

    // Fetch each DISTINCT url once, through the app's own fetcher (never curl:
    // a curl result is a fact about curl, not about what Gaply can reach).
    let fetcher = app_lib::http_fetcher::ReqwestFetcher::new().expect("fetcher");
    let mut pages: std::collections::BTreeMap<String, Result<String, String>> = Default::default();
    for (_, _, url, _) in &targets {
        if pages.contains_key(url) {
            continue;
        }
        let got = fetcher
            .get(&HttpRequest::get(url.clone()))
            .map_err(|e| e.to_string())
            .and_then(|r| {
                if (200..300).contains(&r.status) {
                    Ok(r.body)
                } else {
                    Err(format!("HTTP {}", r.status))
                }
            });
        eprintln!(
            "  {} {}",
            match &got { Ok(b) => format!("{:>7} bytes", b.len()), Err(e) => format!("{e:>13}") },
            url
        );
        pages.insert(url.clone(), got);
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }

    let now = gaply_core::now_epoch();
    let (mut restored, mut unrecovered) = (Vec::new(), Vec::new());
    for (coll, idx, url, old) in &targets {
        let reason = match pages.get(url) {
            Some(Err(e)) => Some(format!("page unreachable: {e}")),
            Some(Ok(html)) => {
                let blocks = app_lib::guidelines::html_to_blocks(html);
                let reqs = gaply_core::journal_extract::extract_requirements(&blocks);
                // THE IDENTITY TEST: the stored 400 chars must be a PREFIX.
                match reqs.iter().find(|r| r.source_span.starts_with(old.as_str())) {
                    Some(hit) if hit.source_span.chars().count() > 400 => {
                        let rec = &mut seed[coll][*idx];
                        rec["source_span"] = serde_json::json!(hit.source_span);
                        // D186 PROVENANCE: this row's text now comes from a
                        // fetch performed NOW, so its fetch time must say so.
                        // `status` is untouched — the row's verification state
                        // did not change, only the completeness of its quote.
                        rec["fetched_at"] = serde_json::json!(now);
                        restored.push((
                            coll.clone(),
                            *idx,
                            old.clone(),
                            hit.source_span.clone(),
                        ));
                        None
                    }
                    Some(_) => Some("the page still yields only the truncated form".into()),
                    None => Some("the stored sentence no longer appears on its page".into()),
                }
            }
            None => Some("not fetched".into()),
        };
        if let Some(r) = reason {
            unrecovered.push((coll.clone(), *idx, r));
        }
    }

    println!("\n=== RESTORED: {} ===", restored.len());
    for (c, i, old, new) in &restored {
        println!("  {c}[{i}]  400 -> {} chars", new.chars().count());
        println!("     was: …{}", &old[old.len().saturating_sub(48)..]);
        println!("     now: …{}", &new[new.len().saturating_sub(48)..]);
    }
    println!("\n=== UNRECOVERED: {} ===", unrecovered.len());
    for (c, i, why) in &unrecovered {
        println!("  {c}[{i}]  {why}");
    }
    // **A PATCH, never a re-serialisation.** `serde_json::Map` is a `BTreeMap`
    // (the `preserve_order` feature is enabled nowhere in this tree), so
    // round-tripping the seed through serde would SORT every key in all 521
    // records and rewrite the whole file — measured: 5101 of 5101 lines changed,
    // for 19 intended edits. The caller applies these replacements in place, so
    // every untouched row stays byte-identical because it is never rewritten.
    if apply {
        let patch: Vec<serde_json::Value> = restored
            .iter()
            .map(|(c, i, _, new)| {
                serde_json::json!({
                    "collection": c, "index": i,
                    "source_span": new, "fetched_at": now
                })
            })
            .collect();
        let out = format!("{}/seed_patch.json", std::env::temp_dir().display());
        std::fs::write(&out, serde_json::to_string_pretty(&patch).expect("patch"))
            .expect("write patch");
        eprintln!("\npatch written: {out} ({} replacements)", patch.len());
    } else {
        eprintln!("\nDRY RUN — pass --apply to emit a patch");
    }
}
