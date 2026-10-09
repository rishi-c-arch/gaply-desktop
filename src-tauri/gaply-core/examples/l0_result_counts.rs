//! Level 0 result-reader counts over real manuscripts (Step 3).
//!
//! ```text
//! cargo run -p gaply_core --example l0_result_counts -- [--explain | --mask] [--verdicts] <file>...
//! ```
//!
//! For each file: results read, not read by reason, the distinct stated
//! thresholds, and the coupling check against `Stat::PValue`. A file that
//! cannot be parsed is reported as such and counts nothing; the exit status is
//! nonzero if any did. A `.docx` is read with its declared tables; anything
//! else as `TableStructure::NotAvailable`.
//!
//! Modes, by what they may print:
//! - default: the printed units of accepted results, and nothing else of the
//!   manuscript.
//! - `--explain`: also the TEXT of each NotLinked and Number rejection, and how
//!   many NotLinked spans hold a partial-eta name. **Open-access papers only.**
//! - `--mask`: no manuscript text at all, not even accepted units. NotLinked
//!   spans, and each PWithoutStatistic with the sentence before it, as MASKED
//!   shapes: statistic symbols, brackets, punctuation and the letter p kept,
//!   every digit 9, every other word `w`. For a researcher's own manuscript.
//!
//! `--verdicts` also runs every method in `MethodRegistry::LEVEL_0` on every
//! result read, and prints per file and in total the count of each verdict and
//! of each Concern and CouldNotCheck reason. With `--explain` it also lists every
//! Inconsistent and Concern row: file, section, paragraph, the printed unit,
//! the reported p, the compared (widened) range, alpha and conclusion_changes.
//! With `--mask` it prints counts only.

use std::collections::BTreeMap;

use gaply_core::extract::{self, sentence::sentence_containing, stats::Stat};
use gaply_core::stats_l0::input::{DocumentContext, L0Input};
use gaply_core::stats_l0::method::{Applicability, Computed, L0Verdict, MethodCheck};
use gaply_core::stats_l0::registry::MethodRegistry;
use gaply_core::stats_l0::result::{coupling_violations, read_results, PValue, Reason, Source, TableStructure};

/// Words a masked shape keeps: the statistic symbols and p.
const KEPT_WORDS: &[&str] = &["t", "F", "r", "z", "Z", "χ", "chi", "p"];

fn mask(text: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if !word.is_empty() {
            out.push_str(if KEPT_WORDS.contains(&word.as_str()) { word.as_str() } else { "w" });
            word.clear();
        }
    };
    for c in text.chars() {
        if c.is_alphabetic() {
            word.push(c);
            continue;
        }
        flush(&mut word, &mut out);
        if c.is_ascii_digit() {
            out.push('9');
        } else if c.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out.trim().to_string()
}

/// The verdict as a count key: the four verdicts, the Consistent note and
/// whether an Inconsistent changes the conclusion.
fn verdict_key(v: &L0Verdict) -> String {
    match v {
        L0Verdict::Consistent(None) => "Consistent".into(),
        L0Verdict::Consistent(Some(n)) => format!("Consistent+{n:?}"),
        L0Verdict::Inconsistent { conclusion_changes } => format!("Inconsistent(conclusion_changes={conclusion_changes:?})"),
        L0Verdict::Concern(_) => "Concern".into(),
        L0Verdict::CouldNotCheck(_) => "CouldNotCheck".into(),
    }
}

fn shown(c: &Computed) -> String {
    match c {
        Computed::Range { lo, hi } => format!("[{lo:.6}, {hi:.6}]"),
        Computed::Exact(q) => q.to_string(),
        Computed::ExactList(v) => v.iter().map(|q| q.to_string()).collect::<Vec<_>>().join("|"),
        Computed::Flag(b) => b.to_string(),
        Computed::Position(p) => format!("{p:?}"),
    }
}

fn get<'c>(c: &'c MethodCheck, name: &str) -> Option<&'c Computed> {
    c.computed.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
}

#[derive(Default)]
struct Tally {
    results: usize,
    verdicts: BTreeMap<String, usize>,
    reasons: BTreeMap<String, usize>,
}

impl Tally {
    fn print(&self, label: &str) {
        println!("   {label}verdicts results={} {:?}", self.results, self.verdicts);
        println!("   {label}reasons {:?}", self.reasons);
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let (mut explain, mut masked, mut verdicts) = (false, false, false);
    while let Some(flag) = args.first().filter(|a| a.starts_with("--")).cloned() {
        match flag.as_str() {
            "--explain" => explain = true,
            "--mask" => masked = true,
            "--verdicts" => verdicts = true,
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
        args.remove(0);
    }
    if args.is_empty() || (explain && masked) {
        eprintln!("usage: l0_result_counts [--explain | --mask] [--verdicts] <file>...");
        std::process::exit(2);
    }
    let mut total = Tally::default();
    let partial_eta = regex::Regex::new(r"(?i)partial\s*(?:η|eta)|η\s*[2²]?\s*p\b|ηp").unwrap();
    let mut failed = 0;
    for path in &args {
        let p = std::path::Path::new(path);
        let name = p.file_name().map_or(path.clone(), |n| n.to_string_lossy().into_owned());
        println!("== {name}");
        let ex = match extract::extract_path(p) {
            Ok(ex) => ex,
            Err(e) => {
                println!("   PARSE FAILED: {e}");
                failed += 1;
                continue;
            }
        };
        let is_docx = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("docx"));
        let tables = if is_docx { TableStructure::Declared } else { TableStructure::NotAvailable };
        let reading = read_results(&ex, tables);
        let paragraphs: usize = ex.sections.iter().map(|s| s.paragraphs.len()).sum();
        let stat_p = ex.statistics.iter().filter(|c| matches!(c.stat, Stat::PValue { .. })).count();
        println!(
            "   paragraphs={paragraphs} tables={tables:?} doc_tables={} stat_pvalues={stat_p}",
            ex.doc_tables.len()
        );

        let mut fam: BTreeMap<String, usize> = BTreeMap::new();
        let mut src: BTreeMap<String, usize> = BTreeMap::new();
        for r in &reading.results {
            *fam.entry(format!("{:?}", r.family)).or_default() += 1;
            *src.entry(format!("{:?}", r.source)).or_default() += 1;
        }
        println!("   results_read={} by_family={fam:?} by_source={src:?}", reading.results.len());

        let mut why: BTreeMap<String, usize> = BTreeMap::new();
        for n in &reading.not_read {
            *why.entry(format!("{:?}", n.reason)).or_default() += 1;
        }
        println!("   not_read={} by_reason={why:?}", reading.not_read.len());

        let text_of = |n: &gaply_core::stats_l0::result::NotRead| -> Option<(&str, std::ops::Range<usize>)> {
            let para = extract::paragraph_at(&ex, &n.location)?;
            let span = n.span.clone()?;
            Some((para, span))
        };

        if explain {
            let mut eta = 0;
            let mut linked = 0;
            for n in &reading.not_read {
                if !matches!(n.reason, Reason::Number(_) | Reason::NotLinked) {
                    continue;
                }
                let Some((para, span)) = text_of(n) else { continue };
                let t = &para[span];
                if n.reason == Reason::NotLinked {
                    linked += 1;
                    if partial_eta.is_match(t) {
                        eta += 1;
                    }
                }
                println!("     EXPLAIN {:?}: {t}", n.reason);
            }
            println!("   notlinked_with_partial_eta={eta} of notlinked={linked}");
        }

        if masked {
            let mut shapes: BTreeMap<(String, String), usize> = BTreeMap::new();
            for n in &reading.not_read {
                let Some((para, span)) = text_of(n) else { continue };
                let shape = match n.reason {
                    Reason::NotLinked => mask(&para[span]),
                    Reason::PWithoutStatistic => {
                        // The sentence up to and including the p, last 80 masked chars.
                        let sent = sentence_containing(para, span.start);
                        let s0 = sent.as_ptr() as usize - para.as_ptr() as usize;
                        let m = mask(&para[s0.min(span.start)..span.end]);
                        let cut = m.char_indices().rev().nth(79).map_or(0, |(i, _)| i);
                        format!("…{}", &m[cut..])
                    }
                    _ => continue,
                };
                *shapes.entry((format!("{:?}", n.reason), shape)).or_default() += 1;
            }
            let mut rows: Vec<_> = shapes.into_iter().collect();
            rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            for ((reason, shape), count) in rows {
                println!("     MASK {count:>3} {reason}: {shape}");
            }
        }

        let th: Vec<String> = reading.thresholds.iter().map(|t| format!("{:?} {}", t.relation(), t.raw())).collect();
        println!("   thresholds={th:?}");

        let v = coupling_violations(&reading, &ex);
        println!("   coupling_violations={}", v.len());

        if verdicts {
            let ctx = DocumentContext::read(&ex, &reading);
            let mut tally = Tally::default();
            for r in &reading.results {
                tally.results += 1;
                for m in MethodRegistry::LEVEL_0 {
                    let input = L0Input { result: r, context: &ctx };
                    if let Applicability::NotApplicable(why) = m.method.applicability(&input) {
                        *tally.verdicts.entry(format!("NotApplicable({why})")).or_default() += 1;
                        continue;
                    }
                    let c = m.method.run(&input);
                    *tally.verdicts.entry(verdict_key(&c.outcome)).or_default() += 1;
                    match c.outcome {
                        L0Verdict::Concern(why) => *tally.reasons.entry(format!("Concern({why:?})")).or_default() += 1,
                        L0Verdict::CouldNotCheck(why) => {
                            *tally.reasons.entry(format!("CouldNotCheck({why:?})")).or_default() += 1
                        }
                        _ => {}
                    }
                    let listed = matches!(c.outcome, L0Verdict::Inconsistent { .. } | L0Verdict::Concern(_));
                    if explain && listed {
                        let p = match &r.p {
                            PValue::Reported(p) => format!("{:?} {}", p.relation(), p.raw()),
                            PValue::NotSignificant => "ns".to_string(),
                        };
                        let alpha = get(&c, "alpha").or_else(|| get(&c, "alpha_values")).map_or("-".into(), shown);
                        let defaulted = get(&c, "alpha_defaulted").map_or("-".into(), shown);
                        let compared = get(&c, "compared_p").map_or("-".into(), shown);
                        println!(
                            "     VERDICT {name} section={:?}({:?}) paragraph={} {:?}: {}   reported p: {p}   compared: {compared}   alpha: {alpha} (defaulted={defaulted})",
                            r.location.section_index, r.location.section, r.location.paragraph, c.outcome, r.raw
                        );
                    }
                }
            }
            tally.print("");
            total.results += tally.results;
            for (k, n) in tally.verdicts {
                *total.verdicts.entry(k).or_default() += n;
            }
            for (k, n) in tally.reasons {
                *total.reasons.entry(k).or_default() += n;
            }
        }
        if masked {
            continue;
        }
        for c in &v {
            println!("     VIOLATION result#{} {:?} p={}", c.result_id, c.location, c.printed_p);
        }
        for r in &reading.results {
            let p = match &r.p {
                PValue::Reported(p) => format!("{:?} {}", p.relation(), p.raw()),
                PValue::NotSignificant => "ns".to_string(),
            };
            let tag = if r.source == Source::Unknown { "?" } else { "" };
            println!("     unit#{} [{:?}{tag}] {}   (p: {p})", r.id, r.source, r.raw);
        }
    }
    if verdicts {
        println!("== TOTAL over {} file(s)", args.len());
        total.print("total ");
    }
    if failed > 0 {
        std::process::exit(1);
    }
}
