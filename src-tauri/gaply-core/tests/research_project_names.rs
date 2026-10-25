//! **A project's name, against the test vectors the screen also reads
//! (Research OS, Phase 5B-3a; decision 121).**
//!
//! `tests/fixtures/research_project/name_vectors.json` lists typed names and
//! whether the rule accepts each, and what it stores. This test runs Rust's
//! rule over them ([`ProjectTitle::from_typed`]); the frontend's
//! `src/screens/statistics/projects/projectName.vitest.ts` runs the screen's
//! copy over the same file, so the two cannot drift apart unseen. The
//! vectors' `reason` is the screen's own word for a refusal; Rust has one
//! code for all of them, `project.bad_title`.

use gaply_core::research_project::{ProjectCode, ProjectTitle};
use serde_json::Value;

fn vectors() -> Vec<Value> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/research_project/name_vectors.json");
    let doc: Value = serde_json::from_slice(&std::fs::read(p).expect("the vectors")).expect("JSON");
    doc["vectors"].as_array().expect("a list").clone()
}

#[test]
fn every_name_vector_gets_the_answer_it_records() {
    let all = vectors();
    assert!(all.len() >= 20, "the vectors are there: {}", all.len());
    for v in &all {
        let input = v["input"].as_str().expect("an input");
        let why = v["why"].as_str().unwrap_or("");
        match (v["accepted"].as_bool(), ProjectTitle::from_typed(input)) {
            (Some(true), Ok(t)) => {
                assert_eq!(t.as_str(), v["stored"].as_str().expect("stored"), "{why}")
            }
            (Some(false), Err(code)) => assert_eq!(code, ProjectCode::BadTitle, "{why}"),
            (want, got) => panic!(
                "{why}: expected accepted={want:?}, got {:?}",
                got.map(|t| t.as_str().to_string())
            ),
        }
    }
}
