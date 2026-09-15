//! **What the SPSS parser recovers from a REAL analysis file.**
//!
//! Not a fixture. The input is SPSS's own journal from a real session on this
//! machine — the only real analysis artefact the corpus sweep found (see
//! `analysis::` module header: zero `.R`, `.py`, `.sps`, `.sav`, `.ipynb` in the
//! researcher corpus, against 29 `.docx`).
//!
//! Prints ROWS WITH THEIR COMMANDS, whole. A record saying "5 factor analyses"
//! has to be trusted; a record showing the twelve lines of `FACTOR` can be
//! refuted in a glance.
//!
//! ```text
//! cargo run --release --example analysis_record_probe -- <file.sps|statistics.jnl>...
//! ```

use gaply_core::analysis::{spss, ProcedureKind};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: analysis_record_probe <file>...");
        std::process::exit(2);
    }
    let files: Vec<(String, String)> = paths
        .iter()
        .map(|p| {
            let name = std::path::Path::new(p)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.clone());
            (name, std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {p}: {e}")))
        })
        .collect();

    let rec = spss::parse_all(&files);

    for s in &rec.sources {
        println!("source: {} ({:?}, {} lines)", s.file_name, s.language, s.lines);
    }
    println!(
        "commands={} statistical={} data-management={} unrecognised={} unparsed_lines={}",
        rec.procedures.len(),
        rec.statistical().count(),
        rec.procedures.iter().filter(|p| p.kind == ProcedureKind::DataManagement).count(),
        rec.procedures
            .iter()
            .filter(|p| matches!(p.kind, ProcedureKind::Unrecognised(_)))
            .count(),
        rec.unparsed_lines,
    );
    println!("kinds: {:?}", rec.kinds());

    // **Every unrecognised command, named.** A parser that discards what it does
    // not understand reports a clean record of a file it mostly failed to read.
    for p in rec.procedures.iter() {
        if let ProcedureKind::Unrecognised(k) = &p.kind {
            println!("  UNRECOGNISED line {}: {k} -> {}", p.line, first_line(&p.raw));
        }
    }

    println!("\n--- statistical procedures, with their commands ---");
    for p in rec.statistical() {
        println!(
            "\n[{}] {:?} line {} — {} variable(s): {}",
            p.id.0,
            p.kind,
            p.line,
            p.variables.len(),
            p.variables.join(" ")
        );
        for s in &p.subcommands {
            println!("    /{} {}", s.name, s.value);
        }
        println!("    raw:\n{}", indent(&p.raw));
    }

    // The known-good rule: this journal contains FACTOR commands. A run that
    // finds no statistical procedure is the parser failing, not the file.
    assert!(
        rec.statistical().count() > 0,
        "no statistical procedure recovered from {} file(s) — the instrument, not the corpus",
        files.len()
    );
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

fn indent(s: &str) -> String {
    s.lines().map(|l| format!("      {l}")).collect::<Vec<_>>().join("\n")
}
