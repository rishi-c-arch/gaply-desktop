//! App-crate model runtimes.
//!
//! These implement gaply-core's model traits (e.g. `PerplexityModel`) using
//! real ML backends. They live in the APP CRATE — never in gaply-core — so the
//! portable core stays sync, network-free and ML-free (the `ReqwestFetcher`
//! precedent): `cargo test -p gaply_core` pulls none of candle/tokenizers, and
//! Windows CI's core-test job stays fast and green.

pub mod candle_perplexity;
pub mod ollama_verify;
pub mod proxy_client;
pub mod quantized_qwen2_lowmem;

use std::path::{Path, PathBuf};

use gaply_core::ai_detect::{ClassifyClient, HeuristicModel, PerplexityModel};
use gaply_core::verify_agent::{MockProxyClient, ProxyClient};

use crate::models::candle_perplexity::CandlePerplexityModel;
use crate::models::ollama_verify::OllamaVerifyClient;
use crate::models::proxy_client::ProxyReqwestClient;

/// Default SLM-2 endpoint/model when the env overrides are unset.
const DEFAULT_SLM2_ENDPOINT: &str = "http://127.0.0.1:11434";
const DEFAULT_SLM2_MODEL: &str = "qwen3:4b";

/// Home dir for the conventional `~/gaply-models/...` layout the
/// `perplexity_probe` uses. Unix `HOME`, Windows `USERPROFILE`.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// First `*.gguf` in `dir`, if any. Lets the tier-appropriate model file
/// (Q3_K_M on 8GB, Q4_K_M on 16GB) be picked up without hardcoding a quant.
fn first_gguf(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("gguf"))
}

/// Resolve the shared Qwen2.5 `tokenizer.json`. The Qwen2.5 tokenizer is
/// IDENTICAL across every size (0.5B … 7B), so SLM-1 (7B) and SLM-1-MINI (1.5B)
/// use the same file. `GAPLY_SLM1_TOKENIZER` wins; otherwise the conventional
/// `~/gaply-models/slm1-adapter/tokenizer.json`.
fn slm1_tokenizer_path() -> Option<PathBuf> {
    match std::env::var_os("GAPLY_SLM1_TOKENIZER") {
        Some(p) => Some(PathBuf::from(p)),
        None => Some(
            home_dir()?
                .join("gaply-models")
                .join("slm1-adapter")
                .join("tokenizer.json"),
        ),
    }
}

/// Resolve the SLM-1 (full 7B) GGUF + tokenizer paths. `GAPLY_SLM1_GGUF` wins
/// for the model; otherwise the conventional `~/gaply-models/slm1/*.gguf`. The
/// tokenizer is the shared Qwen2.5 one ([`slm1_tokenizer_path`]). Returns `None`
/// if paths can't be formed (no home dir, or no GGUF present).
fn slm1_paths() -> Option<(PathBuf, PathBuf)> {
    let gguf = match std::env::var_os("GAPLY_SLM1_GGUF") {
        Some(p) => PathBuf::from(p),
        None => first_gguf(&home_dir()?.join("gaply-models").join("slm1"))?,
    };
    Some((gguf, slm1_tokenizer_path()?))
}

/// Resolve the SLM-1-MINI (compact Qwen2.5-1.5B) GGUF + tokenizer paths.
/// `GAPLY_SLM1_MINI_GGUF` wins for the model; otherwise the conventional
/// `~/gaply-models/slm1-mini/*.gguf`. The tokenizer is the SAME shared Qwen2.5
/// one as SLM-1 ([`slm1_tokenizer_path`]) — no separate download.
fn slm1_mini_paths() -> Option<(PathBuf, PathBuf)> {
    let gguf = match std::env::var_os("GAPLY_SLM1_MINI_GGUF") {
        Some(p) => PathBuf::from(p),
        None => first_gguf(&home_dir()?.join("gaply-models").join("slm1-mini"))?,
    };
    Some((gguf, slm1_tokenizer_path()?))
}

/// The REAL SLM-1 (candle), or honestly `None`. `Some` only when the GGUF +
/// tokenizer are present AND load — never a silent stand-in. The AI-Check
/// tiered flow needs this distinction: `analyze_tiered(deep: None)` labels
/// every flag heuristic-only, which is the truth when the model is absent
/// (a heuristic masquerading as the deep tier would fabricate
/// "deep-verified" labels).
pub fn slm1_model() -> Option<Box<dyn PerplexityModel>> {
    match slm1_paths() {
        Some((gguf, tokenizer)) if gguf.exists() && tokenizer.exists() => {
            match CandlePerplexityModel::from_paths(&gguf, &tokenizer) {
                Ok(m) => {
                    tracing::info!(gguf = %gguf.display(), "SLM-1: loaded candle perplexity model");
                    Some(Box::new(m))
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "SLM-1: candle model present but failed to load"
                    );
                    None
                }
            }
        }
        _ => {
            tracing::warn!(
                "SLM-1: model files absent (set GAPLY_SLM1_GGUF/GAPLY_SLM1_TOKENIZER or populate \
                 ~/gaply-models/slm1)"
            );
            None
        }
    }
}

/// The REAL SLM-1-MINI (candle Qwen2.5-1.5B), or honestly `None`. The COMPACT
/// deep-verifier for <16GB machines where the full 7B is gated off (proven-fatal
/// on 8GB). Same Qwen2 loader, same shared tokenizer as SLM-1; the only
/// difference is the display name — it carries the tier identity so the report
/// can say WHICH model verified (a 1.5B is a lighter verifier than the 7B).
/// `Some` only when the GGUF + tokenizer are present AND load — never a silent
/// stand-in (the same honesty contract as [`slm1_model`]).
pub fn slm1_mini_model() -> Option<Box<dyn PerplexityModel>> {
    match slm1_mini_paths() {
        Some((gguf, tokenizer)) if gguf.exists() && tokenizer.exists() => {
            match CandlePerplexityModel::from_paths_named(
                &gguf,
                &tokenizer,
                "Qwen2.5-1.5B (compact, on-device)",
            ) {
                Ok(m) => {
                    tracing::info!(gguf = %gguf.display(), "SLM-1-MINI: loaded compact candle perplexity model");
                    Some(Box::new(m))
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "SLM-1-MINI: compact model present but failed to load"
                    );
                    None
                }
            }
        }
        _ => {
            tracing::warn!(
                "SLM-1-MINI: model files absent (set GAPLY_SLM1_MINI_GGUF or populate \
                 ~/gaply-models/slm1-mini)"
            );
            None
        }
    }
}

/// Total physical RAM in bytes. macOS: `sysctlbyname("hw.memsize")` (via libc).
/// Other platforms: `None` — the RAM gate targets the proven-fatal 8GB *macOS*
/// case; on non-macOS the caller treats `None` as "allow" (current behaviour).
pub fn total_physical_ram_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let mut mem: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        // SAFETY: "hw.memsize\0" is a valid NUL-terminated C string; `mem`/`size`
        // are valid out-params sized for a u64, and sysctlbyname writes at most
        // `size` bytes. A non-zero return means the sysctl failed → treat as None.
        let rc = unsafe {
            libc::sysctlbyname(
                b"hw.memsize\0".as_ptr() as *const libc::c_char,
                &mut mem as *mut u64 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 && mem > 0 {
            Some(mem)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Minimum total RAM to run the SLM-1 7B deep pass: 15 GiB (framed to users as
/// "16 GB"). Below this the deep pass is proven-fatal on 8GB (a candle-CPU swap
/// death-spiral), so it is gated OFF and AI Check runs the fast heuristic
/// pre-pass only — honestly labelled (`analyze_tiered(deep: None)`).
const DEEP_PASS_MIN_RAM_BYTES: u64 = 15 * 1024 * 1024 * 1024;

/// Which deep-verifier tier AI Check runs for a given machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepTier {
    /// The full Qwen2.5-7B — only on ≥16GB machines (proven-fatal on 8GB) or via
    /// the EXPLICIT `GAPLY_FORCE_DEEP=full` escape hatch.
    Full7B,
    /// The compact Qwen2.5-1.5B — the real-model verifier for <16GB machines.
    Mini,
    /// No deep model — the honest fast pre-pass only.
    HeuristicOnly,
}

/// `GAPLY_FORCE_DEEP` override, parsed from the env value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForceTier {
    /// `=1` — bypass the RAM gate to force a deep pass, tier chosen by presence,
    /// but the low-RAM SAFETY INVARIANT still holds (never the 7B below 16GB).
    Any,
    /// `=mini` — force the compact tier (e.g. to test it on a high-RAM machine).
    Mini,
    /// `=full` — the ONLY way to run the 7B below 16GB (deliberate escape hatch).
    Full,
}

/// Pure tier decision (unit-tested — see `deep_tier_gate_tests`). Precedence,
/// highest first:
///   1. `disable`                       → HeuristicOnly (wins over everything)
///   2. `force=Full` & 7B present       → Full7B (the escape hatch, any RAM)
///   3. `force=Mini` & mini present     → Mini
///   4. `force=Any`                     → Mini if present, else Full7B ONLY when
///                                        high-RAM, else HeuristicOnly
///   5. high-RAM & 7B present           → Full7B
///   6. mini present                    → Mini
///   7. otherwise                       → HeuristicOnly
///
/// SAFETY INVARIANT: below 15 GiB, `Full7B` is UNREACHABLE except via rule 2
/// (`force=Full`). `high_ram` is `true` when `total >= 15 GiB` OR `total = None`
/// (non-macOS, where the 7B was never the fatal case).
pub fn deep_tier(
    total: Option<u64>,
    force: Option<ForceTier>,
    disable: bool,
    full_present: bool,
    mini_present: bool,
) -> DeepTier {
    if disable {
        return DeepTier::HeuristicOnly;
    }
    let high_ram = match total {
        Some(t) => t >= DEEP_PASS_MIN_RAM_BYTES,
        None => true,
    };
    match force {
        // Explicit full override — the ONLY path to the 7B below 16GB.
        Some(ForceTier::Full) if full_present => return DeepTier::Full7B,
        Some(ForceTier::Mini) if mini_present => return DeepTier::Mini,
        Some(ForceTier::Any) => {
            // Bypass the RAM gate to enable a deep pass, but keep the invariant:
            // the 7B is still only reachable on high-RAM.
            if mini_present {
                return DeepTier::Mini;
            }
            if full_present && high_ram {
                return DeepTier::Full7B;
            }
            return DeepTier::HeuristicOnly;
        }
        // A forced tier whose model is absent falls through to the RAM gate.
        _ => {}
    }
    if high_ram && full_present {
        return DeepTier::Full7B;
    }
    if mini_present {
        return DeepTier::Mini;
    }
    DeepTier::HeuristicOnly
}

/// True when SLM-1 (the full 7B) GGUF + tokenizer are both present on disk.
pub fn slm1_present() -> bool {
    slm1_paths().map(|(g, t)| g.exists() && t.exists()).unwrap_or(false)
}

/// True when SLM-1-MINI (the compact 1.5B) GGUF + tokenizer are both present.
pub fn slm1_mini_present() -> bool {
    slm1_mini_paths().map(|(g, t)| g.exists() && t.exists()).unwrap_or(false)
}

/// Parse `GAPLY_FORCE_DEEP`: `full` / `mini` / `1` → the matching override;
/// anything else (unset/other) → `None`.
fn force_deep_from_env() -> Option<ForceTier> {
    match std::env::var("GAPLY_FORCE_DEEP").ok().as_deref() {
        Some("full") => Some(ForceTier::Full),
        Some("mini") => Some(ForceTier::Mini),
        Some("1") => Some(ForceTier::Any),
        _ => None,
    }
}

/// `GAPLY_DISABLE_DEEP=1` → disable (wins over any force).
fn disable_deep_from_env() -> bool {
    std::env::var("GAPLY_DISABLE_DEEP").ok().as_deref() == Some("1")
}

/// Env + machine-driven tier decision used by the AI-Check flow: reads the total
/// RAM, the `GAPLY_FORCE_DEEP` / `GAPLY_DISABLE_DEEP` overrides, and which model
/// files are present, then applies [`deep_tier`].
pub fn deep_tier_from_env() -> DeepTier {
    deep_tier(
        total_physical_ram_bytes(),
        force_deep_from_env(),
        disable_deep_from_env(),
        slm1_present(),
        slm1_mini_present(),
    )
}

#[cfg(test)]
mod deep_tier_gate_tests {
    use super::{deep_tier, DeepTier, ForceTier};
    const GB: u64 = 1024 * 1024 * 1024;
    // Common machine shapes: (total, force, disable, full_present, mini_present).
    const NF: Option<ForceTier> = None;

    #[test]
    fn deep_tier_decision_table() {
        // --- normal RAM gate (no force, no disable) ---
        // 16GB, both present → the full 7B
        assert_eq!(deep_tier(Some(16 * GB), NF, false, true, true), DeepTier::Full7B);
        // 8GB, both present → the compact mini (7B gated off)
        assert_eq!(deep_tier(Some(8 * GB), NF, false, true, true), DeepTier::Mini);
        // 8GB, only mini present → Mini
        assert_eq!(deep_tier(Some(8 * GB), NF, false, false, true), DeepTier::Mini);
        // 16GB, only mini present → Mini (no 7B to run)
        assert_eq!(deep_tier(Some(16 * GB), NF, false, false, true), DeepTier::Mini);
        // nothing present → HeuristicOnly
        assert_eq!(deep_tier(Some(8 * GB), NF, false, false, false), DeepTier::HeuristicOnly);
        // non-macOS (None) is treated as high-RAM → 7B if present
        assert_eq!(deep_tier(None, NF, false, true, false), DeepTier::Full7B);

        // --- THE SAFETY INVARIANT: below 15GiB the 7B is UNREACHABLE without ---
        // --- an explicit GAPLY_FORCE_DEEP=full. This is the fatal-case guard.  ---
        assert_eq!(
            deep_tier(Some(8 * GB), NF, false, /*full*/ true, /*mini*/ false),
            DeepTier::HeuristicOnly,
            "8GB + only 7B present + no force → NEVER Full7B (proven-fatal); heuristic-only"
        );

        // --- disable wins over everything ---
        assert_eq!(deep_tier(Some(32 * GB), NF, true, true, true), DeepTier::HeuristicOnly);
        assert_eq!(
            deep_tier(Some(32 * GB), Some(ForceTier::Full), true, true, true),
            DeepTier::HeuristicOnly,
            "disable beats force=full"
        );

        // --- force=full: the ONLY way to the 7B below 16GB ---
        assert_eq!(
            deep_tier(Some(8 * GB), Some(ForceTier::Full), false, true, false),
            DeepTier::Full7B,
            "explicit force=full runs the 7B even on 8GB"
        );
        // force=full but 7B absent → falls through to the RAM gate (mini here)
        assert_eq!(deep_tier(Some(8 * GB), Some(ForceTier::Full), false, false, true), DeepTier::Mini);

        // --- force=mini: force the compact tier even on a high-RAM machine ---
        assert_eq!(deep_tier(Some(32 * GB), Some(ForceTier::Mini), false, true, true), DeepTier::Mini);
        // force=mini but mini absent → RAM gate (7B on high-RAM)
        assert_eq!(deep_tier(Some(32 * GB), Some(ForceTier::Mini), false, true, false), DeepTier::Full7B);

        // --- force=Any (=1): bypass RAM gate, invariant-safe ---
        // 8GB + mini present → Mini (forced deep, safe tier)
        assert_eq!(deep_tier(Some(8 * GB), Some(ForceTier::Any), false, true, true), DeepTier::Mini);
        // 8GB + only 7B present → still NOT the 7B (invariant) → HeuristicOnly
        assert_eq!(
            deep_tier(Some(8 * GB), Some(ForceTier::Any), false, true, false),
            DeepTier::HeuristicOnly,
            "force=1 never forces the 7B onto low-RAM"
        );
        // 32GB + only 7B present → Full7B (high-RAM, so allowed)
        assert_eq!(deep_tier(Some(32 * GB), Some(ForceTier::Any), false, true, false), DeepTier::Full7B);
    }
}

#[cfg(test)]
mod mini_loader_tests {
    use super::{slm1_mini_model, slm1_mini_paths};

    /// The mini resolver honors `GAPLY_SLM1_MINI_GGUF` for the model and shares
    /// the SLM-1 tokenizer (`GAPLY_SLM1_TOKENIZER`), and `slm1_mini_model`
    /// honestly returns `None` when the GGUF path doesn't exist — never a silent
    /// stand-in, and (critically) never loads the real ~945MB model in tests.
    ///
    /// One test, done sequentially, so it doesn't race itself on the shared
    /// process env. Mirrors the existing env-in-tests pattern (aicheck.rs).
    #[test]
    fn mini_paths_honor_env_and_absent_model_is_none() {
        // SAFETY: single-threaded within this test; set → assert → restore.
        std::env::set_var("GAPLY_SLM1_MINI_GGUF", "/nonexistent/mini.gguf");
        std::env::set_var("GAPLY_SLM1_TOKENIZER", "/nonexistent/tokenizer.json");

        let (gguf, tok) = slm1_mini_paths().expect("env overrides form a path pair");
        assert_eq!(gguf.to_str(), Some("/nonexistent/mini.gguf"));
        assert_eq!(tok.to_str(), Some("/nonexistent/tokenizer.json"), "shares the SLM-1 tokenizer");

        // Files don't exist → honestly None (no panic, no real-model load).
        assert!(slm1_mini_model().is_none(), "absent mini GGUF => None");

        std::env::remove_var("GAPLY_SLM1_MINI_GGUF");
        std::env::remove_var("GAPLY_SLM1_TOKENIZER");
    }
}

/// The SLM-1 perplexity model for the AI-detection lane: the real candle
/// `CandlePerplexityModel` when its GGUF + tokenizer are present and load,
/// otherwise the interim [`HeuristicModel`]. NEVER fails — a user who hasn't
/// downloaded the model must still get a working pipeline (both types implement
/// [`PerplexityModel`], so the choice is a clean either/or at the trait
/// boundary). Callers get a boxed trait object and don't know which ran.
pub fn perplexity_model() -> Box<dyn PerplexityModel> {
    slm1_model().unwrap_or_else(|| {
        tracing::warn!("SLM-1 unavailable; using interim HeuristicModel");
        Box::new(HeuristicModel::gpt2_like())
    })
}

/// The AI-Check classification client (SLM-2 over Ollama), or honestly `None`
/// when Ollama isn't reachable. LOCAL-ONLY by design: AI Check is free and
/// never touches the cloud proxy, so unlike [`verify_proxy`] there is
/// deliberately no cloud tier here.
///
/// NOTE (Set-5 decision): the SHIPPED AI Check flow does not call this — the
/// Set-4 live probe showed qwen3:4b cannot make the generated-vs-paraphrased
/// distinction reliably at usable speed, so `run_aicheck_flow` passes `None`
/// unconditionally (the honest two-way collapse). This resolver stays for
/// probes and for a future local model that can make the distinction.
pub fn aicheck_classifier() -> Option<Box<dyn ClassifyClient>> {
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(client) if client.reachable() => {
            tracing::info!(%endpoint, %model, "SLM-2: routing AI-Check classification to local Ollama");
            Some(Box::new(client))
        }
        _ => {
            tracing::warn!(
                %endpoint,
                "SLM-2: Ollama unreachable; AI-Check paraphrase distinction unavailable (two-way fallback)"
            );
            None
        }
    }
}

/// Resolve the SLM-2 endpoint + model tag: `GAPLY_SLM2_ENDPOINT` /
/// `GAPLY_SLM2_MODEL` win, else localhost defaults.
fn slm2_endpoint_model() -> (String, String) {
    let endpoint = std::env::var("GAPLY_SLM2_ENDPOINT")
        .unwrap_or_else(|_| DEFAULT_SLM2_ENDPOINT.to_string());
    let model =
        std::env::var("GAPLY_SLM2_MODEL").unwrap_or_else(|_| DEFAULT_SLM2_MODEL.to_string());
    (endpoint, model)
}

/// Human-readable label of which verification backend [`verify_proxy`] would
/// pick right now (`ollama …` when reachable, `mock …` when not) — for probes
/// and logging; runs the SAME reachability decision as `verify_proxy`.
pub fn verify_backend_label() -> String {
    // Mirror verify_proxy's tier order: cloud → ollama → mock.
    if let Ok(client) = ProxyReqwestClient::from_env() {
        if client.reachable() {
            return "cloud proxy".to_string();
        }
    }
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(c) if c.reachable() => format!("ollama ({endpoint}, {model})"),
        _ => format!("mock (no cloud proxy; Ollama unreachable at {endpoint})"),
    }
}

/// Explicitly unload the SLM-2 model from Ollama and verify it's gone, after
/// the verification stage. The 8GB OOM guard: guarantees qwen3:4b is out of
/// memory before a subsequent analysis loads SLM-1 (candle) — belt-and-
/// suspenders over the request's `keep_alive: 0`. No-op/harmless when Ollama
/// isn't running or the model was never loaded.
pub fn unload_slm2() {
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(client) if client.unload() => {
            tracing::info!(%endpoint, %model, "SLM-2: model unloaded and confirmed out of memory");
        }
        Ok(_) => {
            tracing::warn!(
                %endpoint, %model,
                "SLM-2: unload requested but model still appears resident (or Ollama unreachable)"
            );
        }
        Err(e) => tracing::warn!(error = %e, "SLM-2: unload client build failed"),
    }
}

/// The verification [`ProxyClient`] for the pipeline's verify stage: the real
/// [`OllamaVerifyClient`] when Ollama is reachable, otherwise gaply-core's
/// [`MockProxyClient`] returning empty verdicts (→ every citation UNKNOWN).
/// NEVER makes the pipeline fail for a user without Ollama running — the mirror
/// of [`perplexity_model`]'s missing-model fallback.
pub fn verify_proxy() -> Box<dyn ProxyClient> {
    // Tier 1 — cloud proxy (deep reasoning). Selected ONLY when the App Check
    // signing key is provisioned (from_env → keychain; Err = skip) AND the
    // proxy answers /health. Otherwise fall through to local, cleanly.
    match ProxyReqwestClient::from_env() {
        Ok(client) if client.reachable() => {
            tracing::info!("verification: routing to cloud proxy");
            return Box::new(client);
        }
        Ok(_) => tracing::debug!("cloud proxy configured but unreachable; falling through to local"),
        Err(_) => tracing::debug!("cloud proxy signing key absent; using local verification"),
    }

    // Tier 2 — local Ollama SLM-2.
    let (endpoint, model) = slm2_endpoint_model();
    match OllamaVerifyClient::with_endpoint(&endpoint, &model) {
        Ok(client) if client.reachable() => {
            tracing::info!(%endpoint, %model, "SLM-2: routing verification to local Ollama");
            Box::new(client)
        }
        // Tier 3 — mock: honest empty verdicts (every citation UNKNOWN).
        _ => {
            tracing::warn!(
                %endpoint,
                "SLM-2: Ollama unreachable; verification falls back to UNKNOWN verdicts"
            );
            Box::new(MockProxyClient::returning(serde_json::json!({ "verdicts": [] })))
        }
    }
}
