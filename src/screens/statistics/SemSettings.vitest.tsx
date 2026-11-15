// The structural equation model's model editor (Research OS, Phase 9C-5, decisions 814 and 815):
// each numeric column out of the model, on its own or in a factor; the factors held by ids and
// numbered by their first column; paths, covariances and groups held equal, each row holding its
// ends by id or alias; the five resolutions; the caps; the codes; readiness in decision 814's
// order; the label never outside text; and the spec, checked by a TypeScript copy of Rust's
// check_spec (stats_sem/mod.rs:1223-1400), itself checked against sem_cases.rs's order case.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import type { SemParameter, SemParams, SemVariable } from './semTypes';
import { SemSettings, buildSemSpec, compareParameters, compareVariables, useSemSettings, type SemSettingsState } from './SemSettings';
import SEM_LIMITS from './fixtures/semLimits.json';
import { SM_CAPS, SM_MISSING } from './semWording';
import { datasetViewPrepared } from './fixtures/describeFixtures';

afterEach(cleanup);

/* ---- a TypeScript copy of Rust's check_spec, in its order --------------------- */

type V = SemVariable;
const vEq = (x: V, y: V) => compareVariables(x, y) === 0;
const pEq = (x: SemParameter, y: SemParameter) => compareParameters(x, y) === 0;
const num = (a: string) => Number(a.slice(1));
/** stats_sem::check_spec's codes, in its order (stats_sem/mod.rs:1223-1400), with the columns'
 *  count (:1420-1438). Each family reports every error it finds. */
function checkSemSpec(p: SemParams): string[] {
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

const col = (alias: string, header: string, over: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias,
  header,
  kind: 'numeric',
  n: 100,
  missing: 0,
  distinct: 50,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

const inputRef = { contentHash: 'ab'.repeat(32), rows: 100, columns: 9, semanticRole: 'table' as const };
const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 100,
    columns: [
      col('C1', 'id', { kind: 'categorical' }),
      col('C2', 'q1', { suspectMissingCodes: [{ code: -99, cells: 2 }] }),
      col('C3', 'q2', { suspectMissingCodes: [{ code: 99, cells: 1 }, { code: -99, cells: 1 }] }),
      col('C4', 'q3', { suspectMissingCodes: [{ code: 77, cells: 4 }] }),
      col('C5', 'q4'),
      col('C6', 'q5'),
      col('C7', 'q6'),
      col('C8', 'C8', { prepared: true, scoreName: 'wellbeing' }),
      col('C9', 'q7'),
    ],
  },
  inputRef,
};
/** A view of n numeric columns, C1 to Cn. */
const wide = (n: number): DatasetView => ({ ...view, profile: { rows: 100, columns: Array.from({ length: n }, (_, i) => col(`C${i + 1}`, `x${i + 1}`)) } });

let state: SemSettingsState | null = null;
const s = () => state as SemSettingsState;
const Harness: React.FC<{ v?: DatasetView }> = ({ v = view }) => {
  const st = useSemSettings(v);
  state = st;
  return <SemSettings view={v} settings={st} />;
};

const byId = (id: string) => screen.getByTestId(id);
const selectOf = (id: string) => byId(id) as HTMLSelectElement;
const optionTexts = (id: string) => Array.from(selectOf(id).options).map((o) => o.textContent);
const chosenText = (id: string) => selectOf(id).selectedOptions[0]?.textContent;
/** Chooses the option with this text in a select. */
const pick = (id: string, text: string) => {
  const o = Array.from(selectOf(id).options).find((x) => x.textContent === text);
  if (!o) throw new Error(`no option ${text} in ${id}: ${optionTexts(id).join(' | ')}`);
  fireEvent.change(selectOf(id), { target: { value: o.value } });
};
const put = (alias: string, text: string) => pick(`sm-assign-${alias}`, text);
const lastPath = () => s().paths[s().paths.length - 1].id;
const lastCov = () => s().covariances[s().covariances.length - 1].id;
const addPath = (from: string, to: string) => {
  fireEvent.click(byId('sm-add-path'));
  const id = lastPath();
  if (from) pick(`sm-path-from-${id}`, from);
  if (to) pick(`sm-path-to-${id}`, to);
  return id;
};
const addCov = (a: string, b: string) => {
  fireEvent.click(byId('sm-add-covariance'));
  const id = lastCov();
  if (a) pick(`sm-cov-a-${id}`, a);
  if (b) pick(`sm-cov-b-${id}`, b);
  return id;
};
const addGroup = (kind: string) => {
  fireEvent.click(byId('sm-add-group'));
  const id = s().groups.reduce((a, b) => (a.id > b.id ? a : b)).id;
  pick(`sm-group-kind-${id}`, kind);
  return id;
};
/** A group's offered labels, and which are ticked or disabled. */
const members = (gid: number) =>
  within(byId(`sm-group-${gid}`))
    .queryAllByRole('checkbox')
    .map((c) => ({ label: c.parentElement?.textContent?.trim(), ticked: (c as HTMLInputElement).checked, disabled: (c as HTMLInputElement).disabled, el: c }));
const tickLabel = (gid: number, label: string) => {
  const m = members(gid).find((x) => x.label === label);
  if (!m) throw new Error(`no ${label} in group ${gid}: ${members(gid).map((x) => x.label).join(' | ')}`);
  fireEvent.click(m.el);
};
/** The right pane's factors: each heading and its columns, as drawn. */
const factorsPane = () =>
  screen.queryAllByTestId(/^sm-factor-\d+$/).map((f) => [
    f.querySelector('h5')?.textContent,
    within(f)
      .queryAllByRole('listitem')
      .map((li) => li.getAttribute('data-testid')?.replace(/^sm-factor-\d+-/, '')),
  ]);
const missing = () => screen.queryByTestId('sm-settings-missing')?.textContent ?? null;
const specRules = () => checkSemSpec(s().spec(1).parameters);

describe('the TypeScript copy of check_spec (stats_sem/mod.rs:1223-1400)', () => {
  it("gives sem_cases.rs's order case its codes, in order, the spec's own (the_spec_checks_come_in_decision_797s_order_and_validate_lists_every_one)", () => {
    const p: SemParams = {
      indicators: [['C4', 'C2'], ['C1'], ['C3', 'C4', 'C9']],
      paths: [
        { to: { factor: 9 }, from: { column: 'C5' } },
        { to: { column: 'C6' }, from: { column: 'C6' } },
        { to: { column: 'C7' }, from: { column: 'C3' } },
        { to: { column: 'C7' }, from: { column: 'C5' } },
        { to: { column: 'C7' }, from: { column: 'C5' } },
        { to: { column: 'C6' }, from: { column: 'C5' } },
      ],
      covariances: [
        { a: { factor: 9 }, b: { column: 'C5' } },
        { a: { column: 'C5' }, b: { column: 'C5' } },
        { a: { column: 'C7' }, b: { column: 'C5' } },
        { a: { column: 'C7' }, b: { column: 'C5' } },
      ],
      equal: [
        [{ variance: { variable: { column: 'C5' } } }],
        [{ path: { to: { column: 'C7' }, from: { column: 'C5' } } }, { variance: { variable: { column: 'C7' } } }],
        [{ loading: { factor: 1, column: 'C4' } }, { loading: { factor: 3, column: 'C8' } }],
        [{ variance: { variable: { column: 'C6' } } }, { variance: { variable: { column: 'C5' } } }, { variance: { variable: { column: 'C6' } } }],
      ],
      missingCodes: [],
    };
    // sem_cases.rs's want list less the missing code's and the columns' codes, which need data.
    expect(checkSemSpec(p)).toEqual([
      'factor_too_few_items', 'items_out_of_order', 'factors_out_of_order', 'item_repeated',
      'unknown_factor', 'path_to_itself', 'indicator_in_path', 'path_repeated', 'paths_out_of_order',
      'unknown_factor', 'covariance_with_itself', 'covariances_out_of_order', 'covariances_out_of_order', 'covariance_repeated',
      'equal_group_too_small', 'equal_mixed_kinds', 'equal_out_of_order', 'equal_fixed_parameter', 'equal_unknown_parameter',
      'equal_out_of_order', 'equal_out_of_order', 'equal_repeated', 'equal_repeated',
    ]);
  });

  it('refuses an empty model and one column, and accepts Wheaton\'s Model C as sem_cases.rs writes it', () => {
    expect(checkSemSpec({ indicators: [], paths: [], covariances: [], equal: [], missingCodes: [] })).toEqual(['empty_model', 'too_few_columns']);
    expect(checkSemSpec({ indicators: [['C1', 'C2'], ['C3', 'C4'], ['C5', 'C6']],
      paths: [{ to: { factor: 1 }, from: { factor: 3 } }, { to: { factor: 2 }, from: { factor: 1 } }, { to: { factor: 2 }, from: { factor: 3 } }],
      covariances: [{ a: { column: 'C1' }, b: { column: 'C3' } }],
      equal: [[{ loading: { factor: 1, column: 'C2' } }, { loading: { factor: 2, column: 'C4' } }], [{ variance: { variable: { column: 'C1' } } }, { variance: { variable: { column: 'C3' } } }], [{ variance: { variable: { column: 'C2' } } }, { variance: { variable: { column: 'C4' } } }]],
      missingCodes: [] })).toEqual([]);
  });
});

describe('the columns, chosen once in the left pane', () => {
  it('offers "Not in the model", "On its own", each factor and "A new factor", numeric columns only, scores included', () => {
    render(<Harness />);
    expect(screen.queryByTestId('sm-assign-C1')).toBeNull();
    expect(optionTexts('sm-assign-C8')).toEqual(['Not in the model', 'On its own', 'A new factor']);
    expect(chosenText('sm-assign-C2')).toBe('Not in the model');
    for (const a of ['C2', 'C8']) expect(selectOf(`sm-assign-${a}`).getAttribute('aria-label')).toBeNull();
    expect(screen.getByText('wellbeing')).toBeTruthy();
  });

  it('draws the factors numbered by first column and the columns in the file\'s order, made in a mixed order', () => {
    render(<Harness />);
    put('C6', 'A new factor');
    put('C3', 'A new factor');
    put('C9', 'On its own');
    put('C7', 'Factor 1'); // C3's factor is now Factor 1, C6's Factor 2
    expect(factorsPane()).toEqual([
      ['Factor 1', ['C3', 'C7']],
      ['Factor 2', ['C6']],
    ]);
    put('C2', 'Factor 2'); // C6's factor gains C2, its first: now Factor 1
    expect(factorsPane()).toEqual([
      ['Factor 1', ['C2', 'C6']],
      ['Factor 2', ['C3', 'C7']],
    ]);
    expect(s().variables.map((v) => v.name)).toEqual(['Factor 1', 'Factor 2', '"q7"']);
    expect(screen.getByTestId('sm-factor-1-C2').textContent).toContain('(loading fixed at 1)');
    expect(screen.getByTestId('sm-factor-1-C6').textContent).not.toContain('(loading fixed at 1)');
  });

  it('renumbers at once in the factors, every row and every select when a factor\'s first column leaves', () => {
    render(<Harness />);
    put('C3', 'A new factor');
    put('C6', 'Factor 1');
    put('C4', 'A new factor');
    put('C5', 'Factor 2');
    const p = addPath('Factor 1', 'Factor 2'); // C3's factor → C4's
    const g = addGroup('Paths');
    tickLabel(g, 'Factor 2 ← Factor 1');
    put('C3', 'Not in the model'); // C3's factor is now C6 alone, after C4's: Factor 2
    expect(factorsPane()).toEqual([
      ['Factor 1', ['C4', 'C5']],
      ['Factor 2', ['C6']],
    ]);
    expect(chosenText(`sm-path-from-${p}`)).toBe('Factor 2');
    expect(chosenText(`sm-path-to-${p}`)).toBe('Factor 1');
    expect(optionTexts(`sm-path-from-${p}`)).toEqual(['Choose a variable', 'Factor 1', 'Factor 2']);
    expect(members(g).map((m) => [m.label, m.ticked])).toEqual([['Factor 1 ← Factor 2', true]]);
    expect(optionTexts('sm-assign-C7')).toEqual(['Not in the model', 'On its own', 'Factor 1', 'Factor 2', 'A new factor']);
  });

  it('offers only "Not in the model" to a column not in the model while the model has the most columns', () => {
    render(<Harness v={wide(SEM_LIMITS.maxColumns + 1)} />);
    act(() => {
      for (let i = 1; i <= SEM_LIMITS.maxColumns; i++) s().assign(`C${i}`, 'own');
    });
    expect(byId('sm-at-cap').textContent).toBe(SM_CAPS.columns);
    expect(optionTexts(`sm-assign-C${SEM_LIMITS.maxColumns + 1}`)).toEqual(['Not in the model']);
    expect(optionTexts('sm-assign-C1')).toContain('On its own');
  });
});

describe('the paths', () => {
  it('adds a row, chooses both ends, and removes it; never offers a factor\'s own column', () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C5', 'On its own');
    put('C6', 'On its own');
    expect(byId('sm-no-path').textContent).toBe('No path yet. Add one to give one variable a regression weight on another.');
    const p = addPath('', '');
    expect(chosenText(`sm-path-from-${p}`)).toBe('Choose a variable');
    expect(optionTexts(`sm-path-from-${p}`)).toEqual(['Choose a variable', 'Factor 1', '"q4"', '"q5"']);
    expect(optionTexts(`sm-path-to-${p}`)).toEqual(['Choose a variable', 'Factor 1', '"q4"', '"q5"']);
    expect(missing()).toBe(SM_MISSING.alone('"q4"'));
    pick(`sm-path-from-${p}`, '"q4"');
    pick(`sm-path-to-${p}`, 'Factor 1');
    expect(s().spec(1).parameters.paths).toEqual([{ to: { factor: 1 }, from: { column: 'C5' } }]);
    fireEvent.click(byId(`sm-path-remove-${p}`));
    expect(screen.queryByTestId(`sm-path-remove-${p}`)).toBeNull();
    expect(s().paths).toEqual([]);
    expect(byId(`sm-add-path`).textContent).toBe('Add a path');
  });

  it('says beside a row that joins a variable to itself, or repeats an earlier row, and is not ready', () => {
    render(<Harness />);
    put('C5', 'On its own');
    put('C6', 'On its own');
    const a = addPath('"q4"', '"q4"');
    expect(byId(`sm-path-itself-${a}`).textContent).toBe('A path needs two different variables. Change one of its ends.');
    pick(`sm-path-to-${a}`, '"q5"');
    const b = addPath('"q4"', '"q5"');
    expect(byId(`sm-path-repeated-${b}`).textContent).toBe('This path is already listed. Change one of its ends.');
    expect(missing()).toBe(SM_MISSING.pathRepeats(2, 1));
    fireEvent.click(byId(`sm-path-remove-${b}`));
    expect(missing()).toBeNull();
  });

  it('disables "Add a path" at the most paths, with its sentence', () => {
    render(<Harness />);
    put('C5', 'On its own');
    put('C6', 'On its own');
    act(() => {
      for (let i = 0; i < SEM_LIMITS.maxPaths; i++) s().addPath();
    });
    expect(s().paths).toHaveLength(SEM_LIMITS.maxPaths);
    expect((byId('sm-add-path') as HTMLButtonElement).disabled).toBe(true);
    expect(byId('sm-paths-at-cap').textContent).toBe(SM_CAPS.paths);
    fireEvent.click(byId('sm-add-path'));
    expect(s().paths).toHaveLength(SEM_LIMITS.maxPaths);
  });
});

describe('the covariances', () => {
  it('names each endogenous end as its error term beneath a complete row, recomputed as paths change', () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C5', 'On its own');
    put('C6', 'On its own');
    put('C7', 'On its own');
    // Two exogenous variables: no line.
    const c1 = addCov('"q5"', '"q6"');
    expect(screen.queryByTestId(`sm-cov-line-${c1}`)).toBeNull();
    // A column on its own a path goes to, with a factor's column: both error terms.
    addPath('"q4"', '"q5"');
    expect(byId(`sm-cov-line-${c1}`).textContent).toBe('Between the error term of "q5" and "q6".');
    const c2 = addCov('"q5"', '"q2"');
    expect(byId(`sm-cov-line-${c2}`).textContent).toBe('Between the error terms of "q2" and "q5".');
    // A column on its own that is only a path's from end, with a factor's column: the ends in the
    // model's order (q1 is C2, q4 is C5), the factor's column by its error term alone.
    const c3 = addCov('"q4"', '"q1"');
    expect(byId(`sm-cov-line-${c3}`).textContent).toBe('Between the error term of "q1" and "q4".');
    // An exogenous factor with an endogenous column on its own: only b's error term.
    const c4 = addCov('"q5"', 'Factor 1');
    expect(byId(`sm-cov-line-${c4}`).textContent).toBe('Between Factor 1 and the error term of "q5".');
    // The path into "q5" removed: its lines change.
    fireEvent.click(byId(`sm-path-remove-${s().paths[0].id}`));
    expect(screen.queryByTestId(`sm-cov-line-${c1}`)).toBeNull();
    expect(byId(`sm-cov-line-${c2}`).textContent).toBe('Between the error term of "q2" and "q5".');
    // Every variable in the model is offered, a factor's columns included.
    expect(optionTexts(`sm-cov-a-${c1}`)).toEqual(['Choose a variable', 'Factor 1', '"q1"', '"q2"', '"q4"', '"q5"', '"q6"']);
  });

  it('says beside a row that joins a variable to itself, or repeats one, and is not ready', () => {
    render(<Harness />);
    put('C5', 'On its own');
    put('C6', 'On its own');
    const a = addCov('"q4"', '"q4"');
    expect(byId(`sm-cov-itself-${a}`).textContent).toBe('A covariance needs two different variables. Change one of its ends.');
    pick(`sm-cov-b-${a}`, '"q5"');
    const b = addCov('"q5"', '"q4"');
    expect(byId(`sm-cov-repeated-${b}`).textContent).toBe('This covariance is already listed. Change one of its ends.');
    expect(missing()).toBe(SM_MISSING.covarianceRepeats(2, 1));
  });

  it('"Add every pair of exogenous variables" adds exactly the missing pairs in the model\'s order, then is disabled', () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C5', 'On its own');
    put('C6', 'On its own');
    put('C7', 'On its own');
    addPath('"q5"', '"q6"'); // "q6" is endogenous
    addCov('"q4"', 'Factor 1');
    fireEvent.click(byId('sm-cov-exogenous'));
    const pairs = s().spec(1).parameters.covariances.map((c) => [c.a, c.b].map((v) => ('factor' in v ? `f${v.factor}` : v.column)).join('-'));
    // Exogenous: Factor 1, C5 and C6 (the path "q5" → "q6" makes C7 endogenous); Factor 1 ↔ C5 is listed.
    expect(pairs).toEqual(['f1-C5', 'f1-C6', 'C5-C6']);
    expect((byId('sm-cov-exogenous') as HTMLButtonElement).disabled).toBe(true);
  });

  it('counts a path row as making its To end endogenous only once both ends are chosen and differ (the review of 9C-5, finding 4)', () => {
    render(<Harness />);
    for (const a of ['C5', 'C6']) put(a, 'On its own');
    const p = addPath('', '"q5"');
    expect(s().exogenousToAdd).toBe(1);
    pick(`sm-path-from-${p}`, '"q5"');
    expect(s().exogenousToAdd).toBe(1);
    pick(`sm-path-from-${p}`, '"q4"');
    expect(s().exogenousToAdd).toBe(0);
  });

  it("names a covariance's ends in the model's order in its line and its group label, whatever order they were chosen in (the review of 9C-5, finding 7)", () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C9', 'On its own');
    const c = addCov('"q7"', 'Factor 1');
    expect(screen.queryByTestId(`sm-cov-line-${c}`)).toBeNull();
    const g = addGroup('Covariances');
    expect(members(g).map((m) => m.label)).toEqual(['Factor 1 ↔ "q7"']);
    const d = addCov('"q2"', '"q1"');
    expect(byId(`sm-cov-line-${d}`).textContent).toBe('Between the error terms of "q1" and "q2".');
  });

  it('is disabled with its sentence when the pairs would pass the most covariances', () => {
    render(<Harness v={wide(24)} />);
    act(() => {
      for (let i = 1; i <= 24; i++) s().assign(`C${i}`, 'own');
    });
    // 24 exogenous variables: 276 pairs.
    expect(s().exogenousToAdd).toBe(276);
    expect((byId('sm-cov-exogenous') as HTMLButtonElement).disabled).toBe(true);
    expect(byId('sm-cov-exogenous-cap').textContent).toBe(SM_CAPS.exogenous(276));
  });
});

describe("decision 814's five resolutions", () => {
  it('a column leaving the model: its ends return to "Choose a variable", its members leave their groups', () => {
    render(<Harness />);
    put('C5', 'On its own');
    put('C6', 'On its own');
    put('C7', 'On its own');
    const p = addPath('"q4"', '"q5"');
    const c = addCov('"q4"', '"q6"');
    const g = addGroup('Variances');
    tickLabel(g, '"q4"');
    tickLabel(g, '"q6"');
    put('C5', 'Not in the model');
    expect(chosenText(`sm-path-from-${p}`)).toBe('Choose a variable');
    expect(chosenText(`sm-cov-a-${c}`)).toBe('Choose a variable');
    expect([s().paths[0].from, s().covariances[0].a]).toEqual(['', '']);
    expect(members(g).filter((m) => m.ticked).map((m) => m.label)).toEqual(['"q6"']);
    expect(missing()).toBe(SM_MISSING.pathEnds(1));
  });

  it("a column joining a factor while a path's end: that end returns; a covariance's end stays, its error term", () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C5', 'On its own');
    put('C6', 'On its own');
    const p = addPath('"q4"', '"q5"');
    const c = addCov('"q4"', '"q5"');
    put('C5', 'Factor 1');
    expect(chosenText(`sm-path-from-${p}`)).toBe('Choose a variable');
    expect(chosenText(`sm-cov-a-${c}`)).toBe('"q4"');
    expect([s().paths[0].from, s().covariances[0].a]).toEqual(['', 'C5']);
    expect(missing()).toBe(SM_MISSING.pathEnds(1));
  });

  it('a factor that is gone: every end naming it returns', () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C5', 'On its own');
    const p = addPath('Factor 1', '"q4"');
    const c = addCov('Factor 1', '"q4"');
    put('C2', 'Not in the model');
    expect(chosenText(`sm-path-from-${p}`)).toBe('Choose a variable');
    expect(chosenText(`sm-cov-a-${c}`)).toBe('Choose a variable');
    expect([s().paths[0].from, s().covariances[0].a]).toEqual(['', '']);
  });

  it("a changed or removed row's parameter leaves its group", () => {
    render(<Harness />);
    for (const a of ['C5', 'C6', 'C7', 'C9']) put(a, 'On its own');
    const p1 = addPath('"q4"', '"q5"');
    const p2 = addPath('"q6"', '"q7"');
    const g = addGroup('Paths');
    tickLabel(g, '"q5" ← "q4"');
    tickLabel(g, '"q7" ← "q6"');
    pick(`sm-path-from-${p1}`, '"q6"');
    expect(members(g).filter((m) => m.ticked).map((m) => m.label)).toEqual(['"q7" ← "q6"']);
    tickLabel(g, '"q5" ← "q6"');
    fireEvent.click(byId(`sm-path-remove-${p2}`));
    expect(members(g).filter((m) => m.ticked).map((m) => m.label)).toEqual(['"q5" ← "q6"']);
    expect(byId(`sm-group-short-${g}`).textContent).toBe('Group 1 holds 1 parameter. Tick at least one more.');
  });

  it("a loading that becomes its factor's first leaves its group: the first leaves, or the column moves before another factor's first", () => {
    render(<Harness />);
    put('C3', 'A new factor');
    put('C4', 'Factor 1');
    put('C6', 'A new factor');
    put('C7', 'Factor 2');
    const g = addGroup('Loadings');
    expect(members(g).map((m) => m.label)).toEqual(['"q3" ← Factor 1', '"q6" ← Factor 2']);
    tickLabel(g, '"q3" ← Factor 1');
    tickLabel(g, '"q6" ← Factor 2');
    put('C3', 'Not in the model'); // C4 is now its factor's first
    expect(members(g).map((m) => m.label)).toEqual(['"q6" ← Factor 2']);
    expect(members(g).filter((m) => m.ticked).map((m) => m.label)).toEqual(['"q6" ← Factor 2']);
    // Held, not only drawn: a member no longer offered is gone from the group, so it neither
    // counts toward the cap nor keeps the group from being short (the mutation record m25).
    expect(s().groups[0].ticked).toHaveLength(1);
    put('C5', 'Factor 1');
    tickLabel(g, '"q4" ← Factor 1');
    put('C5', 'Factor 2'); // C5 precedes C6: it is now Factor 2's first, its loading fixed
    expect(members(g).filter((m) => m.ticked).map((m) => m.label)).toEqual(['"q6" ← Factor 2']);
    expect(members(g).map((m) => m.label)).not.toContain('"q4" ← Factor 2');
    expect(s().groups[0].ticked).toHaveLength(1);
    expect(byId(`sm-group-short-${g}`).textContent).toBe('Group 1 holds 1 parameter. Tick at least one more.');
  });
});

describe('the groups held equal', () => {
  it('holds two paths equal; a parameter ticked in one group is disabled in another; a kind change clears the ticks', () => {
    render(<Harness />);
    for (const a of ['C5', 'C6', 'C7']) put(a, 'On its own');
    addPath('"q4"', '"q5"');
    addPath('"q4"', '"q6"');
    const g1 = addGroup('Paths');
    tickLabel(g1, '"q5" ← "q4"');
    tickLabel(g1, '"q6" ← "q4"');
    expect(s().spec(1).parameters.equal).toEqual([[{ path: { to: { column: 'C6' }, from: { column: 'C5' } } }, { path: { to: { column: 'C7' }, from: { column: 'C5' } } }]]);
    const g2 = addGroup('Paths');
    expect(members(g2).every((m) => m.disabled)).toBe(true);
    pick(`sm-group-kind-${g1}`, 'Variances');
    expect(members(g1).filter((m) => m.ticked)).toEqual([]);
    expect(members(g2).every((m) => !m.disabled)).toBe(true);
  });

  it('never offers a first loading, an incomplete row or a repeated one', () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    for (const a of ['C5', 'C6']) put(a, 'On its own');
    addPath('"q4"', '"q5"');
    addPath('"q4"', '"q5"');
    addPath('"q4"', '');
    const loadings = addGroup('Loadings');
    expect(members(loadings).map((m) => m.label)).toEqual(['"q2" ← Factor 1']);
    const paths = addGroup('Paths');
    expect(members(paths).map((m) => m.label)).toEqual(['"q5" ← "q4"']);
  });

  it('disables every unticked box while the groups hold the most members, with its sentence', () => {
    render(<Harness v={wide(SEM_LIMITS.maxEqualMembers + 2)} />);
    act(() => {
      for (let i = 1; i <= SEM_LIMITS.maxEqualMembers + 2; i++) s().assign(`C${i}`, 'own');
    });
    act(() => s().addGroup());
    const g = s().groups[0].id;
    act(() => s().setGroupKind(g, 'variance'));
    const keys = s().groups[0].offered.map((p) => p.key);
    act(() => {
      for (const k of keys.slice(0, SEM_LIMITS.maxEqualMembers)) s().tick(g, k, true);
    });
    expect(s().groups[0].ticked).toHaveLength(SEM_LIMITS.maxEqualMembers);
    expect(byId('sm-equal-at-cap').textContent).toBe(SM_CAPS.equal);
    const boxes = members(g);
    expect(boxes.filter((b) => !b.ticked).every((b) => b.disabled)).toBe(true);
    expect(boxes.filter((b) => b.ticked).every((b) => !b.disabled)).toBe(true);
  });

  it('numbers the groups by their first parameter in Rust\'s order, whatever order they were made in', () => {
    render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C4', 'Factor 1');
    for (const a of ['C5', 'C6', 'C7']) put(a, 'On its own');
    addPath('"q4"', '"q5"');
    addPath('"q4"', '"q6"');
    const variances = addGroup('Variances');
    tickLabel(variances, '"q4"');
    tickLabel(variances, 'Error of "q5"');
    const paths = addGroup('Paths');
    tickLabel(paths, '"q5" ← "q4"');
    tickLabel(paths, '"q6" ← "q4"');
    const loadings = addGroup('Loadings');
    tickLabel(loadings, '"q2" ← Factor 1');
    tickLabel(loadings, '"q3" ← Factor 1');
    expect(within(byId(`sm-group-${loadings}`)).getByText('Group 1')).toBeTruthy();
    expect(within(byId(`sm-group-${paths}`)).getByText('Group 2')).toBeTruthy();
    expect(within(byId(`sm-group-${variances}`)).getByText('Group 3')).toBeTruthy();
    expect(specRules()).toEqual([]);
  });

  it('removes a group with its "Remove"', () => {
    render(<Harness />);
    const g = addGroup('Paths');
    expect(byId(`sm-group-remove-${g}`).textContent).toBe('Remove');
    fireEvent.click(byId(`sm-group-remove-${g}`));
    expect(byId('sm-no-group').textContent).toBe('No parameters are held equal. Add a group to hold two or more equal.');
  });
});

describe('the codes and readiness', () => {
  it('offers the codes of the columns in the model, drops a tick with its last column, and prunes on a view change', () => {
    const { rerender } = render(<Harness />);
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    expect(screen.getByTestId('sm-code--99')).toBeTruthy();
    fireEvent.click(byId('sm-code--99'));
    fireEvent.click(byId('sm-code-99'));
    expect(s().spec(1).parameters.missingCodes).toEqual([-99, 99]);
    put('C3', 'Not in the model');
    expect(s().spec(1).parameters.missingCodes).toEqual([-99]);
    rerender(<Harness v={{ ...view, profile: { ...view.profile, columns: view.profile.columns.filter((c) => c.alias !== 'C2') } }} />);
    expect(s().columns).toEqual([]);
    expect(s().spec(1).parameters.missingCodes).toEqual([]);
  });

  it("says the first thing missing, in decision 814's order", () => {
    render(<Harness />);
    expect(missing()).toBe(SM_MISSING.empty);
    expect(byId('sm-no-factor').textContent).toBe('No factor yet. Choose a factor for a column on the left.');
    expect(byId('sm-no-covariance').textContent).toBe('No covariance yet. Add one to let two variables, or their error terms, covary.');
    put('C2', 'A new factor');
    expect(missing()).toBe(SM_MISSING.columns);
    put('C5', 'On its own');
    expect(missing()).toBe(SM_MISSING.factorShort(1));
    expect(byId('sm-factor-1-short').textContent).toBe('Factor 1 has 1 column; each factor needs at least 2.');
    put('C3', 'Factor 1');
    expect(missing()).toBe(SM_MISSING.alone('"q4"'));
    expect(byId('sm-unused-C5').textContent).toBe('"q4" is on its own with no path or covariance, so it would not be fitted. Add one, or choose "Not in the model" for it.');
    const p = addPath('"q4"', '');
    expect(missing()).toBe(SM_MISSING.pathEnds(1));
    pick(`sm-path-to-${p}`, '"q4"');
    expect(missing()).toBe(SM_MISSING.pathItself(1));
    pick(`sm-path-to-${p}`, 'Factor 1');
    addPath('"q4"', 'Factor 1');
    expect(missing()).toBe(SM_MISSING.pathRepeats(2, 1));
    fireEvent.click(byId(`sm-path-remove-${lastPath()}`));
    const c = addCov('"q4"', '');
    expect(missing()).toBe(SM_MISSING.covarianceEnds(1));
    pick(`sm-cov-b-${c}`, '"q4"');
    expect(missing()).toBe(SM_MISSING.covarianceItself(1));
    pick(`sm-cov-b-${c}`, '"q2"');
    addCov('"q2"', '"q4"');
    expect(missing()).toBe(SM_MISSING.covarianceRepeats(2, 1));
    fireEvent.click(byId(`sm-cov-remove-${lastCov()}`));
    const g = addGroup('Variances');
    expect(missing()).toBe(SM_MISSING.groupShort(1));
    expect(byId(`sm-group-short-${g}`).textContent).toBe('Group 1 holds no parameters. Tick at least 2.');
    tickLabel(g, '"q4"');
    tickLabel(g, 'Error of "q1"');
    expect(missing()).toBeNull();
    expect(specRules()).toEqual([]);
  });
});

describe('the spec (decision 814)', () => {
  it('holds every one of decision 797\'s order rules on models built in a mixed order', () => {
    render(<Harness />);
    put('C7', 'A new factor');
    put('C9', 'Factor 1');
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C6', 'On its own');
    put('C5', 'On its own');
    addPath('"q5"', 'Factor 2'); // C6 → C7's factor
    addPath('"q4"', 'Factor 1'); // C5 → C2's factor
    addPath('Factor 1', 'Factor 2');
    addCov('"q5"', '"q4"');
    addCov('"q7"', 'Factor 1');
    addCov('Factor 2', '"q1"');
    // The variables in the model's order, though C6 was put on its own before C5.
    expect(optionTexts(`sm-path-from-${s().paths[0].id}`)).toEqual(['Choose a variable', 'Factor 1', 'Factor 2', '"q4"', '"q5"']);
    const g1 = addGroup('Covariances');
    for (const m of members(g1)) fireEvent.click(m.el);
    const g2 = addGroup('Paths');
    tickLabel(g2, 'Factor 2 ← "q5"');
    tickLabel(g2, 'Factor 1 ← "q4"');
    fireEvent.click(byId('sm-code-99'));
    fireEvent.click(byId('sm-code--99'));
    const p = s().spec(1).parameters;
    expect(p.indicators).toEqual([['C2', 'C3'], ['C7', 'C9']]);
    expect(p.paths).toEqual([
      { to: { factor: 1 }, from: { column: 'C5' } },
      { to: { factor: 2 }, from: { factor: 1 } },
      { to: { factor: 2 }, from: { column: 'C6' } },
    ]);
    expect(p.covariances).toEqual([
      { a: { factor: 1 }, b: { column: 'C9' } },
      { a: { factor: 2 }, b: { column: 'C2' } },
      { a: { column: 'C5' }, b: { column: 'C6' } },
    ]);
    expect(p.equal.map((g) => g.map((m) => Object.keys(m)[0]))).toEqual([['path', 'path'], ['covariance', 'covariance', 'covariance']]);
    expect(p.missingCodes).toEqual([-99, 99]);
    expect(missing()).toBeNull();
    expect(specRules()).toEqual([]);
  });

  it("carries the view's preparation exactly when it has one", () => {
    const model = { indicators: [['C1', 'C2']], paths: [], covariances: [], equal: [] };
    expect(buildSemSpec(datasetViewPrepared, model, [], 1).preparation).toBe(datasetViewPrepared.preparation);
  });
});

describe("a column's label", () => {
  it('appears only as text, never in a key, an attribute, a test id or an aria-label', () => {
    const sentinel = 'LABEL_SENTINEL_9c5';
    const v: DatasetView = {
      ...view,
      profile: { rows: 100, columns: [col('C1', sentinel, { suspectMissingCodes: [{ code: -99, cells: 1 }] }), col('C2', 'x'), col('C3', 'y'), col('C4', 'z')] },
    };
    const { container } = render(<Harness v={v} />);
    put('C1', 'On its own');
    put('C2', 'A new factor');
    put('C3', 'Factor 1');
    put('C4', 'On its own');
    addPath(`"${sentinel}"`, 'Factor 1');
    addCov(`"${sentinel}"`, '"z"');
    const g = addGroup('Variances');
    tickLabel(g, `"${sentinel}"`);
    expect(container.textContent).toContain(sentinel);
    for (const el of Array.from(container.querySelectorAll('*'))) {
      for (const attr of Array.from(el.attributes)) expect(attr.value, attr.name).not.toContain(sentinel);
    }
  });
});
