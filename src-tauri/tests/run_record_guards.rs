//! **A run's record holds its recipe, and stays unprintable (Research OS, Phase
//! 8T-1a, decision 491).** Since 8T-1a `RunInput` holds the recipe's canonical
//! bytes, and a recipe holds the score's typed name. `RunInput` and `RunCopy`
//! never derive or implement `Debug` or `Serialize`, so neither can reach a log
//! line, a response or a file but through a Save's store. A source scan of
//! `src/runs.rs`, on what is TYPED there; it fails if it cannot find either type.

use std::path::Path;

fn runs_src() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/runs.rs"))
        .expect("src/runs.rs reads")
}

/// The attribute lines directly above `pub struct <name>`, doc comments skipped.
fn attributes_of(src: &str, name: &str) -> Vec<String> {
    let lines: Vec<&str> = src.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.trim_start().starts_with(&format!("pub struct {name} ")))
        .unwrap_or_else(|| panic!("`pub struct {name}` not found: the guard is inert"));
    lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim())
        .take_while(|l| l.starts_with("#[") || l.starts_with("///") || l.starts_with("//"))
        .filter(|l| l.starts_with("#["))
        .map(str::to_string)
        .collect()
}

#[test]
fn run_input_and_run_copy_never_derive_or_implement_debug_or_serialize() {
    let src = runs_src();
    for name in ["RunInput", "RunCopy"] {
        let attrs = attributes_of(&src, name);
        for a in &attrs {
            assert!(
                !a.contains("Debug") && !a.contains("Serialize"),
                "src/runs.rs: {name} has `{a}`"
            );
        }
        let squashed: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
        for imp in ["Debug for", "Serialize for", "Display for"] {
            assert!(
                !squashed.contains(&format!("{imp} {name} ")),
                "src/runs.rs implements `{imp} {name}`"
            );
        }
    }
    // RunInput still has its derive line, so the scan reads the right place.
    assert_eq!(attributes_of(&src, "RunInput"), ["#[derive(Clone)]"]);
}

#[test]
fn run_input_holds_the_recipe_bytes_taken_through_the_facade() {
    let src = runs_src();
    let squashed: String = src.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        squashed.contains("pub recipe: Option<Vec<u8>>"),
        "RunInput holds the recipe's canonical bytes"
    );
    assert!(
        squashed.contains("stats_adapters::prep_recipe_bytes("),
        "taken in queue through the façade"
    );
}
