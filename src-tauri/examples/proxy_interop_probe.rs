//! Set-2b interop proof: the Rust ProxyReqwestClient against the REAL Python
//! gaply-proxy (run with a JSON-returning stub Claude). Proves the Rust-minted
//! App Check token is verified byte-for-byte by the Python verifier, the
//! round-trip returns the stub JSON, and it parses cleanly.
//!
//!   GAPLY_PROXY_URL=http://127.0.0.1:8091 \
//!     cargo run --release --example proxy_interop_probe
//!
//! Exit 0 = interop proven; non-zero = mismatch.

use app_lib::models::proxy_client::ProxyReqwestClient;
use gaply_core::app_check::{TokenSigner, DEFAULT_APP_ID};
use gaply_core::verify_agent::ProxyClient;

// Must match SHARED_KEY in proxy_interop_launcher.py.
const SHARED_KEY: &str = "interop-shared-key-0123456789abcdef";

fn main() {
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8091".into());
    let payload = serde_json::json!({
        "task": "citation_verification",
        "instruction": "Return JSON with a verdicts array.",
        "summary": { "citations": [] }
    });

    // 1) Correct shared key → token must be accepted, round-trip must parse.
    let client =
        ProxyReqwestClient::new(&url, TokenSigner::new(SHARED_KEY.as_bytes().to_vec(), DEFAULT_APP_ID))
            .expect("build client");
    println!("reachable (/health): {}", client.reachable());
    match client.verify(&payload) {
        Ok(v) => {
            println!("round-trip OK, parsed value: {v}");
            assert_eq!(v, serde_json::json!({ "verdicts": [] }), "unexpected parsed value");
            println!("PASS: Rust token accepted byte-for-byte by the Python verifier; JSON parsed.");
        }
        Err(e) => {
            eprintln!("FAIL: verify() errored with the correct key: {e}");
            std::process::exit(1);
        }
    }

    // 2) WRONG key → the proxy must reject (proves it really checks the token,
    //    not that it accepts anything). Expect a 401 → GaplyError::Internal.
    let bad = ProxyReqwestClient::new(
        &url,
        TokenSigner::new(b"a-completely-different-secret".to_vec(), DEFAULT_APP_ID),
    )
    .expect("build bad client");
    match bad.verify(&payload) {
        Err(e) => println!("PASS: wrong-key token rejected by the proxy: {e}"),
        Ok(v) => {
            eprintln!("FAIL: proxy accepted a wrong-key token: {v}");
            std::process::exit(1);
        }
    }

    println!("\nINTEROP PROVEN.");
}
