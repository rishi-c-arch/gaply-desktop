// The independent-samples t-test's dialog (Research OS, Phase 8S, decisions 481 and
// 483; ADR-012 Part 6), over a mock bridge. The twins of TTestFlow.vitest.tsx's tests
// at :171, :195, :216, :251, :295, :421 and :454. Every assertion on its test ids and
// its spec is that file's. What differs:
// * toSettings becomes the dialog's opening, and tt-check its Run;
// * expectFocusOn('Check the settings') becomes waiting for the spec or the report,
//   and a line on the stage heading becomes "the dialog is still open";
// * tt-error becomes sw-dialog-refusal (:421);
// * :195 checks twice through tt-back-settings, and a dialog starts a run at its first
//   valid press, so it is written as two openings of the dialog.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { DatasetView } from '../describeTypes';
import TTestFlow from '../TTestFlow';
import { makeMockTTestBridge } from '../ttestBridge';
import type { TTestRunStateView, TTestSpec, TTestValidationReportView } from '../ttestTypes';
import { TTestDialog } from './TTestDialog';
import { OUTPUT_FULL } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const INPUT = { contentHash: 'ab'.repeat(32), rows: 10, columns: 3, semanticRole: 'table' as const };

/** "ｚ" (U+FF5A) is before "😀" (U+1F600) in code points and after it in
 *  UTF-16 code units: the profile lists them in code-point order. */
const view: DatasetView = {
  slot: 3,
  displayName: 'trial.csv',
  profile: {
    rows: 10,
    columns: [
      { alias: 'C1', header: 'score', kind: 'numeric', n: 10, missing: 0, distinct: 9, levels: null, suspectMissingCodes: [{ code: -99, cells: 2 }], nonFinite: 0 },
      { alias: 'C2', header: 'arm', kind: 'categorical', n: 10, missing: 0, distinct: 2, levels: [{ label: 'ctrl', count: 5 }, { label: 'trt', count: 5 }], suspectMissingCodes: [], nonFinite: 0 },
      {
        alias: 'C3',
        header: 'site',
        kind: 'categorical',
        n: 10,
        missing: 0,
        distinct: 3,
        levels: [
          { label: 'b', count: 4 },
          { label: 'ｚ', count: 3 },
          { label: '😀', count: 3 },
        ],
        suspectMissingCodes: [],
        nonFinite: 0,
      },
    ],
  },
  inputRef: INPUT,
};

const report = (status: TTestValidationReportView['status'], extra: Partial<TTestValidationReportView> = {}): TTestValidationReportView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 10, columns: 2, cells: 20 },
  ...extra,
});

const running: TTestRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 1200, cancelRequested: false, diagnostics: [] };

function levelsOf(v: DatasetView) {
  return ({ slot, column }: Record<string, unknown>) => {
    const c = v.profile.columns.find((col) => col.alias === column);
    const levels =
      column === 'C4'
        ? [
            { label: '10', count: 3 },
            { label: '9', count: 3 },
          ]
        : (c?.levels ?? []);
    return { slot, column, levels: levels.map((l, index) => ({ index, label: l.label, count: l.count })) };
  };
}

function setup(responses: Record<string, unknown>, v: DatasetView = view, over: { outputFull?: boolean } = {}) {
  const bridge = makeMockTTestBridge({ stats_ttest_group_levels: levelsOf(v), stats_ttest_run: { runId: 5 }, stats_ttest_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<TTestDialog view={v} slot={3} bridge={bridge} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockTTestBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as TTestSpec);
const run = () => screen.getByTestId('tt-run') as HTMLButtonElement;

async function chooseGroup(alias: string) {
  fireEvent.change(screen.getByTestId('tt-group'), { target: { value: alias } });
  await screen.findByTestId('tt-levels');
  await waitFor(() => expect((screen.getByTestId('tt-level-1') as HTMLSelectElement).options.length).toBeGreaterThan(1));
}

describe('the twins of TTestFlow.vitest.tsx', () => {
  it('asks the variant with no default, and sends none until the user chooses', async () => {
    const { bridge, onStarted } = setup({
      stats_ttest_validate: report('needs_user_input', {
        missingInformation: [{ code: 'stats.ttest.variant_not_chosen', params: {}, severity: 'note' }],
      }),
    });
    const radios = within(screen.getByTestId('tt-variant')).getAllByRole('radio') as HTMLInputElement[];
    expect(radios).toHaveLength(2);
    expect(radios.every((r) => !r.checked)).toBe(true);
    expect(screen.getByTestId('tt-variant').textContent).toContain('Gaply does not choose one for you.');
    // Nothing can be run before an outcome and a group are chosen.
    expect(run().disabled).toBe(true);
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(run());
    await screen.findByTestId('tt-validation-questions');
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters.variant).toBeNull();
    expect(screen.getByTestId('tt-validation-questions').textContent).toBe(
      "Choose Student's t-test or Welch's t-test in Settings. Gaply does not choose one for you."
    );
    expect(run().disabled).toBe(true);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('asks about a candidate missing code in the outcome, and applies it only when ticked: not ticked', async () => {
    const { bridge } = setup({ stats_ttest_validate: report('valid') });
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    const q = screen.getByTestId('tt-missing-codes');
    expect(q.textContent).toContain(
      'Is -99 a code for a missing value? It appears in the outcome "score" (2 cells). If you tick it, every row whose outcome is -99 is left out.'
    );
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-student'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_ttest_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters.missingCodes).toEqual([]);
  });

  it('…ticked', async () => {
    const { bridge } = setup({ stats_ttest_validate: report('valid') });
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-student'));
    fireEvent.click(screen.getByTestId('tt-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_ttest_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters.missingCodes).toEqual([-99]);
  });

  it('shows the levels by label in the profile order and sends their positions, where a JavaScript sort would swap them', async () => {
    const labels = view.profile.columns[2].levels!.map((l) => l.label);
    expect([...labels].sort()).not.toEqual(labels);
    const { bridge } = setup({ stats_ttest_validate: report('valid') });
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C3');
    expect(screen.getByTestId('tt-levels-question').textContent).toBe(
      'The group has 3 levels. Choose the two to compare: the first is group 1 and the second is group 2.'
    );
    const g1 = screen.getByTestId('tt-level-1') as HTMLSelectElement;
    const g2 = screen.getByTestId('tt-level-2') as HTMLSelectElement;
    expect([g1.value, g2.value]).toEqual(['', '']);
    const options = Array.from(g1.options).slice(1);
    expect(options.map((o) => o.textContent)).toEqual(['b (4)', 'ｚ (3)', '😀 (3)']);
    expect(options.map((o) => o.value)).toEqual(['0', '1', '2']);
    const by = (select: HTMLSelectElement, label: string) => Array.from(select.options).find((o) => o.textContent?.startsWith(label))!.value;
    fireEvent.change(g1, { target: { value: by(g1, '😀') } });
    fireEvent.change(g2, { target: { value: by(g2, 'ｚ') } });
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_ttest_validate')).toHaveLength(1));
    const sent = specs(bridge, 'stats_ttest_validate')[0];
    expect(sent.parameters.levels).toEqual([2, 1]);
    expect([...labels].sort().indexOf('😀')).toBe(1);
    expect(JSON.stringify(sent)).not.toMatch(/ｚ|😀|"b"/);
  });

  it('offers a group coded in numbers, reads its levels from Rust, and sends positions from that list (decision 81)', async () => {
    const coded: DatasetView = {
      ...view,
      profile: {
        ...view.profile,
        columns: [...view.profile.columns, { alias: 'C4', header: 'dose', kind: 'numeric', n: 6, missing: 0, distinct: 2, levels: null, suspectMissingCodes: [], nonFinite: 0 }],
      },
    };
    const { bridge } = setup({ stats_ttest_validate: report('valid') }, coded);
    const group = screen.getByTestId('tt-group') as HTMLSelectElement;
    expect(Array.from(group.options).map((o) => o.value)).toContain('C4');
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    fireEvent.change(group, { target: { value: 'C4' } });
    await waitFor(() => expect(bridge.calls.filter(([c]) => c === 'stats_ttest_group_levels')).toEqual([['stats_ttest_group_levels', { slot: 3, column: 'C4' }]]));
    const g1 = (await screen.findByTestId('tt-level-1')) as HTMLSelectElement;
    const g2 = screen.getByTestId('tt-level-2') as HTMLSelectElement;
    await waitFor(() => expect(Array.from(g1.options).slice(1).map((o) => o.textContent)).toEqual(['10 (3)', '9 (3)']));
    expect([g1.value, g2.value]).toEqual(['0', '1']);
    const by = (select: HTMLSelectElement, label: string) => Array.from(select.options).find((o) => o.textContent === `${label} (3)`)!.value;
    fireEvent.change(g1, { target: { value: by(g1, '9') } });
    fireEvent.change(g2, { target: { value: by(g2, '10') } });
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_ttest_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters).toEqual({ outcome: 'C1', group: 'C4', levels: [1, 0], variant: 'welch', missingCodes: [] });
  });

  it('offers a group of 2 to 50 values only, and says why a column with more is not offered (decision 87)', () => {
    const numeric = (alias: string, header: string, distinct: number) => ({ alias, header, kind: 'numeric' as const, n: 60, missing: 0, distinct, levels: null, suspectMissingCodes: [], nonFinite: 0 });
    const wide: DatasetView = { ...view, profile: { ...view.profile, columns: [...view.profile.columns, numeric('C4', 'fifty_one', 51), numeric('C5', 'fifty', 50)] } };
    setup({}, wide);
    const offered = Array.from((screen.getByTestId('tt-group') as HTMLSelectElement).options).map((o) => o.value);
    expect(offered).toContain('C5');
    expect(offered).not.toContain('C4');
    expect(screen.getByTestId('tt-group-limit').textContent).toBe(
      'A column with more than 50 distinct values is not offered as a group: a t-test compares two groups, and a column with that many values is a measurement rather than a grouping.'
    );
  });

  it('words a refusal from Rust and keeps the dialog open', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    const { onStarted } = setup({
      stats_ttest_validate: () => {
        throw refusal;
      },
    });
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe('This run is not a t-test, so this screen cannot read it.');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('shows one neutral line for each variant from the wording table, and no default', async () => {
    const { VARIANT_CHOICES } = await import('../ttestWording');
    setup({ stats_ttest_validate: report('needs_user_input') });
    const box = screen.getByTestId('tt-variant');
    expect(VARIANT_CHOICES.map((c) => c.variant)).toEqual(['student', 'welch']);
    for (const c of VARIANT_CHOICES) {
      const line = screen.getByTestId(`tt-variant-line-${c.variant}`);
      expect(box.contains(line)).toBe(true);
      expect(line.textContent).toBe(c.line);
      const radio = screen.getByTestId(`tt-variant-${c.variant}`);
      expect(radio.getAttribute('aria-describedby')).toBe(line.id);
    }
    expect(screen.getByTestId('tt-variant-line-student').textContent).toMatch(/assumes the two groups' variances are equal/i);
    expect(screen.getByTestId('tt-variant-line-welch').textContent).toMatch(/does not assume the two groups' variances are equal/i);
    for (const c of VARIANT_CHOICES) expect(c.line).not.toMatch(/recommend|should|better|prefer|safer|default|usually|choose/i);
    const radios = within(box).getAllByRole('radio') as HTMLInputElement[];
    expect(radios.every((r) => !r.checked)).toBe(true);
  });
});

describe('with nothing chosen', () => {
  it('opens with no outcome, no group, no level and no test, Run off, and its levels rule shown', () => {
    setup({});
    expect((screen.getByTestId('tt-outcome') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('tt-group') as HTMLSelectElement).value).toBe('');
    expect(screen.queryByTestId('tt-levels')).toBeNull();
    expect((within(screen.getByTestId('tt-variant')).getAllByRole('radio') as HTMLInputElement[]).some((r) => r.checked)).toBe(false);
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('tt-levels-rule').textContent).not.toBe('');
  });
});

describe('a prepared score (decision 483)', () => {
  const withScore: DatasetView = {
    ...view,
    profile: {
      ...view.profile,
      columns: [...view.profile.columns, { alias: 'C4', header: 'C4', scoreName: 'Total anxiety', prepared: true, kind: 'numeric', n: 10, missing: 0, distinct: 2, levels: null, suspectMissingCodes: [], nonFinite: 0 }],
    },
  };

  it('is offered as an outcome, by the name the user typed, and never as a group', () => {
    setup({}, withScore);
    const outcomes = Array.from((screen.getByTestId('tt-outcome') as HTMLSelectElement).options);
    expect(outcomes.find((o) => o.value === 'C4')?.textContent).toBe('C4 Total anxiety');
    expect(Array.from((screen.getByTestId('tt-group') as HTMLSelectElement).options).map((o) => o.value)).not.toContain('C4');
  });

  it('nor on the page', async () => {
    const page = makeMockTTestBridge({ stats_desc_open: withScore, stats_desc_close: true });
    render(<TTestFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('tt-choose'));
    fireEvent.click(await screen.findByTestId('tt-to-settings'));
    const group = (await screen.findByTestId('tt-group')) as HTMLSelectElement;
    expect(Array.from(group.options).map((o) => o.value)).not.toContain('C4');
  });

  it("Rust's refusal reading a group's levels shows as the dialog's refusal, and nothing runs", async () => {
    const refused = () => {
      throw new Error(JSON.stringify({ code: 'stats.ttest.score_as_group' }));
    };
    const { bridge, onStarted } = setup({ stats_ttest_group_levels: refused });
    fireEvent.change(screen.getByTestId('tt-group'), { target: { value: 'C2' } });
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).not.toBe('');
    expect(bridge.calls.filter(([c]) => c === 'stats_ttest_run')).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
    expect(screen.queryByTestId('tt-levels')).toBeNull();
  });
});

describe('one set of choices gives one spec', () => {
  it("equal to the page's for the same choices", async () => {
    const { bridge, unmount } = setup({ stats_ttest_validate: report('valid') });
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C3');
    fireEvent.change(screen.getByTestId('tt-level-1'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('tt-level-2'), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(screen.getByTestId('tt-code--99'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_ttest_validate')).toHaveLength(1));
    const fromDialog = specs(bridge, 'stats_ttest_validate')[0];
    unmount();

    const page = makeMockTTestBridge({ stats_desc_open: view, stats_desc_close: true, stats_ttest_group_levels: levelsOf(view), stats_ttest_validate: report('valid') });
    render(<TTestFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('tt-choose'));
    fireEvent.click(await screen.findByTestId('tt-to-settings'));
    fireEvent.change(await screen.findByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C3');
    fireEvent.change(screen.getByTestId('tt-level-1'), { target: { value: '2' } });
    fireEvent.change(screen.getByTestId('tt-level-2'), { target: { value: '0' } });
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(screen.getByTestId('tt-code--99'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await waitFor(() => expect(specs(page, 'stats_ttest_validate')).toHaveLength(1));
    expect({ ...fromDialog, operationId: 0 }).toEqual({ ...specs(page, 'stats_ttest_validate')[0], operationId: 0 });
  });
});

describe("MethodDialog's rule, for the t-test", () => {
  const choices = async () => {
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-student'));
  };

  it('valid: hands the run to its parent, with its own settings line', async () => {
    const { onStarted } = setup({ stats_ttest_validate: report('valid') });
    await choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const s = onStarted.mock.calls[0][0];
    expect(s.runId).toBe(5);
    expect(s.settings).toContain('Outcome: C1 score. Group: C2 arm. Group 1: ctrl. Group 2: trt.');
    expect(s.settings).toContain('Missing codes: none.');
  });

  it('with the Output full, Run says so and nothing is validated or run', async () => {
    const { bridge, onStarted } = setup({}, view, { outputFull: true });
    await choices();
    const before = bridge.calls.length;
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe(OUTPUT_FULL);
    expect(bridge.calls).toHaveLength(before);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('never closes the slot: not on mount, Cancel, Run or unmount', async () => {
    const { bridge, onStarted, unmount } = setup({ stats_ttest_validate: report('valid') });
    const closes = () => bridge.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    await choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('tt-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render TTestSettings, by import, and neither holds a control', () => {
    for (const f of ['../TTestFlow.tsx', 'TTestDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bTTestSettings\b[^}]*\buseTTestSettings\b[^}]*\} from '[^']*TTestSettings'/);
      expect(src(f), f).toMatch(/<TTestSettings\b/);
      expect(src(f), f).not.toMatch(/tt-outcome|tt-group"|tt-variant-\$\{|data-testid="tt-variant"|tt-level-|tt-code-/);
    }
    expect(src('../TTestSettings.tsx')).toMatch(/tt-outcome/);
    expect(src('MethodDialog.tsx')).not.toMatch(/tt-outcome/);
  });

  it('its bridge type omits open and close, keeps groupLevels, and it names no close', () => {
    const typed = src('TTestDialog.tsx');
    expect(typed).toMatch(/Omit<TTestBridge, 'open' \| 'close'>/);
    for (const f of ['TTestDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });
});
