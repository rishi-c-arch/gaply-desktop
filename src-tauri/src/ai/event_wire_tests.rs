//! The IPC event contract, pinned key by key.
//!
//! # Why this file exists
//!
//! Every streamed event crosses into TypeScript, where the reader is a hand-
//! written type. Twice now that pairing has drifted in silence, and both times
//! the failure was invisible rather than loud:
//!
//! * `EngineState`/`GenState` are `#[serde(tag = "state")]`; the panel read
//!   `.kind`, got `undefined`, and could never leave its NotInstalled branch —
//!   the install button sat there with both models on disk.
//! * `#[serde(rename_all = "camelCase")]` renames VARIANTS ONLY. Struct-variant
//!   FIELDS kept their snake_case, so `max_tokens` went out while the panel
//!   asked for `maxTokens` and rendered "552 of up to undefined tokens". The
//!   same slip zeroed the install progress bar (`totalBytes`) and made the
//!   queue indicator (`queuedBehind`) permanently invisible.
//!
//! A missing field does not throw in JavaScript; it renders as `undefined` or
//! quietly fails a comparison. So the contract is asserted here, on the exact
//! JSON, rather than trusted to a convention that has now broken twice.
//!
//! Adding a field to any of these enums SHOULD fail a test here. Update the
//! expectation and the TypeScript type in the same commit.
//!
//! # This file is NOT the whole guard (§11 D103)
//!
//! It was believed to be. The underscore rule below is general over FIELDS but
//! not over TYPES — it iterates the hand-written list of samples in
//! `no_streamed_event_field_carries_an_underscore`, so a type absent from that
//! list is checked by nothing. `FetchReport` was absent, drifted exactly as
//! described above, and shipped.
//!
//! Its scope is also narrower than the boundary: *streamed events*, when a
//! command's RETURN value crosses the identical boundary into the identical
//! kind of hand-written TypeScript reader.
//!
//! `crate::wire_contract_tests` covers the real boundary — every type a
//! `#[tauri::command]` returns or streams, enumerated from the signatures
//! themselves. Keep the exact-JSON pins here; they are the half that catches a
//! renamed tag or a dropped field, which no general rule can.

use serde_json::{json, Value};

fn keys(v: &Value) -> Vec<String> {
    v.as_object().expect("event serializes to an object").keys().cloned().collect()
}

#[test]
fn support_events_are_camel_case_on_the_wire() {
    use crate::commands::AiSupportEvent as E;

    assert_eq!(
        serde_json::to_value(E::Retrieved { chunks_sent: 6, chunks_dropped: 5 }).unwrap(),
        json!({ "kind": "retrieved", "chunksSent": 6, "chunksDropped": 5 })
    );
    assert_eq!(
        serde_json::to_value(E::Generating { queued_behind: 2 }).unwrap(),
        json!({ "kind": "generating", "queuedBehind": 2 })
    );
    assert_eq!(
        serde_json::to_value(E::Decoding { tokens: 552, max_tokens: 768 }).unwrap(),
        json!({ "kind": "decoding", "tokens": 552, "maxTokens": 768 })
    );
    assert_eq!(serde_json::to_value(E::Retrieving).unwrap(), json!({ "kind": "retrieving" }));
    assert_eq!(serde_json::to_value(E::Validating).unwrap(), json!({ "kind": "validating" }));
}

#[test]
fn install_events_are_camel_case_on_the_wire() {
    use crate::ai::model_install::InstallEvent as E;

    // totalBytes is what the progress bar divides by. As `total_bytes` it read
    // undefined, the denominator became 0, and the bar sat at 0% for a 133 MB
    // download while the download itself was fine.
    assert_eq!(
        serde_json::to_value(E::Started { files: 3, total_bytes: 133_466_304 }).unwrap(),
        json!({ "kind": "started", "files": 3, "totalBytes": 133_466_304u64 })
    );
    assert_eq!(
        serde_json::to_value(E::Done { model_id: "bge".into(), dir: "/m".into() }).unwrap(),
        json!({ "kind": "done", "modelId": "bge", "dir": "/m" })
    );
}

#[test]
fn generative_install_events_are_camel_case_on_the_wire() {
    use crate::ai::gen_install::GenInstallEvent as E;

    // fromBytes > 0 is the ONLY signal that an interrupted 2 GB download was
    // resumed rather than restarted, which is the whole reason resume exists.
    assert_eq!(
        serde_json::to_value(E::Downloading {
            file: "m.gguf".into(),
            from_bytes: 500,
            total_bytes: 1000
        })
        .unwrap(),
        json!({ "kind": "downloading", "file": "m.gguf", "fromBytes": 500, "totalBytes": 1000 })
    );
    assert_eq!(
        serde_json::to_value(E::Progress { file: "m.gguf".into(), bytes: 1, total_bytes: 2 })
            .unwrap(),
        json!({ "kind": "progress", "file": "m.gguf", "bytes": 1, "totalBytes": 2 })
    );
}

#[test]
fn no_streamed_event_field_carries_an_underscore() {
    // The general rule, so a NEW field cannot reintroduce the class without
    // failing here even if nobody adds a case above.
    use crate::ai::gen_install::GenInstallEvent as G;
    use crate::ai::model_install::InstallEvent as I;
    use crate::commands::AiSupportEvent as S;

    let samples: Vec<Value> = vec![
        serde_json::to_value(S::Retrieved { chunks_sent: 1, chunks_dropped: 1 }).unwrap(),
        serde_json::to_value(S::Generating { queued_behind: 1 }).unwrap(),
        serde_json::to_value(S::Decoding { tokens: 1, max_tokens: 1 }).unwrap(),
        serde_json::to_value(I::Started { files: 1, total_bytes: 1 }).unwrap(),
        serde_json::to_value(I::Done { model_id: "x".into(), dir: "y".into() }).unwrap(),
        serde_json::to_value(G::Downloading { file: "f".into(), from_bytes: 1, total_bytes: 2 })
            .unwrap(),
        serde_json::to_value(G::Started { model_id: "x".into(), files: 1, total_bytes: 1 })
            .unwrap(),
        serde_json::to_value(G::Done { model_id: "x".into(), dir: "y".into() }).unwrap(),
    ];
    for s in samples {
        for k in keys(&s) {
            assert!(!k.contains('_'), "field `{k}` reaches the webview in snake_case: {s}");
        }
    }
}
