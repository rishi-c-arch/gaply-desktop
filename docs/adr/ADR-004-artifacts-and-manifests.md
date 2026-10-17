# ADR-004: Artifacts and run manifests

**Status: Proposed**, except section 004a, which is **Accepted (17 Oct
2026, decision 5)**. Written 17 Oct 2026, Research OS Phase 2B, from the
Phase 0 audit's proposal (`~/gaply-corpus/os/phase0/phase0_audit.md` §6.6,
§7) as updated by what Phases 2A and 2B built.

## Context

No artifact store exists. The only content hash in the product before Phase
2 was Level 1's `DataFingerprint`, and the verifier's rule is that no row is
kept past the run. The blueprint (§10) asks for immutable artifacts, a
provenance graph and a reproducibility manifest; Prompt 0 forbids persisting
user data until Rishi approves where it is stored, how it is protected and
how the user deletes it.

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

### Everything else: blocked

Run records, datasets, profiles and result bundles stay in memory
(ADR-003): at most 8 records and 4 slots, dropped on eviction, close or
exit. An artifact store (files under `<app_data>/artifacts/<sha256>`,
written atomically, indexed in SQLite with PROV-style edges) remains the
proposal of the Phase 0 audit and is **blocked until the DATA ON DISK
approval** of location, protection, retention and deletion.

### Every operation that would write user data, and its status

| operation | what it writes | status | through a save dialog? |
|---|---|---|---|
| export the result table (CSV) | headers, level labels, summary numbers | **allowed (004a)** | yes, the user names the file |
| export the run manifest (JSON) | hashes, versions, counts | **allowed (004a)** | yes, the user names the file |
| keep run records past the process | run states, manifests | blocked until approval | no |
| keep datasets or profiles on disk | rows, headers, labels | blocked until approval | no |
| an artifact store and provenance graph | results, manifests, edges | blocked until approval | no |
| a project file | all of the above | blocked until approval | would be, if built |
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

## Consequences

- A user can repeat a run and compare the output hash; nothing on disk links
  two runs unless the user keeps both exports.
- Blueprint §8.5's "persist terminal state even if the UI closes" is not met
  until storage is approved.
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

## Revision trigger

Rishi's approval of storage location, protection, retention and deletion;
or a platform reporting other output bytes than the pinned hashes.
