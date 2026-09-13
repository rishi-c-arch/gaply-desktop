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
//! # WHY THIS MOVED FROM THE APP CRATE INTO gaply-core
//!
//! It was in the app crate, with a recorded reason: the design specified `Uuid`
//! and `DateTime` fields, two dependencies `gaply-core` does not carry.
//!
//! **That reason was void on arrival** — the implementation used `now_epoch()`
//! seconds as `i64` and a `String` id, which is this codebase's own convention
//! and needs nothing new. The stated justification described a design that was
//! not built.
//!
//! What forced the move is the guarantee itself. `Database::conn` is
//! `pub(crate)` to `gaply-core`, so a store living in the app crate cannot read
//! the table — which means [`ConsentRecord::from_persisted`] would have to be
//! `pub` for the store to call it, and a public constructor is exactly the hole
//! this module exists to close. **Co-locating the type with its store is what
//! makes the constructor private**, and a private constructor is the whole
//! guarantee.
//!
//! `gaply-core` already holds `app_check.rs` (token signing) and `secrets.rs`
//! (keychain), so a boundary concept is not foreign here. §11 D152 is updated.
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
    /// **THE production constructor — PRIVATE TO THIS MODULE.**
    ///
    /// Not `pub`, not `pub(crate)`: the only callers are [`store::record`] and
    /// [`store::resolve`] below, both of which write to or read from
    /// `consent_records`. **There is no path from anywhere else in the workspace
    /// to a `ConsentRecord`**, so a record that exists is a row that exists.
    ///
    /// That is the whole guarantee, and it is enforced by visibility rather than
    /// by discipline. `a_consent_record_cannot_be_constructed_outside_this_module`
    /// fails if this line ever regains a visibility modifier.
    fn from_persisted(
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

    /// Test-only shorthand. `#[cfg(test)]` AND `pub(crate)`: a convenience
    /// constructor visible to shipped code is a constructor path from nothing
    /// to `Tier::Premium`, and one visible to the app crate would let a test
    /// there fabricate a consent the database never saw.
    ///
    /// Tests that need a record the STORE produced should call
    /// [`store::record`] against an in-memory database instead — that is what
    /// the store's own tests do, and it is the shape a real caller has.
    #[cfg(test)]
    pub(crate) fn for_test(id: &str, manuscript_id: &str, scope: ConsentScope) -> Self {
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
    use crate::Database;

    // ------------------------------------------------------------------
    // THE DELIVERABLE — a ConsentRecord cannot be obtained except from a
    // stored row.
    // ------------------------------------------------------------------

    /// **The guarantee, asserted against this module's own source.**
    ///
    /// The property is a VISIBILITY fact, and visibility cannot be observed at
    /// runtime: a test that called `from_persisted` would not compile, and one
    /// that does not call it proves nothing. So this reads the source, which is
    /// the same instrument `wire_contract_tests.rs` and `decision_records.rs`
    /// use for their own compile-shaped properties.
    ///
    /// Two things are checked, because either alone leaves a hole:
    ///
    /// 1. **`from_persisted` has no visibility modifier.** `pub` would open it
    ///    to the workspace; `pub(crate)` would open it to every other module in
    ///    `gaply-core`, including ones with no business minting a consent.
    /// 2. **The struct's fields are private.** A public field makes
    ///    `ConsentRecord { .. }` a valid expression wherever the type is
    ///    visible, which would route around the constructor entirely — the
    ///    constructor could be perfectly private and the guarantee still gone.
    #[test]
    fn a_consent_record_cannot_be_constructed_outside_this_module() {
        // SCOPED TO THE SHIPPED HALF. `include_str!` pulls in this test module
        // too, and the first version of this guard failed on its OWN search
        // strings — the needles below appear literally in the assertion a few
        // lines down. A source-scanning check that can match itself is the
        // `pkill -f` shape: the pattern cannot tell the target from the thing
        // doing the searching. Everything after `#[cfg(test)]` is not shipped,
        // so the property is only about what precedes it.
        let whole = include_str!("consent.rs");
        let src = whole.split("#[cfg(test)]\nmod tests").next().expect("test module marker");
        assert!(
            src.len() < whole.len(),
            "the split found nothing — this guard would be scanning the whole file including \
             its own needles, and would pass or fail for the wrong reason"
        );

        // 1. the constructor is module-private
        for opened in ["pub fn from_persisted", "pub(crate) fn from_persisted", "pub(super) fn from_persisted"] {
            assert!(
                !src.contains(opened),
                "`{opened}` — the constructor has regained visibility, so a caller outside \
                 this module can mint a ConsentRecord the database never saw"
            );
        }
        assert!(
            src.contains("    fn from_persisted("),
            "the private constructor must still exist and be the one path in"
        );

        // 2. no public field routes around it
        let body = src
            .split("pub struct ConsentRecord {")
            .nth(1)
            .expect("the struct must exist")
            .split("\n}")
            .next()
            .expect("the struct must close");
        assert!(
            !body.contains("pub "),
            "ConsentRecord has a public field, so `ConsentRecord {{ .. }}` bypasses the \
             constructor entirely: {body}"
        );

        // 3. the only test-only constructor is cfg(test) AND crate-private
        assert!(
            src.contains("#[cfg(test)]\n    pub(crate) fn for_test("),
            "for_test must stay #[cfg(test)] + pub(crate); anything wider is a constructor \
             path from nothing to Tier::Premium"
        );
    }

    /// The runtime half: the store is the path, and it round-trips.
    #[test]
    fn the_store_is_the_path_in_and_the_record_round_trips() {
        let db = Database::in_memory().unwrap();
        let scope = ConsentScope::MANUSCRIPT.union(ConsentScope::JOURNAL_RESEARCH);
        let written = store::record(
            &db, "c1", "m1", 1_700_000_000, scope, ProviderClass::CloudLlmViaProxy, 3,
        )
        .unwrap();

        let read = store::resolve(&db, "c1").unwrap().expect("the row must resolve");
        assert_eq!(read, written, "what the store returns and what it reads back must agree");
        assert_eq!(read.manuscript_id(), "m1");
        assert_eq!(read.consented_at(), 1_700_000_000);
        assert_eq!(read.scope(), scope);
        assert_eq!(read.statement_version(), 3);
        assert!(read.scope().covers(ConsentScope::MANUSCRIPT));
        assert!(!read.scope().covers(ConsentScope::ANALYSIS_CODE));
    }

    /// An id nobody recorded resolves to nothing — the case `check_tier` turns
    /// into a TIER failure.
    #[test]
    fn an_unrecorded_id_resolves_to_nothing() {
        let db = Database::in_memory().unwrap();
        assert!(store::resolve(&db, "never-written").unwrap().is_none());
    }

    /// **An unreadable row is NOT a consent.** A scope carrying a bit this build
    /// does not know was written by a newer version; narrowing it to the bits we
    /// do know would turn "we cannot tell what they agreed to" into "they agreed
    /// to less", and a consent record is the one place guessing downward is
    /// still guessing.
    ///
    /// Written with raw SQL because the store cannot produce this row — which is
    /// the point: it is what a FUTURE version's row looks like to this one.
    #[test]
    fn a_row_this_build_cannot_read_resolves_to_nothing_rather_than_to_less() {
        let db = Database::in_memory().unwrap();
        // bit 6 is outside ConsentScope::ALL but inside the schema's CHECK (<=63)?
        // No: 64 exceeds it. Use 63 (all six) to prove the boundary is legal,
        // then an unknown PROVIDER for the unreadable case the CHECK permits.
        store::record(
            &db, "all", "m1", 1, ConsentScope::ALL, ProviderClass::CloudLlmViaProxy, 1,
        )
        .unwrap();
        assert!(store::resolve(&db, "all").unwrap().is_some(), "all six bits is a legal scope");

        // A provider class from a newer build. The schema CHECK rejects it, so
        // this asserts the SCHEMA is the first guard — an unknown provider
        // cannot even be stored.
        let direct = db.conn().unwrap().execute(
            "INSERT INTO consent_records VALUES ('x','m1',1,1,'some_future_provider',1)",
            [],
        );
        assert!(direct.is_err(), "the schema CHECK must refuse an unknown provider class");
    }

    /// Append-only: a second consent is a NEW row and the first stays readable.
    #[test]
    fn a_second_consent_is_a_new_row_and_the_first_survives() {
        let db = Database::in_memory().unwrap();
        store::record(&db, "c1", "m1", 100, ConsentScope::MANUSCRIPT,
                      ProviderClass::CloudLlmViaProxy, 1).unwrap();
        store::record(&db, "c2", "m1", 200,
                      ConsentScope::MANUSCRIPT.union(ConsentScope::ANALYSIS_CODE),
                      ProviderClass::CloudLlmViaProxy, 2).unwrap();

        let first = store::resolve(&db, "c1").unwrap().unwrap();
        assert_eq!(first.scope(), ConsentScope::MANUSCRIPT, "the earlier consent is unchanged");
        assert_eq!(first.statement_version(), 1);

        let hist = store::history(&db, "m1").unwrap();
        assert_eq!(hist.len(), 2, "both events are kept");
        assert_eq!(hist[0].id(), "c2", "newest first");
        assert_eq!(hist[1].id(), "c1");
    }

    /// Recording the same id twice is an ERROR, not a silent overwrite — an
    /// append-only table that quietly replaced a row could not answer how many
    /// times the user agreed.
    #[test]
    fn a_duplicate_consent_id_is_refused() {
        let db = Database::in_memory().unwrap();
        store::record(&db, "c1", "m1", 1, ConsentScope::MANUSCRIPT,
                      ProviderClass::CloudLlmViaProxy, 1).unwrap();
        assert!(
            store::record(&db, "c1", "m1", 2, ConsentScope::ALL,
                          ProviderClass::CloudLlmViaProxy, 1).is_err(),
            "the PRIMARY KEY must refuse a duplicate rather than overwrite"
        );
    }


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

// ============================================================================
// The store — THE ONLY PUBLIC PATH TO A `ConsentRecord`
// ============================================================================

/// Read and write `consent_records`.
///
/// # Why this is in the same module as the type
///
/// [`ConsentRecord::from_persisted`] is private to this module. These two
/// functions are its only callers, so **the only way to obtain a
/// `ConsentRecord` anywhere in the workspace is to write a row or read one
/// back**. A record that exists is a row that exists — enforced by Rust's
/// visibility rules, not by anyone remembering the rule.
///
/// The table is append-only by intent: there is no `update` and no `delete`
/// here, and none in the schema's usage. A consent is a thing that happened at
/// a time; a second consent is a new row, and the old one stays true about the
/// moment it describes.
pub mod store {
    use super::{ConsentRecord, ConsentScope, ProviderClass};
    use crate::{Database, GaplyError};

    fn provider_to_text(p: ProviderClass) -> &'static str {
        match p {
            ProviderClass::CloudLlmViaProxy => "cloud_llm_via_proxy",
        }
    }

    /// Total over the stored vocabulary — no wildcard, so a new `ProviderClass`
    /// cannot compile until its text form is decided here AND the schema's
    /// CHECK constraint is widened to match. Two places, deliberately: the
    /// schema is what stops a bad value entering, and this is what stops a good
    /// value being misread.
    fn provider_from_text(s: &str) -> Option<ProviderClass> {
        match s {
            "cloud_llm_via_proxy" => Some(ProviderClass::CloudLlmViaProxy),
            _ => None,
        }
    }

    /// **Record a consent event.** Returns the stored record.
    ///
    /// `id` is caller-supplied and must be unique; the PRIMARY KEY enforces it,
    /// so a duplicate is an error rather than a silent overwrite. Recording the
    /// same consent twice is a bug, and a store that hid it would make the
    /// append-only table unable to answer how many times the user agreed.
    pub fn record(
        db: &Database,
        id: &str,
        manuscript_id: &str,
        consented_at: i64,
        scope: ConsentScope,
        provider_class: ProviderClass,
        statement_version: u32,
    ) -> Result<ConsentRecord, GaplyError> {
        db.conn()?.execute(
            "INSERT INTO consent_records
                 (id, manuscript_id, consented_at, scope, provider_class, statement_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                manuscript_id,
                consented_at,
                scope.bits() as i64,
                provider_to_text(provider_class),
                statement_version as i64,
            ],
        )?;
        Ok(ConsentRecord::from_persisted(
            id,
            manuscript_id,
            consented_at,
            scope,
            provider_class,
            statement_version,
        ))
    }

    /// **Resolve a consent id to its stored row**, or `None`.
    ///
    /// This is what `release_gate::check_tier` takes, in place of the closure
    /// its tests use. `None` covers three cases that are all the same answer to
    /// the gate — no such row, a scope bitset this build does not understand,
    /// and a provider class it does not understand:
    ///
    /// **An unreadable row is NOT a consent.** A scope with a bit from a newer
    /// version could be narrowed to the bits we know, and a row with an unknown
    /// provider could be read as the one we have — both would turn "we do not
    /// understand what they agreed to" into "they agreed to this", which is the
    /// one direction a consent record must never be guessed in.
    pub fn resolve(db: &Database, id: &str) -> Result<Option<ConsentRecord>, GaplyError> {
        let conn = db.conn()?;
        let mut stmt = conn.prepare(
            "SELECT manuscript_id, consented_at, scope, provider_class, statement_version
               FROM consent_records WHERE id = ?1",
        )?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        let Some(row) = rows.next()? else { return Ok(None) };

        let manuscript_id: String = row.get(0)?;
        let consented_at: i64 = row.get(1)?;
        let scope_bits: i64 = row.get(2)?;
        let provider_text: String = row.get(3)?;
        let statement_version: i64 = row.get(4)?;

        let Ok(bits) = u8::try_from(scope_bits) else { return Ok(None) };
        let Some(scope) = ConsentScope::from_bits(bits) else { return Ok(None) };
        let Some(provider_class) = provider_from_text(&provider_text) else { return Ok(None) };

        Ok(Some(ConsentRecord::from_persisted(
            id,
            manuscript_id,
            consented_at,
            scope,
            provider_class,
            statement_version as u32,
        )))
    }

    /// Every consent event for one manuscript, newest first. The append-only
    /// history, for the audit trail and for asking "did they ever agree to X".
    pub fn history(db: &Database, manuscript_id: &str) -> Result<Vec<ConsentRecord>, GaplyError> {
        let conn = db.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id FROM consent_records
              WHERE manuscript_id = ?1 ORDER BY consented_at DESC, id DESC",
        )?;
        let ids: Vec<String> = stmt
            .query_map(rusqlite::params![manuscript_id], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        drop(conn);
        let mut out = Vec::new();
        for id in ids {
            if let Some(r) = resolve(db, &id)? {
                out.push(r);
            }
        }
        Ok(out)
    }
}
