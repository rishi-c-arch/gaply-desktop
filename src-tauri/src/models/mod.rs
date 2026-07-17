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
use std::sync::OnceLock;

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

/// The BUNDLED models dir (`<resource_dir>/models`), set once at app startup
/// from the Tauri handle. This is the LAST-resort source (Set 1) — a stranger's
/// fresh install has no `~/gaply-models`, so the app falls through to the 0.5B +
/// tokenizer packaged in the installer. gaply-core stays Tauri-free; the app
/// layer injects this path (like the `ReqwestFetcher` seam).
static BUNDLED_MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Record the bundled models dir. Call once from the Tauri `setup` with
/// `app.path().resource_dir()?.join("models")`. Idempotent (later calls no-op).
pub fn set_bundled_models_dir(dir: PathBuf) {
    let _ = BUNDLED_MODELS_DIR.set(dir);
}

fn bundled_models_dir() -> Option<&'static PathBuf> {
    BUNDLED_MODELS_DIR.get()
}

/// Resolve a model GGUF by PRECEDENCE: env var (wins, taken as-is so a dev
/// override always applies) → `~/gaply-models/<sub>/*.gguf` → bundled
/// `<resource_dir>/models/<sub>/*.gguf`. So Rishi's dev models win, and a
/// stranger falls through to the bundled file.
fn resolve_gguf(env_key: &str, sub: &str) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(env_key) {
        return Some(PathBuf::from(p));
    }
    if let Some(g) = home_dir().and_then(|h| first_gguf(&h.join("gaply-models").join(sub))) {
        return Some(g);
    }
    bundled_models_dir().and_then(|d| first_gguf(&d.join(sub)))
}

/// Resolve the shared Qwen2.5 `tokenizer.json`. The Qwen2.5 tokenizer is
/// IDENTICAL across every size (0.5B … 7B), so SLM-1 (7B) and SLM-1-MINI (1.5B)
/// use the same file. `GAPLY_SLM1_TOKENIZER` wins; otherwise the conventional
/// `~/gaply-models/slm1-adapter/tokenizer.json`.
fn slm1_tokenizer_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GAPLY_SLM1_TOKENIZER") {
        return Some(PathBuf::from(p));
    }
    // env → ~/gaply-models (dev) → bundled (stranger). Prefer a path that EXISTS;
    // otherwise return the home path so the honest "absent" message points there.
    let home = home_dir().map(|h| h.join("gaply-models").join("slm1-adapter").join("tokenizer.json"));
    if let Some(ref h) = home {
        if h.exists() {
            return home;
        }
    }
    if let Some(b) = bundled_models_dir().map(|d| d.join("slm1-adapter").join("tokenizer.json")) {
        if b.exists() {
            return Some(b);
        }
    }
    home
}

/// Resolve the SLM-1 (full 7B) GGUF + tokenizer paths. `GAPLY_SLM1_GGUF` wins
/// for the model; otherwise the conventional `~/gaply-models/slm1/*.gguf`. The
/// tokenizer is the shared Qwen2.5 one ([`slm1_tokenizer_path`]). Returns `None`
/// if paths can't be formed (no home dir, or no GGUF present).
fn slm1_paths() -> Option<(PathBuf, PathBuf)> {
    // The 7B is NOT bundled (Set 1 bundles only the 0.5B) — resolve_gguf's bundled
    // fallback simply finds nothing here until the on-demand downloader lands.
    let gguf = resolve_gguf("GAPLY_SLM1_GGUF", "slm1")?;
    Some((gguf, slm1_tokenizer_path()?))
}

/// Resolve the SLM-1-MINI (compact Qwen2.5-1.5B) GGUF + tokenizer paths.
/// `GAPLY_SLM1_MINI_GGUF` wins for the model; otherwise the conventional
/// `~/gaply-models/slm1-mini/*.gguf`. The tokenizer is the SAME shared Qwen2.5
/// one as SLM-1 ([`slm1_tokenizer_path`]) — no separate download.
fn slm1_mini_paths() -> Option<(PathBuf, PathBuf)> {
    // Not bundled in Set 1 (downloads on demand later); the bundled fallback in
    // resolve_gguf is a no-op until then.
    let gguf = resolve_gguf("GAPLY_SLM1_MINI_GGUF", "slm1-mini")?;
    Some((gguf, slm1_tokenizer_path()?))
}

/// Resolve the Stage-1 language-model GGUF + tokenizer paths. This is the small
/// real LM (a Qwen2.5-family model) that replaces the frequency proxy as the
/// Stage-1 perplexity SIGNAL — deliberately named generically so the model can
/// be upgraded without renaming. `GAPLY_STAGE1_LM_GGUF` wins; otherwise the
/// conventional `~/gaply-models/stage1-lm/*.gguf`. Shared Qwen2.5 tokenizer.
fn stage1_lm_paths() -> Option<(PathBuf, PathBuf)> {
    // BUNDLED in Set 1: env → ~/gaply-models/stage1-lm → the packaged 0.5B.
    let gguf = resolve_gguf("GAPLY_STAGE1_LM_GGUF", "stage1-lm")?;
    Some((gguf, slm1_tokenizer_path()?))
}

/// One-line startup log of how the Stage-1 LM resolves — the packaged proof that
/// the bundled model is reachable in a BUILT app (a resource that works in
/// `tauri dev` but silently isn't in the `.app` is the failure this guards).
/// Also a support diagnostic: a user's log shows whether models were found.
pub fn log_model_resolution() {
    match stage1_lm_paths() {
        Some((gguf, tok)) => {
            let source = if std::env::var_os("GAPLY_STAGE1_LM_GGUF").is_some() {
                "env"
            } else if bundled_models_dir().map_or(false, |d| gguf.starts_with(d)) {
                "bundled"
            } else {
                "home(~/gaply-models)"
            };
            tracing::info!(
                source,
                gguf = %gguf.display(),
                gguf_exists = gguf.exists(),
                tokenizer = %tok.display(),
                tokenizer_exists = tok.exists(),
                "model resolution: stage-1 LM"
            );
        }
        None => tracing::info!("model resolution: stage-1 LM — no path resolved (no home dir?)"),
    }
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

/// The Stage-1 language model (candle), or honestly `None`. This small real LM
/// (~0.5B) supplies the Stage-1 perplexity SIGNAL that replaces the frequency
/// proxy — the root-cause fix for the false negative on lexically-rich AI prose.
/// Generic display name so the model can be upgraded without renaming. Same
/// honesty contract as [`slm1_model`]: `Some` only when files are present AND
/// load. (Not wired into the flow yet — the smart-sample scoring lands in B2.)
pub fn stage1_lm_model() -> Option<Box<dyn PerplexityModel>> {
    match stage1_lm_paths() {
        Some((gguf, tokenizer)) if gguf.exists() && tokenizer.exists() => {
            match CandlePerplexityModel::from_paths_named(
                &gguf,
                &tokenizer,
                "stage-1 language model (on-device)",
            ) {
                Ok(m) => {
                    tracing::info!(gguf = %gguf.display(), "Stage-1 LM: loaded perplexity model");
                    Some(Box::new(m))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Stage-1 LM: model present but failed to load");
                    None
                }
            }
        }
        _ => {
            tracing::warn!(
                "Stage-1 LM: model files absent (set GAPLY_STAGE1_LM_GGUF or populate \
                 ~/gaply-models/stage1-lm)"
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

/// Free + reclaimable memory in bytes (the RAM courtesy check — the Chrome
/// lesson, mechanized). macOS PRIMARY: mach `host_statistics64` (VM_INFO64) →
/// (free + inactive + purgeable + speculative) pages × page size, i.e. the
/// working set the OS can hand a new model without swap-thrashing. On any mach
/// error, FALL BACK to `sysctlbyname("vm.page_free_count")` (FREE-ONLY —
/// under-counts, so it skips too eagerly; used only when the primary fails).
/// Non-macOS → `None` (the caller treats `None` as "allow", current behaviour).
pub fn free_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        // mach vm_statistics64 layout (mach/vm_statistics.h). natural_t = u32.
        #[repr(C)]
        #[derive(Default)]
        struct VmStatistics64 {
            free_count: u32,
            active_count: u32,
            inactive_count: u32,
            wire_count: u32,
            zero_fill_count: u64,
            reactivations: u64,
            pageins: u64,
            pageouts: u64,
            faults: u64,
            cow_faults: u64,
            lookups: u64,
            hits: u64,
            purges: u64,
            purgeable_count: u32,
            speculative_count: u32,
            decompressions: u64,
            compressions: u64,
            swapins: u64,
            swapouts: u64,
            compressor_page_count: u32,
            throttled_count: u32,
            external_page_count: u32,
            internal_page_count: u32,
            total_uncompressed_pages_in_compressor: u64,
        }
        const HOST_VM_INFO64: libc::c_int = 4;

        extern "C" {
            fn mach_host_self() -> libc::mach_port_t;
            fn host_statistics64(
                host: libc::mach_port_t,
                flavor: libc::c_int,
                info: *mut libc::c_void,
                count: *mut libc::mach_msg_type_number_t,
            ) -> libc::c_int;
        }

        let page = page_size_bytes();
        let mut vm = VmStatistics64::default();
        let mut count =
            (std::mem::size_of::<VmStatistics64>() / std::mem::size_of::<libc::integer_t>())
                as libc::mach_msg_type_number_t;
        // SAFETY: `vm` is a valid, correctly-sized out-param for HOST_VM_INFO64;
        // `count` holds its length in integer_t units; host_statistics64 writes at
        // most `count` units. Non-zero return = mach failure → fall back.
        let rc = unsafe {
            host_statistics64(
                mach_host_self(),
                HOST_VM_INFO64,
                &mut vm as *mut VmStatistics64 as *mut libc::c_void,
                &mut count,
            )
        };
        if rc == 0 {
            let reclaimable = vm.free_count as u64
                + vm.inactive_count as u64
                + vm.purgeable_count as u64
                + vm.speculative_count as u64;
            return Some(reclaimable * page);
        }
        // FALLBACK: free pages only (conservative — under-counts).
        let mut free_pages: u32 = 0;
        let mut size = std::mem::size_of::<u32>();
        let rc = unsafe {
            libc::sysctlbyname(
                b"vm.page_free_count\0".as_ptr() as *const libc::c_char,
                &mut free_pages as *mut u32 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if rc == 0 {
            Some(free_pages as u64 * page)
        } else {
            None
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// VM page size in bytes (macOS: `hw.pagesize`; default 16384 on Apple silicon).
#[cfg(target_os = "macos")]
fn page_size_bytes() -> u64 {
    let mut ps: u64 = 0;
    let mut size = std::mem::size_of::<u64>();
    let rc = unsafe {
        libc::sysctlbyname(
            b"hw.pagesize\0".as_ptr() as *const libc::c_char,
            &mut ps as *mut u64 as *mut libc::c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc == 0 && ps > 0 {
        ps
    } else {
        16384
    }
}

/// The RAM courtesy check: is there enough free+reclaimable memory to load a
/// model with `resident_bytes` working set (require ~1.5×, so activations +
/// the aarch64 repack cache fit without swap-thrashing)? `None` free (non-macOS
/// or query failure) → `true` (allow — unchanged behaviour).
pub fn enough_free_memory(resident_bytes: u64) -> bool {
    match free_memory_bytes() {
        Some(free) => free >= resident_bytes.saturating_mul(3) / 2,
        None => true,
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

/// True when the Stage-1 LM GGUF + tokenizer are both present.
pub fn stage1_lm_present() -> bool {
    stage1_lm_paths().map(|(g, t)| g.exists() && t.exists()).unwrap_or(false)
}

/// The Stage-1 LM's norms key (derived from its GGUF filename), for looking up
/// the per-model absolute human-academic norm. `None` when no GGUF resolves.
pub fn stage1_lm_model_id() -> Option<String> {
    let (gguf, _) = stage1_lm_paths()?;
    Some(gaply_core::stage1_norms::model_id_from_gguf(&gguf.to_string_lossy()))
}

/// The full 7B deep verifier's norms key. `None` when no GGUF resolves. (No 7B
/// norm is calibrated yet — the lookup simply misses, so the 7B verifies without
/// a perplexity-placement row until a higher-RAM measurement lands.)
pub fn slm1_model_id() -> Option<String> {
    let (gguf, _) = slm1_paths()?;
    Some(gaply_core::stage1_norms::model_id_from_gguf(&gguf.to_string_lossy()))
}

/// The compact 1.5B deep verifier's norms key (Set E), for placing its
/// re-scored passages against its OWN absolute human-academic norm.
pub fn slm1_mini_model_id() -> Option<String> {
    let (gguf, _) = slm1_mini_paths()?;
    Some(gaply_core::stage1_norms::model_id_from_gguf(&gguf.to_string_lossy()))
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
mod ram_courtesy_tests {
    use super::{enough_free_memory, free_memory_bytes, total_physical_ram_bytes};

    #[test]
    fn free_memory_is_plausible_and_gate_is_directional() {
        // 0 resident always fits; an impossible ask never does.
        assert!(enough_free_memory(0), "zero working set always fits");
        assert!(!enough_free_memory(u64::MAX / 2), "an impossible ask is refused");

        #[cfg(target_os = "macos")]
        {
            let free = free_memory_bytes().expect("macOS reports free memory");
            assert!(free > 0, "free memory is non-zero");
            if let Some(total) = total_physical_ram_bytes() {
                assert!(free <= total, "free ({free}) cannot exceed total ({total})");
            }
        }
    }
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

    /// The Stage-1 LM resolver honors `GAPLY_STAGE1_LM_GGUF`, shares the SLM-1
    /// tokenizer, and `stage1_lm_model` honestly returns `None` when absent —
    /// never a silent stand-in, never loads a real model in tests.
    #[test]
    fn stage1_lm_paths_honor_env_and_absent_model_is_none() {
        std::env::set_var("GAPLY_STAGE1_LM_GGUF", "/nonexistent/stage1.gguf");
        std::env::set_var("GAPLY_SLM1_TOKENIZER", "/nonexistent/tokenizer.json");

        let (gguf, tok) = super::stage1_lm_paths().expect("env overrides form a path pair");
        assert_eq!(gguf.to_str(), Some("/nonexistent/stage1.gguf"));
        assert_eq!(tok.to_str(), Some("/nonexistent/tokenizer.json"), "shares the SLM-1 tokenizer");
        assert!(super::stage1_lm_model().is_none(), "absent Stage-1 GGUF => None");
        assert!(!super::stage1_lm_present());

        std::env::remove_var("GAPLY_STAGE1_LM_GGUF");
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
