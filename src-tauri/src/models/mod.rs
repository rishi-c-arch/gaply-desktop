//! App-crate model runtimes.
//!
//! These implement gaply-core's model traits (e.g. `PerplexityModel`) using
//! real ML backends. They live in the APP CRATE — never in gaply-core — so the
//! portable core stays sync, network-free and ML-free (the `ReqwestFetcher`
//! precedent): `cargo test -p gaply_core` pulls none of candle/tokenizers, and
//! Windows CI's core-test job stays fast and green.

pub mod candle_perplexity;
pub mod quantized_qwen2_lowmem;
