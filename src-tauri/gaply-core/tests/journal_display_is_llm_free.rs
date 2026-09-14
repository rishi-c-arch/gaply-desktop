//! **No model may reach the journal layer's display path.**
//!
//! §7's whole claim about the fingerprint is *"Nothing on this screen is a
//! model's opinion"* (Prompt 5 item 10). That claim is worth exactly as much as
//! whatever enforces it, and in this repo the enforcement has to be structural:
//! §11 D128 records three modules that assert purity in a doc comment and have
//! held only because nobody tried.
//!
//! This is the SECOND guard of its kind. The first is
//! `equation_is_llm_free.rs`, and the difference is worth stating because the
//! two ban different things:
//!
//! - The equation guard bans the database, the filesystem and the clock too,
//!   because a Tier-0 arithmetic result must be reproducible from its inputs
//!   alone.
//! - **This one MUST allow the database** — reading stored rows is the entire
//!   job — and must allow the clock, because a provenance row carries
//!   `fetched_at`. What it bans is a MODEL, the PROXY, and the NETWORK: the
//!   three ways a journal fact could arrive as an opinion rather than as a
//!   stored sentence with a span.
//!
//! # What it cannot catch
//!
//! 1. **A model reached through a helper in another module.** The scan sees
//!    `crate::foo::bar()`, not what `bar` does.
//! 2. **A model on the FRONTEND side.** That is a separate structural test,
//!    `src/screens/publishready/network_invariant.vitest.ts`, and it has its
//!    own recorded blind spots. Neither test covers the other's half.
//! 3. **A fact written into the tables by a model-backed writer.** This guards
//!    the READ path. The write path's `extracted_by` column is the vocabulary
//!    that distinguishes `pattern` from `model`, and the display is required to
//!    show it — that is a rendering obligation, not something a scan enforces.

use std::path::Path;

const SCANNED: &[&str] = &["src/journal_fingerprint.rs"];

/// Routes to an opinion. The database, filesystem and clock are DELIBERATELY
/// absent — see the header.
const FORBIDDEN: &[(&str, &str)] = &[
    ("reqwest", "an HTTP client"),
    ("ureq", "an HTTP client"),
    ("std::net", "the network"),
    ("ProxyClient", "the cloud proxy"),
    ("proxy", "the cloud proxy"),
    ("PerplexityModel", "a model"),
    ("Embedder", "an embedding model"),
    ("embed(", "an embedding model"),
    ("llm", "a model"),
    ("Llm", "a model"),
    ("generative", "a model"),
    ("candle", "a model runtime"),
    ("tokenizers", "a model runtime"),
    ("ai_detect", "a model-backed module"),
    ("verify_agent", "the only cloud-calling agent"),
    ("reviewer_agent", "a model-backed module"),
    ("swarm", "the debate runtime — a consensus is an opinion"),
];

#[test]
fn the_fingerprint_read_path_reaches_no_model() {
    for rel in SCANNED {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} unreadable ({e}) — has the module moved?", path.display()));
        // Strip doc comments: this file's own header names the proxy and the
        // models it bans, and a scan that cannot tell prose from code would
        // make the guard unwritable.
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for (needle, what) in FORBIDDEN {
            assert!(
                !code.contains(needle),
                "{rel} reaches {what} (`{needle}`). §7: nothing on the fingerprint \
                 screen is a model's opinion — every line is a stored sentence."
            );
        }
    }
}

/// The scan must be looking at real code, or it passes on a moved file.
#[test]
fn the_guard_is_not_scanning_an_empty_string() {
    for rel in SCANNED {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        let src = std::fs::read_to_string(&path).unwrap();
        assert!(src.len() > 2000, "{rel} is {} bytes — too small to be the module", src.len());
        assert!(src.contains("pub fn fingerprint_for"), "{rel} no longer defines fingerprint_for");
    }
}
