//! Raw POST to the proxy with a real App Check token, printing status and body
//! verbatim. Diagnostic only: the typed client reported "proxy reply is not
//! valid JSON" and a parse error is not evidence about what the server sent.
use gaply_core::app_check::{proxy_auth_header, TokenSigner, DEFAULT_APP_ID};

fn main() {
    let key = std::env::var("GRRB_APP_CHECK_KEY").expect("GRRB_APP_CHECK_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let signer = TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID);
    let (hname, token) = proxy_auth_header(&signer).expect("mint");

    let payload = serde_json::json!({
        "summary": "The pH of the water samples was measured using a calibrated digital pH meter.",
        "instruction": "Reply with one word: yes or no.",
        "max_tokens": 8,
    });
    let c = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap();
    let r = c
        .post(format!("{url}/verify"))
        .header(hname, token)
        .json(&payload)
        .send()
        .expect("send");
    println!("status: {}", r.status());
    for (k, v) in r.headers().iter() {
        println!("  {k}: {}", v.to_str().unwrap_or("<bin>"));
    }
    println!("--- body ---\n{}", r.text().unwrap_or_default());
}
