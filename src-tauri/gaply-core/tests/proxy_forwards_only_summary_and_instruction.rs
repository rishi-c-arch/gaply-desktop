//! **The Rust side believes the proxy forwards exactly `summary` and
//! `instruction`. This is what checks that belief.**
//!
//! `reviewer_agent::build_review_payload` nests every model-visible field under
//! `summary` and says why in a comment. That comment is a claim about a Python
//! file in a different directory, reached over HTTP, that no Rust test compiles
//! — the jurisdiction problem CLAUDE.md records: a statement true of the file it
//! sits in, asserting a fact about a system.
//!
//! # The defect this exists for
//!
//! §11 D205 measured "the same 160 paragraphs WITH their document context" and
//! concluded that position was worth +7.5 and +11.2 points. The context was sent
//! as top-level siblings of `summary`. The proxy dropped them. The model never
//! saw a heading or a neighbour, the entry's central claim was false, and
//! **nothing anywhere failed**:
//!
//! * the validator walks every string leaf, so it saw all four fields, applied
//!   its limits, and refused 16 of 160 — a real, correct, reassuring number;
//! * the provider returned 200;
//! * the answers changed, because the INSTRUCTION had also changed;
//! * and the change was position-shaped, because the paragraphs a more
//!   conservative model stops accepting are disproportionately the ones outside
//!   Methods.
//!
//! Every instrument agreed and every one of them was answering a different
//! question from the one being asked.
//!
//! # What this test can and cannot see
//!
//! It is a SOURCE SCAN of the two provider clients: it reads the payload keys
//! each one projects into the user message. It does not execute Python and does
//! not run the proxy, so a change in `main.py`'s routing, or a new provider
//! client added beside these two, is invisible to it. **Nothing in CI runs the
//! proxy's own tests** — none of the five workflows invokes `pytest` (checked
//! 22 Sep 2026) — so this scan is the only automatic check on the contract, and
//! it checks the SHAPE of the projection, not the behaviour of the server.
//!
//! Its sibling is `reviewer_agent`'s
//! `everything_the_model_must_read_survives_the_proxys_summary_instruction_projection`,
//! which applies the projection to a real payload. Two halves of one rule: this
//! one pins what the proxy forwards, that one pins that the payload survives it.

use std::path::PathBuf;

/// The keys the Rust side is entitled to assume reach the model. Sorted, because
/// the scan compares sorted sets — the ORDER a client happens to write them in is
/// not part of the contract.
const FORWARDED: [&str; 2] = ["instruction", "summary"];

fn proxy_client_src(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gaply-proxy/app").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| {
        panic!(
            "cannot read {} — the reviewer payload's shape is built around what this file \
             forwards: {e}",
            p.display()
        )
    })
}

/// Pull the `payload.get("...")` keys out of the `json.dumps({...})` that builds
/// the user message. Keyed on `payload.get(` rather than on quoted strings, so a
/// docstring mentioning "summary" cannot satisfy it.
fn forwarded_keys(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("payload.get(") {
        rest = &rest[i + "payload.get(".len()..];
        let quote = match rest.chars().next() {
            Some(c @ ('"' | '\'')) => c,
            _ => continue,
        };
        if let Some(end) = rest[1..].find(quote) {
            out.push(rest[1..=end].to_string());
        }
    }
    out
}

#[test]
fn both_provider_clients_forward_exactly_summary_and_instruction() {
    let mut checked = 0usize;
    for name in ["openai_client.py", "claude_client.py"] {
        let src = proxy_client_src(name);
        let keys = forwarded_keys(&src);

        // POSITIVE count first: a scan that matches nothing would otherwise
        // satisfy every assertion below forever.
        assert!(
            !keys.is_empty(),
            "{name}: found no `payload.get(...)` calls at all — this scan has stopped \
             matching how the proxy is written, and is now silently inert"
        );

        let mut model_visible: Vec<String> =
            keys.iter().filter(|k| k.as_str() != "max_tokens").cloned().collect();
        model_visible.sort();
        model_visible.dedup();
        assert_eq!(
            model_visible,
            FORWARDED.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "{name} forwards {model_visible:?} to the model, not {FORWARDED:?}.\n\
             If a key was ADDED, the Rust payload builders may now nest less than they must \
             — and the reviewer's own comment (`reviewer_agent.rs`, 'Everything the model \
             must see lives under `summary`') is out of date.\n\
             If a key was REMOVED, something the model is instructed to reason over is no \
             longer reaching it, and every cloud number in §11 is about a different input \
             than it claims. See §11 D205, which was invalidated by exactly this gap."
        );
        checked += 1;
    }
    assert_eq!(checked, 2, "both provider clients must be scanned");
}
