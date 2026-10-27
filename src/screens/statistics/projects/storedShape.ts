// Gaply — the shape check a saved result passes before anything renders it
// (Research OS, Phase 5B-3b; decision 150). project_run_result returns the
// exact JSON text Rust stored (decision 129); this reads it, checks every
// field the live result components read, and only then hands them the same
// props the live screens give them.
//
// * The capability must be one this app knows (the summary, the t-test, or
//   since Phase 8D-2b the one-sample t-test),
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
import type { OneSampleDiagnostic, OneSampleResultView } from '../onesampleTypes';
import DESCRIBE_CODES from '../describeCodes.json';
import TTEST_CODES from '../ttestCodes.json';
import ONESAMPLE_CODES from '../onesampleCodes.json';
import { capabilityOfKind } from '../methods';

/** The schema version this app reads, for every stored part. */
export const KNOWN_SCHEMA_VERSION = 1;

/** The two methods' capability ids, from the methods table (Phase 8C). */
export const SUMMARY_CAPABILITY = capabilityOfKind('describe');
export const TTEST_CAPABILITY = capabilityOfKind('ttest');
export const ONE_SAMPLE_CAPABILITY = capabilityOfKind('one_sample');

/** A data file's formats: the ingest's (DataFormat in Rust, without json). */
const DATA_FORMATS = ['csv', 'tsv', 'xlsx', 'xlsm', 'xlsb', 'xls', 'ods', 'sav', 'mat'];

/** The case a method's checked results are shown as. */
export type CaseKind = 'summary' | 'ttest' | 'one_sample';

/** A failed run's stored diagnostic, in its method's codes. */
export type StoredFailure = Diagnostic | TTestDiagnostic | OneSampleDiagnostic;

/** What a saved result is, once checked: what to show, or why not. */
export type Checked =
  | { kind: 'summary'; result: ResultBundleView; dataFile: DataFileFacts }
  | { kind: 'ttest'; result: TTestResultView; dataFile: DataFileFacts }
  | { kind: 'one_sample'; result: OneSampleResultView; dataFile: DataFileFacts }
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
const SEVERITIES = ['error', 'warning', 'note'];
const DESC_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found', 'from', 'event'];
const T_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
const OS_PARAM_KEYS = ['column', 'count', 'index', 'value', 'limit', 'expected', 'found'];
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

function reproducibility(v: unknown): boolean {
  if (v === 'bitwise' || v === 'not_reproducible') return true;
  return isObject(v) && isObject(v.within_tolerance) && isNum(v.within_tolerance.relative);
}

function manifest(v: unknown, capability: string, codes: readonly string[], keys: readonly string[], strings: boolean): boolean {
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
    arrayOf(v.exclusions, exclusion(codes, keys, strings)) &&
    isObject(v.platform) &&
    oneOf(v.platform.os, ['macos', 'windows', 'linux', 'other']) &&
    oneOf(v.platform.arch, ['aarch64', 'x86_64', 'other']) &&
    isCount(v.elapsedMs) &&
    isHex64(v.outputHash) &&
    reproducibility(v.reproducibility)
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
    manifest(v.manifest, SUMMARY_CAPABILITY, DESC_CODES, DESC_PARAM_KEYS, true);
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
    arrayOf(r.exclusions, exclusion(T_CODES, T_PARAM_KEYS, false)) &&
    arrayOf(r.limitations, (c) => oneOf(c, T_CODES)) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, TTEST_CAPABILITY, T_CODES, T_PARAM_KEYS, false);
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
    arrayOf(r.exclusions, exclusion(OS_CODES, OS_PARAM_KEYS, false)) &&
    arrayOf(r.limitations, (c) => oneOf(c, OS_CODES)) &&
    arrayOf(v.diagnostics, d) &&
    arrayOf(v.limitations, d) &&
    manifest(v.manifest, ONE_SAMPLE_CAPABILITY, OS_CODES, OS_PARAM_KEYS, false);
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

/* ---- the checkers, one per method (Phase 8C; decision 249) ------------------- */

/** How one method's saved result is checked: the case it is shown as, its
 *  code list, its parameter keys, whether a parameter may be a string, and
 *  its bundle's check. */
interface StoredChecker {
  which: CaseKind;
  codes: readonly string[];
  keys: readonly string[];
  strings: boolean;
  bundle: (v: O, dataFile: DataFileFacts) => Checked | null;
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

  if (view.state === 'failed') {
    if (!isObject(record) || record.status !== 'failed') return { kind: 'badShape' };
    const ok = diagnostic(checker.codes, checker.keys, checker.strings);
    if (!ok(record.failure)) return { kind: 'badShape' };
    return { kind: 'failed', capability: which, failure: record.failure as StoredFailure, dataFile };
  }
  if (!isObject(bundle)) return { kind: 'badShape' };
  return checker.bundle(bundle, dataFile) ?? { kind: 'badShape' };
}
