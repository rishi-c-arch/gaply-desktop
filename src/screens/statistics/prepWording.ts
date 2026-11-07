// Gaply — scale scores' words (Research OS, Phase 8T-1b, decision 499).
// Every code a scale score's command can give has one sentence here, keyed by
// the closed set in prepCodes.json, which src-tauri/tests/stats_prep_screen_wire.rs
// pins to PrepCode, the two run codes the prep commands give, WorkspaceCode and
// CapabilityCode exactly. The four count codes have no sentence of their own:
// a score's counts are said by scoreCountsSentence.
//
// The tone is plain (decision 66): each sentence says what is the case and,
// where it helps, what the user can change, with "take … out", "leave … out",
// "list it once" or "must". Nothing here calls a score good or poor, and no
// sentence echoes a scale end or a missing code from the recipe. A name codes'
// sentence says "the name" and never quotes it, since it may be empty or hold
// invisible characters. prepWording.vitest.ts reads every sentence.
//
// No screen uses these yet: the scale score's dialog is 8T-2's.
import { refusalOf } from '../statsverifier/l1/l1Wording';
import { workspaceSentence } from '../workspace/workspaceWording';
import type { WorkspaceRefusal } from '../workspace/workspaceTypes';
import { LIFECYCLE_LABELS } from './capabilityWording';
import { count } from './corrWording';
import type { DatasetView, ScoreCounts } from './describeTypes';
import type { Recipe } from './prepTypes';
import { columnNames, scoreCountsSentence } from './window/windowWording';

/**
 * Each code's parameter keys, as the code that sends them gives them
 * (gaply-core/src/stats_prep/mod.rs: name_errors, step_errors, validate and
 * build; src-tauri/src/stats_prep_commands.rs: make; the workspace's and the
 * registry's refusals). An `index` is a step's place in recipe.steps, but for
 * the three missing-code codes, whose `index` is a code's place in the last
 * step's codes. Every code in prepCodes.json but the four count codes is a key.
 */
export const PREP_PARAMS: Readonly<Record<string, readonly string[]>> = {
  'stats.prep.recipe_unreadable': [],
  'stats.prep.unknown_capability': [],
  'stats.prep.capability_version_mismatch': ['expected', 'found'],
  'stats.prep.engine_version_mismatch': ['expected', 'found'],
  'stats.prep.schema_version_mismatch': ['expected', 'found'],
  'stats.prep.source_already_prepared': [],
  'stats.prep.source_columns_mismatch': ['expected', 'found'],
  'stats.prep.no_steps': [],
  'stats.prep.alias_not_next': ['index', 'expected', 'found'],
  'stats.prep.too_wide': ['found', 'limit'],
  'stats.prep.cells_over_cap': ['found', 'limit'],
  'stats.prep.name_not_trimmed': ['index'],
  'stats.prep.name_length': ['index', 'count'],
  'stats.prep.name_has_invisible_character': ['index'],
  'stats.prep.name_equals_file_header': ['index', 'column'],
  'stats.prep.name_equals_other_score': ['index', 'from'],
  'stats.prep.name_is_an_alias_word': ['index'],
  'stats.prep.too_few_items': ['index', 'count'],
  'stats.prep.item_repeated': ['index', 'column'],
  'stats.prep.items_out_of_order': ['index', 'column'],
  'stats.prep.item_not_a_file_column': ['index', 'column'],
  'stats.prep.item_not_numeric': ['index', 'column'],
  'stats.prep.item_holds_text': ['index', 'column', 'count'],
  'stats.prep.item_non_finite': ['index', 'column', 'count'],
  'stats.prep.reversed_repeated': ['index', 'column'],
  'stats.prep.reversed_out_of_order': ['index', 'column'],
  'stats.prep.reversed_not_an_item': ['index', 'column'],
  'stats.prep.scale_missing': ['index'],
  'stats.prep.scale_without_reversed': ['index'],
  'stats.prep.scale_not_finite': ['index'],
  'stats.prep.scale_ends_out_of_order': ['index'],
  'stats.prep.reversed_outside_scale': ['index', 'column', 'count'],
  'stats.prep.at_least_out_of_range': ['index', 'found', 'limit'],
  'stats.prep.missing_code_not_finite': ['index'],
  'stats.prep.missing_code_repeated': ['index'],
  'stats.prep.missing_codes_out_of_order': ['index'],
  'stats.prep.statistic_not_chosen': ['index'],
  'stats.prep.rule_not_chosen': ['index'],
  'stats.prep.negative_zero': ['index'],
  'stats.prep.no_row_scored': ['column'],
  'stats.prep.table_refused_column': [],
  'stats.prep.sum_over_fewer_items': ['index'],
  'run.run_busy': [],
  'run.unknown_input': [],
  'workspace.unavailable': ['workspace'],
  'workspace.not_ready': ['workspace'],
  'workspace.inactive': ['expected', 'from'],
  'workspace.wrong_workspace': ['expected', 'found'],
  'workspace.run_active': ['run', 'workspace'],
  'workspace.illegal_transition': ['from', 'event', 'workspace'],
  'capability.unknown_capability': [],
  'capability.version_mismatch': ['expected', 'found'],
  'capability.not_runnable': ['lifecycle'],
  'capability.confirmation_missing': [],
  'capability.confirmation_unmatched': [],
};

/** The four count codes: words for a score's counts, said by
 *  scoreCountsSentence, never as a finding of their own. */
export const COUNT_CODES = [
  'stats.prep.scored_from_every_item',
  'stats.prep.scored_from_fewer',
  'stats.prep.not_scored_too_few',
  'stats.prep.not_scored_overflow',
] as const;

/** A code with its parameters, as a diagnostic or a refusal carries it. */
export interface PrepCoded {
  code: string;
  params?: Record<string, unknown>;
}

/** What a sentence may name things from: the slot's view (its columns, scores
 *  by the name typed), the recipe sent, and a refused make's counts. */
interface Context {
  view?: DatasetView | null;
  recipe?: Recipe | null;
  counts?: readonly ScoreCounts[];
}

const num = (d: PrepCoded, k: string): number | undefined => {
  const v = d.params?.[k];
  return typeof v === 'number' ? v : undefined;
};
const n = (d: PrepCoded, k: string): string => {
  const v = num(d, k);
  return v === undefined ? 'an unknown number' : count(v);
};
const cells = (v: number | undefined) => (v === 1 ? '1 cell' : v === undefined ? 'some cells' : `${count(v)} cells`);
const values = (v: number | undefined) => (v === 1 ? '1 value' : v === undefined ? 'some values' : `${count(v)} values`);

/** A name shown in quotes only when it is plain: not empty, no space at an end,
 *  no control or invisible character. A display choice, never Rust's rule: a
 *  name that is not plain is said by its place instead. */
function plain(name: string): boolean {
  if (name.length === 0 || name.trim() !== name) return false;
  for (let i = 0; i < name.length; i += 1) {
    const c = name.charCodeAt(i);
    if (c < 0x20 || (c >= 0x7f && c <= 0x9f)) return false;
    if (c === 0x00ad || c === 0x061c || c === 0x180e || c === 0xfeff) return false;
    if ((c >= 0x200b && c <= 0x200f) || (c >= 0x2028 && c <= 0x202e) || (c >= 0x2060 && c <= 0x206f)) return false;
    if (c >= 0xfff9 && c <= 0xfffb) return false;
  }
  return true;
}

/** recipe.steps[i] by its typed name, or by its place when there is no plain
 *  name to show. `capital` for the start of a sentence. */
function scoreName(ctx: Context, i: number | undefined, capital: boolean): string {
  const step = i === undefined ? undefined : ctx.recipe?.steps[i];
  if (step && plain(step.name)) return `"${step.name}"`;
  const word = capital ? 'This score' : 'this score';
  return i === undefined || !ctx.recipe ? word : `${capital ? 'Score' : 'score'} ${i + 1}`;
}

/** The step the dialog adds, last in the recipe: where the three missing-code
 *  codes' `index` points (stats_prep/mod.rs, step_errors). */
const lastStep = (ctx: Context) => (ctx.recipe && ctx.recipe.steps.length > 0 ? ctx.recipe.steps.length - 1 : undefined);

const stepAt = (d: PrepCoded) => num(d, 'index');
const Name = (d: PrepCoded, ctx: Context) => scoreName(ctx, stepAt(d), true);
const name = (d: PrepCoded, ctx: Context) => scoreName(ctx, stepAt(d), false);
const last = (ctx: Context) => scoreName(ctx, lastStep(ctx), false);

/** A column by the name the user sees, through columnNames (columnLabel, then
 *  C{n}). */
const col = (d: PrepCoded, ctx: Context) => {
  const c = num(d, 'column');
  return c === undefined ? 'a column' : columnNames(ctx.view)(c);
};
const Col = (d: PrepCoded, ctx: Context) => {
  const s = col(d, ctx);
  return s === 'a column' ? 'A column' : s;
};

/** A missing code's place in the last step's codes, 1-based. */
const code = (d: PrepCoded) => {
  const i = num(d, 'index');
  return i === undefined ? 'a missing code' : `missing code ${i + 1}`;
};

/** no_row_scored's column: the step whose alias it is, by its typed name. */
function scoreAt(d: PrepCoded, ctx: Context): string {
  const c = num(d, 'column');
  const i = ctx.recipe?.steps.findIndex((s) => s.alias === `C${c}`) ?? -1;
  if (i >= 0) return scoreName(ctx, i, false);
  return c === undefined ? 'this score' : columnNames(ctx.view)(c);
}

const lifecycle = (d: PrepCoded): string => {
  const l = d.params?.lifecycle;
  return typeof l === 'string' && l in LIFECYCLE_LABELS ? LIFECYCLE_LABELS[l as keyof typeof LIFECYCLE_LABELS] : 'not available';
};

const workspace = (d: PrepCoded) => workspaceSentence({ code: d.code, params: d.params ?? {}, severity: 'error' } as WorkspaceRefusal);

type Words = (d: PrepCoded, ctx: Context) => string;

/** One sentence per code. A code with no entry is shown as itself, never
 *  hidden; prepWording.vitest.ts keeps every code in prepCodes.json but the
 *  four count codes covered. */
export const PREP_WORDS: Readonly<Record<string, Words>> = {
  'stats.prep.recipe_unreadable': () => 'Gaply could not read this recipe.',
  'stats.prep.unknown_capability': () => 'This recipe is not for scale scores, so Gaply cannot make it.',
  'stats.prep.capability_version_mismatch': (d) =>
    `This recipe was written for version ${n(d, 'found')} of scale scores, and this is version ${n(d, 'expected')}.`,
  'stats.prep.engine_version_mismatch': (d) =>
    `This recipe was written for version ${n(d, 'found')} of how Gaply computes a score, and this is version ${n(d, 'expected')}.`,
  'stats.prep.schema_version_mismatch': (d) =>
    `This recipe is written in form ${n(d, 'found')}, and this version of Gaply reads form ${n(d, 'expected')}.`,
  'stats.prep.source_already_prepared': () => "A score must be made from the file's own columns, and this table already holds scores.",
  'stats.prep.source_columns_mismatch': (d) =>
    `This recipe was written for a file of ${n(d, 'found')} columns, and the open file has ${n(d, 'expected')}.`,
  'stats.prep.no_steps': () => 'This recipe has no score in it.',
  'stats.prep.alias_not_next': (d, ctx) =>
    `${Name(d, ctx)} must be column ${n(d, 'expected')}, the next after the file's columns and the scores before it. It was sent as column ${n(d, 'found')}.`,
  'stats.prep.too_wide': (d) => `With the scores, the table would have ${n(d, 'found')} columns, and Gaply holds at most ${n(d, 'limit')}.`,
  'stats.prep.cells_over_cap': (d) =>
    `With the scores, the table would have ${n(d, 'found')} cells, and this computer's limit is ${n(d, 'limit')}.`,
  'stats.prep.name_not_trimmed': () => 'The name starts or ends with a space. Take the space out.',
  'stats.prep.name_length': (d) =>
    num(d, 'count') === 0
      ? 'The name is empty. Type a name for the score.'
      : `The name has ${n(d, 'count')} characters. A name must have from 1 to 128.`,
  'stats.prep.name_has_invisible_character': () =>
    'The name holds a control character or an invisible one. Type the name again without it.',
  'stats.prep.name_equals_file_header': () => 'The name is already a column of the file. Give the score another name.',
  'stats.prep.name_equals_other_score': () => "The name is already another score's. Give this score another name.",
  'stats.prep.name_is_an_alias_word': () => "The name is written as a column's alias, such as C3. Give the score another name.",
  'stats.prep.too_few_items': (d, ctx) => `${Name(d, ctx)} needs at least 2 items. It has ${n(d, 'count')}.`,
  'stats.prep.item_repeated': (d, ctx) => `${Name(d, ctx)} lists ${col(d, ctx)} twice. List it once.`,
  'stats.prep.items_out_of_order': (d, ctx) =>
    `${Name(d, ctx)} lists ${col(d, ctx)} out of the file's column order, so Gaply refuses it rather than reorder the items.`,
  'stats.prep.item_not_a_file_column': (d, ctx) =>
    `${Col(d, ctx)} is not one of the file's own columns, so it cannot be an item of ${name(d, ctx)}. Leave it out.`,
  'stats.prep.item_not_numeric': (d, ctx) => `${Col(d, ctx)} holds no numbers, so it cannot be an item of ${name(d, ctx)}.`,
  'stats.prep.item_holds_text': (d, ctx) =>
    `${Col(d, ctx)} holds text in ${cells(num(d, 'count'))}, so it cannot be an item of ${name(d, ctx)}.`,
  'stats.prep.item_non_finite': (d, ctx) =>
    `${Col(d, ctx)} holds an infinity or "not a number" in ${cells(num(d, 'count'))}, so it cannot be an item of ${name(d, ctx)}.`,
  'stats.prep.reversed_repeated': (d, ctx) => `${Name(d, ctx)} reverse-scores ${col(d, ctx)} twice. List it once.`,
  'stats.prep.reversed_out_of_order': (d, ctx) =>
    `${Name(d, ctx)} lists the reverse-scored item ${col(d, ctx)} out of the file's column order, so Gaply refuses it rather than reorder the items.`,
  'stats.prep.reversed_not_an_item': (d, ctx) =>
    `${Name(d, ctx)} reverse-scores ${col(d, ctx)}, which is not one of its items. Add it as an item, or leave it out.`,
  'stats.prep.scale_missing': (d, ctx) =>
    `${Name(d, ctx)} has a reverse-scored item, so the scale's lowest and highest values must be given.`,
  'stats.prep.scale_without_reversed': (d, ctx) =>
    `${Name(d, ctx)} gives the scale's ends, but no item is reverse-scored, so Gaply refuses it rather than ignore the ends.`,
  'stats.prep.scale_not_finite': (d, ctx) => `For ${name(d, ctx)}, the scale's ends must be finite numbers.`,
  'stats.prep.scale_ends_out_of_order': (d, ctx) => `For ${name(d, ctx)}, the scale's lowest value must be below its highest.`,
  'stats.prep.reversed_outside_scale': (d, ctx) =>
    `For ${name(d, ctx)}, ${col(d, ctx)} has ${values(num(d, 'count'))} outside the scale's ends you typed, once the ticked missing codes are left out. Check the ends.`,
  'stats.prep.at_least_out_of_range': (d, ctx) => {
    const top = num(d, 'limit');
    return top !== undefined && top < 1
      ? `For ${name(d, ctx)}, the number of items a person needs can be set once the score has at least 2 items.`
      : `For ${name(d, ctx)}, the number of items a person needs must be from 1 to ${n(d, 'limit')}.`;
  },
  'stats.prep.missing_code_not_finite': (d, ctx) => `For ${last(ctx)}, ${code(d)} must be a finite number.`,
  'stats.prep.missing_code_repeated': (d, ctx) => `For ${last(ctx)}, ${code(d)} is listed twice. List it once.`,
  'stats.prep.missing_codes_out_of_order': (d, ctx) =>
    `For ${last(ctx)}, ${code(d)} is out of ascending order, so Gaply refuses the codes rather than reorder them.`,
  'stats.prep.statistic_not_chosen': (d, ctx) => `For ${name(d, ctx)}, choose whether the score is the sum or the mean of its items.`,
  'stats.prep.rule_not_chosen': (d, ctx) =>
    `For ${name(d, ctx)}, choose when a person gets a score: when every item has a value, or when at least a number of them do.`,
  'stats.prep.negative_zero': (d, ctx) =>
    `For ${name(d, ctx)}, a scale end or a missing code is zero with a minus sign. Zero must be sent without one.`,
  'stats.prep.no_row_scored': (d, ctx) => {
    const who = scoreAt(d, ctx);
    const first = `No row would get a score for ${who}, so it was not made.`;
    const c = ctx.counts?.[0];
    return c ? `${first} ${scoreCountsSentence(who, c)}` : first;
  },
  'stats.prep.table_refused_column': () => 'Gaply could not add the new column to the table. This is a fault in Gaply, not in the choices made.',
  'stats.prep.sum_over_fewer_items': (d, ctx) =>
    `For ${name(d, ctx)}, a person scored from fewer items has a sum over fewer items, which is not on the same scale as a sum over every item. Gaply does not rescale it.`,
  'run.run_busy': () =>
    'The scores cannot change while a run is going, or while another change to them is being made. Wait for it to end, then try again.',
  'run.unknown_input': () => 'This file is no longer open, or it has no scores to export yet.',
  'capability.unknown_capability': () => 'Gaply does not have scale scores in this version.',
  'capability.version_mismatch': (d) =>
    `This recipe was written for version ${n(d, 'found')} of scale scores, and this is version ${n(d, 'expected')}.`,
  'capability.not_runnable': (d) => `Scale scores are listed as ${lifecycle(d)} in this version of Gaply, so they cannot be made here.`,
  'capability.confirmation_missing': () => 'Scale scores need a confirmation that this recipe does not carry.',
  'capability.confirmation_unmatched': () => 'This recipe carries a confirmation that scale scores do not use.',
  'workspace.unavailable': workspace,
  'workspace.not_ready': workspace,
  'workspace.inactive': workspace,
  'workspace.wrong_workspace': workspace,
  'workspace.run_active': workspace,
  'workspace.illegal_transition': workspace,
};

/**
 * The sentence for a finding of a make: `view` is the slot's DatasetView, which
 * names the file's columns and the scores already held; `recipe` is the recipe
 * sent, which names each step; `counts` is the refused make's counts, which
 * stats.prep.no_row_scored says.
 */
export function prepSentence(
  d: PrepCoded,
  view?: DatasetView | null,
  recipe?: Recipe | null,
  counts?: readonly ScoreCounts[]
): string {
  const f = PREP_WORDS[d.code];
  return f ? f(d, { view, recipe, counts }) : `Gaply has no wording for the code ${d.code}.`;
}

/** The sentence for an error that carries no code Gaply knows. Nothing from
 *  the error's text is shown: it could quote what was sent. */
export const PREP_UNCODED = 'Gaply could not do this, and gave no reason it can show here.';

/** One plain sentence for a thrown refusal of a make, a clear, a recipe or a
 *  csv. Clear, recipe and csv send no recipe, so only the view names
 *  anything. */
export function prepRefusalSentence(e: unknown, view?: DatasetView | null): string {
  const c = refusalOf(e);
  if (!c) return PREP_UNCODED;
  const params = (c as { params?: unknown }).params;
  return prepSentence({ code: c.code, params: params && typeof params === 'object' ? (params as Record<string, unknown>) : {} }, view);
}
