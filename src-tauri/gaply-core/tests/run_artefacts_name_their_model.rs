//! **A committed model-run artefact must say which model produced it, and its
//! filename must not claim a different one.**
//!
//! # The defect this exists for
//!
//! §11 D196 and §11 D197 labelled every small-model row `SLM1`. The run had
//! actually loaded the bundled **stock** `Qwen2.5-0.5B-Instruct-Q4_K_M`, and the
//! probe said so on stderr at the time — `model: qwen2.5-0.5b-instruct-q4km`.
//! Nothing connected that line to the entry, and the raw results were committed
//! in a file called `grrb-slm1-qwen05b-raw.tsv`: **a filename asserting a model
//! that produced none of the rows inside it.** SLM-1 is the 7B (`models/mod.rs`
//! :102) and has never been measured (§11 D199).
//!
//! A number is only as good as the answer to "which model produced this", and
//! that question had no artefact to answer it. Now it does.
//!
//! # Why the filename is checked against the header and not just for presence
//!
//! A header alone would have sat happily inside a file still called `slm1`. The
//! failure mode is a NAME that disagrees with the CONTENT, so the assertion has
//! to compare the two — which is the stale-pointer shape `cited_tests_exist.rs`
//! and `decision_records.rs` already guard in prose.
//!
//! Runs in `gaply_core` because both CI workflows run `-p gaply_core`; the
//! probes that produce these files are app-crate examples that CI never runs.

use std::path::Path;

const DIR: &str = "../evals/reports";

/// Words in a filename that name a model family rather than describing the run.
/// A filename token from this list must appear in the file's own `# model:` or
/// `# weights:` header.
const MODEL_TOKENS: [&str; 8] =
    ["slm1", "slm2", "qwen", "gpt", "claude", "llama", "mistral", "gemma"];

#[test]
fn every_raw_run_artefact_names_the_model_that_produced_it() {
    let dir = Path::new(DIR);
    let entries = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{DIR}: {e} — has the reports directory moved?"));

    let mut checked = 0usize;
    for e in entries.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        if !name.ends_with("-raw.tsv") {
            continue;
        }
        checked += 1;
        let src = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
        let header: String = src
            .lines()
            .take_while(|l| l.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n")
            .to_lowercase();

        assert!(
            header.contains("# model:"),
            "{name}: no `# model:` header. A run artefact that does not say which model \
             produced it cannot be checked against the entry that cites it — which is how \
             §11 D196 came to label a stock 0.5B as SLM-1."
        );
        assert!(
            header.contains("# run:"),
            "{name}: no `# run:` header naming the probe and date"
        );

        // The filename must not claim a model the header does not carry.
        for tok in MODEL_TOKENS {
            if name.contains(tok) {
                assert!(
                    header.contains(tok),
                    "{name}: the FILENAME says {tok:?} and the header does not.\n  header: {}\n  \
                     A file named for a model that produced none of its rows is the §11 D199 \
                     defect: rename the file or fix the header, and do not leave them \
                     disagreeing.",
                    header.lines().take(2).collect::<Vec<_>>().join(" | ")
                );
            }
        }
    }

    assert!(
        checked > 0,
        "no `*-raw.tsv` artefacts found under {DIR} — this guard would pass vacuously. \
         If run artefacts moved, move this scan with them."
    );
}
