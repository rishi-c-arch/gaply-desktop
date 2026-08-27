//! The spec's task prompts, one module each.
//!
//! Each implements `AiTask` and consumes the Phase 3 engine — `ModelManager`
//! for the lifecycle, `run_task` for validation plus one retry. A task owns its
//! prompt text, its schema, and its validator; it owns no lifecycle, no
//! retrying, and no persistence.
//!
//! Prompt text is VERBATIM from `docs/AI_ENGINE_SPEC.md`. The ARCHITECTURE
//! OVERRIDE at the head of that document covers runtime and decoding only.
//!
//! Implemented so far: Prompt 3 (citation_need). The other seven arrive in
//! later phases.

pub mod citation_need;
