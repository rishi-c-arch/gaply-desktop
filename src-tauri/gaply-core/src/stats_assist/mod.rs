//! **The Level 1 helper layer, offline: four task specs, their outbound
//! structs, consent, previews, validators and the web citation parser.**
//!
//! Phase 2, Step 5. A helper task may suggest a method or a binding, explain a
//! finished result, or find cited guidance on a method. The engine
//! ([`crate::stats_l1`]) computes every number and chooses every verdict;
//! nothing here may create, change or suppress a verdict, a computed value, a
//! binding confirmation, a variant decision or a reason.
//!
//! # Which of those guarantees hold by types, and which only by the guard
//!
//! `stats_assist` is in the same crate as `stats_l1`, so anything `pub` there
//! is reachable from here. Precisely:
//!
//! - **By types, for all of them:** no public function in this module returns
//!   a `stats_l1` decision type. The validators return only
//!   [`response::ClaimMethodSuggestion`], [`response::BindingSuggestion`],
//!   [`response::ModelText`] and [`citation::ContextEvidence`]; a suggested
//!   variant is a `&'static str` hint, never a `VariantChoice`.
//! - **`ConfirmedBinding`: by types, for construction.** Its fields are private
//!   and its only constructor, `binding::confirm`, is `pub(super)`. The one
//!   public route, `L1RunState::confirm`, needs an `L1RunState`, which nothing
//!   here takes; that route is closed only by the guard.
//! - **A token that opens the gate: by types.** Only a token held in the run
//!   state's private token store passes, and only `L1RunState::confirm` puts
//!   one there.
//! - **Kept out of this module ONLY by the guard's source scan**
//!   (`tests/stats_assist_is_offline_and_inert.rs`), because each is publicly
//!   constructible: `L1Verdict` (a `pub` enum; allowed here only in match
//!   arms), `ConfirmationToken::presented` (with `TokenStore` and
//!   `TokenRecord`), `VariantChoice` (`user_ticked` and `from_wording` are
//!   `pub` and can produce a stated variant) and `ComputedResult` (all fields
//!   `pub`, as is `compute`).
//!
//! Exactly four tasks ([`spec::HelperTask`]): `claim_method`,
//! `binding_suggestion`, `result_explanation`, `web_context`. No agent loop,
//! no function tools, no model-chosen tools, no code execution, no file access
//! for a model. `web_context` uses the hosted `web_search` tool only, forced
//! and filtered to the spec's domain list.
//!
//! **No transport and no HTTP here.** [`transport::HelperTransport`] is a
//! trait; Step 6 implements it in the app crate. Everything works with AI
//! switched off: the setting ([`consent::AiHelpSetting`]) is off by default,
//! and with it off every preview and every send is refused here, in Rust.
//!
//! **What may be serialised:** the four outbound structs and their row types
//! (in [`outbound`]), the closed codes they carry ([`codes`]), and the private
//! request body around them. Nothing else in this module derives `Serialize`.
//!
//! `stats_l1` never references this module; this module reads `stats_l1`'s
//! types to build inputs and to check replies against what was sent.

pub mod citation;
pub mod codes;
pub mod consent;
pub mod outbound;
pub mod query;
pub mod response;
pub mod session;
pub mod spec;
pub mod transport;
