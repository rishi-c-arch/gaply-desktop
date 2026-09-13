//! **The premium tier's boundary type — `Tier` and `ConsentRecord`.**
//!
//! The teardown found consent enforced in seven screens via `localStorage`,
//! missing in an eighth, and nowhere in Rust. That is a preference, not a
//! boundary. This module is the boundary.
//!
//! # What this is for
//!
//! [`Tier::Premium`] holds a [`ConsentRecord`] by construction, so there is no
//! value of `Tier` that names the premium route without one. The premium
//! release gate ([`crate::release_gate::run_premium_gate`]) and, in Phase 1, the
//! premium proxy client take a `ConsentRecord` directly — a free-tier call
//! cannot reach the premium route because there is no constructor path from
//! [`Tier::Free`] to it. This is the type system doing what `localStorage`
//! cannot.
//!
//! # WHY THIS LIVES IN THE APP CRATE, NOT gaply-core
//!
//! `gaply-core` is declared *"portable, Tauri-free"* at `rust-version =
//! "1.77.2"`, with no network and no clock. A consent record is a fact about a
//! NETWORK boundary, and the design it came from specified `Uuid` and
//! `DateTime` fields — two dependencies `gaply-core` does not carry and should
//! not acquire for this.
//!
//! So it lives here, beside the proxy client and the release gate it exists to
//! constrain, and it uses the conventions this codebase already has:
//! `now_epoch()` seconds as `i64` and a `String` id, not `Uuid`/`DateTime`.
//!
//! # THE PROVIDER IS A CLASS, NOT A VENDOR — and that is forced, not a choice
//!
//! The design named `provider: Provider // OpenAI, named`. **The desktop cannot
//! honestly record that.** `gaply-proxy/app/main.py:146-152` selects the
//! provider SERVER-SIDE from `GAPLY_LLM_PROVIDER`, and `:262` states the
//! property deliberately: *"nothing is persisted. The desktop never knows which
//! one handled it."* A consent record signed on this machine naming "OpenAI"
//! would be a claim the machine has no way to check and no way to be told is
//! wrong.
//!
//! [`ProviderClass`] therefore names what the user is actually consenting to —
//! *a cloud LLM reached exclusively through gaply-proxy* — which is both true
//! and the thing that matters to them. Naming the vendor would require the proxy
//! to return it, which means giving up provider-blindness; that trade is
//! recorded in `docs/AI_ENGINE_PLAN.md` §11 D152 and is NOT taken here.

use serde::{Deserialize, Serialize};

/// Which parts of the researcher's work a consent covers. A bitset, because a
/// researcher may send the manuscript and keep the code local — that
/// combination is common and should be one click.
///
/// A newtype over `u8` rather than a `bitflags` dependency: six flags do not
/// justify a crate, and the operations needed are `covers` and `union`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConsentScope(u8);

impl ConsentScope {
    pub const NONE: Self = Self(0);
    pub const MANUSCRIPT: Self = Self(1 << 0);
    pub const ANALYSIS_METADATA: Self = Self(1 << 1);
    pub const ANALYSIS_CODE: Self = Self(1 << 2);
    pub const ANALYSIS_DATA: Self = Self(1 << 3);
    pub const EXTERNAL_EVIDENCE: Self = Self(1 << 4);
    pub const JOURNAL_RESEARCH: Self = Self(1 << 5);

    /// Every scope, for tests and for the "send everything" consent.
    pub const ALL: Self = Self(0b0011_1111);

    /// Does this consent cover `other` — every bit of it?
    ///
    /// Total, not partial: a consent covering `MANUSCRIPT` does not cover
    /// `MANUSCRIPT | ANALYSIS_CODE`.
    pub fn covers(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    /// Rebuild from a stored bitset. Bits outside [`ConsentScope::ALL`] are an
    /// unknown scope written by a NEWER version — rejected rather than masked
    /// off, because silently dropping a bit turns "they consented to something
    /// we no longer understand" into "they consented to less", which is the
    /// wrong direction to guess in.
    pub fn from_bits(bits: u8) -> Option<Self> {
        (bits & !Self::ALL.0 == 0).then_some(Self(bits))
    }
}

/// What the user is consenting to send to, at the granularity the desktop can
/// honestly record. See this module's header for why this is not a vendor name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderClass {
    /// A cloud LLM reached EXCLUSIVELY through gaply-proxy, which chooses the
    /// provider server-side and never tells the desktop which one answered.
    CloudLlmViaProxy,
}

/// One consent event. Append-only: one row per manuscript per consent event,
/// never deleted, never updated.
///
/// # Fields are private on purpose
///
/// A `ConsentRecord` must only ever come from a persisted row. Public fields
/// would make `ConsentRecord { .. }` a valid expression anywhere in the crate,
/// and the whole point is that the premium route cannot be reached by writing
/// a struct literal. [`ConsentRecord::from_persisted`] is the one production
/// constructor and Phase 1 wires it to the `consent_records` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentRecord {
    id: String,
    manuscript_id: String,
    consented_at: i64,
    scope: ConsentScope,
    provider_class: ProviderClass,
    /// Which consent text the user actually saw. If the text changes, existing
    /// records do not cover the new statement and the user is asked again.
    statement_version: u32,
}

impl ConsentRecord {
    /// **THE production constructor — call this ONLY when rehydrating a row
    /// that is already in `consent_records`.**
    ///
    /// Phase 1 makes this `pub(crate)` and puts the store in front of it. It is
    /// `pub` now only because the table does not exist yet, and that is the
    /// single thing about this module that is weaker than the design requires.
    pub fn from_persisted(
        id: impl Into<String>,
        manuscript_id: impl Into<String>,
        consented_at: i64,
        scope: ConsentScope,
        provider_class: ProviderClass,
        statement_version: u32,
    ) -> Self {
        Self {
            id: id.into(),
            manuscript_id: manuscript_id.into(),
            consented_at,
            scope,
            provider_class,
            statement_version,
        }
    }

    /// Test-only shorthand. Deliberately `#[cfg(test)]`: a convenience
    /// constructor visible to shipped code is a constructor path from nothing
    /// to `Tier::Premium`.
    #[cfg(test)]
    pub fn for_test(id: &str, manuscript_id: &str, scope: ConsentScope) -> Self {
        Self::from_persisted(id, manuscript_id, 0, scope, ProviderClass::CloudLlmViaProxy, 1)
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn manuscript_id(&self) -> &str {
        &self.manuscript_id
    }
    pub fn consented_at(&self) -> i64 {
        self.consented_at
    }
    pub fn scope(&self) -> ConsentScope {
        self.scope
    }
    pub fn provider_class(&self) -> ProviderClass {
        self.provider_class
    }
    pub fn statement_version(&self) -> u32 {
        self.statement_version
    }
}

/// Which route a run is on. `Premium` carries its consent by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tier {
    Free,
    Premium(ConsentRecord),
}

impl Tier {
    /// The consent behind this run, or `None` on the free route. The only way
    /// to get a `ConsentRecord` out of a `Tier`, and it is honestly optional —
    /// callers that need one must handle the free case rather than unwrap.
    pub fn consent(&self) -> Option<&ConsentRecord> {
        match self {
            Tier::Free => None,
            Tier::Premium(r) => Some(r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_is_total_not_partial() {
        let manuscript_only = ConsentScope::MANUSCRIPT;
        assert!(manuscript_only.covers(ConsentScope::MANUSCRIPT));
        assert!(manuscript_only.covers(ConsentScope::NONE));
        assert!(!manuscript_only.covers(ConsentScope::ANALYSIS_CODE));
        assert!(
            !manuscript_only.covers(ConsentScope::MANUSCRIPT.union(ConsentScope::ANALYSIS_CODE)),
            "a partial overlap must not read as covered"
        );
        assert!(ConsentScope::ALL.covers(ConsentScope::MANUSCRIPT.union(ConsentScope::ANALYSIS_DATA)));
    }

    /// An unknown bit is a scope a NEWER version wrote. Rejecting beats masking:
    /// masking turns "consented to something we do not understand" into
    /// "consented to less", and guessing downward is still guessing.
    #[test]
    fn an_unknown_scope_bit_is_rejected_not_masked() {
        assert_eq!(ConsentScope::from_bits(0b0000_0011), Some(ConsentScope(0b0000_0011)));
        assert_eq!(ConsentScope::from_bits(0b1000_0001), None, "bit 7 is not a scope we know");
        assert_eq!(ConsentScope::from_bits(ConsentScope::ALL.bits()), Some(ConsentScope::ALL));
    }

    #[test]
    fn the_free_tier_has_no_consent_to_offer() {
        assert!(Tier::Free.consent().is_none());
        let r = ConsentRecord::for_test("c1", "m1", ConsentScope::MANUSCRIPT);
        assert_eq!(Tier::Premium(r.clone()).consent(), Some(&r));
    }

    /// The provider is a CLASS. If a vendor name is ever added here, the proxy
    /// must first return which provider answered — see the module header and
    /// §11 D152. This asserts the decision so adding a variant is a deliberate
    /// edit against a recorded reason.
    #[test]
    fn the_provider_is_recorded_as_a_class_reached_through_the_proxy() {
        let r = ConsentRecord::for_test("c1", "m1", ConsentScope::MANUSCRIPT);
        assert_eq!(r.provider_class(), ProviderClass::CloudLlmViaProxy);
    }
}
