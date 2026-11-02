// Gaply — Cronbach's alpha's words (Research OS, Phase 8I-2, decision 384).
// Every code alpha's screen can show has one sentence here, keyed by the
// closed set in alphaCodes.json, which src-tauri/tests/stats_alpha_wire.rs
// pins to AlphaCode, RunCode, WorkspaceCode and CapabilityCode exactly. A
// refusal from the file ingest is said by methodsIngestWording, as on the
// correlations' screen.
//
// The tone is neutral (decision 66): alpha describes how the ticked items'
// totals vary against their parts; nothing here passes or fails it. No
// sentence names a threshold or calls a value acceptable, good or poor, and
// Gaply says which items the user reverse-scored, never which they should
// reverse or drop. alphaWording.vitest.ts reads every sentence for the words a
// verdict would use.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { methodsIngestSentence } from './methodsIngestWording';
import type { WireCode } from '../statsverifier/statsL1Types';
import { workspaceSentence } from '../workspace/workspaceWording';
import type { WorkspaceRefusal } from '../workspace/workspaceTypes';
// The run manager's ceiling and the runs it keeps, pinned to
// runs::RUN_CEILING_MS and runs::MAX_RUNS.
import LIMITS from './describeLimits.json';
import type { Stage } from './describeWording';
import type { AlphaInterval, AlphaStat } from './alphaTypes';
import { shownNumber } from './ttestWording';
import { count } from './corrWording';

export const ALPHA_STAGE_HEADINGS: Record<Stage, string> = {
  import: 'Import a data file',
  profile: 'Profile of the file',
  settings: 'Choose the items, any to reverse-score, and the missing codes',
  validate: 'Check the settings',
  run: "Run Cronbach's alpha",
  results: 'Results',
  export: 'Export',
};

export const ALPHA_ENTRY_TEXT =
  "Cronbach's alpha on your own data: alpha and standardized alpha for two or more numeric items you tick, with alpha's 95% interval, the inter-item correlations and each item's statistics. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.";

export const ITEMS_QUESTION = 'Items. None is ticked unless you tick it; tick at least two numeric columns.';

export const REVERSE_LABEL = 'Reverse-score';

export const LOWEST_LABEL = 'Lowest value on the scale';
export const HIGHEST_LABEL = 'Highest value on the scale';

/** Beside the scale's two fields (decision 384), word for word. */
export const REVERSE_SENTENCE =
  'Reverse-scoring turns each value x into lowest + highest − x. Type the ends your questionnaire allows, for example 1 and 5, not the lowest and highest answers in your data. Every reverse-scored item uses these ends.';

/** A typed scale end as a number: a finite number, or null for an empty or
 *  unreadable field. Never taken from the data. */
export function scaleEnd(text: string): number | null {
  const t = text.trim();
  if (t === '') return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
}

/** What is still missing before the settings can be checked, or null. */
export function settingsMissingSentence(items: number, reversed: number, lowest: string, highest: string): string | null {
  const parts: string[] = [];
  if (items < 2) parts.push(items === 1 ? 'one more item' : 'two items');
  if (reversed > 0) {
    if (scaleEnd(lowest) === null) parts.push('a number for the lowest value on the scale');
    if (scaleEnd(highest) === null) parts.push('a number for the highest value on the scale');
  }
  if (parts.length > 0) {
    const list = parts.length === 1 ? parts[0] : `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
    return `To check the settings, choose ${list}.`;
  }
  const lo = scaleEnd(lowest);
  const hi = scaleEnd(highest);
  if (reversed > 0 && lo !== null && hi !== null && !(lo < hi)) {
    return 'To check the settings, type a lowest value on the scale that is below the highest.';
  }
  return null;
}

/** The run's progress, in what it counts (decision 380). */
export function stepsSentence(done: number, total: number): string {
  return `${done} of ${total} steps`;
}

/** A column's name in a sentence, from the user's own profile on the user's
 *  own screen, or by position on a saved result. */
export interface AlphaNames {
  column: (alias: number) => string;
}
const NO_NAMES: AlphaNames = { column: (a) => `C${a}` };

/** A code with its parameters, and an exclusion's row count. */
export interface Coded {
  code: string;
  params?: Record<string, unknown>;
  count?: number;
}

const num = (d: Coded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const cells = (n: number | undefined) => (n === 1 ? '1 cell' : `${n ?? '?'} cells`);
const rows = (n: number | undefined) => (n === 1 ? '1 row' : `${n === undefined ? '?' : count(n)} rows`);
const was = (n: number | undefined) => (n === 1 ? 'was' : 'were');
const values = (n: number | undefined) => (n === 1 ? '1 value' : `${n === undefined ? '?' : count(n)} values`);

type Words = (d: Coded, names: AlphaNames) => string;

const LIFECYCLE_NAMES: Record<string, string> = {
  planned: 'planned',
  experimental: 'experimental',
  beta: 'in beta',
  stable: 'stable',
  deprecated: 'retired',
};

const workspace: Words = (d) => workspaceSentence({ code: d.code, params: d.params ?? {}, severity: 'error' } as WorkspaceRefusal);

const column = (d: Coded, n: AlphaNames) => n.column(num(d, 'column') ?? 0);

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; alphaWording.vitest.ts keeps every code in alphaCodes.json
 *  covered. */
export const ALPHA_WORDS: Record<string, Words> = {
  'stats.alpha.unknown_capability': () => 'These settings name a capability Gaply does not have here.',
  'stats.alpha.capability_version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of Cronbach's alpha, and this is version ${num(d, 'expected') ?? '?'}.`,
  'stats.alpha.input_count': (d) => `These settings name ${num(d, 'count') ?? '?'} files, and Cronbach's alpha reads exactly one.`,
  'stats.alpha.input_mismatch': () => 'These settings were made for a different file from the one open. Open the file again.',
  'stats.alpha.preparation_mismatch': () => 'These settings do not match how the open data was prepared. Choose the settings again.',
  'stats.alpha.code_equals_score': (d, n) =>
    `Column ${column(d, n)} is a score, and it holds a missing code you ticked in ${cells(num(d, 'count'))}. Untick the code, or leave this score out.`,
  'stats.alpha.unknown_column': (d, n) => `Column ${column(d, n)} is not in this file.`,
  'stats.alpha.too_few_items': (d) =>
    `${num(d, 'count') === 1 ? '1 item is' : `${num(d, 'count') ?? '?'} items are`} ticked, and Cronbach's alpha needs at least two.`,
  'stats.alpha.item_repeated': (d, n) => `Column ${column(d, n)} is ticked twice as an item.`,
  'stats.alpha.items_out_of_order': (d, n) =>
    `These settings list item ${column(d, n)} out of the file's column order, so Gaply refuses them rather than reorder them.`,
  'stats.alpha.reversed_repeated': (d, n) => `Column ${column(d, n)} is reverse-scored twice.`,
  'stats.alpha.reversed_out_of_order': (d, n) =>
    `These settings list the reverse-scored item ${column(d, n)} out of the file's column order, so Gaply refuses them rather than reorder them.`,
  'stats.alpha.reversed_not_an_item': (d, n) => `Column ${column(d, n)} is reverse-scored but is not one of the items.`,
  'stats.alpha.scale_without_reversed': () =>
    "These settings give the scale's ends, but no item is reverse-scored, so Gaply refuses them rather than ignore the ends.",
  'stats.alpha.scale_missing': () => "An item is reverse-scored, but the scale's lowest and highest values are not given.",
  'stats.alpha.scale_not_finite': () => "A scale end is not a finite number.",
  'stats.alpha.scale_ends_out_of_order': () => "The scale's lowest value is not below its highest.",
  'stats.alpha.missing_code_not_finite': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is not a finite number.`,
  'stats.alpha.missing_code_repeated': (d) => `Missing code ${(num(d, 'index') ?? 0) + 1} is listed twice.`,
  'stats.alpha.missing_codes_out_of_order': (d) =>
    `Missing code ${(num(d, 'index') ?? 0) + 1} is out of ascending order, so Gaply refuses these settings rather than reorder them.`,
  'stats.alpha.item_holds_text': (d, n) =>
    `Column ${column(d, n)} holds text in ${cells(num(d, 'count'))}, so it cannot be used as a numeric item.`,
  'stats.alpha.item_non_finite': (d, n) =>
    `Column ${column(d, n)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be used as a numeric item.`,
  'stats.alpha.suspect_missing_code': (d, n) =>
    `Column ${column(d, n)} holds ${num(d, 'value') ?? '?'} in ${cells(num(d, 'count'))}, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.`,
  'stats.alpha.reversed_outside_scale': (d, n) =>
    `${column(d, n)} has ${values(num(d, 'count'))} outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends, or tick a missing code.`,
  'stats.alpha.too_few_complete_rows': (d) =>
    `${rows(num(d, 'count'))} ${num(d, 'count') === 1 ? 'has' : 'have'} a value in every ticked item, and Cronbach's alpha needs at least 3. Tick fewer items, or check the missing codes.`,
  'stats.alpha.total_constant': () => "Not computable: the items' total is the same number in every row used.",
  'stats.alpha.item_constant': () =>
    'Not computable: an item is the same number in every row used, so a correlation with it is not defined.',
  'stats.alpha.standardized_not_defined': () =>
    'Not computable: 1 + (k − 1) times the mean inter-item correlation is 0 or below, so standardized alpha is not defined.',
  'stats.alpha.others_constant': () => 'Not computable: the total of the other items is the same number in every row used.',
  'stats.alpha.two_items': () => 'Not computable: with two items, leaving one out leaves a single item, which has no alpha.',
  'stats.alpha.distribution_out_of_range': () =>
    "Not computable: Feldt's interval needs the F distribution past 100,000 rows used or 10,000,000 degrees of freedom, where Gaply has not measured its accuracy.",
  'stats.alpha.overflow': () => 'Not computable: the arithmetic went past the largest number a double can hold.',
  'stats.alpha.underflow': () => 'Not computable: a sum of squares went below the smallest number a double can hold, though the values differ.',
  'stats.alpha.arithmetic_refused': () => 'Not computable: the arithmetic refused to give a value.',
  'stats.alpha.missing_value': (d, n) =>
    `${rows(d.count)} had no value (an empty cell or a missing-value token) in ${column(d, n)}, the first item without one, and ${was(d.count)} left out.`,
  'stats.alpha.missing_code': (d, n) =>
    `${rows(d.count)} held a missing code you ticked in ${column(d, n)}, the first item without a value, and ${was(d.count)} left out.`,
  'stats.alpha.negative_alpha': () =>
    "Alpha is negative: the items' average covariance is negative. An item may need to be reverse-scored.",
  'stats.alpha.assumes_one_dimension': () => 'Alpha assumes the items measure one thing; Gaply does not check that.',
  'stats.alpha.no_omega': () => "McDonald's omega and squared multiple correlations are not shown.",
  'run.unknown_run': () => `This run is no longer held. Gaply keeps the last ${LIMITS.maxRuns} runs in memory only; run Cronbach's alpha again.`,
  'run.run_busy': () => 'Another run is going in this workspace. Wait for it to end, or cancel it.',
  'run.unknown_input': () => 'This file is no longer open. Open it again.',
  'run.not_finished': () => 'This run has not ended yet.',
  'run.no_result': () => 'This run ended without a result.',
  'run.cancelled': () => 'This run was cancelled, so it has no result.',
  'run.timed_out': () => `This run passed its ${LIMITS.runCeilingMs / 1000}-second limit and was stopped, so it has no result.`,
  'run.resource_limit': () => 'This run passed a resource limit and was stopped, so it has no result.',
  'run.illegal_transition': () => 'Gaply could not change this run in that way.',
  'run.already_terminal': () => 'This run has already ended.',
  'run.wrong_kind': () => "This run is not a Cronbach's alpha run, so this screen cannot read it.",
  'capability.unknown_capability': () => 'Gaply does not have this method in this version.',
  'capability.version_mismatch': (d) =>
    `These settings were made for version ${num(d, 'found') ?? '?'} of the method, and this is version ${num(d, 'expected') ?? '?'}.`,
  'capability.not_runnable': (d) => {
    const l = d.params?.lifecycle;
    return `This method is ${typeof l === 'string' ? LIFECYCLE_NAMES[l] ?? l : 'not available'}, so it cannot run in this version of Gaply.`;
  },
  'capability.confirmation_missing': () => 'This method needs a confirmation that these settings do not carry.',
  'capability.confirmation_unmatched': () => 'These settings carry a confirmation this method does not use.',
  'workspace.unavailable': workspace,
  'workspace.not_ready': workspace,
  'workspace.inactive': workspace,
  'workspace.wrong_workspace': workspace,
  'workspace.run_active': workspace,
  'workspace.illegal_transition': workspace,
};

/** The sentence for a code: a diagnostic, an exclusion, a note or a
 *  limitation. */
export function alphaSentence(d: Coded, names: AlphaNames = NO_NAMES): string {
  const f = ALPHA_WORDS[d.code];
  return f ? f(d, names) : `Gaply has no wording for the code ${d.code}.`;
}

/** One plain sentence for a command's refusal. */
export function alphaRefusalSentence(e: unknown): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${message.trim()}` : 'This could not be done.';
  }
  if (c.code in ALPHA_WORDS) {
    const params = (c as { params?: unknown }).params;
    return alphaSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} });
  }
  return methodsIngestSentence(c as WireCode);
}

/** A candidate missing code in the ticked items, as a question naming the
 *  value and where it appears. A ticked code applies to every item. */
export function alphaMissingCodeQuestion(code: number, where: ReadonlyArray<{ header: string; cells: number }>): string {
  const places = where.map((w) => `"${w.header}" (${cells(w.cells)})`).join(', ');
  return `Is ${code} a code for a missing value? It appears in ${places}. If you tick it, it is treated as missing in every item.`;
}

/** Why a run that ended without a result has none. */
export function alphaRunEndedSentence(state: string): string {
  switch (state) {
    case 'cancelled':
      return alphaSentence({ code: 'run.cancelled' });
    case 'timed_out':
      return alphaSentence({ code: 'run.timed_out' });
    case 'resource_limit':
      return alphaSentence({ code: 'run.resource_limit' });
    case 'refused':
      return 'Gaply refused these settings, so nothing ran. The reasons are below.';
    case 'needs_user_input':
      return 'The settings need an answer from you, so nothing ran. See below.';
    case 'failed':
      return "Cronbach's alpha could not be computed. The reason is below.";
    default:
      return alphaSentence({ code: 'run.no_result' });
  }
}

/** A number, or its sentence where its number would be (decision 384). */
export function alphaStatText(s: AlphaStat): string {
  return 'value' in s ? shownNumber(s.value) : alphaSentence({ code: s.not_computable });
}

/** Alpha's interval: its two ends, or its sentence. */
export function alphaIntervalText(i: AlphaInterval): string {
  return 'computed' in i ? `${shownNumber(i.computed.lower)} to ${shownNumber(i.computed.upper)}` : alphaSentence({ code: i.not_computable });
}

export const ALPHA_DIGITS_NOTE =
  'Numbers are shown rounded to six figures, counted from the first digit that is not zero; the CSV export keeps every digit.';

export const ALPHA_EXPORT_CHOICES = [
  { format: 'csv', label: 'The result tables (CSV)', file: 'cronbach_alpha.csv', mime: 'text/csv' },
  { format: 'manifestJson', label: 'The run manifest (JSON)', file: 'cronbach_alpha_manifest.json', mime: 'application/json' },
] as const;

export const ALPHA_EXPORT_TEXT =
  "Gaply writes only the file you name in the save dialog and keeps no copy. The result tables name the items by position (C1, C2, …), never by header, with every digit, each reverse-scored item marked 1 and every other 0, and the manifest's hashes. The scale's ends you typed are settings, not results, so they are not in the tables; a project's report gives them. The manifest holds hashes, versions and counts, never your data, headers or file name.";

export const ALPHA_ROWS_USED_NOTE =
  'The rows with a value in every ticked item. Rows read is rows used plus the rows left out above.';
