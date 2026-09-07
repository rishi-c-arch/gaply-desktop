//! Label `citation_support` cases against a source that is really in the
//! library (§11 D99).
//!
//! ```text
//! cargo run --release --bin label-cs -- --list
//! cargo run --release --bin label-cs -- ~/Desktop/"R PAPER .pdf" --doc 9
//! ```
//!
//! # Why this exists
//!
//! `citation_support` LEADS the report — §11 D78 calls it "the half worth
//! trusting" — and its whole eval is six synthetic seeds written in-house,
//! scoring 20-25% verdict agreement. `citation_need`, the half demoted to
//! advisory, has 42 cold labels from real papers. That is the wrong way round,
//! and §11 D98 is why it matters: four checks in a row were wrong on their
//! first real document, and two of them missed the exact defect they were
//! written for.
//!
//! # Candidates come from MENTIONS, not from resolved markers
//!
//! Measured before this was written: every audit job in the database holds
//! FOUR distinct `citation_support` claims, one of them an artefact of the §11
//! D80 mis-run. Driving the tool from marker resolution would offer three cases
//! and stop. `R PAPER` mentions a library source in 22 sentences, so a sentence
//! is a candidate when it MENTIONS the chosen source.
//!
//! # Each case carries its own evidence
//!
//! The retrieved passages are written to a fixture beside the case, so
//! `ai-eval` shows the model the evidence the labeller saw. The set measures
//! JUDGEMENT over fixed evidence rather than retrieval, and replays on a
//! machine that has never held this library.
use std::io::{Read, Write};

/// Cold labels required before `--suggest` is allowed (§11 D75).
const COLD_BEFORE_SUGGEST: usize = 20;

/// The five spec verdicts, with the key that selects each.
const VERDICTS: &[(char, &str, &str)] = &[
    ('s', "strong", "every element of the claim is in the evidence"),
    ('p', "partial", "some of it is; the claim overreaches"),
    ('w', "weak", "only topically related"),
    ('c', "contradicts", "the evidence says otherwise"),
    ('i', "insufficient_evidence", "the passages cannot settle it"),
];

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
        if col + w.chars().count() + 1 > width {
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

/// Candidate names for a source, from its own title.
fn title_words(title: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "of", "and", "in", "for", "on", "with", "task", "dataset", "data",
        "study", "analysis", "using", "based", "among", "from", "fine", "grained", "guidelines",
        "retracted", "chapter", "review", "literature", "incidence",
    ];
    title
        .split(|c: char| !c.is_alphanumeric() && c != '-')
        .filter(|w| w.chars().count() >= 5 && !STOP.contains(&w.to_lowercase().as_str()))
        .map(|w| w.to_string())
        .collect()
}

/// A word distinctive enough to say "this sentence is about that source".
///
/// DERIVED FROM THE MANUSCRIPT, not guessed from the title. Picking the longest
/// title word chose "Fine-Grained" for *GoEmotions: A Dataset of Fine-Grained
/// Emotions* — a phrase the citing paper never uses — and picking the first
/// would choose "Incidence" for the needlestick study. Neither heuristic is
/// right; how often a candidate actually appears in the text that cites it is.
///
/// Falls back to the longest when nothing matches, and the caller says so.
fn match_word(title: &str, sentences: &[String]) -> Option<(String, usize)> {
    let words = title_words(title);
    let best = words
        .iter()
        .map(|w| {
            let lw = w.to_lowercase();
            let hits = sentences.iter().filter(|s| s.to_lowercase().contains(&lw)).count();
            (w.clone(), hits)
        })
        .max_by_key(|(w, hits)| (*hits, w.chars().count()))?;
    if best.1 > 0 {
        return Some(best);
    }
    words.into_iter().max_by_key(|w| w.chars().count()).map(|w| (w, 0))
}

/// The library, as this tool needs it. Uses `store::list_documents` rather than
/// SQL: `Database::conn` is `pub(crate)` and the app crate does not execute SQL
/// (plan §7). Returns the struct it already has — flattening it into a 4-tuple
/// gained nothing and cost the reader the field names.
fn checkable_docs(
    db: &gaply_core::Database,
) -> Result<Vec<gaply_core::ai_engine::store::DocumentSummary>, Box<dyn std::error::Error>> {
    Ok(gaply_core::ai_engine::store::list_documents(db)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| -> Option<String> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
    };
    let home = std::path::PathBuf::from(std::env::var("HOME")?);
    let app_data = home.join("Library/Application Support/ai.gaply.app");
    let db = gaply_core::Database::open(&app_data.join("gaply.db"))?;

    // --list: which sources a claim can actually be checked against.
    if args.iter().any(|a| a == "--list") {
        println!("\x1b[1mSources in the library a claim can be checked against\x1b[0m");
        for d in checkable_docs(&db)? {
            let ready = if d.embedded_count > 0 {
                "\x1b[32mready      \x1b[0m"
            } else {
                "\x1b[31mnot embedded\x1b[0m"
            };
            println!(
                "  --doc {:<3} {ready} {:>4} passages  {:<44} \x1b[2m[names: {}]\x1b[0m",
                d.id,
                d.chunk_count,
                d.title.chars().take(44).collect::<String>(),
                title_words(&d.title).join(" | ")
            );
        }
        return Ok(());
    }

    let doc_arg = arg("--doc");
    let match_arg = arg("--match");
    let manuscript = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .find(|a| Some((*a).clone()) != doc_arg && Some((*a).clone()) != match_arg)
        .filter(|a| std::path::Path::new(a).exists())
        .ok_or("usage: label-cs <manuscript> --doc <id> [--match <word>]  |  label-cs --list")?
        .clone();
    let doc_id: i64 = doc_arg
        .ok_or("--doc <id> is required; run --list to see the options")?
        .parse()?;
    let suggest_mode = args.iter().any(|a| a == "--suggest");

    let out_path = std::path::Path::new("evals/citation_support.jsonl");
    let fixtures_dir = std::path::Path::new("evals/fixtures");
    if !out_path.exists() {
        return Err("run this from src-tauri/ so evals/ is where ai-eval expects it".into());
    }
    std::fs::create_dir_all(fixtures_dir)?;

    // COLD ONLY until the set is real (§11 D75).
    let existing_raw = std::fs::read_to_string(out_path)?;
    let mut cold = 0usize;
    let mut total_cases = 0usize;
    let mut max_label = 0usize;
    let mut done: std::collections::HashSet<String> = Default::default();
    for line in existing_raw.lines().filter(|l| !l.trim().is_empty()) {
        total_cases += 1;
        let v: serde_json::Value = serde_json::from_str(line)?;
        if v["labelling"]["provenance"] == "cold" {
            cold += 1;
        }
        if let Some(c) = v["input"]["claim"].as_str() {
            done.insert(c.trim().to_string());
        }
        if let Some(n) = v["id"]
            .as_str()
            .and_then(|i| i.strip_prefix("cs-label-"))
            .and_then(|n| n.parse::<usize>().ok())
        {
            max_label = max_label.max(n);
        }
    }
    if suggest_mode && cold < COLD_BEFORE_SUGGEST {
        return Err(format!(
            "--suggest is refused: {cold} cold labels exist and {COLD_BEFORE_SUGGEST} are \
             required first.\nA suggested-accepted label carries the model's own answer and \
             cannot score it (§11 D75), and a confident proposal moves the labeller's \
             judgement rather than only their keystroke."
        )
        .into());
    }

    let (title, embedded) = checkable_docs(&db)?
        .into_iter()
        .find(|d| d.id == doc_id)
        .map(|d| (d.title, d.embedded_count))
        .ok_or_else(|| format!("no document {doc_id} with indexed passages; run --list"))?;
    if embedded == 0 {
        return Err(format!(
            "document {doc_id} (“{title}”) has no embedded passages, so nothing can be \
             retrieved from it. Index and embed it first."
        )
        .into());
    }
    let blocks = gaply_core::extract::docparse::parse_path_paged(std::path::Path::new(&manuscript))?;
    let pre = gaply_core::ai_engine::audit_prepass::prepass_blocks(&blocks);
    let all_sentences: Vec<String> =
        pre.planned.iter().map(|p| p.sentence.clone()).collect();

    let (needle, hits) = match match_arg {
        Some(w) => {
            let lw = w.to_lowercase();
            let n = all_sentences.iter().filter(|s| s.to_lowercase().contains(&lw)).count();
            (w, n)
        }
        None => match_word(&title, &all_sentences)
            .ok_or("could not derive a name from the title; pass --match <word>")?,
    };
    if hits == 0 {
        println!(
            "\x1b[33mno sentence mentions “{needle}”.\x1b[0m Candidates from this title: {}.\n\
             Pass --match <word> with whatever this manuscript calls the source.",
            title_words(&title).join(", ")
        );
        return Ok(());
    }
    let candidates: Vec<_> = pre
        .planned
        .iter()
        .filter(|p| p.sentence.to_lowercase().contains(&needle.to_lowercase()))
        .filter(|p| !done.contains(p.sentence.trim()))
        .collect();

    println!("\x1b[1m{manuscript}\x1b[0m");
    println!("  source: \x1b[36m{title}\x1b[0m  (document {doc_id}, {embedded} embedded passages)");
    println!(
        "  candidates: sentences mentioning \x1b[36m{needle}\x1b[0m — {} unlabelled",
        candidates.len()
    );
    println!("  case file: {total_cases} cases, {cold} of them cold\n");
    if candidates.is_empty() {
        println!("nothing left to label for this source. Try another --doc, or --match <word>.");
        return Ok(());
    }

    let embedder = match app_lib::ai::embeddings::EmbeddingEngine::load_if_installed(&app_data).0 {
        Some(e) => e,
        None => return Err("no embedding model installed — retrieval cannot run".into()),
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
        let claim = p.sentence.trim();
        // THE EVIDENCE THE MODEL WOULD SEE — same retrieval, same budget.
        let qv = embedder.embed_query(claim)?;
        let bundle = app_lib::ai::evidence::assemble(
            &db,
            doc_id,
            claim,
            &qv,
            app_lib::ai::evidence::EVIDENCE_BUDGET_TOKENS,
        )?;
        let passages = match &bundle {
            app_lib::ai::evidence::Assembled::NoEvidence { reason } => {
                println!("\x1b[2m─────────────────────────────────────────────────────\x1b[0m");
                println!("  {}\n", wrap(claim, 72, "  "));
                println!("  \x1b[31mno evidence retrieved:\x1b[0m {reason}");
                println!(
                    "  \x1b[2mskipped — a case with no evidence measures retrieval, \
                     not judgement\x1b[0m\n"
                );
                continue;
            }
            // `examined` is the rank-ordered set, and public. `ctx` keys them by
            // id, which loses the ranking a labeller reads them in.
            app_lib::ai::evidence::Assembled::Ready(b) => b.examined.clone(),
        };

        println!("\x1b[2m─────────────────────────────────────────────────────\x1b[0m");
        println!(
            "\x1b[2m{} of {} · case file {} · cold {}\x1b[0m",
            i + 1,
            candidates.len(),
            total_cases + added,
            cold + added
        );
        println!("\n\x1b[1mCLAIM\x1b[0m");
        println!("  {}\n", wrap(claim, 72, "  "));
        println!("\x1b[1mCITED SOURCE\x1b[0m");
        println!("  {title}\n");
        println!(
            "\x1b[1mRETRIEVED PASSAGES\x1b[0m \x1b[2m(what the model would be shown)\x1b[0m"
        );
        for (n, c) in passages.iter().enumerate() {
            let page = c.page.map(|p| format!("p.{p}")).unwrap_or_else(|| "page unknown".into());
            println!("\n  \x1b[33m[{}]\x1b[0m \x1b[2m{page}\x1b[0m", n + 1);
            println!("  {}", wrap(&c.text, 70, "  "));
        }

        println!("\n\x1b[1mYOUR VERDICT\x1b[0m");
        for (k, name, help) in VERDICTS {
            println!("  \x1b[33m{k}\x1b[0m  {name:<22} \x1b[2m{help}\x1b[0m");
        }
        println!("  \x1b[33mk\x1b[0m  skip    \x1b[33mq\x1b[0m  quit");
        print!("\n> ");
        std::io::stdout().flush()?;

        let verdict = loop {
            match key(raw) {
                'q' => break 'outer,
                'k' => {
                    println!("skipped\n");
                    continue 'outer;
                }
                c => {
                    if let Some((_, name, _)) = VERDICTS.iter().find(|(k, _, _)| *k == c) {
                        println!("{name}");
                        break *name;
                    }
                }
            }
        };

        // WHICH passages settle it — the second half of the label, and what
        // makes `citedPlantedChunk` scoreable.
        //
        // §11 D106. The question follows the VERDICT. On `contradicts` the
        // passages being marked are the ones that refute the claim, and asking
        // which of them "support it" invites the wrong selection — or an empty
        // one, which would score as no evidence rather than as refuting
        // evidence. The stored field is the same either way: the passages that
        // settle it.
        let mut supporting: Vec<usize> = Vec::new();
        if verdict != "insufficient_evidence" {
            let question = if verdict == "contradicts" {
                "WHICH PASSAGES CONTRADICT IT?"
            } else {
                "WHICH PASSAGES SUPPORT IT?"
            };
            println!(
                "\n\x1b[1m{question}\x1b[0m \x1b[2m1-{} to toggle, \
                 Enter when done, x for none\x1b[0m",
                passages.len().min(9)
            );
            loop {
                print!("  selected: {supporting:?} > ");
                std::io::stdout().flush()?;
                match key(raw) {
                    'q' => break 'outer,
                    'x' => {
                        supporting.clear();
                        println!("none");
                        break;
                    }
                    '\n' | '\r' | ' ' => {
                        println!();
                        break;
                    }
                    c if c.is_ascii_digit() => {
                        let n = c.to_digit(10).unwrap_or(0) as usize;
                        if n >= 1 && n <= passages.len() {
                            if let Some(pos) = supporting.iter().position(|x| *x == n) {
                                supporting.remove(pos);
                            } else {
                                supporting.push(n);
                            }
                            supporting.sort_unstable();
                        }
                        println!();
                    }
                    _ => println!(),
                }
            }
        }

        // The fixture: exactly the passages shown, so the case replays against
        // the evidence the label was formed on.
        let n = max_label + added + 1;
        let id = format!("cs-label-{n:03}");
        let fixture_name = format!("{id}.txt");
        let fixture_body: String =
            passages.iter().map(|c| c.text.trim().to_string()).collect::<Vec<_>>().join("\n\n");
        std::fs::write(fixtures_dir.join(&fixture_name), &fixture_body)?;

        // A distinctive substring of the FIRST supporting passage, for
        // `citedPlantedChunk`. Absent when nothing supports the claim.
        let planted = supporting
            .first()
            .and_then(|n| passages.get(n - 1))
            .map(|c| c.text.split_whitespace().take(8).collect::<Vec<_>>().join(" "));

        let mut expected = serde_json::Map::new();
        expected.insert("verdict".into(), serde_json::json!(verdict));
        if let Some(p) = planted {
            expected.insert("planted_chunk_contains".into(), serde_json::json!(p));
        }
        expected.insert("supporting_count".into(), serde_json::json!(supporting.len()));

        let case = serde_json::json!({
            "id": id,
            "task": "citation_support",
            "note": format!(
                "{} · {} · doc {doc_id}",
                std::path::Path::new(&manuscript)
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default(),
                title
            ),
            "input": {
                "claim": claim,
                "cited_source": title,
                "fixture": fixture_name,
            },
            "expected": serde_json::Value::Object(expected),
            "labelling": {
                "provenance": "cold",
                "model_id": serde_json::Value::Null,
                "prompt_version": app_lib::ai::tasks::citation_support::PROMPT_VERSION,
                "suggested": serde_json::Value::Null,
                "differs": serde_json::Value::Null,
            }
        });
        writeln!(file, "{case}")?;
        file.flush()?;
        added += 1;
        println!(
            "  \x1b[32mrecorded {id}\x1b[0m  ({verdict}, {} supporting)\n",
            supporting.len()
        );
    }

    println!(
        "\n\x1b[1m{added} labelled\x1b[0m  ·  {} cases in the file, {} cold",
        total_cases + added,
        cold + added
    );
    if cold + added < COLD_BEFORE_SUGGEST {
        println!(
            "\x1b[2m{} more cold labels before --suggest is allowed.\x1b[0m",
            COLD_BEFORE_SUGGEST - (cold + added)
        );
    }
    Ok(())
}
