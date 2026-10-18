// The independent-samples t-test's screen (Research OS, Phase 4B-2), over a
// mock bridge: the seven stages and focus on each heading; the variant asked
// with no default (decision 64); the missing-code question; the two levels
// chosen by label and sent as positions in the profile's list, with labels
// whose UTF-16 order and code-point order differ, so a JavaScript sort would
// swap them (decision 78); Cancel; the neutral result card (decision 66);
// the exclusions, limitations and manifest; export through a mocked save
// dialog; and no path anywhere in the DOM.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import TTestFlow from './TTestFlow';
import { makeMockTTestBridge } from './ttestBridge';
import type { DatasetView } from './describeTypes';
import type { TTestResultView, TTestRunStateView, TTestSpec, TTestValidationReportView } from './ttestTypes';
import { ttestResultView } from './fixtures/ttestFixtures';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

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

const state = (s: TTestRunStateView['state'], terminal: boolean, cancelRequested = false): TTestRunStateView => ({
  runId: 5,
  state: s,
  terminal,
  elapsedMs: 1200,
  cancelRequested,
  diagnostics: [],
});

const PATH = '/Users/someone/PATH_SENTINEL_z9/trial.csv';

/** What stats_ttest_group_levels answers (decision 81): a column's levels
 *  as the core indexes them. For a categorical column of this test's view,
 *  its profile's list; for the numeric C4, "10" before "9" (code points). */
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

function setup(responses: Record<string, unknown>, save = vi.fn().mockResolvedValue('/chosen/out.csv'), v: DatasetView = view) {
  const bridge = makeMockTTestBridge({ stats_desc_open: v, stats_desc_close: true, stats_ttest_group_levels: levelsOf(v), ...responses });
  render(<TTestFlow bridge={bridge} pickPath={async () => PATH} save={save} pollMs={10} />);
  return { bridge, save };
}

const heading = () => screen.getByTestId('tt-stage-heading');
async function expectFocusOn(text: string) {
  await waitFor(() => expect(heading().textContent).toBe(text));
  await waitFor(() => expect(document.activeElement).toBe(heading()));
}

const specs = (bridge: ReturnType<typeof makeMockTTestBridge>, cmd: string) =>
  bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as TTestSpec);

/** Chooses the group, then waits for Rust's list of its levels (decision 81). */
async function chooseGroup(alias: string) {
  fireEvent.change(screen.getByTestId('tt-group'), { target: { value: alias } });
  await screen.findByTestId('tt-levels');
  await waitFor(() => expect((screen.getByTestId('tt-level-1') as HTMLSelectElement).options.length).toBeGreaterThan(1));
}

async function toSettings() {
  fireEvent.click(screen.getByTestId('tt-choose'));
  await expectFocusOn('Profile of the file');
  fireEvent.click(screen.getByTestId('tt-to-settings'));
  await expectFocusOn('Choose the outcome, the group and the test');
}

describe('TTestFlow', () => {
  it('walks Import → Profile → Settings → Validate → Run → Results → Export, focus on each heading', async () => {
    let polls = 0;
    const { bridge, save } = setup({
      stats_ttest_validate: report('valid'),
      stats_ttest_run: { runId: 5 },
      stats_ttest_status: () => (polls++ < 1 ? state('running', false) : state('succeeded', true)),
      stats_ttest_result: ttestResultView,
      stats_ttest_export: { bytes: Array.from(new TextEncoder().encode('"statistic","value"\r\n')), sha256: 'cd'.repeat(32) },
    });
    expect(heading().textContent).toBe('Import a data file');
    expect(screen.getAllByRole('navigation')[0].textContent).toBe('ImportProfileSettingsValidateRunResultsExport');
    await toSettings();
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    expect(screen.getByTestId('tt-validation-status').textContent).toBe('The settings can run.');
    fireEvent.click(screen.getByTestId('tt-run'));
    await expectFocusOn('Run the t-test');
    expect(await screen.findByTestId('tt-see-results')).toBeTruthy();
    expect(screen.getByTestId('tt-run-state').textContent).toBe('Finished');
    expect(screen.getByTestId('tt-elapsed').textContent).toBe('Elapsed since you pressed Run: 1.2 s');
    fireEvent.click(screen.getByTestId('tt-see-results'));
    await expectFocusOn('Results');
    fireEvent.click(screen.getByTestId('tt-to-export'));
    await expectFocusOn('Export');
    fireEvent.click(screen.getByTestId('tt-save'));
    expect((await screen.findByTestId('tt-export-note')).textContent).toBe('Saved. SHA-256 of the file: cdcdcdcdcdcd…');
    // Through the save dialog only, with the bytes Rust returned.
    expect(save).toHaveBeenCalledWith('independent_t_test.csv', '"statistic","value"\r\n', 'text/csv');
    // Two levels, defaulted to the profile's positions 0 and 1.
    const sent = specs(bridge, 'stats_ttest_run')[0];
    expect(sent.capabilityId).toBe('stats.analyze.independent_t');
    expect(sent.parameters).toEqual({ outcome: 'C1', group: 'C2', levels: [0, 1], variant: 'welch', missingCodes: [] });
    // The path went to open once, and appears nowhere on screen.
    expect(bridge.calls[0]).toEqual(['stats_desc_open', { path: '<path>' }]);
    expect(document.body.innerHTML).not.toContain('PATH_SENTINEL');
  });

  it('asks the variant with no default, and sends none until the user chooses', async () => {
    const { bridge } = setup({
      stats_ttest_validate: report('needs_user_input', {
        missingInformation: [{ code: 'stats.ttest.variant_not_chosen', params: {}, severity: 'note' }],
      }),
    });
    await toSettings();
    const radios = within(screen.getByTestId('tt-variant')).getAllByRole('radio') as HTMLInputElement[];
    expect(radios).toHaveLength(2);
    expect(radios.every((r) => !r.checked)).toBe(true);
    expect(screen.getByTestId('tt-variant').textContent).toContain('Gaply does not choose one for you.');
    // Nothing can be checked before an outcome and a group are chosen.
    expect((screen.getByTestId('tt-check') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters.variant).toBeNull();
    expect(screen.getByTestId('tt-validation-questions').textContent).toBe(
      "Choose Student's t-test or Welch's t-test in Settings. Gaply does not choose one for you."
    );
    expect((screen.getByTestId('tt-run') as HTMLButtonElement).disabled).toBe(true);
  });

  it('asks about a candidate missing code in the outcome, and applies it only when ticked', async () => {
    const { bridge } = setup({ stats_ttest_validate: report('valid') });
    await toSettings();
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    const q = screen.getByTestId('tt-missing-codes');
    expect(q.textContent).toContain(
      'Is -99 a code for a missing value? It appears in the outcome "score" (2 cells). If you tick it, every row whose outcome is -99 is left out.'
    );
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-student'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters.missingCodes).toEqual([]);
    fireEvent.click(screen.getByTestId('tt-back-settings'));
    await expectFocusOn('Choose the outcome, the group and the test');
    fireEvent.click(screen.getByTestId('tt-code--99'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_ttest_validate')[1].parameters.missingCodes).toEqual([-99]);
  });

  it('shows the levels by label in the profile order and sends their positions, where a JavaScript sort would swap them', async () => {
    const labels = view.profile.columns[2].levels!.map((l) => l.label);
    // The premise of this test: the two orders differ.
    expect([...labels].sort()).not.toEqual(labels);
    const { bridge } = setup({ stats_ttest_validate: report('valid') });
    await toSettings();
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C3');
    expect(screen.getByTestId('tt-levels-question').textContent).toBe(
      'The group has 3 levels. Choose the two to compare: the first is group 1 and the second is group 2.'
    );
    // More than two levels: none is chosen for the user.
    const g1 = screen.getByTestId('tt-level-1') as HTMLSelectElement;
    const g2 = screen.getByTestId('tt-level-2') as HTMLSelectElement;
    expect([g1.value, g2.value]).toEqual(['', '']);
    // The options are the profile's labels, in its order, valued by position.
    const options = Array.from(g1.options).slice(1);
    expect(options.map((o) => o.textContent)).toEqual(['b (4)', 'ｚ (3)', '😀 (3)']);
    expect(options.map((o) => o.value)).toEqual(['0', '1', '2']);
    // Choose by label: group 1 is 😀, group 2 is ｚ.
    const by = (select: HTMLSelectElement, label: string) =>
      Array.from(select.options).find((o) => o.textContent?.startsWith(label))!.value;
    fireEvent.change(g1, { target: { value: by(g1, '😀') } });
    fireEvent.change(g2, { target: { value: by(g2, 'ｚ') } });
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    const sent = specs(bridge, 'stats_ttest_validate')[0];
    expect(sent.parameters.levels).toEqual([2, 1]);
    // A JavaScript sort would have put 😀 at 1 and ｚ at 2.
    expect([...labels].sort().indexOf('😀')).toBe(1);
    // No label goes into a spec.
    expect(JSON.stringify(sent)).not.toMatch(/ｚ|😀|"b"/);
  });

  it('offers a group coded in numbers, reads its levels from Rust, and sends positions from that list (decision 81)', async () => {
    // C4 is numeric, so the profile lists no levels for it: only its
    // distinct count says it has two.
    const coded: DatasetView = {
      ...view,
      profile: {
        ...view.profile,
        columns: [
          ...view.profile.columns,
          { alias: 'C4', header: 'dose', kind: 'numeric', n: 6, missing: 0, distinct: 2, levels: null, suspectMissingCodes: [], nonFinite: 0 },
        ],
      },
    };
    const { bridge } = setup({ stats_ttest_validate: report('valid') }, undefined, coded);
    await toSettings();
    const group = screen.getByTestId('tt-group') as HTMLSelectElement;
    expect(Array.from(group.options).map((o) => o.value)).toContain('C4');
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    fireEvent.change(group, { target: { value: 'C4' } });
    await waitFor(() =>
      expect(bridge.calls.filter(([c]) => c === 'stats_ttest_group_levels')).toEqual([['stats_ttest_group_levels', { slot: 3, column: 'C4' }]])
    );
    const g1 = (await screen.findByTestId('tt-level-1')) as HTMLSelectElement;
    const g2 = screen.getByTestId('tt-level-2') as HTMLSelectElement;
    await waitFor(() => expect(Array.from(g1.options).slice(1).map((o) => o.textContent)).toEqual(['10 (3)', '9 (3)']));
    // Two levels: the command's positions 0 and 1 by default.
    expect([g1.value, g2.value]).toEqual(['0', '1']);
    // Choose by label: group 1 is "9", group 2 is "10". A numeric sort in
    // JavaScript would have put "9" first, at position 0.
    const by = (select: HTMLSelectElement, label: string) => Array.from(select.options).find((o) => o.textContent === `${label} (3)`)!.value;
    fireEvent.change(g1, { target: { value: by(g1, '9') } });
    fireEvent.change(g2, { target: { value: by(g2, '10') } });
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_ttest_validate')[0].parameters).toEqual({
      outcome: 'C1',
      group: 'C4',
      levels: [1, 0],
      variant: 'welch',
      missingCodes: [],
    });
  });

  it('offers a group of 2 to 50 values only, and says why a column with more is not offered (decision 87)', async () => {
    const numeric = (alias: string, header: string, distinct: number) => ({
      alias,
      header,
      kind: 'numeric' as const,
      n: 60,
      missing: 0,
      distinct,
      levels: null,
      suspectMissingCodes: [],
      nonFinite: 0,
    });
    const wide: DatasetView = {
      ...view,
      profile: { ...view.profile, columns: [...view.profile.columns, numeric('C4', 'fifty_one', 51), numeric('C5', 'fifty', 50)] },
    };
    setup({}, undefined, wide);
    await toSettings();
    const offered = Array.from((screen.getByTestId('tt-group') as HTMLSelectElement).options).map((o) => o.value);
    expect(offered).toContain('C5');
    expect(offered).not.toContain('C4');
    expect(screen.getByTestId('tt-group-limit').textContent).toBe(
      'A column with more than 50 distinct values is not offered as a group: a t-test compares two groups, and a column with that many values is a measurement rather than a grouping.'
    );
  });

  it('Cancel asks Rust to stop the run, and shows the end it reports', async () => {
    let cancelled = false;
    const { bridge } = setup({
      stats_ttest_validate: report('valid'),
      stats_ttest_run: { runId: 5 },
      stats_ttest_status: () => (cancelled ? state('cancelled', true, true) : state('running', false)),
      stats_ttest_cancel: () => {
        cancelled = true;
        return state('running', false, true);
      },
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-student'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    fireEvent.click(screen.getByTestId('tt-run'));
    await expectFocusOn('Run the t-test');
    fireEvent.click(await screen.findByTestId('tt-cancel'));
    expect((await screen.findByTestId('tt-run-ended')).textContent).toBe('This run was cancelled, so it has no result.');
    expect(screen.getByTestId('tt-run-state').textContent).toBe('Cancelled');
    expect(bridge.calls.filter(([c]) => c === 'stats_ttest_cancel')).toEqual([['stats_ttest_cancel', { runId: 5 }]]);
    expect(screen.queryByTestId('tt-see-results')).toBeNull();
  });

  it('shows a neutral result card: the numbers, what d used, the exclusions, the limitations and the manifest', async () => {
    const { bridge } = setup({
      stats_ttest_validate: report('valid'),
      stats_ttest_run: { runId: 5 },
      stats_ttest_status: state('succeeded', true),
      stats_ttest_result: ttestResultView,
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-variant-welch'));
    fireEvent.click(screen.getByTestId('tt-check'));
    await expectFocusOn('Check the settings');
    fireEvent.click(screen.getByTestId('tt-run'));
    fireEvent.click(await screen.findByTestId('tt-see-results'));
    await expectFocusOn('Results');
    const card = screen.getByTestId('tt-result-card');
    const text = (id: string) => screen.getByTestId(id).textContent;
    expect(text('tt-variant-name')).toBe("Welch's t-test (equal variances not assumed)");
    expect(text('tt-t')).toBe('1.5');
    expect(text('tt-df')).toBe('7.25');
    expect(text('tt-p')).toBe('0.125');
    expect(card.textContent).toContain('Mean difference (ctrl minus trt)');
    expect(text('tt-mean-difference')).toBe('30.5');
    expect(text('tt-se')).toBe('16.25');
    expect(card.textContent).toContain('95% confidence interval');
    expect(text('tt-ci')).toBe('-8.5 to 68.75');
    expect(text('tt-d')).toBe('1.125');
    expect(text('tt-d-standardizer')).toBe('Divided by the square root of the mean of the two variances: 26.5');
    expect(text('tt-d-no-interval')).toBe("Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.");
    expect(text('tt-sd-ratio')).toBe('1.25');
    expect(card.textContent).toContain('Shown as information, not as advice on which t-test to use.');
    const groups = within(screen.getByTestId('tt-groups')).getAllByRole('row').slice(1).map((r) => r.textContent);
    expect(groups).toEqual(['1ctrl563.12529.25', '2trt433.062522.75']);
    expect(text('tt-exclusions')).toContain('1 row had no outcome (an empty cell or a missing-value token), and was left out.');
    expect(within(screen.getByTestId('tt-limitations')).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      "Normality is not assessed: the t-test assumes each group's values come from a roughly normal distribution, or that the groups are large, and Gaply does not check either.",
      "Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.",
    ]);
    // The manifest panel, as on the describe screen: hashes and counts.
    expect(text('tt-manifest-input')).toBe('0123456789ab…');
    expect(text('tt-manifest-capability')).toBe('stats.analyze.independent_t, version 1');
    expect(text('tt-manifest-rows-read')).toBe('10');
    expect(text('tt-manifest-rows-used')).toBe('9');
    expect(text('tt-manifest-output')).toBe('5fd0af5daee4…');
    // Neutral (decision 66): no verdict word and no star, anywhere on screen.
    const shown = document.body.textContent ?? '';
    expect(shown).not.toMatch(/significan/i);
    expect(shown).not.toMatch(/[*★☆]/);
    expect(card.getAttribute('data-tone')).toBe('neutral');
    expect(bridge.calls.some(([c]) => c === 'stats_ttest_result')).toBe(true);
  });

  it('words a refusal from Rust and keeps the screen where it was', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    setup({
      stats_ttest_validate: () => {
        throw refusal;
      },
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('tt-outcome'), { target: { value: 'C1' } });
    await chooseGroup('C2');
    fireEvent.click(screen.getByTestId('tt-check'));
    expect((await screen.findByTestId('tt-error')).textContent).toBe('This run is not a t-test, so this screen cannot read it.');
    expect(heading().textContent).toBe('Choose the outcome, the group and the test');
  });

  it('closes the file, and Rust drops its rows, when the screen goes', async () => {
    const { bridge } = setup({});
    fireEvent.click(screen.getByTestId('tt-choose'));
    await expectFocusOn('Profile of the file');
    cleanup();
    await waitFor(() => expect(bridge.calls.some(([c, a]) => c === 'stats_desc_close' && a.slot === 3)).toBe(true));
  });
});

// Keep the fixture's shape honest: the card above reads exactly this view.
const _typed: TTestResultView = ttestResultView;
void _typed;

// Decision 93 (the Phase 4 evaluation's F7): beside the variant question, one
// neutral line for each variant from the wording table, saying what Student
// assumes and what Welch does not. Information, never a recommendation, and
// still no default (decision 64).
describe('the variant question (decision 93)', () => {
  it('shows one neutral line for each variant from the wording table, and no default', async () => {
    const { VARIANT_CHOICES } = await import('./ttestWording');
    setup({ stats_ttest_validate: report('needs_user_input') });
    await toSettings();
    const box = screen.getByTestId('tt-variant');
    expect(VARIANT_CHOICES.map((c) => c.variant)).toEqual(['student', 'welch']);
    for (const c of VARIANT_CHOICES) {
      const line = screen.getByTestId(`tt-variant-line-${c.variant}`);
      expect(box.contains(line)).toBe(true);
      expect(line.textContent).toBe(c.line);
      // The line describes the radio it sits beside.
      const radio = screen.getByTestId(`tt-variant-${c.variant}`);
      expect(radio.getAttribute('aria-describedby')).toBe(line.id);
    }
    expect(screen.getByTestId('tt-variant-line-student').textContent).toMatch(/assumes the two groups' variances are equal/i);
    expect(screen.getByTestId('tt-variant-line-welch').textContent).toMatch(/does not assume the two groups' variances are equal/i);
    // Information only: no line advises, and nothing is chosen for the user.
    for (const c of VARIANT_CHOICES) {
      expect(c.line).not.toMatch(/recommend|should|better|prefer|safer|default|usually|choose/i);
    }
    const radios = within(box).getAllByRole('radio') as HTMLInputElement[];
    expect(radios.every((r) => !r.checked)).toBe(true);
  });
});
