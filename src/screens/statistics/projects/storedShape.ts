// Gaply — the shape check a saved result passes before anything renders it
// (Research OS, Phase 5B-3b; decision 150). project_run_result returns the
// exact JSON text Rust stored (decision 129); this reads it, checks every
// field the live result components read, and only then hands them the same
// props the live screens give them.
//
// * The capability must be one this app knows (the summary, the t-test, or
//   since Phase 8D-2b the one-sample t-test, since 8D-2c the paired, since
//   8G-2 the one-way ANOVA, since 8H-2 the correlations, since 8I-2
//   Cronbach's alpha, and since 8X-2 crosstabs),
//   and every schema version the one this app reads: 1, the literal type of
//   `schemaVersion` in describeTypes.ts and ttestTypes.ts and Rust's
//   `SchemaVersion::NUMBER` (gaply-core/src/contracts/mod.rs).
// * Every number a component prints must be a finite number (a null where a
//   number belongs fails), every closed word one of its set, and every code
//   one of its kind's list (describeCodes.json, ttestCodes.json), so no
//   sentence ever repeats a stored string as if it were a code.
// * Parsing runs in a try/catch that drops the error: a JSON parse error's
//   message can quote its input. Nothing here throws, and nothing here keeps
//   or returns any part of what failed. The outcome is a closed case.
import type { ColumnResult, Diagnostic, ResultBundleView, RunManifest } from '../describeTypes';
import type { TTestDiagnostic, TTestResultView } from '../ttestTypes';
import type { OneSampleDiagnostic, OneSampleResultView, PairedResultView } from '../onesampleTypes';
import type { AnovaDiagnostic, AnovaResultView } from '../anovaTypes';
import type { CorrDiagnostic, CorrResultView } from '../corrTypes';
import type { AlphaDiagnostic, AlphaResultView } from '../alphaTypes';
import type { XtabDiagnostic, XtabResultView } from '../xtabTypes';
import type { RanksDiagnostic, RanksResultView } from '../ranksTypes';
import type { RelatedRanksDiagnostic, RelatedRanksResultView } from '../relatedRanksTypes';
import DESCRIBE_CODES from '../describeCodes.json';
import TTEST_CODES from '../ttestCodes.json';
import ONESAMPLE_CODES from '../onesampleCodes.json';
import ANOVA_CODES from '../anovaCodes.json';
import CORR_CODES from '../corrCodes.json';
import ALPHA_CODES from '../alphaCodes.json';
import XTAB_CODES from '../xtabCodes.json';
import RANKS_CODES from '../ranksCodes.json';
import RELATED_RANKS_CODES from '../relatedRanksCodes.json';
import RELATED_LIMITS from '../fixtures/relatedRanksLimits.json';
import { capabilityOfKind } from '../methods';
import TTEST_LIMITS from '../ttestLimits.json';

/** The schema version this app reads, for every stored part. */
export const KNOWN_SCHEMA_VERSION = 1;

/** The two methods' capability ids, from the methods table (Phase 8C). */
export const SUMMARY_CAPABILITY = capabilityOfKind('describe');
export const TTEST_CAPABILITY = capabilityOfKind('ttest');
export const ONE_SAMPLE_CAPABILITY = capabilityOfKind('one_sample');
export const PAIRED_CAPABILITY = capabilityOfKind('paired');
export const ANOVA_CAPABILITY = capabilityOfKind('anova');
export const CORR_CAPABILITY = capabilityOfKind('correlations');
export const ALPHA_CAPABILITY = capabilityOfKind('cronbach_alpha');
export const XTAB_CAPABILITY = capabilityOfKind('crosstabs');
export const RANKS_CAPABILITY = capabilityOfKind('independent_ranks');
export const RELATED_RANKS_CAPABILITY = capabilityOfKind('related_ranks');

/** A data file's formats: the ingest's (DataFormat in Rust, without json). */
const DATA_FORMATS = ['csv', 'tsv', 'xlsx', 'xlsm', 'xlsb', 'xls', 'ods', 'sav', 'mat'];

/** The case a method's checked results are shown as. */
export type CaseKind = 'summary' | 'ttest' | 'one_sample' | 'paired' | 'anova' | 'correlations' | 'cronbach_alpha' | 'crosstabs' | 'independent_ranks' | 'related_ranks';

/** A failed run's stored diagnostic, in its method's codes. */
export type StoredFailure = Diagnostic | TTestDiagnostic | OneSampleDiagnostic | AnovaDiagnostic | CorrDiagnostic | AlphaDiagnostic | XtabDiagnostic | RanksDiagnostic | RelatedRanksDiagnostic;

/** What a saved result is, once checked: what to show, or why not. */
export type Checked =
  | { kind: 'summary'; result: ResultBundleView; dataFile: DataFileFacts }
  | { kind: 'ttest'; result: TTestResultView; dataFile: DataFileFacts }
  | { kind: 'one_sample'; result: OneSampleResultView; dataFile: DataFileFacts }
  | { kind: 'paired'; result: PairedResultView; dataFile: DataFileFacts }
  | { kind: 'anova'; result: AnovaResultView; dataFile: DataFileFacts }
  | { kind: 'correlations'; result: CorrResultView; dataFile: DataFileFacts }
  | { kind: 'cronbach_alpha'; result: AlphaResultView; dataFile: DataFileFacts }
  | { kind: 'crosstabs'; result: XtabResultView; dataFile: DataFileFacts }
  | { kind: 'independent_ranks'; result: RanksResultView; dataFile: DataFileFacts }
  | { kind: 'related_ranks'; result: RelatedRanksResultView; dataFile: DataFileFacts }
  | { kind: 'failed'; capability: CaseKind; failure: StoredFailure; dataFile: DataFileFacts }
  | { kind: 'unknownCapability' }
  | { kind: 'unknownSchema' }
  | { kind: 'badShape' };

/** The data file as the view may show it: its format and size, nothing else. */
export interface DataFileFacts {
  format: string;
  bytes: number;
}

/* ---- the small checks ------------------------------------------------------ */

type O = Record<string, unknown>;

const isObject = (v: unknown): v is O => typeof v === 'object' && v !== null && !Array.isArray(v);
const isNum = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
const isCount = (v: unknown): v is number => isNum(v) && Number.isInteger(v) && v >= 0;
const isStr = (v: unknown): v is string => typeof v === 'string';
const isHex64 = (v: unknown): v is string => isStr(v) && /^[0-9a-f]{64}$/.test(v);
const oneOf = (v: unknown, set: readonly string[]): v is string => isStr(v) && set.indexOf(v) >= 0;
const arrayOf = (v: unknown, each: (x: unknown) => boolean): boolean => Array.isArray(v) && v.every(each);

const DESC_CODES = (DESCRIBE_CODES as string[]).filter((c) => c.indexOf('stats.desc.') === 0);
const T_CODES = (TTEST_CODES as string[]).filter((c) => c.indexOf('stats.ttest.') === 0);
const OS_CODES = (ONESAMPLE_CODES as string[]).filter((c) => c.indexOf('stats.onesample.') === 0);
const AN_CODES = (ANOVA_CODES as string[]).filter((c) => c.indexOf('stats.anova.') === 0);
const CR_CODES = (CORR_CODES as string[]).filter((c) => c.indexOf('stats.corr.') === 0);
const AL_CODES = (ALPHA_CODES as string[]).filter((c) => c.indexOf('stats.alpha.') === 0);
const XT_CODES = (XTAB_CODES as string[]).filter((c) => c.indexOf('stats.xtab.') === 0);
const RK_CODES = (RANKS_CODES as string[]).filter((c) => c.indexOf('stats.ranks.') === 0);
const RR_CODES = (RELATED_RANKS_CODES as string[]).filter((c) => c.indexOf('stats.related_ranks.') === 0);
const SEVERITIES = ['error', 'warning', 'note'];
const DESC_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found', 'from', 'event'];
const T_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const OS_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const AN_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const CR_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const AL_PARAM_KEYS = ['column', 'count', 'index', 'value', 'expected', 'found'];
const XT_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const RK_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const RR_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const RUN_STATES = [
  'draft',
  'validating',
  'needs_user_input',
  'awaiting_confirmation',
  'refused',
  'validated',
  'invalidated',
  'queued',
  'running',
  'succeeded',
  'failed',
  'cancelled',
  'timed_out',
  'resource_limit',
  'numerical_failure',
];

/** A parameter map: known keys only; numbers (and, for the summary, closed
 *  strings) as values. */
function params(v: unknown, keys: readonly string[], strings: boolean): boolean {
  if (!isObject(v)) return false;
  return Object.keys(v).every((k) => keys.indexOf(k) >= 0 && (isNum(v[k]) || (strings && isStr(v[k]))));
}

const diagnostic = (codes: readonly string[], keys: readonly string[], strings: boolean) => (v: unknown) =>
  isObject(v) && oneOf(v.code, codes) && params(v.params, keys, strings) && oneOf(v.severity, SEVERITIES);

const exclusion = (codes: readonly string[], keys: readonly string[], strings: boolean) => (v: unknown) =>
  isObject(v) && oneOf(v.code, codes) && isCount(v.count) && params(v.params, keys, strings);

/** An exclusion that, when its code is one of `named`, names its column: the
 *  codes on which the method's Rust sets a column every time, at every
 *  commit that wrote stored results of it, so a stored one without it is not
 *  one Rust wrote (decision 337b). A code not listed may name one or not. */
const exclusionNaming =
  (codes: readonly string[], keys: readonly string[], strings: boolean, named: readonly string[]) => (v: unknown) =>
    exclusion(codes, keys, strings)(v) &&
    isObject(v) &&
    (named.indexOf(v.code as string) < 0 || (isObject(v.params) && isCount(v.params.column)));

/* Each method's exclusions, and the codes of them that always name their
 * column (decision 337b; measured in each module's history, 1 Nov 2026):
 * stats_desc's manifest since 9c54030, stats_ttest's since ee244bc, and
 * stats_onesample's since 04d8705, where the paired's missing_code names no
 * column (a code in either column names neither). */
const DESC_EXCLUSION = exclusionNaming(DESC_CODES, DESC_PARAM_KEYS, true, ['stats.desc.missing_cell', 'stats.desc.missing_code']);
const T_EXCLUSION = exclusionNaming(T_CODES, T_PARAM_KEYS, false, [
  'stats.ttest.missing_group',
  'stats.ttest.other_level',
  'stats.ttest.missing_outcome',
  'stats.ttest.missing_code',
]);
const OS_EXCLUSION = exclusionNaming(OS_CODES, OS_PARAM_KEYS, false, ['stats.onesample.missing_value', 'stats.onesample.missing_code']);
const PAIRED_EXCLUSION = exclusionNaming(OS_CODES, OS_PARAM_KEYS, false, ['stats.onesample.missing_first', 'stats.onesample.missing_second']);

function reproducibility(v: unknown): boolean {
  if (v === 'bitwise' || v === 'not_reproducible') return true;
  return isObject(v) && isObject(v.within_tolerance) && isNum(v.within_tolerance.relative);
}

function manifest(v: unknown, capability: string, excluded: (e: unknown) => boolean): boolean {
  return (
    isObject(v) &&
    v.schemaVersion === KNOWN_SCHEMA_VERSION &&
    v.capabilityId === capability &&
    isCount(v.capabilityVersion) &&
    isCount(v.engineVersion) &&
    arrayOf(v.inputs, isHex64) &&
    isHex64(v.parametersHash) &&
    (v.seed === null || isNum(v.seed)) &&
    isCount(v.rowsRead) &&
    isCount(v.rowsUsed) &&
    arrayOf(v.exclusions, excluded) &&
    isObject(v.platform) &&
    oneOf(v.platform.os, ['macos', 'windows', 'linux', 'other']) &&
    oneOf(v.platform.arch, ['aarch64', 'x86_64', 'other']) &&
    isCount(v.elapsedMs) &&
    isHex64(v.outputHash) &&
    reproducibility(v.reproducibility)
  );
}

/* ---- a run on prepared data (Research OS, Phase 8M-4, decision 436) -------- */

/** A stored preparation, with exactly its five fields: hashes and numbers. */
function preparationShape(v: unknown): v is O {
  return (
    isObject(v) &&
    Object.keys(v).length === 5 &&
    isHex64(v.sourceHash) &&
    isHex64(v.recipeHash) &&
    isCount(v.sourceColumns) &&
    isCount(v.capabilityVersion) &&
    isCount(v.engineVersion)
  );
}

/** A manifest's preparation is present exactly when its spec's is, and equal to
 *  it: Rust copies the one into the other, and writes neither when absent. */
function preparationsAgree(spec: O, manifest: unknown): boolean {
  if (!isObject(manifest)) return false;
  if (!('preparation' in spec) && !('preparation' in manifest)) return true;
  const [a, b] = [spec.preparation, manifest.preparation];
  return (
    preparationShape(a) &&
    preparationShape(b) &&
    a.sourceHash === b.sourceHash &&
    a.recipeHash === b.recipeHash &&
    a.sourceColumns === b.sourceColumns &&
    a.capabilityVersion === b.capabilityVersion &&
    a.engineVersion === b.engineVersion
  );
}

/* ---- the summary ----------------------------------------------------------- */

const stat = (v: unknown) =>
  isObject(v) && ((Object.keys(v).length === 1 && isNum(v.value)) || (Object.keys(v).length === 1 && oneOf(v.not_computable, DESC_CODES)));

function column(v: unknown): boolean {
  if (!isObject(v) || Object.keys(v).length !== 1) return false;
  if (isObject(v.numeric)) {
    const c = v.numeric;
    return (
      isStr(c.alias) &&
      isStr(c.header) &&
      isCount(c.n) &&
      isCount(c.missing) &&
      [c.mean, c.sd, c.min, c.q1, c.median, c.q3, c.max].every(stat)
    );
  }
  if (isObject(v.categorical)) {
    const c = v.categorical;
    return (
      isStr(c.alias) &&
      isStr(c.header) &&
      isCount(c.n) &&
      isCount(c.missing) &&
      arrayOf(c.levels, (l) => isObject(l) && isStr(l.label) && isCount(l.count))
    );
  }
  return false;
}

function summaryBundle(v: O): ResultBundleView | null {
  const d = diagnostic(DESC_CODES, DESC_PARAM_KEYS, true);
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(v.primaryOutputs) &&
    arrayOf(v.primaryOutputs.columns, column) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, SUMMARY_CAPABILITY, DESC_EXCLUSION);
  if (!ok) return null;
  // The same props the live screen gets: Rust's stats_desc_commands::result
  // makes its view from the bundle in exactly this way.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    columns: (v.primaryOutputs as O).columns as ColumnResult[],
    diagnostics: v.diagnostics as Diagnostic[],
    manifest: v.manifest as RunManifest,
  };
}

/* ---- the t-test ------------------------------------------------------------- */

const tStat = (v: unknown) =>
  isObject(v) && Object.keys(v).length === 1 && (isNum(v.value) || oneOf(v.not_computable, T_CODES));

const group = (v: unknown) => isObject(v) && isCount(v.level) && isStr(v.label) && isCount(v.n) && isNum(v.mean) && isNum(v.sd);

function ttestBundle(v: O): TTestResultView | null {
  const d = diagnostic(T_CODES, T_PARAM_KEYS, false);
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    oneOf(r.variant, ['student', 'welch']) &&
    [r.t, r.df, r.p, r.meanDifference, r.standardError, r.confidenceLevel, r.ciLower, r.ciUpper, r.cohensD, r.dStandardizerValue].every(isNum) &&
    oneOf(r.dStandardizer, ['pooled_sd', 'root_mean_variance']) &&
    Array.isArray(r.groups) &&
    r.groups.length === 2 &&
    r.groups.every(group) &&
    tStat(r.sdRatio) &&
    arrayOf(r.exclusions, T_EXCLUSION) &&
    arrayOf(r.limitations, (c) => oneOf(c, T_CODES)) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, TTEST_CAPABILITY, T_EXCLUSION);
  if (!ok) return null;
  // As Rust's stats_ttest_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as TTestResultView['result'],
    diagnostics: v.diagnostics as TTestDiagnostic[],
    manifest: v.manifest as TTestResultView['manifest'],
  };
}

/* ---- the one-sample t-test (Phase 8D-2b, decision 269) ---------------------- */

function oneSampleBundle(v: O): OneSampleResultView | null {
  const d = diagnostic(OS_CODES, OS_PARAM_KEYS, false);
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    isCount(r.n) &&
    [r.mean, r.sd, r.testValue, r.t, r.df, r.p, r.meanDifference, r.standardError, r.confidenceLevel, r.ciLower, r.ciUpper, r.cohensD].every(isNum) &&
    arrayOf(r.exclusions, OS_EXCLUSION) &&
    arrayOf(r.limitations, (c) => oneOf(c, OS_CODES)) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, ONE_SAMPLE_CAPABILITY, OS_EXCLUSION);
  if (!ok) return null;
  // As Rust's stats_onesample_commands::result_one_sample makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as OneSampleResultView['result'],
    diagnostics: v.diagnostics as OneSampleDiagnostic[],
    manifest: v.manifest as OneSampleResultView['manifest'],
  };
}

/* ---- the paired t-test (Phase 8D-2c, decision 272) --------------------------- */

const columnStats = (v: unknown) => isObject(v) && isNum(v.mean) && isNum(v.sd);

function pairedBundle(v: O): PairedResultView | null {
  const d = diagnostic(OS_CODES, OS_PARAM_KEYS, false);
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    isCount(r.nPairs) &&
    columnStats(r.first) &&
    columnStats(r.second) &&
    [r.t, r.df, r.p, r.meanDifference, r.sdDifferences, r.standardError, r.confidenceLevel, r.ciLower, r.ciUpper, r.cohensDz].every(isNum) &&
    arrayOf(r.exclusions, PAIRED_EXCLUSION) &&
    arrayOf(r.limitations, (c) => oneOf(c, OS_CODES)) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, PAIRED_CAPABILITY, PAIRED_EXCLUSION);
  if (!ok) return null;
  // As Rust's stats_onesample_commands::result_paired makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as PairedResultView['result'],
    diagnostics: v.diagnostics as OneSampleDiagnostic[],
    manifest: v.manifest as PairedResultView['manifest'],
  };
}

/* ---- the one-way ANOVA (Phase 8G-2, decision 330) ----------------------------- */

/** One key of `keys` holding what `check` accepts, and nothing else. */
const oneKey = (v: unknown, cases: Record<string, (x: unknown) => boolean>) =>
  isObject(v) && Object.keys(v).length === 1 && Object.keys(cases).some((k) => k in v && cases[k](v[k]));

const anCode = (x: unknown) => oneOf(x, AN_CODES);
const aStat = (v: unknown) => oneKey(v, { value: isNum, not_computable: anCode });
const fTest = (v: unknown) => isObject(v) && [v.f, v.df1, v.df2, v.p].every(isNum);
const fOutcome = (v: unknown) => oneKey(v, { computed: fTest, not_computable: anCode });
const postHocP = (v: unknown) => oneKey(v, { value: isNum, below: isNum });
const pairStats = (v: unknown) =>
  isObject(v) && [v.difference, v.se, v.statistic, v.df, v.ciLower, v.ciUpper].every(isNum) && postHocP(v.p);
const pair = (v: unknown) =>
  isObject(v) && isCount(v.first) && isCount(v.second) && oneKey(v.outcome, { computed: pairStats, not_computable: anCode });
const postHocTable = (v: unknown) =>
  isObject(v) &&
  oneOf(v.test, ['tukey', 'games_howell', 'bonferroni']) &&
  oneKey(v.pairs, { computed: (x) => arrayOf(x, pair), not_computable: anCode });
const anovaTable = (v: unknown) =>
  isObject(v) &&
  [v.ssBetween, v.ssWithin, v.ssTotal, v.dfBetween, v.dfWithin, v.msBetween, v.msWithin].every(isNum) &&
  aStat(v.f) &&
  aStat(v.p);
const groupRow = (v: unknown) =>
  isObject(v) &&
  isCount(v.level) &&
  isStr(v.label) &&
  isCount(v.n) &&
  [v.mean, v.sd, v.se, v.ciLower, v.ciUpper, v.min, v.max].every(isNum);

/** An ANOVA exclusion: as every method's, and it names its column, which
 *  `stats_anova` always sets, so a stored one without it is not one Rust
 *  wrote. */
const anExclusion = (e: unknown) =>
  exclusion(AN_CODES, AN_PARAM_KEYS, false)(e) && isObject(e) && isObject(e.params) && isCount(e.params.column);

function anovaBundle(v: O): AnovaResultView | null {
  const d = diagnostic(AN_CODES, AN_PARAM_KEYS, false);
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    arrayOf(r.groups, groupRow) &&
    oneKey(r.table, { computed: anovaTable, not_computable: anCode }) &&
    fOutcome(r.welch) &&
    fOutcome(r.leveneMean) &&
    fOutcome(r.leveneMedian) &&
    aStat(r.etaSquared) &&
    aStat(r.omegaSquared) &&
    arrayOf(r.postHoc, postHocTable) &&
    arrayOf(r.exclusions, anExclusion) &&
    arrayOf(r.notes, anCode) &&
    arrayOf(r.limitations, anCode) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, ANOVA_CAPABILITY, anExclusion);
  if (!ok) return null;
  // As Rust's stats_anova_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as AnovaResultView['result'],
    diagnostics: v.diagnostics as AnovaDiagnostic[],
    manifest: v.manifest as AnovaResultView['manifest'],
  };
}

/* ---- the correlations (Phase 8H-2, decision 358) ------------------------------ */

const crCode = (x: unknown) => oneOf(x, CR_CODES);
const isAlias = (v: unknown) => isStr(v) && /^C[0-9]+$/.test(v);
const cStat = (v: unknown) => oneKey(v, { value: isNum, not_computable: crCode });
/** A t: a number, or the one closed word for an infinite t. */
const bounded = (v: unknown) => oneKey(v, { value: isNum, infinite: (x) => x === 'stats.corr.perfect_coefficient' });
const testStatistic = (v: unknown) =>
  oneKey(v, {
    t: (x) => isObject(x) && Object.keys(x).length === 2 && bounded(x.t) && isNum(x.df),
    z: isNum,
  });
const interval = (v: unknown) =>
  oneKey(v, { computed: (x) => isObject(x) && Object.keys(x).length === 2 && isNum(x.lower) && isNum(x.upper), not_computable: crCode });
const corrPairStats = (v: unknown) => isObject(v) && isNum(v.estimate) && testStatistic(v.test) && isNum(v.p) && interval(v.interval);
const corrPair = (v: unknown) =>
  isObject(v) && isAlias(v.first) && isAlias(v.second) && isCount(v.n) && oneKey(v.outcome, { computed: corrPairStats, not_computable: crCode });
const coefficientTable = (v: unknown) =>
  isObject(v) && oneOf(v.coefficient, ['pearson', 'spearman', 'kendall']) && arrayOf(v.pairs, corrPair);
const descriptive = (v: unknown) => isObject(v) && isAlias(v.variable) && isCount(v.n) && cStat(v.mean) && cStat(v.sd);

/** A correlations exclusion, exactly as stats_corr writes it (decision 358,
 *  337b's rule): missing_value and missing_code (listwise) always name their
 *  column; fewer_than_two_values (pairwise) never does. */
const CR_NAMED = ['stats.corr.missing_value', 'stats.corr.missing_code'];
const crExclusion = (e: unknown) =>
  exclusion(CR_CODES, CR_PARAM_KEYS, false)(e) &&
  isObject(e) &&
  isObject(e.params) &&
  (CR_NAMED.indexOf(e.code as string) >= 0
    ? isCount(e.params.column)
    : e.code !== 'stats.corr.fewer_than_two_values' || Object.keys(e.params).length === 0);

function corrBundle(v: O): CorrResultView | null {
  const d = diagnostic(CR_CODES, CR_PARAM_KEYS, false);
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    arrayOf(r.descriptives, descriptive) &&
    arrayOf(r.coefficients, coefficientTable) &&
    isCount(r.rowsUsed) &&
    arrayOf(r.exclusions, crExclusion) &&
    arrayOf(r.limitations, crCode) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, CORR_CAPABILITY, crExclusion);
  if (!ok) return null;
  // As Rust's stats_corr_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as CorrResultView['result'],
    diagnostics: v.diagnostics as CorrDiagnostic[],
    manifest: v.manifest as CorrResultView['manifest'],
  };
}

/* ---- Cronbach's alpha (Phase 8I-2, decision 383) ------------------------------ */

/** The codes stats_alpha writes where a number would be, field by field,
 *  read from its code (decision 337b's rule): a refused sum, square or root
 *  is overflow, underflow or arithmetic_refused; `finite` turns a value it
 *  cannot hold into overflow. */
const RANGE = ['overflow', 'underflow', 'arithmetic_refused'];
const al = (codes: string[]) => codes.map((c) => `stats.alpha.${c}`);
const AL_NC = {
  alpha: al(['total_constant', ...RANGE]),
  interval: al(['total_constant', 'distribution_out_of_range', ...RANGE]),
  standardized: al(['item_constant', 'standardized_not_defined', ...RANGE]),
  interItem: al(['item_constant', ...RANGE]),
  mean: al(['overflow']),
  sd: al(RANGE),
  scaleMeanIfDeleted: al(['overflow']),
  scaleVarianceIfDeleted: al(RANGE),
  correctedItemTotal: al(['item_constant', 'others_constant', ...RANGE]),
  alphaIfDeleted: al(['two_items', 'others_constant', ...RANGE]),
};
const alStat = (codes: readonly string[]) => (v: unknown) => oneKey(v, { value: isNum, not_computable: (x) => oneOf(x, codes) });
const alInterval = (v: unknown) =>
  oneKey(v, {
    computed: (x) => isObject(x) && Object.keys(x).length === 2 && isNum(x.lower) && isNum(x.upper),
    not_computable: (x) => oneOf(x, AL_NC.interval),
  });
const interItem = (v: unknown) =>
  isObject(v) && Object.keys(v).length === 3 && [v.mean, v.min, v.max].every(alStat(AL_NC.interItem));
const ITEM_STATS = ['mean', 'sd', 'scaleMeanIfDeleted', 'scaleVarianceIfDeleted', 'correctedItemTotal', 'alphaIfDeleted'] as const;
const itemStats = (v: unknown) =>
  isObject(v) &&
  Object.keys(v).length === 2 + ITEM_STATS.length &&
  isAlias(v.item) &&
  typeof v.reversed === 'boolean' &&
  ITEM_STATS.every((k) => alStat(AL_NC[k])(v[k]));
const ALPHA_LIMITATIONS = ['stats.alpha.assumes_one_dimension', 'stats.alpha.no_omega'];

/** An alpha exclusion, exactly as stats_alpha writes it (decision 383, 337b's
 *  rule): missing_value or missing_code, each naming its column and nothing
 *  else. */
const alExclusion = (e: unknown) =>
  isObject(e) &&
  oneOf(e.code, ['stats.alpha.missing_value', 'stats.alpha.missing_code']) &&
  isCount(e.count) &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

function alphaBundle(v: O): AlphaResultView | null {
  const d = diagnostic(AL_CODES, AL_PARAM_KEYS, false);
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    isCount(r.k) &&
    r.k >= 2 &&
    isCount(r.rowsUsed) &&
    alStat(AL_NC.alpha)(r.alpha) &&
    alInterval(r.interval) &&
    alStat(AL_NC.standardized)(r.standardized) &&
    interItem(r.interItem) &&
    arrayOf(r.items, itemStats) &&
    (r.items as unknown[]).length === r.k &&
    arrayOf(r.exclusions, alExclusion) &&
    Array.isArray(r.notes) &&
    (r.notes.length === 0 || (r.notes.length === 1 && r.notes[0] === 'stats.alpha.negative_alpha')) &&
    Array.isArray(r.limitations) &&
    JSON.stringify(r.limitations) === JSON.stringify(ALPHA_LIMITATIONS) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, ALPHA_CAPABILITY, alExclusion);
  if (!ok) return null;
  // As Rust's stats_alpha_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as AlphaResultView['result'],
    diagnostics: v.diagnostics as AlphaDiagnostic[],
    manifest: v.manifest as AlphaResultView['manifest'],
  };
}

/* ---- crosstabs (Phase 8X-2, decision 591) ------------------------------------- */

/** The codes stats_xtab writes where a number would be (stats_xtab/mod.rs's
 *  not-computable codes): overflow, a refused chi-square, or a df past 2,401. */
const XT_NC = ['overflow', 'arithmetic_refused', 'distribution_out_of_range'].map((c) => `stats.xtab.${c}`);
const xtNc = (x: unknown) => oneOf(x, XT_NC);
const xtStat = (v: unknown) => oneKey(v, { value: isNum, not_computable: xtNc });
const chiTest = (v: unknown) => isObject(v) && Object.keys(v).length === 3 && [v.statistic, v.df, v.p].every(isNum);
const chiOutcome = (v: unknown) => oneKey(v, { computed: chiTest, not_computable: xtNc });
const fisherTest = (v: unknown) => isObject(v) && Object.keys(v).length === 3 && [v.twoSided, v.less, v.greater].every(isNum);
const levelLabel = (v: unknown) => isObject(v) && Object.keys(v).length === 2 && isCount(v.level) && isStr(v.label);
const FREQUENCY_NUMBERS = ['expected', 'rowProportion', 'columnProportion', 'totalProportion', 'residual', 'pearsonResidual', 'adjustedResidual'];
const frequency = (v: unknown) =>
  isObject(v) &&
  Object.keys(v).length === 3 + FREQUENCY_NUMBERS.length &&
  isCount(v.row) &&
  isCount(v.column) &&
  isCount(v.observed) &&
  FREQUENCY_NUMBERS.every((k) => isNum(v[k]));

/** A crosstabs exclusion, exactly as stats_xtab writes it (stats_xtab/mod.rs's
 *  exclusions, 337b's rule): missing_row, missing_column or missing_code, each
 *  naming its column and nothing else. missing_code can come twice, once for
 *  each column. */
const XT_EXCLUDED = ['missing_row', 'missing_column', 'missing_code'].map((c) => `stats.xtab.${c}`);
const xtExclusion = (e: unknown) =>
  isObject(e) &&
  oneOf(e.code, XT_EXCLUDED) &&
  isCount(e.count) &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

/** A diagnostic as stats_xtab writes it: as every method's, and the suspect-code
 *  warning always names its column, value and count (stats_xtab/mod.rs's
 *  validate), so a stored one missing any is not one Rust wrote. */
const xtDiagnostic = (v: unknown) =>
  diagnostic(XT_CODES, XT_PARAM_KEYS, false)(v) &&
  isObject(v) &&
  (v.code !== 'stats.xtab.suspect_missing_code' || (isObject(v.params) && ['column', 'value', 'count'].every((k) => isNum((v.params as O)[k]))));

/** Every result's limitations, in stats_xtab's order (its LIMITATIONS). */
const XTAB_LIMITATIONS = ['no_layers', 'fisher_two_by_two_only', 'no_further_nominal_measures', 'no_weights', 'code_point_order'].map(
  (c) => `stats.xtab.${c}`
);

/** CrosstabsOutcome's keys, every one always written. */
const XTAB_RESULT_KEYS = [
  'rowLevels',
  'columnLevels',
  'frequencies',
  'rowTotals',
  'columnTotals',
  'n',
  'pearson',
  'likelihoodRatio',
  'continuityCorrected',
  'fisher',
  'phi',
  'cramersV',
  'contingencyCoefficient',
  'expectedBelowFive',
  'cellCount',
  'smallestExpected',
  'exclusions',
  'notes',
  'limitations',
];

function xtabBundle(v: O): XtabResultView | null {
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    Object.keys(r).length === XTAB_RESULT_KEYS.length &&
    XTAB_RESULT_KEYS.every((k) => k in r) &&
    arrayOf(r.rowLevels, levelLabel) &&
    arrayOf(r.columnLevels, levelLabel) &&
    arrayOf(r.frequencies, frequency) &&
    arrayOf(r.rowTotals, isCount) &&
    arrayOf(r.columnTotals, isCount) &&
    isCount(r.n) &&
    chiOutcome(r.pearson) &&
    chiOutcome(r.likelihoodRatio) &&
    (r.continuityCorrected === null || chiOutcome(r.continuityCorrected)) &&
    (r.fisher === null || fisherTest(r.fisher)) &&
    xtStat(r.phi) &&
    xtStat(r.cramersV) &&
    xtStat(r.contingencyCoefficient) &&
    isCount(r.expectedBelowFive) &&
    isCount(r.cellCount) &&
    isNum(r.smallestExpected) &&
    arrayOf(r.exclusions, xtExclusion) &&
    Array.isArray(r.notes) &&
    (r.notes.length === 0 || (r.notes.length === 1 && r.notes[0] === 'stats.xtab.expected_below_five')) &&
    Array.isArray(r.limitations) &&
    JSON.stringify(r.limitations) === JSON.stringify(XTAB_LIMITATIONS) &&
    arrayOf(v.diagnostics, xtDiagnostic) &&
    arrayOf(v.limitations, xtDiagnostic) &&
    manifest(v.manifest, XTAB_CAPABILITY, xtExclusion);
  if (!ok) return null;
  // As Rust's stats_xtab_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as XtabResultView['result'],
    diagnostics: v.diagnostics as XtabDiagnostic[],
    manifest: v.manifest as XtabResultView['manifest'],
  };
}

/* ---- the rank tests (Phase 8Y-2, decision 622) --------------------------------- */

/** The codes stats_ranks writes where a number would be: every value equal, an
 *  overflow, or a df past the chi-square's measured range. */
const RK_NC = ['all_values_tied', 'overflow', 'distribution_out_of_range'].map((c) => `stats.ranks.${c}`);
const rkNc = (x: unknown) => oneOf(x, RK_NC);
const rkStat = (v: unknown) => oneKey(v, { value: isNum, not_computable: rkNc });
/** The codes a normal approximation's z and p can have: no distribution's. */
const RK_NC_NORMAL = RK_NC.filter((c) => c !== 'stats.ranks.distribution_out_of_range');
const rkNormalStat = (v: unknown) => oneKey(v, { value: isNum, not_computable: (x) => oneOf(x, RK_NC_NORMAL) });
const ncCode = (v: unknown): unknown => (isObject(v) ? v.not_computable : undefined);
const rankGroup = (v: unknown) =>
  isObject(v) &&
  Object.keys(v).length === 6 &&
  isCount(v.level) &&
  isStr(v.label) &&
  isCount(v.n) &&
  [v.median, v.rankSum, v.meanRank].every(isNum);

/** The Mann-Whitney form: its eight keys, every one always written; exactP a number
 *  or null. */
const mannWhitney = (v: unknown) =>
  isObject(v) &&
  Object.keys(v).length === 8 &&
  isCount(v.first) &&
  isCount(v.second) &&
  [v.u1, v.u2, v.rankBiserial].every(isNum) &&
  rkNormalStat(v.z) &&
  rkNormalStat(v.p) &&
  'exactP' in v &&
  (v.exactP === null || (isNum(v.exactP) && v.exactP >= 0 && v.exactP <= 1));

/** The Kruskal-Wallis form: its four keys, every one always written. */
const kruskalWallis = (v: unknown) =>
  isObject(v) && Object.keys(v).length === 4 && isNum(v.df) && rkStat(v.h) && rkStat(v.p) && rkStat(v.epsilonSquared);

/** The test the count of groups chose: Mann-Whitney's form for two, Kruskal-Wallis's
 *  for three or more, as stats_ranks chooses. */
const rankTest = (test: unknown, groups: unknown) =>
  Array.isArray(groups) &&
  (groups.length === 2 ? oneKey(test, { mann_whitney: mannWhitney }) : groups.length >= 3 && oneKey(test, { kruskal_wallis: kruskalWallis }));

/** An exclusion, exactly as stats_ranks writes it: one of its four reasons, naming
 *  its column and nothing else. */
const RK_EXCLUDED = ['missing_group', 'other_level', 'missing_outcome', 'missing_code'].map((c) => `stats.ranks.${c}`);
const rkExclusion = (e: unknown) =>
  isObject(e) &&
  Object.keys(e).length === 3 &&
  oneOf(e.code, RK_EXCLUDED) &&
  isCount(e.count) &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

/** A diagnostic as stats_ranks writes it: as every method's, and the suspect-code
 *  warning always names its column, value and count. */
const rkDiagnostic = (v: unknown) =>
  diagnostic(RK_CODES, RK_PARAM_KEYS, false)(v) &&
  isObject(v) &&
  (v.code !== 'stats.ranks.suspect_missing_code' || (isObject(v.params) && ['column', 'value', 'count'].every((k) => isNum((v.params as O)[k]))));

/** The notes stats_ranks writes, in its order. */
const RK_NOTES = ['exact_not_given_ties', 'exact_not_given_size', 'group_below_five'].map((c) => `stats.ranks.${c}`);
const rkNotes = (v: unknown) => Array.isArray(v) && JSON.stringify(v) === JSON.stringify(RK_NOTES.filter((c) => v.indexOf(c) >= 0));

/** A column's number from its alias, as an exclusion names it, or null for a word that is
 *  not an alias (Alias's Deserialize in stats_desc). */
const aliasNumber = (a: unknown): number | null => (isStr(a) && /^C[1-9][0-9]*$/.test(a) ? Number(a.slice(1)) : null);

/** What stats_ranks always writes together (stats_ranks::run, mann_whitney and
 *  kruskal_wallis; decision 635): at most MAX_GROUP_LEVELS groups, their levels strictly
 *  ascending; every value equal only with one tie run. For two groups, the first and
 *  second the groups' own two levels in order, U1 and U2 as the first rank sum gives them, z not
 *  computable only with its p, a ties note exactly when there are tie runs, a size note
 *  exactly when a group has 50 values or more, and an absent exact p exactly when one of
 *  them says why. For three or more, df the groups less one, H not computable only with
 *  its p and epsilon-squared, and no note about an exact p. */
const rkConsistent = (r: O) => {
  const groups = r.groups as Array<{ level: number; n: number; rankSum: number; meanRank: number }>;
  if (groups.length > TTEST_LIMITS.maxGroupLevels) return false;
  if (groups.some((g, i) => i > 0 && g.level <= groups[i - 1].level)) return false;
  const notes = r.notes as string[];
  const tiesNote = notes.indexOf(RK_NOTES[0]) >= 0;
  const sizeNote = notes.indexOf(RK_NOTES[1]) >= 0;
  const tieRuns = r.tieRuns as number;
  const allTied = 'stats.ranks.all_values_tied';
  const test = r.test as O;
  if (!isObject(test.mann_whitney)) {
    const t = test.kruskal_wallis as O;
    const h = ncCode(t.h);
    // H's codes follow from epsilon-squared's here, since H's code is epsilon-squared's below.
    if (!oneOf(ncCode(t.epsilonSquared), RK_NC_NORMAL) && ncCode(t.epsilonSquared) !== undefined) return false;
    if (h !== undefined && (ncCode(t.p) !== h || ncCode(t.epsilonSquared) !== h)) return false;
    if (h !== allTied && (ncCode(t.p) === allTied || ncCode(t.epsilonSquared) === allTied)) return false;
    if (h === allTied && tieRuns !== 1) return false;
    if (h === allTied && !rkOneRun(r)) return false;
    return t.df === groups.length - 1 && !tiesNote && !sizeNote;
  }
  const t = test.mann_whitney;
  const z = ncCode(t.z);
  if (z !== undefined && ncCode(t.p) !== z) return false;
  if (z !== allTied && ncCode(t.p) === allTied) return false;
  if (z === allTied && tieRuns !== 1) return false;
  if (z === allTied && !rkOneRun(r)) return false;
  const [n1, n2] = [groups[0].n, groups[1].n];
  if (t.u1 !== groups[0].rankSum - (n1 * (n1 + 1)) / 2) return false;
  if (t.u2 !== n1 * n2 - (t.u1 as number)) return false;
  if (t.rankBiserial !== (2 * (t.u1 as number)) / (n1 * n2) - 1) return false;
  return (
    t.first === groups[0].level &&
    t.second === groups[1].level &&
    tiesNote === (tieRuns > 0) &&
    sizeNote === groups.some((g) => g.n >= 50) &&
    (t.exactP === null) === (tiesNote || sizeNote)
  );
};

/** One run of N equal values: every group's rank sum is n_i (N + 1) / 2, exactly, since
 *  each rank is the half-integer (N + 1) / 2 (stats_ranks::Ranked::of). */
const rkOneRun = (r: O) => (r.groups as Array<{ n: number; rankSum: number }>).every((g) => g.rankSum === (g.n * ((r.n as number) + 1)) / 2);

/** What the groups, the counts, the notes and the manifest always say of one another
 *  (stats_ranks::run :698-749, split :357-384, manifest :939-941; decision 652). */
const rkCounts = (v: O, r: O) => {
  const groups = r.groups as Array<{ n: number; rankSum: number; meanRank: number }>;
  const m = v.manifest as O;
  const n = r.n as number;
  if (groups.some((g) => g.n < 1)) return false;
  if (groups.some((g) => g.n !== 0 && g.meanRank !== g.rankSum / g.n)) return false;
  if (n !== groups.reduce((s, g) => s + g.n, 0)) return false;
  if ((r.tieRuns as number) > Math.floor(n / 2)) return false;
  if (((r.notes as string[]).indexOf(RK_NOTES[2]) >= 0) !== groups.some((g) => g.n < 5)) return false;
  if (m.rowsUsed !== n) return false;
  return m.rowsRead === n + (r.exclusions as Array<{ count: number }>).reduce((s, e) => s + e.count, 0);
};

/** What only the spec can say (decision 652): each exclusion names the column Rust names
 *  for its code (:730-733), missing_code appears only with missing codes, and the groups
 *  are the spec's levels, which Rust sorts (:544) before it splits. A spec without the
 *  field read is not one Rust wrote. */
const rkAgreesWithSpec = (r: O, spec: O) => {
  const p = spec.parameters;
  if (!isObject(p)) return false;
  const [outcome, group] = [aliasNumber(p.outcome), aliasNumber(p.group)];
  if (outcome === null || group === null || !Array.isArray(p.missingCodes) || !('levels' in p)) return false;
  if (outcome === group) return false;
  const named = (c: unknown) => (c === RK_EXCLUDED[0] || c === RK_EXCLUDED[1] ? group : outcome);
  const exclusions = r.exclusions as Array<{ code: string; params: { column: number } }>;
  if (exclusions.some((e) => e.params.column !== named(e.code))) return false;
  if (p.missingCodes.length === 0 && exclusions.some((e) => e.code === RK_EXCLUDED[3])) return false;
  if (p.levels === null && exclusions.some((e) => e.code === RK_EXCLUDED[1])) return false;
  if (p.levels === null) return true;
  const levels = (p.levels as number[]).slice().sort((a, b) => a - b);
  return JSON.stringify((r.groups as Array<{ level: number }>).map((g) => g.level)) === JSON.stringify(levels);
};

/** The exclusions as stats_ranks lists them: each reason once, in its order, the
 *  manifest's the same. */
const rkExclusionsAgree = (v: O, r: O) => {
  const codes = (r.exclusions as O[]).map((e) => e.code as string);
  return JSON.stringify(codes) === JSON.stringify(RK_EXCLUDED.filter((c) => codes.indexOf(c) >= 0)) && JSON.stringify((v.manifest as O).exclusions) === JSON.stringify(r.exclusions);
};

/** Every result's limitations, in stats_ranks' order (its LIMITATIONS). */
const RANKS_LIMITATIONS = ['no_pairwise_comparisons', 'two_sided_only', 'no_location_shift_estimate', 'exact_without_ties_only', 'no_further_k_sample_tests', 'no_weights', 'code_point_order'].map(
  (c) => `stats.ranks.${c}`
);

/** IndependentRanksOutcome's keys, every one always written. */
const RANKS_RESULT_KEYS = ['groups', 'n', 'tieRuns', 'test', 'exclusions', 'notes', 'limitations'];

function ranksBundle(v: O, spec: O): RanksResultView | null {
  const r = v.primaryOutputs;
  const ok =
    isCount(v.runId) &&
    v.status === 'succeeded' &&
    isObject(r) &&
    Object.keys(r).length === RANKS_RESULT_KEYS.length &&
    RANKS_RESULT_KEYS.every((k) => k in r) &&
    arrayOf(r.groups, rankGroup) &&
    isCount(r.n) &&
    isCount(r.tieRuns) &&
    rankTest(r.test, r.groups) &&
    arrayOf(r.exclusions, rkExclusion) &&
    rkNotes(r.notes) &&
    rkConsistent(r) &&
    Array.isArray(r.limitations) &&
    JSON.stringify(r.limitations) === JSON.stringify(RANKS_LIMITATIONS) &&
    arrayOf(v.diagnostics, rkDiagnostic) &&
    arrayOf(v.limitations, rkDiagnostic) &&
    manifest(v.manifest, RANKS_CAPABILITY, rkExclusion) &&
    rkExclusionsAgree(v, r) &&
    rkCounts(v, r) &&
    rkAgreesWithSpec(r, spec);
  if (!ok) return null;
  // As Rust's stats_ranks_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as RanksResultView['result'],
    diagnostics: v.diagnostics as RanksDiagnostic[],
    manifest: v.manifest as RanksResultView['manifest'],
  };
}

/* ---- the related rank tests (Phase 8Z-2, decision 657) ------------------------ */

const rrCode = (c: string) => `stats.related_ranks.${c}`;
const RR_TIED = rrCode('all_values_tied');
const RR_OVERFLOW = rrCode('overflow');
const RR_RANGE = rrCode('distribution_out_of_range');
const RR_ZERO = rrCode('all_differences_zero');
const rrStat = (v: unknown, codes: readonly string[]) => oneKey(v, { value: isNum, not_computable: (x) => oneOf(x, codes) });
/** The number in a stat; undefined for a code. */
const statValue = (v: unknown): unknown => (isObject(v) ? v.value : undefined);

/** A diagnostic as stats_related_ranks writes it: as every method's, and the
 *  suspect-code warning always names its column, value and count. */
const rrDiagnostic = (v: unknown) =>
  diagnostic(RR_CODES, RR_PARAM_KEYS, false)(v) &&
  isObject(v) &&
  (v.code !== 'stats.related_ranks.suspect_missing_code' || (isObject(v.params) && ['column', 'value', 'count'].every((k) => isNum((v.params as O)[k]))));

/** Every result's limitations, in stats_related_ranks' order (its LIMITATIONS). */
const RELATED_LIMITATIONS = ['no_pairwise_comparisons', 'two_sided_only', 'no_location_estimate', 'exact_without_ties_or_zeros_only', 'pratt_not_offered', 'no_further_related_tests', 'no_weights', 'column_order'].map(rrCode);

/** RelatedRanksOutcome's keys, every one always written. */
const RELATED_RESULT_KEYS = ['columns', 'rowsUsed', 'test', 'exclusions', 'notes', 'limitations'];
const WILCOXON_KEYS = ['first', 'second', 'n', 'zeroDifferences', 'tieRuns', 'negative', 'positive', 'v', 'tMinus', 'z', 'p', 'exactP', 'rankBiserial', 'medianDifference'];
const FRIEDMAN_KEYS = ['columns', 'q', 'df', 'p', 'kendallsW'];
const hasExactly = (v: O, keys: readonly string[]) => Object.keys(v).length === keys.length && keys.every((k) => k in v);

/** Each of a Wilcoxon test's two sides (stats_related_ranks::sign_ranks): its count whole,
 *  its rank sum a multiple of 0.5 within what c ranks of n can reach, its mean null exactly
 *  when it has no pair. */
const rrSide = (v: unknown, n: number) => {
  if (!isObject(v) || !hasExactly(v, ['count', 'rankSum', 'meanRank'])) return false;
  if (!isCount(v.count) || !isNum(v.rankSum)) return false;
  const [c, s] = [v.count, v.rankSum];
  if (!Number.isInteger(s * 2)) return false;
  if (s < (c * (c + 1)) / 2 || s > (c * (2 * n - c + 1)) / 2) return false;
  return c === 0 ? v.meanRank === null : v.meanRank === s / c;
};

/** The Wilcoxon form (stats_related_ranks::wilcoxon :716-790): decision 657's rules,
 *  each its own line, each expression in Rust's order. */
const rrWilcoxon = (v: unknown, r: O, columns: string[]) => {
  if (!isObject(v) || !hasExactly(v, WILCOXON_KEYS)) return false;
  if (v.first !== columns[0] || v.second !== columns[1]) return false;
  if (!isCount(v.n) || !isCount(v.zeroDifferences) || !isCount(v.tieRuns)) return false;
  const n = v.n;
  if (!rrSide(v.negative, n) || !rrSide(v.positive, n)) return false;
  const [negative, positive] = [v.negative as { count: number; rankSum: number }, v.positive as { count: number; rankSum: number }];
  if (n !== negative.count + positive.count) return false;
  if (r.rowsUsed !== n + v.zeroDifferences) return false;
  if (v.tieRuns > Math.floor(n / 2)) return false;
  if (v.tieRuns === 0 && (!Number.isInteger(negative.rankSum) || !Number.isInteger(positive.rankSum))) return false;
  if (v.v !== positive.rankSum) return false;
  const top = (n * (n + 1)) / 2;
  if (v.tMinus !== top - (v.v as number)) return false;
  if (negative.rankSum !== v.tMinus) return false;
  if ((n === 0) !== (ncCode(v.z) === RR_ZERO && ncCode(v.p) === RR_ZERO && ncCode(v.rankBiserial) === RR_ZERO)) return false;
  if (n > 0) {
    if (!rrStat(v.z, [RR_OVERFLOW]) || !rrStat(v.p, [RR_OVERFLOW]) || !rrStat(v.rankBiserial, [RR_OVERFLOW])) return false;
    if (ncCode(v.z) !== undefined && ncCode(v.p) !== ncCode(v.z)) return false;
    if (statValue(v.p) !== undefined && ((statValue(v.p) as number) < 0 || (statValue(v.p) as number) > 1)) return false;
    if (statValue(v.z) !== undefined && Math.sign(statValue(v.z) as number) !== Math.sign((v.v as number) - (v.tMinus as number))) return false;
    if (statValue(v.rankBiserial) !== undefined && statValue(v.rankBiserial) !== ((v.v as number) - (v.tMinus as number)) / top) return false;
  }
  if (!rrStat(v.medianDifference, [RR_OVERFLOW])) return false;
  const notes = r.notes as string[];
  const has = (c: string) => notes.indexOf(c) >= 0;
  const [ties, zeros, size, dropped, below] = [rrCode('exact_not_given_ties'), rrCode('exact_not_given_zeros'), rrCode('exact_not_given_size'), rrCode('zero_differences_dropped'), rrCode('rows_below_five')];
  if (has(ties) !== (v.tieRuns > 0)) return false;
  if (has(zeros) !== (v.zeroDifferences > 0)) return false;
  if (has(size) !== (n >= 50)) return false;
  if (has(dropped) !== (v.zeroDifferences > 0)) return false;
  if (has(below) !== (n < 5)) return false;
  if (JSON.stringify(notes) !== JSON.stringify([ties, zeros, size, dropped, below].filter(has))) return false;
  const noExact = has(ties) || has(zeros) || has(size);
  if (noExact) return v.exactP === null;
  return isNum(v.exactP) && v.exactP > 0 && v.exactP <= 1;
};

/** The Friedman form (stats_related_ranks::friedman :809-856, q_p_w :793-806). */
const rrFriedman = (v: unknown, r: O, columns: string[]) => {
  if (!isObject(v) || !hasExactly(v, FRIEDMAN_KEYS)) return false;
  const k = columns.length;
  const rows = r.rowsUsed as number;
  if (!Array.isArray(v.columns) || v.columns.length !== k) return false;
  const sums: number[] = [];
  for (let i = 0; i < k; i += 1) {
    const c = v.columns[i];
    if (!isObject(c) || !hasExactly(c, ['column', 'rankSum', 'meanRank']) || c.column !== columns[i] || !isNum(c.rankSum)) return false;
    if (c.meanRank !== c.rankSum / rows) return false;
    if (c.rankSum < rows || c.rankSum > rows * k) return false;
    sums.push(c.rankSum);
  }
  if (sums.reduce((s, x) => s + x, 0) !== (rows * k * (k + 1)) / 2) return false;
  if (v.df !== k - 1) return false;
  const equalSums = sums.every((s) => s === (rows * (k + 1)) / 2);
  if (!rrStat(v.q, [RR_TIED, RR_OVERFLOW]) || !rrStat(v.p, [RR_TIED, RR_OVERFLOW, RR_RANGE]) || !rrStat(v.kendallsW, [RR_TIED, RR_OVERFLOW])) return false;
  const q = ncCode(v.q);
  const anyTied = [q, ncCode(v.p), ncCode(v.kendallsW)].indexOf(RR_TIED) >= 0;
  if (anyTied && !(q === RR_TIED && ncCode(v.p) === RR_TIED && ncCode(v.kendallsW) === RR_TIED)) return false;
  if (q === RR_TIED && !equalSums) return false;
  if (q !== undefined && (ncCode(v.p) !== q || ncCode(v.kendallsW) !== q)) return false;
  if (q === undefined) {
    const value = statValue(v.q) as number;
    if (value < 0) return false;
    if ((value === 0) !== equalSums) return false;
    const p = statValue(v.p);
    if (p !== undefined && ((p as number) < 0 || (p as number) > 1)) return false;
    const w = statValue(v.kendallsW);
    if (w !== undefined && w !== value / (rows * (k - 1))) return false;
  }
  const below = rrCode('rows_below_five');
  return JSON.stringify(r.notes) === JSON.stringify(rows < 5 ? [below] : []);
};

/** An exclusion, exactly as stats_related_ranks writes it. */
const RR_EXCLUDED = [rrCode('missing_value'), rrCode('missing_code')];
const rrExclusion = (e: unknown) =>
  isObject(e) &&
  Object.keys(e).length === 3 &&
  oneOf(e.code, RR_EXCLUDED) &&
  isCount(e.count) &&
  e.count >= 1 &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

/** The exclusions as stats_related_ranks lists them (:638-652): in the spec's column order,
 *  missing_value before missing_code for the same column, each pair once, each column the
 *  spec's, and missing_code only with missing codes. */
const rrExclusionsAgree = (list: O[], columns: string[], codes: unknown[]) => {
  const numbers = columns.map(aliasNumber);
  const order = list.map((e) => numbers.indexOf(((e.params as O).column as number) ?? -1) * 2 + (e.code === RR_EXCLUDED[1] ? 1 : 0));
  if (order.some((o) => o < 0)) return false;
  if (order.some((o, i) => i > 0 && o <= order[i - 1])) return false;
  return codes.length > 0 || list.every((e) => e.code !== RR_EXCLUDED[1]);
};

function relatedRanksBundle(v: O, spec: O): RelatedRanksResultView | null {
  const p = spec.parameters;
  const r = v.primaryOutputs;
  if (!isObject(p) || !Array.isArray(p.columns) || !Array.isArray(p.missingCodes)) return null;
  const columns = p.columns as string[];
  if (columns.length < 2 || columns.length > RELATED_LIMITS.maxColumns || columns.some((c) => aliasNumber(c) === null)) return null;
  if (columns.some((c, i) => columns.indexOf(c) !== i)) return null;
  if (!isObject(r) || !hasExactly(r, RELATED_RESULT_KEYS)) return null;
  if (!isCount(v.runId) || v.status !== 'succeeded') return null;
  if (!isCount(r.rowsUsed) || r.rowsUsed < 1) return null;
  if (!Array.isArray(r.columns) || r.columns.length !== columns.length) return null;
  for (let i = 0; i < columns.length; i += 1) {
    const c = r.columns[i];
    if (!isObject(c) || !hasExactly(c, ['column', 'median']) || c.column !== columns[i] || !isNum(c.median)) return null;
  }
  if (!Array.isArray(r.notes) || !arrayOf(r.exclusions, rrExclusion)) return null;
  const test = columns.length === 2 ? oneKey(r.test, { wilcoxon: (t) => rrWilcoxon(t, r, columns) }) : oneKey(r.test, { friedman: (t) => rrFriedman(t, r, columns) });
  if (!test) return null;
  if (!rrExclusionsAgree(r.exclusions as O[], columns, p.missingCodes)) return null;
  if (!Array.isArray(r.limitations) || JSON.stringify(r.limitations) !== JSON.stringify(RELATED_LIMITATIONS)) return null;
  if (!arrayOf(v.diagnostics, rrDiagnostic) || !arrayOf(v.limitations, rrDiagnostic)) return null;
  if (!manifest(v.manifest, RELATED_RANKS_CAPABILITY, rrExclusion)) return null;
  const m = v.manifest as O;
  if (m.rowsUsed !== r.rowsUsed) return null;
  if (m.rowsRead !== r.rowsUsed + (r.exclusions as Array<{ count: number }>).reduce((s, e) => s + e.count, 0)) return null;
  if (JSON.stringify(m.exclusions) !== JSON.stringify(r.exclusions)) return null;
  // As Rust's stats_related_ranks_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as RelatedRanksResultView['result'],
    diagnostics: v.diagnostics as RelatedRanksDiagnostic[],
    manifest: v.manifest as RelatedRanksResultView['manifest'],
  };
}

/* ---- the checkers, one per method (Phase 8C; decision 249) ------------------- */

/** How one method's saved result is checked: the case it is shown as, its
 *  code list, its parameter keys, whether a parameter may be a string, and
 *  its bundle's check. */
interface StoredChecker {
  which: CaseKind;
  codes: readonly string[];
  keys: readonly string[];
  strings: boolean;
  /** `spec` is the stored spec, already parsed (decision 652): a checker that reads it
   *  answers null when a field it reads is absent. */
  bundle: (v: O, dataFile: DataFileFacts, spec: O) => Checked | null;
}

/** The checkers, keyed by capability id. A new method adds one entry. */
export const STORED_CHECKERS: Record<string, StoredChecker> = {
  [SUMMARY_CAPABILITY]: {
    which: 'summary',
    codes: DESC_CODES,
    keys: DESC_PARAM_KEYS,
    strings: true,
    bundle: (v, dataFile) => {
      const result = summaryBundle(v);
      return result ? { kind: 'summary', result, dataFile } : null;
    },
  },
  [TTEST_CAPABILITY]: {
    which: 'ttest',
    codes: T_CODES,
    keys: T_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = ttestBundle(v);
      return result ? { kind: 'ttest', result, dataFile } : null;
    },
  },
  [ONE_SAMPLE_CAPABILITY]: {
    which: 'one_sample',
    codes: OS_CODES,
    keys: OS_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = oneSampleBundle(v);
      return result ? { kind: 'one_sample', result, dataFile } : null;
    },
  },
  [PAIRED_CAPABILITY]: {
    which: 'paired',
    codes: OS_CODES,
    keys: OS_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = pairedBundle(v);
      return result ? { kind: 'paired', result, dataFile } : null;
    },
  },
  [ANOVA_CAPABILITY]: {
    which: 'anova',
    codes: AN_CODES,
    keys: AN_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = anovaBundle(v);
      return result ? { kind: 'anova', result, dataFile } : null;
    },
  },
  [CORR_CAPABILITY]: {
    which: 'correlations',
    codes: CR_CODES,
    keys: CR_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = corrBundle(v);
      return result ? { kind: 'correlations', result, dataFile } : null;
    },
  },
  [ALPHA_CAPABILITY]: {
    which: 'cronbach_alpha',
    codes: AL_CODES,
    keys: AL_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = alphaBundle(v);
      return result ? { kind: 'cronbach_alpha', result, dataFile } : null;
    },
  },
  // Research OS, Phase 8X-2 (decision 591).
  [XTAB_CAPABILITY]: {
    which: 'crosstabs',
    codes: XT_CODES,
    keys: XT_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile) => {
      const result = xtabBundle(v);
      return result ? { kind: 'crosstabs', result, dataFile } : null;
    },
  },
  // Research OS, Phase 8Y-2 (decision 622).
  [RANKS_CAPABILITY]: {
    which: 'independent_ranks',
    codes: RK_CODES,
    keys: RK_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile, spec) => {
      const result = ranksBundle(v, spec);
      return result ? { kind: 'independent_ranks', result, dataFile } : null;
    },
  },
  // Research OS, Phase 8Z-2 (decision 657).
  [RELATED_RANKS_CAPABILITY]: {
    which: 'related_ranks',
    codes: RR_CODES,
    keys: RR_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile, spec) => {
      const result = relatedRanksBundle(v, spec);
      return result ? { kind: 'related_ranks', result, dataFile } : null;
    },
  },
};

/** A capability's checker, or undefined for one no method has. An own key
 *  only, never one from Object.prototype. */
function checkerFor(capability: string): StoredChecker | undefined {
  return Object.prototype.hasOwnProperty.call(STORED_CHECKERS, capability) ? STORED_CHECKERS[capability] : undefined;
}

/** The capability id whose checked results are shown as `which`. */
export function capabilityOfCase(which: CaseKind): string {
  for (const id of Object.keys(STORED_CHECKERS)) if (STORED_CHECKERS[id].which === which) return id;
  throw new Error('a case with no checker');
}

/* ---- the whole saved result ------------------------------------------------- */

/** JSON text to a value, or undefined. The error, whose message can quote
 *  the text, is dropped here. */
function parsed(text: unknown): unknown {
  if (!isStr(text)) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** A stored part's schema version: known, another one, or no version at all. */
function version(v: unknown): 'known' | 'other' | 'none' {
  if (!isObject(v) || !isNum(v.schemaVersion)) return 'none';
  return v.schemaVersion === KNOWN_SCHEMA_VERSION ? 'known' : 'other';
}

/** The whole check, from project_run_result's answer (a RunResultView, as
 *  far as anything says). Never throws. */
export function checkStored(view: unknown): Checked {
  try {
    return check(view);
  } catch {
    return { kind: 'badShape' };
  }
}

function check(view: unknown): Checked {
  if (!isObject(view)) return { kind: 'badShape' };
  const capability = view.capabilityId;
  if (!isStr(capability)) return { kind: 'badShape' };
  const checker = checkerFor(capability);
  if (checker === undefined) return { kind: 'unknownCapability' };
  const which = checker.which;
  const f = view.dataFile;
  if (!isCount(view.storedRunId) || !oneOf(view.state, RUN_STATES) || !isObject(f)) return { kind: 'badShape' };
  if (f.kind !== 'data_file' || !oneOf(f.format, DATA_FORMATS) || !isCount(f.bytes) || !isHex64(f.sha256)) return { kind: 'badShape' };
  const dataFile = { format: f.format as string, bytes: f.bytes as number };

  const spec = parsed(view.specJson);
  const parts: unknown[] = [spec];
  let bundle: unknown;
  let record: unknown;
  if (view.state === 'succeeded') {
    bundle = parsed(view.bundleJson);
    parts.push(bundle, parsed(view.manifestJson));
  } else if (view.state === 'failed') {
    record = parsed(view.failureJson);
    parts.push(record);
  } else {
    return { kind: 'badShape' };
  }
  const versions = parts.map(version);
  if (versions.indexOf('other') >= 0) return { kind: 'unknownSchema' };
  if (versions.indexOf('none') >= 0) return { kind: 'badShape' };
  if (!isObject(spec) || spec.capabilityId !== capability) return { kind: 'badShape' };
  if ('preparation' in spec && !preparationShape(spec.preparation)) return { kind: 'badShape' };

  if (view.state === 'failed') {
    if (!isObject(record) || record.status !== 'failed') return { kind: 'badShape' };
    const ok = diagnostic(checker.codes, checker.keys, checker.strings);
    if (!ok(record.failure)) return { kind: 'badShape' };
    return { kind: 'failed', capability: which, failure: record.failure as StoredFailure, dataFile };
  }
  if (!isObject(bundle)) return { kind: 'badShape' };
  if (!preparationsAgree(spec, bundle.manifest)) return { kind: 'badShape' };
  return checker.bundle(bundle, dataFile, spec) ?? { kind: 'badShape' };
}
