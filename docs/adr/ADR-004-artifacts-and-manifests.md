# ADR-004: Artifacts and run manifests

**Status: Proposed**, except section 004a, which is **Accepted (17 Oct
2026, decision 5)**, and section 004b, which is **Accepted (19 Oct 2026,
decisions 99 to 111) for what those decisions cover, and (25 Oct 2026,
decision 234) for its spec, bundle, run-row and export-bytes rows and its
manifest-schema sentence**; the operations table's rerun row and the
consequence about blueprint §8.5 are **Accepted (decision 234)** too. Each
part of 004b is marked. Written 17 Oct 2026, Research OS Phase 2B, from the Phase 0 audit's
proposal (`~/gaply-corpus/os/phase0/phase0_audit.md` §6.6, §7) as updated by
what Phases 2A and 2B built; revised 19 Oct 2026 (Phase 5A draft, made to say
what decisions 99 to 111 say in Phase 5B-1, decision 112). The history is at
the end.

## Context

No artifact store exists. The only content hash in the product before Phase
2 was Level 1's `DataFingerprint`, and the verifier's rule is that no row is
kept past the run. The blueprint (§10) asks for immutable artifacts, a
provenance graph and a reproducibility manifest; Prompt 0 forbids persisting
user data until Rishi approves where it is stored, how it is protected and
how the user deletes it.

Revision of 19 Oct 2026: Phase 4 produced a second capability with its own
manifest and export (`stats_ttest`, D277, D278); Level 0 and Level 1 still
produce no `ResultBundle` and no manifest (decision 50: "their ResultBundle
envelope and manifest wait for Phase 5's provenance"); and decisions 99 to
111 approved where a project lives, how it is protected, how long it is kept
and how it is deleted (ADR-009).

## Decision

### What a manifest holds (decision 12)

A `RunManifest` (`gaply_core::contracts`) holds the capability id and
version, the engine version, each input's content hash (SHA-256 of the bytes
read), the parameters' hash, the seed, rows read and used, exclusions as
closed codes with counts and a column position, the platform, the elapsed
time, the output's hash (SHA-256 of `DescribeResult::canonical_bytes`) and
the reproducibility claim. **No path, no file name, no header, no level
label, no cell.** It is shown on the user's own screen and goes into an
export the user saves; it never goes into an outbound struct or a log.

"Bitwise" means the same inputs, engine version and platform give the same
bytes (decision 15). Whether other platforms agree is asked, not claimed:
`gaply-core/tests/desc_output_hashes.rs` pins three output hashes, and the
Linux and Windows CI jobs go red if they compute other bytes.

### 004a: the save-dialog export — Accepted, 17 Oct 2026 (decision 5)

The user triggers an export through the OS save dialog, and the existing
frontend save path (`src/utils/saveTextFile.ts`: the dialog plugin, then the
fs plugin's `writeTextFile`) writes only the file the user named. Rust
writes nothing: `stats_desc_export` returns the bytes and their SHA-256, and
the screen writes them only after re-encoding the text and finding it equal
to those bytes. Gaply keeps no copy. The suggested file names are fixed
(`descriptive_summary.csv`, `descriptive_summary_manifest.json`) and do not
carry the data file's name.

### 004b: a manifest and artifacts, stored inside a project

Run records, datasets, profiles and result bundles of unsaved work stay in
memory (ADR-003): at most 8 records and 4 slots, dropped on eviction, close
or exit (decision 6). **[Accepted, decision 105]** Nothing is written to a
project except when the user clicks Save; there is no autosave.

When the user saves a run to a project (ADR-009), these are stored under
`projects/<id>/`:

| what | where | holds user data? | status |
|---|---|---|---|
| the input data file, as the ingest read it | an artifact file, `artifacts/<sha256>`, kind `data_file`, its format a closed code | **yes**: rows, headers, labels | **[Accepted, decision 101]**: copied in, identified by its SHA-256, within the ingest caps, deleted with the project; its path and file name never stored |
| the `OperationSpec` | an artifact file, kind `operation_spec` (canonical JSON) | no header or label: indices, a variant, a seed, hashes | **[Accepted, decision 234]** |
| the `ResultBundle` (`DescribeResult` or `TTestResult`, and the `RunManifest` inside it) | an artifact file, kind `result_bundle` (canonical JSON) | **yes**: headers and level labels (`stats_desc/mod.rs:45-46`, `:253`, `:308`); the one place they are kept (5E evaluation §8) | **[Accepted, decision 234]** |
| the run | a row of `runs` in `project.db`: ids, the capability id and version, the state, closed codes, times; its links to the artifacts as edge rows, by hash | no | **[Accepted, decision 234]** |
| a Level 0 or Level 1 run (5C) | the sentences it checked inside the run's bundle artifact, never as a row | **yes**: manuscript sentences | **[Accepted, decision 102]**; the save sentence says so |
| an AI explanation | not stored in Phase 5 | — | **[Accepted, decision 102]** |
| the export bytes | not stored: regenerated from the bundle; the manifest's `output_hash` pins what the export was | — | **[Accepted, decision 234]** |

An artifact is never modified and never overwritten (ADR-009 §5). The data
file's display name stays on the user's own screen (decision 4) and is never
stored. **[Accepted, decision 234]** The manifest's own schema stays at
version 1 in 5B, and was still 1 after 5C-1 (`SchemaVersion::NUMBER`, and
`KNOWN_SCHEMA_VERSION` in `storedShape.ts`; 5E evaluation §1); 5C-2, which
gives Level 0 and Level 1 their bundles, may add fields, and whether that is
a schema bump is decided then by ADR-003's rule.

### Every operation that would write user data, and its status

| operation | what it writes | status | through a save dialog? |
|---|---|---|---|
| export the result table (CSV) | headers, level labels, summary numbers | **allowed (004a)** | yes, the user names the file |
| export the run manifest (JSON) | hashes, versions, counts | **allowed (004a)** | yes, the user names the file |
| create a project | a folder, `project.db`, the title | **approved (decisions 99, 100, 103, 106)**; built in 5B-1 with no caller, reachable from 5B-2 | no: an in-app form |
| save a run to a project | the data file, spec and bundle artifacts, the run row and its edges | **approved (decisions 101, 105, 107)**; built in 5B-1 with no caller, reachable from 5B-2 | no: a click, with the sentence that says a copy is kept on this computer only |
| recovery when a project opens | removes the store's own leftovers, marks states; never adds user data | **approved (decision 105)**; built in 5B-1 | no |
| delete a project | removes all of the above, through a tombstone | **approved (decision 104)**; built in 5B-1 with no caller; the confirmation is 5B-2's | no: an in-app confirmation |
| export the project report, export the provenance graph (JSON) | hashes, versions, counts, codes; ids, kinds, states, times | Proposed, 5B-2 (decision 111) | yes |
| rerun from a project (5C) | one write at its end (decision 186): its run row with `rerun_of`, its edges and its new bundle artifact; no row and no transition while it runs, and nothing on a cancel, refusal, quit or crash | **Accepted (decision 234)**; built in 5C-1 | no: a Run again click, with decision 191's sentence |
| save a Level 0 or Level 1 run (5C) | a bundle holding the sentences checked | the storage rule is approved (decision 102); the feature is 5C's | no |
| export or import a project as one file | a file the user names | later (decision 108) | would be |
| the import-guard rate (`cache` key `import:seconds_per_page`) | a running mean of seconds per page, no user data | existing, outside this slice: not written by the descriptive summary's path (measured, D272) | no |

## Alternatives considered

- **Rust writes the export to a path the webview passes.** Rejected: it
  would make a Rust command a file writer for any path the webview names,
  and the save path that every other export uses already exists.
- **A project file as one JSON blob.** Rejected by blueprint §10.1, and
  blocked anyway.
- **Copy to clipboard only, until the ADR.** Offered in the audit (Q5);
  decision 5 approved the save dialog instead.
- **The display name in the manifest.** Rejected (decision 4): a file name
  is often the manuscript's or the dataset's own title; the manifest carries
  the content hash.
- **Store the export bytes as an artifact too** (revision). Rejected: they
  are a function of the bundle and the format; the output hash in the
  manifest pins what the export was.
- **Store the spec, the manifest or the bundle as rows** (revision).
  Rejected: headers and labels, and column positions tied to them, would
  then be in `project.db`; rows hold ids, hashes, codes and times only.
- **Store a manifest for every in-memory run automatically** (revision).
  Rejected by decision 105.

## Consequences

- A user can repeat a run and compare the output hash. Until 5B-2 wires the
  store to a Save, nothing on disk links two runs unless the user keeps both
  exports.
- **[Accepted, decision 234]** Blueprint §8.5's "persist terminal state
  even if the UI closes" is met for saved runs since 5B-2, and for a rerun
  started from a project only if the app lives to its end: a quit or crash
  while it runs persists nothing (decision 186, 5C-1). It is not met for
  unsaved runs, by decision 105.
- The privacy matrix (`src-tauri/tests/stats_desc_privacy_matrix.rs`) checks
  that the manifest, in the view and as exported bytes, holds none of seven
  sentinels, and that the app-data folder is byte-identical after the whole
  flow.

## Evidence

- `gaply-core/tests/desc_export.rs` (Phase 2A): the manifest holds no header,
  label, cell or path; R reads the CSV back bit-identical.
- `src-tauri/tests/stats_desc_writes_nothing.rs`: no file write, create,
  rename, copy or removal in `runs.rs`, `stats_desc_commands.rs`,
  `contracts/` or `stats_desc/`; mutation-tested.
- `src/screens/statistics/DescribeFlow.vitest.tsx`: the export goes through
  a mocked save dialog and writes exactly the returned bytes; a closed
  dialog writes nothing.
- 004b: Phase 5B-1's store tests and guards (D281; ADR-009 Evidence).

## Revision trigger

A platform reporting other output bytes than the pinned hashes; Phase 5C's
Level 0 and Level 1 bundles; or an export or import format (decision 108).

## History

- 17 Oct 2026: written, Proposed (Research OS Phase 2B). Section 004a
  Accepted the same day (decision 5).
- 19 Oct 2026: revised. Phase 5A drafted the revision
  (`~/gaply-corpus/os/phase5/ADR-004-revision.draft.md`). Phase 5B-1 made it
  say what decisions 99 to 111 say (decision 112): "Everything else:
  blocked" became 004b, Accepted where those decisions cover it; the
  operations table records each operation's approval. Where the draft said
  more than the decisions, and what this ADR says now: (a) the spec and
  manifest as `runs.spec_json` and `runs.manifest_json`: now the spec and the
  bundle are artifacts and rows hold no JSON, Proposed; (b) the project
  report and provenance exports in Phase 5B: now 5B-2 (decision 111),
  Proposed; (c) a writer guard naming `gaply-core/src/project/store.rs`: the
  store is `src-tauri/src/project_store.rs` in the app crate, as the 5B-1
  step prompt says; (d) the privacy matrices gaining a saved project and
  `desc_output_hashes.rs` pinning the bundle artifact's hash: 5B-2, Proposed,
  not in this revision's Evidence; (e) the data file "or a hash-only
  reference": settled as a copy by decision 101.
- 25 Oct 2026, Phase 5E-fix-2 (decision 234), on the 5E evaluation's
  evidence: 004b's spec, bundle, run-row and export-bytes rows and its
  manifest-schema sentence marked Accepted; the operations table's rerun row
  rewritten to what 5C-1 built (one write at the end, decision 186) and
  Accepted; the §8.5 consequence corrected in its direction (a rerun's
  terminal state persists only if the app lives to its end) and Accepted.
  Not covered by decision 234, and unchanged: "What a manifest holds" and
  the operations table's row for the project report and provenance exports.
