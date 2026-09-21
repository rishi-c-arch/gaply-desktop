//! Raw POST of /tmp/payload.json to the proxy, printing `result.text` VERBATIM.
//!
//! `verify_with_envelope` parses `result.text` as JSON and reports only
//! "proxy reply is not valid JSON" when it cannot — which is a fact about the
//! parse, not about what the model said. This prints the bytes, so a format
//! failure can be told apart from a judgment failure (§11 D196's distinction).
use gaply_core::app_check::{proxy_auth_header, TokenSigner, DEFAULT_APP_ID};

fn main() {
    let key = std::env::var("GRRB_APP_CHECK_KEY").expect("GRRB_APP_CHECK_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("/tmp/payload.json").expect("payload"))
            .expect("json");
    let signer = TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID);
    let (hname, token) = proxy_auth_header(&signer).expect("mint");
    let c = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .unwrap();
    let r = c.post(format!("{url}/verify")).header(hname, token).json(&payload).send().expect("send");
    println!("status: {}", r.status());
    let body = r.text().unwrap_or_default();
    println!("--- raw body ---\n{body}");
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(t) = v["result"]["text"].as_str() {
            println!("--- result.text VERBATIM ({} bytes) ---", t.len());
            println!("{t:?}");
            println!("--- model: {} stop_reason: {} ---",
                v["result"]["model"], v["result"]["stop_reason"]);
        }
    }
}
