//! **The same 160 paragraphs, with their place in the document restored.**
//!
//! §11 D204 declined the scientific layer against three model tiers and wrote
//! the reopening condition as an INPUT rather than a model: *"whether a
//! paragraph describes what THESE authors did is a fact about its place in a
//! document, and the probe hands over a paragraph with its place removed."*
//! This hands it back — the section heading that governs the paragraph, and the
//! paragraphs immediately before and after it — and changes nothing else.
//!
//! **What is byte-identical to §11 D204**: the 160 paragraphs, the §11 D198
//! corrected labels, the proxy, the App Check header, the `/verify` route, the
//! structured validator, the fence-tolerant verbatim read at `max_tokens: 24`,
//! and the two instruction texts, which are reproduced here character for
//! character from `methods_cloud_probe.rs`.
//!
//! **What deviates, stated because a comparison is only as honest as its
//! deviations**: one framing sentence is PREPENDED to each instruction, naming
//! the three new fields and saying the question is about `summary` alone. There
//! is no way to supply context without telling the model what it is looking at,
//! so the deviation is unavoidable; it is confined to a prefix, and the question
//! itself is untouched.
//!
//! **The context travels as four separate fields, not one concatenated blob.**
//! The proxy's validator caps a single field at 2000 chars, and the joined text
//! runs to 5734 — a concatenated payload would be refused on most of the set.
//! Measured against `app/validation.py` directly before this probe was written:
//! the four-field shape passes 144 of 160, the §11 D204 shape passes 148, and
//! the 16 exclusions are a STRICT SUPERSET of D204's 12, so the two runs are
//! comparable on 144 common rows without relaxing anything. The extra four are
//! each a long NEIGHBOUR, never the total (max 5734 of an 8000 limit).
//!
//! ```text
//! GAPLY_PROXY_URL=http://127.0.0.1:8080 GRRB_APP_CHECK_KEY=<key> GRRB_DELAY_MS=1200 \
//!   cargo run --release --example methods_ctx_probe -- evals/grrb/scientific_extraction_ctx.jsonl
//! ```
use gaply_core::app_check::{TokenSigner, DEFAULT_APP_ID};

/// Verbatim from `methods_cloud_probe.rs`. Do not edit: a changed question
/// makes the §11 D204 rows uncomparable, which is the whole point of the run.
fn question(variant: &str) -> &'static str {
    match variant {
        "A" => "Does the summary state something the authors DID in this study - a procedure \
                they performed, an instrument they used, how they sampled, their study design, \
                or an analysis they ran? Answer no if it is a heading, a table, a figure \
                caption, a formula, a title, keywords, an author list, page furniture, \
                background about the field, someone else's work, or a result or an \
                interpretation. Reply with ONLY a JSON object: {\"answer\":\"yes\"} or \
                {\"answer\":\"no\"}.",
        _ => "Is the summary part of the Methods of the study that wrote it - that is, does it \
              describe what these authors actually did? Reply with ONLY a JSON object: \
              {\"answer\":\"yes\"} or {\"answer\":\"no\"}.",
    }
}

/// The only deviation from §11 D204's prompt, isolated so it can be read.
const FRAMING: &str = "The summary is one paragraph of a manuscript, given with its place in \
    that manuscript: section_heading is the heading of the section it sits in, and \
    preceding_paragraph and following_paragraph are the paragraphs immediately before and \
    after it. Those three fields are context only - judge the summary field itself. ";

fn parse(raw: &str) -> Option<bool> {
    let head: String = raw.trim().to_lowercase().chars().take(24).collect();
    let y = head.starts_with("yes") || head.contains(" yes");
    let n = head.starts_with("no") || head.contains(" no");
    match (y, n) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

/// Fence-tolerant, as §11 D204 established: gpt-4o wraps its reply in a ```json
/// fence about 80% of the time, and a strict JSON parse records a correct answer
/// as an error.
fn extract_answer(text: &str) -> Option<bool> {
    if let (Some(a), Some(b)) = (text.find('{'), text.rfind('}')) {
        if b > a {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text[a..=b]) {
                if let Some(s) = v["answer"].as_str() {
                    return parse(s);
                }
            }
        }
    }
    parse(text)
}

/// Returns `(text, stop_reason, model)`. The model is read from the API's own
/// echo rather than from config — §11 D199 is why.
fn raw_verify(
    url: &str,
    signer: &TokenSigner,
    payload: &serde_json::Value,
) -> Result<(String, String, String), String> {
    let (hname, token) =
        gaply_core::app_check::proxy_auth_header(signer).map_err(|e| e.to_string())?;
    let c = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let r = c
        .post(format!("{url}/verify"))
        .header(hname, token)
        .json(payload)
        .send()
        .map_err(|e| e.to_string())?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("{status}: {}", body.chars().take(160).collect::<String>()));
    }
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    Ok((
        v["result"]["text"].as_str().unwrap_or_default().to_string(),
        v["result"]["stop_reason"].as_str().unwrap_or_default().to_string(),
        v["result"]["model"].as_str().unwrap_or("UNREPORTED").to_string(),
    ))
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: methods_ctx_probe <cases_ctx.jsonl>");
    let key = std::env::var("GRRB_APP_CHECK_KEY")
        .expect("GRRB_APP_CHECK_KEY must match the proxy's APP_CHECK_SIGNING_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let signer = TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID);

    let cases: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .expect("cases")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert!(
        cases.iter().all(|c| c["context"]["heading"].is_string()),
        "every case must carry context — this probe exists to send it"
    );
    eprintln!("cases: {}  proxy: {url}", cases.len());

    let delay: u64 =
        std::env::var("GRRB_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1200);
    let only = std::env::var("GRRB_ONLY").ok();
    let max_tokens: u32 =
        std::env::var("GRRB_MAX_TOKENS").ok().and_then(|v| v.parse().ok()).unwrap_or(24);

    println!("variant\tid\tlabel\tanswer\tkind\tdetail");
    let mut models: std::collections::BTreeSet<String> = Default::default();
    for variant in ["A", "B"] {
        if only.as_deref().is_some_and(|o| o != variant) {
            continue;
        }
        let instruction = format!("{FRAMING}{}", question(variant));
        let t0 = std::time::Instant::now();
        for (i, c) in cases.iter().enumerate() {
            let ctx = &c["context"];
            let payload = serde_json::json!({
                "section_heading": ctx["heading"],
                "preceding_paragraph": ctx["prev"],
                "summary": c["input"]["text"],
                "following_paragraph": ctx["next"],
                "instruction": instruction,
                "max_tokens": max_tokens,
            });
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            let (answer, detail) = match raw_verify(&url, &signer, &payload) {
                Ok((text, stop, model)) => {
                    models.insert(model);
                    (
                        match extract_answer(&text) {
                            Some(true) => "yes",
                            Some(false) => "no",
                            None => "unparseable",
                        },
                        format!(
                            "stop={stop} {}",
                            text.replace(['\n', '\t'], " ").chars().take(48).collect::<String>()
                        ),
                    )
                }
                // A 422 is the VALIDATOR refusing the payload. Recorded as
                // excluded, never as a wrong answer — §11 D204's rule.
                Err(msg) => {
                    let kind = if msg.contains("422") || msg.contains("validation") {
                        "excluded"
                    } else {
                        "error"
                    };
                    (kind, msg.replace(['\n', '\t'], " ").chars().take(150).collect::<String>())
                }
            };
            println!(
                "{variant}\t{}\t{}\t{answer}\t{}\t{detail}",
                c["id"].as_str().unwrap_or("?"),
                if c["expected"]["finding"].as_bool().unwrap_or(false) { 1 } else { 0 },
                ctx["kind"].as_str().unwrap_or("?"),
            );
            if i % 25 == 0 {
                eprintln!("  {variant}: {i}/{}  {:?}", cases.len(), t0.elapsed());
            }
        }
        eprintln!("variant {variant} done in {:?}", t0.elapsed());
    }
    eprintln!("models echoed by the API: {models:?}");
}
