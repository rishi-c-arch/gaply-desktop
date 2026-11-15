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
import type { LinearRegressionDiagnostic, LinearRegressionResultView } from '../linearRegressionTypes';
import type { FactorDiagnostic, FactorResultView } from '../factorTypes';
import type { CfaDiagnostic, CfaResultView } from '../cfaTypes';
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
import LINEAR_REGRESSION_CODES from '../linearRegressionCodes.json';
import LR_LIMITS from '../fixtures/linearRegressionLimits.json';
import FACTOR_CODES from '../factorCodes.json';
import FA_LIMITS from '../fixtures/factorLimits.json';
import CFA_CODES from '../cfaCodes.json';
import CFA_LIMITS from '../fixtures/cfaLimits.json';
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
export const LINEAR_REGRESSION_CAPABILITY = capabilityOfKind('linear_regression');
export const FACTOR_CAPABILITY = capabilityOfKind('factor');
export const CFA_CAPABILITY = capabilityOfKind('cfa');

/** A data file's formats: the ingest's (DataFormat in Rust, without json). */
const DATA_FORMATS = ['csv', 'tsv', 'xlsx', 'xlsm', 'xlsb', 'xls', 'ods', 'sav', 'mat'];

/** The case a method's checked results are shown as. */
export type CaseKind =
  | 'summary'
  | 'ttest'
  | 'one_sample'
  | 'paired'
  | 'anova'
  | 'correlations'
  | 'cronbach_alpha'
  | 'crosstabs'
  | 'independent_ranks'
  | 'related_ranks'
  | 'linear_regression'
  | 'factor'
  | 'cfa';

/** A failed run's stored diagnostic, in its method's codes. */
export type StoredFailure =
  | Diagnostic
  | TTestDiagnostic
  | OneSampleDiagnostic
  | AnovaDiagnostic
  | CorrDiagnostic
  | AlphaDiagnostic
  | XtabDiagnostic
  | RanksDiagnostic
  | RelatedRanksDiagnostic
  | LinearRegressionDiagnostic
  | FactorDiagnostic
  | CfaDiagnostic;

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
  | { kind: 'linear_regression'; result: LinearRegressionResultView; dataFile: DataFileFacts }
  | { kind: 'factor'; result: FactorResultView; dataFile: DataFileFacts }
  | { kind: 'cfa'; result: CfaResultView; dataFile: DataFileFacts }
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
const LR_CODES = (LINEAR_REGRESSION_CODES as string[]).filter((c) => c.indexOf('stats.linear_regression.') === 0);
const FA_CODES = (FACTOR_CODES as string[]).filter((c) => c.indexOf('stats.factor.') === 0);
const CF_CODES = (CFA_CODES as string[]).filter((c) => c.indexOf('stats.cfa.') === 0);
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
const LR_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const FA_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const CF_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
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
  // Decision 668: an exact p, when present, is above 0 (stats_dist::exact_two_sided).
  (v.exactP === null || (isNum(v.exactP) && v.exactP > 0 && v.exactP <= 1));

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
    // Decision 668, in stats_ranks::kruskal_wallis's order (:888-903): a numeric H is 0 or more, and 0
    // exactly when every Dᵢ = Rᵢ − nᵢ (N + 1) / 2 is 0 (:891); then its p in [0, 1] (:899) and
    // epsilon-squared H / (N − 1), always a number beside a numeric H (:903).
    if (h === undefined) {
      const value = statValue(t.h) as number;
      const big = r.n as number;
      const centred = groups.every((g) => g.rankSum - (g.n * (big + 1)) / 2 === 0);
      if (value < 0 || (value === 0) !== centred) return false;
      // Decision 683 (finding 5): with no tie runs the tie sum is 0, so the correction is exactly
      // 1 (:895) and H is Rust's value, the spread summed in the groups' order (:888-894, :896).
      if (tieRuns === 0) {
        const spread = groups.reduce((s, g) => {
          const d = g.rankSum - (g.n * (big + 1)) / 2;
          return s + (d * d) / g.n;
        }, 0);
        const correction = 1 - 0 / (big * big * big - big);
        if (value !== ((12 / (big * (big + 1))) * spread) / correction) return false;
      }
      const p = statValue(t.p);
      if (p !== undefined && ((p as number) < 0 || (p as number) > 1)) return false;
      if (statValue(t.epsilonSquared) !== value / (big - 1)) return false;
    }
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
  // Decision 668, in stats_ranks::mann_whitney's order (:829-835): z's sign that of u1 − n1 n2 / 2,
  // 0 exactly when u1 = n1 n2 / 2 (:831); with no tie runs the tie sum is 0 and z is Rust's value
  // (:829-831); then its p in [0, 1] (:833).
  const u1 = t.u1 as number;
  const zValue = statValue(t.z);
  if (zValue !== undefined && Math.sign(zValue as number) !== Math.sign(u1 - (n1 * n2) / 2)) return false;
  if (tieRuns === 0) {
    const bigN = r.n as number;
    const variance = ((n1 * n2) / 12) * (bigN + 1 - 0 / (bigN * (bigN - 1)));
    if (zValue !== (u1 - (n1 * n2) / 2) / Math.sqrt(variance)) return false;
  }
  const pValue = statValue(t.p);
  if (pValue !== undefined && ((pValue as number) < 0 || (pValue as number) > 1)) return false;
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
  // Decision 668: with no levels given, Rust includes every level, 0 to k − 1 (stats_ranks/mod.rs:529-530),
  // and refuses an empty group, so the groups are exactly those.
  if (p.levels === null) return (r.groups as Array<{ level: number }>).every((g, i) => g.level === i);
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
  if (n === 0) {
    // Decision 668: each of the three exactly {not_computable: all_differences_zero}, no other key,
    // and the median of differences that are all zero a value equal to 0 (:722, :740-741).
    if (!rrStat(v.z, [RR_ZERO]) || !rrStat(v.p, [RR_ZERO]) || !rrStat(v.rankBiserial, [RR_ZERO])) return false;
    if (!rrStat(v.medianDifference, []) || statValue(v.medianDifference) !== 0) return false;
  }
  if (n > 0) {
    if (!rrStat(v.z, [RR_OVERFLOW]) || !rrStat(v.p, [RR_OVERFLOW]) || !rrStat(v.rankBiserial, [RR_OVERFLOW])) return false;
    if (ncCode(v.z) !== undefined && ncCode(v.p) !== ncCode(v.z)) return false;
    if (statValue(v.p) !== undefined && ((statValue(v.p) as number) < 0 || (statValue(v.p) as number) > 1)) return false;
    if (statValue(v.z) !== undefined && Math.sign(statValue(v.z) as number) !== Math.sign((v.v as number) - (v.tMinus as number))) return false;
    // Decision 668: with no tie runs the tie sum is 0, so z is z_and_p's value from V and n (:700-701).
    if (v.tieRuns === 0) {
      const variance = (n * (n + 1) * (2 * n + 1)) / 24 - 0 / 48;
      if (statValue(v.z) !== ((v.v as number) - (n * (n + 1)) / 4) / Math.sqrt(variance)) return false;
    }
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

/* ---- the linear regression (Research OS, Phase 9A-2, decision 688) ------------------ */

/* Every rule below is stats_linear_regression::model's (mod.rs), cited by line, and
 * equality is as doubles in Rust's order. Rust writes every result through `stat`
 * (:722-730: a finite value, NaN too_small, an infinity overflow) or `own` (:739-745:
 * a subnormal value too_small); lrStatOf and lrOwnOf are those two, so a field Rust
 * computes from stored fields is compared with what Rust would have written. */

const lrCode = (c: string) => `stats.linear_regression.${c}`;
const LR_PERFECT = lrCode('perfect_fit');
const LR_OVERFLOW = lrCode('overflow');
const LR_SMALL = lrCode('too_small');
const LR_RANGE = lrCode('distribution_out_of_range');
const LR_NO_SPREAD = lrCode('predicted_no_spread');
const LR_NOTE = lrCode('essentially_perfect_fit');
/** f64::MIN_POSITIVE, the smallest normal double. */
const MIN_NORMAL = 2.2250738585072014e-308;
/** stats_linear_regression::NO_SPREAD_FACTOR (:758) and ESSENTIALLY_PERFECT_FACTOR (:104). */
const NO_SPREAD_FACTOR = 8;
const ESSENTIALLY_PERFECT_FACTOR = 1e-30;

const lrStat = (v: unknown, codes: readonly string[]) => oneKey(v, { value: isNum, not_computable: (x) => oneOf(x, codes) });
const subnormalOf = (x: number) => x !== 0 && Math.abs(x) < MIN_NORMAL;
type Expected = { value: number } | { code: string };
/** Rust's `stat` (:722-730). */
const lrStatOf = (x: number): Expected => (Number.isFinite(x) ? { value: x } : Number.isNaN(x) ? { code: LR_SMALL } : { code: LR_OVERFLOW });
/** Rust's `own` (:739-745). */
const lrOwnOf = (x: number): Expected => (subnormalOf(x) ? { code: LR_SMALL } : lrStatOf(x));
/** A stored statistic is what Rust would have written. */
const lrIs = (stored: unknown, e: Expected) => ('value' in e ? statValue(stored) === e.value : ncCode(stored) === e.code);
const num = (v: unknown) => statValue(v) as number;
const has = (v: unknown) => statValue(v) !== undefined;

/** LinearRegressionOutcome's keys and each object's, every one always written. */
const LR_RESULT_KEYS = ['outcome', 'rowsUsed', 'columns', 'model', 'anova', 'intercept', 'predictors', 'residuals', 'exclusions', 'notes', 'limitations'];
const LR_COLUMN_KEYS = ['column', 'mean', 'sd'];
const LR_MODEL_KEYS = ['r', 'rSquared', 'adjustedRSquared', 'standardErrorOfEstimate', 'durbinWatson'];
const LR_ANOVA_KEYS = ['regression', 'residual', 'total', 'f', 'p'];
const LR_SOURCE_KEYS = ['sumOfSquares', 'df', 'meanSquare'];
const LR_TOTAL_KEYS = ['sumOfSquares', 'df'];
const LR_ESTIMATE_KEYS = ['b', 'standardError', 't', 'p', 'lower', 'upper'];
const LR_PREDICTOR_KEYS = ['column', 'b', 'standardError', 'beta', 't', 'p', 'lower', 'upper', 'zeroOrder', 'partial', 'part', 'tolerance', 'vif'];
const LR_RESIDUAL_KEYS = ['predicted', 'residual', 'standardizedPredicted', 'standardizedResidual'];
const LR_SPREAD_KEYS = ['minimum', 'maximum', 'mean', 'standardDeviation'];
const LR_EXTREMES_KEYS = ['minimum', 'maximum'];

/** Every result's limitations, in stats_linear_regression's order (its LIMITATIONS, :367-378). */
const LINEAR_REGRESSION_LIMITATIONS = [
  'enter_only',
  'no_categorical_predictors',
  'no_interaction_terms',
  'intercept_always',
  'listwise_only',
  'no_robust_standard_errors',
  'no_casewise_diagnostics',
  'durbin_watson_row_order',
  'intervals_95_only',
  'no_weights',
].map(lrCode);

/** The codes Rust can write in each field after Part 0 (FIND FIRST of 9A-2): an estimate's
 *  five from `estimate` (:787-822), interval_95's two codes and two_sided_p's (stats_ttest
 *  :848-877); F and its p from :954-970; partial and part t's code or perfect_fit (:1010-1014);
 *  tolerance and VIF too_small, or a VIF that overflows (:1015-1033); beta `own` (:1038). */
const LR_ESTIMATE_CODES = [LR_PERFECT, LR_OVERFLOW, LR_SMALL];
const LR_P_CODES = [LR_PERFECT, LR_OVERFLOW, LR_SMALL, LR_RANGE];

/** A diagnostic as stats_linear_regression writes it: as every method's, and the suspect-code
 *  warning always names its column, value and count. */
const lrDiagnostic = (v: unknown) =>
  diagnostic(LR_CODES, LR_PARAM_KEYS, false)(v) &&
  isObject(v) &&
  (v.code !== lrCode('suspect_missing_code') || (isObject(v.params) && ['column', 'value', 'count'].every((k) => isNum((v.params as O)[k]))));

/** An exclusion, exactly as stats_linear_regression writes it (:1062-1076). */
const LR_EXCLUDED = [lrCode('missing_value'), lrCode('missing_code')];
const lrExclusion = (e: unknown) =>
  isObject(e) &&
  Object.keys(e).length === 3 &&
  oneOf(e.code, LR_EXCLUDED) &&
  isCount(e.count) &&
  e.count >= 1 &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

/** The exclusions in the model's column order, the outcome first (:1063, model_columns),
 *  missing_value before missing_code for one column, each pair once, each column one of the
 *  model's, and missing_code only with missing codes. */
const lrExclusionsAgree = (list: O[], columns: string[], codes: unknown[]) => {
  const numbers = columns.map(aliasNumber);
  const order = list.map((e) => numbers.indexOf(((e.params as O).column as number) ?? -1) * 2 + (e.code === LR_EXCLUDED[1] ? 1 : 0));
  if (order.some((o) => o < 0)) return false;
  if (order.some((o, i) => i > 0 && o <= order[i - 1])) return false;
  return codes.length > 0 || list.every((e) => e.code !== LR_EXCLUDED[1]);
};

/** stats_dist::MEASURED_MAX_DF (stats_dist/mod.rs:135): above it t_quantile_975 and
 *  t_two_tailed refuse (their `measured`, :162-164), so interval_95 and two_sided_p give
 *  distribution_out_of_range (stats_ttest :856, :873-875); at or below it neither does. */
const MEASURED_MAX_DF = 1e7;

/** One coefficient's five, as `estimate` writes them (:787-822), at the residual df. */
const lrEstimate = (c: O, perfect: boolean, df: number) => {
  if (!lrStat(c.b, [])) return false;
  const fields = [c.standardError, c.t, c.p, c.lower, c.upper];
  if (!lrStat(c.standardError, LR_ESTIMATE_CODES) || !lrStat(c.t, LR_ESTIMATE_CODES) || !lrStat(c.p, LR_P_CODES)) return false;
  if (!lrStat(c.lower, LR_P_CODES) || !lrStat(c.upper, LR_P_CODES)) return false;
  // The perfect-fit set, both ways (:788-790).
  if (perfect) return fields.every((f) => ncCode(f) === LR_PERFECT);
  if (fields.some((f) => ncCode(f) === LR_PERFECT)) return false;
  // interval_95 gives two finite ends or one code for both (stats_ttest :848-859).
  if (has(c.lower) ? !has(c.upper) : ncCode(c.lower) !== ncCode(c.upper)) return false;
  const b = num(c.b);
  if (has(c.lower) && !(num(c.lower) <= b && b <= num(c.upper))) return false;
  const se = ncCode(c.standardError);
  // An s_j or an SE that cannot be held: all five the same code (:791-805).
  if (se !== undefined) return [c.t, c.p, c.lower, c.upper].every((f) => ncCode(f) === se);
  // An SE Rust writes as a value is normal: a subnormal one is too_small (:808-810; decision 698).
  if (!(num(c.standardError) >= MIN_NORMAL)) return false;
  const t = ncCode(c.t);
  const ratio = b / num(c.standardError);
  // t overflows: p and the interval too (:810-813); t too small: p too (:814-816).
  if (t === LR_OVERFLOW) return !Number.isFinite(ratio) && [c.p, c.lower, c.upper].every((f) => ncCode(f) === LR_OVERFLOW);
  // The interval otherwise (decision 698): interval_95 gives two values, overflow, or
  // distribution_out_of_range exactly when df is above 1e7, and never too_small (stats_ttest
  // :848-859).
  const big = df > MEASURED_MAX_DF;
  const ends = ncCode(c.lower);
  if (big ? ends !== LR_RANGE : ends === LR_RANGE || ends === LR_SMALL) return false;
  if (t === LR_SMALL) return subnormalOf(ratio) && ncCode(c.p) === LR_SMALL;
  if (t !== undefined) return false;
  if (num(c.t) !== ratio || subnormalOf(ratio)) return false;
  // two_sided_p beside a numeric t (:817-820; decision 698): distribution_out_of_range exactly
  // when df is above 1e7; otherwise a p in [0, 1], since t_two_tailed answers every finite t at
  // a measured df (stats_dist :188-198, stats_l0/pvalue.rs:51-66), so overflow needs a t code.
  if (big) return ncCode(c.p) === LR_RANGE;
  return has(c.p) && num(c.p) >= 0 && num(c.p) <= 1;
};

/** Rust's no_spread rule (:863-904), from the stored fields, in its order. */
const lrNoSpread = (r: O): boolean => {
  const anova = r.anova as O;
  const mss = num((anova.regression as O).sumOfSquares);
  if (mss === 0) return true;
  const predicted = (r.residuals as O).predicted as O;
  const columns = r.columns as O[];
  const predictors = r.predictors as O[];
  if (![predicted.standardDeviation, predicted.minimum, predicted.maximum, columns[0].sd].every(has)) return false;
  const [sHat, min, max, sY] = [num(predicted.standardDeviation), num(predicted.minimum), num(predicted.maximum), num(columns[0].sd)];
  const m = Math.max(Math.abs(min), Math.abs(max));
  const n = r.rowsUsed as number;
  const one = predictors.length === 1;
  let kappa = 0;
  for (let j = 0; j < predictors.length; j += 1) {
    const vif = one ? 1 : has(predictors[j].vif) ? num(predictors[j].vif) : undefined;
    const c = columns[j + 1];
    if (vif === undefined || !has(c.mean) || !has(c.sd)) {
      kappa = 0;
      break;
    }
    const ratio = num(c.mean) / num(c.sd);
    const g = Math.sqrt(vif) * Math.sqrt(1 + (n / (n - 1)) * (ratio * ratio));
    kappa = Math.max(kappa, g);
  }
  return sHat <= NO_SPREAD_FACTOR * Number.EPSILON * (m + kappa * sY);
};

function linearRegressionBundle(v: O, spec: O): LinearRegressionResultView | null {
  const p = spec.parameters;
  const r = v.primaryOutputs;
  if (!isObject(p) || !Array.isArray(p.predictors) || !Array.isArray(p.missingCodes)) return null;
  const predictorsSpec = p.predictors as unknown[];
  const k = predictorsSpec.length;
  if (aliasNumber(p.outcome) === null || k < 1 || k > LR_LIMITS.maxPredictors || predictorsSpec.some((c) => aliasNumber(c) === null)) return null;
  const columnsSpec = [p.outcome, ...predictorsSpec] as string[];
  if (columnsSpec.some((c, i) => columnsSpec.indexOf(c) !== i)) return null;
  if (!isObject(r) || !hasExactly(r, LR_RESULT_KEYS)) return null;
  if (!isCount(v.runId) || v.status !== 'succeeded') return null;

  // Shape: the outcome and the columns as the spec names them, in its order (:930-940, :1004).
  if (r.outcome !== p.outcome) return null;
  if (!isCount(r.rowsUsed) || r.rowsUsed < k + 2) return null;
  const n = r.rowsUsed;
  if (!Array.isArray(r.columns) || r.columns.length !== k + 1 || !Array.isArray(r.predictors) || r.predictors.length !== k) return null;
  for (let i = 0; i <= k; i += 1) {
    const c = r.columns[i];
    if (!isObject(c) || !hasExactly(c, LR_COLUMN_KEYS) || c.column !== columnsSpec[i] || !lrStat(c.mean, []) || !lrStat(c.sd, [])) return null;
  }
  const columns = r.columns as O[];
  const m = r.model;
  const a = r.anova;
  const c0 = r.intercept;
  const res = r.residuals;
  if (!isObject(m) || !hasExactly(m, LR_MODEL_KEYS) || !isObject(a) || !hasExactly(a, LR_ANOVA_KEYS)) return null;
  if (!isObject(c0) || !hasExactly(c0, LR_ESTIMATE_KEYS) || !isObject(res) || !hasExactly(res, LR_RESIDUAL_KEYS)) return null;
  const [reg, rsd, tot] = [a.regression, a.residual, a.total];
  if (!isObject(reg) || !hasExactly(reg, LR_SOURCE_KEYS) || !isObject(rsd) || !hasExactly(rsd, LR_SOURCE_KEYS) || !isObject(tot) || !hasExactly(tot, LR_TOTAL_KEYS)) return null;
  const [pr, re, sp, sr] = [res.predicted, res.residual, res.standardizedPredicted, res.standardizedResidual];
  if (!isObject(pr) || !hasExactly(pr, LR_SPREAD_KEYS) || !isObject(re) || !hasExactly(re, LR_SPREAD_KEYS)) return null;
  if (!isObject(sp) || !hasExactly(sp, LR_EXTREMES_KEYS) || !isObject(sr) || !hasExactly(sr, LR_EXTREMES_KEYS)) return null;
  if (!Array.isArray(r.limitations) || JSON.stringify(r.limitations) !== JSON.stringify(LINEAR_REGRESSION_LIMITATIONS)) return null;
  if (!Array.isArray(r.notes) || !(r.notes.length === 0 || (r.notes.length === 1 && r.notes[0] === LR_NOTE))) return null;

  // Degrees of freedom (:975, :980, :985; df_residual is n - k - 1).
  const df = n - k - 1;
  if (reg.df !== k || rsd.df !== df || tot.df !== n - 1) return null;

  // Each field holds a value or a code Rust can write there.
  if (!lrStat(reg.sumOfSquares, []) || !lrStat(rsd.sumOfSquares, []) || !lrStat(tot.sumOfSquares, [LR_OVERFLOW])) return null;
  if (!lrStat(reg.meanSquare, [LR_SMALL]) || !lrStat(rsd.meanSquare, [LR_SMALL, LR_PERFECT])) return null;
  if (!lrStat(a.f, LR_ESTIMATE_CODES) || !lrStat(a.p, LR_P_CODES)) return null;
  if (!lrStat(m.r, []) || !lrStat(m.rSquared, []) || !lrStat(m.adjustedRSquared, [])) return null;
  if (!lrStat(m.standardErrorOfEstimate, [LR_PERFECT]) || !lrStat(m.durbinWatson, [LR_PERFECT])) return null;
  for (const s of [pr.minimum, pr.maximum, pr.mean, pr.standardDeviation, re.minimum, re.maximum, re.mean, re.standardDeviation]) if (!lrStat(s, [])) return null;
  if (!lrStat(sp.minimum, [LR_NO_SPREAD, LR_OVERFLOW, LR_SMALL]) || !lrStat(sp.maximum, [LR_NO_SPREAD, LR_OVERFLOW, LR_SMALL])) return null;
  if (!lrStat(sr.minimum, [LR_PERFECT, LR_OVERFLOW, LR_SMALL]) || !lrStat(sr.maximum, [LR_PERFECT, LR_OVERFLOW, LR_SMALL])) return null;

  const mss = num(reg.sumOfSquares);
  const rss = num(rsd.sumOfSquares);
  if (mss < 0 || rss < 0) return null;
  // resSS = 0 if and only if the perfect-fit set holds perfect_fit (:918).
  const perfect = rss === 0;
  const sigma2 = rss / df;

  // Sums and squares (:953, :976, :981, :984).
  if (!lrIs(reg.meanSquare, lrOwnOf(mss / k))) return null;
  if (!lrIs(rsd.meanSquare, perfect ? { code: LR_PERFECT } : lrOwnOf(sigma2))) return null;
  if (!lrIs(tot.sumOfSquares, lrStatOf(mss + rss))) return null;

  // The model (:942-951): R², R and adjusted R² in Rust's order, exactly 1 in a perfect fit.
  const r2 = mss / (mss + rss);
  if (!lrIs(m.rSquared, lrStatOf(r2)) || !lrIs(m.r, lrStatOf(Math.sqrt(r2)))) return null;
  if (!lrIs(m.adjustedRSquared, lrStatOf(1 - ((1 - r2) * (n - 1)) / df))) return null;
  // σ: √σ² in the normal range, √RSS / √df where σ² is subnormal (:922, :949).
  const sigma = sigma2 >= MIN_NORMAL ? Math.sqrt(sigma2) : Math.sqrt(rss) / Math.sqrt(df);
  if (!lrIs(m.standardErrorOfEstimate, perfect ? { code: LR_PERFECT } : lrStatOf(sigma))) return null;
  // The Durbin-Watson statistic lies in [0, 4] (decision 698; the largest over 20,009 replica fits was 3.99992).
  if (perfect ? ncCode(m.durbinWatson) !== LR_PERFECT : !(num(m.durbinWatson) >= 0 && num(m.durbinWatson) <= 4)) return null;

  // F and its p (:953-971).
  if (perfect) {
    if (ncCode(a.f) !== LR_PERFECT || ncCode(a.p) !== LR_PERFECT) return null;
  } else {
    const ms = mss / k;
    const f = sigma2 >= MIN_NORMAL && !subnormalOf(ms) ? ms / sigma2 : (mss / rss) * (df / k);
    if (!Number.isFinite(f)) {
      if (ncCode(a.f) !== LR_OVERFLOW || ncCode(a.p) !== LR_OVERFLOW) return null;
    } else if (subnormalOf(f)) {
      if (ncCode(a.f) !== LR_SMALL || ncCode(a.p) !== LR_SMALL) return null;
    } else {
      if (statValue(a.f) !== f) return null;
      if (!(has(a.p) ? num(a.p) >= 0 && num(a.p) <= 1 : ncCode(a.p) === LR_RANGE)) return null;
    }
  }

  // Each coefficient (:991-1049).
  if (!lrEstimate(c0, perfect, df)) return null;
  const sdY = num(columns[0].sd);
  const unexplained = rss / (mss + rss);
  for (let j = 0; j < k; j += 1) {
    const q = r.predictors[j];
    if (!isObject(q) || !hasExactly(q, LR_PREDICTOR_KEYS) || q.column !== predictorsSpec[j] || !lrEstimate(q, perfect, df)) return null;
    if (!lrStat(q.beta, [LR_SMALL, LR_OVERFLOW]) || !lrStat(q.zeroOrder, [])) return null;
    if (!lrStat(q.partial, LR_ESTIMATE_CODES) || !lrStat(q.part, LR_ESTIMATE_CODES)) return null;
    if (!lrStat(q.tolerance, [LR_SMALL, LR_OVERFLOW]) || !lrStat(q.vif, [LR_SMALL, LR_OVERFLOW])) return null;
    // Beta (:1038), from the stored SDs, which are the SDs Rust used (:932-938).
    if (!lrIs(q.beta, lrOwnOf((num(q.b) * num(columns[j + 1].sd)) / sdY))) return null;
    // Zero-order, clamped (:1009).
    if (!(num(q.zeroOrder) >= -1 && num(q.zeroOrder) <= 1)) return null;
    // Partial and part from t, or t's code, or perfect_fit (:1010-1014, partial_of :826-833).
    if (perfect) {
      if (ncCode(q.partial) !== LR_PERFECT || ncCode(q.part) !== LR_PERFECT) return null;
    } else if (has(q.t)) {
      const t = num(q.t);
      const t2 = t * t;
      const partial = Number.isFinite(t2) ? t / Math.sqrt(t2 + df) : Math.sign(t) / Math.sqrt(1 + df / t2);
      if (!lrIs(q.partial, lrStatOf(partial)) || !lrIs(q.part, lrStatOf(t * Math.sqrt(unexplained / df)))) return null;
    } else if (ncCode(q.partial) !== ncCode(q.t) || ncCode(q.part) !== ncCode(q.t)) return null;
    // Tolerance and VIF (:1015-1033): exactly 1 with one predictor; otherwise 1 / VIF.
    if (k === 1) {
      if (statValue(q.tolerance) !== 1 || statValue(q.vif) !== 1) return null;
    } else if (ncCode(q.vif) === LR_SMALL) {
      // A subnormal s_j (:1024-1026), which gives the SE too_small, or perfect_fit in a perfect
      // fit (:793-797); never beside a numeric SE (decision 698).
      if (ncCode(q.tolerance) !== LR_SMALL) return null;
      if (ncCode(q.standardError) !== LR_SMALL && ncCode(q.standardError) !== LR_PERFECT) return null;
    } else {
      const vif = has(q.vif) ? num(q.vif) : Infinity;
      if (!lrIs(q.tolerance, lrStatOf(1 / vif))) return null;
      // A VIF is at least 1 and a tolerance at most 1, but for rounding: Rust's reaches 1 -
      // 6,373 ε on ten rows with a year-like predictor uncorrelated with the others, so the
      // bound is 1e-6, not a multiple of ε (decision 698).
      if (!(vif >= 0.999999) || (has(q.tolerance) && !(num(q.tolerance) <= 1.000001))) return null;
    }
  }

  // The residuals (:1094-1151): each spread's minimum, mean and maximum in order; the
  // standardized predicted values by Rust's expression or predicted_no_spread exactly where
  // its rule gives it; the standardized residuals over σ, or perfect_fit.
  for (const s of [pr, re]) {
    if (!(num(s.minimum) <= num(s.mean) && num(s.mean) <= num(s.maximum)) || !(num(s.standardDeviation) >= 0)) return null;
  }
  if (lrNoSpread(r)) {
    if (ncCode(sp.minimum) !== LR_NO_SPREAD || ncCode(sp.maximum) !== LR_NO_SPREAD) return null;
  } else {
    const [mean, sd] = [num(pr.mean), num(pr.standardDeviation)];
    if (!lrIs(sp.minimum, lrStatOf((num(pr.minimum) - mean) / sd)) || !lrIs(sp.maximum, lrStatOf((num(pr.maximum) - mean) / sd))) return null;
  }
  if (perfect) {
    if (ncCode(sr.minimum) !== LR_PERFECT || ncCode(sr.maximum) !== LR_PERFECT) return null;
  } else {
    const s = num(m.standardErrorOfEstimate);
    if (!lrIs(sr.minimum, lrStatOf(num(re.minimum) / s)) || !lrIs(sr.maximum, lrStatOf(num(re.maximum) / s))) return null;
  }

  // The note (:1053-1060): σ² below (mean ŷ² + var ŷ) × 1e-30. The fitted values' variance
  // is held only as its root, so the rule is exact away from its threshold T, and either
  // answer is taken where |σ² - T| ≤ max(4 ε T, 4 × 5e-324) (decisions 688 and 698): below the
  // normal range one subnormal step is wider than 4 ε T, and Rust's threshold may round to 0
  // where this one is 5e-324 (the review of 9e4b7ad9, finding 1).
  const fm = num(pr.mean);
  const fs = num(pr.standardDeviation);
  const threshold = (fm * fm + fs * fs) * ESSENTIALLY_PERFECT_FACTOR;
  const noted = r.notes.length === 1;
  if (!(Math.abs(sigma2 - threshold) <= Math.max(4 * Number.EPSILON * threshold, 4 * Number.MIN_VALUE)) && noted !== (Number.isFinite(sigma2) && sigma2 < threshold)) return null;

  // Exclusions and the manifest, as the related rank tests' (decision 652).
  if (!arrayOf(r.exclusions, lrExclusion) || !lrExclusionsAgree(r.exclusions as O[], columnsSpec, p.missingCodes)) return null;
  if (!arrayOf(v.diagnostics, lrDiagnostic) || !arrayOf(v.limitations, lrDiagnostic)) return null;
  if (!manifest(v.manifest, LINEAR_REGRESSION_CAPABILITY, lrExclusion)) return null;
  const mf = v.manifest as O;
  if (mf.rowsUsed !== n) return null;
  if (mf.rowsRead !== n + (r.exclusions as Array<{ count: number }>).reduce((s, e) => s + e.count, 0)) return null;
  if (JSON.stringify(mf.exclusions) !== JSON.stringify(r.exclusions)) return null;
  // As Rust's stats_linear_regression_commands::result makes the live view.
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as LinearRegressionResultView['result'],
    diagnostics: v.diagnostics as LinearRegressionDiagnostic[],
    manifest: v.manifest as LinearRegressionResultView['manifest'],
  };
}

/** A failed run's diagnostic (decision 688): one of the run's own two refusals, which only
 *  compute reaches (:700-715), the collinear one naming one of the spec's predictors. */
function linearRegressionFailure(failure: unknown, spec: O): boolean {
  if (!isObject(failure) || failure.severity !== 'error' || !isObject(failure.params)) return false;
  const p = spec.parameters;
  if (failure.code === lrCode('fit_out_of_range')) return Object.keys(failure.params).length === 0;
  if (failure.code !== lrCode('predictors_collinear') || !isObject(p) || !Array.isArray(p.predictors)) return false;
  const column = failure.params.column;
  return Object.keys(failure.params).length === 1 && isCount(column) && p.predictors.some((a) => aliasNumber(a) === column);
}

/* ---- the factor analysis (Research OS, Phase 9B-3, decision 733) ----------------------- */

/* Every rule below is stats_factor::run's (stats_factor/mod.rs, as at 9B-3's Part 0), cited by
 * line, and equality is as doubles in Rust's order. Rust writes every number through `stat`
 * (:1028-1036: a finite value, NaN too_small, an infinity overflow), so a field Rust computes
 * from stored fields is compared with what `stat` would have written. */

const faCode = (c: string) => `stats.factor.${c}`;
const FA_SINGULAR = faCode('correlations_singular');
const FA_SMALL = faCode('too_small');
const FA_OVERFLOW = faCode('overflow');
const FA_NO_CORRELATION = faCode('no_correlation');
/** stats_factor::DISTINCT_TOLERANCE (:111). */
const FA_DISTINCT = 1e-8;
/** stats_factor's MAX_ITERATIONS, MIN_CONVERGENCE and MAX_CONVERGENCE (:91-95), the spec's
 *  ranges check (:616-625) refuses outside (decision 742's m1). */
const FA_MAX_ITERATIONS = 1000;
const FA_MIN_CONVERGENCE = 1e-12;
const FA_MAX_CONVERGENCE = 0.1;
/** ln of the smallest normal double: Rust writes the determinant too_small only when Π l_jj²
 *  is below it (:823-825), so only beside a log below this, within the determinant's band. */
const FA_LN_MIN_NORMAL = Math.log(2.2250738585072014e-308);
/** ln of stats_linalg's CHOLESKY_TOLERANCE, 1e-9: each pivot's floor relative to its diagonal. */
const FA_LN_CHOLESKY_TOLERANCE = Math.log(1e-9);

/** The bands of decision 733, each the worst measured over the stored checker's hunt (seed
 *  20260734, 256 saved runs with 16 of 128 items at m of 32 to 127, in two runs: 182 and 176
 *  results, the second after the probe's duplicated item was made an exact copy) and the four
 *  stored fixtures, times ten and rounded up, measured in full with the bands lifted (13
 *  Nov 2026, ~/gaply-corpus/os/phase9/9b3/hunt_all/bands_full.tsv). The two rotation bands grow
 *  with the rotation's work: the first measurement, at m of at most 16 and a fixed band, refused
 *  three varimax results Rust wrote at m of 32, 64 and 100 (the review of 9B-3, finding 7). */
export const FA_BANDS = {
  /** |Σ eigenvalues − p| / p: worst 8.745e-16 (3.94 ε) over 9B-3's hunt, then 2.22e-15 over
   *  9C-1's slow runs (128 items, m 3, 246 cycles), a 4× margin, so ten times that (Research OS,
   *  Phase 9C-2, decision 761; decision 754's third answer). */
  eigenSum: 2.3e-14,
  /** |Φ_jj − 1|: worst 4.441e-16, at m = 64. */
  phiDiagonal: 4.5e-15,
  /** |(TᵀT)_jk − δ_jk| over ε m cycles: worst 0.3334 (m 3, 3 cycles); 0.12 at m of 32 to
   *  100 with 50 to 76 cycles. */
  orthogonal: 3.4,
  /** |rotated row − extracted row × T| over ε m cycles, as the orthogonality band (Research OS,
   *  Phase 9C-1, decision 742's B1: over ε m alone it refused a varimax of 995 cycles Rust wrote,
   *  at 10.17). Worst 0.25 (m 2, 2 cycles) over 9B-3's hunt, 40 runs of 202 to 995 cycles and
   *  the five stored fixtures (~/gaply-corpus/os/phase9/9c1/hunt_fa/bands_full.tsv), and 0.2
   *  over 39,996 one-factor varimax results of 128 items (9c1/hunt/slow_varimax_1_40000.tsv). */
  rotatedRow: 2.5,
  /** How far below 0 a principal axis initial communality, 1 − 1/(R⁻¹)_ii, may round: none was
   *  below 0 (worst 0), so ten times it would be 0; the band is 1e-15, about 4.5 ε, since
   *  (R⁻¹)_ii ≥ 1 holds for the exact inverse only and the arithmetic's own can round below. */
  initialBelowZero: 1e-15,
  /** |Π l_jj² − exp(log det)| / Π l_jj², the determinant's value against its log (:823-826;
   *  Research OS, Phase 9C-1, decision 742's m1), and in the log's units how far above ln of
   *  the smallest normal double a log beside too_small may be: worst 7.804e-13 over 96 values
   *  Rust wrote at logs from −708.29 down (9c1/hunt/det_boundary.tsv), 1.447e-13 over the hunt;
   *  ten times the worst, rounded up. The 304 too_small beside them had logs at least 0.189
   *  below ln of the smallest normal double. */
  determinant: 8e-12,
}

const faStat = (v: unknown, codes: readonly string[]) => oneKey(v, { value: isNum, not_computable: (x) => oneOf(x, codes) });
/** Rust's `stat` (:1028-1036). */
const faStatOf = (x: number): Expected => (Number.isFinite(x) ? { value: x } : Number.isNaN(x) ? { code: FA_SMALL } : { code: FA_OVERFLOW });
const faIs = (stored: unknown, e: Expected) => ('value' in e ? statValue(stored) === e.value : ncCode(stored) === e.code);
const faVal = (v: unknown) => statValue(v) as number;
const faValues = (v: unknown) => faStat(v, []);

/** FactorOutcome's keys and each object's, every one always written (:291-403). */
const FA_RESULT_KEYS = ['items', 'rowsUsed', 'descriptives', 'determinant', 'kmo', 'bartlett', 'eigenvalues', 'extraction', 'rotation', 'exclusions', 'notes', 'limitations'];
const FA_EXTRACTION_KEYS = ['method', 'factors', 'iterations', 'communalities', 'sums', 'loadings'];
const FA_ROTATION_KEYS = ['method', 'iterations', 'loadings', 'transformation', 'sums', 'structure', 'factorCorrelations'];
const FA_SUMS_KEYS = ['total', 'percent', 'cumulative'];

/** Every result's limitations, in stats_factor's order (LIMITATIONS, :413-423). */
const FACTOR_LIMITATIONS = [
  'no_ml_or_other_extraction',
  'no_oblique_other_than_promax',
  'promax_kappa_four',
  'no_factor_scores',
  'no_matrices_shown',
  'pearson_only',
  'listwise_only',
  'no_scree_plot',
  'no_weights',
].map(faCode);
/** The notes in decision 717's order (:977-987). */
const FACTOR_NOTES = [
  'communality_above_one',
  'extraction_eigenvalue_negative',
  'one_factor_not_rotated',
  'rotation_not_converged',
  'rotation_singular',
  'eigenvalues_not_distinct',
].map(faCode);

const faDiagnostic = (v: unknown) =>
  diagnostic(FA_CODES, FA_PARAM_KEYS, false)(v) &&
  isObject(v) &&
  (v.code !== faCode('suspect_missing_code') || (isObject(v.params) && ['column', 'value', 'count'].every((k) => isNum((v.params as O)[k]))));

/** An exclusion, exactly as stats_factor writes it (:962-975). */
const FA_EXCLUDED = [faCode('missing_value'), faCode('missing_code')];
const faExclusion = (e: unknown) =>
  isObject(e) &&
  Object.keys(e).length === 3 &&
  oneOf(e.code, FA_EXCLUDED) &&
  isCount(e.count) &&
  e.count >= 1 &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

/** The exclusions in the items' order, missing_value before missing_code for one item, each
 *  pair once, each column an item, and missing_code only with missing codes (:962-975). */
const faExclusionsAgree = (list: O[], items: string[], codes: unknown[]) => {
  const numbers = items.map(aliasNumber);
  const order = list.map((e) => numbers.indexOf(((e.params as O).column as number) ?? -1) * 2 + (e.code === FA_EXCLUDED[1] ? 1 : 0));
  if (order.some((o) => o < 0)) return false;
  if (order.some((o, i) => i > 0 && o <= order[i - 1])) return false;
  return codes.length > 0 || list.every((e) => e.code !== FA_EXCLUDED[1]);
};

/** A p × m matrix of values. */
const faMatrix = (v: unknown, rows: number, cols: number): v is unknown[][] =>
  Array.isArray(v) && v.length === rows && v.every((row) => Array.isArray(row) && row.length === cols && row.every(faValues));
const faRows = (v: unknown[][]) => v.map((row) => row.map(faVal));

/** Sums, each a value, with their percents of p and the running percent (sums_of, :1046-1057). */
const faSums = (v: unknown, totals: number[] | null, count: number, p: number): boolean => {
  if (!Array.isArray(v) || v.length !== count) return false;
  let cumulative = 0;
  for (let k = 0; k < count; k += 1) {
    const s = v[k];
    if (!isObject(s) || !hasExactly(s, FA_SUMS_KEYS) || !faValues(s.total) || !faValues(s.percent) || !faValues(s.cumulative)) return false;
    const t = faVal(s.total);
    if (totals !== null && t !== totals[k]) return false;
    const percent = (t / p) * 100;
    cumulative += percent;
    if (!faIs(s.percent, faStatOf(percent)) || !faIs(s.cumulative, faStatOf(cumulative))) return false;
  }
  return true;
};

/** Column sums of squares, in Rust's order (:1076-1079). */
const columnSquares = (m: number[][], k: number) => Array.from({ length: k }, (_, j) => m.reduce((s, row) => s + row[j] * row[j], 0));
/** Row sums of squares (:1072-1074). */
const rowSquares = (m: number[][]) => m.map((row) => row.reduce((s, x) => s + x * x, 0));
/** a × b, in Rust's order (multiply, :1252-1256). */
const product = (a: number[][], b: number[][]) => a.map((row) => b[0].map((_, j) => row.reduce((s, x, k) => s + x * b[k][j], 0)));

/** A banded quantity, by its band's name. */
export type FactorBand = keyof typeof FA_BANDS;
/** Whoever measures the bands (decision 733's hunt): none in the app, so the checker writes
 *  nothing outside itself; a probe sets one with `measureFactorBands` and clears it after. */
let bandWatcher: ((band: FactorBand, value: number) => void) | null = null;
export function measureFactorBands(watch: ((band: FactorBand, value: number) => void) | null): void {
  bandWatcher = watch;
}
const measure = (k: FactorBand, v: number) => {
  if (bandWatcher) bandWatcher(k, v);
  return v;
};

function factorBundle(v: O, spec: O): FactorResultView | null {
  const sp = spec.parameters;
  const r = v.primaryOutputs;
  if (!isObject(sp) || !Array.isArray(sp.items) || !Array.isArray(sp.missingCodes)) return null;
  const items = sp.items as unknown[];
  const p = items.length;
  if (p < 3 || p > FA_LIMITS.maxItems || items.some((c) => aliasNumber(c) === null)) return null;
  if (!oneOf(sp.extraction, ['principal_components', 'principal_axis']) || !oneOf(sp.rotation, ['none', 'varimax', 'promax'])) return null;
  if (!isCount(sp.iterations) || sp.iterations < 1 || !(sp.factors === null || isCount(sp.factors))) return null;
  // The spec's ranges as check refuses outside them (:536-545, :616-625; decision 742's m1).
  if (sp.iterations > FA_MAX_ITERATIONS || !isNum(sp.convergence) || sp.convergence < FA_MIN_CONVERGENCE || sp.convergence > FA_MAX_CONVERGENCE) return null;
  const codes = sp.missingCodes as unknown[];
  if (!codes.every(isNum) || codes.some((c, i) => i > 0 && !((c as number) > (codes[i - 1] as number)))) return null;
  if (!isObject(r) || !hasExactly(r, FA_RESULT_KEYS)) return null;
  if (!isCount(v.runId) || v.status !== 'succeeded') return null;
  const pc = sp.extraction === 'principal_components';
  const limit = sp.iterations as number;

  // Shape: the items as the spec names them, in its order (:1000).
  if (JSON.stringify(r.items) !== JSON.stringify(items)) return null;
  if (!isCount(r.rowsUsed) || r.rowsUsed < 3) return null;
  const n = r.rowsUsed;
  if (!Array.isArray(r.descriptives) || r.descriptives.length !== p) return null;
  for (let i = 0; i < p; i += 1) {
    const d = r.descriptives[i];
    // mean `stat` (:775); sd `stat` or a range code (:767-771).
    if (!isObject(d) || !hasExactly(d, ['item', 'mean', 'sd']) || d.item !== items[i]) return null;
    if (!faStat(d.mean, [FA_SMALL, FA_OVERFLOW]) || !faStat(d.sd, [FA_SMALL, FA_OVERFLOW])) return null;
    // A square root (:771): never below 0 (decision 742's m1).
    if (statValue(d.sd) !== undefined && faVal(d.sd) < 0) return null;
  }
  const det = r.determinant;
  const kmo = r.kmo;
  const b = r.bartlett;
  if (!isObject(det) || !hasExactly(det, ['value', 'log']) || !isObject(kmo) || !hasExactly(kmo, ['overall', 'items']) || !isObject(b) || !hasExactly(b, ['chiSquare', 'df', 'p'])) return null;
  if (!Array.isArray(kmo.items) || kmo.items.length !== p) return null;
  // The codes each field can hold (:818-860): the determinant too_small below the smallest
  // normal double; the inverse's statistics correlations_singular together; KMO and the MSAs
  // no_correlation at 0/0 (adequacy, :1107-1113) and never `stat`'s codes, since r² / (r² +
  // q²) of finite squares with a nonzero sum is a value in [0, 1]; Bartlett's p overflow,
  // never distribution_out_of_range, since df is at most 8,128 and the chi-square finite
  // (decision 742's m1).
  if (!faStat(det.value, [FA_SMALL, FA_SINGULAR]) || !faStat(det.log, [FA_SINGULAR])) return null;
  const msaCodes = [FA_SINGULAR, FA_NO_CORRELATION];
  if (!faStat(kmo.overall, msaCodes)) return null;
  for (let i = 0; i < p; i += 1) {
    const a = kmo.items[i];
    if (!isObject(a) || !hasExactly(a, ['item', 'msa']) || a.item !== items[i] || !faStat(a.msa, msaCodes)) return null;
  }
  if (!faStat(b.chiSquare, [FA_SINGULAR]) || !faStat(b.p, [FA_SINGULAR, FA_OVERFLOW])) return null;
  // KMO no_correlation exactly when every MSA is (decision 742's m1): the overall sums are the
  // items' sums, each of squares, so the overall 0/0 is every item's.
  const msas = (kmo.items as O[]).map((a) => ncCode(a.msa) === FA_NO_CORRELATION);
  if ((ncCode(kmo.overall) === FA_NO_CORRELATION) !== msas.every((x) => x)) return null;
  // The singular set, all or none (:823-861): the Cholesky factor refuses, so no log, no
  // inverse and no chi-square.
  const singular = ncCode(det.log) === FA_SINGULAR;
  const inverseFields = [det.value, kmo.overall, b.chiSquare, b.p, ...(kmo.items as O[]).map((a) => a.msa)];
  if (singular ? !inverseFields.every((s) => ncCode(s) === FA_SINGULAR) : inverseFields.some((s) => ncCode(s) === FA_SINGULAR)) return null;
  // Bartlett's df (:840) and chi-square from the stored log and rows (:843-849).
  if (b.df !== (p * (p - 1)) / 2) return null;
  if (!singular) {
    // The determinant's value, Π l_jj², against exp of its log, Σ ln l_jj² (:823-826), within
    // a band; too_small only beside a log below ln of the smallest normal double (decision
    // 742's m1).
    const logDet = faVal(det.log);
    // Every pivot left is above 1e-9 times a diagonal of 1 (stats_linalg::cholesky, decision
    // 699), so Σ ln l_jj² is above p ln(1e-9); the margin is rounding's (the review of 9C-1,
    // finding 7).
    if (!(logDet > p * FA_LN_CHOLESKY_TOLERANCE - 1e-9 * p)) return null;
    if (ncCode(det.value) === FA_SMALL) {
      if (!(measure('determinant', Math.max(logDet - FA_LN_MIN_NORMAL, 0)) <= FA_BANDS.determinant)) return null;
    } else {
      const value = faVal(det.value);
      if (!(value >= 2.2250738585072014e-308)) return null;
      if (!(measure('determinant', Math.abs(value - Math.exp(logDet)) / value) <= FA_BANDS.determinant)) return null;
    }
    const log = Math.min(faVal(det.log), 0);
    const multiplier = n - 1 - (2 * p + 5) / 6;
    const chi = log === 0 ? 0 : -(multiplier * log);
    if (!faIs(b.chiSquare, faStatOf(chi))) return null;
    if (ncCode(b.p) === FA_OVERFLOW) return null; // chi is finite here, so never overflow (:850).
    if (statValue(b.p) !== undefined && !(faVal(b.p) >= 0 && faVal(b.p) <= 1)) return null;
    // KMO and each MSA lie in [0, 1]: r² / (r² + q²) of sums of squares.
    for (const s of [kmo.overall, ...(kmo.items as O[]).map((a) => a.msa)]) if (statValue(s) !== undefined && !(faVal(s) >= 0 && faVal(s) <= 1)) return null;
  }

  // Every eigenvalue (:864-865), descending exactly (eigen_symmetric's sort), summing to p
  // within a band.
  if (!faSums(r.eigenvalues, null, p, p)) return null;
  const values = (r.eigenvalues as O[]).map((e) => faVal(e.total));
  if (values.some((x, i) => i > 0 && x > values[i - 1])) return null;
  if (measure('eigenSum', Math.abs(values.reduce((s, x) => s + x, 0) - p) / p) > FA_BANDS.eigenSum) return null;

  // The extraction (:867-946).
  const e = r.extraction;
  if (!isObject(e) || !hasExactly(e, FA_EXTRACTION_KEYS) || e.method !== sp.extraction || !isCount(e.factors) || !isCount(e.iterations)) return null;
  const m = e.factors;
  const kaiser = values.filter((x) => x > 1).length;
  if (sp.factors === null ? m !== kaiser || m === 0 : m !== sp.factors) return null;
  if (m < 1 || m > (pc ? p : p - 1)) return null;
  if (!faMatrix(e.loadings, p, m)) return null;
  const loadings = faRows(e.loadings);
  if (!faSums(e.sums, columnSquares(loadings, m), m, p)) return null;
  if (!Array.isArray(e.communalities) || e.communalities.length !== p) return null;
  const extracted = rowSquares(loadings);
  for (let i = 0; i < p; i += 1) {
    const c = e.communalities[i];
    if (!isObject(c) || !hasExactly(c, ['item', 'initial', 'extraction']) || c.item !== items[i] || !faValues(c.initial) || !faValues(c.extraction)) return null;
    if (faVal(c.extraction) !== extracted[i]) return null;
    if (pc ? faVal(c.initial) !== 1 : !(measure('initialBelowZero', -faVal(c.initial)) <= FA_BANDS.initialBelowZero && faVal(c.initial) <= 1)) return null;
  }
  // Principal components does not iterate; principal axis takes 1 to the spec's (:893-929).
  if (pc ? e.iterations !== 0 : e.iterations < 1 || e.iterations > limit) return null;
  if (pc && values.slice(0, m).some((x) => !(x > 0))) return null; // component_loadings' refusal.

  // The notes (:977-987): a subset of the six in order.
  const notes = r.notes;
  if (!Array.isArray(notes) || notes.some((x) => !oneOf(x, FACTOR_NOTES))) return null;
  const at = notes.map((x) => FACTOR_NOTES.indexOf(x as string));
  if (at.some((x, i) => i > 0 && x <= at[i - 1])) return null;
  const noted = (c: string) => notes.indexOf(faCode(c)) >= 0;
  if (pc && (noted('communality_above_one') || noted('extraction_eigenvalue_negative'))) return null;
  // A final communality above 1 was the last iteration's, which notes it (:917-920).
  if (!pc && extracted.some((h) => h > 1) && !noted('communality_above_one')) return null;
  if (noted('one_factor_not_rotated') !== (m === 1 && sp.rotation !== 'none')) return null;
  if ((noted('rotation_not_converged') || noted('rotation_singular')) && (m === 1 || sp.rotation === 'none')) return null;
  if (noted('rotation_singular') && sp.rotation !== 'promax') return null;
  if (noted('rotation_not_converged') && noted('rotation_singular')) return null;
  // eigenvalues_not_distinct (:930-935): both ways for principal components, from the stored
  // eigenvalues; for principal axis it reads the last reduced matrix, not stored.
  if (pc) {
    const distinct = m < p && Math.abs(values[m - 1] - values[m]) <= FA_DISTINCT * Math.max(Math.abs(values[m - 1]), Math.abs(values[m]));
    if (noted('eigenvalues_not_distinct') !== distinct) return null;
  }

  // The rotation (:950-959): null exactly when none is asked, m is 1, or a note says why.
  const rot = r.rotation;
  const none = sp.rotation === 'none' || m === 1 || noted('rotation_not_converged') || noted('rotation_singular');
  if (none !== (rot === null)) return null;
  if (isObject(rot)) {
    if (!hasExactly(rot, FA_ROTATION_KEYS) || rot.method !== sp.rotation || !isCount(rot.iterations) || rot.iterations < 1 || rot.iterations > limit) return null;
    if (!faMatrix(rot.loadings, p, m)) return null;
    const rotated = faRows(rot.loadings);
    if (rot.method === 'varimax') {
      if (rot.structure !== null || rot.factorCorrelations !== null || !faMatrix(rot.transformation, m, m)) return null;
      if (!faSums(rot.sums, columnSquares(rotated, m), m, p)) return null;
      const t = faRows(rot.transformation);
      // T orthogonal, and each rotated row the extracted row times T, within bands.
      const tt = product(t[0].map((_, j) => t.map((row) => row[j])), t);
      for (let j = 0; j < m; j += 1) for (let k = 0; k < m; k += 1) if (measure('orthogonal', Math.abs(tt[j][k] - (j === k ? 1 : 0)) / (Number.EPSILON * m * (rot.iterations as number))) > FA_BANDS.orthogonal) return null;
      const again = product(loadings, t);
      for (let i = 0; i < p; i += 1) for (let k = 0; k < m; k += 1) if (measure('rotatedRow', Math.abs(again[i][k] - rotated[i][k]) / (Number.EPSILON * m * (rot.iterations as number))) > FA_BANDS.rotatedRow) return null;
    } else {
      if (rot.transformation !== null || rot.sums !== null || !faMatrix(rot.structure, p, m) || !faMatrix(rot.factorCorrelations, m, m)) return null;
      const phi = faRows(rot.factorCorrelations);
      // Φ exactly symmetric (decision 728), its diagonal within a band of 1, and the structure
      // exactly the pattern times Φ (:1350-1352).
      for (let j = 0; j < m; j += 1) {
        if (measure('phiDiagonal', Math.abs(phi[j][j] - 1)) > FA_BANDS.phiDiagonal) return null;
        for (let k = 0; k < j; k += 1) if (phi[j][k] !== phi[k][j]) return null;
      }
      const structure = product(rotated, phi);
      const stored = faRows(rot.structure);
      for (let i = 0; i < p; i += 1) for (let k = 0; k < m; k += 1) if (stored[i][k] !== structure[i][k]) return null;
    }
  }

  if (!Array.isArray(r.limitations) || JSON.stringify(r.limitations) !== JSON.stringify(FACTOR_LIMITATIONS)) return null;

  // Exclusions and the manifest, as the regression's.
  if (!arrayOf(r.exclusions, faExclusion) || !faExclusionsAgree(r.exclusions as O[], items as string[], sp.missingCodes)) return null;
  if (!arrayOf(v.diagnostics, faDiagnostic) || !arrayOf(v.limitations, faDiagnostic)) return null;
  // A succeeded run's diagnostics are validate's warnings, each a suspect code of an item
  // (:675-691; runs.rs's bundle); its limitations the module's nine, as notes, in order
  // (decision 742's m1).
  const itemNumbers = items.map(aliasNumber);
  for (const d of v.diagnostics as O[]) {
    if (d.severity !== 'warning' || d.code !== faCode('suspect_missing_code') || itemNumbers.indexOf((d.params as O).column as number) < 0) return null;
  }
  const lims = v.limitations as O[];
  if (lims.length !== FACTOR_LIMITATIONS.length || lims.some((d, i) => d.code !== FACTOR_LIMITATIONS[i] || d.severity !== 'note' || Object.keys(d.params as O).length !== 0)) return null;
  if (!manifest(v.manifest, FACTOR_CAPABILITY, faExclusion)) return null;
  const mf = v.manifest as O;
  if (mf.rowsUsed !== n) return null;
  if (mf.rowsRead !== n + (r.exclusions as Array<{ count: number }>).reduce((s, x) => s + x.count, 0)) return null;
  if (JSON.stringify(mf.exclusions) !== JSON.stringify(r.exclusions)) return null;
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as FactorResultView['result'],
    diagnostics: v.diagnostics as FactorDiagnostic[],
    manifest: v.manifest as FactorResultView['manifest'],
  };
}

/** A failed run's diagnostic (decision 733): one of the run's own six refusals, which only
 *  compute reaches (:741-1002); the two that name an item name one of the spec's, principal
 *  axis's singular one only under principal axis, and extraction_not_converged's limit the
 *  spec's iterations. */
function factorFailure(failure: unknown, spec: O): boolean {
  if (!isObject(failure) || failure.severity !== 'error' || !isObject(failure.params)) return false;
  const p = spec.parameters;
  if (!isObject(p) || !Array.isArray(p.items)) return false;
  const keys = Object.keys(failure.params);
  const column = failure.params.column;
  const names = () => keys.length === 1 && isCount(column) && (p.items as unknown[]).some((a) => aliasNumber(a) === column);
  switch (failure.code) {
    case faCode('correlation_out_of_range'):
      return names();
    case faCode('correlations_singular'):
      return p.extraction === 'principal_axis' && names();
    case faCode('no_eigenvalue_over_one'):
      return keys.length === 0 && p.factors === null;
    case faCode('extraction_eigenvalue_not_positive'):
      return keys.length === 0 && p.extraction === 'principal_components';
    case faCode('extraction_not_converged'):
      return keys.length === 1 && p.extraction === 'principal_axis' && failure.params.limit === p.iterations;
    case faCode('eigen_not_converged'):
      return keys.length === 0;
    default:
      return false;
  }
}


/* ---- the confirmatory factor analysis (Research OS, Phase 9C-3, decision 786) ----------------
 * Accepts what stats_cfa writes and nothing else. Each rule names the line of
 * gaply-core/src/stats_cfa/mod.rs it mirrors (as at c0bb7c03); a rule of the measures of fit,
 * moved unchanged to gaply-core/src/stats_fit/mod.rs in 9C-4 (decision 796), names its line
 * there as fit:N (as at 9C-4). Equality is as doubles in
 * Rust's order: every field Rust computes from stored fields with + − × ÷ and √ is recomputed
 * here with the same operations in the same order, which IEEE 754 makes bit for bit the same.
 * BIC and CAIC take ln N, which JavaScript's Math.log and Rust's ln need not round alike, so
 * they are held to a band (CFA_BANDS). Rust writes every number through `stat` (:1274-1282,
 * and fit:141-149 for the fit: a finite value, NaN too_small, an infinity overflow). */

const cfCode = (c: string) => `stats.cfa.${c}`;
const CF_SMALL = cfCode('too_small');
const CF_OVERFLOW = cfCode('overflow');
const CF_RANGE = cfCode('distribution_out_of_range');
const CF_DF_ZERO = cfCode('df_zero');
const CF_FIXED = cfCode('fixed_parameter');
const CF_NEGATIVE = cfCode('negative_variance');
const CF_CMIN_ZERO = cfCode('cmin_zero');
const CF_BASELINE_ZERO = cfCode('independence_cmin_zero');
const CF_BCC = cfCode('too_few_rows_for_bcc');
/** stats_cfa::MAX_ITERATIONS (:97): the run's cap, and cfa_not_converged's limit. */
const CF_MAX_ITERATIONS = 500;

/** The bands of decision 786, each ten times the worst measured over the stored checker's hunt
 *  and the stored fixtures, rounded up: 236 stored results (seed 20260786 and 9C-2's B3
 *  datasets of seed 20260764; 14 Nov 2026, ~/gaply-corpus/os/phase9/9c3/hunt/), measured with
 *  the band lifted. Every other quantity decision 786 names is recomputed exactly. */
export const CFA_BANDS = {
  /** |stored − recomputed| / |recomputed| for BIC = C + q ln N and CAIC = C + q (ln N + 1)
   *  (fit:360-361), where Math.log and Rust's ln may round apart: worst 2.751e-16 (about
   *  1.24 ε), so 2.8e-15. */
  lnN: 2.8e-15,
};
export type CfaBand = keyof typeof CFA_BANDS;
let cfaBandWatcher: ((band: CfaBand, value: number) => void) | null = null;
/** Whoever measures the bands (decision 786's hunt): none in the app. */
export function measureCfaBands(watch: ((band: CfaBand, value: number) => void) | null): void {
  cfaBandWatcher = watch;
}
const cfMeasure = (k: CfaBand, v: number) => {
  if (cfaBandWatcher) cfaBandWatcher(k, v);
  return v;
};

/** Rust's `stat` (:1274-1282, and fit:141-149). */
const cfStatOf = (x: number): Expected => (Number.isFinite(x) ? { value: x } : Number.isNaN(x) ? { code: CF_SMALL } : { code: CF_OVERFLOW });
/** A stored Stat is what Rust would write: exactly {"value": x} or {"not_computable": code}, as
 *  serde writes Stat (the review of 9C-3, finding 5: a value beside another key was accepted). */
const cfIs = (stored: unknown, e: Expected) =>
  isObject(stored) && ('value' in e ? statValue(stored) === e.value && hasExactly(stored, ['value']) : ncCode(stored) === e.code && hasExactly(stored, ['not_computable']));
const cfStat = (v: unknown, codes: readonly string[]) => oneKey(v, { value: isNum, not_computable: (x) => oneOf(x, codes) });
const cfVal = (v: unknown) => statValue(v) as number;
const isValue = (v: unknown) => cfStat(v, []);
/** `against_baseline` (fit:275-281): `stat`, but independence_cmin_zero for a value that is not
 *  finite where C_b is 0. */
const againstOf = (x: number, cb: number): Expected => (cb === 0 && !Number.isFinite(x) ? { code: CF_BASELINE_ZERO } : cfStatOf(x));
/** `unless_df_zero` (fit:157-163). */
const unlessDfZero = (d: number, e: () => Expected): Expected => (d === 0 ? { code: CF_DF_ZERO } : e());

/** CfaOutcome's keys and each object's, every one always written (:290-418). */
const CF_RESULT_KEYS = ['items', 'rowsUsed', 'indicators', 'loadings', 'factorVariances', 'factorCovariances', 'errorVariances', 'smc', 'fit', 'iterations', 'exclusions', 'notes', 'limitations'];
const CF_LOADING_KEYS = ['factor', 'item', 'estimate', 'se', 'cr', 'p', 'standardized'];
const CF_VARIANCE_KEYS = ['factor', 'estimate', 'se', 'cr', 'p'];
const CF_COVARIANCE_KEYS = ['factors', 'estimate', 'se', 'cr', 'p', 'correlation'];
const CF_ERROR_KEYS = ['item', 'estimate', 'se', 'cr', 'p'];
const CF_SMC_KEYS = ['item', 'value'];
const CF_FIT_KEYS = ['model', 'saturated', 'independence'];
/** CfaModelFit's 38 measures (:359-399). */
const CF_MEASURES = [
  'npar', 'cmin', 'df', 'p', 'cminDf', 'rmr', 'gfi', 'agfi', 'pgfi', 'nfi', 'rfi', 'ifi', 'tli', 'cfi', 'pratio', 'pnfi', 'pcfi',
  'ncp', 'ncpLo90', 'ncpHi90', 'fmin', 'f0', 'f0Lo90', 'f0Hi90', 'rmsea', 'rmseaLo90', 'rmseaHi90', 'pclose',
  'aic', 'bcc', 'bic', 'caic', 'ecvi', 'ecviLo90', 'ecviHi90', 'mecvi', 'hoelter05', 'hoelter01',
];
/** LIMITATIONS (:434-446), in decision 745's order. */
const CFA_LIMITATIONS = [
  'ml_only',
  'no_means',
  'one_group_only',
  'simple_structure_only',
  'marker_identification_only',
  'no_modification_indices',
  'normal_theory_standard_errors',
  'no_srmr',
  'listwise_only',
  'no_weights',
].map(cfCode);
/** The notes in their order (:1382-1392). */
const CFA_NOTES = ['negative_variance', 'factor_covariance_not_positive_definite', 'df_zero'].map(cfCode);

const cfDiagnostic = (v: unknown) =>
  diagnostic(CF_CODES, CF_PARAM_KEYS, false)(v) &&
  isObject(v) &&
  (v.code !== cfCode('suspect_missing_code') || (isObject(v.params) && ['column', 'value', 'count'].every((k) => isNum((v.params as O)[k]))));

/** An exclusion, exactly as stats_cfa writes it (:1394-1406). */
const CF_EXCLUDED = [cfCode('missing_value'), cfCode('missing_code')];
const cfExclusion = (e: unknown) =>
  isObject(e) &&
  Object.keys(e).length === 3 &&
  oneOf(e.code, CF_EXCLUDED) &&
  isCount(e.count) &&
  e.count >= 1 &&
  isObject(e.params) &&
  Object.keys(e.params).length === 1 &&
  isCount(e.params.column);

/** The exclusions in the columns' file order (sorted by alias, :1394-1396), missing_value
 *  before missing_code for one column, each pair once, each column a column of the model, and
 *  missing_code only with missing codes. A row is counted once, under its first column in the
 *  file's order without a value (Rows::of, :464-493). */
const cfExclusionsAgree = (list: O[], numbers: number[], codes: unknown[]) => {
  const sorted = numbers.slice().sort((a, b) => a - b);
  const order = list.map((e) => sorted.indexOf(((e.params as O).column as number) ?? -1) * 2 + (e.code === CF_EXCLUDED[1] ? 1 : 0));
  if (order.some((o) => o < 0)) return false;
  if (order.some((o, i) => i > 0 && o <= order[i - 1])) return false;
  return codes.length > 0 || list.every((e) => e.code !== CF_EXCLUDED[1]);
};

/** stats_linalg::cholesky_within(Φ, 0) (stats_linalg/mod.rs:173-195), its operations in its
 *  order: true when it factors, false at the first value left at or below 0, or not a number. */
const choleskyAtZero = (a: number[][]): boolean => {
  const n = a.length;
  const l = a.map(() => a.map(() => 0));
  for (let j = 0; j < n; j += 1) {
    let left = a[j][j];
    for (let k = 0; k < j; k += 1) left -= l[j][k] * l[j][k];
    if (!(left > 0 * a[j][j])) return false;
    const d = Math.sqrt(left);
    l[j][j] = d;
    for (let i = j + 1; i < n; i += 1) {
      let s = a[i][j];
      for (let k = 0; k < j; k += 1) s -= l[i][k] * l[j][k];
      l[i][j] = s / d;
    }
  }
  return true;
};

/** A spec's indicators as check requires them (:545-641): 1 to 16 factors, each of at least
 *  2 columns, 3 to 128 in all, each factor's columns ascending, the factors ordered by first
 *  column, no column twice; with the factors uncorrelated and two or more, no factor of two
 *  columns (:579-586). Their alias numbers, or null. */
function cfIndicators(sp: O): number[][] | null {
  if (!Array.isArray(sp.indicators) || typeof sp.correlatedFactors !== 'boolean') return null;
  const f = sp.indicators as unknown[];
  if (f.length < 1 || f.length > CFA_LIMITS.maxFactors) return null;
  const out: number[][] = [];
  for (const columns of f) {
    if (!Array.isArray(columns) || columns.length < CFA_LIMITS.minFactorItems) return null;
    const numbers = columns.map(aliasNumber);
    if (numbers.some((x) => x === null)) return null;
    const ns = numbers as number[];
    if (ns.some((x, i) => i > 0 && !(x > ns[i - 1]))) return null;
    out.push(ns);
  }
  if (out.some((x, i) => i > 0 && !(x[0] > out[i - 1][0]))) return null;
  const all = out.flat();
  if (all.length < CFA_LIMITS.minItems || all.length > CFA_LIMITS.maxItems || new Set(all).size !== all.length) return null;
  if (!sp.correlatedFactors && out.length >= 2 && out.some((x) => x.length === 2)) return null;
  return out;
}

/** An estimate with its S.E., C.R. and two-tailed P, as `tested` writes them (:1299-1302): the
 *  estimate a value, the S.E. `stat` of a square root, C.R. exactly estimate / S.E., and P a
 *  value in [0, 1], or too_small where C.R. is not a number. */
function cfTested(t: O): boolean {
  if (!isValue(t.estimate) || !cfStat(t.se, [CF_SMALL, CF_OVERFLOW]) || !cfStat(t.cr, [CF_SMALL, CF_OVERFLOW]) || !cfStat(t.p, [CF_SMALL])) return false;
  const se = isValue(t.se) ? cfVal(t.se) : ncCode(t.se) === CF_SMALL ? Number.NaN : Number.POSITIVE_INFINITY;
  if (isValue(t.se) && !(se >= 0)) return false;
  const cr = cfVal(t.estimate) / se;
  if (!cfIs(t.cr, cfStatOf(cr))) return false;
  if (Number.isNaN(cr)) return ncCode(t.p) === CF_SMALL || (isValue(t.p) && cfVal(t.p) >= 0 && cfVal(t.p) <= 1);
  return isValue(t.p) && cfVal(t.p) >= 0 && cfVal(t.p) <= 1;
}

/** One model's row (model_fit, fit:287-372), against the independence model's C_b and d_b, for
 *  p columns and N rows. `npar` and `df` as the spec gives them; every measure Rust builds from
 *  CMIN, DF, NPAR, GFI and N with + − × ÷ and √, exactly; the intervals from the stored NCP
 *  interval; BIC and CAIC within a band; P, PCLOSE, the NCP interval and HOELTER, which take a
 *  distribution, by their ranges and codes. */
function cfRow(f: O, q: number, d: number, cb: number, db: number, p: number, bigN: number): boolean {
  if (!hasExactly(f, CF_MEASURES)) return false;
  if (f.npar !== q || f.df !== d) return false;
  // CMIN `stat(c)`, C = (N − 1) F at a converged F ≥ 0 (fit:183-184): a value at least 0.
  if (!isValue(f.cmin) || !(cfVal(f.cmin) >= 0)) return false;
  // RMR a square root and GFI 1 − a ratio of sums (fit:221-243): values.
  // RMR and GFI are `stat`'s (fit:333-334), so either may overflow: rmr_and_gfi squares the
  // residuals with no guard (fit:221-243; the review of 9C-3, finding 4). A GFI that is not a
  // number gives AGFI and PGFI too_small, an infinite one overflow (1 − (p*/d)(1 − ±∞) and
  // (d/p*)(±∞) are both infinite at d above 0).
  if (!cfStat(f.rmr, [CF_SMALL, CF_OVERFLOW]) || (isValue(f.rmr) && !(cfVal(f.rmr) >= 0)) || !cfStat(f.gfi, [CF_SMALL, CF_OVERFLOW])) return false;
  const c = cfVal(f.cmin);
  const gfi = isValue(f.gfi) ? cfVal(f.gfi) : ncCode(f.gfi) === CF_SMALL ? Number.NaN : Number.POSITIVE_INFINITY;
  const n = bigN - 1;
  const pf = p;
  const qf = q;
  const df = d;
  const pstar = (p * (p + 1)) / 2;
  const dbf = db;
  const ncp = Math.max(c - df, 0);
  const nfi = 1 - c / cb;
  const cfiDenominator = Math.max(Math.max(cb - dbf, c - df), 0);
  const cfi = cfiDenominator === 0 ? 1 : 1 - ncp / cfiDenominator;
  const pratio = df / dbf;
  const bcc: Expected = bigN <= pf + 2 ? { code: CF_BCC } : cfStatOf(c + (2 * qf * (bigN - 1)) / (bigN - pf - 2));
  const exact: Array<[string, Expected]> = [
    ['cminDf', unlessDfZero(d, () => cfStatOf(c / df))],
    ['agfi', unlessDfZero(d, () => cfStatOf(1 - (pstar / df) * (1 - gfi)))],
    ['pgfi', unlessDfZero(d, () => cfStatOf((df / pstar) * gfi))],
    ['nfi', againstOf(nfi, cb)],
    ['rfi', unlessDfZero(d, () => againstOf(1 - c / df / (cb / dbf), cb))],
    ['ifi', againstOf((cb - c) / (cb - df), cb)],
    ['tli', unlessDfZero(d, () => againstOf((cb / dbf - c / df) / (cb / dbf - 1), cb))],
    ['cfi', againstOf(cfi, cb)],
    ['pratio', unlessDfZero(d, () => cfStatOf(pratio))],
    ['pnfi', unlessDfZero(d, () => againstOf(nfi * pratio, cb))],
    ['pcfi', unlessDfZero(d, () => againstOf(cfi * pratio, cb))],
    ['ncp', cfStatOf(ncp)],
    ['fmin', cfStatOf(c / n)],
    ['f0', cfStatOf(ncp / n)],
    ['rmsea', unlessDfZero(d, () => cfStatOf(Math.sqrt(ncp / n / df)))],
    ['aic', cfStatOf(c + 2 * qf)],
    ['bcc', bcc],
    ['ecvi', cfStatOf((c + 2 * qf) / n)],
    ['mecvi', 'value' in bcc ? cfStatOf(bcc.value / n) : bcc],
  ];
  for (const [k, e] of exact) if (!cfIs(f[k], e)) return false;
  // BIC and CAIC (fit:360-361), within a band of ln N's rounding.
  const lnN = Math.log(bigN);
  for (const [k, x] of [
    ['bic', c + qf * lnN],
    ['caic', c + qf * (lnN + 1)],
  ] as Array<[string, number]>) {
    if (!isValue(f[k])) return false;
    const gap = Math.abs(cfVal(f[k]) - x) / Math.max(Math.abs(x), Number.MIN_VALUE);
    if (cfMeasure('lnN', gap) > CFA_BANDS.lnN) return false;
  }
  // P and PCLOSE (fit:331, fit:355-357): df_zero at 0 degrees of freedom; else a probability, or
  // distribution_out_of_range where the distribution gives none.
  for (const k of ['p', 'pclose']) {
    if (d === 0 ? ncCode(f[k]) !== CF_DF_ZERO : !(cfStat(f[k], [CF_RANGE]) && (!isValue(f[k]) || (cfVal(f[k]) >= 0 && cfVal(f[k]) <= 1)))) return false;
  }
  // The NCP interval (fit:303-305, fit:346-347) and what Rust builds from it (fit:350-354,
  // fit:363-364): at 0 degrees of freedom df_zero; else each end a value at least 0 or
  // distribution_out_of_range, and F0's, RMSEA's and ECVI's ends exactly from it.
  for (const [end, f0k, rk, ek] of [
    ['ncpLo90', 'f0Lo90', 'rmseaLo90', 'ecviLo90'],
    ['ncpHi90', 'f0Hi90', 'rmseaHi90', 'ecviHi90'],
  ]) {
    if (d === 0) {
      if ([end, f0k, rk, ek].some((k) => ncCode(f[k]) !== CF_DF_ZERO)) return false;
      continue;
    }
    if (!cfStat(f[end], [CF_RANGE])) return false;
    if (!isValue(f[end])) {
      if ([f0k, rk, ek].some((k) => ncCode(f[k]) !== CF_RANGE)) return false;
      continue;
    }
    const x = cfVal(f[end]);
    if (!(x >= 0)) return false;
    if (!cfIs(f[f0k], cfStatOf(x / n)) || !cfIs(f[rk], cfStatOf(Math.sqrt(x / (n * df)))) || !cfIs(f[ek], cfStatOf((x + df + 2 * qf) / n))) return false;
  }
  if (d !== 0 && isValue(f.ncpLo90) && isValue(f.ncpHi90) && !(cfVal(f.ncpLo90) <= cfVal(f.ncpHi90))) return false;
  // HOELTER (fit:315-326): df_zero at 0 degrees of freedom, cmin_zero at a CMIN of 0, else a
  // whole number at least 1, overflow, or distribution_out_of_range.
  for (const k of ['hoelter05', 'hoelter01']) {
    const h = f[k];
    if (d === 0) {
      if (ncCode(h) !== CF_DF_ZERO) return false;
    } else if (c === 0) {
      if (ncCode(h) !== CF_CMIN_ZERO) return false;
    } else if (!cfStat(h, [CF_OVERFLOW, CF_RANGE]) || (isValue(h) && !(Number.isInteger(cfVal(h)) && cfVal(h) >= 1))) {
      return false;
    }
  }
  return true;
}

function cfaBundle(v: O, spec: O): CfaResultView | null {
  const sp = spec.parameters;
  const r = v.primaryOutputs;
  if (!isObject(sp) || !hasExactly(sp, ['indicators', 'correlatedFactors', 'missingCodes'])) return null;
  const factors = cfIndicators(sp);
  if (factors === null) return null;
  // The codes finite, ascending and once each (:547-557).
  const codes = sp.missingCodes;
  if (!Array.isArray(codes) || !codes.every(isNum) || codes.some((c, i) => i > 0 && !((c as number) > (codes[i - 1] as number)))) return null;
  if (!isObject(r) || !hasExactly(r, CF_RESULT_KEYS)) return null;
  if (!isCount(v.runId) || v.status !== 'succeeded') return null;
  const k = factors.length;
  const numbers = factors.flat();
  const p = numbers.length;
  const items = numbers.map((x) => `C${x}`);

  // Shape: the items, factor by factor, and the indicators, as the spec (:1408-1410).
  if (JSON.stringify(r.items) !== JSON.stringify(items) || JSON.stringify(r.indicators) !== JSON.stringify(sp.indicators)) return null;
  // At least p + 1 complete rows (:668-674), and 0 to 500 iterations (:97).
  if (!isCount(r.rowsUsed) || r.rowsUsed < p + 1) return null;
  if (!isCount(r.iterations) || r.iterations > CF_MAX_ITERATIONS) return null;
  const bigN = r.rowsUsed;
  const factorOf = factors.flatMap((cols, f) => cols.map(() => f + 1));
  const marker = factors.flatMap((cols) => cols.map((_, t) => t === 0));

  // The loadings, one per column in items' order (:1328-1342): a marker's estimate exactly 1
  // and its S.E., C.R. and P fixed_parameter; every other one tested.
  if (!Array.isArray(r.loadings) || r.loadings.length !== p) return null;
  const lambda: number[] = [];
  for (let i = 0; i < p; i += 1) {
    const l = r.loadings[i];
    if (!isObject(l) || !hasExactly(l, CF_LOADING_KEYS) || l.factor !== factorOf[i] || l.item !== items[i]) return null;
    if (marker[i]) {
      if (statValue(l.estimate) !== 1 || !hasExactly(l.estimate as O, ['value'])) return null;
      if ([l.se, l.cr, l.p].some((s) => ncCode(s) !== CF_FIXED || !hasExactly(s as O, ['not_computable']))) return null;
    } else if (!cfTested(l)) {
      return null;
    }
    lambda.push(cfVal(l.estimate));
  }
  // The factors' variances, one per factor in order (:1343-1349).
  if (!Array.isArray(r.factorVariances) || r.factorVariances.length !== k) return null;
  const phi = Array.from({ length: k }, () => Array.from({ length: k }, () => 0));
  for (let f = 0; f < k; f += 1) {
    const x = r.factorVariances[f];
    if (!isObject(x) || !hasExactly(x, CF_VARIANCE_KEYS) || x.factor !== f + 1 || !cfTested(x)) return null;
    phi[f][f] = cfVal(x.estimate);
  }
  // The covariances: every pair f < g in Rust's order when the factors covary and there are
  // two or more (Model::of, :816-820), none otherwise (:1350-1364).
  const pairs: Array<[number, number]> = [];
  if (sp.correlatedFactors) for (let f = 0; f < k; f += 1) for (let g = f + 1; g < k; g += 1) pairs.push([f, g]);
  if (!Array.isArray(r.factorCovariances) || r.factorCovariances.length !== pairs.length) return null;
  for (let t = 0; t < pairs.length; t += 1) {
    const x = r.factorCovariances[t];
    const [f, g] = pairs[t];
    if (!isObject(x) || !hasExactly(x, CF_COVARIANCE_KEYS) || JSON.stringify(x.factors) !== JSON.stringify([f + 1, g + 1]) || !cfTested(x)) return null;
    phi[f][g] = cfVal(x.estimate);
    phi[g][f] = cfVal(x.estimate);
  }
  // The error variances, one per column in items' order (:1366-1372).
  if (!Array.isArray(r.errorVariances) || r.errorVariances.length !== p || !Array.isArray(r.smc) || r.smc.length !== p) return null;
  const theta: number[] = [];
  for (let i = 0; i < p; i += 1) {
    const x = r.errorVariances[i];
    if (!isObject(x) || !hasExactly(x, CF_ERROR_KEYS) || x.item !== items[i] || !cfTested(x)) return null;
    theta.push(cfVal(x.estimate));
  }
  // Σ's diagonal, λ_i λ_i φ_ff + θ_i (Model::sigma, :850-864), and from it the standardized
  // weights (:1331-1335), the correlations (:1356-1360) and the SMCs (:1373-1378), exactly;
  // negative_variance exactly where Rust's test `> 0.0` fails for a variance under a root.
  const negative = { code: CF_NEGATIVE };
  for (let i = 0; i < p; i += 1) {
    const f = factorOf[i] - 1;
    const sii = lambda[i] * lambda[i] * phi[f][f] + theta[i];
    const l = r.loadings[i] as O;
    const standardized = phi[f][f] > 0 && sii > 0 ? cfStatOf((lambda[i] * Math.sqrt(phi[f][f])) / Math.sqrt(sii)) : negative;
    if (!cfIs(l.standardized, standardized)) return null;
    const s = r.smc[i];
    if (!isObject(s) || !hasExactly(s, CF_SMC_KEYS) || s.item !== items[i]) return null;
    if (!cfIs(s.value, sii > 0 ? cfStatOf(1 - theta[i] / sii) : negative)) return null;
  }
  for (let t = 0; t < pairs.length; t += 1) {
    const [f, g] = pairs[t];
    const x = r.factorCovariances[t] as O;
    const e = phi[f][f] > 0 && phi[g][g] > 0 ? cfStatOf(phi[f][g] / Math.sqrt(phi[f][f] * phi[g][g])) : negative;
    if (!cfIs(x.correlation, e)) return null;
  }

  // The three rows (fit_measures, fit:172-217): NPAR and DF from the spec; the saturated row's
  // and the independence row's written cells exactly; each row's measures from its CMIN.
  const fit = r.fit;
  if (!isObject(fit) || !hasExactly(fit, CF_FIT_KEYS) || !isObject(fit.model) || !isObject(fit.saturated) || !isObject(fit.independence)) return null;
  const moments = (p * (p + 1)) / 2;
  const q = p - k + k + pairs.length + p;
  if (q > moments) return null; // degrees_of_freedom refuses it (:1162-1170).
  const d = moments - q;
  const db = (p * (p - 1)) / 2;
  const ind = fit.independence as O;
  if (!isValue(ind.cmin)) return null;
  const cb = cfVal(ind.cmin);
  // The independence row (fit:199-215): Σ = diag(S), q = p, d = d_b, C = C_b; its comparisons
  // written, 0 but PRATIO 1.
  const written = (row: O, cells: Record<string, number>) => Object.keys(cells).every((key) => statValue(row[key]) === cells[key] && hasExactly(row[key] as O, ['value']));
  // The written cells are checked as written; the rest of the row by model_fit's rules
  // (cfRowExcept).
  if (!written(ind, { nfi: 0, rfi: 0, ifi: 0, tli: 0, cfi: 0, pratio: 1, pnfi: 0, pcfi: 0 })) return null;
  if (!cfRowExcept(ind, p, db, cb, db, p, bigN, ['nfi', 'rfi', 'ifi', 'tli', 'cfi', 'pratio', 'pnfi', 'pcfi'])) return null;
  // The saturated row (saturated_fit, fit:259-268): q = p*, d = 0, C = 0, RMR 0 and GFI 1
  // written, and NFI and IFI written 1.
  const sat = fit.saturated as O;
  if (!written(sat, { cmin: 0, rmr: 0, gfi: 1, nfi: 1, ifi: 1 })) return null;
  if (!cfRowExcept(sat, moments, 0, cb, db, p, bigN, ['nfi', 'ifi'])) return null;
  // The default row (fit:194).
  if (!cfRow(fit.model as O, q, d, cb, db, p, bigN)) return null;

  // The notes (:1382-1392): a subset of the three in order; negative_variance exactly when a
  // factor variance or an error variance is below 0; the factors' covariance note exactly when
  // cholesky_within(Φ, 0) refuses; df_zero exactly at 0 degrees of freedom.
  const notes = r.notes;
  if (!Array.isArray(notes) || notes.some((x) => !oneOf(x, CFA_NOTES))) return null;
  const at = notes.map((x) => CFA_NOTES.indexOf(x as string));
  if (at.some((x, i) => i > 0 && x <= at[i - 1])) return null;
  const noted = (c: string) => notes.indexOf(cfCode(c)) >= 0;
  if (noted('negative_variance') !== (phi.some((row, f) => row[f] < 0) || theta.some((e) => e < 0))) return null;
  if (noted('factor_covariance_not_positive_definite') !== !choleskyAtZero(phi)) return null;
  if (noted('df_zero') !== (d === 0)) return null;

  if (!Array.isArray(r.limitations) || JSON.stringify(r.limitations) !== JSON.stringify(CFA_LIMITATIONS)) return null;

  // Exclusions and the manifest, as the factor analysis's, the model's columns as its items.
  if (!arrayOf(r.exclusions, cfExclusion) || !cfExclusionsAgree(r.exclusions as O[], numbers, codes)) return null;
  if (!arrayOf(v.diagnostics, cfDiagnostic) || !arrayOf(v.limitations, cfDiagnostic)) return null;
  // A succeeded run's diagnostics are validate's warnings, each a suspect code of a column of
  // the model (:705-722); its limitations the module's ten, as notes, in order.
  for (const x of v.diagnostics as O[]) {
    if (x.severity !== 'warning' || x.code !== cfCode('suspect_missing_code') || numbers.indexOf((x.params as O).column as number) < 0) return null;
  }
  const lims = v.limitations as O[];
  if (lims.length !== CFA_LIMITATIONS.length || lims.some((x, i) => x.code !== CFA_LIMITATIONS[i] || x.severity !== 'note' || Object.keys(x.params as O).length !== 0)) return null;
  if (!manifest(v.manifest, CFA_CAPABILITY, cfExclusion)) return null;
  const mf = v.manifest as O;
  if (mf.rowsUsed !== bigN) return null;
  if (mf.rowsRead !== bigN + (r.exclusions as Array<{ count: number }>).reduce((s, x) => s + x.count, 0)) return null;
  if (JSON.stringify(mf.exclusions) !== JSON.stringify(r.exclusions)) return null;
  return {
    runId: v.runId as number,
    status: 'succeeded',
    result: r as unknown as CfaResultView['result'],
    diagnostics: v.diagnostics as CfaDiagnostic[],
    manifest: v.manifest as CfaResultView['manifest'],
  };
}

/** cfRow with the named cells, written by fit_measures over model_fit's, replaced by what
 *  model_fit computes before the rules run, so every other cell is held to its rule. */
function cfRowExcept(row: O, q: number, d: number, cb: number, db: number, p: number, bigN: number, writtenKeys: string[]): boolean {
  if (!hasExactly(row, CF_MEASURES) || !isValue(row.cmin)) return false;
  const c = cfVal(row.cmin);
  const df = d;
  const ncp = Math.max(c - df, 0);
  const nfi = 1 - c / cb;
  const cfiDenominator = Math.max(Math.max(cb - db, c - df), 0);
  const cfi = cfiDenominator === 0 ? 1 : 1 - ncp / cfiDenominator;
  const pratio = df / db;
  const computed: Record<string, Expected> = {
    nfi: againstOf(nfi, cb),
    rfi: unlessDfZero(d, () => againstOf(1 - c / df / (cb / db), cb)),
    ifi: againstOf((cb - c) / (cb - df), cb),
    tli: unlessDfZero(d, () => againstOf((cb / db - c / df) / (cb / db - 1), cb)),
    cfi: againstOf(cfi, cb),
    pratio: unlessDfZero(d, () => cfStatOf(pratio)),
    pnfi: unlessDfZero(d, () => againstOf(nfi * pratio, cb)),
    pcfi: unlessDfZero(d, () => againstOf(cfi * pratio, cb)),
  };
  const asComputed: O = { ...row };
  for (const key of writtenKeys) {
    const e = computed[key];
    asComputed[key] = 'value' in e ? { value: e.value } : { not_computable: e.code };
  }
  return cfRow(asComputed, q, d, cb, db, p, bigN);
}

/** A failed run's diagnostic (decision 786): one of the run's four refusals, which only the run
 *  reaches (:1173-1268). sample_covariance_singular and overflow name a column of the spec;
 *  cfa_not_converged's count at most its limit, and its limit 500; model_not_identified with
 *  no params, or with count. */
function cfaFailure(failure: unknown, spec: O): boolean {
  if (!isObject(failure) || failure.severity !== 'error' || !isObject(failure.params)) return false;
  const sp = spec.parameters;
  // The spec held to the rules cfaBundle holds it to (the review of 9C-3, finding 6): exactly
  // CfaParams' keys, the indicators as check requires them, the codes finite, ascending, once.
  if (!isObject(sp) || !hasExactly(sp, ['indicators', 'correlatedFactors', 'missingCodes']) || cfIndicators(sp) === null) return false;
  const codes = sp.missingCodes;
  if (!Array.isArray(codes) || !codes.every(isNum) || codes.some((c, i) => i > 0 && !((c as number) > (codes[i - 1] as number)))) return false;
  const numbers = (cfIndicators(sp) as number[][]).flat();
  const keys = Object.keys(failure.params).sort();
  const pr = failure.params as O;
  switch (failure.code) {
    case cfCode('sample_covariance_singular'):
    case cfCode('overflow'):
      return keys.length === 1 && keys[0] === 'column' && isCount(pr.column) && numbers.indexOf(pr.column) >= 0;
    case cfCode('cfa_not_converged'):
      return JSON.stringify(keys) === JSON.stringify(['count', 'limit']) && pr.limit === CF_MAX_ITERATIONS && isCount(pr.count) && pr.count <= CF_MAX_ITERATIONS;
    case cfCode('model_not_identified'):
      return keys.length === 0 || (JSON.stringify(keys) === JSON.stringify(['count']) && isCount(pr.count) && pr.count >= 1);
    default:
      return false;
  }
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
  /** A failed run's diagnostic beyond its code and keys, given the stored spec (decision
   *  688). Only the linear regression's, the factor analysis's (decision 733) and the
   *  confirmatory factor analysis's (decision 786) checkers set it; every other failed run is
   *  checked by its code and keys alone, as before. */
  failure?: (failure: unknown, spec: O) => boolean;
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
  // Research OS, Phase 9A-2 (decision 688).
  [LINEAR_REGRESSION_CAPABILITY]: {
    which: 'linear_regression',
    codes: LR_CODES,
    keys: LR_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile, spec) => {
      const result = linearRegressionBundle(v, spec);
      return result ? { kind: 'linear_regression', result, dataFile } : null;
    },
    failure: linearRegressionFailure,
  },
  // Research OS, Phase 9B-3 (decision 733).
  [FACTOR_CAPABILITY]: {
    which: 'factor',
    codes: FA_CODES,
    keys: FA_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile, spec) => {
      const result = factorBundle(v, spec);
      return result ? { kind: 'factor', result, dataFile } : null;
    },
    failure: factorFailure,
  },
  // Research OS, Phase 9C-3 (decision 786).
  [CFA_CAPABILITY]: {
    which: 'cfa',
    codes: CF_CODES,
    keys: CF_PARAM_KEYS,
    strings: false,
    bundle: (v, dataFile, spec) => {
      const result = cfaBundle(v, spec);
      return result ? { kind: 'cfa', result, dataFile } : null;
    },
    failure: cfaFailure,
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
    if (checker.failure && !checker.failure(record.failure, spec)) return { kind: 'badShape' };
    return { kind: 'failed', capability: which, failure: record.failure as StoredFailure, dataFile };
  }
  if (!isObject(bundle)) return { kind: 'badShape' };
  if (!preparationsAgree(spec, bundle.manifest)) return { kind: 'badShape' };
  return checker.bundle(bundle, dataFile, spec) ?? { kind: 'badShape' };
}
