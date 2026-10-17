//! Level 0 over a set of manuscripts, through the command path's own seam
//! (Phase 1, Step 7). A devtools instrument; never bundled.
//!
//! ```text
//! cargo run -p app --features devtools --example l0_corpus_run -- \
//!     --set NAME --out FILE.tsv [--xml-dir DIR] [--dump-text] [--not-read FILE.tsv] <file>...
//! ```
//!
//! Each file goes through `stats_consistency::run_stats_consistency_measured`,
//! never the internals, against a fresh in-memory database (preflight reads
//! only the stored import rate, so a real one would change nothing but the
//! estimate). A file preflight refuses is reported as not run.
//!
//! **Output.** One TSV row per claim, and a summary on stdout per manuscript
//! and for the set: claims read, not read by reason, verdict counts. Counts
//! only; stdout prints no manuscript text, so it is safe for a private set.
//! The command line is printed first.
//!
//! **`--xml-dir`**: for `<stem>.pdf`, the paper's `<stem>.xml` in DIR. Each
//! claim's printed unit is looked for in the text of the FIRST `<body>` (the
//! article's own; later ones are sub-articles), tags stripped, entities
//! decoded, and both sides normalised: every whitespace character removed,
//! every minus sign and dash made `-`, `²` made `2`. Column `in_xml` is
//! `yes`/`no`, `no-xml` when DIR has no file for the paper, and empty without
//! the flag. A hint for a human labeller, never a verdict.
//!
//! Rows are sorted: Inconsistent with `in_xml = no` first, then the other
//! Inconsistent, then Concern, then the rest; document order within each.
//!
//! **`--not-read FILE`** also writes one row per attempt the reader did not
//! read: its section, paragraph, reason and the sentence holding it (empty
//! when the reader kept no span). The sentence comes from a second
//! `extract_path` of the same file, the parse the seam itself runs.
//!
//! **`--dump-text`** writes the text the extractor read to
//! `$HOME/gaply-corpus/l0/text/<sha12>.txt`, outside the repository. That text
//! is `docparse::parse_path`'s output: the function `extract_path` itself
//! calls on the same file, so it is the same text, read a second time.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use app_lib::harness_log::manuscript_sha256;
use app_lib::stats_consistency::run_stats_consistency_measured;
use gaply_core::stats_l0::method::{Computed, L0Verdict, MethodCheck};
use gaply_core::stats_l0::outcome::{CheckOutcome, ClaimOutcome, StatsConsistencyOutcome, VerdictCounts};
use gaply_core::extract;
use gaply_core::stats_l0::number::Relation;
use gaply_core::stats_l0::result::{PValue, Reason};

const HEADER: &str = "set\tsha12\tfile\tclaim\tsection\tparagraph\traw_unit\tfamily\tverdict\treasons\treported_p\talpha\trecomputed_p\tconclusion_changes\tin_xml\tlabel";

fn usage() -> ! {
    eprintln!("usage: l0_corpus_run --set NAME --out FILE.tsv [--xml-dir DIR] [--dump-text] [--not-read FILE.tsv] <file>...");
    std::process::exit(2);
}

fn quoted(a: &str) -> String {
    if a.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,+".contains(c)) {
        a.to_string()
    } else {
        format!("'{}'", a.replace('\'', r"'\''"))
    }
}

fn tsv_cell(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '\t' | '\n' | '\r') { ' ' } else { c }).collect()
}

fn verdict_cell(v: &L0Verdict) -> String {
    match v {
        L0Verdict::Consistent(None) => "Consistent".into(),
        L0Verdict::Consistent(Some(n)) => format!("Consistent({n:?})"),
        L0Verdict::Inconsistent { .. } => "Inconsistent".into(),
        L0Verdict::Concern(r) => format!("Concern({r:?})"),
        L0Verdict::CouldNotCheck(_) => "CouldNotCheck".into(),
    }
}

fn check_cell(c: &CheckOutcome) -> String {
    match c {
        CheckOutcome::Ran(m) => match m.outcome {
            L0Verdict::CouldNotCheck(r) => format!("{}:CouldNotCheck({r:?})", m.method_id),
            v => format!("{}:{}", m.method_id, verdict_cell(&v)),
        },
        CheckOutcome::NotApplicable { method_id, reason } => format!("{method_id}:NotApplicable({reason:?})"),
    }
}

fn section_cell(l: &extract::Location) -> String {
    format!("{:?}#{}", l.section, l.section_index.map_or("?".into(), |i| i.to_string()))
}

/// The p as printed, with its relation: `= .045`, `< .001`, or `ns`.
fn reported_p(p: &PValue) -> String {
    match p {
        PValue::NotSignificant => "ns".into(),
        PValue::Reported(n) => {
            let op = match n.relation() {
                Relation::Eq => "=",
                Relation::Lt => "<",
                Relation::Le => "<=",
                Relation::Gt => ">",
                Relation::Ge => ">=",
            };
            format!("{op} {}", n.raw())
        }
    }
}

fn p_check(c: &ClaimOutcome) -> Option<&MethodCheck> {
    c.checks.iter().find_map(|k| match k {
        CheckOutcome::Ran(m) if m.method_id == "p_consistency" => Some(m),
        _ => None,
    })
}

fn p_range(c: &ClaimOutcome) -> String {
    let f = |x: f64| if x != 0.0 && x.abs() < 1e-4 { format!("{x:.3e}") } else { format!("{x:.6}") };
    p_check(c)
        .and_then(|m| m.computed.iter().find(|(n, _)| *n == "recomputed_p"))
        .map_or(String::new(), |(_, v)| match v {
            Computed::Range { lo, hi } => format!("[{}, {}]", f(*lo), f(*hi)),
            _ => String::new(),
        })
}

/// Whitespace removed, dashes and minus signs made `-`, `²` made `2`.
fn normalise(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '\u{2212}' | '\u{2010}'..='\u{2015}' | '\u{FE63}' | '\u{FF0D}' => '-',
            '²' => '2',
            c => c,
        })
        .collect()
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(j) = rest.find(';').filter(|&j| j <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..j];
        let ch = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[j + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The normalised text of the first `<body>…</body>`, or `None`.
fn xml_body(xml: &str) -> Option<String> {
    let start = xml.find("<body")?;
    let end = start + xml[start..].find("</body>")?;
    let mut text = String::new();
    let mut in_tag = false;
    for c in xml[start..end].chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    Some(normalise(&decode_entities(&text)))
}

#[derive(Default)]
struct SetTotals {
    manuscripts: usize,
    failed: usize,
    claims_read: usize,
    not_read: Vec<(Reason, usize)>,
    counts: VerdictCounts,
}

fn add_reasons(into: &mut Vec<(Reason, usize)>, from: &[(Reason, usize)]) {
    for (r, n) in from {
        match into.iter_mut().find(|(k, _)| k == r) {
            Some((_, m)) => *m += n,
            None => into.push((*r, *n)),
        }
    }
}

fn reasons_line(v: &[(Reason, usize)]) -> String {
    let total: usize = v.iter().map(|(_, n)| n).sum();
    let parts: Vec<String> = v.iter().map(|(r, n)| format!("{r:?}={n}")).collect();
    format!("{total} [{}]", parts.join(", "))
}

fn counts_line(c: &VerdictCounts) -> String {
    format!(
        "Consistent={} Inconsistent={} Concern={} CouldNotCheck={}",
        c.consistent, c.inconsistent, c.concern, c.could_not_check
    )
}

fn alpha_cell(o: &StatsConsistencyOutcome) -> String {
    let v: Vec<String> = o.alpha_used.iter().map(|a| a.to_string()).collect();
    format!("{}{}", v.join("|"), if o.alpha_defaulted { " (default)" } else { "" })
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    println!("command: {}", argv.iter().map(|a| quoted(a)).collect::<Vec<_>>().join(" "));

    let (mut set, mut out, mut xml_dir) = (None::<String>, None::<PathBuf>, None::<PathBuf>);
    let mut not_read_out = None::<PathBuf>;
    let mut dump = false;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut it = argv.iter().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--set" => set = Some(it.next().cloned().unwrap_or_else(|| usage())),
            "--out" => out = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--xml-dir" => xml_dir = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            "--dump-text" => dump = true,
            "--not-read" => not_read_out = Some(PathBuf::from(it.next().unwrap_or_else(|| usage()))),
            s if s.starts_with("--") => usage(),
            s => files.push(PathBuf::from(s)),
        }
    }
    let (Some(set), Some(out)) = (set, out) else { usage() };
    if files.is_empty() {
        usage();
    }
    let text_dir = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("gaply-corpus/l0/text"));
    if dump {
        let dir = text_dir.as_ref().expect("HOME is set");
        std::fs::create_dir_all(dir).expect("create text dir");
    }

    let mut totals = SetTotals::default();
    let mut not_read_rows: Vec<String> = Vec::new();
    // (sort group, document order, row)
    let mut rows: Vec<(u8, usize, String)> = Vec::new();
    for path in &files {
        totals.manuscripts += 1;
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let sha = manuscript_sha256(path);
        let sha12 = sha.get(..12).unwrap_or("unreadable").to_string();
        println!("== {name}  sha12={sha12}");

        let db = gaply_core::Database::in_memory().expect("in-memory db");
        let outcome = match run_stats_consistency_measured(&db, path) {
            Ok(o) => o,
            Err(e) => {
                // The error is the preflight's or the parser's message about
                // the FILE, never its contents.
                println!("   NOT RUN ({}): {e}", e.code());
                totals.failed += 1;
                continue;
            }
        };

        if dump {
            match gaply_core::extract::docparse::parse_path(path) {
                Ok(text) => {
                    let p = text_dir.as_ref().unwrap().join(format!("{sha12}.txt"));
                    std::fs::write(&p, text).expect("write dumped text");
                }
                Err(e) => println!("   TEXT DUMP FAILED: {}", e.code()),
            }
        }

        let body = xml_dir.as_ref().map(|d| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            std::fs::read_to_string(d.join(format!("{stem}.xml"))).ok().and_then(|x| xml_body(&x))
        });

        let alpha = alpha_cell(&outcome);
        for c in &outcome.claims {
            let r = &c.result;
            let in_xml = match &body {
                None => "",
                Some(None) => "no-xml",
                Some(Some(b)) if b.contains(&normalise(&r.raw)) => "yes",
                Some(Some(_)) => "no",
            };
            let conclusion = match c.verdict {
                L0Verdict::Inconsistent { conclusion_changes: Some(b) } => b.to_string(),
                L0Verdict::Inconsistent { conclusion_changes: None } => "unassessed".into(),
                _ => String::new(),
            };
            let group = match (&c.verdict, in_xml) {
                (L0Verdict::Inconsistent { .. }, "no") => 0,
                (L0Verdict::Inconsistent { .. }, _) => 1,
                (L0Verdict::Concern(_), _) => 2,
                _ => 3,
            };
            let reasons: Vec<String> = c.checks.iter().map(check_cell).collect();
            let section = section_cell(&r.location);
            let mut row = String::new();
            write!(
                row,
                "{set}\t{sha12}\t{}\t{}\t{section}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{alpha}\t{}\t{conclusion}\t{in_xml}\t",
                tsv_cell(&name),
                r.id,
                r.location.paragraph,
                tsv_cell(&r.raw),
                r.family,
                verdict_cell(&c.verdict),
                reasons.join("; "),
                reported_p(&r.p),
                p_range(c),
            )
            .unwrap();
            rows.push((group, rows.len(), row));
        }

        if not_read_out.is_some() {
            match extract::extract_path(path) {
                Ok(ex) => {
                    for n in &outcome.not_read {
                        let sentence = extract::paragraph_at(&ex, &n.location)
                            .zip(n.span.clone())
                            .map_or(String::new(), |(para, span)| {
                                extract::sentence::sentence_containing(para, span.start).to_string()
                            });
                        not_read_rows.push(format!(
                            "{set}\t{sha12}\t{}\t{}\t{}\t{alpha}\t{:?}\t{}",
                            tsv_cell(&name),
                            section_cell(&n.location),
                            n.location.paragraph,
                            n.reason,
                            tsv_cell(&sentence),
                        ));
                    }
                }
                Err(e) => println!("   NOT-READ TEXT FAILED ({})", e.code()),
            }
        }

        println!("   claims_read={}", outcome.claims_read);
        println!("   not_read={}", reasons_line(&outcome.not_read_by_reason));
        println!("   verdicts: {}", counts_line(&outcome.verdict_counts));
        println!("   alpha={alpha}");
        totals.claims_read += outcome.claims_read;
        add_reasons(&mut totals.not_read, &outcome.not_read_by_reason);
        let v = &outcome.verdict_counts;
        totals.counts.consistent += v.consistent;
        totals.counts.inconsistent += v.inconsistent;
        totals.counts.concern += v.concern;
        totals.counts.could_not_check += v.could_not_check;
    }

    rows.sort_by_key(|(g, i, _)| (*g, *i));
    let mut tsv = String::from(HEADER);
    tsv.push('\n');
    for (_, _, row) in &rows {
        tsv.push_str(row);
        tsv.push('\n');
    }
    std::fs::write(&out, tsv).expect("write tsv");
    if let Some(p) = &not_read_out {
        let mut t = String::from("set\tsha12\tfile\tsection\tparagraph\talpha\treason\tsentence\n");
        for row in &not_read_rows {
            t.push_str(row);
            t.push('\n');
        }
        std::fs::write(p, t).expect("write not-read tsv");
        println!("   not-read rows written={} to {}", not_read_rows.len(), p.display());
    }

    println!("== SET {set}: {} manuscript(s), {} not run", totals.manuscripts, totals.failed);
    println!("   claims_read={}", totals.claims_read);
    println!("   not_read={}", reasons_line(&totals.not_read));
    println!("   verdicts: {}", counts_line(&totals.counts));
    println!("   rows written={} to {}", rows.len(), Path::new(&out).display());
    if totals.failed > 0 {
        std::process::exit(1);
    }
}
