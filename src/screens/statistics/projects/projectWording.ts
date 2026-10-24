// Gaply — the project screens' words (Research OS, Phase 5B-3a; decisions
// 132 to 134). One plain sentence for every project.* code Rust can send
// (projectCodes.json, pinned to ProjectCode::ALL by
// src-tauri/tests/project_commands.rs), one fallback for anything else, and
// decision 133's sentences word for word.
//
// A refusal is always shown as its code's sentence. A raw message is never
// shown or logged: not the error's text, and not a JSON parse error, whose
// message can quote its input. No sentence here says anything about the
// user's data or passes a verdict on it.
import { workspaceRefusal } from '../../workspace/workspaceTypes';
import { workspaceSentence } from '../../workspace/workspaceWording';
import type { NameRefusal } from './projectName';
import { PROJECT_CODES, type ProjectCode, type ProjectRefusalView } from './projectTypes';

/* ---- decision 133, word for word ------------------------------------------ */

export const SAVE_SENTENCE =
  "Saving keeps a copy of the data file, exactly as it is, in this app's folder on this computer. Gaply uploads none of it. You can delete the project at any time.";

export const REPORT_EXPORT_SENTENCE =
  "This report holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. The project's name is its title.";

export const PROVENANCE_EXPORT_SENTENCE =
  "This file holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. It does not hold the project's name.";

export const DELETE_SENTENCE =
  "Deleting removes this project and everything in it from this app's folder. It cannot remove copies in Time Machine or other backups, in file-system snapshots, or files you exported. The disk may keep traces until the space is reused.";

export const SETTINGS_SENTENCE =
  "Projects are kept in this app's folder on this computer. Other user accounts on this computer cannot open them, though administrators, the apps you run and your backups can. Gaply does not encrypt them: they rely on your computer's disk encryption (FileVault on a Mac, BitLocker on Windows, LUKS on Linux). Turn it on in your computer's settings if it is off.";

/* ---- sizes ----------------------------------------------------------------- */

/** The limits, as fixed words (decisions 163 and 174). The caps in code are
 *  100 MiB and 10 GiB (decision 107), in binary bytes; these name less than
 *  that, so no sentence promises more than the app allows. They are never
 *  formatted from a cap or a refusal's limit. The byte counts beside them
 *  exist only so a test can pin that each is no more than its cap. */
export const PROJECT_LIMIT = '100 MB';
export const ALL_PROJECTS_LIMIT = '10 GB';
export const PROJECT_LIMIT_BYTES = 100000000;
export const ALL_PROJECTS_LIMIT_BYTES = 10000000000;

/** Bytes in decimal MB, as the Mac's Finder shows them (decision 163), to
 *  two decimals, unit included. 0 bytes is "0 MB"; above 0 and below 0.01
 *  MB is "under 0.01 MB" (decision 174). */
export function mb(bytes: number): string {
  if (bytes === 0) return '0 MB';
  if (bytes < 10000) return 'under 0.01 MB';
  return `${(bytes / 1000000).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })} MB`;
}

/* ---- every code ------------------------------------------------------------ */

type Say = (r: ProjectRefusalView) => string;

const AGAIN = 'Choose Delete again for a new confirmation.';

export const PROJECT_WORDS: Record<ProjectCode, Say> = {
  'project.bad_project_id': () => 'Gaply did not recognise this project.',
  'project.bad_hash': () => "A stored file's fingerprint was not in the expected form, so the file was not used.",
  'project.bad_title': () => 'This name cannot be used. A name has 1 to 100 characters and no hidden characters.',
  'project.bad_agent_version': () => 'This project records an app version that Gaply cannot read.',
  'project.bad_format': () => 'A file of this kind cannot be kept in a project.',
  'project.root_unavailable': () => "Gaply could not reach its projects folder on this computer.",
  'project.not_found': () => 'This project is no longer on this computer.',
  'project.symlink_refused': () => "Part of this project's folder is a link to somewhere else, so Gaply did not use it.",
  'project.db_missing': () => "This project's record file is missing, so it cannot be read. You can still delete it.",
  'project.db_unreadable': () => "This project's record file cannot be read. You can still delete it.",
  'project.db_newer_schema': () => 'This project was saved by a newer version of Gaply, so this version cannot read it.',
  'project.busy': () => 'This project is busy with another task. Try again in a moment.',
  'project.still_open': () => 'This project is open, so it was not deleted. Close it first.',
  'project.run_not_finished': () => 'This run has not finished, so it cannot be saved yet.',
  'project.unknown_artifact': () => 'A file this project refers to is not in it.',
  'project.hash_mismatch': () => 'A file did not match its fingerprint, so nothing was saved.',
  'project.artifact_missing': () => 'A file in this project is missing.',
  'project.artifact_not_regular': () => 'A file in this project is not an ordinary file, so it was not read.',
  'project.artifact_wrong_size': () => 'A file in this project has changed size since it was saved, so it was not read.',
  'project.artifact_wrong_hash': () => 'A file in this project no longer matches its fingerprint, so it was not read.',
  'project.prov_cycle': () => "This project's record of what came from what has a loop, so it cannot be exported.",
  'project.prov_unlinked_derivation': () => "This project's record of what came from what is incomplete, so it cannot be exported.",
  'project.project_cap': () => `Saving this would take the project past its limit of ${PROJECT_LIMIT}, so nothing was saved.`,
  'project.all_projects_cap': () =>
    `Saving this would take all projects together past their limit of ${ALL_PROJECTS_LIMIT}, so nothing was saved.`,
  'project.disk_full': () => "This computer's disk is full, so nothing was saved.",
  'project.io_failed': () => 'Gaply could not read or write a file in its projects folder.',
  'project.db_failed': () => "Gaply could not update this project's record file.",
  'project.run_interrupted': () => 'This saved run was interrupted before it finished.',
  'project.not_open': () => 'That project is not open in this window.',
  'project.run_unknown': () => 'Gaply does not know this run, so it cannot be saved.',
  'project.run_evicted': () => 'This run is no longer held in memory, so it cannot be saved. Run it again to save it.',
  'project.run_cancelled': () => 'This run was cancelled, so it cannot be saved.',
  'project.run_not_saveable': () => 'Only a run that finished, or one that failed with its reason, can be saved.',
  'project.run_failed_without_code': () => 'This run failed without a recorded reason, so it cannot be saved.',
  'project.run_already_saved': () => 'This run is already saved in this project.',
  'project.data_unavailable': () =>
    'The data file this run used is no longer open, so it cannot be saved with it. Open the file and run it again to save it.',
  'project.data_mismatch': () => 'The data file open now is not the one this run used, so nothing was saved.',
  'project.confirmation_missing': () => `This delete had no confirmation. ${AGAIN}`,
  'project.confirmation_unknown': () => `This delete's confirmation was not recognised. ${AGAIN}`,
  'project.confirmation_used': () => `This delete's confirmation was already used. ${AGAIN}`,
  'project.confirmation_replaced': () => `A newer confirmation replaced this one. ${AGAIN}`,
  'project.confirmation_expired': () => `This delete's confirmation is more than five minutes old. ${AGAIN}`,
  'project.confirmation_other_project': () => `This confirmation was for another project. ${AGAIN}`,
  'project.stored_run_unknown': () => 'This project holds no saved run with that number.',
  'project.result_too_large': () => 'This saved result is too large to show here.',
  'project.open_elsewhere': () => 'Another copy of Gaply has this project open, so it was not deleted. Close it there first.',
  'project.being_deleted': () => 'Another copy of Gaply is deleting this project, so it was not opened.',
  'project.lock_failed': () =>
    'Gaply could not check whether another copy of the app is using this project, so it did nothing.',
  // 5C-1a: reruns (decisions 185 to 190).
  'project.rerun_unsupported': () =>
    'This result was made by a method, or with settings, that this version of Gaply cannot run again, so nothing was run.',
  'project.rerun_not_succeeded': () => 'Only a result that finished can be run again.',
  'project.rerun_busy': () => 'Another run is going. Wait for it to finish, or cancel it, then run this again.',
  'project.rerun_refused': () => "This result's settings were not accepted this time, so nothing was run.",
  'project.rerun_in_progress': () => 'A run is still going in this project. Wait for it to finish, or cancel it, first.',
  'project.rerun_data_unreadable': () =>
    'The copy of the data file kept in this project could not be read this time, so nothing was run.',
};

/** For a refusal this table does not know. */
export const FALLBACK_SENTENCE = 'Gaply could not do this, and gave no reason this screen knows.';

const isProjectCode = (c: string): c is ProjectCode => (PROJECT_CODES as readonly string[]).indexOf(c) >= 0;

/** A refusal's code and numbers, or null. Reads the error's message as the
 *  refusal JSON Rust sends; never returns or keeps the message itself. */
export function projectRefusal(e: unknown): ProjectRefusalView | null {
  const message =
    typeof e === 'string'
      ? e
      : e && typeof e === 'object' && typeof (e as { message?: unknown }).message === 'string'
        ? (e as { message: string }).message
        : null;
  if (message === null) return null;
  let v: unknown;
  try {
    v = JSON.parse(message);
  } catch {
    return null;
  }
  if (!v || typeof v !== 'object') return null;
  const o = v as { code?: unknown; needed?: unknown; limit?: unknown };
  if (typeof o.code !== 'string' || !isProjectCode(o.code)) return null;
  const out: ProjectRefusalView = { code: o.code };
  if (typeof o.needed === 'number') out.needed = o.needed;
  if (typeof o.limit === 'number') out.limit = o.limit;
  return out;
}

/** One sentence for any failed project call: the project code's, the
 *  workspace code's (from the workspace table), or the fallback. */
export function projectSentence(e: unknown): string {
  const r = projectRefusal(e);
  if (r) return PROJECT_WORDS[r.code](r);
  const w = workspaceRefusal(e);
  // A workspace sentence reads the refusal's parameters; one that arrives
  // without them gives the fallback rather than a throw (Phase 5B-3b).
  if (w) {
    try {
      return workspaceSentence(w);
    } catch {
      return FALLBACK_SENTENCE;
    }
  }
  return FALLBACK_SENTENCE;
}

/* ---- names ----------------------------------------------------------------- */

export const NAME_WORDS: Record<NameRefusal, string> = {
  empty: 'Type a name for the new project.',
  too_long: 'A name can have at most 100 characters.',
  hidden_character: 'This name has a hidden or control character, such as a direction mark. Type it again without one.',
};

/* ---- the screens' own words ------------------------------------------------ */

export const PROJECT_TEXT = {
  saveButton: 'Save to a project…',
  saveTitle: 'Save to a project',
  whereToSave: 'Where to save this run',
  newProject: 'A new project',
  openProject: 'The open project:',
  nameLabel: 'Name of the new project',
  saveConfirm: 'Save',
  saving: 'Saving…',
  cancel: 'Cancel',
  // The delete dialog's Cancel, once a refusal shows (decision 175).
  closeDialog: 'Close',
  savedNew: 'Saved. This run is the first in the new project',
  savedInto: 'Saved. This run was added to the project',
  panelHeading: 'Projects',
  loading: 'Reading the projects on this computer…',
  empty: 'No projects yet. Save a result to make one.',
  unread: 'The projects on this computer could not be read.',
  unreadable: 'This project cannot be read.',
  openNow: 'Open now',
  savedOn: 'Saved',
  results: (n: number) => (n === 1 ? '1 saved run' : `${n} saved runs`),
  delete: 'Delete…',
  deleteTitle: 'Delete a project',
  deleteConfirm: 'Delete',
  deleting: 'Deleting…',
  deleted: 'The project was deleted.',
  unnamed: 'A project that cannot be read',
  settingsHeading: 'Projects',
  // Phase 5B-3b: Open, the project page, the saved result view and the exports.
  open: 'Open',
  opening: 'Opening…',
  pageHeading: 'Project',
  pageLoading: 'Reading this project…',
  pageUnread: 'This project could not be read.',
  tryAgain: 'Try again',
  pageInactive: 'A project is shown only while the Statistics workspace is open.',
  pageNotFound: 'There is no such project on this computer.',
  pageNotOpen: 'This project is not open.',
  backToStatistics: 'Back to Statistics',
  close: 'Close the project',
  closing: 'Closing…',
  savedResults: 'Saved results',
  runsLoading: 'Reading the saved results…',
  runsUnread: "This project's saved results could not be read.",
  runsEmpty: 'This project holds no saved results.',
  openResult: 'Open this result',
  backToResults: 'Back to the saved results',
  resultLoading: 'Reading this saved result…',
  exportReport: 'Export report…',
  exportProvenance: 'Export provenance…',
  exportReportTitle: 'Export the project report',
  exportProvenanceTitle: 'Export the provenance file',
  exportConfirm: 'Export',
  exporting: 'Exporting…',
  exportCancelled: 'Nothing was saved: the save dialog was closed.',
  unknownState: 'In a state this version of Gaply does not know',
} as const;

/* ---- the project page (Phase 5B-3b) ---------------------------------------- */

/** What a saved run is, by its capability. A capability this version does not
 *  know is named as such, never shown as its id. */
export function capabilityName(capabilityId: string): string {
  if (capabilityId === 'stats.describe') return 'Descriptive summary';
  if (capabilityId === 'stats.analyze.independent_t') return 'Independent-samples t-test';
  return 'An analysis this version of Gaply does not know';
}

/** The line above a saved result (decision 150). */
export const savedResultLine = (secs: number) => `Saved result, saved ${savedOn(secs)}.`;

/** The saved result view's sentences, one per case it cannot show
 *  (decision 150). None of them repeats anything from the stored result. */
export const SAVED_TEXT = {
  unknownCapability: 'This result was saved by an analysis this version of Gaply does not know, so it is not shown.',
  unknownSchema: 'This result was saved in a form this version of Gaply does not read, so it is not shown.',
  badShape: 'This saved result is not in the form Gaply expects, so it is not shown.',
  summaryHidden: 'Descriptive summaries are not shown in this build of Gaply, so this saved result is not shown.',
  cannotShow: 'This saved result could not be shown.',
  failed: 'This run failed. Gaply saved its reason:',
  dataFile: (format: string, bytes: number) => `Data file: ${format.toUpperCase()} file, ${mb(bytes)}.`,
} as const;

/** The export dialog's and the export's own sentences (decision 151). */
export const EXPORT_WORDS = {
  wrongName: 'Gaply did not save this export: it did not come with the file name expected.',
  wrongHash: 'Gaply did not save this export: its bytes did not match their fingerprint (SHA-256).',
  notExact: 'Gaply did not save this export: it could not be saved exactly as Gaply made it.',
  writeFailed: 'The file could not be written where you chose. Nothing else was changed.',
  saved: (sha256: string) => `Saved. SHA-256 of the file: ${sha256.slice(0, 12)}…`,
} as const;

/** The storage line: the total against both limits (decisions 163 and 174),
 *  worded so it cannot be read as free disk space. */
export function totalsSentence(totalBytes: number): string {
  return `Your projects use ${mb(totalBytes)} of their ${ALL_PROJECTS_LIMIT} limit. A project can hold up to ${PROJECT_LIMIT}.`;
}

/** A project's size, for a row and the delete dialog. */
export const sizeSentence = (bytes: number) => `Size: ${mb(bytes)}.`;

/** A saved time, in seconds since the epoch, in this computer's time zone. */
export function savedOn(secs: number): string {
  return new Date(secs * 1000).toLocaleString('en-GB', {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}
