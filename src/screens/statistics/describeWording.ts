// Gaply — the descriptive summary's words (Research OS, Phase 2B). Every
// code a command or a view can carry has one sentence here, keyed by the
// closed set in describeCodes.json, which a Rust test pins to DescCode and
// RunCode exactly. A refusal from the file ingest is Level 1's own code, so
// its sentence is Level 1's (l1Wording's sentenceFor): one reading of the
// file, one set of words for its refusals.
//
// The tone is neutral: a summary describes data, it does not pass or fail
// it, so nothing here congratulates, and "Could not compute" is a reason,
// not an error.
import { refusalOf, sentenceFor } from '../statsverifier/l1/l1Wording';
// The run manager's ceiling, pinned to runs::RUN_CEILING_MS by
// src-tauri/tests/stats_desc_wire.rs (evaluation of Phases 2 and 3, F11).
import LIMITS from './describeLimits.json';
import type { WireCode } from '../statsverifier/statsL1Types';
import type { ColumnKind, Diagnostic, Platform, Reproducibility, RunState, ValidationStatus } from './describeTypes';

/** The wizard's stages, in order: blueprint §4.4's Inputs → Settings →
 *  Validation → Run → Results, with Inputs split into Import and Profile and
 *  Export after Results, as Phase 2B's prompt sets them. */
export const STAGES = [
  { id: 'import', label: 'Import' },
  { id: 'profile', label: 'Profile' },
  { id: 'settings', label: 'Settings' },
  { id: 'validate', label: 'Validate' },
  { id: 'run', label: 'Run' },
  { id: 'results', label: 'Results' },
  { id: 'export', label: 'Export' },
] as const;
export type Stage = (typeof STAGES)[number]['id'];

export const STAGE_HEADINGS: Record<Stage, string> = {
  import: 'Import a data file',
  profile: 'Profile of the file',
  settings: 'Choose the columns to summarise',
  validate: 'Check the settings',
  run: 'Run the summary',
  results: 'Results',
  export: 'Export',
};

export const ENTRY_TEXT =
  'A descriptive summary of a data file: for each column you choose, n, missing, mean, standard deviation, minimum, quartiles and maximum, or the count of each level. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.';

/** Decision 17: one sentence on how levels are read. */
export const LEVELS_RULE =
  "Levels follow Level 1's reading of cells: 1 and 1.0 are one level, and text is trimmed of spaces at either end.";

export const KIND_NAMES: Record<ColumnKind, string> = {
  numeric: 'Numeric',
  categorical: 'Categorical',
  empty: 'Empty',
};

export const STATUS_SENTENCES: Record<ValidationStatus, string> = {
  valid: 'The settings can run.',
  valid_with_warnings: 'The settings can run. Read the notes below first.',
  needs_user_input: 'The settings need an answer from you before they can run.',
  unsupported: 'Gaply cannot run these settings.',
  invalid: 'The settings cannot run as they are. Change what is listed below.',
};

/** What each run state is called on screen. Neutral: a finished summary is
 *  "Finished", never a success. */
export const RUN_STATE_NAMES: Record<RunState, string> = {
  draft: 'Draft',
  validating: 'Checking the settings',
  needs_user_input: 'Needs your input',
  awaiting_confirmation: 'Waiting for confirmation',
  refused: 'Refused',
  validated: 'Checked',
  invalidated: 'No longer valid',
  queued: 'Queued',
  running: 'Running',
  succeeded: 'Finished',
  failed: 'Failed',
  cancelled: 'Cancelled',
  timed_out: 'Stopped at the time limit',
  resource_limit: 'Stopped at a resource limit',
  numerical_failure: 'Stopped by a numerical failure',
};

/** A column's name in a sentence: its header, which is the user's own text
 *  on the user's own screen. */
export type ColumnName = (alias: number) => string;
const NO_HEADERS: ColumnName = (a) => `C${a}`;

const num = (d: { params: Diagnostic['params'] }, k: keyof Diagnostic['params']): number | undefined => {
  const v = d.params[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? '?'} cells`);

type Words = (d: Diagnostic, col: ColumnName) => string;

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; describeWording.vitest.ts keeps every code covered. */
export const WORDS: Record<string, Words> = {
  'stats.desc.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.desc.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the summary, and this is version ${num(d, 'expected') ?? '?'}.`,
  'stats.desc.input_count': (d) => `These settings name ${num(d, 'count') ?? '?'} files, and the summary reads exactly one.`,
  'stats.desc.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.desc.no_columns_selected': () => 'No column is chosen. Choose at least one column to summarise.',
  'stats.desc.unknown_column': (d, col) => `Column ${col(num(d, 'column') ?? 0)} is not in this file.`,
  'stats.desc.duplicate_column': (d, col) => `Column ${col(num(d, 'column') ?? 0)} is chosen twice.`,
  'stats.desc.numeric_on_text': (d, col) =>
    `Column ${col(num(d, 'column') ?? 0)} holds text in ${cells(num(d, 'count'))}, so it cannot be summarised as numeric. Choose Categorical for it.`,
  'stats.desc.non_finite_cell': (d, col) =>
    `Column ${col(num(d, 'column') ?? 0)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be summarised as numeric.`,
  'stats.desc.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.desc.suspect_missing_code': (d, col) =>
    `Column ${col(num(d, 'column') ?? 0)} holds ${num(d, 'value') ?? '?'} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.desc.no_values': () => 'The column has no values.',
  'stats.desc.single_value': () => 'A standard deviation needs at least two values.',
  'stats.desc.overflow': () => 'The arithmetic went past the largest number a double can hold.',
  'stats.desc.underflow': () => 'The arithmetic went below the smallest number a double can hold, though the values differ.',
  'stats.desc.arithmetic_refused': () => 'The arithmetic refused to give a value.',
  'stats.desc.missing_cell': (d, col) =>
    `${cells(num(d, 'count'))} of column ${col(num(d, 'column') ?? 0)} were empty or held a missing-value token, and were left out.`,
  'stats.desc.missing_code': (d, col) =>
    `${cells(num(d, 'count'))} of column ${col(num(d, 'column') ?? 0)} held a missing code you ticked, and were left out.`,
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run the summary again.`,
  'run.run_busy': () => 'A summary is already running. Wait for it to end, or cancel it.',
  'run.unknown_input': () => 'This file is no longer open. Open it again.',
  'run.not_finished': () => 'This run has not ended yet.',
  'run.no_result': () => 'This run ended without a result.',
  'run.cancelled': () => 'This run was cancelled, so it has no result.',
  'run.timed_out': () =>
    `This run passed its ${LIMITS.runCeilingMs / 1000}-second limit and was stopped, so it has no result.`,
  'run.resource_limit': () => 'This run passed a resource limit and was stopped, so it has no result.',
  'run.illegal_transition': () => 'Gaply could not change this run in that way.',
  'run.already_terminal': () => 'This run has already ended.',
  'run.wrong_kind': () => 'This run is not a descriptive summary, so this screen cannot read it.',
};

/** The sentence for a diagnostic. */
export function diagnosticSentence(d: Diagnostic, col: ColumnName = NO_HEADERS): string {
  const f = WORDS[d.code];
  return f ? f(d, col) : `Gaply has no wording for the code ${d.code}.`;
}

/** "Could not compute" and why, for a statistic. */
export function notComputableReason(code: string): string {
  const f = WORDS[code];
  return f ? f({ code: code as Diagnostic['code'], params: {}, severity: 'note' }, NO_HEADERS) : `No reason was recorded (${code}).`;
}

export const COULD_NOT_COMPUTE = 'Could not compute';

/** One plain sentence for a command's refusal: the run manager's codes in
 *  these words, the ingest's in Level 1's. */
export function refusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in WORDS) return WORDS[c.code]({ code: c.code as Diagnostic['code'], params: {}, severity: 'error' }, NO_HEADERS);
  return sentenceFor(c as WireCode);
}

/** A candidate missing code, as a question naming the value (decision on
 *  missing codes: never applied unless ticked). */
export function missingCodeQuestion(code: number, where: Array<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, every ${code} in the columns you summarise as numeric is left out.`;
}

/** Why a run that ended without a result has none. */
export function runEndedSentence(state: RunState): string {
  const said = (code: string) => WORDS[code]({ code: 'stats.desc.no_values', params: {}, severity: 'note' }, NO_HEADERS);
  switch (state) {
    case 'cancelled':
      return said('run.cancelled');
    case 'timed_out':
      return said('run.timed_out');
    case 'resource_limit':
      return said('run.resource_limit');
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return 'The run failed, so it has no result.';
    default:
      return said('run.no_result');
  }
}

export function progressSentence(done: number, total: number): string {
  return `${done} of ${total} ${total === 1 ? 'column' : 'columns'} summarised.`;
}

/** The status view's elapsed time runs from the moment the run was made,
 *  when Run was pressed; the manifest's runs from the computation's start
 *  to its end (evaluation of Phases 2 and 3, F6). Each says which it is. */
export function elapsedSentence(ms: number): string {
  return `Elapsed since you pressed Run: ${(ms / 1000).toFixed(1)} s`;
}

export function computeTimeSentence(ms: number): string {
  return `${(ms / 1000).toFixed(3)} s, from the start of the computation to its end`;
}

/** Rows used always equals rows read in this summary (F6). */
export const ROWS_USED_NOTE =
  "The same as rows read: this summary leaves out cells, never whole rows. Each column's Missing count says how many.";

/** A number as shown in the table: six significant figures. The export
 *  keeps every digit. */
export function shown(v: number): string {
  return String(Number.parseFloat(v.toPrecision(6)));
}

export const SHOWN_DIGITS_NOTE = 'Numbers are shown to six significant figures; the CSV export keeps every digit.';

/** A hash shortened for the screen: the first 12 hex digits. */
export function shortHash(hex: string): string {
  return hex.length > 12 ? `${hex.slice(0, 12)}…` : hex;
}

export function platformName(p: Platform): string {
  const os: Record<Platform['os'], string> = { macos: 'macOS', windows: 'Windows', linux: 'Linux', other: 'another system' };
  return `${os[p.os]}, ${p.arch}`;
}

export function reproducibilitySentence(r: Reproducibility): string {
  if (r === 'bitwise') return 'Bitwise: the same file, settings, engine version and platform give the same bytes.';
  if (r === 'not_reproducible') return 'Not reproducible.';
  return `Within a relative tolerance of ${r.within_tolerance.relative}.`;
}

export const EXPORT_CHOICES = [
  { format: 'csv', label: 'The result table (CSV)', file: 'descriptive_summary.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'descriptive_summary_manifest.json', mime: 'application/json' },
] as const;

export const EXPORT_TEXT =
  'Gaply writes only the file you name in the save dialog and keeps no copy. The manifest holds hashes, versions and counts, never your data, headers or file name.';
