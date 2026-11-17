// The structural equation model's model editor (Research OS, Phase 9C-5, decisions 814 and 815):
// each numeric column out of the model, on its own or in a factor; the factors held by ids and
// numbered by their first column; paths, covariances and groups held equal, each row holding its
// ends by id or alias; the five resolutions; the caps; the codes; readiness in decision 814's
// order; the label never outside text; and the spec, checked by a TypeScript copy of Rust's
// check_spec (stats_sem/mod.rs:1223-1400), itself checked against sem_cases.rs's order case. The
// copy is semSpecRules.ts, which the seeded fuzz files share (9C-5b, decision 823's N9).
import React from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import type { SemParams } from './semTypes';
import { SemSettings, buildSemSpec, semStillMissing, useSemSettings, type SemModel, type SemSettingsState } from './SemSettings';
import SEM_LIMITS from './fixtures/semLimits.json';
import { SM_CAPS, SM_MISSING } from './semWording';
import { datasetViewPrepared } from './fixtures/describeFixtures';
import { checkSemSpec } from './semSpecRules';

afterEach(cleanup);

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

  // Rebuilt by 9C-5b (decision 823's N4): it assigned 130 columns in one act, past maxColumns, so
  // with assign's guard its unticked boxes would be none and its check would pass on nothing. Now
  // 128 columns on their own, their variances in one group, and one complete covariance row in a
  // second group: 129 parameters, so at maxEqualMembers ticks the one left is disabled, and the
  // 129th tick through the settings' handle is refused (N5).
  it('disables every unticked box while the groups hold the most members, with its sentence', () => {
    const n = SEM_LIMITS.maxColumns;
    expect(SEM_LIMITS.maxEqualMembers, 'the premise: 128 variances fill the groups').toBe(n);
    render(<Harness v={wide(n)} />);
    act(() => {
      for (let i = 1; i <= n; i++) s().assign(`C${i}`, 'own');
    });
    expect(s().columns).toHaveLength(n);
    addCov('"x1"', '"x2"');
    const newest = () => s().groups.reduce((a, b) => (a.id > b.id ? a : b)).id;
    act(() => s().addGroup());
    const gv = newest();
    act(() => s().setGroupKind(gv, 'variance'));
    act(() => s().addGroup());
    const gc = newest();
    act(() => s().setGroupKind(gc, 'covariance'));
    const group = (id: number) => s().groups.find((x) => x.id === id) as SemSettingsState['groups'][number];
    const keys = group(gv).offered.map((p) => p.key);
    expect(keys).toHaveLength(n);
    expect(group(gc).offered).toHaveLength(1);
    act(() => {
      for (const k of keys) s().tick(gv, k, true);
    });
    expect(group(gv).ticked).toHaveLength(SEM_LIMITS.maxEqualMembers);
    expect(byId('sm-equal-at-cap').textContent).toBe(SM_CAPS.equal);
    const left = members(gc);
    expect(left).toHaveLength(1);
    expect(left[0].ticked).toBe(false);
    expect(left[0].disabled).toBe(true);
    expect(members(gv).every((b) => b.ticked && !b.disabled)).toBe(true);
    // N5: the 129th tick through the settings' handle is refused (SemSettings.tsx's tick cap).
    act(() => s().tick(gc, group(gc).offered[0].key, true));
    expect(group(gc).ticked).toEqual([]);
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

describe("the review of 365b638's findings in the editor (9C-5b, decision 823)", () => {
  it('N3: "Add every pair of exogenous variables" called twice in one act adds each pair once', () => {
    render(<Harness />);
    for (const a of ['C5', 'C6', 'C7']) put(a, 'On its own');
    expect(s().exogenousToAdd).toBe(3);
    act(() => {
      s().addExogenousPairs();
      s().addExogenousPairs();
    });
    expect(s().covariances).toHaveLength(3);
    expect(s().covariances.every((c) => c.complete && c.repeats === null)).toBe(true);
  });

  it('N4: assign refuses a column that would pass the most columns, the model unchanged', () => {
    const n = SEM_LIMITS.maxColumns;
    render(<Harness v={wide(n + 1)} />);
    put('C1', 'A new factor');
    act(() => {
      for (let i = 2; i <= n; i++) s().assign(`C${i}`, i === 2 ? s().factors[0].id : 'own');
    });
    expect(s().columns).toHaveLength(n);
    const factor = s().factors[0].id;
    const before = JSON.stringify(s().spec(1));
    for (const t of ['own', 'new', factor] as const) {
      act(() => s().assign(`C${n + 1}`, t));
      expect(s().placeOf(`C${n + 1}`), String(t)).toBe('none');
    }
    expect(s().columns).toHaveLength(n);
    expect(JSON.stringify(s().spec(1))).toBe(before);
  });

  it('N4: assign refuses a choice that would pass the most factors, the model unchanged', () => {
    const k = SEM_LIMITS.maxFactors;
    render(<Harness v={wide(k + 2)} />);
    act(() => {
      for (let i = 1; i <= k; i++) s().assign(`C${i}`, 'new');
    });
    expect(s().factors).toHaveLength(k);
    act(() => s().assign(`C${k + 1}`, 'new'));
    expect(s().placeOf(`C${k + 1}`)).toBe('none');
    // A column already in a factor of two, moved to a new factor: refused, it stays.
    const first = s().factors[0].id;
    act(() => s().assign(`C${k + 2}`, first));
    act(() => s().assign(`C${k + 2}`, 'new'));
    expect(s().placeOf(`C${k + 2}`)).toBe(first);
    expect(s().factors).toHaveLength(k);
  });

  it("N4: readiness, as a pure helper of the model and the view, says what passes each cap", () => {
    const n = SEM_LIMITS.maxColumns;
    // count columns on their own and, past 1, the path C1 → C2, so the model is not empty.
    const own = (count: number): SemModel => ({
      assigned: Object.fromEntries(Array.from({ length: count }, (_, i) => [`C${i + 1}`, 'own' as const])),
      paths: count > 1 ? [{ id: 1, first: { column: 'C1' }, second: { column: 'C2' } }] : [],
      covariances: [],
      groups: [],
      next: 2,
    });
    expect(semStillMissing(own(n + 1), wide(n + 1))).toBe(SM_MISSING.tooManyColumns(n + 1));
    expect(SM_MISSING.tooManyColumns(n + 1)).toBe(`The model has ${n + 1} columns, more than the ${n} Gaply fits. Choose "Not in the model" for 1 of them.`);
    expect(SM_MISSING.tooManyColumns(n + 3)).toBe(`The model has ${n + 3} columns, more than the ${n} Gaply fits. Choose "Not in the model" for 3 of them.`);
    const k = SEM_LIMITS.maxFactors;
    const factors: SemModel = { ...own(0), assigned: Object.fromEntries(Array.from({ length: 2 * (k + 1) }, (_, i) => [`C${i + 1}`, Math.floor(i / 2) + 1])), next: k + 2 };
    expect(semStillMissing(factors, wide(2 * (k + 1)))).toBe(SM_MISSING.tooManyFactors(k + 1));
    expect(SM_MISSING.tooManyFactors(k + 1)).toBe(`The model has ${k + 1} factors, more than the ${k} Gaply fits. Move every column of 1 of them to another factor or out of the model.`);
    // Within the caps the helper agrees with the screen's sentence.
    expect(semStillMissing(own(0), wide(3))).toBe(SM_MISSING.empty);
    expect(semStillMissing(own(n), wide(n))).toBe(SM_MISSING.alone('"x3"'));
    expect(semStillMissing(own(2), wide(2))).toBeNull();
  });

  it('E3: a model past the paths\', the covariances\' or the members\' cap is not ready, and says what to do (decision 848)', () => {
    // n columns on their own, `paths` and `covariances` as rows of distinct pairs, `groups` as given.
    const pairsOf = (n: number) => {
      const out: Array<[string, string]> = [];
      for (let i = 1; i <= n; i++) for (let j = 1; j <= n; j++) if (i !== j) out.push([`C${i}`, `C${j}`]);
      return out;
    };
    const model = (n: number, paths: number, covariances: number, groups: SemModel['groups'] = []): SemModel => ({
      assigned: Object.fromEntries(Array.from({ length: n }, (_, i) => [`C${i + 1}`, 'own' as const])),
      paths: pairsOf(n).slice(0, paths).map(([a, b], i) => ({ id: i + 1, first: { column: a }, second: { column: b } })),
      covariances: pairsOf(n).filter(([a, b]) => Number(a.slice(1)) < Number(b.slice(1))).slice(0, covariances).map(([a, b], i) => ({ id: 1000 + i, first: { column: a }, second: { column: b } })),
      groups,
      next: 5000,
    });
    const P = SEM_LIMITS.maxPaths;
    const K = SEM_LIMITS.maxCovariances;
    const M = SEM_LIMITS.maxEqualMembers;
    expect(semStillMissing(model(12, P + 1, 0), wide(12))).toBe(`The model has ${P + 1} paths, more than the ${P} Gaply fits. Take out 1 of them.`);
    expect(semStillMissing(model(12, P + 3, 0), wide(12))).toBe(`The model has ${P + 3} paths, more than the ${P} Gaply fits. Take out 3 of them.`);
    expect(semStillMissing(model(24, 0, K + 1), wide(24))).toBe(`The model has ${K + 1} covariances, more than the ${K} Gaply fits. Take out 1 of them.`);
    expect(semStillMissing(model(24, 0, K + 2), wide(24))).toBe(`The model has ${K + 2} covariances, more than the ${K} Gaply fits. Take out 2 of them.`);
    // 20 columns: their 20 variances and 109 of their 190 covariances held equal, 129 members.
    const held = (extra: number): SemModel['groups'] => [
      { id: 1, kind: 'covariance', members: Array.from({ length: M - 19 + extra }, (_, i) => `C:${1000 + i}`) },
      { id: 2, kind: 'variance', members: Array.from({ length: 20 }, (_, i) => `V:C${i + 1}`) },
    ];
    expect(semStillMissing(model(20, 0, 190, held(0)), wide(20))).toBe(`The groups hold ${M + 1} parameters, more than the ${M} Gaply holds equal. Untick 1 of them.`);
    expect(semStillMissing(model(20, 0, 190, held(2)), wide(20))).toBe(`The groups hold ${M + 3} parameters, more than the ${M} Gaply holds equal. Untick 3 of them.`);
    expect([SM_MISSING.tooManyPaths(P + 1), SM_MISSING.tooManyCovariances(K + 1), SM_MISSING.tooManyMembers(M + 1)]).toEqual([
      `The model has ${P + 1} paths, more than the ${P} Gaply fits. Take out 1 of them.`,
      `The model has ${K + 1} covariances, more than the ${K} Gaply fits. Take out 1 of them.`,
      `The groups hold ${M + 1} parameters, more than the ${M} Gaply holds equal. Untick 1 of them.`,
    ]);
    // At each cap exactly, none of the three is said.
    expect(semStillMissing(model(12, P, 0), wide(12))).toBeNull();
    expect(semStillMissing(model(24, 0, K), wide(24))).toBeNull();
  });

  it('E6: a path held in a group and then taken out leaves its group, so the group is short and the model not ready (decision 848)', () => {
    // The fuzz reaches this fault at 1 of 20 fresh seeds at most (9c5f/rates/table.txt), so it is
    // pinned here by its own sequence: two paths held equal, the first taken out, its freed column
    // put "Not in the model".
    render(<Harness v={wide(5)} />);
    act(() => {
      for (const a of ['C1', 'C2', 'C3']) s().assign(a, 'own');
    });
    act(() => {
      s().addPath();
      s().addPath();
    });
    const [p1, p2] = s().paths.map((p) => p.id);
    act(() => {
      s().setPathEnd(p1, 'from', 'C1');
      s().setPathEnd(p1, 'to', 'C2');
      s().setPathEnd(p2, 'from', 'C1');
      s().setPathEnd(p2, 'to', 'C3');
    });
    act(() => s().addGroup());
    const g = s().groups[0].id;
    act(() => s().setGroupKind(g, 'path'));
    for (const p of s().groups[0].offered) act(() => s().tick(g, p.key, true));
    expect(s().groups[0].ticked).toHaveLength(2);
    expect(s().stillMissing).toBeNull();
    act(() => s().removePath(p1));
    act(() => s().assign('C2', 'none'));
    expect(s().groups[0].ticked).toEqual([`P:${p2}`]);
    expect(s().stillMissing).toBe(SM_MISSING.groupShort(1));
  });

  it('E5: a path row\'s two selects are named "Path i, from" and "Path i, to", and still read "From" and "To" (decision 848)', () => {
    render(<Harness />);
    for (const a of ['C5', 'C6']) put(a, 'On its own');
    act(() => {
      s().addPath();
      s().addPath();
    });
    for (const i of [1, 2]) {
      for (const end of ['from', 'to'] as const) {
        const el = screen.getByRole('combobox', { name: `Path ${i}, ${end}` });
        expect(el.getAttribute('aria-label')).toBe(`Path ${i}, ${end}`);
        expect((el.closest('label') as HTMLElement).textContent?.startsWith(end === 'from' ? 'From' : 'To')).toBe(true);
      }
    }
  });

  it('N5: "Add every pair of exogenous variables" is enabled at exactly the most covariances, and disabled one row past', () => {
    render(<Harness v={wide(23)} />);
    act(() => {
      for (let i = 1; i <= 23; i++) s().assign(`C${i}`, 'own');
    });
    act(() => {
      for (let i = 0; i < 3; i++) s().addCovariance();
    });
    // 23 exogenous variables: 253 pairs, and three rows left at "Choose a variable": 256.
    expect([s().exogenousToAdd, s().exogenousTotal]).toEqual([253, SEM_LIMITS.maxCovariances]);
    expect((byId('sm-cov-exogenous') as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByTestId('sm-cov-exogenous-cap')).toBeNull();
    act(() => s().addCovariance());
    expect((byId('sm-cov-exogenous') as HTMLButtonElement).disabled).toBe(true);
    expect(byId('sm-cov-exogenous-cap').textContent).toBe(SM_CAPS.exogenous(SEM_LIMITS.maxCovariances + 1));
    // E1 (decision 848): back at exactly the most, clicking it adds every pair, 256 rows in all,
    // and leaves it disabled, with nothing left to add.
    act(() => s().removeCovariance(s().covariances[s().covariances.length - 1].id));
    expect([s().exogenousToAdd, s().exogenousTotal]).toEqual([253, SEM_LIMITS.maxCovariances]);
    fireEvent.click(byId('sm-cov-exogenous'));
    expect(screen.getAllByTestId(/^sm-cov-\d+$/)).toHaveLength(SEM_LIMITS.maxCovariances);
    expect(s().covariances).toHaveLength(SEM_LIMITS.maxCovariances);
    expect((byId('sm-cov-exogenous') as HTMLButtonElement).disabled).toBe(true);
  });

  it('N5: a group with members is numbered before an empty one made earlier', () => {
    render(<Harness />);
    for (const a of ['C5', 'C6']) put(a, 'On its own');
    const empty = addGroup('Variances');
    const full = addGroup('Variances');
    tickLabel(full, '"q4"');
    tickLabel(full, '"q5"');
    expect(within(byId(`sm-group-${full}`)).getByText('Group 1')).toBeTruthy();
    expect(within(byId(`sm-group-${empty}`)).getByText('Group 2')).toBeTruthy();
  });

  it('N5: a path joining a variable to itself is never offered in a Paths group', () => {
    render(<Harness />);
    for (const a of ['C5', 'C6']) put(a, 'On its own');
    addPath('"q4"', '"q4"');
    addPath('"q4"', '"q5"');
    const g = addGroup('Paths');
    expect(members(g).map((m) => m.label)).toEqual(['"q5" ← "q4"']);
  });

  it("N7: a covariance row's two selects and a group's kind select have accessible names, with no column name in them", () => {
    render(<Harness />);
    for (const a of ['C5', 'C6']) put(a, 'On its own');
    addCov('"q4"', '"q5"');
    const c = addCov('"q5"', '');
    const g = addGroup('Variances');
    expect(screen.getByRole('combobox', { name: 'Covariance 2, first end' })).toBe(byId(`sm-cov-a-${c}`));
    expect(screen.getByRole('combobox', { name: 'Covariance 2, second end' })).toBe(byId(`sm-cov-b-${c}`));
    expect(screen.getByRole('combobox', { name: 'Covariance 1, first end' })).toBeTruthy();
    expect(screen.getByRole('combobox', { name: 'Group 1, kind' })).toBe(byId(`sm-group-kind-${g}`));
  });
});
