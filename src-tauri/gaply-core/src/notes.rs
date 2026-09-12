//! Note Creator — the free, LOCAL-FIRST notes store (Set 2).
//!
//! Local sqlite is the SOURCE OF TRUTH: create/edit/list/search/tag work fully
//! offline with zero connectivity, zero sign-in, and — by design — zero model.
//! Note-taking is capture, not reasoning: this module contains **no LLM/SLM, no
//! proxy, no network** — only deterministic SQL, exactly like
//! [`crate::citation_library`] (the 1:1 template) and [`crate::projects`].
//!
//! Two note types live in ONE table:
//! - `paper`  — a structured template attached to a paper (the optional 8-field
//!   template lives in `fields_json`).
//! - `project` — the researcher's own ideas / hypotheses / meeting notes / todos
//!   (fast quick-capture; usually just `title` + `body`).
//!
//! # The soft paper anchor (why a note outlives its paper)
//!
//! `paper_id` references a paper by CONVENTION (→ `citation_library.id`) but has
//! **NO foreign-key constraint** — a note is the researcher's OWN work and must
//! never be cascade-deleted when the paper it annotates leaves a library. This
//! module never touches any library table, and `paper_title` is denormalized so
//! a note stays self-sufficient (still shows its paper) even if the referenced
//! row never existed or was deleted.
//!
//! # Sync
//!
//! `sync_status` is the same honest marker as citation_library
//! ('local_only' | 'pending' | 'synced'); every mutation resets it. Actual
//! local↔cloud sync is DEFERRED (no engine exists yet) — this marker is unused
//! for now and never gates note-taking.
//!
//! Placement note: CRUD lives in gaply-core because [`Database::conn`] is
//! deliberately crate-private. The Tauri commands (Set 3) are thin wrappers.

use rusqlite::params;
use serde::Serialize;

use crate::{now_epoch, Database, GaplyError};

/// The note types. `paper` carries the structured template; `project` is free
/// quick-capture; `manuscript` is the Research Paper Writer (sections + metadata
/// in fields_json). Additive: no schema change — note_type is a plain TEXT column.
pub const NOTE_TYPES: &[&str] = &["paper", "project", "manuscript"];

/// Honest sync markers (mirrors citation_library). Sync is deferred; every
/// local mutation resets to 'local_only'.
pub const SYNC_STATUSES: &[&str] = &["local_only", "pending", "synced"];

/// One stored note, either type.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    pub id: String,
    /// 'paper' | 'project'.
    pub note_type: String,
    /// Soft anchor (by convention → citation_library.id); None for project
    /// notes. NO foreign key — the note survives the paper's deletion.
    pub paper_id: Option<String>,
    /// Denormalized paper label so the note is self-sufficient for display.
    pub paper_title: String,
    pub title: String,
    /// The optional per-paper template fields as JSON — `citation`,
    /// `research_question`, `methodology`, `key_findings`, `notable_quotes`
    /// (each `{text, page}`), `limitations`, `my_evaluation`. ALL optional /
    /// free-form; `{}` for project notes. Stored verbatim so the template can
    /// evolve without a migration.
    pub fields_json: String,
    /// Free-form body (especially project quick-capture).
    pub body: String,
    pub tags: Vec<String>,
    pub sync_status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The writable fields of a note. Everything but `note_type` is optional /
/// free-form (the template guides, it never forces).
#[derive(Debug, Clone, Default)]
pub struct NoteInput {
    pub note_type: String,
    pub paper_id: Option<String>,
    pub paper_title: String,
    pub title: String,
    /// JSON object of template fields; empty string is treated as `{}`.
    pub fields_json: String,
    pub body: String,
    pub tags: Vec<String>,
}

const COLS: &str = "id, note_type, paper_id, paper_title, title, fields_json, body, tags, \
                    sync_status, created_at, updated_at";

fn row_to_note(row: &rusqlite::Row) -> rusqlite::Result<Note> {
    let tags_json: String = row.get(7)?;
    Ok(Note {
        id: row.get(0)?,
        note_type: row.get(1)?,
        paper_id: row.get(2)?,
        paper_title: row.get(3)?,
        title: row.get(4)?,
        fields_json: row.get(5)?,
        body: row.get(6)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        sync_status: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

/// Normalize + validate the input into the values we store. `fields_json`
/// defaults to `{}` and must be valid JSON when present; `note_type` must be
/// one of [`NOTE_TYPES`].
fn prepare(input: &NoteInput) -> Result<(String, String), GaplyError> {
    if !NOTE_TYPES.contains(&input.note_type.as_str()) {
        return Err(GaplyError::Validation(format!(
            "note_type must be one of {NOTE_TYPES:?}, got {:?}",
            input.note_type
        )));
    }
    let fields = if input.fields_json.trim().is_empty() { "{}".to_string() } else { input.fields_json.clone() };
    serde_json::from_str::<serde_json::Value>(&fields)
        .map_err(|e| GaplyError::Validation(format!("fields_json is not valid JSON: {e}")))?;
    let tags_json = serde_json::to_string(&input.tags)
        .map_err(|e| GaplyError::Internal(format!("tags serialize: {e}")))?;
    Ok((fields, tags_json))
}

/// Create or replace a note by `id` (the caller supplies a stable uuid, like
/// citation_library). Sets timestamps; `created_at` is preserved on update.
/// Any mutation honestly resets `sync_status` to 'local_only'.
pub fn upsert_note(db: &Database, id: &str, input: &NoteInput) -> Result<Note, GaplyError> {
    if id.trim().is_empty() {
        return Err(GaplyError::Validation("note id must not be empty".into()));
    }
    let (fields_json, tags_json) = prepare(input)?;
    let now = now_epoch();
    db.conn()?.execute(
        "INSERT INTO notes
             (id, note_type, paper_id, paper_title, title, fields_json, body, tags,
              sync_status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'local_only', ?9, ?9)
         ON CONFLICT(id) DO UPDATE SET
             note_type = excluded.note_type,
             paper_id = excluded.paper_id,
             paper_title = excluded.paper_title,
             title = excluded.title,
             fields_json = excluded.fields_json,
             body = excluded.body,
             tags = excluded.tags,
             sync_status = 'local_only',
             updated_at = excluded.updated_at",
        params![
            id,
            input.note_type,
            input.paper_id,
            input.paper_title,
            input.title,
            fields_json,
            input.body,
            tags_json,
            now
        ],
    )?;
    get_note(db, id)?.ok_or_else(|| GaplyError::Internal("upsert_note lost the row".into()))
}

/// Edit an EXISTING note (errors if it does not exist). Bumps `updated_at` and
/// resets `sync_status`. `create` is [`upsert_note`] with a fresh id.
pub fn update_note(db: &Database, id: &str, input: &NoteInput) -> Result<Note, GaplyError> {
    if get_note(db, id)?.is_none() {
        return Err(GaplyError::NotFound { entity: "note", id: id.to_string() });
    }
    upsert_note(db, id, input)
}

pub fn get_note(db: &Database, id: &str) -> Result<Option<Note>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM notes WHERE id = ?1"))?;
    let mut rows = stmt.query_map(params![id], row_to_note)?;
    Ok(rows.next().transpose()?)
}

/// List notes, newest first — optionally filtered by `note_type` ('paper' /
/// 'project') and/or `paper_id` (all notes for one paper). Both None = all.
pub fn list_notes(
    db: &Database,
    note_type: Option<&str>,
    paper_id: Option<&str>,
) -> Result<Vec<Note>, GaplyError> {
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM notes
         WHERE (?1 IS NULL OR note_type = ?1)
           AND (?2 IS NULL OR paper_id = ?2)
         ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map(params![note_type, paper_id], row_to_note)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Escape a user's literal text for use inside a `LIKE` pattern that declares
/// `ESCAPE '\'`. The two SQL wildcards — `%` (any run) and `_` (any single
/// character) — plus the escape character itself become literals.
///
/// This used to STRIP `%` and `_` from the pattern instead, which silently
/// changed the search: `my_note` became the pattern `%mynote%`, which matches
/// nothing, so a note sitting right there reported "No notes match your search"
/// — indistinguishable from an empty library. Underscores are ordinary in this
/// audience's vocabulary (gene names, dataset ids, file names, variables).
fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '\\' || c == '%' || c == '_' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Deterministic local search over title / body / template fields / paper title
/// / tags. `tag` additionally filters to notes carrying that exact tag.
///
/// Every `LIKE` here declares `ESCAPE '\'` so a query is matched as the literal
/// text the user typed — wildcards included.
pub fn search_notes(
    db: &Database,
    query: &str,
    tag: Option<&str>,
) -> Result<Vec<Note>, GaplyError> {
    let q = query.trim();
    let like = format!("%{}%", escape_like(q));
    // The tag filter is an exact-tag test expressed as a substring of the JSON
    // array, so its needle needs the same escaping — a tag like `to_read` was
    // unfindable for exactly the same reason.
    let tag_like = tag.map(|t| format!("%\"{}\"%", escape_like(&t.replace('"', ""))));
    let conn = db.conn()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM notes
         WHERE (?1 = '' OR title LIKE ?2 ESCAPE '\\' COLLATE NOCASE
                OR body LIKE ?2 ESCAPE '\\' COLLATE NOCASE
                OR fields_json LIKE ?2 ESCAPE '\\' COLLATE NOCASE
                OR paper_title LIKE ?2 ESCAPE '\\' COLLATE NOCASE
                OR tags LIKE ?2 ESCAPE '\\' COLLATE NOCASE)
           AND (?3 IS NULL OR tags LIKE ?3 ESCAPE '\\')
         ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map(params![q, like, tag_like], row_to_note)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Replace a note's tags. Resets sync_status honestly.
pub fn set_tags(db: &Database, id: &str, tags: &[String]) -> Result<Note, GaplyError> {
    let tags_json = serde_json::to_string(tags)
        .map_err(|e| GaplyError::Internal(format!("tags serialize: {e}")))?;
    let n = db.conn()?.execute(
        "UPDATE notes SET tags = ?2, sync_status = 'local_only', updated_at = ?3 WHERE id = ?1",
        params![id, tags_json, now_epoch()],
    )?;
    if n == 0 {
        return Err(GaplyError::NotFound { entity: "note", id: id.to_string() });
    }
    get_note(db, id)?.ok_or_else(|| GaplyError::Internal("set_tags lost the row".into()))
}

/// Delete a NOTE (this is the researcher deleting their own note — fine).
/// Deleting a PAPER never reaches here: notes carry no foreign key.
pub fn delete_note(db: &Database, id: &str) -> Result<(), GaplyError> {
    db.conn()?.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
    Ok(())
}

/// The deferred sync layer's honest marker — only the [`SYNC_STATUSES`] accepted.
pub fn set_sync_status(db: &Database, id: &str, status: &str) -> Result<(), GaplyError> {
    if !SYNC_STATUSES.contains(&status) {
        return Err(GaplyError::Validation(format!(
            "unknown sync_status {status:?} (expected local_only, pending or synced)"
        )));
    }
    let n = db
        .conn()?
        .execute("UPDATE notes SET sync_status = ?2 WHERE id = ?1", params![id, status])?;
    if n == 0 {
        return Err(GaplyError::NotFound { entity: "note", id: id.to_string() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Database {
        Database::in_memory().unwrap()
    }

    /// A per-paper structured note: the 8-field template lives in fields_json.
    fn paper_input(paper_id: &str) -> NoteInput {
        NoteInput {
            note_type: "paper".into(),
            paper_id: Some(paper_id.into()),
            paper_title: "Molecular Structure of Nucleic Acids".into(),
            title: "Watson & Crick — my notes".into(),
            fields_json: r#"{
                "citation":"Watson & Crick, 1953",
                "research_question":"What is the structure of DNA?",
                "methodology":"X-ray crystallography interpretation",
                "key_findings":"Double helix, antiparallel strands",
                "notable_quotes":[{"text":"This structure has novel features","page":737}],
                "limitations":"Model, not direct imaging",
                "my_evaluation":"Foundational; base-pairing insight is the key"
            }"#
            .into(),
            body: String::new(),
            tags: vec!["dna".into(), "toread".into()],
        }
    }

    /// A project quick-capture note: just title + body, empty fields_json.
    fn project_input() -> NoteInput {
        NoteInput {
            note_type: "project".into(),
            paper_id: None,
            paper_title: String::new(),
            title: "Thesis idea".into(),
            fields_json: String::new(), // → {}
            body: "Hypothesis: sleep extension improves recall. Ask advisor Friday.".into(),
            tags: vec!["idea".into()],
        }
    }

    #[test]
    fn migration_v9_applied_and_table_usable() {
        let d = db(); // in_memory() runs migrations through v9
        assert!(list_notes(&d, None, None).unwrap().is_empty());
        assert!(upsert_note(&d, "n1", &project_input()).is_ok());
    }

    #[test]
    fn crud_round_trip_both_types_with_filters() {
        let d = db();
        upsert_note(&d, "p1", &paper_input("cite-watson")).unwrap();
        upsert_note(&d, "j1", &project_input()).unwrap();

        // get
        let p = get_note(&d, "p1").unwrap().unwrap();
        assert_eq!(p.note_type, "paper");
        assert_eq!(p.paper_id.as_deref(), Some("cite-watson"));
        assert_eq!(p.tags, vec!["dna", "toread"]);

        // list all + filtered by type + by paper_id
        assert_eq!(list_notes(&d, None, None).unwrap().len(), 2);
        assert_eq!(list_notes(&d, Some("paper"), None).unwrap().len(), 1);
        assert_eq!(list_notes(&d, Some("project"), None).unwrap().len(), 1);
        assert_eq!(list_notes(&d, None, Some("cite-watson")).unwrap().len(), 1);
        assert_eq!(list_notes(&d, None, Some("nobody")).unwrap().len(), 0);

        // update (must-exist) bumps updated_at, edits fields
        let mut edited = paper_input("cite-watson");
        edited.title = "Watson & Crick — revised".into();
        let up = update_note(&d, "p1", &edited).unwrap();
        assert_eq!(up.title, "Watson & Crick — revised");
        assert!(update_note(&d, "ghost", &project_input()).is_err(), "update must exist");

        // delete a note (the researcher's own choice)
        delete_note(&d, "j1").unwrap();
        assert_eq!(list_notes(&d, None, None).unwrap().len(), 1);
    }

    #[test]
    fn search_across_title_body_fields_and_tags() {
        let d = db();
        upsert_note(&d, "p1", &paper_input("cite-watson")).unwrap();
        upsert_note(&d, "j1", &project_input()).unwrap();

        assert_eq!(search_notes(&d, "helix", None).unwrap().len(), 1, "fields_json match");
        assert_eq!(search_notes(&d, "advisor", None).unwrap().len(), 1, "body match");
        assert_eq!(search_notes(&d, "revised", None).unwrap().len(), 0);
        assert_eq!(search_notes(&d, "nucleic", None).unwrap().len(), 1, "paper_title match");
        assert_eq!(search_notes(&d, "", Some("idea")).unwrap().len(), 1, "tag filter");
        assert_eq!(search_notes(&d, "zebrafish", None).unwrap().len(), 0);

        // tag management
        let tagged = set_tags(&d, "j1", &["idea".into(), "priority".into()]).unwrap();
        assert_eq!(tagged.tags, vec!["idea", "priority"]);
        assert_eq!(search_notes(&d, "", Some("priority")).unwrap().len(), 1);
    }

    #[test]
    fn soft_anchor_survives_missing_paper() {
        // THE soft-anchor proof: a per-paper note referencing a paper id that
        // has NO library row (never created / already deleted). The note is
        // accepted (no foreign key), persists, and reads correctly — the
        // denormalized paper_title carries the paper's identity on its own.
        let d = db();
        upsert_note(&d, "orphan", &paper_input("ghost-paper-id-that-never-existed")).unwrap();

        let n = get_note(&d, "orphan").unwrap().unwrap();
        assert_eq!(n.paper_id.as_deref(), Some("ghost-paper-id-that-never-existed"));
        assert_eq!(n.paper_title, "Molecular Structure of Nucleic Acids");
        assert!(n.fields_json.contains("Double helix"), "the note keeps ALL its content");
        // it lists like any other paper note — nothing cascaded it away
        assert_eq!(list_notes(&d, Some("paper"), None).unwrap().len(), 1);
    }

    #[test]
    fn schema_has_no_foreign_key_on_paper_id() {
        // Structural proof of the soft anchor: notes declares NO foreign keys,
        // so a paper deletion can never cascade into a note.
        let d = db();
        let conn = d.conn().unwrap();
        let fk_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_foreign_key_list('notes')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fk_count, 0, "notes must have NO foreign key (soft anchor)");
    }

    /// A query is LITERAL TEXT, not a pattern. The wildcards `%` and `_` used to
    /// be stripped from the LIKE pattern, so any query containing one silently
    /// matched nothing (`my_note` → the pattern `%mynote%`) and the UI reported
    /// "No notes match your search" for a note that was right there.
    #[test]
    fn search_treats_sql_wildcards_as_literal_text() {
        let d = db();

        let mut underscored = project_input();
        underscored.title = "my_note on ANOVA".into();
        underscored.body = "uses the gene BRCA_1 and dataset ukb_500k".into();
        underscored.tags = vec!["to_read".into()];
        upsert_note(&d, "u1", &underscored).unwrap();

        let mut percent = project_input();
        percent.title = "Power at 80% and beyond".into();
        percent.body = String::new();
        percent.tags = vec![];
        upsert_note(&d, "p1", &percent).unwrap();

        // a decoy WITHOUT the separator — what the stripped pattern used to hit
        let mut decoy = project_input();
        decoy.title = "mynote".into();
        decoy.body = "BRCA1 ukb500k".into();
        decoy.tags = vec![];
        upsert_note(&d, "d1", &decoy).unwrap();

        // THE PIN: a query carrying BOTH wildcards finds the real note and only it
        let hits = search_notes(&d, "my_note", None).unwrap();
        assert_eq!(hits.len(), 1, "underscore must match literally");
        assert_eq!(hits[0].id, "u1");

        let hits = search_notes(&d, "80%", None).unwrap();
        assert_eq!(hits.len(), 1, "percent must match literally");
        assert_eq!(hits[0].id, "p1");

        // body + fields searching honors it too
        assert_eq!(search_notes(&d, "BRCA_1", None).unwrap().len(), 1);
        assert_eq!(search_notes(&d, "ukb_500k", None).unwrap().len(), 1);

        // and a wildcard is NOT a wildcard: these must not match anything
        assert_eq!(search_notes(&d, "my_n%", None).unwrap().len(), 0, "% must not glob");
        assert_eq!(search_notes(&d, "myXnote", None).unwrap().len(), 0, "_ must not match any char");

        // the tag filter shares the escaping
        assert_eq!(search_notes(&d, "", Some("to_read")).unwrap().len(), 1);
        assert_eq!(search_notes(&d, "", Some("to%read")).unwrap().len(), 0);

        // a literal backslash is itself, not an escape
        let mut backslash = project_input();
        backslash.title = r"path\to\thing".into();
        backslash.body = String::new();
        backslash.tags = vec![];
        upsert_note(&d, "b1", &backslash).unwrap();
        assert_eq!(search_notes(&d, r"path\to", None).unwrap().len(), 1);

        // ordinary queries are unaffected
        assert_eq!(search_notes(&d, "ANOVA", None).unwrap().len(), 1);
        assert_eq!(search_notes(&d, "", None).unwrap().len(), 4, "empty query still lists all");
    }

    #[test]
    fn fields_json_defaults_to_empty_object_and_rejects_bad_json() {
        let d = db();
        // project note with empty fields_json → stored as {}
        let j = upsert_note(&d, "j1", &project_input()).unwrap();
        assert_eq!(j.fields_json, "{}");
        // invalid template JSON is refused (honest)
        let mut bad = paper_input("x");
        bad.fields_json = "{not json".into();
        assert!(upsert_note(&d, "bad", &bad).is_err());
        // an unknown note_type is refused
        let mut wrong = project_input();
        wrong.note_type = "diary".into();
        assert!(upsert_note(&d, "w", &wrong).is_err());
        // empty id refused
        assert!(upsert_note(&d, "  ", &project_input()).is_err());
    }

    #[test]
    fn mutations_reset_sync_status_and_markers_are_honest() {
        let d = db();
        let n = upsert_note(&d, "n1", &project_input()).unwrap();
        assert_eq!(n.sync_status, "local_only");

        set_sync_status(&d, "n1", "synced").unwrap();
        assert_eq!(get_note(&d, "n1").unwrap().unwrap().sync_status, "synced");

        // any edit makes 'synced' a lie → resets to local_only
        update_note(&d, "n1", &project_input()).unwrap();
        assert_eq!(get_note(&d, "n1").unwrap().unwrap().sync_status, "local_only");

        set_sync_status(&d, "n1", "pending").unwrap();
        set_tags(&d, "n1", &[]).unwrap();
        assert_eq!(get_note(&d, "n1").unwrap().unwrap().sync_status, "local_only");

        assert!(set_sync_status(&d, "n1", "faked").is_err());
        assert!(set_sync_status(&d, "missing", "synced").is_err());
    }

    #[test]
    fn deterministic_same_input_same_result() {
        let d1 = db();
        let d2 = db();
        let a = upsert_note(&d1, "n1", &paper_input("c1")).unwrap();
        let b = upsert_note(&d2, "n1", &paper_input("c1")).unwrap();
        // identical but for the timestamps (wall clock) — compare the content
        assert_eq!(a.note_type, b.note_type);
        assert_eq!(a.paper_id, b.paper_id);
        assert_eq!(a.fields_json, b.fields_json);
        assert_eq!(a.tags, b.tags);
    }
}
