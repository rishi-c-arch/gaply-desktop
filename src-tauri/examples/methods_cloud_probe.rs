//! **Phase B, cloud tier: the same 160 paragraphs through the REAL ProxyClient.**
//!
//! §11 D196 scored SLM1 (Qwen2.5-0.5B-Q4_K_M) on this pool and it landed below
//! the regex, below the 50% no-skill baseline, and below the base rate — on
//! judgment, not format (0 unparseable in 320 calls). This asks whether scale
//! fixes a judgment failure.
//!
//! **It goes through `ProxyClient::verify_with_envelope`, exactly as the product
//! does** — App Check header, the `/verify` route, the structured validator. Not
//! a direct provider call: a direct call would skip the one thing the proxy
//! exists for, and the resulting number would be about a payload Gaply's own
//! architecture refuses.
//!
//! **That refusal is a measured part of the result, not an obstacle to it.** The
//! validator rejects a field over 2000 chars or over 8 sentences, so 12 of the
//! 160 never reach a model at all. They are recorded as `excluded`, never as a
//! wrong answer, and the entry states that they are systematically the longest
//! and most prose-like — the half where the judgment is hardest.
//!
//! ```text
//! GAPLY_PROXY_URL=http://127.0.0.1:8080 \
//! GRRB_APP_CHECK_KEY=<same key the proxy was started with> \
//!   cargo run --release --example methods_cloud_probe -- evals/grrb/scientific_extraction.jsonl
//! ```
use gaply_core::app_check::{TokenSigner, DEFAULT_APP_ID};

fn instruction(variant: &str) -> &'static str {
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

/// Fence-tolerant read of `result.text` (§11 D204). `verify_with_envelope`
/// parses that field as JSON and reports only "proxy reply is not valid JSON"
/// when it cannot — a fact about the PARSE, not about what the model said.
/// gpt-4o wraps its reply in a ```json fence about 80% of the time (measured:
/// 8 of 10 identical calls), and at `max_tokens: 8` the fence truncates before
/// it closes, so a correct, legible "yes" is recorded as an error.
///
/// Enabled by `GRRB_VERBATIM=1`, OFF by default, so the path §11 D197 measured
/// is byte-identical unless asked otherwise. `GRRB_MAX_TOKENS` raises the
/// budget so a fenced reply can finish.
fn extract_answer(text: &str) -> Option<bool> {
    // the object inside any fence or prose
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

fn raw_verify(
    url: &str,
    signer: &TokenSigner,
    payload: &serde_json::Value,
) -> Result<(String, String), String> {
    let (hname, token) = gaply_core::app_check::proxy_auth_header(signer).map_err(|e| e.to_string())?;
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
        return Err(format!("{status}: {}", body.chars().take(120).collect::<String>()));
    }
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let text = v["result"]["text"].as_str().unwrap_or_default().to_string();
    let stop = v["result"]["stop_reason"].as_str().unwrap_or_default().to_string();
    Ok((text, stop))
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: methods_cloud_probe <cases.jsonl>");
    let key = std::env::var("GRRB_APP_CHECK_KEY")
        .expect("GRRB_APP_CHECK_KEY must match the proxy's APP_CHECK_SIGNING_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());

    let signer = TokenSigner::new(key.clone().into_bytes(), DEFAULT_APP_ID);
    let signer2 = TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID);
    let client = app_lib::models::proxy_client::ProxyReqwestClient::new(&url, signer)
        .expect("build ProxyClient");
    eprintln!("proxy: {url}   reachable: {}", client.reachable());

    let cases: Vec<serde_json::Value> = std::fs::read_to_string(&path)
        .expect("cases")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    eprintln!("cases: {}", cases.len());

    println!("variant\tid\tlabel\tanswer\tdetail");
    // **The proxy rate-limits at 60 capacity, 1 token/s refill.** Firing 160
    // requests back to back drains it and the rest return 429 — which is the
    // proxy working, not failing, and is itself a measured property of this
    // path. `GRRB_DELAY_MS` paces to stay inside it; `GRRB_ONLY` runs one
    // variant so a drained bucket from the first does not corrupt the second.
    let delay = std::env::var("GRRB_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0u64);
    let only = std::env::var("GRRB_ONLY").ok();
    let verbatim = std::env::var("GRRB_VERBATIM").is_ok();
    let max_tokens: u32 = std::env::var("GRRB_MAX_TOKENS").ok()
        .and_then(|v| v.parse().ok()).unwrap_or(8);
    eprintln!("mode: {}  max_tokens: {max_tokens}",
        if verbatim { "VERBATIM (fence-tolerant)" } else { "typed client (as §11 D197)" });
    for variant in ["A", "B"] {
        if only.as_deref().is_some_and(|o| o != variant) {
            continue;
        }
        let t0 = std::time::Instant::now();
        for (i, c) in cases.iter().enumerate() {
            let text = c["input"]["text"].as_str().unwrap_or("");
            let payload = serde_json::json!({
                "summary": text,
                "instruction": instruction(variant),
                "max_tokens": max_tokens,
            });
            if delay > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            let (answer, detail) = if verbatim {
                match raw_verify(&url, &signer2, &payload) {
                    Ok((text, stop)) => (
                        match extract_answer(&text) {
                            Some(true) => "yes",
                            Some(false) => "no",
                            None => "unparseable",
                        },
                        format!("stop={stop} {}", text.replace(['\n', '\t'], " ")
                            .chars().take(48).collect::<String>()),
                    ),
                    Err(msg) => {
                        let kind = if msg.contains("422") || msg.contains("validation") {
                            "excluded"
                        } else {
                            "error"
                        };
                        (kind, msg.replace(['\n', '\t'], " ").chars().take(120).collect::<String>())
                    }
                }
            } else { match client.verify_with_envelope(&payload) {
                // `verify_with_envelope` parses `result.text` as JSON and
                // returns the PARSED value — the product's replies are objects,
                // which is why a bare "yes" fails here. The question put to the
                // model is identical to the SLM1 run; only the requested output
                // FORMAT differs, because this transport requires it. Recorded
                // rather than smoothed over: the two tiers were not asked in
                // byte-identical words.
                Ok((v, _env)) => {
                    let raw = v["answer"].as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
                    (
                        match parse(&raw) {
                            Some(true) => "yes",
                            Some(false) => "no",
                            None => "unparseable",
                        },
                        raw.replace(['\n', '\t'], " ").chars().take(48).collect::<String>(),
                    )
                }
                // A 422 is the VALIDATOR refusing the payload — the proxy doing
                // its job. Recorded as excluded, never as a wrong answer.
                Err(e) => {
                    let msg = e.to_string();
                    let kind = if msg.contains("422") || msg.contains("validation") {
                        "excluded"
                    } else {
                        "error"
                    };
                    (kind, msg.replace(['\n', '\t'], " ").chars().take(120).collect::<String>())
                }
            } };
            println!(
                "{variant}\t{}\t{}\t{answer}\t{detail}",
                c["id"].as_str().unwrap_or("?"),
                if c["expected"]["finding"].as_bool().unwrap_or(false) { 1 } else { 0 },
            );
            if i % 25 == 0 {
                eprintln!("  {variant}: {i}/{}  {:?}", cases.len(), t0.elapsed());
            }
        }
        eprintln!("variant {variant} done in {:?}", t0.elapsed());
    }
}
