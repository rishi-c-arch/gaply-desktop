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

/// WHICH HALF OF THE AUDIT'S POPULATION A CASE WAS DRAWN FROM (§11 D125).
///
/// `prior_work` — the sentence makes a claim about the field, where "does this
/// need a citation?" is a real question. `own_work` — it reports what the
/// AUTHORS did or found: their hardware, their split, their numbers, their
/// ablations, their reading of them. Mostly own-work needs no citation, and it
/// is where the shipped audit does most of its flagging.
///
/// The distinction is NOT front-half/back-half. A Methodology sentence
/// describing the proposed algorithm is own-work; a Methodology sentence naming
/// the corpus it borrowed is not. That is why a human declares it.
pub const POPULATION_PRIOR: &str = "prior_work";
pub const POPULATION_OWN: &str = "own_work";

/// Per-document section counts, measured by this tool so the guard can compare
/// the labelled set's MIX against the population's (§11 D125).
const MIX_PATH: &str = "evals/population_mix.json";

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
    // Flags that TAKE A VALUE, so the value is not mistaken for the manuscript.
    const VALUED: &[&str] = &["--section", "--section-exact", "--population"];
    let value_of = |flag: &str| -> Option<String> {
        args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
    };
    let mut positional: Vec<&String> = Vec::new();
    let mut skip_next = false;
    for a in args.iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if VALUED.contains(&a.as_str()) {
            skip_next = true;
            continue;
        }
        if !a.starts_with("--") {
            positional.push(a);
        }
    }
    let manuscript = positional.first().copied().ok_or(
        "usage: label-cn <manuscript> [--with-neighbours] [--all] [--suggest] \
         [--section <substring>] [--population prior_work|own_work]",
    )?;

    // WHICH HALF OF THE POPULATION THIS SESSION IS LABELLING (§11 D125).
    //
    // Declared once per session rather than asked per sentence: a labelling run
    // is already scoped to a part of a paper, and 40 identical keystrokes is a
    // worse instrument than one statement. It is the LABELLER's judgement, not
    // a section-name regex — matching section titles classified 0 of 42 real
    // cases, because papers name their sections whatever they like.
    let population = value_of("--population");
    if let Some(v) = population.as_deref() {
        if v != POPULATION_PRIOR && v != POPULATION_OWN {
            return Err(format!(
                "--population must be {POPULATION_PRIOR} or {POPULATION_OWN}, got {v:?}"
            )
            .into());
        }
    }
    // Case-insensitive substring on the section heading. Without it, reaching
    // the Results half of R PAPER meant pressing through 30 already-covered
    // front-half candidates first.
    let section_filter = value_of("--section").map(|s| s.to_lowercase());
    // Exact match, for a name that is a SUBSTRING OF OTHERS — above all the
    // UNNAMED section, which `--section ""` would match everywhere and
    // overwrite every declaration in the document.
    let section_exact = value_of("--section-exact");
    let in_scope = |sec: Option<&str>| -> bool {
        let sec = sec.unwrap_or("");
        match (&section_exact, &section_filter) {
            (Some(e), _) => sec == e,
            (None, Some(f)) => sec.to_lowercase().contains(f),
            (None, None) => true,
        }
    };
    let doc_name = std::path::Path::new(manuscript)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("manuscript")
        .to_string();
    // Record the section->half declaration and STOP. Most sections are never
    // sampled — the population's mix still needs them classified, and walking
    // an interactive labelling loop to declare one is friction with no product.
    let declare_only = args.iter().any(|a| a == "--declare-only");
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
        .filter(|p| in_scope(p.section.as_deref()))
        .collect();

    // THE POPULATION, MEASURED — not the sample (§11 D125).
    //
    // The guard needs the mix of the sentences the AUDIT judges, and only a
    // pre-pass over the real document knows that. Counting is done here, where
    // the pre-pass already ran; classifying is the labeller's `--population`
    // declaration, recorded per section. Sections nobody has declared stay
    // `null` and the guard reports them as unknown rather than assuming a half.
    {
        let mut mix: serde_json::Value = std::fs::read_to_string(MIX_PATH)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        // Every PLANNED section in scope, not merely the unlabelled ones: a
        // section whose cases are all labelled is still a section that needs a
        // half, and most sections will never be sampled at all.
        let declared_here: std::collections::BTreeSet<String> = match &population {
            None => Default::default(),
            Some(_) => pre
                .planned
                .iter()
                .filter(|p| in_scope(p.section.as_deref()))
                .map(|p| p.section.clone().unwrap_or_default())
                .collect(),
        };
        let entry = mix
            .as_object_mut()
            .ok_or("population_mix.json is not an object")?
            .entry(doc_name.clone())
            .or_insert_with(|| serde_json::json!({"sections": {}}));
        entry["measured_at"] = serde_json::json!(gaply_core::now_epoch());
        entry["planned_total"] = serde_json::json!(pre.planned.len());
        let mut per_section: std::collections::BTreeMap<String, usize> = Default::default();
        for p in &pre.planned {
            *per_section.entry(p.section.clone().unwrap_or_default()).or_default() += 1;
        }
        for (name, planned) in per_section {
            let prev = entry["sections"][&name]["population"].clone();
            let declared = if declared_here.contains(&name) {
                serde_json::json!(population)
            } else {
                // Never silently un-declares: an earlier session's judgement
                // stands until a later one overwrites that same section.
                if prev.is_null() { serde_json::Value::Null } else { prev }
            };
            entry["sections"][&name] =
                serde_json::json!({ "planned": planned, "population": declared });
        }
        std::fs::write(MIX_PATH, serde_json::to_string_pretty(&mix)? + "\n")?;

        if declare_only {
            if population.is_none() {
                return Err("--declare-only needs --population".into());
            }
            println!(
                "declared {} section(s) of {doc_name} as {}:",
                declared_here.len(),
                population.as_deref().unwrap_or("?")
            );
            for name in &declared_here {
                println!("  {name}");
            }
            return Ok(());
        }
    }

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
                // WHICH DOCUMENT (§11 D125). The half is NOT stored here: it
                // is looked up from `population_mix.json` by (document,
                // section), the SAME map the population's mix is summed from.
                // Storing a per-case answer would give the two sides of that
                // comparison two different instruments, and the sample would
                // drift from the population one override at a time.
                "document": doc_name,
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
