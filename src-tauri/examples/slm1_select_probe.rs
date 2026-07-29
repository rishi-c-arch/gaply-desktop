//! Which PerplexityModel does `perplexity_model()` pick on THIS machine — and
//! why? Prints the whole decision chain of the shared memory-safety gate, so
//! the tier choice can be verified without running the pipeline:
//!
//!   cargo run --release --example slm1_select_probe
//!   GAPLY_DISABLE_DEEP=1 cargo run --release --example slm1_select_probe
//!   GAPLY_FORCE_DEEP=full cargo run --release --example slm1_select_probe
//!
//! The gate is `models::plan_deep_load` — the STRUCTURAL tier (`deep_tier`:
//! total RAM, the force/disable overrides, which GGUFs exist) composed with the
//! PER-RUN RAM courtesy check. Both the AI Check flow and the PublishReady
//! pipeline's AI lane run it, so what this prints is what BOTH lanes will do.
//! Whatever the gate refuses, `perplexity_model()` answers with the interim
//! HeuristicModel — never a panic, never a silent stand-in.

use app_lib::models::{
    self, perplexity_model, plan_deep_load, FULL_7B_RESIDENT_BYTES, MINI_RESIDENT_BYTES,
};

fn gb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

fn main() {
    let total = models::total_physical_ram_bytes();
    let free = models::free_memory_bytes();
    let full_present = models::slm1_present();
    let mini_present = models::slm1_mini_present();
    let tier = models::deep_tier_from_env();
    let plan = plan_deep_load(tier, free, full_present, mini_present);

    let fmt = |b: Option<u64>| b.map_or("unknown".to_string(), |b| format!("{:.2} GB", gb(b)));
    println!("total RAM       : {}", fmt(total));
    println!("free+reclaimable: {}", fmt(free));
    println!(
        "  7B needs      : {:.2} GB free (1.5x {:.2} GB resident)",
        gb(FULL_7B_RESIDENT_BYTES) * 1.5,
        gb(FULL_7B_RESIDENT_BYTES)
    );
    println!(
        "  mini needs    : {:.2} GB free (1.5x {:.2} GB resident)",
        gb(MINI_RESIDENT_BYTES) * 1.5,
        gb(MINI_RESIDENT_BYTES)
    );
    println!("GGUFs on disk   : 7B={full_present}  mini={mini_present}");
    println!("structural tier : {tier:?}");
    println!("gate plan       : {plan:?}");

    let model = perplexity_model();
    println!("selected model  : {}", model.name());
}
