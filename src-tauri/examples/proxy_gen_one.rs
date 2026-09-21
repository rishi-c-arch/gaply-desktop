//! One generation through the proxy, payload read from /tmp/payload.json.
//! Used to build the AI half of the SLM-1 detector set with a NAMED generator:
//! whatever `result.model` reports is recorded beside each paragraph.
use gaply_core::app_check::{TokenSigner, DEFAULT_APP_ID};
fn main() {
    let key = std::env::var("GRRB_APP_CHECK_KEY").expect("GRRB_APP_CHECK_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("/tmp/payload.json").expect("payload")).expect("json");
    let c = app_lib::models::proxy_client::ProxyReqwestClient::new(&url, TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID))
        .expect("client");
    match c.verify_with_envelope(&payload) {
        Ok((v, _)) => println!("{}", v),
        Err(e) => eprintln!("ERR {e}"),
    }
}
