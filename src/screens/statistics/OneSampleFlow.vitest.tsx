// The one-sample t-test's screen (Research OS, Phase 8D-2b, decision 268),
// over a mock bridge, in TTestFlow.vitest.tsx's shape: the seven stages and
// focus on each heading; the column picker; the test value, empty at first
// and never filled in (an empty field and "abc" both send null, 2500 sends
// 2500, and Rust's test_value_not_chosen is said in its own words); Check the
// settings once a column is chosen, whatever the field holds; the
// missing-code question; Cancel; a failed run, with its sentence and Save to
// a project…; the neutral result card from the fixture, with the manifest;
// export through a mocked save dialog; and no path anywhere in the DOM.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import OneSampleFlow, { testValueOf } from './OneSampleFlow';
import { makeMockOneSampleBridge } from './onesampleBridge';
import type { DatasetView } from './describeTypes';
import type { OneSampleResultView, OneSampleRunStateView, OneSampleSpec, OneSampleValidationView } from './onesampleTypes';
import resultJson from './fixtures/onesampleResultView.json';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));
// Save to a project… shows only in the desktop app with both flags on.
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

const result = resultJson as OneSampleResultView;

const INPUT = { contentHash: 'ab'.repeat(32), rows: 10, columns: 2, semanticRole: 'table' as const };

const view: DatasetView = {
  slot: 3,
  displayName: 'trial.csv',
  profile: {
    rows: 10,
    columns: [
      { alias: 'C1', header: 'weight', kind: 'numeric', n: 10, missing: 0, distinct: 9, levels: null, suspectMissingCodes: [{ code: -99, cells: 2 }], nonFinite: 0 },
      { alias: 'C2', header: 'arm', kind: 'categorical', n: 10, missing: 0, distinct: 2, levels: [{ label: 'ctrl', count: 5 }, { label: 'trt', count: 5 }], suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: INPUT,
};

const report = (status: OneSampleValidationView['status'], extra: Partial<OneSampleValidationView> = {}): OneSampleValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 10, columns: 1, cells: 10 },
  ...extra,
});

const state = (
  s: OneSampleRunStateView['state'],
  terminal: boolean,
  cancelRequested = false,
  diagnostics: OneSampleRunStateView['diagnostics'] = []
): OneSampleRunStateView => ({ runId: 5, state: s, terminal, elapsedMs: 1200, cancelRequested, diagnostics });

const PATH = '/Users/someone/PATH_SENTINEL_q4/trial.csv';

function setup(responses: Record<string, unknown>, save = vi.fn().mockResolvedValue('/chosen/out.csv')) {
  const bridge = makeMockOneSampleBridge({ stats_desc_open: view, stats_desc_close: true, ...responses });
  render(<OneSampleFlow bridge={bridge} pickPath={async () => PATH} save={save} pollMs={10} />);
  return { bridge, save };
}

const heading = () => screen.getByTestId('os-stage-heading');
async function expectFocusOn(text: string) {
  await waitFor(() => expect(heading().textContent).toBe(text));
  await waitFor(() => expect(document.activeElement).toBe(heading()));
}

const specs = (bridge: ReturnType<typeof makeMockOneSampleBridge>, cmd: string) =>
  bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as OneSampleSpec);

async function toSettings() {
  fireEvent.click(screen.getByTestId('os-choose'));
  await expectFocusOn('Profile of the file');
  fireEvent.click(screen.getByTestId('os-to-settings'));
  await expectFocusOn('Choose the column and the test value');
}

const typeValue = (text: string) => fireEvent.change(screen.getByTestId('os-test-value'), { target: { value: text } });

async function checkAndRun() {
  fireEvent.click(screen.getByTestId('os-check'));
  await expectFocusOn('Check the settings');
  fireEvent.click(screen.getByTestId('os-run'));
  await expectFocusOn('Run the t-test');
}

describe('OneSampleFlow', () => {
  it('walks Import → Profile → Settings → Validate → Run → Results → Export, focus on each heading', async () => {
    let polls = 0;
    const { bridge, save } = setup({
      stats_onesample_validate: report('valid'),
      stats_onesample_run: { runId: 5 },
      stats_onesample_status: () => (polls++ < 1 ? state('running', false) : state('succeeded', true)),
      stats_onesample_result: result,
      stats_onesample_export: { bytes: Array.from(new TextEncoder().encode('"statistic","column","value","word"\r\n')), sha256: 'cd'.repeat(32) },
    });
    expect(heading().textContent).toBe('Import a data file');
    expect(screen.getAllByRole('navigation')[0].textContent).toBe('ImportProfileSettingsValidateRunResultsExport');
    await toSettings();
    // The column picker offers numeric columns only.
    const picker = screen.getByTestId('os-outcome') as HTMLSelectElement;
    expect(Array.from(picker.options).map((o) => o.value)).toEqual(['', 'C1']);
    fireEvent.change(picker, { target: { value: 'C1' } });
    typeValue('2500');
    await checkAndRun();
    expect(await screen.findByTestId('os-see-results')).toBeTruthy();
    expect(screen.getByTestId('os-run-state').textContent).toBe('Finished');
    fireEvent.click(screen.getByTestId('os-see-results'));
    await expectFocusOn('Results');
    fireEvent.click(screen.getByTestId('os-to-export'));
    await expectFocusOn('Export');
    fireEvent.click(screen.getByTestId('os-save'));
    expect((await screen.findByTestId('os-export-note')).textContent).toBe('Saved. SHA-256 of the file: cdcdcdcdcdcd…');
    expect(save).toHaveBeenCalledWith('one_sample_t_test.csv', '"statistic","column","value","word"\r\n', 'text/csv');
    const sent = specs(bridge, 'stats_onesample_run')[0];
    expect(sent.capabilityId).toBe('stats.analyze.one_sample_t');
    expect(sent.parameters).toEqual({ outcome: 'C1', testValue: 2500, missingCodes: [] });
    expect(bridge.calls[0]).toEqual(['stats_desc_open', { path: '<path>' }]);
    expect(document.body.innerHTML).not.toContain('PATH_SENTINEL');
  });

  it('leaves the test value empty, sends null, and says Rust’s test_value_not_chosen in its own words', async () => {
    const { bridge } = setup({
      stats_onesample_validate: report('needs_user_input', {
        missingInformation: [{ code: 'stats.onesample.test_value_not_chosen', params: {}, severity: 'note' }],
      }),
    });
    await toSettings();
    const field = screen.getByTestId('os-test-value') as HTMLInputElement;
    expect(field.value).toBe('');
    expect(field.getAttribute('inputmode')).toBe('decimal');
    expect(screen.getByTestId('os-test-value-line').textContent).toBe('The mean difference is the mean minus the test value.');
    // Check the settings waits for a column only, whatever the field holds.
    expect((screen.getByTestId('os-check') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    expect((screen.getByTestId('os-check') as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByTestId('os-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_onesample_validate')[0].parameters.testValue).toBeNull();
    expect(screen.getByTestId('os-validation-questions').textContent).toBe(
      'Type the test value in Settings: the value the mean is compared with. Gaply does not choose one for you.'
    );
    expect((screen.getByTestId('os-run') as HTMLButtonElement).disabled).toBe(true);
  });

  it('sends null for "abc" and for an empty or blank field, and the number for 2500', async () => {
    const { bridge } = setup({ stats_onesample_validate: report('valid') });
    await toSettings();
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    for (const text of ['abc', '', '   ', '2500']) {
      typeValue(text);
      fireEvent.click(screen.getByTestId('os-check'));
      await expectFocusOn('Check the settings');
      fireEvent.click(screen.getByTestId('os-back-settings'));
      await expectFocusOn('Choose the column and the test value');
    }
    expect(specs(bridge, 'stats_onesample_validate').map((s) => s.parameters.testValue)).toEqual([null, null, null, 2500]);
    // The rule itself: a finite number, or null; never a default.
    expect([testValueOf(''), testValueOf(' 1.5 '), testValueOf('1e-7'), testValueOf('Infinity'), testValueOf('NaN'), testValueOf('-0.25')]).toEqual([
      null,
      1.5,
      1e-7,
      null,
      null,
      -0.25,
    ]);
  });

  it('asks about a candidate missing code in the column, and applies it only when ticked', async () => {
    const { bridge } = setup({ stats_onesample_validate: report('valid') });
    await toSettings();
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('3');
    expect(screen.getByTestId('os-missing-codes').textContent).toContain(
      'Is -99 a code for a missing value? It appears in the column "weight" (2 cells). If you tick it, every row whose value is -99 is left out.'
    );
    fireEvent.click(screen.getByTestId('os-check'));
    await expectFocusOn('Check the settings');
    fireEvent.click(screen.getByTestId('os-back-settings'));
    await expectFocusOn('Choose the column and the test value');
    fireEvent.click(screen.getByTestId('os-code--99'));
    fireEvent.click(screen.getByTestId('os-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_onesample_validate').map((s) => s.parameters.missingCodes)).toEqual([[], [-99]]);
  });

  it('Cancel asks Rust to stop the run, and shows the end it reports', async () => {
    let cancelled = false;
    const { bridge } = setup({
      stats_onesample_validate: report('valid'),
      stats_onesample_run: { runId: 5 },
      stats_onesample_status: () => (cancelled ? state('cancelled', true, true) : state('running', false)),
      stats_onesample_cancel: () => {
        cancelled = true;
        return state('running', false, true);
      },
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('1');
    await checkAndRun();
    fireEvent.click(await screen.findByTestId('os-cancel'));
    expect((await screen.findByTestId('os-run-ended')).textContent).toBe('This run was cancelled, so it has no result.');
    expect(bridge.calls.filter(([c]) => c === 'stats_onesample_cancel')).toEqual([['stats_onesample_cancel', { runId: 5 }]]);
    expect(screen.queryByTestId('os-see-results')).toBeNull();
  });

  it('a failed run says why, in decision 259’s words, and offers Save to a project…', async () => {
    setup({
      stats_onesample_validate: report('valid'),
      stats_onesample_run: { runId: 5 },
      stats_onesample_status: state('failed', true, false, [
        { code: 'stats.onesample.values_essentially_constant', params: {}, severity: 'error' },
      ]),
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('0');
    await checkAndRun();
    expect((await screen.findByTestId('os-run-ended')).textContent).toBe('The t-test could not be computed. The reason is below.');
    expect(screen.getByTestId('os-run-diagnostics').textContent).toBe(
      'The values vary so little, compared with their size, that the standard error is lost in rounding, so t is not computed. R refuses such data too: "data are essentially constant".'
    );
    expect(screen.getByTestId('os-project-save-button')).toBeTruthy();
    expect(screen.queryByTestId('os-see-results')).toBeNull();
  });

  it('shows a neutral result card from the fixture: the numbers, the exclusions, the limitations and the manifest', async () => {
    setup({
      stats_onesample_validate: report('valid'),
      stats_onesample_run: { runId: 5 },
      stats_onesample_status: state('succeeded', true),
      stats_onesample_result: result,
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    typeValue('50.5');
    await checkAndRun();
    fireEvent.click(await screen.findByTestId('os-see-results'));
    await expectFocusOn('Results');
    const card = screen.getByTestId('os-result-card');
    const text = (id: string) => screen.getByTestId(id).textContent;
    expect(text('os-n')).toBe('9');
    expect(text('os-mean')).toBe('63.125');
    expect(text('os-sd')).toBe('29.25');
    expect(text('os-test-value-shown')).toBe('50.5');
    expect(card.textContent).toContain('Mean difference (the mean minus the test value)');
    expect(text('os-mean-difference')).toBe('12.625');
    expect(text('os-se')).toBe('9.75');
    expect(card.textContent).toContain('95% confidence interval');
    expect(text('os-ci')).toBe('-9.875 to 35.125');
    expect(text('os-t')).toBe('1.5');
    expect(text('os-df')).toBe('8');
    expect(text('os-p')).toBe('0.125');
    expect(text('os-d')).toBe('0.4375');
    expect(text('os-d-no-interval')).toBe("Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.");
    expect(text('os-result-notes')).toBe(
      'The column "weight" holds 99 in 2 cells, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.'
    );
    expect(text('os-exclusions')).toContain('1 row had no value (an empty cell or a missing-value token), and was left out.');
    expect(within(screen.getByTestId('os-limitations')).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      'Normality is not assessed: the t-test assumes the values come from a roughly normal distribution, or that there are many of them, and Gaply does not check either.',
      "Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.",
    ]);
    expect(text('os-manifest-input')).toBe('0123456789ab…');
    expect(text('os-manifest-capability')).toBe('stats.analyze.one_sample_t, version 1');
    expect(text('os-manifest-rows-read')).toBe('10');
    expect(text('os-manifest-rows-used')).toBe('9');
    expect(text('os-manifest-output')).toBe('5fd0af5daee4…');
    expect(screen.getByTestId('os-manifest').textContent).toContain('The values used. Rows read is rows used plus the rows left out above.');
    expect(screen.getByTestId('os-project-save-button')).toBeTruthy();
    // Neutral (decision 66): no verdict word and no star, anywhere on screen.
    const shown = document.body.textContent ?? '';
    expect(shown).not.toMatch(/significan|reject|accept the|null hypothesis/i);
    expect(shown).not.toMatch(/[*★☆]/);
    expect(card.getAttribute('data-tone')).toBe('neutral');
  });

  it('words a refusal from Rust and keeps the screen where it was', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    setup({
      stats_onesample_validate: () => {
        throw refusal;
      },
    });
    await toSettings();
    fireEvent.change(screen.getByTestId('os-outcome'), { target: { value: 'C1' } });
    fireEvent.click(screen.getByTestId('os-check'));
    expect((await screen.findByTestId('os-error')).textContent).toBe('This run is not of this test, so this screen cannot read it.');
    expect(heading().textContent).toBe('Choose the column and the test value');
  });

  it('closes the file, and Rust drops its rows, when the screen goes', async () => {
    const { bridge } = setup({});
    fireEvent.click(screen.getByTestId('os-choose'));
    await expectFocusOn('Profile of the file');
    cleanup();
    await waitFor(() => expect(bridge.calls.some(([c, a]) => c === 'stats_desc_close' && a.slot === 3)).toBe(true));
  });
});
