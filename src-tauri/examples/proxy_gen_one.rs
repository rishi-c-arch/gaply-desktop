//! One generation through the proxy, payload read from /tmp/payload.json.
//! Used to build the AI half of the SLM-1 detector set (§11 D200) and the
//! PARAPHRASED third of the SLM-2 set (§11 D201) with a NAMED generator.
//!
//! It prints the envelope's `model` BESIDE the reply, because the generator's
//! name is provenance: a set whose AI half says "some cloud model" cannot be
//! re-derived. Earlier it printed the reply alone and the name was read out of
//! band; now the artefact and its attribution come out of the same call.
use gaply_core::app_check::{TokenSigner, DEFAULT_APP_ID};
fn main() {
    let key = std::env::var("GRRB_APP_CHECK_KEY").expect("GRRB_APP_CHECK_KEY");
    let url = std::env::var("GAPLY_PROXY_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let payload: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("/tmp/payload.json").expect("payload")).expect("json");
    let c = app_lib::models::proxy_client::ProxyReqwestClient::new(&url, TokenSigner::new(key.into_bytes(), DEFAULT_APP_ID))
        .expect("client");
    match c.verify_with_envelope(&payload) {
        Ok((v, meta)) => println!(
            "{}",
            serde_json::json!({
                "model": meta.model,
                "stop_reason": meta.stop_reason,
                "reply": v,
            })
        ),
        Err(e) => eprintln!("ERR {e}"),
    }
}
