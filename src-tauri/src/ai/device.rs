//! Which device the AI engine runs on, and the gate that keeps it safe (§11 D36).
//!
//! # Why a gate exists at all
//!
//! Phase 7's feasibility work (§11 D35) established two things that have to be
//! held together:
//!
//! 1. candle 0.11 implements every operation this workload needs on Metal.
//! 2. `MetalDevice::new` unconditionally constructs `MTLResidencySetDescriptor`,
//!    which is **macOS 15+**, and objc2's binding **panics** on a missing class.
//!
//! A `Result`-based fallback cannot catch a panic, so "try Metal and handle the
//! error" is not a safe design here. The probe must be **gated before it runs**.
//!
//! # The shape
//!
//! ```text
//!   force-CPU env?  ──yes──> CPU
//!         │no
//!   gate: is the class there?  ──no──> CPU  (the macOS 14 path — no panic possible)
//!         │yes
//!   probe inside catch_unwind  ──panic/Err──> CPU
//!         │Ok
//!        Metal
//! ```
//!
//! The gate is the correctness mechanism; `catch_unwind` is the belt to the
//! gate's braces, for a future Apple release that moves the goalposts again.
//! Neither alone is sufficient: the gate cannot know about failures it has not
//! been taught, and `catch_unwind` cannot be relied on as a control-flow tool.
//!
//! **CPU is the floor everywhere.** Metal is an acceleration, never a
//! requirement — an engine that refuses to start because the GPU is a version
//! behind is worse than a slow engine.

use candle_core::Device;

/// What the engine actually ended up running on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceKind {
    Metal,
    Cpu,
}

impl DeviceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            DeviceKind::Metal => "metal",
            DeviceKind::Cpu => "cpu",
        }
    }
}

/// The chosen device, plus the reason when it is not the fast one.
#[derive(Debug, Clone)]
pub struct Selected {
    pub device: Device,
    pub kind: DeviceKind,
    /// Present ONLY when Metal was not obtained. Surfaced so a slow install can
    /// be diagnosed without a rebuild — "it fell back and said why" beats "it
    /// was mysteriously slow".
    pub fallback_reason: Option<String>,
}

/// Force CPU regardless of platform. Exists so a CPU-vs-Metal comparison can be
/// run by ONE binary, which is the only way that comparison means anything.
pub const FORCE_CPU_ENV: &str = "GAPLY_FORCE_CPU";

fn force_cpu_requested() -> bool {
    std::env::var(FORCE_CPU_ENV).map(|v| v != "0" && !v.is_empty()).unwrap_or(false)
}

fn cpu(reason: impl Into<String>) -> Selected {
    Selected { device: Device::Cpu, kind: DeviceKind::Cpu, fallback_reason: Some(reason.into()) }
}

/// Is it SAFE to construct a Metal device on this machine?
///
/// Asks the ObjC runtime whether the class candle will construct actually
/// exists. `AnyClass::get` returns `Option` and cannot panic — that is the
/// entire reason this is a class lookup rather than a version parse: it tests
/// the precise thing that fails, and it keeps testing the right thing if Apple
/// changes which release ships the class.
#[cfg(target_os = "macos")]
pub fn metal_gate() -> Result<(), String> {
    use objc2::runtime::AnyClass;

    // Control first. If Metal's classes are not registered in this process at
    // all, the residency-set lookup below would return None for a completely
    // different reason, and the log line would blame the OS version wrongly.
    if AnyClass::get(c"MTLCaptureDescriptor").is_none() {
        return Err("Metal framework classes are not registered in this process".to_string());
    }
    if AnyClass::get(c"MTLResidencySetDescriptor").is_none() {
        return Err(
            "MTLResidencySetDescriptor is absent — candle 0.11's Metal device requires \
             macOS 15+ (§11 D35)"
                .to_string(),
        );
    }
    Ok(())
}

/// Non-macOS: Metal does not exist. Not a degradation, just a fact.
#[cfg(not(target_os = "macos"))]
pub fn metal_gate() -> Result<(), String> {
    Err("not a macOS target; Metal is unavailable".to_string())
}

/// Construct the Metal device. ONLY ever called after [`metal_gate`] passes.
#[cfg(target_os = "macos")]
fn default_probe() -> Result<Device, String> {
    Device::new_metal(0).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
fn default_probe() -> Result<Device, String> {
    Err("not a macOS target".to_string())
}

/// Select the best available device. Never fails.
pub fn select() -> Selected {
    select_with(metal_gate, default_probe)
}

/// Seam for testing. Both the gate and the probe are injected because the two
/// failure modes they defend against cannot both be reproduced on one machine.
pub fn select_with(
    gate: impl Fn() -> Result<(), String>,
    probe: impl Fn() -> Result<Device, String>,
) -> Selected {
    if force_cpu_requested() {
        tracing::info!("{FORCE_CPU_ENV} set — running on CPU by request");
        return cpu(format!("{FORCE_CPU_ENV} was set"));
    }

    // THE GATE. Nothing below this line runs unless the class exists.
    if let Err(reason) = gate() {
        // INFO, not WARN. On any machine below macOS 15 this is the CORRECT
        // outcome, not a degradation — D44: the device is a FACT, not a
        // warning, and a log that shouts about the expected state trains
        // people to ignore it. The two WARNs below stay warnings because they
        // describe Metal failing or panicking PAST the gate, which is not
        // expected anywhere.
        tracing::info!("Metal unavailable, running on CPU: {reason}");
        return cpu(reason);
    }

    // Belt to the gate's braces. A panic here is not expected — the gate just
    // said the class is there — but a panic escaping into a Tauri command would
    // take the process with it, and a slow engine beats a dead one.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(probe)) {
        Ok(Ok(device)) => {
            tracing::info!("AI engine device: metal");
            Selected { device, kind: DeviceKind::Metal, fallback_reason: None }
        }
        Ok(Err(e)) => {
            tracing::warn!("Metal device creation failed, running on CPU: {e}");
            cpu(e)
        }
        Err(_) => {
            tracing::warn!("Metal device creation PANICKED past the gate; running on CPU");
            cpu("Metal device creation panicked past the availability gate")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Does the ObjC runtime have the class candle will construct? Ground truth
    /// for the assertions below, asked the same way the gate asks it.
    #[cfg(target_os = "macos")]
    fn residency_class_present() -> bool {
        objc2::runtime::AnyClass::get(c"MTLResidencySetDescriptor").is_some()
    }

    /// THE gate test. Written to be correct on BOTH sides of the macOS 15
    /// boundary rather than hardcoding today's answer — a test that starts
    /// failing the day the machine is upgraded would be a landmine, and the
    /// property that matters is agreement with reality, not a fixed verdict.
    ///
    /// On this machine (macOS 14.5) the class is absent and the gate refuses;
    /// on macOS 15+ the class is present and the gate permits.
    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_gate_agrees_with_the_runtime() {
        let present = residency_class_present();
        let gate = metal_gate();
        assert_eq!(
            gate.is_ok(),
            present,
            "gate said {gate:?} while the class present={present} — the gate has drifted \
             from the thing it is meant to be testing"
        );
        // The SAFETY direction is the one that must never regress: class
        // missing => the probe must not be reached.
        if !present {
            let e = gate.expect_err("class is absent, so the gate must refuse");
            assert!(e.contains("MTLResidencySetDescriptor"), "unhelpful reason: {e}");
        }
    }

    /// The belt, proven against the real failure. Forcing the gate open on a
    /// machine whose class is missing makes `default_probe` genuinely panic —
    /// this asserts the caller still gets a working CPU engine instead of an
    /// unwind. On macOS 15+ the probe simply succeeds and this asserts the
    /// happy path; either way the caller does not panic.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_forced_open_gate_cannot_panic_the_caller() {
        // The probe is EXPECTED to panic here; keep the scary backtrace out of
        // the test log, then put the real hook back.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let selected = select_with(|| Ok(()), default_probe);
        std::panic::set_hook(previous);

        if residency_class_present() {
            assert_eq!(selected.kind, DeviceKind::Metal, "Metal is available and should be used");
        } else {
            assert_eq!(
                selected.kind,
                DeviceKind::Cpu,
                "a panicking probe must yield CPU, not an unwind"
            );
            assert!(matches!(selected.device, Device::Cpu));
            assert!(selected.fallback_reason.unwrap().contains("panicked"));
        }
    }

    /// A gate that refuses must never reach the probe. If it did, the panic
    /// would escape on macOS 14 — so the probe here is one that would fail the
    /// test loudly if it were ever called.
    #[test]
    fn a_closed_gate_never_reaches_the_probe() {
        let s = select_with(
            || Err("no metal here".to_string()),
            || panic!("the probe was called despite a closed gate"),
        );
        assert_eq!(s.kind, DeviceKind::Cpu);
        assert_eq!(s.fallback_reason.as_deref(), Some("no metal here"));
    }

    #[test]
    fn an_open_gate_and_a_working_probe_selects_metal() {
        let s = select_with(|| Ok(()), || Ok(Device::Cpu)); // Cpu stands in for a Metal device
        assert_eq!(s.kind, DeviceKind::Metal);
        assert!(s.fallback_reason.is_none(), "a clean selection must claim no degradation");
    }

    #[test]
    fn an_open_gate_and_a_failing_probe_falls_back() {
        let s = select_with(|| Ok(()), || Err("device creation refused".to_string()));
        assert_eq!(s.kind, DeviceKind::Cpu);
        assert_eq!(s.fallback_reason.as_deref(), Some("device creation refused"));
    }

    #[test]
    fn device_kind_strings_are_stable() {
        assert_eq!(DeviceKind::Metal.as_str(), "metal");
        assert_eq!(DeviceKind::Cpu.as_str(), "cpu");
    }
}
