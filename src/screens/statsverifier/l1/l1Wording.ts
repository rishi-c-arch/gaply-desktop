// Gaply — Level 1 wording (Phase 2, Step 7). The ONE place a Level 1 code
// becomes words a researcher reads.
//
// `WORDS` is keyed by the shared code set, `l1Codes.json`, which a gaply_core
// test (`tests/stats_l1_code_set.rs`) pins to exactly the codes
// `gaply_core::stats_l1::codes` can emit; `l1Wording.vitest.ts` requires one
// entry here per code in that file and no others. Reason and note codes read
// as one plain sentence; the few that NAME something (a variant, a number
// kind, a source) read as a short name, because they are used inside other
// sentences.
//
// `REFUSALS` words the closed codes the Level 1 commands reject with
// (`stats_l1_commands.rs`): those are command errors, not result codes, so
// they are kept apart from the shared set.
//
// Phase 1's rules hold for every string here: no score, no percentage, no
// gauge, and nothing says the paper is wrong.
import type { WireCode } from '../statsL1Types';

/** What a sentence may need to know: how to name a column on this device. */
export interface WordContext {
  /** `C3 anxiety_post` for alias 3; the alias alone when unknown. */
  column(alias: number): string;
}

export const NO_COLUMNS: WordContext = { column: (a) => `C${a}` };

type Words = (c: WireCode, w: (inner: WireCode | undefined) => string, ctx: WordContext) => string;

/** A parameter as a nested code: a bare word, or a code with parameters. */
export function nested(c: WireCode, key: string): WireCode | undefined {
  const v = c[key];
  if (typeof v === 'string') return { code: v };
  if (v && typeof v === 'object') return v as WireCode;
  return undefined;
}

export function num(c: WireCode, key: string): number | undefined {
  const v = c[key];
  return typeof v === 'number' ? v : undefined;
}

const plural = (n: number | undefined, one: string, many: string) => `${n ?? '?'} ${n === 1 ? one : many}`;

/** "the 2nd degrees of freedom" style ordinal for a zero-based index. */
function dfName(c: WireCode): string {
  const i = num(c, 'index');
  return i === undefined ? 'the degrees of freedom' : `degrees of freedom ${i + 1}`;
}

const lead = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const tail = (s: string) => (s.endsWith('.') ? s.slice(0, -1) : s);

/** The stored cells' rounding a data-precision code names: whole numbers
 *  for 0 decimals (evaluation fixes, part C, B-1). */
function roundingOf(c: WireCode | undefined): string {
  const d = c ? num(c, 'decimals') : undefined;
  if (d === 0) return 'rounding to whole numbers';
  return d === undefined ? 'rounding' : `rounding to ${plural(d, 'decimal', 'decimals')}`;
}

/** Bytes as megabytes, for a reader. */
export const megabytes = (b: number | undefined) => (b === undefined ? '?' : `${Math.round(b / (1024 * 1024))} MB`);

/** Bytes as megabytes to one decimal: a file's size beside its limit (part C
 *  follow-up, minor wording). */
const megabytesOne = (b: number | undefined) => (b === undefined ? '?' : `${(b / (1024 * 1024)).toFixed(1)} MB`);

export const WORDS: Record<string, Words> = {
  /* ---- Level 0's notes and reasons, carried into Level 1 ----------------- */
  one_tailed_stated: () => 'Consistent as a one-tailed test, which the paper states.',
  floor_printed_as_equality: () => 'The p is printed as an equality at a floor such as .001; the exact p is smaller still.',
  double_rounded: () => 'Matches if the paper rounded a number the software had already rounded (for example .1249 shown as .125, printed .13).',
  one_tailed_unstated: () => 'These numbers agree only if the test was one-tailed, and the paper does not say it was.',
  f_two_tailed: () =>
    'These numbers agree only if the p was taken from both tails of the F distribution, and an F test uses the upper tail only.',
  estimate_direction: () =>
    'The effect size lies outside its interval but would lie inside it with its sign reversed, so the direction may be printed inconsistently.',
  other_input_kind: () => 'This check does not apply to this kind of result.',
  no_estimate_and_interval: () => 'The result has no estimate printed with a confidence interval.',
  interval_not_after_estimate: () => 'The interval is not printed right after its estimate, so the two were not paired.',
  df_not_fixed_by_n: () => 'For this test the degrees of freedom are not fixed by the sample size.',
  no_sample_size_in_unit: () => 'No sample size is printed with this result.',
  partial_correlation: () => 'This is a partial correlation, whose degrees of freedom depend on covariates not printed here.',
  granularity_unknown: () =>
    'This check needs to know the data are whole numbers; Gaply never assumes that, so it was not run.',
  adjusted_p: () =>
    'The printed p is larger than the recomputed one and the paper names a correction for multiple testing, so it may be an adjusted p.',
  invalid_input: () => 'The printed numbers do not define this computation, so it was not made.',
  uninformative: () => 'Every value at the printed precision is possible here, so a match would say nothing.',
  df_beyond_measured_range: () =>
    "The degrees of freedom are above 100,000, where Gaply's p-value calculation has not been checked against its reference, so the p was not judged.",
  not_applicable: (c, w) => w(nested(c, 'reason')),
  no_method_ran: () => 'No check ran on this result.',

  /* ---- which number ------------------------------------------------------ */
  statistic: () => 'the test statistic',
  df: (c) => dfName(c),
  p: () => 'the p-value',
  n: (c, w) => `the sample size n (${w(nested(c, 'source'))})`,
  stated: () => 'as printed',
  implied_by_df: () => 'implied by the degrees of freedom',

  /* ---- a number's status, and its notes ---------------------------------- */
  matched: (c, w) => {
    const note = nested(c, 'note');
    // A truncation or double-rounding note is its own sentence: "Matches if
    // the paper ...". A p's note comes nested: "Matches if the paper rounded".
    const own = ['truncated', 'truncated_in_magnitude', 'double_rounded', 'double_rounded_in_magnitude'];
    if (note && (own.includes(note.code) || (note.code === 'p_note' && nested(note, 'note')?.code === 'double_rounded'))) return w(note);
    return note ? `Matches. ${w(note)}` : 'Matches.';
  },
  // A miss with a reason (part C follow-up) says that reason instead.
  not_matched: (c, w) => {
    const why = nested(c, 'why');
    return why ? w(why) : 'Differs from the value computed from the data under this binding.';
  },
  p_ruled_out_by_printed_statistic: () =>
    "The printed p cannot follow from the printed statistic and its degrees of freedom, whatever the data's rounding.",
  // One term per idea (part C, M-S5): a number left undecided is "not
  // decided", in its own sentence, never "not compared: The ...".
  not_checked: (c, w) => {
    const number = nested(c, 'number');
    const why = nested(c, 'why');
    const name = number ? lead(w(number)) : 'This number';
    if (why?.code === 'data_precision') {
      return `${name} differs from the value computed from the data by no more than the file's ${roundingOf(why)} could explain, so it was not decided.`;
    }
    return `${name} was not decided. ${w(why)}`;
  },
  in_magnitude: () => 'Matched in magnitude; the sign was not compared.',
  truncated: () => 'Matches if the paper cut this number off instead of rounding it.',
  truncated_in_magnitude: () =>
    'Matches in magnitude if the paper cut this number off instead of rounding it; the sign was not compared.',
  double_rounded_in_magnitude: () =>
    'Matches in magnitude if the paper rounded a number the software had already rounded; the sign was not compared.',
  df_truncated: (c) => `${lead(dfName(c))} match once the computed value is truncated.`,
  df_rounded: (c) => `${lead(dfName(c))} match once the computed value is rounded.`,
  p_note: (c, w) => w(nested(c, 'note')),
  p_concern_at_level_0: (c, w) => `Level 0 already raised a concern about the p: ${tail(w(nested(c, 'reason')))}.`,
  p_could_not_check_at_level_0: (c, w) => `Level 0 could not check the p: ${tail(w(nested(c, 'reason')))}.`,
  one_tailed_unstated_on_exact_p: () =>
    'The p agrees only as a one-tailed test, and the paper does not say the test was one-tailed.',
  p_concern: (c, w) => `The p raises a concern: ${tail(w(nested(c, 'reason')))}.`,
  p_could_not_check: (c, w) => `Could not check the p: ${tail(w(nested(c, 'reason')))}.`,
  spearman_p_may_be_exact: () =>
    "The printed p does not match the t approximation, but it lies where Spearman's exact p could be, and Gaply does not compute the exact p yet.",
  spearman_p_with_ties: () =>
    "The printed p does not match the t approximation, and with tied values some software gives an exact p that Gaply cannot bound.",
  data_precision: (c) =>
    `The printed statistic differs from the value computed from the data by no more than the file's ${roundingOf(c)} could explain, so it was not decided.`,

  /* ---- what was computed (names) ----------------------------------------- */
  student_pooled: () => "Student's t-test (pooled variance)",
  welch: () => "Welch's t-test (unequal variances)",
  paired: () => 'paired t-test',
  one_way: () => 'one-way ANOVA',
  pearson: () => 'Pearson correlation',
  spearman_t_approximation: () => 'Spearman correlation, p from the t approximation',
  chi_square_uncorrected: () => 'chi-square test without continuity correction',
  chi_square_yates: () => "chi-square test with Yates's continuity correction",
  ols_overall_f: () => 'linear regression, overall F test',
  ols_coefficient: (c, w) => `linear regression, the coefficient of ${w(nested(c, 'term'))}`,
  intercept: () => 'the intercept',
  predictor: (c, _w, ctx) => ctx.column(num(c, 'column') ?? 0),

  /* ---- conclusions ------------------------------------------------------- */
  candidates_disagree: () => 'The variants Gaply tried disagree about the conclusion, so no change in conclusion is reported.',
  several_alphas: () => 'The paper states more than one significance threshold, so a change in conclusion was not assessed.',
  p_not_checked: () => 'The p was not decided, so a change in conclusion was not assessed.',
  unplaced: () => 'The result could not be placed against a significance threshold, so a change in conclusion was not assessed.',
  p_judged_on_printed_numbers: () =>
    "The p was judged against the printed statistic, because the data's rounding left the data's own p undecided, so a change in conclusion was not assessed.",

  /* ---- Level 1's notes --------------------------------------------------- */
  variant_matched: (c, w) => `Matched as ${w(nested(c, 'variant'))}.`,
  sign_compared_in_magnitude: () => 'The statistic was compared in magnitude, because its sign depends on the order of the groups.',
  printed_truncated: (c, w) => `${lead(w(nested(c, 'number')))} matches only if the paper cut it off instead of rounding it.`,
  printed_double_rounded: (c, w) => `${lead(w(nested(c, 'number')))} matches if the paper rounded a number the software had already rounded.`,
  spearman_p_t_approximation: () => "The p matches Spearman's p by the t approximation, as R (exact = FALSE) and SPSS compute it.",
  continuity_does_not_apply: () => 'A continuity correction applies only to a 2 by 2 table, so it was not used.',
  conclusion_not_assessed: (c, w) => w(nested(c, 'gap')),

  /* ---- rows left out (names, shown as "N rows: ...") --------------------- */
  filtered_out: () => 'outside the filter you chose',
  missing_in_bound_column: () => 'a missing value in a chosen column',
  level_not_selected: () => 'in a level you did not select',

  /* ---- a confirmation that no longer holds ------------------------------- */
  claim_revised: () => 'The claim was edited after its binding was confirmed.',
  dataset_replaced: () => 'The data file was replaced after the binding was confirmed.',
  withdrawn: () => 'The confirmation was withdrawn.',
  fingerprint: () => 'the data file',
  claim_revision: () => 'the claim',
  method_version: () => "the method's version",
  engine_version: () => "Gaply's engine version",
  binding: () => 'the binding',
  unknown_token: () => 'The confirmation was not recognised.',
  revoked: (c, w) => w(nested(c, 'because')),
  claim_mismatch: () => 'The confirmation belongs to another claim.',
  stale: (c, w) => `${lead(w(nested(c, 'field')))} changed after the binding was confirmed, so the confirmation no longer holds.`,

  /* ---- why the engine refused to compute --------------------------------- */
  input: () => 'an input value',
  result: () => 'the computed value',
  gate: (c, w) => w(nested(c, 'rejection')),
  fingerprint_changed: () => 'The data file changed after you opened it, so nothing was computed from it.',
  weighted_file: () => 'The file declares a weight variable, and Level 1 does not compute weighted statistics.',
  method_not_available: () => 'This method is not available in Level 1.',
  too_few_complete_cases: (c) =>
    `Only ${plural(num(c, 'n'), 'complete row remains', 'complete rows remain')}, fewer than the ${num(c, 'minimum') ?? '?'} this method needs.`,
  zero_variance: () => 'A chosen column does not vary, so the statistic is undefined.',
  single_group: (c) => `Only ${plural(num(c, 'groups'), 'group remains', 'groups remain')}, so there is nothing to compare.`,
  empty_row_or_column: (c) =>
    `The table has ${plural(num(c, 'empty_rows'), 'empty row', 'empty rows')} and ${plural(num(c, 'empty_columns'), 'empty column', 'empty columns')}, so the test is undefined.`,
  singular_design: () => 'The predictors are collinear or do not vary, so the regression is undefined.',
  not_numeric: (c, _w, ctx) => `Column ${ctx.column(num(c, 'column') ?? 0)} holds values that are not numbers.`,
  non_finite: (c, w) => `The computation gave an infinite or undefined value at ${w(nested(c, 'at'))}, so no verdict was given.`,
  underflow: () => 'The computed p is too small to represent, so no verdict was given rather than a p of zero.',

  /* ---- reading the data file --------------------------------------------- */
  too_wide: (c) => `The file has ${num(c, 'columns') ?? '?'} columns, more than the ${num(c, 'limit') ?? '?'} Level 1 reads.`,
  no_columns: () => 'The file has no columns.',
  not_readable: () => 'The data file could not be read.',
  too_large: (c) => {
    const [size, limit] = [megabytesOne(num(c, 'bytes')), megabytesOne(num(c, 'limit'))];
    return size === limit ? `The data file is just over the ${limit} limit.` : `The data file is ${size}, over the ${limit} limit.`;
  },
  unsupported_format: () => 'This data file format is not supported.',
  malformed: () => 'The data file is malformed.',
  no_table: () => 'The data file holds no table.',
  file_too_large: (c) =>
    `The file has more than ${num(c, 'limit_rows') ?? '?'} rows, and Level 1 never computes on part of a file.`,
  file_changed_while_reading: () => 'The data file changed while it was being read.',
  xls_not_readable: () =>
    'This .xls file is in a format Level 1 cannot read, such as an older Excel version. Save it as .xlsx and open that.',
  dataset: (c, w) => w(nested(c, 'refusal')),
  not_a_regular_file: () => 'The chosen data file is not a regular file (it may be a pipe, a device or a socket), so it was not read.',
  // `declared` at or below the limit: a part unpacked to more than the
  // archive said it holds (part C follow-up, item 2). Also a .mat file whose
  // compressed elements unpack past the limit (follow-up 2), so "file".
  archive_too_large: (c) => {
    const declared = num(c, 'declared');
    const limit = num(c, 'limit');
    if (declared !== undefined && limit !== undefined && declared <= limit) {
      return 'This file unpacks to more than its own contents list says, so it was not opened.';
    }
    return `This file would take at least ${megabytes(declared)} once unpacked, more than the ${megabytes(limit)} Level 1 opens, so it was not opened.`;
  },
  // The ingest's cap on the text of all cells together (Research OS
  // evaluation, F4). Shown by the descriptive summary too, so it does not
  // name Level 1.
  content_too_large: (c) =>
    `This file holds more than ${megabytes(num(c, 'limit_bytes'))} of text across its cells, more than Gaply reads from one file, so it was not opened. A copy with only the columns you need may fit.`,
  // Text saved in another encoding, Latin-1 or UTF-16 (Research OS
  // evaluation, F10). Shown by the descriptive summary too.
  encoding_not_utf8: () =>
    'This file is text saved in an encoding other than UTF-8, such as Latin-1 or UTF-16, so it was not opened. Save a copy in UTF-8 and open that copy instead.',

  /* ---- why a claim could not be checked ---------------------------------- */
  spearman_exact_p: () => "Gaply does not compute Spearman's exact p.",
  fractional_df: (c) => `${lead(dfName(c))} is printed as a fraction, which this method cannot produce.`,
  df_contradicts_stated_variant: (c) =>
    `${lead(dfName(c))} is printed as a fraction, which contradicts the stated Student variant: Student's t has whole-number degrees of freedom.`,
  family_does_not_fit_method: () => 'The printed statistic is not the kind this method produces.',
  ols_target_does_not_fit_family: () => 'The regression target you chose does not produce the kind of statistic printed.',
  df_count_does_not_fit: (c) =>
    `The paper prints ${plural(num(c, 'printed'), 'degree of freedom', 'degrees of freedom')} and this method has ${num(c, 'expected') ?? '?'}.`,
  printed_out_of_range: () =>
    'The printed statistic is at or past the limit this statistic can take, so it was not checked. A correlation printed as ±1.00 can be a rounding of a value just inside ±1.',
  no_computed_result: () => 'No computed value exists to compare with the printed statistic.',
  token_stale: (c, w) => w(nested(c, 'rejection')),
  refused: (c, w) => w(nested(c, 'refusal')),
  data_unreadable: (c, w) => w(nested(c, 'refusal')),
  variant_not_available: (c, w) => w(nested(c, 'gap')),
  binding_invalid: (c, w) => w(nested(c, 'gap')),
  statistic_not_checkable: (c, w) => w(nested(c, 'gap')),

  /* ---- a binding's variant, and who set it (names) ----------------------- */
  none: () => 'no variant',
  student: () => "Student's t (equal variances)",
  not_stated: () => 'not stated',
  continuity_correction: () => 'with continuity correction',
  no_continuity_correction: () => 'without continuity correction',
  t_approximation: () => 'p from the t approximation',
  exact: () => 'exact p',
  claim_sentence: () => "the result's sentence",
  methods_passage: () => 'the Methods passage you selected',
  stated_in_paper: (c, w) => `stated in the paper (${w(nested(c, 'source'))})`,
  user_ticked: () => 'you ticked that the paper states this',

  /* ---- where a binding came from (names) --------------------------------- */
  header_matches_sentence: () => 'a column header matches words in the sentence',
  first_eligible_columns: () => 'the first columns of the right type',
  local: (c, w) => `suggested on this device (${w(nested(c, 'reason'))})`,
  ai_suggestion: () => 'suggested by AI',
  manual: () => 'set by you',
  manual_edited_from_ai: () => 'set by you, edited from an AI suggestion',
};

/** The words for a code. A code with no entry is shown as itself, never
 *  hidden: `l1Wording.vitest.ts` keeps that from happening for the shared set. */
export function sentenceFor(c: WireCode | undefined, ctx: WordContext = NO_COLUMNS): string {
  if (!c) return 'No reason was recorded.';
  const f = WORDS[c.code];
  if (!f) return `Gaply has no wording for the code ${c.code}.`;
  return f(c, (inner) => sentenceFor(inner, ctx), ctx);
}

/* ------------------------------------------------------------------------ *
 *  Names the screen uses that are not codes
 * ------------------------------------------------------------------------ */

export const METHOD_NAMES: Record<string, string> = {
  independent_t: 'Independent-samples t-test',
  paired_t: 'Paired t-test',
  one_way_anova: 'One-way ANOVA',
  pearson: 'Pearson correlation',
  spearman: 'Spearman correlation',
  chi_square_independence: 'Chi-square test of independence',
  ols: 'Linear regression (OLS)',
};

/** The method as a confirm summary names it, with its variant when stated. */
export function methodPhrase(method: string, variant: string | null): string {
  if (method === 'independent_t' && variant === 'welch') return "Welch's t-test";
  if (method === 'independent_t' && variant === 'student') return "Student's t-test";
  const base = METHOD_NAMES[method] ?? method;
  if (!variant || variant === 'none' || variant === 'not_stated') return base;
  return `${base}, ${sentenceFor({ code: variant })}`;
}

export const ROLE_NAMES: Record<string, string> = {
  outcome: 'Outcome',
  group: 'Groups',
  first: 'First measurement',
  second: 'Second measurement',
  x: 'X',
  y: 'Y',
  row_variable: 'Row variable',
  column_variable: 'Column variable',
  predictors: 'Predictors',
};

export const TASK_NAMES: Record<string, string> = {
  claim_method: 'Suggest method with AI',
  binding_suggestion: 'Suggest columns with AI',
  result_explanation: 'Explain this result',
  web_context: 'Method context (web)',
};

export const FAMILY_SYMBOL: Record<string, string> = { t: 't', F: 'F', chi_square: 'χ²', r: 'r', z: 'z' };

export const COLUMN_TYPE_NAMES: Record<string, string> = {
  numeric: 'numeric',
  integer: 'whole numbers',
  categorical: 'categories',
  text: 'text',
  date: 'dates',
  empty: 'empty',
};

/** Level 0's verdict on the printed numbers, as the claims screen shows it. */
export function level0Phrase(v: WireCode | null): string {
  if (!v) return 'Level 0: not run on an edited or added claim';
  switch (v.code) {
    case 'consistent': {
      const note = nested(v, 'note');
      return note ? `Level 0: consistent. ${sentenceFor(note)}` : 'Level 0: consistent';
    }
    case 'inconsistent': {
      const cc = nested(v, 'conclusion_changes')?.code;
      return cc === 'yes'
        ? 'Level 0: inconsistent (these numbers cannot all be correct as printed), and the conclusion would change'
        : 'Level 0: inconsistent (these numbers cannot all be correct as printed)';
    }
    case 'concern':
      return `Level 0: concern. ${sentenceFor(nested(v, 'reason'))}`;
    case 'could_not_check':
      return `Level 0: could not check. ${sentenceFor(nested(v, 'reason'))}`;
    default:
      return `Level 0: ${v.code}`;
  }
}

/** The numbers a Reproduced claim did not compare, from its verdict's
 *  `not_checked` notes, in order. Empty when every printed number matched. */
export function notCompared(notes: WireCode[]): WireCode[] {
  return notes.filter((n) => n.code === 'not_checked');
}

function joinNames(names: string[]): string {
  return names.length <= 1 ? names.join('') : `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

/** The label for a Reproduced claim: plain only when every printed number
 *  matched; otherwise it names what was not compared. */
export function reproducedLabel(notes: WireCode[]): string {
  const missing = notCompared(notes);
  if (missing.length === 0) return 'Reproduced';
  const names = missing.map((n) => sentenceFor(nested(n, 'number')));
  return `Reproduced, except ${joinNames(names)}, which ${names.length === 1 ? 'was' : 'were'} not decided`;
}

/** The threshold a conclusion is judged at, with where it came from. */
export function thresholdPhrase(alpha: { printed: string[]; stated: boolean }): string {
  if (!alpha.stated) return 'the usual threshold of .05, because the paper states none';
  return alpha.printed.length === 1
    ? `the paper's stated threshold (\u03b1 = ${alpha.printed[0]})`
    : `the paper's stated thresholds (\u03b1 = ${alpha.printed.join(', ')})`;
}

/** Why Level 0 and the check against the data differ on a claim the data
 *  matches only as a truncated print (Step 8b, rule 2, accepted in D270). */
export const LEVELS_DIFFER_ON_TRUNCATION =
  'Level 0 found these printed numbers inconsistent because it assumes every printed number was rounded; checked against the data, they match if the paper cut a number off instead.';

/** A NotReproduced result, worded the one way it may be. Never "wrong". */
export const NOT_REPRODUCED_TEXT =
  'The data under this binding gives a different value from the one printed. The usual causes are a binding that differs from the paper\'s analysis, a different subset of rows, a different version of the data, a different method, or a typo.';

/* ------------------------------------------------------------------------ *
 *  Command refusals
 * ------------------------------------------------------------------------ */

const REPLY_REASONS: Record<string, string> = {
  malformed_json: 'it was not valid JSON',
  incomplete: 'it was incomplete',
  refused: 'the model declined to answer',
  empty: 'it was empty',
  several_text_parts: 'it had more than one text part',
  wrong_shape: 'it did not have the requested shape',
  unknown_method: 'it named a method Gaply does not have',
  variant_not_in_method: 'it named a variant the method does not have',
  unknown_role: 'it named a role the method does not have',
  missing_role: 'it left a role empty',
  role_repeated: 'it named a role twice',
  unknown_alias: 'it named a column that was not sent',
  alias_in_two_roles: 'it put one column in two roles',
  level_count_invalid: 'it gave a level count the method does not allow',
  relied_on_unsent: 'it relied on something that was not sent',
  text_too_long: 'it was longer than allowed',
  number_not_in_input: 'it stated a number that was not in the request',
  wrong_verdict: "it described a verdict other than the engine's",
  non_ascii_digit: 'it wrote a number in digits other than 0 to 9',
  number_word: 'it wrote a number in words',
  sign_disagrees: "it gave a sign that disagrees with the engine's result",
  misconduct_word: 'it used a word that suggests misconduct',
};

const BINDING_REASONS: Record<string, (c: WireCode, ctx: WordContext) => string> = {
  stale_fingerprint: () => 'the data file changed since it was opened',
  claim_revision_changed: () => 'the claim changed since this binding was drafted',
  method_not_in_catalogue: () => 'the method is not in Level 1',
  transformation_requested: () => 'a transformation was requested, and Level 1 does not transform data',
  paired_method_bound_to_outcome_and_group: () => 'a paired test needs two measurement columns, not an outcome and a group',
  independent_method_bound_to_two_outcomes: () => 'an independent test needs an outcome and a group, not two measurements',
  role_not_in_method: (c) => `the method has no ${ROLE_NAMES[nested(c, 'role')?.code ?? ''] ?? 'such'} role`,
  missing_role: (c) => `${ROLE_NAMES[nested(c, 'role')?.code ?? ''] ?? 'a role'} has no column`,
  role_has_too_many_columns: (c) => `${ROLE_NAMES[nested(c, 'role')?.code ?? ''] ?? 'a role'} has too many columns`,
  unknown_column: (c, ctx) => `column ${ctx.column(num(c, 'column') ?? 0)} is not in the file`,
  column_in_two_roles: (c, ctx) => `column ${ctx.column(num(c, 'column') ?? 0)} is in two roles`,
  wrong_type: (c, ctx) =>
    `column ${ctx.column(num(c, 'column') ?? 0)} holds ${COLUMN_TYPE_NAMES[nested(c, 'found')?.code ?? ''] ?? 'the wrong type'}, which ${ROLE_NAMES[nested(c, 'role')?.code ?? ''] ?? 'this role'} cannot take`,
  filter_on_non_group_column: (c, ctx) => `column ${ctx.column(num(c, 'column') ?? 0)} has no levels to filter on`,
  filter_level_not_in_column: (c, ctx) => `a filter level is not in column ${ctx.column(num(c, 'column') ?? 0)}`,
  level_not_in_column: (c) => `a selected level is not in the ${ROLE_NAMES[nested(c, 'role')?.code ?? '']?.toLowerCase() ?? ''} column`,
  unselected_level: (c) => `${plural(num(c, 'unselected'), 'level is', 'levels are')} neither selected nor filtered out`,
  wrong_group_count: (c) => {
    const e = nested(c, 'expected');
    const k = e ? num(e, 'levels') : undefined;
    const rule = e?.code === 'exactly' ? `exactly ${k}` : `at least ${k}`;
    return `${plural(num(c, 'selected'), 'level is', 'levels are')} selected, and this method needs ${rule}`;
  },
  target_missing: () => 'a regression needs the overall F or one coefficient chosen',
  target_not_a_predictor: () => 'the chosen coefficient is not one of the predictors',
  target_not_applicable: () => 'this method has no regression target',
  whole_number_answer_not_applicable: (c, ctx) =>
    `column ${ctx.column(num(c, 'column') ?? 0)} is not a bound column of whole numbers, so "Exact whole numbers" does not apply to it`,
};

const SIMPLE_REFUSALS: Record<string, string> = {
  ai_help_off: 'AI help is off in Settings, so nothing was sent.',
  web_context_off: 'Web context is off in Settings, so nothing was sent.',
  no_key: 'No OpenAI key is saved, so nothing was sent.',
  unknown_preview: 'That preview is no longer held, so nothing was sent.',
  no_grant: 'No consent is held for this request, so nothing was sent.',
  grant_does_not_cover: 'The consent does not cover everything in this request, so nothing was sent.',
  body_changed: 'The request changed after its preview, so nothing was sent.',
  offline: 'The request could not reach OpenAI; this device appears to be offline.',
  timeout: 'The request timed out.',
  cancelled: 'The request was cancelled, and any reply is discarded.',
  redirect_refused: 'OpenAI answered with a redirect, and Gaply does not follow redirects, so no reply was used and nothing went to the other address.',
  transport_failed: 'The request failed.',
  manuscript_unreadable: 'The manuscript could not be read.',
  manuscript_refused: 'The manuscript was refused by the import check.',
  manuscript_not_extracted: 'The manuscript could not be read into sections.',
  unknown_run: 'This check is no longer open. Start again from the files.',
  unknown_claim: 'That claim is not in this check.',
  unknown_family: 'That kind of statistic is not one Level 1 reads.',
  unknown_relation: 'That p relation is not one Level 1 reads.',
  unknown_method: 'That method is not in Level 1.',
  unknown_variant: 'That variant is not one this method has.',
  unknown_role: 'That role is not one this method has.',
  unknown_topic: 'That topic is not one Gaply offers.',
  unknown_category: 'That data category is not one Gaply offers.',
  unknown_task: 'That task is not one Gaply offers.',
  unknown_proposal: 'That binding is no longer held.',
  unknown_token_handle: 'That confirmation is not held in this check.',
  no_dataset: 'No data file is open in this check.',
  no_finished_result: 'This result has not been computed yet.',
  proposal_from_another_run: 'That binding belongs to another check.',
  proposal_for_another_claim: 'That binding belongs to another claim.',
  preview_from_another_run: 'That preview belongs to another check.',
  not_an_opt_in_for_this_task: 'That item cannot be added to this request.',
  opt_in_for_another_task: 'That item was ticked for another request.',
  passage_not_in_methods: 'The passage is not text from the Methods section of this manuscript.',
  opt_in_empty: 'A ticked item is empty.',
  doi_malformed: 'That is not a DOI. Type a DOI only, such as 10.1234/abcd, with nothing else.',
  doi_contains_data: 'That DOI contains a column name, a level name, a cell or the file name from your data, so nothing was sent.',
  sentence_not_in_manuscript: "The result's sentence is not text from this manuscript, so it was not sent.",
  no_sentence: 'This claim has no sentence to send.',
  link_malformed: 'That link is malformed, so it was not opened.',
  link_not_opened: 'The link could not be opened.',
};

/** A command's refusal, read from its `{ code, message }` rejection, whose
 *  message is a code in JSON. `null` when the rejection is not one. */
export function refusalOf(e: unknown): WireCode | null {
  const message = e instanceof Error ? e.message : typeof e === 'object' && e && 'message' in e ? String((e as { message: unknown }).message) : typeof e === 'string' ? e : '';
  try {
    const v = JSON.parse(message);
    return v && typeof v === 'object' && typeof v.code === 'string' ? (v as WireCode) : null;
  } catch {
    return null;
  }
}

/** One plain sentence for a command refusal. */
export function refusalSentence(e: unknown, ctx: WordContext = NO_COLUMNS): string {
  const c = refusalOf(e);
  if (!c) {
    const message = e instanceof Error ? e.message : typeof e === 'string' ? e : '';
    return message.trim() ? `This could not be done: ${tail(message.trim())}.` : 'This could not be done.';
  }
  if (c.code in SIMPLE_REFUSALS) return SIMPLE_REFUSALS[c.code];
  const inner = (k: string) => nested(c, k);
  switch (c.code) {
    case 'budget_spent':
      return `This check has used its ${num(c, 'max') ?? '?'} requests for this task.`;
    case 'transport': {
      const t = inner('error');
      if (t?.code === 'http') return `OpenAI answered with status ${num(t, 'status') ?? '?'}, so no reply was used.`;
      return SIMPLE_REFUSALS[t?.code ?? ''] ?? 'The request failed.';
    }
    case 'reply_rejected':
      return `The reply was not used: ${REPLY_REASONS[inner('reason')?.code ?? ''] ?? 'it did not pass the check'}.`;
    case 'binding_rejected': {
      const r = inner('reason');
      const f = r && BINDING_REASONS[r.code];
      return `This binding cannot be confirmed: ${f ? f(r, ctx) : 'it did not pass the check'}.`;
    }
    case 'data_refused':
      return sentenceFor(inner('reason'), ctx);
    case 'number_not_read':
      return 'A number in the claim could not be read; type it as it is printed, with a dot for decimals.';
    case 'passage_too_long':
    case 'sentence_too_long':
      return `That text is ${num(c, 'chars') ?? '?'} characters, over the ${num(c, 'max') ?? '?'} allowed.`;
    case 'link_refused':
      return 'That link is not on the list of sources Gaply opens, so it was not opened.';
    default:
      return `This could not be done (${c.code.replace(/_/g, ' ')}).`;
  }
}

/** One sentence for a dataset-level issue the open view carries
 *  (`stats_l1_commands.rs`'s `dataset_issue_code`). Command codes, kept apart
 *  from the shared result code set like `REFUSALS`. */
export function datasetIssueSentence(c: WireCode): string {
  switch (c.code) {
    case 'looks_separated_by': {
      const sep = nested(c, 'separator')?.code === 'tab' ? 'tab' : 'semicolon';
      return `The data file looks ${sep}-separated, so it was read as one column. Save it as a comma-separated file or as .xlsx and open that.`;
    }
    case 'weight_variable_declared':
      return 'The file declares a weight variable; Level 1 does not compute weighted results, so claims on it cannot be checked.';
    case 'weight_variable_unknown':
      return 'The file declares a weight variable that is not among its columns; Level 1 does not compute weighted results.';
    case 'source_metadata_not_known':
      return "The file's variable information could not be read.";
    case 'overlong_rows':
      return `${plural(num(c, 'rows'), 'row is', 'rows are')} longer than the header; the extra cells were left out.`;
    default:
      return `Data file note: ${c.code}.`;
  }
}

/** The data-file line of a finished run. */
export function dataReadPhrase(c: WireCode): string {
  if (c.code === 'read') return 'The data file was read again for this run.';
  if (c.code === 'not_needed') return 'No claim was confirmed, so the data file was not read.';
  return sentenceFor(nested(c, 'refusal'));
}

/* ------------------------------------------------------------------------ *
 *  Evaluation fixes, part C
 * ------------------------------------------------------------------------ */

/** The entry card's one line on what Gaply reads (M-S3). */
export const ENTRY_TEXT =
  'Gaply reads the t, F, χ², r and z results it can find and recomputes those it has a method for, from your data, under a binding you confirm. Both files stay on this device.';

/** The Confirm step's question, asked once for each bound column whose
 *  cells are all whole numbers (B-1). The first answer is the default. */
export const WHOLE_NUMBER_QUESTION =
  'Does this column hold exact whole numbers, such as counts or points on a rating scale, or values rounded to whole numbers, such as times, percentages or ages?';
export const WHOLE_NUMBER_ANSWERS: Array<{ answer: 'rounded' | 'exact'; label: string }> = [
  { answer: 'rounded', label: 'Rounded, or not sure' },
  { answer: 'exact', label: 'Exact whole numbers' },
];

/** What the result card says about one bound whole-number column. */
export function wholeNumberLine(w: { alias: string; answer: 'rounded' | 'exact' }, ctx: WordContext): string {
  const column = ctx.column(Number(w.alias.replace(/^C/, '')));
  return w.answer === 'exact'
    ? `Column ${column} was treated as exact whole numbers, as you answered, so its values were taken as they are.`
    : `Column ${column} holds whole numbers and was treated as rounded to whole numbers (your answer: rounded, or not sure), so each value may be off by up to half a unit.`;
}

/** Skipped claims after a run (M-4): counted apart, never "could not be
 *  checked". */
export function skippedPhrase(n: number): string {
  if (n === 0) return '';
  return `${n} ${n === 1 ? 'claim was' : 'claims were'} skipped by you and not run.`;
}

/** One sentence for a per-column issue the open view carries
 *  (`stats_l1_commands.rs`'s `column_issue_code`; M-S6). */
export function columnIssueSentence(c: WireCode, column: string): string {
  switch (c.code) {
    case 'empty_header':
      return `Column ${column} has no header.`;
    case 'duplicate_header':
      return `Column ${column} has the same header as column C${num(c, 'first') ?? '?'}.`;
    case 'all_missing':
      return `Column ${column} has no values: every cell is missing.`;
    case 'mixed_types':
      return `Column ${column} holds ${plural(num(c, 'numbers'), 'number', 'numbers')} and ${plural(num(c, 'texts'), 'text value', 'text values')}, so it is read as text and cannot be bound as numbers.`;
    case 'non_finite':
      return `Column ${column} has ${plural(num(c, 'cells'), 'cell', 'cells')} reading NaN or infinity, so it cannot be bound as numbers.`;
    case 'category_explosion':
      return `Column ${column} has ${num(c, 'distinct') ?? '?'} different text values, too many to use as groups.`;
    case 'ragged_rows':
      return `Column ${column} is missing in ${plural(num(c, 'short_rows'), 'row', 'rows')} shorter than the header; those cells were read as missing.`;
    case 'user_missing_applied':
      return `Column ${column}: the file marks ${plural(num(c, 'cells'), 'cell', 'cells')} as missing with its own missing-value codes, and Gaply treats them as missing.`;
    default:
      return `Column ${column}: ${c.code}.`;
  }
}
