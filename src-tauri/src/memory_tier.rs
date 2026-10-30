//! **This computer's memory tier, and the methods' limits for it (Research
//! OS, Phase 8F-1, decisions 279 and 281).** The methods on the user's data
//! (the descriptive summary and the three t-tests) get limits set by this
//! computer's total memory, as decision 242 accepted from the 8A proposal
//! (§6 and §8).
//!
//! * [`total_memory_bytes`] reads total physical memory once per platform:
//!   macOS through `models::total_physical_ram_bytes` (`hw.memsize`), called
//!   as it is; Linux through `sysinfo(2)` (`totalram × mem_unit`); Windows
//!   through `GlobalMemoryStatusEx`'s `ullTotalPhys`.
//! * [`tier_for`] is pure: a reading in, a tier out. Under 12 GiB is the
//!   8 GB tier, 12 GiB to under 24 GiB the 16 GB tier, 24 GiB or more the
//!   32 GB tier. **An unknown reading, or a reading of 0, is the 8 GB tier
//!   with `memory_known` false**: the opposite of the AI check's "unknown
//!   means allow" (`models::fits_free_memory`), which does not change.
//! * [`MethodLimits::for_tier`] is decision 281's table: a tier's
//!   [`IngestLimits`] (built by `IngestLimits::for_tier`, since nothing
//!   outside that impl builds one) and its slot budget in cells.
//! * [`this_computer`] takes the reading once per process and keeps it.
//!
//! **Switched on in 8F-2 (decision 288).** The methods' open and the rerun
//! read through `IngestLimits::this_computer()`, and the Statistics engine's
//! slots take [`this_computer`]'s slot budget. The screens read the tier
//! through `stats_desc_limits`, which sends the tier's word and its numbers,
//! never the memory itself. Level 1, the Stats Verifier and the
//! supplementary files keep their own limits.
//!
//! The 16 and 32 GB tiers are inferred, not measured on such a machine
//! (8A §6): the release measurement is from an 8 GB Mac.

use std::sync::OnceLock;

use crate::supplementary::IngestLimits;

const GIB: u64 = 1024 * 1024 * 1024;

/// A computer's memory tier (decision 281).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTier {
    Gb8,
    Gb16,
    Gb32,
}

impl MemoryTier {
    /// The tier's word on the wire: `8gb`, `16gb` or `32gb`.
    pub fn word(self) -> &'static str {
        match self {
            MemoryTier::Gb8 => "8gb",
            MemoryTier::Gb16 => "16gb",
            MemoryTier::Gb32 => "32gb",
        }
    }
}

/// A tier, and whether it came from a reading of this computer's memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryReading {
    pub tier: MemoryTier,
    /// `false` when total memory could not be read, or read as 0: the tier
    /// is then 8 GB, Gaply's smallest limits.
    pub memory_known: bool,
}

/// The tier for a total-memory reading, in bytes. Pure, so every boundary
/// is tested with injected values.
pub fn tier_for(total_bytes: Option<u64>) -> MemoryReading {
    match total_bytes {
        None | Some(0) => MemoryReading {
            tier: MemoryTier::Gb8,
            memory_known: false,
        },
        Some(t) => MemoryReading {
            tier: if t < 12 * GIB {
                MemoryTier::Gb8
            } else if t < 24 * GIB {
                MemoryTier::Gb16
            } else {
                MemoryTier::Gb32
            },
            memory_known: true,
        },
    }
}

/// A tier's limits for the methods on the user's data (decision 281).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodLimits {
    /// What the ingest opens.
    pub ingest: IngestLimits,
    /// The cells the dataset slots may hold in all, the newest dataset
    /// included. It counts cells only: not the file bytes a slot keeps, nor
    /// a rerun's one-run slot. On 16 and 32 GB it is four full slots, so
    /// the slot count binds first.
    pub slot_budget_cells: u64,
}

impl MethodLimits {
    /// Decision 281's table.
    pub const fn for_tier(tier: MemoryTier) -> MethodLimits {
        MethodLimits {
            ingest: IngestLimits::for_tier(tier),
            slot_budget_cells: match tier {
                MemoryTier::Gb8 => 10_000_000,
                MemoryTier::Gb16 | MemoryTier::Gb32 => 51_200_000,
            },
        }
    }
}

/// This computer's tier, read once per process and kept. A reading that
/// fails is logged as a closed fact, never with the OS's error.
pub fn this_computer() -> MemoryReading {
    static READING: OnceLock<MemoryReading> = OnceLock::new();
    *READING.get_or_init(|| {
        let reading = tier_for(total_memory_bytes());
        if !reading.memory_known {
            tracing::info!("memory_tier: total memory could not be read");
        }
        reading
    })
}

/// Total physical memory in bytes, or `None` when it cannot be read.
pub fn total_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        crate::models::total_physical_ram_bytes()
    }
    #[cfg(target_os = "linux")]
    {
        // SAFETY: `sysinfo` is a plain C struct of integers, for which all
        // zeroes is a valid value, and `libc::sysinfo` writes only into the
        // struct it is given. A non-zero return means the call failed.
        let mut info: libc::sysinfo = unsafe { std::mem::zeroed() };
        let rc = unsafe { libc::sysinfo(&mut info) };
        if rc != 0 {
            return None;
        }
        #[allow(clippy::unnecessary_cast)]
        let total = (info.totalram as u64).checked_mul(u64::from(info.mem_unit));
        total
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        // SAFETY: MEMORYSTATUSEX is a plain C struct of integers, for which
        // all zeroes is a valid value; `dwLength` is set to its size, as the
        // call requires, and the call writes only into the struct it is
        // given. A zero return means the call failed.
        let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        if ok == 0 {
            return None;
        }
        Some(status.ullTotalPhys)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}
