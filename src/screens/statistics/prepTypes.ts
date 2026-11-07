// Gaply — scale scores' wire types (Research OS, Phase 8T-1b, decision 499).
// TypeScript mirrors of the recipe src-tauri/src/stats_prep_commands.rs takes
// and of the two views it returns (PrepMadeView and PrepRecipeView). Each view
// is pinned by a JSON fixture in ./fixtures (prep*.json), which
// src-tauri/tests/stats_prep_screen_wire.rs takes through IPC on a synthetic
// file and compares byte for byte, and prepTypes.vitest.ts deep-compares with
// a typed copy. Through IPC their keys come out sorted.
//
// What these hold: aliases, numbers, closed codes, and the one text a recipe
// has, the name the user typed for a score, for the user's own screen. Never a
// header, a row or a path.
//
// No screen uses these yet: the scale score's dialog is 8T-2's.
import type { ScaleEnds } from './alphaTypes';
// Preparation reaches these types inside DatasetView, and ExportView is the
// bridge's (prepBridge.ts); both are describeTypes', never copied.
import type { DatasetView, ParamKey, ResourceEstimate, ScoreCounts, Severity, ValidationStatus } from './describeTypes';

/** Every refusal, note and count of scale scores (stats_prep::PrepCode). Kept
 *  equal to prepCodes.json's stats.prep words by prepTypes.vitest.ts. */
export type PrepCode =
  | 'stats.prep.recipe_unreadable'
  | 'stats.prep.unknown_capability'
  | 'stats.prep.capability_version_mismatch'
  | 'stats.prep.engine_version_mismatch'
  | 'stats.prep.schema_version_mismatch'
  | 'stats.prep.source_already_prepared'
  | 'stats.prep.source_columns_mismatch'
  | 'stats.prep.no_steps'
  | 'stats.prep.alias_not_next'
  | 'stats.prep.too_wide'
  | 'stats.prep.cells_over_cap'
  | 'stats.prep.name_not_trimmed'
  | 'stats.prep.name_length'
  | 'stats.prep.name_has_invisible_character'
  | 'stats.prep.name_equals_file_header'
  | 'stats.prep.name_equals_other_score'
  | 'stats.prep.name_is_an_alias_word'
  | 'stats.prep.too_few_items'
  | 'stats.prep.item_repeated'
  | 'stats.prep.items_out_of_order'
  | 'stats.prep.item_not_a_file_column'
  | 'stats.prep.item_not_numeric'
  | 'stats.prep.item_holds_text'
  | 'stats.prep.item_non_finite'
  | 'stats.prep.reversed_repeated'
  | 'stats.prep.reversed_out_of_order'
  | 'stats.prep.reversed_not_an_item'
  | 'stats.prep.scale_missing'
  | 'stats.prep.scale_without_reversed'
  | 'stats.prep.scale_not_finite'
  | 'stats.prep.scale_ends_out_of_order'
  | 'stats.prep.reversed_outside_scale'
  | 'stats.prep.at_least_out_of_range'
  | 'stats.prep.missing_code_not_finite'
  | 'stats.prep.missing_code_repeated'
  | 'stats.prep.missing_codes_out_of_order'
  | 'stats.prep.statistic_not_chosen'
  | 'stats.prep.rule_not_chosen'
  | 'stats.prep.negative_zero'
  | 'stats.prep.no_row_scored'
  | 'stats.prep.table_refused_column'
  | 'stats.prep.sum_over_fewer_items'
  | 'stats.prep.scored_from_every_item'
  | 'stats.prep.scored_from_fewer'
  | 'stats.prep.not_scored_too_few'
  | 'stats.prep.not_scored_overflow';

/** The most characters a score's name may have, counted by code point, as Rust
 *  counts them. Pinned to stats_prep::NAME_MAX_CHARS by
 *  src-tauri/tests/stats_prep_screen_wire.rs, which reads this line. */
export const SCORE_NAME_MAX_CHARS = 128;

/** What a score is, of the values a row has. */
export type Statistic = 'sum' | 'mean';

/** When a row has a score: every item has a value, or at least n do, n from 1
 *  to the number of items less one. */
export type MissingRule = 'every_item' | { at_least: number };

/** One score. `statistic` and `rule` are null until the user chooses each; the
 *  scale's ends only when an item is reverse-scored; codes ascending. */
export interface ScaleScoreStep {
  kind: 'scale_score';
  /** This score's column, C(k + j). */
  alias: string;
  /** As the user typed it, trimmed. The one text in a recipe. */
  name: string;
  /** The file's columns, in column order. */
  items: string[];
  /** A subset of the items, in the same order. */
  reversed: string[];
  scale: ScaleEnds | null;
  statistic: Statistic | null;
  rule: MissingRule | null;
  missingCodes: number[];
}

/** The steps that make a prepared table from a file's. */
export interface Recipe {
  schemaVersion: 1;
  capabilityId: 'stats.prepare.scale_score';
  capabilityVersion: 1;
  engineVersion: 1;
  /** How many columns the file has. */
  sourceColumns: number;
  steps: ScaleScoreStep[];
}

/** A finding about a recipe: a closed code with numeric parameters. */
export interface PrepDiagnostic {
  code: PrepCode;
  params: Partial<Record<ParamKey, number>>;
  severity: Severity;
}

/** A make's answer: the report, each score's counts, and the prepared view
 *  when the slot now holds it. A refused make has no dataset, and any counts
 *  are of the score no row can have. */
export interface PrepMadeView {
  status: ValidationStatus;
  errors: PrepDiagnostic[];
  warnings: PrepDiagnostic[];
  missingInformation: PrepDiagnostic[];
  estimatedResources: ResourceEstimate;
  counts: ScoreCounts[];
  dataset: DatasetView | null;
}

/** The recipe a slot holds and its counts. */
export interface PrepRecipeView {
  recipe: Recipe;
  counts: ScoreCounts[];
}

/* ---- the screen's number rule ------------------------------------------- */

/**
 * A number the user typed, as it may enter a recipe: a finite number, with -0
 * sent as 0 (decision 413's answer 4); or null for an empty, unreadable or
 * non-finite field, which never enters a recipe.
 *
 * Rust refuses -0 (stats.prep.negative_zero) and non-finite ends and codes,
 * but JSON.stringify writes -0 as "0" and NaN or an infinity as null, so
 * neither can reach Rust from here: only a test of the built value can show
 * this rule (prepTypes.vitest.ts).
 */
export function typedNumber(text: string): number | null {
  const t = text.trim();
  if (t === '') return null;
  const v = Number(t);
  if (!Number.isFinite(v)) return null;
  return v === 0 ? 0 : v;
}
