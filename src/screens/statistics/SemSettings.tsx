// Gaply — the structural equation model's settings, its model editor (Research OS, Phase 9C-5,
// decisions 814 and 815), on CfaSettings.tsx's pattern. A column is chosen once, in the left pane:
// "Not in the model" (chosen at the start), "On its own", "Factor 1" to "Factor k" for the k
// factors made, and "A new factor" while k is below the most factors. Everything else is built
// in the right pane from what is chosen there: the factors, the paths, the covariances, the
// groups of parameters held equal, and each candidate missing code of the model's columns.
//
// Factors are held by ids that do not change as the user edits, and shown numbered by their
// first column in the file's order, as the CFA's editor holds them. Path and covariance rows
// hold their ends by factor id or alias, never by a number or a label, so a factor's
// renumbering shows at once. The model's variables, in the model's order, are the factors by
// number, then the columns on their own in the file's order, which is Rust's SemVariable order.
//
// An edit that would leave the spec naming something no longer there, or no longer allowed, is
// resolved at once and never written: an end whose variable left the model, or a path's end that
// joined a factor, returns to "Choose a variable"; a factor whose last column left it is gone; a
// changed or removed row's parameter, and any parameter no longer offered (a loading that became
// its factor's first, a row made incomplete or repeated), leaves its group. Nothing is chosen for
// the user.
//
// The spec lists the factors in the order of their first columns, each its columns in the file's
// order; the paths by (to, from); each covariance's ends, and the covariances, in the model's
// order; each group's members, and the groups by their first members, in Rust's parameter order;
// the codes ascending and once each. So every one of decision 797's order rules always holds.
// Identification stays Rust's: validation says so after Run. What is held here is the user's
// choices, in this component's state only.
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button } from '../../design-system';
import type { DatasetView } from './describeTypes';
import type { SemParameter, SemSpec, SemVariable } from './semTypes';
import {
  MAX_COLUMNS,
  MAX_COVARIANCES,
  MAX_EQUAL_MEMBERS,
  MAX_FACTORS,
  MAX_PATHS,
  MIN_COLUMNS,
  MIN_FACTOR_ITEMS,
  SM_ADD_COVARIANCE,
  SM_ADD_EXOGENOUS,
  SM_ADD_GROUP,
  SM_ADD_PATH,
  SM_CAPS,
  SM_CHOOSE,
  SM_CODES_LEGEND,
  SM_COLUMNS_HEADING,
  SM_COVARIANCE_REPEATED,
  SM_COVARIANCE_WITH_ITSELF,
  SM_COVARIANCES_HEADING,
  SM_EQUAL_HEADING,
  SM_FACTORS_HEADING,
  SM_FIXED_MARK,
  SM_FROM,
  SM_KINDS,
  SM_MISSING,
  SM_NEW_FACTOR,
  SM_NO_COVARIANCE,
  SM_NO_FACTOR,
  SM_NO_GROUP,
  SM_NO_PATH,
  SM_NOT_IN_MODEL,
  SM_ON_ITS_OWN,
  SM_PATH_REPEATED,
  SM_PATH_TO_ITSELF,
  SM_PATHS_HEADING,
  SM_TO,
  semAloneUnused,
  semCovarianceLine,
  semCovarianceRow,
  semErrorName,
  semFactorHeading,
  semFactorOption,
  semFactorTooFew,
  semGroupHeading,
  semGroupShort,
  semMissingCodeQuestion,
  semWeightRow,
  type SemKind,
  type SemNames,
} from './semWording';
import { offeredBy } from './orderedColumns';
import { ALPHA_REMOVE_ITEM, columnLabel, columnNames } from './window/windowWording';

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');
const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

/* ---- Rust's orders (stats_sem/mod.rs:313-375) --------------------------------- */

/** SemVariable's order: a factor before a column, factors by number, columns by alias number. */
export function compareVariables(x: SemVariable, y: SemVariable): number {
  if ('factor' in x) return 'factor' in y ? x.factor - y.factor : -1;
  return 'factor' in y ? 1 : aliasNumber(x.column) - aliasNumber(y.column);
}
const KIND_ORDER = { loading: 0, path: 1, covariance: 2, variance: 3 } as const;
const kindOf = (p: SemParameter): SemKind => ('loading' in p ? 'loading' : 'path' in p ? 'path' : 'covariance' in p ? 'covariance' : 'variance');
/** SemParameter's order: loading, path, covariance, variance, then by its fields in order. */
export function compareParameters(x: SemParameter, y: SemParameter): number {
  const k = KIND_ORDER[kindOf(x)] - KIND_ORDER[kindOf(y)];
  if (k !== 0) return k;
  if ('loading' in x && 'loading' in y) return x.loading.factor - y.loading.factor || aliasNumber(x.loading.column) - aliasNumber(y.loading.column);
  if ('path' in x && 'path' in y) return compareVariables(x.path.to, y.path.to) || compareVariables(x.path.from, y.path.from);
  if ('covariance' in x && 'covariance' in y) return compareVariables(x.covariance.a, y.covariance.a) || compareVariables(x.covariance.b, y.covariance.b);
  if ('variance' in x && 'variance' in y) return compareVariables(x.variance.variable, y.variance.variable);
  return 0;
}
const sameVariable = (x: SemVariable, y: SemVariable) => compareVariables(x, y) === 0;

/** Decision 814: the spec, every one of decision 797's order rules holding. Paths sorted by (to,
 *  from); each covariance's ends, and the covariances, in the model's order; each group sorted,
 *  the groups by their first members; the codes ascending and once each; the view's preparation
 *  exactly when it has one. */
export function buildSemSpec(
  view: DatasetView,
  model: { indicators: string[][]; paths: Array<{ to: SemVariable; from: SemVariable }>; covariances: Array<{ a: SemVariable; b: SemVariable }>; equal: SemParameter[][] },
  missingCodes: number[],
  operationId: number
): SemSpec {
  const order = view.profile.columns.map((c) => c.alias);
  const at = (a: string) => order.indexOf(a);
  const indicators = model.indicators
    .map((f) => order.filter((a) => f.includes(a)))
    .filter((f) => f.length > 0)
    .sort((a, b) => at(a[0]) - at(b[0]));
  const paths = model.paths.map((p) => ({ to: p.to, from: p.from })).sort((x, y) => compareVariables(x.to, y.to) || compareVariables(x.from, y.from));
  const covariances = model.covariances
    .map((c) => (compareVariables(c.a, c.b) <= 0 ? { a: c.a, b: c.b } : { a: c.b, b: c.a }))
    .sort((x, y) => compareVariables(x.a, y.a) || compareVariables(x.b, y.b));
  const equal = model.equal
    .filter((g) => g.length > 0)
    .map((g) => [...g].sort(compareParameters))
    .sort((x, y) => compareParameters(x[0], y[0]));
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.sem',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { indicators, paths, covariances, equal, missingCodes: codes },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

/* ---- the model as the user holds it ------------------------------------------ */

/** A row's end: a factor by its id, or a column by its alias; null while not chosen. */
export type End = { factor: number } | { column: string } | null;

interface Row {
  id: number;
  first: End;
  second: End;
}
interface Group {
  id: number;
  kind: SemKind;
  /** Each member by its key (keyOf below). */
  members: string[];
}
interface Model {
  /** Each column in the model, by alias, to its factor's id or 'own'. */
  assigned: Record<string, number | 'own'>;
  /** Paths: `first` is the from end, `second` the to end. */
  paths: Row[];
  /** Covariances: `first` is a, `second` b, as the user chose them. */
  covariances: Row[];
  groups: Group[];
  next: number;
}

const EMPTY: Model = { assigned: {}, paths: [], covariances: [], groups: [], next: 1 };

/** A factor as drawn: its stable id, its number (from 1, by its first column in the file) and
 *  its columns in the file's order. */
export interface DrawnFactor {
  id: number;
  number: number;
  columns: string[];
}

/** An end's key, for a select's value and a group member's key: never a label. */
const endKey = (e: End): string => (e === null ? '' : 'factor' in e ? `f${e.factor}` : e.column);
const endOf = (key: string): End => (key === '' ? null : key.startsWith('f') ? { factor: Number(key.slice(1)) } : { column: key });

/** A parameter offered for a group, with its key, its spec form and its label. */
export interface OfferedParameter {
  key: string;
  kind: SemKind;
  parameter: SemParameter;
  label: string;
}

/** Where a select puts its column. */
export type SemTarget = 'none' | 'own' | 'new' | number;

export interface SemSettingsState {
  factors: DrawnFactor[];
  /** Every column in the model, in the file's order. */
  columns: string[];
  /** The columns on their own, in the file's order. */
  alone: string[];
  /** The model's variables in the model's order: factors by number, then columns on their own. */
  variables: Array<{ key: string; variable: SemVariable; name: string }>;
  paths: Array<{ id: number; from: string; to: string; complete: boolean; itself: boolean; repeats: number | null }>;
  covariances: Array<{ id: number; a: string; b: string; complete: boolean; itself: boolean; repeats: number | null; line: string | null }>;
  groups: Array<{ id: number; number: number; kind: SemKind; offered: OfferedParameter[]; ticked: string[] }>;
  /** Every parameter ticked in some group, by key, to the group's id. */
  heldBy: Record<string, number>;
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  names: SemNames;
  atCap: boolean;
  pathsAtCap: boolean;
  covariancesAtCap: boolean;
  equalAtCap: boolean;
  /** The pairs "Add every pair of exogenous variables" would add, and the count it would make. */
  exogenousToAdd: number;
  exogenousTotal: number;
  /** The columns on their own with no path or covariance, in the file's order. */
  unused: string[];
  stillMissing: string | null;
  /** The column's place in the model: 'none', 'own' or a factor's id. */
  placeOf: (alias: string) => 'none' | 'own' | number;
  spec: (operationId: number) => SemSpec;
  reset: () => void;
  assign: (alias: string, target: SemTarget) => void;
  addPath: () => void;
  setPathEnd: (id: number, which: 'from' | 'to', key: string) => void;
  removePath: (id: number) => void;
  addCovariance: () => void;
  setCovarianceEnd: (id: number, which: 'a' | 'b', key: string) => void;
  removeCovariance: (id: number) => void;
  addExogenousPairs: () => void;
  addGroup: () => void;
  setGroupKind: (id: number, kind: SemKind) => void;
  tick: (id: number, key: string, on: boolean) => void;
  removeGroup: (id: number) => void;
  toggleCode: (code: number, on: boolean) => void;
}

/** The factors as drawn from the columns in the file's order: numbered by first column. */
function drawFactors(model: Model, shown: string[]): DrawnFactor[] {
  const byId = new Map<number, string[]>();
  for (const a of shown) {
    const id = model.assigned[a];
    if (typeof id === 'number') byId.set(id, [...(byId.get(id) ?? []), a]);
  }
  return Array.from(byId.entries()).map(([id, cols], i) => ({ id, number: i + 1, columns: cols }));
}

/** An end as a spec variable, or null where it names nothing in the model. */
function variableOf(e: End, factors: DrawnFactor[], model: Model): SemVariable | null {
  if (e === null) return null;
  if ('factor' in e) {
    const f = factors.find((g) => g.id === e.factor);
    return f ? { factor: f.number } : null;
  }
  return model.assigned[e.column] !== undefined ? { column: e.column } : null;
}

/** Whether a variable is endogenous: a factor's column, or one some path goes to. A path row
 *  counts only once both its ends are chosen and differ (the review of 9C-5, finding 4). */
function endogenous(e: End, model: Model): boolean {
  if (e === null) return false;
  if ('column' in e && typeof model.assigned[e.column] === 'number') return true;
  return model.paths.some((p) => p.first !== null && p.second !== null && endKey(p.first) !== endKey(p.second) && endKey(p.second) === endKey(e));
}

/** Every parameter a group may hold now, keyed, in Rust's parameter order. */
function offeredParameters(model: Model, factors: DrawnFactor[], shown: string[], name: (e: End) => string): OfferedParameter[] {
  const out: OfferedParameter[] = [];
  for (const f of factors) {
    for (const a of f.columns.slice(1)) {
      out.push({ key: `L:${f.id}:${a}`, kind: 'loading', parameter: { loading: { factor: f.number, column: a } }, label: semWeightRow(name({ column: a }), name({ factor: f.id })) });
    }
  }
  const rowsOf = (rows: Row[], kind: 'path' | 'covariance') => {
    const seen = new Set<string>();
    for (const r of rows) {
      const x = variableOf(r.first, factors, model);
      const y = variableOf(r.second, factors, model);
      if (x === null || y === null || sameVariable(x, y)) continue;
      const key = kind === 'path' ? `${endKey(r.second)}<${endKey(r.first)}` : [endKey(r.first), endKey(r.second)].sort().join('~');
      if (seen.has(key)) continue;
      seen.add(key);
      if (kind === 'path') {
        out.push({ key: `P:${r.id}`, kind, parameter: { path: { to: y, from: x } }, label: semWeightRow(name(r.second), name(r.first)) });
      } else {
        const ordered = compareVariables(x, y) <= 0;
        const [a, b] = ordered ? [x, y] : [y, x];
        const [ea, eb] = ordered ? [r.first, r.second] : [r.second, r.first];
        const nameEnd = (e: End) => (endogenous(e, model) ? semErrorName(name(e)) : name(e));
        out.push({ key: `C:${r.id}`, kind, parameter: { covariance: { a, b } }, label: semCovarianceRow(nameEnd(ea), nameEnd(eb)) });
      }
    }
  };
  rowsOf(model.paths, 'path');
  rowsOf(model.covariances, 'covariance');
  const variances: End[] = [...factors.map((f) => ({ factor: f.id })), ...shown.filter((a) => model.assigned[a] !== undefined).map((a) => ({ column: a }))];
  for (const e of variances) {
    const v = variableOf(e, factors, model) as SemVariable;
    out.push({ key: `V:${endKey(e)}`, kind: 'variance', parameter: { variance: { variable: v } }, label: endogenous(e, model) ? semErrorName(name(e)) : name(e) });
  }
  return out.sort((x, y) => compareParameters(x.parameter, y.parameter));
}

/** Decision 814's resolutions: ends that name nothing in the model, or a path's end at a
 *  factor's column, return to "Choose a variable"; a group member no longer offered leaves its
 *  group. Applied after every edit and every view change. */
function normalize(model: Model, shown: string[]): Model {
  const assigned = Object.fromEntries(Object.entries(model.assigned).filter(([a]) => shown.includes(a)));
  const m: Model = { ...model, assigned };
  const factors = drawFactors(m, shown);
  const valid = (e: End, path: boolean): End => {
    if (e === null) return null;
    if ('factor' in e) return factors.some((f) => f.id === e.factor) ? e : null;
    const place = assigned[e.column];
    if (place === undefined) return null;
    return path && typeof place === 'number' ? null : e;
  };
  m.paths = model.paths.map((r) => ({ ...r, first: valid(r.first, true), second: valid(r.second, true) }));
  m.covariances = model.covariances.map((r) => ({ ...r, first: valid(r.first, false), second: valid(r.second, false) }));
  const keys = new Set(offeredParameters(m, factors, shown, () => '').map((p) => p.key));
  const kindOfKey = (k: string): SemKind => (k[0] === 'L' ? 'loading' : k[0] === 'P' ? 'path' : k[0] === 'C' ? 'covariance' : 'variance');
  m.groups = model.groups.map((g) => ({ ...g, members: g.members.filter((k) => keys.has(k) && kindOfKey(k) === g.kind) }));
  return m;
}

/** The user's choices, over the open file's view. */
export function useSemSettings(view: DatasetView | null): SemSettingsState {
  const [model, setModel] = useState<Model>(EMPTY);
  const [ticked, setTicked] = useState<number[]>([]);

  const shown = useMemo(() => (view ? numeric(view).map((c) => c.alias) : []), [view]);
  // A view change that removes a column takes it out of the model (decision 698's prune).
  useEffect(() => {
    setModel((m) => {
      const n = normalize(m, shown);
      return JSON.stringify(n) === JSON.stringify(m) ? m : n;
    });
  }, [shown]);
  const edit = useCallback((f: (m: Model) => Model) => setModel((m) => normalize(f(m), shown)), [shown]);

  const names: SemNames = useMemo(() => ({ column: columnNames(view) }), [view]);
  const factors = useMemo(() => drawFactors(model, shown), [model, shown]);
  const columns = useMemo(() => shown.filter((a) => model.assigned[a] !== undefined), [shown, model]);
  const alone = useMemo(() => shown.filter((a) => model.assigned[a] === 'own'), [shown, model]);
  const name = useCallback(
    (e: End): string => {
      if (e === null) return '';
      if ('factor' in e) return semFactorOption(factors.find((f) => f.id === e.factor)?.number ?? 0);
      return names.column(aliasNumber(e.column));
    },
    [factors, names]
  );
  const variables = useMemo(
    () => [
      ...factors.map((f) => ({ key: `f${f.id}`, variable: { factor: f.number } as SemVariable, name: semFactorOption(f.number) })),
      ...alone.map((a) => ({ key: a, variable: { column: a } as SemVariable, name: name({ column: a }) })),
    ],
    [factors, alone, name]
  );

  const rowsView = (rows: Row[], kind: 'path' | 'covariance') => {
    const done: string[] = [];
    return rows.map((r) => {
      const x = variableOf(r.first, factors, model);
      const y = variableOf(r.second, factors, model);
      const complete = x !== null && y !== null;
      const itself = complete && sameVariable(x as SemVariable, y as SemVariable);
      const key = kind === 'path' ? `${endKey(r.second)}<${endKey(r.first)}` : [endKey(r.first), endKey(r.second)].sort().join('~');
      const earlier = complete && !itself ? done.indexOf(key) : -1;
      done.push(complete && !itself ? key : `#${r.id}`);
      return { r, complete, itself, repeats: earlier >= 0 ? earlier + 1 : null };
    });
  };
  const pathRows = rowsView(model.paths, 'path');
  const covarianceRows = rowsView(model.covariances, 'covariance');
  const paths = pathRows.map(({ r, complete, itself, repeats }) => ({ id: r.id, from: endKey(r.first), to: endKey(r.second), complete, itself, repeats }));
  // A complete row's ends in the model's order, as the card names them (the review of 9C-5, finding 7).
  const inOrder = (r: Row): [End, End] => {
    const x = variableOf(r.first, factors, model);
    const y = variableOf(r.second, factors, model);
    return x !== null && y !== null && compareVariables(x, y) > 0 ? [r.second, r.first] : [r.first, r.second];
  };
  const covariances = covarianceRows.map(({ r, complete, itself, repeats }) => ({
    id: r.id,
    a: endKey(r.first),
    b: endKey(r.second),
    complete,
    itself,
    repeats,
    line: complete && !itself ? semCovarianceLine(name(inOrder(r)[0]), endogenous(inOrder(r)[0], model), name(inOrder(r)[1]), endogenous(inOrder(r)[1], model)) : null,
  }));

  const offeredAll = useMemo(() => offeredParameters(model, factors, shown, name), [model, factors, shown, name]);
  const heldBy: Record<string, number> = {};
  for (const g of model.groups) for (const k of g.members) heldBy[k] = g.id;
  // Decision 814: groups numbered by their first parameter in Rust's order; a group with none
  // after every group that has one, in the order made.
  const byKey = new Map(offeredAll.map((p) => [p.key, p]));
  const firstOf = (g: Group) => g.members.map((k) => byKey.get(k)?.parameter).filter((p): p is SemParameter => p !== undefined).sort(compareParameters)[0];
  const numbered = [...model.groups].sort((x, y) => {
    const a = firstOf(x);
    const b = firstOf(y);
    if (a && b) return compareParameters(a, b);
    if (a) return -1;
    if (b) return 1;
    return x.id - y.id;
  });
  const groups = numbered.map((g, i) => ({ id: g.id, number: i + 1, kind: g.kind, offered: offeredAll.filter((p) => p.kind === g.kind), ticked: g.members }));
  const memberCount = model.groups.reduce((n, g) => n + g.members.length, 0);

  const offered = useMemo(() => offeredBy(view, columns), [view, columns]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  // Decision 698: the ticks are pruned to the codes then offered.
  useEffect(() => {
    setTicked((t) => {
      const kept = t.filter((c) => offered.some((s) => s.code === c));
      return kept.length === t.length ? t : kept;
    });
  }, [offered]);

  // Exogenous variables: factors and columns on their own with no path into them.
  const exogenous = variables.filter((v) => !endogenous(endOf(v.key), model));
  const listed = new Set(model.covariances.filter((r) => r.first !== null && r.second !== null).map((r) => [endKey(r.first), endKey(r.second)].sort().join('~')));
  const pairs: Array<[string, string]> = [];
  for (let i = 0; i < exogenous.length; i++) {
    for (let j = i + 1; j < exogenous.length; j++) {
      const key = [exogenous[i].key, exogenous[j].key].sort().join('~');
      if (!listed.has(key)) pairs.push([exogenous[i].key, exogenous[j].key]);
    }
  }
  const exogenousTotal = model.covariances.length + pairs.length;

  const used = new Set([...model.paths, ...model.covariances].flatMap((r) => [endKey(r.first), endKey(r.second)]));
  const unused = alone.filter((a) => !used.has(a));

  const short = factors.find((f) => f.columns.length < MIN_FACTOR_ITEMS);
  const firstPath = (test: (p: (typeof paths)[number]) => boolean) => paths.findIndex(test);
  const firstCov = (test: (c: (typeof covariances)[number]) => boolean) => covariances.findIndex(test);
  const shortGroup = groups.find((g) => g.ticked.length < 2);
  let stillMissing: string | null = null;
  if (factors.length === 0 && model.paths.length === 0 && model.covariances.length === 0) stillMissing = SM_MISSING.empty;
  else if (columns.length < MIN_COLUMNS) stillMissing = SM_MISSING.columns;
  else if (short) stillMissing = SM_MISSING.factorShort(short.number);
  else if (unused.length > 0) stillMissing = SM_MISSING.alone(name({ column: unused[0] }));
  else if (firstPath((p) => !p.complete) >= 0) stillMissing = SM_MISSING.pathEnds(firstPath((p) => !p.complete) + 1);
  else if (firstPath((p) => p.itself) >= 0) stillMissing = SM_MISSING.pathItself(firstPath((p) => p.itself) + 1);
  else if (firstPath((p) => p.repeats !== null) >= 0) {
    const i = firstPath((p) => p.repeats !== null);
    stillMissing = SM_MISSING.pathRepeats(i + 1, paths[i].repeats as number);
  } else if (firstCov((c) => !c.complete) >= 0) stillMissing = SM_MISSING.covarianceEnds(firstCov((c) => !c.complete) + 1);
  else if (firstCov((c) => c.itself) >= 0) stillMissing = SM_MISSING.covarianceItself(firstCov((c) => c.itself) + 1);
  else if (firstCov((c) => c.repeats !== null) >= 0) {
    const i = firstCov((c) => c.repeats !== null);
    stillMissing = SM_MISSING.covarianceRepeats(i + 1, covariances[i].repeats as number);
  } else if (shortGroup) stillMissing = SM_MISSING.groupShort(shortGroup.number);

  const spec = useCallback(
    (operationId: number) => {
      const complete = (rows: Row[]) =>
        rows
          .map((r) => ({ x: variableOf(r.first, factors, model), y: variableOf(r.second, factors, model) }))
          .filter((r): r is { x: SemVariable; y: SemVariable } => r.x !== null && r.y !== null);
      return buildSemSpec(
        view as DatasetView,
        {
          indicators: factors.map((f) => f.columns),
          paths: complete(model.paths).map((r) => ({ from: r.x, to: r.y })),
          covariances: complete(model.covariances).map((r) => ({ a: r.x, b: r.y })),
          equal: model.groups.map((g) => g.members.map((k) => byKey.get(k)?.parameter).filter((p): p is SemParameter => p !== undefined)),
        },
        missingCodes,
        operationId
      );
    },
    // byKey is derived from model, factors and names.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [view, factors, model, missingCodes, offeredAll]
  );

  const newRow = (m: Model): [number, Model] => [m.next, { ...m, next: m.next + 1 }];
  const dropMember = (m: Model, key: string): Model => ({ ...m, groups: m.groups.map((g) => ({ ...g, members: g.members.filter((k) => k !== key) })) });

  return {
    factors,
    columns,
    alone,
    variables,
    paths,
    covariances,
    groups,
    heldBy,
    ticked,
    offered,
    names,
    atCap: columns.length >= MAX_COLUMNS,
    pathsAtCap: model.paths.length >= MAX_PATHS,
    covariancesAtCap: model.covariances.length >= MAX_COVARIANCES,
    equalAtCap: memberCount >= MAX_EQUAL_MEMBERS,
    exogenousToAdd: pairs.length,
    exogenousTotal,
    unused,
    stillMissing,
    placeOf: (alias) => {
      const p = model.assigned[alias];
      return p === undefined ? 'none' : p;
    },
    spec,
    reset: () => {
      setModel(EMPTY);
      setTicked([]);
    },
    assign: (alias, target) =>
      edit((m) => {
        const assigned = { ...m.assigned };
        if (target === 'none') {
          delete assigned[alias];
          return { ...m, assigned };
        }
        if (target === 'new') {
          assigned[alias] = m.next;
          return { ...m, assigned, next: m.next + 1 };
        }
        assigned[alias] = target;
        return { ...m, assigned };
      }),
    addPath: () =>
      edit((m) => {
        if (m.paths.length >= MAX_PATHS) return m;
        const [id, n] = newRow(m);
        return { ...n, paths: [...m.paths, { id, first: null, second: null }] };
      }),
    setPathEnd: (id, which, key) =>
      edit((m) => {
        const changed = dropMember(m, `P:${id}`);
        return { ...changed, paths: m.paths.map((r) => (r.id !== id ? r : which === 'from' ? { ...r, first: endOf(key) } : { ...r, second: endOf(key) })) };
      }),
    removePath: (id) => edit((m) => ({ ...dropMember(m, `P:${id}`), paths: m.paths.filter((r) => r.id !== id) })),
    addCovariance: () =>
      edit((m) => {
        if (m.covariances.length >= MAX_COVARIANCES) return m;
        const [id, n] = newRow(m);
        return { ...n, covariances: [...m.covariances, { id, first: null, second: null }] };
      }),
    setCovarianceEnd: (id, which, key) =>
      edit((m) => {
        const changed = dropMember(m, `C:${id}`);
        return { ...changed, covariances: m.covariances.map((r) => (r.id !== id ? r : which === 'a' ? { ...r, first: endOf(key) } : { ...r, second: endOf(key) })) };
      }),
    removeCovariance: (id) => edit((m) => ({ ...dropMember(m, `C:${id}`), covariances: m.covariances.filter((r) => r.id !== id) })),
    addExogenousPairs: () =>
      edit((m) => {
        if (pairs.length === 0 || m.covariances.length + pairs.length > MAX_COVARIANCES) return m;
        let next = m.next;
        const rows = pairs.map(([a, b]) => ({ id: next++, first: endOf(a), second: endOf(b) }));
        return { ...m, next, covariances: [...m.covariances, ...rows] };
      }),
    addGroup: () => edit((m) => ({ ...m, next: m.next + 1, groups: [...m.groups, { id: m.next, kind: 'loading', members: [] }] })),
    setGroupKind: (id, kind) => edit((m) => ({ ...m, groups: m.groups.map((g) => (g.id === id ? { ...g, kind, members: [] } : g)) })),
    tick: (id, key, on) =>
      edit((m) => {
        if (on) {
          const total = m.groups.reduce((n, g) => n + g.members.length, 0);
          if (total >= MAX_EQUAL_MEMBERS || m.groups.some((g) => g.members.includes(key))) return m;
          return { ...m, groups: m.groups.map((g) => (g.id === id ? { ...g, members: [...g.members, key] } : g)) };
        }
        return { ...m, groups: m.groups.map((g) => (g.id === id ? { ...g, members: g.members.filter((k) => k !== key) } : g)) };
      }),
    removeGroup: (id) => edit((m) => ({ ...m, groups: m.groups.filter((g) => g.id !== id) })),
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

/** A select's value for a column's place. */
const valueOf = (t: 'none' | 'own' | number) => (t === 'none' ? 'none' : t === 'own' ? 'own' : `f${t}`);
const targetOf = (v: string): SemTarget => (v === 'none' ? 'none' : v === 'own' ? 'own' : v === 'new' ? 'new' : Number(v.slice(1)));

/** A row's end's select: "Choose a variable" and the variables offered; a path's ends are
 *  labelled "From" and "To", a covariance's are not. */
const EndSelect: React.FC<{ value: string; options: Array<{ key: string; name: string }>; testId: string; label?: string; onChange: (key: string) => void }> = ({
  value,
  options,
  testId,
  label,
  onChange,
}) => {
  const select = (
    <select value={value} data-testid={testId} onChange={(e) => onChange(e.target.value)}>
      <option value="">{SM_CHOOSE}</option>
      {options.map((o) => (
        <option key={o.key} value={o.key}>
          {o.name}
        </option>
      ))}
    </select>
  );
  return label === undefined ? (
    select
  ) : (
    <label>
      {label} {select}
    </label>
  );
};

/** The controls: the dialog's. */
export const SemSettings: React.FC<{ view: DatasetView; settings: SemSettingsState }> = ({ view, settings: s }) => {
  const columns = numeric(view);
  const byAlias = new Map(columns.map((c) => [c.alias, c]));
  const label = (alias: string) => {
    const c = byAlias.get(alias);
    return c ? columnLabel(c) : alias;
  };
  // Every variable in the model, a factor's columns included: a covariance's ends.
  const covarianceOptions = [
    ...s.factors.map((f) => ({ key: `f${f.id}`, name: semFactorOption(f.number) })),
    ...s.columns.map((a) => ({ key: a, name: s.names.column(aliasNumber(a)) })),
  ].sort((x, y) => {
    const v = (k: string): SemVariable => (k.startsWith('f') ? { factor: s.factors.find((f) => `f${f.id}` === k)?.number ?? 0 } : { column: k });
    return compareVariables(v(x.key), v(y.key));
  });
  const pathOptions = s.variables.map((v) => ({ key: v.key, name: v.name }));
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="sm-settings">
      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
        <div data-testid="sm-columns">
          <h4 style={{ margin: '4px 0' }}>{SM_COLUMNS_HEADING}</h4>
          {s.atCap && (
            <p role="status" style={{ margin: '0 0 4px' }} data-testid="sm-at-cap">
              {SM_CAPS.columns}
            </p>
          )}
          {columns.map((c) => {
            const place = s.placeOf(c.alias);
            const locked = place === 'none' && s.atCap;
            return (
              <label key={c.alias} style={{ display: 'block' }}>
                <span>
                  {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>{' '}
                <select value={valueOf(place)} data-testid={`sm-assign-${c.alias}`} onChange={(e) => s.assign(c.alias, targetOf(e.target.value))}>
                  <option value="none">{SM_NOT_IN_MODEL}</option>
                  {!locked && <option value="own">{SM_ON_ITS_OWN}</option>}
                  {!locked &&
                    s.factors.map((g) => (
                      <option key={g.id} value={valueOf(g.id)}>
                        {semFactorOption(g.number)}
                      </option>
                    ))}
                  {!locked && s.factors.length < MAX_FACTORS && <option value="new">{SM_NEW_FACTOR}</option>}
                </select>
              </label>
            );
          })}
        </div>
        <div data-testid="sm-model">
          <section data-testid="sm-factors">
            <h4 style={{ margin: '4px 0' }}>{SM_FACTORS_HEADING}</h4>
            {s.factors.length === 0 && (
              <p style={{ margin: 0 }} data-testid="sm-no-factor">
                {SM_NO_FACTOR}
              </p>
            )}
            {s.factors.map((f) => (
              <section key={f.id} data-testid={`sm-factor-${f.number}`}>
                <h5 style={{ margin: '4px 0' }}>{semFactorHeading(f.number)}</h5>
                <ul style={{ margin: 0 }}>
                  {f.columns.map((a, i) => (
                    <li key={a} data-testid={`sm-factor-${f.number}-${a}`}>
                      {a} <bdi>{label(a)}</bdi>
                      {i === 0 && <> {SM_FIXED_MARK}</>}
                    </li>
                  ))}
                </ul>
                {f.columns.length < MIN_FACTOR_ITEMS && (
                  <p style={{ margin: 0 }} data-testid={`sm-factor-${f.number}-short`}>
                    {semFactorTooFew(f.number, f.columns.length)}
                  </p>
                )}
              </section>
            ))}
            {s.unused.map((a) => (
              <p key={a} style={{ margin: 0 }} data-testid={`sm-unused-${a}`}>
                {semAloneUnused(s.names.column(aliasNumber(a)))}
              </p>
            ))}
          </section>

          <section data-testid="sm-paths">
            <h4 style={{ margin: '4px 0' }}>{SM_PATHS_HEADING}</h4>
            {s.paths.length === 0 && (
              <p style={{ margin: 0 }} data-testid="sm-no-path">
                {SM_NO_PATH}
              </p>
            )}
            {s.paths.map((p, i) => (
              <div key={p.id} data-testid={`sm-path-${p.id}`}>
                <EndSelect value={p.from} options={pathOptions} testId={`sm-path-from-${p.id}`} label={SM_FROM} onChange={(k) => s.setPathEnd(p.id, 'from', k)} /> →{' '}
                <EndSelect value={p.to} options={pathOptions} testId={`sm-path-to-${p.id}`} label={SM_TO} onChange={(k) => s.setPathEnd(p.id, 'to', k)} />{' '}
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} path ${i + 1}`} onClick={() => s.removePath(p.id)} data-testid={`sm-path-remove-${p.id}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
                {p.itself && (
                  <p style={{ margin: 0 }} data-testid={`sm-path-itself-${p.id}`}>
                    {SM_PATH_TO_ITSELF}
                  </p>
                )}
                {p.repeats !== null && (
                  <p style={{ margin: 0 }} data-testid={`sm-path-repeated-${p.id}`}>
                    {SM_PATH_REPEATED}
                  </p>
                )}
              </div>
            ))}
            <Button variant="secondary" disabled={s.pathsAtCap} onClick={s.addPath} data-testid="sm-add-path">
              {SM_ADD_PATH}
            </Button>
            {s.pathsAtCap && (
              <p role="status" style={{ margin: 0 }} data-testid="sm-paths-at-cap">
                {SM_CAPS.paths}
              </p>
            )}
          </section>

          <section data-testid="sm-covariances">
            <h4 style={{ margin: '4px 0' }}>{SM_COVARIANCES_HEADING}</h4>
            {s.covariances.length === 0 && (
              <p style={{ margin: 0 }} data-testid="sm-no-covariance">
                {SM_NO_COVARIANCE}
              </p>
            )}
            {s.covariances.map((c, i) => (
              <div key={c.id} data-testid={`sm-cov-${c.id}`}>
                <EndSelect value={c.a} options={covarianceOptions} testId={`sm-cov-a-${c.id}`} onChange={(k) => s.setCovarianceEnd(c.id, 'a', k)} /> ↔{' '}
                <EndSelect value={c.b} options={covarianceOptions} testId={`sm-cov-b-${c.id}`} onChange={(k) => s.setCovarianceEnd(c.id, 'b', k)} />{' '}
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} covariance ${i + 1}`} onClick={() => s.removeCovariance(c.id)} data-testid={`sm-cov-remove-${c.id}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
                {c.line !== null && (
                  <p style={{ margin: 0 }} data-testid={`sm-cov-line-${c.id}`}>
                    {c.line}
                  </p>
                )}
                {c.itself && (
                  <p style={{ margin: 0 }} data-testid={`sm-cov-itself-${c.id}`}>
                    {SM_COVARIANCE_WITH_ITSELF}
                  </p>
                )}
                {c.repeats !== null && (
                  <p style={{ margin: 0 }} data-testid={`sm-cov-repeated-${c.id}`}>
                    {SM_COVARIANCE_REPEATED}
                  </p>
                )}
              </div>
            ))}
            <Button variant="secondary" disabled={s.covariancesAtCap} onClick={s.addCovariance} data-testid="sm-add-covariance">
              {SM_ADD_COVARIANCE}
            </Button>{' '}
            <Button
              variant="secondary"
              disabled={s.exogenousToAdd === 0 || s.exogenousTotal > MAX_COVARIANCES}
              onClick={s.addExogenousPairs}
              data-testid="sm-cov-exogenous"
            >
              {SM_ADD_EXOGENOUS}
            </Button>
            {s.covariancesAtCap && (
              <p role="status" style={{ margin: 0 }} data-testid="sm-covariances-at-cap">
                {SM_CAPS.covariances}
              </p>
            )}
            {s.exogenousToAdd > 0 && s.exogenousTotal > MAX_COVARIANCES && (
              <p role="status" style={{ margin: 0 }} data-testid="sm-cov-exogenous-cap">
                {SM_CAPS.exogenous(s.exogenousTotal)}
              </p>
            )}
          </section>

          <section data-testid="sm-equal">
            <h4 style={{ margin: '4px 0' }}>{SM_EQUAL_HEADING}</h4>
            {s.groups.length === 0 && (
              <p style={{ margin: 0 }} data-testid="sm-no-group">
                {SM_NO_GROUP}
              </p>
            )}
            {s.groups.map((g) => (
              <fieldset key={g.id} data-testid={`sm-group-${g.id}`}>
                <legend>{semGroupHeading(g.number)}</legend>
                <select value={g.kind} data-testid={`sm-group-kind-${g.id}`} onChange={(e) => s.setGroupKind(g.id, e.target.value as SemKind)}>
                  {(Object.keys(SM_KINDS) as SemKind[]).map((k) => (
                    <option key={k} value={k}>
                      {SM_KINDS[k]}
                    </option>
                  ))}
                </select>{' '}
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} group ${g.number}`} onClick={() => s.removeGroup(g.id)} data-testid={`sm-group-remove-${g.id}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
                {g.offered.map((p, n) => {
                  const on = g.ticked.includes(p.key);
                  const elsewhere = s.heldBy[p.key] !== undefined && s.heldBy[p.key] !== g.id;
                  return (
                    <label key={p.key} style={{ display: 'block' }}>
                      <input
                        type="checkbox"
                        checked={on}
                        disabled={elsewhere || (!on && s.equalAtCap)}
                        data-testid={`sm-group-member-${g.id}-${n + 1}`}
                        onChange={(e) => s.tick(g.id, p.key, e.target.checked)}
                      />{' '}
                      {p.label}
                    </label>
                  );
                })}
                {g.ticked.length < 2 && (
                  <p style={{ margin: 0 }} data-testid={`sm-group-short-${g.id}`}>
                    {semGroupShort(g.number, g.ticked.length)}
                  </p>
                )}
              </fieldset>
            ))}
            <Button variant="secondary" onClick={s.addGroup} data-testid="sm-add-group">
              {SM_ADD_GROUP}
            </Button>
            {s.equalAtCap && (
              <p role="status" style={{ margin: 0 }} data-testid="sm-equal-at-cap">
                {SM_CAPS.equal}
              </p>
            )}
          </section>
        </div>
      </div>
      {s.offered.length > 0 && (
        <fieldset data-testid="sm-missing-codes">
          <legend>{SM_CODES_LEGEND}</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input type="checkbox" checked={s.ticked.includes(o.code)} data-testid={`sm-code-${o.code}`} onChange={(e) => s.toggleCode(o.code, e.target.checked)} />{' '}
              {semMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
      {s.stillMissing && (
        <p role="status" data-testid="sm-settings-missing">
          {s.stillMissing}
        </p>
      )}
    </div>
  );
};
