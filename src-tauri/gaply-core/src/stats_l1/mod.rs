//! Level 1 of the Statistical Analysis Verifier: does a data file reproduce
//! each result a manuscript reports, under a binding the user has confirmed?
//!
//! Phase 2, Step 2 builds the ground the computation stands on, and no
//! computation: the dataset schema ([`dataset`]), the method catalogue
//! ([`catalogue`]), claim identity ([`claim`]), bindings and their
//! confirmation ([`binding`]), the confirmation token ([`token`]), a local
//! proposer ([`propose`]) and the run state that holds them all
//! ([`run_state`]).
//!
//! No data file is parsed here: the app crate's ingest reads the file and
//! hands this module a [`dataset::RawTable`] of strings, so there is one
//! parser. No model, no proxy, no network, no persistence, no clock: everything
//! lives in memory for the session and dies with the process. `stats_assist`
//! is never referenced.
//!
//! Level 0 ([`crate::stats_l0`]) is reused and never changed: a Level 1 claim
//! wraps a [`crate::stats_l0::result::ReportedResult`], a user-entered one goes
//! through [`crate::stats_l0::number::ReportedNumber`] like an extracted one,
//! and a sentence ordinal is computed here with the Phase 1 sentence splitter
//! rather than added to any Level 0 type.
//!
//! # What may be serialised
//!
//! Exactly one type in this module derives `Serialize`:
//! [`dataset::AliasedSchema`], the only form of the schema that may leave the
//! device (alias, type, has-missing, level bucket). Every other type here holds
//! cell values, headers, level labels or manuscript text and does not. The
//! guard `tests/stats_l1_is_llm_free_and_offline.rs` scans for both.

pub mod binding;
pub mod catalogue;
pub mod claim;
pub mod dataset;
pub mod propose;
pub mod run_state;
pub mod token;

/// The version of the Level 1 engine a confirmation token is bound to. Step 2
/// computes nothing; the first method implementation (Step 3) changes this
/// string, and every token issued under the old one dies with it.
pub const ENGINE_VERSION: &str = "stats_l1 engine 0; no computation (Phase 2, Step 2)";
