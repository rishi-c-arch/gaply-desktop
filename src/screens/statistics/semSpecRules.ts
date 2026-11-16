// A TypeScript copy of Rust's stats_sem::check_spec, in its order (Research OS, Phase 9C-5,
// decision 814; moved here by 9C-5b, decision 823's N9), for the tests only: SemSettings.vitest.tsx
// and the two seeded fuzz files import it. The app never imports this module, so nothing a user
// reaches depends on it; it is a test oracle written from the Rust, checked against sem_cases.rs's
// order case in SemSettings.vitest.tsx. Below it, the seeded fuzz's driver, which both fuzz files
// share (decision 823's N9); it renders the screen, so this module imports the testing library.
import React from 'react';
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { SemParameter, SemParams, SemVariable } from './semTypes';
import { SemSettings, compareParameters, compareVariables, useSemSettings, type SemSettingsState } from './SemSettings';
import SEM_LIMITS from './fixtures/semLimits.json';

type V = SemVariable;
const vEq = (x: V, y: V) => compareVariables(x, y) === 0;
const pEq = (x: SemParameter, y: SemParameter) => compareParameters(x, y) === 0;
const num = (a: string) => Number(a.slice(1));
/** stats_sem::check_spec's codes, in its order (stats_sem/mod.rs:1223-1400), with the columns'
 *  count (:1420-1438). Each family reports every error it finds. */
export function checkSemSpec(p: SemParams): string[] {
  const out: string[] = [];
  const k = p.indicators.length;
  if (k === 0 && p.paths.length === 0 && p.covariances.length === 0) out.push('empty_model');
  if (k > SEM_LIMITS.maxFactors) out.push('too_many_factors');
  p.indicators.forEach((f) => f.length < SEM_LIMITS.minFactorItems && out.push('factor_too_few_items'));
  for (const f of p.indicators) for (let i = 1; i < f.length; i++) if (num(f[i - 1]) > num(f[i])) out.push('items_out_of_order');
  for (let i = 1; i < k; i++) if (p.indicators[i - 1][0] !== undefined && p.indicators[i][0] !== undefined && num(p.indicators[i - 1][0]) > num(p.indicators[i][0])) out.push('factors_out_of_order');
  const seen = new Set<string>();
  for (const a of p.indicators.flat()) if (seen.has(a) || !seen.add(a)) out.push('item_repeated');
  const indicator = new Set(p.indicators.flat());
  const known = (v: V) => ('factor' in v ? v.factor >= 1 && v.factor <= k : true);
  if (p.paths.length > SEM_LIMITS.maxPaths) out.push('too_many_paths');
  p.paths.forEach((x, i) => {
    if (!known(x.to) || !known(x.from)) out.push('unknown_factor');
    if (vEq(x.to, x.from)) out.push('path_to_itself');
    for (const e of [x.to, x.from]) if ('column' in e && indicator.has(e.column)) out.push('indicator_in_path');
    const before = p.paths[i - 1];
    if (i > 0 && (compareVariables(before.to, x.to) || compareVariables(before.from, x.from)) > 0) out.push('paths_out_of_order');
    if (p.paths.slice(0, i).some((y) => vEq(y.to, x.to) && vEq(y.from, x.from))) out.push('path_repeated');
  });
  if (p.covariances.length > SEM_LIMITS.maxCovariances) out.push('too_many_covariances');
  p.covariances.forEach((c, i) => {
    if (!known(c.a) || !known(c.b)) out.push('unknown_factor');
    if (vEq(c.a, c.b)) out.push('covariance_with_itself');
    const before = p.covariances[i - 1];
    if (compareVariables(c.a, c.b) > 0 || (i > 0 && (compareVariables(before.a, c.a) || compareVariables(before.b, c.b)) > 0)) out.push('covariances_out_of_order');
    if (p.covariances.slice(0, i).some((y) => vEq(y.a, c.a) && vEq(y.b, c.b))) out.push('covariance_repeated');
  });
  if (p.equal.reduce((n, g) => n + g.length, 0) > SEM_LIMITS.maxEqualMembers) out.push('too_many_equal_members');
  const columns = new Set([...Array.from(indicator), ...[...p.paths.flatMap((x) => [x.to, x.from]), ...p.covariances.flatMap((x) => [x.a, x.b])].flatMap((v) => ('column' in v ? [v.column] : []))]);
  const hasVariable = (v: V) => ('factor' in v ? v.factor >= 1 && v.factor <= k : columns.has(v.column));
  const kind = (m: SemParameter) => ('loading' in m ? 0 : 'path' in m ? 1 : 'covariance' in m ? 2 : 3);
  const held: SemParameter[] = [];
  p.equal.forEach((g, gi) => {
    if (g.length < 2) out.push('equal_group_too_small');
    if (g.some((m, i) => i > 0 && kind(g[i - 1]) !== kind(m))) out.push('equal_mixed_kinds');
    for (const m of g) {
      if ('loading' in m) {
        const f = m.loading.factor >= 1 && m.loading.factor <= k ? p.indicators[m.loading.factor - 1] : null;
        if (f && f[0] === m.loading.column) out.push('equal_fixed_parameter');
        else if (!(f && f.includes(m.loading.column))) out.push('equal_unknown_parameter');
      } else if ('path' in m) {
        if (!p.paths.some((x) => vEq(x.to, m.path.to) && vEq(x.from, m.path.from))) out.push('equal_unknown_parameter');
      } else if ('covariance' in m) {
        if (!p.covariances.some((x) => vEq(x.a, m.covariance.a) && vEq(x.b, m.covariance.b))) out.push('equal_unknown_parameter');
      } else if (!hasVariable(m.variance.variable)) out.push('equal_unknown_parameter');
    }
    if (gi > 0 && p.equal[gi - 1][0] && g[0] && compareParameters(p.equal[gi - 1][0], g[0]) > 0) out.push('equal_out_of_order');
    g.forEach((m, i) => i > 0 && compareParameters(g[i - 1], m) > 0 && out.push('equal_out_of_order'));
    for (const m of g) {
      if (held.some((h) => pEq(h, m))) out.push('equal_repeated');
      else held.push(m);
    }
  });
  if (columns.size < SEM_LIMITS.minColumns) out.push('too_few_columns');
  else if (columns.size > SEM_LIMITS.maxColumns) out.push('too_many_columns');
  return out;
}

/* ---- the seeded fuzz's driver (9C-5b, decision 823's N9) ------------------------------------ */
//
// On the review of 365b638's design: random user actions through the screen (columns assigned and
// taken out, factors made and emptied, path and covariance rows added, changed and removed, "Add
// every pair of exogenous variables", groups made, ticked, changed and removed, view changes that
// take columns away, and two actions in one act), each state checked: no cap passed, no parameter
// in two groups, no label in an attribute, a test id or an aria-label; and at every ready state the
// spec passes checkSemSpec, names only the view's numeric columns and holds every row and group
// member the screen shows. A failure throws with its seed and its shortest failing sequence, found
// by removing actions one at a time while the sequence still fails. SemSettings.fuzz.vitest.tsx
// runs it at the real limits, SemSettings.fuzzSmall.vitest.tsx at small ones (semFixtures mocked).

/** A seeded generator (mulberry32), so a failing seed replays. */
export function seeded(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Every label holds this, outside ASCII, so a scan finds it wherever it leaks. */
export const FUZZ_SENTINEL = 'LBL_ü';

const fuzzColumn = (alias: string, i: number, over: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias,
  header: `${FUZZ_SENTINEL}${i}_é`,
  kind: 'numeric',
  n: 100,
  missing: 0,
  distinct: 50,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

/** A view of n numeric columns C1 to Cn, some with candidate codes, and one text column after them,
 *  which is never offered; `keep` takes columns away, as a view change does. */
export function fuzzView(n: number, keep?: string[]): DatasetView {
  const all = Array.from({ length: n }, (_, i) =>
    fuzzColumn(`C${i + 1}`, i + 1, i % 3 === 1 ? { suspectMissingCodes: [{ code: -99, cells: 1 + (i % 2) }] } : {})
  );
  const numericColumns = keep ? all.filter((c) => keep.includes(c.alias)) : all;
  const columns = [...numericColumns, fuzzColumn(`C${n + 1}`, n + 1, { kind: 'categorical' })];
  return {
    slot: 1,
    displayName: 'fuzz.csv',
    profile: { rows: 100, columns },
    inputRef: { contentHash: 'cd'.repeat(32), rows: 100, columns: columns.length, semanticRole: 'table' },
  };
}

/** One user action, as data, so a sequence replays and shrinks. */
export type FuzzAction =
  | { t: 'select'; id: string; value: string }
  | { t: 'click'; id: string }
  | { t: 'view'; keep: string[] | null }
  | { t: 'double'; a: HandleAction; b: HandleAction };
/** An action through the settings' handle, for two in one act. */
export type HandleAction =
  | { h: 'assign'; alias: string; target: 'none' | 'own' | 'new' | 'first' }
  | { h: 'exogenous' }
  | { h: 'addPath' }
  | { h: 'addCovariance' }
  | { h: 'addGroup' }
  | { h: 'tickFirstFree' };

export interface FuzzLimits {
  maxColumns: number;
  maxFactors: number;
  maxPaths: number;
  maxCovariances: number;
  maxEqualMembers: number;
}

let fuzzState: SemSettingsState | null = null;
const FuzzHarness: React.FC<{ v: DatasetView }> = ({ v }) => {
  const st = useSemSettings(v);
  fuzzState = st;
  return React.createElement(SemSettings, { view: v, settings: st });
};

function applyHandle(s: SemSettingsState, a: HandleAction): void {
  if (a.h === 'assign') s.assign(a.alias, a.target === 'first' ? (s.factors[0]?.id ?? 'new') : a.target);
  else if (a.h === 'exogenous') s.addExogenousPairs();
  else if (a.h === 'addPath') s.addPath();
  else if (a.h === 'addCovariance') s.addCovariance();
  else if (a.h === 'addGroup') s.addGroup();
  else {
    const g = s.groups.find((x) => x.offered.some((p) => s.heldBy[p.key] === undefined));
    const p = g?.offered.find((x) => s.heldBy[x.key] === undefined);
    if (g && p) s.tick(g.id, p.key, true);
  }
}

/** Applies one action; an action whose control is gone or disabled is skipped, as a user could not
 *  make it. Returns whether it was applied. */
function applyAction(container: HTMLElement, rerender: (v: DatasetView) => void, n: number, a: FuzzAction): boolean {
  if (a.t === 'view') {
    rerender(fuzzView(n, a.keep ?? undefined));
    return true;
  }
  if (a.t === 'double') {
    const s = fuzzState as SemSettingsState;
    act(() => {
      applyHandle(s, a.a);
      applyHandle(s, a.b);
    });
    return true;
  }
  const el = container.querySelector(`[data-testid="${a.id}"]`) as HTMLSelectElement | HTMLInputElement | HTMLButtonElement | null;
  if (!el || el.disabled) return false;
  if (a.t === 'select') {
    const sel = el as HTMLSelectElement;
    if (!Array.from(sel.options).some((o) => o.value === a.value)) return false;
    fireEvent.change(sel, { target: { value: a.value } });
  } else fireEvent.click(el);
  return true;
}

/** A random action from what the screen offers now. */
function randomAction(container: HTMLElement, rnd: () => number, n: number): FuzzAction {
  const pick = <T,>(xs: readonly T[]): T => xs[Math.floor(rnd() * xs.length)];
  const r = rnd();
  if (r < 0.04) {
    const aliases = Array.from({ length: n }, (_, i) => `C${i + 1}`);
    return { t: 'view', keep: rnd() < 0.5 ? null : aliases.filter(() => rnd() < 0.75) };
  }
  if (r < 0.1) {
    const handle = (): HandleAction => {
      const k = rnd();
      if (k < 0.35) return { h: 'assign', alias: `C${1 + Math.floor(rnd() * n)}`, target: pick(['none', 'own', 'new', 'first'] as const) };
      if (k < 0.5) return { h: 'exogenous' };
      if (k < 0.65) return { h: 'addPath' };
      if (k < 0.8) return { h: 'addCovariance' };
      if (k < 0.9) return { h: 'addGroup' };
      return { h: 'tickFirstFree' };
    };
    return { t: 'double', a: handle(), b: handle() };
  }
  const selects = Array.from(container.querySelectorAll('select[data-testid]')) as HTMLSelectElement[];
  const buttons = (Array.from(container.querySelectorAll('button[data-testid]')) as HTMLButtonElement[]).filter((b) => !b.disabled);
  const boxes = (Array.from(container.querySelectorAll('input[type="checkbox"][data-testid]')) as HTMLInputElement[]).filter((b) => !b.disabled);
  const idOf = (el: Element) => el.getAttribute('data-testid') as string;
  // A column's select: "On its own" and the factors made more often than "A new factor" or "Not in
  // the model", so models reach readiness often enough for the spec to be checked.
  const choose = (sel: HTMLSelectElement): FuzzAction => {
    const values = Array.from(sel.options).map((o) => o.value);
    if (!idOf(sel).startsWith('sm-assign-')) return { t: 'select', id: idOf(sel), value: pick(values) };
    const k = rnd();
    const factors = values.filter((v) => /^f\d+$/.test(v));
    const value = k < 0.15 || values.length === 1 ? 'none' : k < 0.5 ? 'own' : k < 0.85 && factors.length > 0 ? pick(factors) : values.includes('new') ? 'new' : 'own';
    return { t: 'select', id: idOf(sel), value: values.includes(value) ? value : pick(values) };
  };
  // Repairs: a factor of one column given a second, an end left at "Choose a variable" given a
  // variable, or a group short of 2 ticked.
  if (r < 0.32) {
    const lone = container.querySelector('[data-testid$="-short"][data-testid^="sm-factor-"]');
    const number = lone ? (lone.getAttribute('data-testid') as string).replace(/^sm-factor-(\d+)-short$/, '$1') : null;
    if (number !== null && rnd() < 0.6) {
      const movable = selects.filter((x) => idOf(x).startsWith('sm-assign-') && (x.value === 'none' || x.value === 'own'));
      const sel = movable.length > 0 ? pick(movable) : null;
      const option = sel ? Array.from(sel.options).find((o) => o.textContent === `Factor ${number}`) : undefined;
      if (sel && option) return { t: 'select', id: idOf(sel), value: option.value };
    }
    const open = selects.filter((x) => x.value === '' && x.options.length > 1);
    if (open.length > 0) {
      const sel = pick(open);
      return { t: 'select', id: idOf(sel), value: pick(Array.from(sel.options).slice(1)).value };
    }
    const short = Array.from(container.querySelectorAll('fieldset[data-testid^="sm-group-"]')).find((g) => g.querySelectorAll('input:checked').length < 2);
    const free = short ? (Array.from(short.querySelectorAll('input[type="checkbox"]')) as HTMLInputElement[]).filter((b) => !b.disabled && !b.checked) : [];
    if (free.length > 0) return { t: 'click', id: idOf(pick(free)) };
  }
  if (r < 0.58 && selects.length > 0) return choose(pick(selects));
  if (r < 0.84 && buttons.length > 0) return { t: 'click', id: idOf(pick(buttons)) };
  if (boxes.length > 0) return { t: 'click', id: idOf(pick(boxes)) };
  return choose(pick(selects));
}

const specColumns = (p: SemParams): string[] => {
  const ofVar = (v: SemVariable) => ('column' in v ? [v.column] : []);
  return [
    ...p.indicators.flat(),
    ...p.paths.flatMap((x) => [...ofVar(x.to), ...ofVar(x.from)]),
    ...p.covariances.flatMap((x) => [...ofVar(x.a), ...ofVar(x.b)]),
    ...p.equal.flat().flatMap((m) =>
      'loading' in m ? [m.loading.column] : 'path' in m ? [...ofVar(m.path.to), ...ofVar(m.path.from)] : 'covariance' in m ? [...ofVar(m.covariance.a), ...ofVar(m.covariance.b)] : ofVar(m.variance.variable)
    ),
  ];
};

/** The state's faults, or null. `ready` counts the ready states checked. */
/** The caps a state sits at, counted so a test can show its cap checks were reached (the review of
 *  this step, finding 1). */
export interface FuzzAtCap {
  columns: number;
  factors: number;
  paths: number;
  covariances: number;
  members: number;
}

function stateFault(
  container: HTMLElement,
  view: DatasetView,
  limits: FuzzLimits,
  tally: { ready: number; missing?: Record<string, number>; atCap?: FuzzAtCap }
): string | null {
  const s = fuzzState as SemSettingsState;
  if (tally.atCap) {
    const c = tally.atCap;
    if (s.columns.length === limits.maxColumns) c.columns += 1;
    if (s.factors.length === limits.maxFactors) c.factors += 1;
    if (s.paths.length === limits.maxPaths) c.paths += 1;
    if (s.covariances.length === limits.maxCovariances) c.covariances += 1;
    if (s.groups.reduce((n, g) => n + g.ticked.length, 0) === limits.maxEqualMembers) c.members += 1;
  }
  if (s.columns.length > limits.maxColumns) return `${s.columns.length} columns, past ${limits.maxColumns}`;
  if (s.factors.length > limits.maxFactors) return `${s.factors.length} factors, past ${limits.maxFactors}`;
  if (s.paths.length > limits.maxPaths) return `${s.paths.length} paths, past ${limits.maxPaths}`;
  if (s.covariances.length > limits.maxCovariances) return `${s.covariances.length} covariances, past ${limits.maxCovariances}`;
  const ticked = s.groups.flatMap((g) => g.ticked);
  if (ticked.length > limits.maxEqualMembers) return `${ticked.length} members, past ${limits.maxEqualMembers}`;
  if (new Set(ticked).size !== ticked.length) return 'a parameter is in two groups';
  for (const el of Array.from(container.querySelectorAll('*'))) {
    for (const attr of Array.from(el.attributes)) if (attr.value.includes(FUZZ_SENTINEL)) return `a label in the attribute ${attr.name}`;
  }
  if (s.stillMissing !== null) {
    if (tally.missing) {
      const k = s.stillMissing.replace(/[0-9]+/g, 'n').split(' ').slice(0, 4).join(' ');
      tally.missing[k] = (tally.missing[k] ?? 0) + 1;
    }
    return null;
  }
  tally.ready += 1;
  const p = s.spec(1).parameters;
  const broken = checkSemSpec(p);
  if (broken.length > 0) return `a ready spec breaks ${broken.join(', ')}`;
  const shown = new Set(view.profile.columns.filter((c) => c.kind === 'numeric').map((c) => c.alias));
  const outside = specColumns(p).filter((a) => !shown.has(a));
  if (outside.length > 0) return `a ready spec names ${outside.join(', ')}, outside the view`;
  if (p.paths.length !== s.paths.length) return `the spec holds ${p.paths.length} of ${s.paths.length} path rows`;
  if (p.covariances.length !== s.covariances.length) return `the spec holds ${p.covariances.length} of ${s.covariances.length} covariance rows`;
  if (p.equal.length !== s.groups.length) return `the spec holds ${p.equal.length} of ${s.groups.length} groups`;
  if (p.equal.flat().length !== ticked.length) return `the spec holds ${p.equal.flat().length} of ${ticked.length} members`;
  if (JSON.stringify(p.indicators) !== JSON.stringify(s.factors.map((f) => f.columns))) return 'the spec\'s indicators are not the factors drawn';
  // Each row by identity, not only by count (the review of this step, finding 6): the screen's ends,
  // a factor by its number, against the spec's.
  const asVar = (key: string): SemVariable => {
    if (!key.startsWith('f')) return { column: key };
    const f = s.factors.find((x) => `f${x.id}` === key);
    return { factor: f ? f.number : 0 };
  };
  const word = (v: SemVariable) => ('factor' in v ? `f${v.factor}` : v.column);
  const shownPaths = s.paths.map((x) => `${word(asVar(x.to))}<${word(asVar(x.from))}`).sort();
  const specPaths = p.paths.map((x) => `${word(x.to)}<${word(x.from)}`).sort();
  if (JSON.stringify(shownPaths) !== JSON.stringify(specPaths)) return `the spec's paths ${specPaths.join(' ')} are not the screen's ${shownPaths.join(' ')}`;
  const pair = (a: SemVariable, b: SemVariable) => (compareVariables(a, b) <= 0 ? `${word(a)}~${word(b)}` : `${word(b)}~${word(a)}`);
  const shownCovs = s.covariances.map((x) => pair(asVar(x.a), asVar(x.b))).sort();
  const specCovs = p.covariances.map((x) => pair(x.a, x.b)).sort();
  if (JSON.stringify(shownCovs) !== JSON.stringify(specCovs)) return `the spec's covariances ${specCovs.join(' ')} are not the screen's ${shownCovs.join(' ')}`;
  return null;
}

/** Runs one sequence from a fresh render; the first fault, with its step, or null. */
function runSequence(actions: FuzzAction[], n: number, limits: FuzzLimits, tally: { ready: number; applied: number }): { step: number; fault: string } | null {
  cleanup();
  let view = fuzzView(n);
  const r = render(React.createElement(FuzzHarness, { v: view }));
  const rerender = (v: DatasetView) => {
    view = v;
    r.rerender(React.createElement(FuzzHarness, { v }));
  };
  try {
    for (let i = 0; i < actions.length; i++) {
      if (applyAction(r.container, rerender, n, actions[i])) tally.applied += 1;
      const fault = stateFault(r.container, view, limits, tally);
      if (fault) return { step: i, fault };
    }
    return null;
  } finally {
    cleanup();
  }
}

/** Removes actions one at a time while the sequence still fails, within a budget of replays. */
function shrink(actions: FuzzAction[], n: number, limits: FuzzLimits): { actions: FuzzAction[]; fault: string } {
  let best = actions;
  let fault = runSequence(best, n, limits, { ready: 0, applied: 0 })?.fault ?? 'did not fail again';
  let budget = 400;
  let changed = true;
  while (changed && budget > 0) {
    changed = false;
    for (let i = 0; i < best.length && budget > 0; i++) {
      const candidate = [...best.slice(0, i), ...best.slice(i + 1)];
      budget -= 1;
      const got = runSequence(candidate, n, limits, { ready: 0, applied: 0 });
      if (got) {
        best = candidate.slice(0, got.step + 1);
        fault = got.fault;
        changed = true;
        i -= 1;
      }
    }
  }
  return { actions: best, fault };
}

/** The fuzz: `sequences` sequences of `steps` random actions each, from `seed`, over a view of `n`
 *  numeric columns, sequences numbered from `first` (each sequence's stream depends only on the seed
 *  and its number, so a run split into chunks replays the same sequences). Throws on the first fault, with its seed, its sequence's index and its shortest
 *  failing sequence. Returns what it ran. */
export function fuzzSemSettings(
  seed: number,
  sequences: number,
  steps: number,
  n: number,
  limits: FuzzLimits,
  first = 0
): { actions: number; applied: number; ready: number; missing: Record<string, number>; atCap: FuzzAtCap } {
  const tally = { ready: 0, applied: 0, missing: {} as Record<string, number>, atCap: { columns: 0, factors: 0, paths: 0, covariances: 0, members: 0 } };
  let actions = 0;
  for (let q = first; q < first + sequences; q++) {
    const rnd = seeded(seed * 100003 + q);
    cleanup();
    let view = fuzzView(n);
    const r = render(React.createElement(FuzzHarness, { v: view }));
    const rerender = (v: DatasetView) => {
      view = v;
      r.rerender(React.createElement(FuzzHarness, { v }));
    };
    const done: FuzzAction[] = [];
    for (let i = 0; i < steps; i++) {
      const a = randomAction(r.container, rnd, n);
      done.push(a);
      actions += 1;
      if (applyAction(r.container, rerender, n, a)) tally.applied += 1;
      const fault = stateFault(r.container, view, limits, tally);
      if (fault) {
        cleanup();
        const small = shrink(done, n, limits);
        throw new Error(
          `the SEM settings fuzz failed at seed ${seed}, sequence ${q}, step ${i}: ${fault}\n` +
            `shortest failing sequence (${small.actions.length} actions, failing with: ${small.fault}):\n` +
            small.actions.map((x) => JSON.stringify(x)).join('\n')
        );
      }
    }
    cleanup();
  }
  return { actions, applied: tally.applied, ready: tally.ready, missing: tally.missing, atCap: tally.atCap };
}
