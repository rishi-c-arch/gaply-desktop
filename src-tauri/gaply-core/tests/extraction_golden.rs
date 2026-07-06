//! Golden-file test: extraction output for a known manuscript must match a
//! frozen expected result. Regenerate with `UPDATE_GOLDEN=1 cargo test -p
//! gaply_core --test extraction_golden` after an intentional change, then
//! review the diff before committing.

use gaply_core::extract::extract_from_text;

const MANUSCRIPT: &str = include_str!("fixtures/manuscript.txt");

#[test]
fn extraction_matches_golden_file() {
    let result = extract_from_text(MANUSCRIPT);
    let actual = serde_json::to_value(&result).unwrap();

    let golden_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/expected.json");

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        let pretty = serde_json::to_string_pretty(&actual).unwrap();
        std::fs::write(golden_path, pretty + "\n").unwrap();
        eprintln!("golden file updated: {golden_path}");
        return;
    }

    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/expected.json")).unwrap();
    assert_eq!(actual, expected, "extraction output drifted from golden file");
}
