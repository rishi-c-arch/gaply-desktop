//! Print the shipped and neutral envelopes side by side and diff them, so the
//! claim "the prompt is the only difference" is checked rather than asserted.
fn main() {
    let text = "Some passage text for the comparison.";
    let shipped = gaply_core::ai_detect::classify_payload(text);
    let mut neutral = shipped.clone();
    neutral["instruction"] = serde_json::json!("<NEUTRAL>");
    neutral["output_schema"]["properties"]["category"]["enum"] =
        serde_json::json!(["human", "ai_generated", "ai_paraphrased", "unclear"]);
    let a = shipped.as_object().unwrap();
    let b = neutral.as_object().unwrap();
    let mut differing: Vec<&str> = Vec::new();
    for (k, v) in a {
        if b.get(k) != Some(v) {
            differing.push(k);
        }
    }
    println!("top-level keys: {:?}", a.keys().collect::<Vec<_>>());
    println!("differing top-level keys: {differing:?}");
    println!("shipped category enum: {}", shipped["output_schema"]["properties"]["category"]["enum"]);
    println!("required fields unchanged: {}", a["output_schema"]["required"] == b["output_schema"]["required"]);
    println!("passage unchanged: {}", a["passage"] == b["passage"]);
    println!("task unchanged: {}", a["task"] == b["task"]);
    println!("strength enum unchanged: {}",
        a["output_schema"]["properties"]["strength"] == b["output_schema"]["properties"]["strength"]);
}
