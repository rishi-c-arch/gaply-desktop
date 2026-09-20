//! **Phase B: score SLM1 on D165's fixed inputs.**
//!
//! §11 D165 declined the scientific layer at 5.9% precision against a 50%
//! no-skill baseline. That precision was measured over the extractor's OWN
//! output, so a second extractor cannot be compared to it. This runs the model
//! over the SAME paragraphs the regex was scored on, which fixes the denominator.
//!
//! **Two prompt variants, because one measures the prompt as much as the model.**
//! §11 D121 measured that this model class imitates worked examples as templates
//! and that five variants across two tasks landed at or below baseline. So there
//! are no few-shot examples here, and a second phrasing runs alongside the first.
//!
//! ```text
//! cargo run --release --example methods_model_probe -- /tmp/pool_labelled.jsonl
//! ```
use std::sync::atomic::AtomicBool;

use app_lib::ai::generative::{BundledGenerativeLoader, GenRequest};
use app_lib::ai::model_manager::BackendLoader;

const SYSTEM: &str =
    "You read paragraphs from research manuscripts and answer a single question about each. \
     You answer with one word only.";

fn prompt_a(text: &str) -> String {
    format!(
        "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
         <|im_start|>user\n\
         Does this paragraph state something the authors DID in this study - a procedure they \
         performed, an instrument they used, how they sampled, their study design, or an \
         analysis they ran?\n\n\
         Answer no if it is a heading, a table, a figure caption, a formula, a title, keywords, \
         an author list, page furniture, background about the field, someone else's work, or a \
         result or an interpretation.\n\n\
         Paragraph:\n---\n{}\n---\n\n\
         Answer yes or no.<|im_end|>\n\
         <|im_start|>assistant\n",
        text.chars().take(1800).collect::<String>()
    )
}

fn prompt_b(text: &str) -> String {
    format!(
        "<|im_start|>system\n{SYSTEM}\n<|im_end|>\n\
         <|im_start|>user\n\
         Is the text below part of the Methods of the study that wrote it - that is, does it \
         describe what these authors actually did?\n\n\
         Text:\n---\n{}\n---\n\n\
         Reply with one word: yes or no.<|im_end|>\n\
         <|im_start|>assistant\n",
        text.chars().take(1800).collect::<String>()
    )
}

/// Parse a one-word answer. **`None` is a real outcome, not a zero** — an
/// unparseable reply is a format failure and is counted as one rather than
/// silently scored as "no", which would flatter the model on a pool that is 82%
/// negative.
fn parse(raw: &str) -> Option<bool> {
    let l = raw.trim().to_lowercase();
    let head: String = l.chars().take(24).collect();
    let y = head.starts_with("yes") || head.contains(" yes") || head.starts_with("**yes");
    let n = head.starts_with("no") || head.contains(" no") || head.starts_with("**no");
    match (y, n) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: methods_model_probe <pool_labelled.jsonl>");
    let rows: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .expect("pool")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();

    let loader = BundledGenerativeLoader::resolve().expect(
        "no bundled generative model resolved - SLM1 must be present for this measurement",
    );
    let est = loader.ram_estimate().expect("ram estimate");
    eprintln!("model: {}  ram estimate: {:?}", loader.model_id(), est);
    let backend = loader.load().expect("load");
    eprintln!("loaded weights: {}", backend.loaded_model_file());

    let cancel = AtomicBool::new(false);
    let variants: [(&str, fn(&str) -> String); 2] = [("A", prompt_a), ("B", prompt_b)];

    println!("variant\tidx\tlabel\tregex\tnoskill\tanswer\traw");
    for (name, build) in variants {
        let t0 = std::time::Instant::now();
        for (i, r) in rows.iter().enumerate() {
            let text = r["text"].as_str().unwrap_or("");
            let out = backend.generate(GenRequest {
                prompt: build(text),
                max_tokens: 6,
                cancel: &cancel,
                on_token: None,
            });
            let (ans, raw) = match &out {
                Ok(o) => (parse(&o.text), o.text.replace(['\n', '\t'], " ")),
                Err(e) => (None, format!("ERROR {e}")),
            };
            println!(
                "{name}\t{i}\t{}\t{}\t{}\t{}\t{}",
                r["label"].as_i64().unwrap_or(-1),
                r["regex_flagged"].as_bool().unwrap_or(false),
                r["noskill_guess"].as_bool().unwrap_or(false),
                match ans {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unparseable",
                },
                raw.chars().take(40).collect::<String>()
            );
            if i % 20 == 0 {
                eprintln!("  {name}: {i}/{}  {:?}", rows.len(), t0.elapsed());
            }
        }
        eprintln!("variant {name} done in {:?}", t0.elapsed());
    }
}
