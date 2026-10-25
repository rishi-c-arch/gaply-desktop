# ADR-009: Projects, the artifact store and provenance

**Status: Accepted, 19 Oct 2026, for what decisions 99 to 111 cover, and 25
Oct 2026 (decision 234), on the 5E evaluation's evidence and 5E-fix-1's, for
§1's resolution as built on macOS, §2, §3, §5 but its Windows sentence, §6,
§7, §9, §10 for Unix, §11 but its Windows clause, §15 and §16. Not yet:
§1's resolution on Windows and Linux, the Windows sentences of §5, §10 and
§11, and decision 110's retirement of the seam commands (§13). What decision
158 adds is Proposed (§13's Stored data part and §17).** Each part below is
marked **[Accepted, decision N]**, **[Not yet, decision N]** or **[Proposed]**. Written 19 Oct 2026 in Research
OS Phase 5A as a draft (`~/gaply-corpus/os/phase5/`), made to say what
decisions 99 to 111 say and copied here in Phase 5B-1 (decision 112). The
places where the 5A draft said more than the decisions are listed at the end,
each with what this ADR now says.

The blueprint (§20) lists ADR-009 as "project schema and migration
compatibility". This ADR covers the project model, the artifact store and the
provenance graph as well, because the schema holds the other two and the
three could not be approved apart. ADR-004 (as revised on 19 Oct 2026) covers
what a manifest and an artifact hold; this ADR covers where they live and how
they are kept, linked and deleted.

## Context

- Blueprint §10.1 to §10.5 ask for a project as a container of references, an
  immutable content-addressed artifact store with atomic writes, SQLite for
  metadata with transactions, a provenance DAG with `used`, `generated` and
  `wasDerivedFrom` edges mapped to PROV later, and a reproducibility manifest.
  §8.5 asks that terminal run states persist. §18.1 asks for migration tests
  and fault injection for disk-full and interrupted writes. §23: "interrupted
  writes and app restart do not corrupt project metadata or misattach
  artifacts".
- Before Phase 5 nothing of it existed, and the Research OS wrote nothing to
  disk except an export the user saves (ADR-004a, decisions 5, 12, 20, 79).
  Prompt 0's DATA ON DISK rule: nothing new persists user data until
  location, protection, retention and deletion are approved as an ADR.
- The existing database (`gaply.db`) already holds manuscript text, document
  chunks and file paths with no delete path (`phase5a_proposal.md` §1). Phase
  5 must not add to that pattern: every new row and file has a delete path
  from the day it exists.

## Decision

### 1. Where projects live [Accepted, decision 99]

Projects live in the app's local data folder, never a synced one, resolved
through Tauri's local app data directory (`PathResolver::app_local_data_dir`):

| platform | folder |
|---|---|
| macOS | `~/Library/Application Support/ai.gaply.app/` |
| Windows | `%LOCALAPPDATA%\ai.gaply.app\` (Local, not Roaming) |
| Linux | `$XDG_DATA_HOME/ai.gaply.app/` (default `~/.local/share/ai.gaply.app/`) |

Never Documents, Desktop, or an iCloud or OneDrive folder. On Windows this is
not the folder `gaply.db` is in (that one is under `%APPDATA%`, Roaming).

**[Accepted on macOS, decision 234]** The resolution as built in 5B-1:
`project_store::projects_root(app)` is the one function that resolves the
folder, and it calls `app_local_data_dir()` and no other resolver (pinned by
`src-tauri/tests/project_store_guards.rs`). The projects are under
`<that folder>/projects/`. No `appDirectoriesOverride` is set in
`tauri.conf.json`. The resolution table rests on docs.rs tauri 2.12.4
(`PathResolver`), read 19 Oct 2026; `Cargo.lock` holds tauri 2.11.5 (inferred
to resolve the same way). **[Not yet, decision 234]** On Windows and Linux the
resolution as built is not measured: the table's rows for them rest on
tauri's docs (5E evaluation §1). The table itself stays Accepted by
decision 99.

### 2. One database per project; the list is a scan [Accepted, decision 106]

One database per project: `projects/<id>/project.db`, beside its `artifacts`
folder. The list of projects is a scan of the folder.

**[Accepted, decision 234]** The layout as built in 5B-1 (`src-tauri/src/project_store.rs`):

```
<app_local_data_dir>/projects/          0700
  <project_id>/                          0700, named by the id, never the title
    project.db                           0600, created before SQLite opens it
    project.db-wal, project.db-shm       SQLite's own; they take the db's mode
    artifacts/                           0700
      <sha256>                           one file per artifact, 0400
      .tmp-<32 hex>                      a write in progress, never read
  .creating-<project_id>/                a create in progress, renamed into place when complete
  .deleting-<project_id>/                a tombstone: a delete in progress
```

- A project id is 32 lowercase hex characters from the operating system's
  random source (`getrandom`); a hash is 64 lowercase hex characters. A
  parser is the only way to make either (`gaply_core::research_project`), and
  every path is joined only from a parsed id or hash or a fixed name. A user
  string never enters a path. The store never follows a symlink: a project
  folder, `project.db`, `artifacts/` or an artifact that is a symlink is
  refused (`project.symlink_refused` or `project.artifact_not_regular`), and
  nothing outside the root is read or removed.
- A project is created inside `.creating-<id>/` and renamed to `<id>/` only
  once its database holds its schema and its one project row, so a folder
  named by an id is never half-made.
- The list reads each folder's `project.db` for its project row and sums the
  folder's file sizes from metadata. It opens no artifact and hashes nothing.

### 3. `project.db` [Accepted, decision 234]

- Its own schema version in `PRAGMA user_version`, set inside the store:
  version 2 since 5C-1a (`research_project/mod.rs`), which added
  `runs.rerun_of`. A version 1 project migrates when it is opened, each step
  in its own `BEGIN IMMEDIATE` with the version read again under its lock; a
  newer version is refused and left untouched (decision 158, item 1). The
  `gaply.db` migration runner (`migrations.rs`) is hard-wired to its own list
  and is left unchanged.
- `PRAGMA journal_mode=WAL; synchronous=FULL; foreign_keys=ON;
  busy_timeout`. FULL, not NORMAL: in WAL mode NORMAL "might roll back
  following a power loss" while FULL is durable (sqlite.org/pragma.html,
  read 19 Oct 2026), so a Save that reports success survives a power cut.
- Tables: `project` (one row: `project_id`, `title`, `workspace_home`,
  `created_at`, `app_version`), `artifacts` (`hash`, `kind`, `format`,
  `bytes`, `schema_version`, `created_at`, `damage`), `runs` (`run_id`,
  `capability_id`, `capability_version`, `state`, `end_code`,
  `agent_version`, `started_at`, `ended_at`, `saved_at`, and since version
  2 `rerun_of`, the run a rerun repeats), and three edge
  tables, `used`, `generated` and `derived`.
- A row holds only ids, hashes, the capability id and version, a state,
  closed codes, sizes and times. The one user-typed column is
  `project.title`. Anything that names a column, a level or a value lives
  only inside an artifact (the operation spec, the result bundle, the data
  file), referred to by its hash.
- A `project.db` that is missing, unreadable or at a newer schema version is
  refused with its own code (`project.db_missing`, `project.db_unreadable`,
  `project.db_newer_schema`), and nothing in that project is removed.

### 4. The data file, copied in [Accepted, decision 101]

The user's original data file is copied into the project as an immutable
artifact identified by its SHA-256, within the ingest caps, and is deleted
with the project. Its path and its file name are never stored; its format is
kept as a closed code. The save sentence says a copy is kept on this computer
only.

### 5. The artifact write [Accepted, decision 234, but its Windows sentence]

As built in 5B-1, one Save is one `BEGIN IMMEDIATE` transaction held from
the first step to the last:

1. `BEGIN IMMEDIATE`.
2. The caps (§9): nothing is written if either would be passed.
3. For each artifact: its bytes must hash to the hash the caller holds
   (`project.hash_mismatch`). If `artifacts/<sha256>` already exists it is
   re-verified by hashing it, then reused; a file that does not verify is
   refused with its damage code and never overwritten. Otherwise: write
   `artifacts/.tmp-<random>` (created new, never opened over an existing
   file), flush and `sync_all`, set it to 0400, re-read it and verify its
   hash, rename it to `artifacts/<sha256>`, and sync the `artifacts` folder.
4. Insert the artifact rows, the run row and its edges, and commit.
5. If step 4 or the commit fails, the files this Save renamed into place are
   removed at once; anything left over is recovery's.

A row is therefore a promise that its bytes exist and verified when written.
**[Not yet, decision 234: the store's tests do not run on Windows, decision
114]** **On Windows a folder cannot be synced;** what stands in its place is NTFS's
metadata journal for the rename (inferred, not measured), with the recovery
rule below covering a lost rename: a row whose file is missing is marked
damaged, never read.

### 6. Runs [Accepted, decisions 102 and 105, in part]

- **[Accepted, decision 105]** Nothing is written to a project except when
  the user clicks Save. There is no autosave. Outside a saved project, today's
  in-memory rules hold (at most 8 runs and 4 dataset slots, decision 6). The
  one exception is recovery when a project opens: it may remove the store's
  own leftovers and mark states, and never adds user data.
- **[Accepted, decision 102]** When Level 0 and Level 1 runs come into
  projects (5C), the sentences they checked are kept inside the run's bundle
  artifact, never as a database row, and the save sentence says so. AI
  explanations are not stored in Phase 5.
- **[Accepted, decision 234]** In 5B only finished runs are saved, written once, in their
  terminal state (`project.run_not_finished` otherwise). Recovery at open, in
  its own `BEGIN IMMEDIATE` so it cannot remove a file another writer is
  writing: leftover `.tmp-*` files are removed; an artifact file with no row
  is removed; a row whose file is missing, not a regular file, or of the
  wrong size is marked damaged with a closed code; and a run left `Queued` or
  `Running` becomes `Failed` with `project.run_interrupted`, never
  `Succeeded` (for 5C's reruns; tested now). A file is hashed whenever it is
  read, and a mismatch marks it damaged and refuses the read. A same-size
  corruption of an artifact is therefore not seen at open or in the list,
  only at the read (`project.artifact_wrong_hash`; 5E evaluation N1).
- Decision 186, as built in 5C-1a: a rerun's save at its end is the second
  write a click starts (the click is Run again). No run row exists until a
  run has ended, and nothing is written while it runs; a cancel, refusal,
  quit or crash writes nothing. The `RunSink` of the 5A draft is not built,
  so recovery's interrupted-run rule above has no writer behind it and is
  kept as a guard (decision 158, item 2).

### 7. Provenance [Accepted, decision 234]

The graph, in `gaply_core::research_project`: artifact versions (by hash),
runs and the app as agent, with the edges `used` (run → artifact, with a
closed role), `wasGeneratedBy` (artifact → run), `wasDerivedFrom` (artifact
→ artifact, via a run) and `wasAssociatedWith` (run → agent). The agent is
the app and its version, never a person's name or account. Adding an edge
that would close a cycle is refused with `project.prov_cycle`, and a property
test asserts acyclicity. **[Accepted, decision 111]** The PROV JSON export
and the project report are built in 5B-2, not 5B-1.

As built (decision 158, item 3): the export adds `wasInformedBy` (a rerun →
the run it repeats) only in a project that holds a rerun; `wasDerivedFrom`
is derived at export from `used` and `wasGeneratedBy`, and no Save stores
one. The export has the PROV-JSON shape: its `prov:type` values are Gaply's
own (`gaply:data_file` and the like), its `prefix` map declares only
`gaply:`, and its relation keys are blank-node strings. A PROV-JSON reader
accepts it; a PROV-DM reasoner sees typed agents only (5E evaluation N10).

### 8. Retention [Accepted, decision 103]

A project is kept until the user deletes it. Sizes are shown. Nothing is
deleted automatically.

### 9. Limits [Accepted, decision 107]

100 MiB per project and 10 GiB for all projects, each one constant pinned by
a test. A full disk or a cap is a closed refusal that leaves nothing
half-written.

**[Accepted, decision 234]** As built in 5B-1: a project's size is every file in its
folder (artifacts, temporary files and database files), except
`project.db-wal` and `project.db-shm`, which are skipped since decision 173. Before a Save writes
anything the store adds the project's size on disk, every new artifact in
that Save, and `DB_GROWTH_ALLOWANCE_BYTES` (1 MiB); over either cap it refuses
(`project.project_cap`, `project.all_projects_cap`, with the needed and limit
bytes) and nothing is written. A tombstone counts toward the all-projects cap
until it is removed. A full disk (`ENOSPC`, `SQLITE_FULL`) is
`project.disk_full`. The caps are parameters of the store, so tests use small
ones. No free-space probe: none is in `Cargo.lock`, and the write is attempted
and its failure mapped to a closed code. A checkpoint runs after each Save
and when a project closes, so the `-wal` stays small; a checkpoint left busy
by another copy's reader waited at most 5.2 s (5E evaluation §3) (decision
158, item 6).

### 10. Protection [Accepted, decision 100]

- Owner-only permissions: folders 0700, databases 0600, artifacts 0400 where
  the platform has them. On Windows, the per-user folder's own access list,
  stated as such.
- No encryption at rest in v1. Settings says plainly that projects rely on
  the computer's disk encryption (FileVault, BitLocker or LUKS).
- Encryption is a later decision of its own.

**[Accepted for Unix, decision 234]** As built: permissions set when each folder and file is
created, so a permissive umask cannot widen them; `project.db` is created at
0600 before SQLite opens it, so its `-wal` and `-shm` files take the same
mode; an artifact's temporary file is set to 0400 before the rename, so the
final file is never writable. **[Not yet, decision 234: not measured on
Windows]** On Windows no access list is changed: the
default security descriptor is inherited from `%LOCALAPPDATA%`
(learn.microsoft.com, file security and access rights, read 19 Oct 2026; not
measured on a Windows machine).

### 11. Deleting a project [Accepted, decision 104]

- It removes the project's folder and everything in it, through a tombstone,
  so an interrupted delete finishes at the next start.
- It asks for confirmation, naming what it cannot reach: Time Machine and
  other backups, file-system snapshots, an SSD's spare blocks, and files the
  user exported.
- It makes no overwrite pass.

**[Accepted, decision 234, but its Windows clause]** As built in 5B-1 and
5E-fix-1 (decision 223, D291): the project's connections are closed first
(a delete is refused with `project.still_open` while this process holds the
project open); the store then checks that it can empty the folder, and when
it cannot, the folder keeps its name, nothing is deleted, and the delete
refuses with `project.folder_unchangeable` (Gaply never changes a folder's
permissions to get past this); otherwise the folder is renamed to
`.deleting-<id>`, then removed. A tombstone found when the store starts is
finished; one it cannot remove is skipped, logged by a closed code and a
count, and tried again at the next start, and never fails the open. A
project whose database cannot be read and whose folder fails the same check
is listed with `project.folder_unchangeable`. Read-only files block
neither the delete nor recovery's removals. **[Not yet, decision 234]** On
Windows the read-only attribute is cleared first; not tested on Windows. The
confirmation screen is 5B-2's.

### 12. Exporting and importing a project [Accepted, decision 108]

Exporting and importing a project as one file come later, after a project
has been saved and deleted in the real app once.

### 13. The old database and the dormant `projects` table [Accepted, decisions 109 and 110]

- **Decision 109 (5D):** first, a Stored data screen in Settings, with
  counts, sizes and a Delete for each category, never gated; then each
  stop-writing change as its own approval, since each touches a shipped
  feature. The Stats Check page's write of manuscript rows through the old
  path is one of them, and is fixed before the Stats Check's flag ships.
- **Decision 110:** the dormant `projects` table in `gaply.db` and its three
  seam commands (`create_project`, `list_projects`, `get_project`) stay as
  they are, unused, and are retired later. Phase 5's store does not use them.
  **[Not yet, decision 234]** The retirement is not yet done (decision 221
  moves 5D-2 after Phase 8's first methods): the three are registered and
  ungated, and `create_project` writes a typed name into the dormant table
  with no delete path (5E evaluation N6, N12).

**[Proposed, decision 158, items 10 and 11]** The Stored data section as
built in 5D-1a, 5D-1b, 5E-fix-1 and 5E-fix-2 (D289, D290, D291 and D292):

- Nine categories, counted from one read transaction and file metadata,
  bytes not characters, symlinks never followed; each with a Delete… that
  needs a confirmation issued for that category just before
  (`crate::confirmation`, shared with the projects' delete).
- A delete is refused while work that writes into its category is going:
  one running-work counter in the app's state, its guard taken first by six
  shipped commands, and the thesis audits' jobs map (decision 214).
- Categories 1 to 4 are deleted from their root, 5 by `cache_evict_stale`'s
  statement, 6 to 9 file by file. Categories 1 to 5 run with
  `secure_delete=ON` on their own connection, so the text they delete is
  zeroed in the pages they change, and then one `wal_checkpoint(TRUNCATE)`,
  retried once; no VACUUM (decision 224). Each start tries one checkpoint
  too, which never fails the open.
- The checkpoint after a delete and the one at open run with a busy timeout
  of 1 s on their own connection, restored afterwards, and the retry waits
  200 ms first (decision 230). On a 590 MB database with a reader held
  across it, the checkpoint gave up in 2.45 s (8.0 s with an 8 s reader at
  5a0089c) and a concurrent writer's longest wait was 1.17 s with no failed
  write (5.26 s and one failed write at 5a0089c); an open with the reader
  held took 1.15 s (5.27 s at 5a0089c) (5E-fix-2 item 1).
- Category 1 deletes in batches (decision 230): each batch its own
  `IMMEDIATE` transaction under `secure_delete`, at most 64 MiB of the
  category's counted bytes and at least one manuscript, 200 ms between two
  batches with no transaction open, all under the one running-work hold. A
  failure after the first batch stops the delete, keeps the earlier batches
  deleted, still runs the checkpoint and answers `stored_data.delete_partial`.
  On the 590 MB database (550 manuscripts, 576,750,130 counted bytes) the
  delete took 4.22 s and a concurrent writer's longest wait was 519 ms with
  no failed write; as one transaction it held the write lock 4.8 s (D291)
  and the writer waited 2.3 s (5E-fix-2 item 1). Categories 2 to 5 stay one
  transaction each; their writer waits were at most 234 ms.
- The file does not shrink; older copies of deleted text that SQLite left in
  free space before a delete are not overwritten (decision 224).
- Not done: 5D's stop-writing changes (5D-2) and decision 110's retirement.

### 14. The order of Phase 5 [Accepted, decision 111]

5B-1, 5B-2, 5C, 5D and 5E, as `phase5a_proposal.md` §7 sets out, with one
change: the PROV JSON export and the project report move from 5B-1 to 5B-2.
Each step is its own commit.

### 15. A project belongs to one workspace [Accepted, decision 234]

`workspace_home` is `statistics`, the only value Phase 5 stores. The
commands of 5B-2 that create, open, save to or read a project check that
Statistics is active first; listing, the sizes, preparing a delete and
deleting are never gated (decision 120). Closing a project needs Statistics
active, and leaving Statistics closes it (decision 135). `installed_workspaces` and `active_workspace` of blueprint
§10.1 are not stored: the first is a build fact, the second the session's
state (ADR-001), and a stored copy would be a second source of truth.

### 16. The AI boundary and the Supabase client [Accepted, decision 234]

No project, artifact, run or provenance type may reach an outbound struct,
the AI helper's transport or the Supabase client. As built in 5B-1:
`src-tauri/tests/project_store_guards.rs` scans the outbound and transport
files of both crates and the frontend's Supabase service for every type the
project modules declare (the list is read from their source), and the
project modules for any route to an HTTP client, the proxy or the helper
layer; it cannot see data copied into a generic carrier (a `String`, a
`Vec<u8>`) by a third module, nor a future screen handing a project's
values to the Supabase client at run time. The tracing guards
(`no_path_in_tracing.rs`, `no_error_text_in_tracing.rs`) scan the store as
they scan every file; it logs a project only by its id and an artifact only
by an 8-character hash prefix.

### 17. Added at the 5E evaluation [Proposed, decision 158]

What the evaluation found built and not yet written here (5E evaluation §1,
"What ADR-009 does not yet mention"); items 1, 2, 3, 6, 10 and 11 are in §3,
§6, §7, §9 and §13 above.

- Item 4: the run manager's end hook (`runs.rs`), the going flag, `leave()`'s
  `workspace.run_active` refusal while a rerun is going, and
  `project_current`'s rerun state.
- Item 5: the twelfth and thirteenth commands, `project_current` (exempt
  from the workspace check, answering an id and a rerun state only) and
  `project_rerun` (gated).
- Item 7: the staging folder of a first Save is held under the project's
  shared lock, so another copy's recovery skips it.
- Item 8: a result read back is at most `RESULT_READ_LIMIT_BYTES` (4 MiB),
  and its stored JSON is returned as text, unparsed (decision 129).
- Item 9: the title rule's list of invisible format characters (decision
  121).

## Alternatives considered

- **New tables in `gaply.db`.** Rejected: on Windows that file is under
  Roaming; it mixes the user's projects with tables that hold undeletable
  manuscript data; and one project's deletion would be row-level surgery on
  a shared file.
- **One `projects.db` for all projects.** Rejected by decision 106.
- **A project as one JSON blob.** Rejected by blueprint §10.1.
- **A global content-addressed store shared across projects.** Rejected:
  deleting one project could not promise to delete its bytes, and two
  projects would share a file holding one user's data. Two projects never
  share a folder or an artifact file.
- **Encryption at rest now.** Deferred by decision 100.
- **Referring to the data file by path.** Rejected by decision 101.
- **Autosave.** Rejected by decision 105.

## Consequences

- The DATA ON DISK rule gets its first approved exception, bounded to
  `<app_local_data_dir>/projects/`, with a delete path that exists before the
  first row. In 5B-1 the store has no caller: nothing is written to a user's
  disk until 5B-2 wires it to a Save click.
- The store is the one Research OS module allowed to write
  (`stats_desc_writes_nothing.rs`), and no non-test code calls it in 5B-1
  (`project_store_guards.rs`).
- Two copies of the app can open the same project (macOS has no
  single-instance plugin; `src-tauri/src/lib.rs:68-79` adds it on Windows and
  Linux only). They share it through SQLite and the decision 132 lock:
  `BEGIN IMMEDIATE` and `busy_timeout` keep the database consistent, and a
  second writer gets `project.busy`; an open project holds its lock file,
  `projects/.locks/<project_id>.lock`, shared, and a delete refuses with
  `project.open_elsewhere` while another copy has the project open.

## Evidence

Phase 5B-1's tests (D281 in `docs/AI_ENGINE_PLAN.md`): the fault injections
(a stop before the rename, a stop before the commit, a full disk at the
write, `SQLITE_FULL` at the insert), recovery (temporary files, orphans,
damaged rows, an interrupted run), the refusals of a missing, corrupt and
newer `project.db`, the id and hash refusals including a symlinked folder,
the caps at the limit and one byte over, permissions, the sentinel sweep,
acyclicity, and the mutation record.

## Revision trigger

A second workspace that needs a project; a measured need for encryption at
rest; projects numbering in the hundreds; an export or import format; or
Windows CI running the store's tests and finding a difference.

## Where the 5A draft said more than decisions 99 to 111, and what this ADR says now

1. Ids minted by SQLite's `randomblob(16)`: now the operating system's random
   source through `getrandom`, as the 5B-1 step prompt says. Accepted with
   §2 (decision 234).
2. A `tmp/` folder for writes in progress: now `artifacts/.tmp-<hex>`, in the
   artifacts folder, as the step prompt says. Accepted with §2 (decision 234).
3. A `DELETING` tombstone file inside the folder: now the folder renamed to
   `.deleting-<id>`. Accepted with §2 and §11 (decision 234).
4. `project.db` on the `gaply.db` migration runner, made generic: now its own
   `PRAGMA user_version`, the runner unchanged. Accepted with §3 (decision 234).
5. `secure_delete=ON`, rows deleted and `VACUUM` at delete: not built for a
   project. Decision 104 removes the folder and makes no overwrite pass.
   Dropped. (The Stored data deletes of `gaply.db` use `secure_delete` and
   no VACUUM; that is §13, decision 224.)
6. A one-project cap of 1 GiB and a one-artifact cap of 100 MiB: decision 107
   sets 100 MiB per project, which also bounds an artifact. Corrected.
7. "Settings states that projects are stored unencrypted under the user's
   account": now decision 100's sentence. Corrected.
8. Run rows holding `spec_json` and `manifest_json`, an `artifacts.display_name`
   column: now the spec and the bundle (with its manifest) are artifacts, and
   no display name is stored (decision 101: no file name). Accepted with §3
   and ADR-004's 004b rows (decision 234).
9. The write order "file durable, then `BEGIN IMMEDIATE`": now `BEGIN
   IMMEDIATE` first and held to the end, as the step prompt says. Accepted
   with §5 (decision 234).
10. A delete preview with a one-use token, the PROV JSON export, the
    workspace check on project commands, the `RunSink` for durable reruns,
    ADR-010's disk row, and the privacy-matrix and vitest guards: built in
    5B-2 and 5C, except the `RunSink`. The confirmation (decision 124) is
    Accepted with §11, the PROV export with §7, the workspace check with
    §15, and the guards with §16 (decision 234). The `RunSink` is not built
    (decision 186; §6). ADR-010's disk row is Proposed.
11. Two edge tables named `run_inputs` and `run_outputs`, `artifacts` with an
    integer id: now `used`, `generated`, `derived`, keyed by hash. Accepted
    with §3 (decision 234).

## Revisions

- 19 Oct 2026, Phase 5B-3a (decision 141): §15 corrected to decision 135.
  It said closing a project is never gated (decision 49); closing needs
  Statistics active, and leaving Statistics closes the project. The
  commands as built in 5B-2 already did this (D282's table:
  `project_close` is gated).
- 19 Oct 2026, Phase 5B-3a (decision 146): Consequences corrected to
  decision 132. It recorded two app instances on one project as a
  limitation with only SQLite between them; they now share it through
  SQLite and the decision 132 lock, and a delete refuses while another copy
  has the project open (D283).
- 25 Oct 2026, Phase 5E-fix-2 (decision 234), on the 5E evaluation's
  evidence and 5E-fix-1's (D291): §1's resolution on macOS, §2, §3, §5 but
  its Windows sentence, §6, §7, §9, §10 for Unix, §11 but its Windows
  clause, §15 and §16 marked Accepted, with the evaluation's text changes
  (§3's version 2, `rerun_of` and migration; §6's decision 186 and N1; §7's
  `wasInformedBy`, the derived `wasDerivedFrom` and N10; §9's skipped
  `-wal`/`-shm`, the checkpoints and the 5.2 s wait; §11 as 5E-fix-1 built
  it). §1 on Windows and Linux, the Windows sentences of §5, §10 and §11,
  and decision 110's retirement marked Not yet; §13's "in 5D" corrected to
  decision 221 (N6). Decision 158's items added as Proposed: the Stored
  data section in §13, with its numbers from D291 and 5E-fix-2's item 1,
  and §17. The "said more" list marked as its sections now are.
