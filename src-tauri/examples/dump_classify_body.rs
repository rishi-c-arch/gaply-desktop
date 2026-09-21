//! Dump the EXACT `/api/chat` body the shipped classify path sends, so a
//! diagnostic can replay it without re-typing the prompt (§11 D201).
fn main() {
    let text = std::fs::read_to_string(std::env::args().nth(1).expect("text file")).expect("read");
    let tag = std::env::args().nth(2).unwrap_or_else(|| "slm2-tuned".into());
    let payload = gaply_core::ai_detect::classify_payload(text.trim());
    let body = app_lib::models::ollama_verify::classify_chat_body_for_probe(&tag, &payload);
    println!("{}", serde_json::to_string_pretty(&body).expect("ser"));
}
