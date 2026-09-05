//! Label `citation_need` cases from a real manuscript, one sentence at a time.
//!
//! ```text
//! cargo run --release --bin label-cn -- ~/Desktop/"R PAPER .docx"
//! ```
//!
//! # Why this exists
//!
//! §11 D59 records that `citation_need` prompt work is blocked on eval
//! throughput; Metal fixed the throughput, and what remains missing is a
//! LABELLED SET. Every accuracy number so far has leaned on a fixed keyword
//! heuristic for "the authors' own work", applied identically to both arms —
//! honest about direction and size, and explicitly NOT ground truth. The
//! reason/verdict coupling defect (D59 item 1) cannot be settled without one.
//!
//! # What it records, and what it deliberately does not
//!
//! `preceding_sentence` and `following_sentence` are written EMPTY by default,
//! because `job_runner` passes them empty — the product never shows the model a
//! neighbour. `ai-eval` deserialises whatever the case file says, so recording
//! real neighbours would measure a configuration that does not ship. That is
//! the §11 D68 mistake (verifying a path the product does not run), and the
//! default avoids it. `--with-neighbours` opts in for anyone who wants to
//! measure the richer configuration deliberately.
//!
//! `severity` is omitted when `needs_citation` is false: §11 D66 established
//! that a sentence needing no citation has no severity-of-need, and the schema
//! treats the field as conditional.
use std::io::{Read, Write};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

const TARGET_CASES: usize = 50;

/// How a label came to exist. Written on every case this tool produces, because
/// the three kinds are NOT interchangeable when scoring (§11 D75).
///
/// - `cold` — labelled without seeing the model's answer. THE ONLY KIND THAT
///   GIVES AN UNBIASED ACCURACY NUMBER.
/// - `suggested_overridden` — the model proposed, the human disagreed. Scoring
///   on these alone is a LOWER BOUND, not accuracy: they are selected precisely
///   for disagreement.
/// - `suggested_accepted` — the model proposed, the human agreed. **Cannot
///   score the model at all.** The label carries the model's own answer, so
///   comparing the two is circular.
///
/// Anchoring is the reason this is recorded rather than trusted to memory: a
/// confident proposal shifts the reader's judgement, not just the keystroke, so
/// "I would have said the same anyway" is not evidence.
const PROVENANCE_COLD: &str = "cold";
const PROVENANCE_ACCEPTED: &str = "suggested_accepted";
const PROVENANCE_OVERRIDDEN: &str = "suggested_overridden";

/// The ten spec sentence types, with the key that selects each.
const TYPES: &[(char, &str, &str)] = &[
    ('e', "empirical_claim", "empirical claim"),
    ('s', "statistic", "statistic"),
    ('d', "definition", "definition"),
    ('p', "prior_work", "prior work"),
    ('m', "method_borrowed", "method borrowed"),
    ('c', "common_knowledge", "common knowledge"),
    ('o', "author_own_result", "author's own result"),
    ('t', "transition", "transition"),
    ('i', "interpretation", "interpretation"),
    ('h', "hedged_speculation", "hedged speculation"),
];

/// Put the terminal in raw mode so a single key is a decision, and ALWAYS put
/// it back — a labelling session that leaves the shell unusable is worse than
/// one that needs Enter.
#[cfg(unix)]
struct RawMode(libc::termios);

#[cfg(unix)]
impl RawMode {
    fn enter() -> Option<Self> {
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut t) != 0 {
                return None;
            }
            let saved = t;
            t.c_lflag &= !(libc::ICANON | libc::ECHO);
            t.c_cc[libc::VMIN] = 1;
            t.c_cc[libc::VTIME] = 0;
            (libc::tcsetattr(0, libc::TCSANOW, &t) == 0).then_some(RawMode(saved))
        }
    }
}

#[cfg(unix)]
impl Drop for RawMode {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.0);
        }
    }
}

/// One keypress, lowercased. Falls back to a line read where raw mode is not
/// available, so the tool still works rather than not existing.
fn key(raw: bool) -> char {
    if raw {
        let mut b = [0u8; 1];
        if std::io::stdin().read_exact(&mut b).is_ok() {
            return (b[0] as char).to_ascii_lowercase();
        }
        return 'q';
    }
    let mut s = String::new();
    if std::io::stdin().read_line(&mut s).is_err() {
        return 'q';
    }
    s.trim().chars().next().unwrap_or('\n').to_ascii_lowercase()
}

fn wrap(s: &str, width: usize, indent: &str) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    for w in s.split_whitespace() {
        if col + w.len() + 1 > width {
            out.push('\n');
            out.push_str(indent);
            col = 0;
        } else if col > 0 {
            out.push(' ');
            col += 1;
        }
        out.push_str(w);
        col += w.chars().count();
    }
    out
}

/// Ask the model what it thinks. Returns its answer VERBATIM — the reason is
/// the model's own text, never paraphrased or tidied, so what the labeller reads
/// is what the engine actually said.
async fn suggest(
    manager: &app_lib::ai::model_manager::ModelManager,
    sentence: &str,
    section: &str,
) -> Option<app_lib::ai::tasks::citation_need::CitationNeedOutput> {
    use app_lib::ai::tasks::citation_need::{CitationNeedInput, CitationNeedTask};
    let task = CitationNeedTask::new(CitationNeedInput {
        sentence: sentence.to_string(),
        // EMPTY, matching job_runner (§11 D73). A suggestion produced with
        // richer input than the product supplies would be a different engine's
        // opinion.
        preceding_sentence: String::new(),
        following_sentence: String::new(),
        section: section.to_string(),
    });
    let cancel = Arc::new(AtomicBool::new(false));
    app_lib::ai::task::run_task(manager, &task, &Default::default(), cancel, None)
        .await
        .ok()
        .map(|r| r.output)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let manuscript = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .ok_or("usage: label-cn <manuscript> [--with-neighbours] [--all]")?;
    let with_neighbours = args.iter().any(|a| a == "--with-neighbours");
    // By default only UNCITED sentences: those are the ones the audit routes to
    // citation_need. A sentence that already carries a marker is a
    // citation_support question, not this one.
    let include_cited = args.iter().any(|a| a == "--all");
    let suggest_mode = args.iter().any(|a| a == "--suggest");

    let out_path = std::path::Path::new("evals/citation_need.jsonl");
    if !out_path.exists() {
        return Err(format!(
            "{} not found — run this from src-tauri/ so the case file is where ai-eval expects it",
            out_path.display()
        )
        .into());
    }

    let blocks = gaply_core::extract::docparse::parse_path_paged(std::path::Path::new(manuscript))?;
    let pre = gaply_core::ai_engine::audit_prepass::prepass_blocks(&blocks);

    // RESUME: everything already labelled, by sentence text. Matching on the
    // sentence rather than an index means re-running after the pre-pass changes
    // does not re-ask what was already answered.
    let existing_raw = std::fs::read_to_string(out_path)?;
    let mut done: std::collections::HashSet<String> = Default::default();
    let mut total_cases = 0usize;
    let mut max_label = 0usize;
    for line in existing_raw.lines().filter(|l| !l.trim().is_empty()) {
        total_cases += 1;
        let v: serde_json::Value = serde_json::from_str(line)?;
        if let Some(s) = v["input"]["sentence"].as_str() {
            done.insert(s.trim().to_string());
        }
        if let Some(id) = v["id"].as_str() {
            if let Some(n) = id.strip_prefix("cn-label-").and_then(|n| n.parse::<usize>().ok()) {
                max_label = max_label.max(n);
            }
        }
    }

    let candidates: Vec<_> = pre
        .planned
        .iter()
        .filter(|p| include_cited || p.markers.is_empty())
        .filter(|p| !done.contains(p.sentence.trim()))
        .collect();

    println!("\x1b[1m{}\x1b[0m", manuscript);
    println!(
        "{} planned sentences, {} unlabelled {}",
        pre.planned.len(),
        candidates.len(),
        if include_cited { "(all)" } else { "(uncited only; --all for every one)" }
    );
    println!(
        "case file: {} cases  ->  target {TARGET_CASES}\n",
        total_cases
    );
    if candidates.is_empty() {
        println!("nothing left to label.");
        return Ok(());
    }

    // The model, only when asked for. A labelling session must still work with
    // no model installed — suggestions are an accelerator, never a requirement.
    let manager = if suggest_mode {
        let home = std::path::PathBuf::from(std::env::var("HOME")?);
        let app_data = home.join("Library/Application Support/ai.gaply.app");
        let db = gaply_core::Database::open(&app_data.join("gaply.db"))?;
        match app_lib::ai::generative::resolve_generative_loader(&db, &app_data) {
            Some(l) => {
                let m = app_lib::ai::model_manager::ModelManager::new(l);
                println!(
                    "\x1b[33msuggest mode\x1b[0m — {} on {}. Each sentence costs one model call.",
                    m.model_id(),
                    app_lib::ai::device::shared().kind.as_str()
                );
                println!(
                    "\x1b[33mAnchoring:\x1b[0m accepted suggestions CANNOT score the model \
                     (the label carries its answer). Overrides are a lower bound.\n\
                     Label a cold set with no --suggest for the unbiased number.\n"
                );
                Some(m)
            }
            None => {
                println!("\x1b[31mno generative model installed — running without suggestions\x1b[0m\n");
                None
            }
        }
    } else {
        None
    };

    #[cfg(unix)]
    let _raw = RawMode::enter();
    #[cfg(unix)]
    let raw = _raw.is_some();
    #[cfg(not(unix))]
    let raw = false;
    if !raw {
        println!("(no raw terminal; press the key then Enter)\n");
    }

    let mut added = 0usize;
    let mut file = std::fs::OpenOptions::new().append(true).open(out_path)?;

    'outer: for (i, p) in candidates.iter().enumerate() {
        let locator = match (p.page, p.paragraph) {
            (Some(pg), _) => format!("p.{pg}"),
            (None, Some(par)) => format!("¶{par}"),
            (None, None) => "no locator".to_string(),
        };
        println!("\x1b[2m─────────────────────────────────────────────────────────\x1b[0m");
        println!(
            "\x1b[2m{} of {} unlabelled   ·   case file {} / {TARGET_CASES}\x1b[0m",
            i + 1,
            candidates.len(),
            total_cases + added
        );
        println!(
            "\x1b[36m{}\x1b[0m \x1b[2m·\x1b[0m \x1b[36m{}\x1b[0m",
            locator,
            p.section.as_deref().unwrap_or("(no section)")
        );
        println!("\n  {}\n", wrap(p.sentence.trim(), 72, "  "));

        // The model's proposal, shown VERBATIM. It is labelled as a proposal
        // every time — never rendered as a finding, never merged silently into
        // the record, and never accepted without a keystroke.
        let proposal = match manager.as_ref() {
            Some(m) => {
                print!("  \x1b[2masking the model…\x1b[0m");
                std::io::stdout().flush()?;
                let p = suggest(m, p.sentence.trim(), p.section.as_deref().unwrap_or("")).await;
                print!("\r                    \r");
                std::io::stdout().flush()?;
                p
            }
            None => None,
        };
        if let Some(sg) = &proposal {
            let label = TYPES
                .iter()
                .find(|(_, v, _)| *v == serde_json::to_value(sg.sentence_type)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default())
                .map(|(_, _, l)| *l)
                .unwrap_or("?");
            println!(
                "  \x1b[35mmodel proposes\x1b[0m  {}  ·  {}{}",
                if sg.needs_citation { "needs a citation" } else { "no citation needed" },
                label,
                sg.severity
                    .and_then(|s| serde_json::to_value(s).ok())
                    .and_then(|v| v.as_str().map(|x| format!("  ·  {x}")))
                    .unwrap_or_default(),
            );
            println!("  \x1b[2m  its reason: {}\x1b[0m", wrap(&sg.reason, 66, "              "));
            println!("  \x1b[2m  [a] accept as-is   — or answer below to override\x1b[0m");
        } else if suggest_mode {
            println!("  \x1b[31mmodel gave no usable answer — label it cold\x1b[0m");
        }

        // 1. needs_citation
        let mut accepted = false;
        let needs = loop {
            print!("  needs a citation?  \x1b[1my\x1b[0mes  \x1b[1mn\x1b[0mo  \x1b[1ms\x1b[0mkip  \x1b[1mq\x1b[0muit  ");
            std::io::stdout().flush()?;
            match key(raw) {
                'a' if proposal.is_some() => {
                    println!("accepted");
                    accepted = true;
                    break proposal.as_ref().expect("checked").needs_citation;
                }
                'y' => { println!("yes"); break true }
                'n' => { println!("no"); break false }
                's' => { println!("skipped\n"); continue 'outer }
                'q' => { println!("quit\n"); break 'outer }
                _ => println!(),
            }
        };

        // 2. sentence_type — skipped entirely when the whole proposal was
        //    accepted, so accepting is genuinely ONE key.
        let accepted_type = accepted
            .then(|| proposal.as_ref().and_then(|sg| serde_json::to_value(sg.sentence_type).ok()))
            .flatten()
            .and_then(|v| v.as_str().map(str::to_string));
        let stype: String = if let Some(t) = accepted_type {
            t
        } else {
        loop {
            println!("  type?");
            for row in TYPES.chunks(2) {
                let cells: Vec<String> = row
                    .iter()
                    .map(|(k, _, label)| format!("[\x1b[1m{k}\x1b[0m] {label:<22}"))
                    .collect();
                println!("    {}", cells.join(" "));
            }
            print!("  > ");
            std::io::stdout().flush()?;
            let k = key(raw);
            if k == 'q' {
                println!("quit\n");
                break 'outer;
            }
            if let Some((_, v, label)) = TYPES.iter().find(|(c, _, _)| *c == k) {
                println!("{label}");
                break v.to_string();
            }
            println!();
        }
        };

        // 3. severity — ONLY when a citation is needed. §11 D66: a sentence
        //    that needs none has no severity-of-need, and the schema treats the
        //    field as conditional rather than always-present.
        let severity: Option<String> = if !needs {
            None
        } else if accepted {
            proposal
                .as_ref()
                .and_then(|sg| sg.severity)
                .and_then(|s| serde_json::to_value(s).ok())
                .and_then(|v| v.as_str().map(str::to_string))
        } else {
            loop {
                print!("  severity?  \x1b[1mh\x1b[0migh  \x1b[1mm\x1b[0medium  \x1b[1ml\x1b[0mow  ");
                std::io::stdout().flush()?;
                match key(raw) {
                    'h' => { println!("high"); break Some("high".to_string()) }
                    'm' => { println!("medium"); break Some("medium".to_string()) }
                    'l' => { println!("low"); break Some("low".to_string()) }
                    'q' => { println!("quit\n"); break 'outer }
                    _ => println!(),
                }
            }
        };

        let idx = pre.planned.iter().position(|q| q.sentence == p.sentence).unwrap_or(0);
        let neighbour = |i: isize| -> String {
            if !with_neighbours {
                return String::new();
            }
            let j = idx as isize + i;
            if j < 0 {
                return String::new();
            }
            pre.planned.get(j as usize).map(|q| q.sentence.trim().to_string()).unwrap_or_default()
        };

        let mut expected = serde_json::Map::new();
        expected.insert("needs_citation".into(), needs.into());
        expected.insert("sentence_type".into(), stype.clone().into());
        if let Some(s) = &severity {
            expected.insert("severity".into(), s.clone().into());
        }

        max_label += 1;
        let case = serde_json::json!({
            "id": format!("cn-label-{max_label:03}"),
            "task": "citation_need",
            // For humans, never scored: where it came from, so a case can be
            // read back against the source it was labelled from.
            "note": format!(
                "{} · {} · {}",
                std::path::Path::new(manuscript)
                    .file_name().and_then(|n| n.to_str()).unwrap_or("manuscript"),
                locator,
                p.section.as_deref().unwrap_or("no section")
            ),
            "input": {
                "sentence": p.sentence.trim(),
                "preceding_sentence": neighbour(-1),
                "following_sentence": neighbour(1),
                "section": p.section.clone().unwrap_or_default(),
            },
            "expected": expected,
            // §11 D75. HOW this label came to exist. Written on every case,
            // because the three kinds are not interchangeable when scoring:
            // accepted suggestions cannot score the model at all, overrides are
            // a lower bound, and only cold labels give an unbiased number.
            // The model's own proposal is stored VERBATIM beside the human
            // answer so agreement is computable and contamination stays visible
            // rather than being remembered.
            "labelling": {
                "provenance": match (&proposal, accepted) {
                    (Some(_), true) => PROVENANCE_ACCEPTED,
                    (Some(_), false) => PROVENANCE_OVERRIDDEN,
                    (None, _) => PROVENANCE_COLD,
                },
                "model_id": manager.as_ref().map(|m| m.model_id()),
                "prompt_version":
                    app_lib::ai::tasks::citation_need::PROMPT_VERSION,
                "suggested": proposal.as_ref().map(|sg| serde_json::json!({
                    "needs_citation": sg.needs_citation,
                    "sentence_type": sg.sentence_type,
                    "severity": sg.severity,
                    "reason": sg.reason,
                })),
                // Which FIELDS the human changed — the per-field agreement the
                // aggregate rate would hide.
                "differs": proposal.as_ref().map(|sg| {
                    let sgt = serde_json::to_value(sg.sentence_type).ok()
                        .and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                    let sgs = sg.severity.and_then(|x| serde_json::to_value(x).ok())
                        .and_then(|v| v.as_str().map(str::to_string));
                    serde_json::json!({
                        "needs_citation": sg.needs_citation != needs,
                        "sentence_type": sgt != stype,
                        // NOT APPLICABLE when no citation is needed. §11 D66
                        // makes severity conditional, and the model emitting
                        // one anyway is the spec-compliant shape rather than a
                        // disagreement — comparing there manufactures a false
                        // difference and corrupts any per-field agreement rate.
                        "severity": if needs {
                            serde_json::json!(sgs != severity)
                        } else {
                            serde_json::Value::Null
                        },
                    })
                }),
            },
        });
        writeln!(file, "{}", serde_json::to_string(&case)?)?;
        file.flush()?;
        added += 1;
        println!();

        if total_cases + added >= TARGET_CASES {
            println!("\x1b[1m  reached {TARGET_CASES} cases.\x1b[0m Keep going or press q.\n");
        }
    }

    println!("\x1b[2m─────────────────────────────────────────────────────────\x1b[0m");
    println!(
        "\x1b[1madded {added}\x1b[0m  ·  case file now {} / {TARGET_CASES}",
        total_cases + added
    );
    println!("written to {}", out_path.display());
    Ok(())
}
