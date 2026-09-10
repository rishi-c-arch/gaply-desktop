//! WHICH source a citation resolves to, or a fetch is for.
//!
//! # ONE definition, two crates, three consumers (§11 D133)
//!
//! A cited work reaches the audit by exactly one of two routes: it is in the
//! user's `citation_library`, or it was staged from the manuscript's own
//! reference list (§11 D132). Every part of the system that names a source has
//! to say which, and they must all say it the same way.
//!
//! This type began in the app crate as `oa_fetch::FetchSubject`, for the fetch
//! alone. `Resolution::Checkable` then needed the same distinction and lives
//! here in `gaply-core`, which cannot depend on the app crate — so the type
//! MOVED DOWN rather than being copied up. `FetchSubject` is now an alias for
//! it.
//!
//! §11 D129 is why that is a move: two definitions of "which source is this"
//! would drift exactly as two definitions of "what this reference list says"
//! did, and the wire shape would drift with them.
//!
//! # An ENUM, not a nullable pair
//!
//! `library_id: Option<String>` beside `staged_id: Option<i64>` permits neither
//! and both — two states that cannot occur and that every consumer would then
//! have to handle or, more likely, quietly mishandle. Exactly one is true.

use serde::{Deserialize, Serialize};

/// A reference to one source, by the route it came in through.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
// `rename_all` on an enum renames the VARIANTS; the fields inside them need
// `rename_all_fields`. Without it this emits snake_case to TypeScript — §11
// D103's exact defect, which §11 D103's own guard caught here once already.
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SourceRef {
    /// A work the user collected. Links through `citation_documents`, and the
    /// user curated its metadata.
    Citation { citation_id: String },
    /// A reference the audit staged from the manuscript. Links through
    /// `audit_staged_sources`, is scoped to one job, and NEVER gains a
    /// `citation_library` row as a side effect of being checked.
    Staged { staged_id: i64 },
}

impl SourceRef {
    /// A stable key for grouping and for map lookups, distinct across kinds.
    ///
    /// `staged:` prefixed rather than bare, so a staged id can never collide
    /// with a library id that happens to be numeric.
    pub fn key(&self) -> String {
        match self {
            Self::Citation { citation_id } => citation_id.clone(),
            Self::Staged { staged_id } => format!("staged:{staged_id}"),
        }
    }

    /// The library id, when this IS a library citation.
    ///
    /// Exists because several call sites genuinely only apply to the library —
    /// retraction state, DOI presence, the per-citation audit scope — and should
    /// say so by asking, rather than by assuming the field is always there.
    pub fn citation_id(&self) -> Option<&str> {
        match self {
            Self::Citation { citation_id } => Some(citation_id),
            Self::Staged { .. } => None,
        }
    }
}
