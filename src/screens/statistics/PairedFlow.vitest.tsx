// The paired-samples t-test's screen (Research OS, Phase 8D-2c, decision
// 271), over a mock bridge, in OneSampleFlow.vitest.tsx's shape: the seven
// stages and focus on each heading; a first and a second column picked from
// the numeric columns, with no default, and Check the settings once both are
// chosen; the spec carrying them as chosen; the same column twice sent and
// refused by Rust in same_column's words; each candidate missing code of
// either column offered once, with both columns' counts; Cancel; a failed
// run, with decision 259's sentence and Save to a project…; the neutral
// result card from the fixture, with the manifest; export through a mocked
// save dialog; and no path anywhere in the DOM.
//
// Written before the screen (8D-2c's rules): setup() asserts the first
// heading, so against a stub that renders nothing every case starts and
// fails on that assertion.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import PairedFlow from './PairedFlow';
import { makeMockPairedBridge } from './onesampleBridge';
import type { DatasetView } from './describeTypes';
import type { OneSampleRunStateView, OneSampleValidationView, PairedResultView, PairedSpec } from './onesampleTypes';
import resultJson from './fixtures/pairedResultView.json';

vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));
// Save to a project… shows only in the desktop app with both flags on.
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(cleanup);

const result = resultJson as PairedResultView;

const INPUT = { contentHash: 'ab'.repeat(32), rows: 10, columns: 3, semanticRole: 'table' as const };

// Two numeric columns, each with its own candidate missing codes: -99 in
// both, 999 in the first only, 77 in the second only.
const view: DatasetView = {
  slot: 3,
  displayName: 'pairs.csv',
  profile: {
    rows: 10,
    columns: [
      {
        alias: 'C1',
        header: 'before',
        kind: 'numeric',
        n: 10,
        missing: 0,
        distinct: 9,
        levels: null,
        suspectMissingCodes: [
          { code: -99, cells: 2 },
          { code: 999, cells: 1 },
        ],
        nonFinite: 0,
      },
      {
        alias: 'C2',
        header: 'after',
        kind: 'numeric',
        n: 10,
        missing: 0,
        distinct: 9,
        levels: null,
        suspectMissingCodes: [
          { code: -99, cells: 1 },
          { code: 77, cells: 3 },
        ],
        nonFinite: 0,
      },
      { alias: 'C3', header: 'arm', kind: 'categorical', n: 10, missing: 0, distinct: 2, levels: [{ label: 'ctrl', count: 5 }, { label: 'trt', count: 5 }], suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: INPUT,
};

const report = (status: OneSampleValidationView['status'], extra: Partial<OneSampleValidationView> = {}): OneSampleValidationView => ({
  status,
  errors: [],
  warnings: [],
  missingInformation: [],
  estimatedResources: { rows: 10, columns: 2, cells: 20 },
  ...extra,
});

const state = (
  s: OneSampleRunStateView['state'],
  terminal: boolean,
  cancelRequested = false,
  diagnostics: OneSampleRunStateView['diagnostics'] = []
): OneSampleRunStateView => ({ runId: 6, state: s, terminal, elapsedMs: 1200, cancelRequested, diagnostics });

const PATH = '/Users/someone/PATH_SENTINEL_p7/pairs.csv';

const heading = () => screen.queryByTestId('pt-stage-heading');

function setup(responses: Record<string, unknown>, save = vi.fn().mockResolvedValue('/chosen/out.csv')) {
  const bridge = makeMockPairedBridge({ stats_desc_open: view, stats_desc_close: true, ...responses });
  render(<PairedFlow bridge={bridge} pickPath={async () => PATH} save={save} pollMs={10} />);
  // The first stage's heading: the assertion every case meets first.
  expect(heading()?.textContent).toBe('Import a data file');
  return { bridge, save };
}

async function expectFocusOn(text: string) {
  await waitFor(() => expect(heading()?.textContent).toBe(text));
  await waitFor(() => expect(document.activeElement).toBe(heading()));
}

const specs = (bridge: ReturnType<typeof makeMockPairedBridge>, cmd: string) =>
  bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as PairedSpec);

const SETTINGS = 'Choose the first and the second column';

async function toSettings() {
  fireEvent.click(screen.getByTestId('pt-choose'));
  await expectFocusOn('Profile of the file');
  fireEvent.click(screen.getByTestId('pt-to-settings'));
  await expectFocusOn(SETTINGS);
}

const pick = (which: 'first' | 'second', alias: string) => fireEvent.change(screen.getByTestId(`pt-${which}`), { target: { value: alias } });

async function checkAndRun() {
  fireEvent.click(screen.getByTestId('pt-check'));
  await expectFocusOn('Check the settings');
  fireEvent.click(screen.getByTestId('pt-run'));
  await expectFocusOn('Run the t-test');
}

/** Decision 66's words, as onesampleWording.vitest.ts reads them. */
const VERDICT = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

describe('PairedFlow', () => {
  it('walks Import → Profile → Settings → Validate → Run → Results → Export, focus on each heading, and sends first and second as chosen', async () => {
    let polls = 0;
    const { bridge, save } = setup({
      stats_paired_validate: report('valid'),
      stats_paired_run: { runId: 6 },
      stats_paired_status: () => (polls++ < 1 ? state('running', false) : state('succeeded', true)),
      stats_paired_result: result,
      stats_paired_export: { bytes: Array.from(new TextEncoder().encode('"statistic","column","value","word"\r\n')), sha256: 'cd'.repeat(32) },
    });
    expect(screen.getAllByRole('navigation')[0].textContent).toBe('ImportProfileSettingsValidateRunResultsExport');
    expect(screen.getByTestId('pt-entry-text').textContent).toBe(
      'A paired-samples t-test on your own data: the mean of the differences between two numeric columns measured on the same rows, first minus second. The file is read on this device and nothing is sent anywhere. The data stays in memory while this screen is open and is dropped when you close the file or the app.'
    );
    await toSettings();
    pick('first', 'C2');
    pick('second', 'C1');
    await checkAndRun();
    expect(await screen.findByTestId('pt-see-results')).toBeTruthy();
    expect(screen.getByTestId('pt-run-state').textContent).toBe('Finished');
    fireEvent.click(screen.getByTestId('pt-see-results'));
    await expectFocusOn('Results');
    fireEvent.click(screen.getByTestId('pt-to-export'));
    await expectFocusOn('Export');
    expect(document.body.textContent).toContain(
      "Gaply writes only the file you name in the save dialog and keeps no copy. The result table holds the numbers, with each column's mean and SD marked first or second. The manifest holds hashes, versions and counts, never your data, headers or file name."
    );
    fireEvent.click(screen.getByTestId('pt-save'));
    expect((await screen.findByTestId('pt-export-note')).textContent).toBe('Saved. SHA-256 of the file: cdcdcdcdcdcd…');
    expect(save).toHaveBeenCalledWith('paired_t_test.csv', '"statistic","column","value","word"\r\n', 'text/csv');
    // First and second as the user chose them, never reordered.
    for (const cmd of ['stats_paired_validate', 'stats_paired_run']) {
      const sent = specs(bridge, cmd)[0];
      expect(sent.capabilityId, cmd).toBe('stats.analyze.paired_t');
      expect(sent.parameters, cmd).toEqual({ first: 'C2', second: 'C1', missingCodes: [] });
    }
    expect(bridge.calls[0]).toEqual(['stats_desc_open', { path: '<path>' }]);
    expect(document.body.innerHTML).not.toContain('PATH_SENTINEL');
  });

  it('offers both pickers with no default and every numeric column, says which way the difference goes, and checks once both are chosen', async () => {
    setup({ stats_paired_validate: report('valid') });
    await toSettings();
    const first = screen.getByTestId('pt-first') as HTMLSelectElement;
    const second = screen.getByTestId('pt-second') as HTMLSelectElement;
    expect(first.value).toBe('');
    expect(second.value).toBe('');
    // Numeric columns only, and the second list keeps the first's choice.
    expect(Array.from(first.options).map((o) => o.value)).toEqual(['', 'C1', 'C2']);
    expect(Array.from(second.options).map((o) => o.value)).toEqual(['', 'C1', 'C2']);
    expect(screen.getByTestId('pt-difference-line').textContent).toBe('The difference is first minus second.');
    const check = () => (screen.getByTestId('pt-check') as HTMLButtonElement).disabled;
    expect(check()).toBe(true);
    pick('first', 'C1');
    expect(check()).toBe(true);
    expect(Array.from(second.options).map((o) => o.value)).toEqual(['', 'C1', 'C2']);
    pick('second', 'C2');
    expect(check()).toBe(false);
    pick('first', '');
    expect(check()).toBe(true);
  });

  it('sends the same column twice, and shows Rust’s same_column refusal in its words', async () => {
    const { bridge } = setup({
      stats_paired_validate: report('invalid', {
        // As Rust sends it (stats_onesample/mod.rs:588): no parameters.
        errors: [{ code: 'stats.onesample.same_column', params: {}, severity: 'error' }],
      }),
    });
    await toSettings();
    pick('first', 'C1');
    pick('second', 'C1');
    fireEvent.click(screen.getByTestId('pt-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_paired_validate')[0].parameters).toEqual({ first: 'C1', second: 'C1', missingCodes: [] });
    expect(screen.getByTestId('pt-validation-errors').textContent).toBe(
      'The first and the second column are the same column. Choose two different columns.'
    );
    expect((screen.getByTestId('pt-run') as HTMLButtonElement).disabled).toBe(true);
  });

  it('offers each candidate missing code of either column once, with both counts, and applies it only when ticked', async () => {
    const { bridge } = setup({ stats_paired_validate: report('valid') });
    await toSettings();
    pick('first', 'C1');
    pick('second', 'C2');
    const fieldset = screen.getByTestId('pt-missing-codes');
    const questions = within(fieldset)
      .getAllByRole('checkbox')
      .map((b) => (b.closest('label')?.textContent ?? '').trim());
    expect(questions).toEqual([
      'Is -99 a code for a missing value? It appears in the first column "before" (2 cells) and in the second column "after" (1 cell). If you tick it, every pair in which either column holds -99 is left out.',
      'Is 999 a code for a missing value? It appears in the first column "before" (1 cell) and in the second column "after" (0 cells). If you tick it, every pair in which either column holds 999 is left out.',
      'Is 77 a code for a missing value? It appears in the first column "before" (0 cells) and in the second column "after" (3 cells). If you tick it, every pair in which either column holds 77 is left out.',
    ]);
    for (const q of questions) expect(q).not.toMatch(VERDICT);
    fireEvent.click(screen.getByTestId('pt-check'));
    await expectFocusOn('Check the settings');
    fireEvent.click(screen.getByTestId('pt-back-settings'));
    await expectFocusOn(SETTINGS);
    fireEvent.click(screen.getByTestId('pt-code-77'));
    fireEvent.click(screen.getByTestId('pt-code--99'));
    fireEvent.click(screen.getByTestId('pt-check'));
    await expectFocusOn('Check the settings');
    expect(specs(bridge, 'stats_paired_validate').map((s) => s.parameters.missingCodes)).toEqual([[], [77, -99]]);
  });

  it('Cancel asks Rust to stop the run, and shows the end it reports', async () => {
    let cancelled = false;
    const { bridge } = setup({
      stats_paired_validate: report('valid'),
      stats_paired_run: { runId: 6 },
      stats_paired_status: () => (cancelled ? state('cancelled', true, true) : state('running', false)),
      stats_paired_cancel: () => {
        cancelled = true;
        return state('running', false, true);
      },
    });
    await toSettings();
    pick('first', 'C1');
    pick('second', 'C2');
    await checkAndRun();
    fireEvent.click(await screen.findByTestId('pt-cancel'));
    expect((await screen.findByTestId('pt-run-ended')).textContent).toBe('This run was cancelled, so it has no result.');
    expect(bridge.calls.filter(([c]) => c === 'stats_paired_cancel')).toEqual([['stats_paired_cancel', { runId: 6 }]]);
    expect(screen.queryByTestId('pt-see-results')).toBeNull();
  });

  it('a failed run says why, in decision 259’s words, and offers Save to a project…', async () => {
    setup({
      stats_paired_validate: report('valid'),
      stats_paired_run: { runId: 6 },
      stats_paired_status: state('failed', true, false, [
        { code: 'stats.onesample.differences_essentially_constant', params: {}, severity: 'error' },
      ]),
    });
    await toSettings();
    pick('first', 'C1');
    pick('second', 'C2');
    await checkAndRun();
    expect((await screen.findByTestId('pt-run-ended')).textContent).toBe('The t-test could not be computed. The reason is below.');
    expect(screen.getByTestId('pt-run-diagnostics').textContent).toBe(
      'The differences vary so little, compared with the size of the values, that the standard error is lost in rounding, so t is not computed. This happens when every pair differs by the same amount, such as 0.1, which a computer stores with a tiny rounding error.'
    );
    expect(screen.getByTestId('pt-project-save-button')).toBeTruthy();
    expect(screen.queryByTestId('pt-see-results')).toBeNull();
  });

  it('shows a neutral result card from the fixture: the numbers, the exclusions, the limitations and the manifest', async () => {
    setup({
      stats_paired_validate: report('valid'),
      stats_paired_run: { runId: 6 },
      stats_paired_status: state('succeeded', true),
      stats_paired_result: result,
    });
    await toSettings();
    pick('first', 'C1');
    pick('second', 'C2');
    await checkAndRun();
    fireEvent.click(await screen.findByTestId('pt-see-results'));
    await expectFocusOn('Results');
    const card = screen.getByTestId('pt-result-card');
    const text = (id: string) => screen.getByTestId(id).textContent;
    expect(text('pt-n-pairs')).toBe('9');
    expect(text('pt-first-mean')).toBe('63.125');
    expect(text('pt-first-sd')).toBe('29.25');
    expect(text('pt-second-mean')).toBe('33.0625');
    expect(text('pt-second-sd')).toBe('22.75');
    expect(text('pt-mean-difference')).toBe('30.0625');
    expect(text('pt-sd-differences')).toBe('36');
    expect(text('pt-se')).toBe('12');
    expect(text('pt-ci')).toBe('2.375 to 57.75');
    expect(text('pt-t')).toBe('2.5');
    expect(text('pt-df')).toBe('8');
    expect(text('pt-p')).toBe('0.0375');
    expect(text('pt-dz')).toBe('0.8125');
    expect(text('pt-d-no-interval')).toBe("Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.");
    // The rows name the columns by role, never by header, so a saved card
    // reads as the live one.
    const numbers = screen.getByTestId('pt-result-numbers');
    const terms = within(numbers)
      .getAllByRole('term')
      .map((t) => t.textContent);
    expect(terms).toEqual([
      'Pairs',
      'First column: mean',
      'First column: SD',
      'Second column: mean',
      'Second column: SD',
      'Mean difference (first minus second)',
      'SD of the differences',
      'Standard error',
      '95% confidence interval',
      't',
      'Degrees of freedom',
      'p (two-sided)',
      "Cohen's d_z",
    ]);
    expect(numbers.textContent).not.toMatch(/before|after/);
    expect(text('pt-result-notes')).toBe(
      'The column "before" holds 99 in 2 cells, which may be a code for a missing value. It is counted as a number unless you tick it in Settings.'
    );
    expect(text('pt-exclusions')).toContain('1 row had no second value (an empty cell or a missing-value token), and was left out.');
    expect(within(screen.getByTestId('pt-limitations')).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      'Normality is not assessed: the paired t-test assumes the differences come from a roughly normal distribution, or that there are many pairs, and Gaply does not check either.',
      "Cohen's d is shown without a confidence interval: Gaply does not compute one for d yet.",
    ]);
    expect(text('pt-manifest-input')).toBe('0123456789ab…');
    expect(text('pt-manifest-capability')).toBe('stats.analyze.paired_t, version 1');
    expect(text('pt-manifest-rows-read')).toBe('10');
    expect(text('pt-manifest-rows-used')).toBe('9');
    expect(text('pt-manifest-output')).toBe('5fd0af5daee4…');
    expect(screen.getByTestId('pt-manifest').textContent).toContain('The pairs used. Rows read is rows used plus the rows left out above.');
    expect(screen.getByTestId('pt-project-save-button')).toBeTruthy();
    // Neutral (decision 66): no verdict word and no star, anywhere on screen.
    expect(document.body.textContent ?? '').not.toMatch(VERDICT);
    expect(card.getAttribute('data-tone')).toBe('neutral');
  });

  it('words a refusal from Rust and keeps the screen where it was', async () => {
    const refusal = Object.assign(new Error(JSON.stringify({ code: 'run.wrong_kind' })), { code: 'validation' });
    setup({
      stats_paired_validate: () => {
        throw refusal;
      },
    });
    await toSettings();
    pick('first', 'C1');
    pick('second', 'C2');
    fireEvent.click(screen.getByTestId('pt-check'));
    expect((await screen.findByTestId('pt-error')).textContent).toBe('This run is not of this test, so this screen cannot read it.');
    expect(heading()?.textContent).toBe(SETTINGS);
  });

  it('closes the file, and Rust drops its rows, when the screen goes', async () => {
    const { bridge } = setup({});
    fireEvent.click(screen.getByTestId('pt-choose'));
    await expectFocusOn('Profile of the file');
    cleanup();
    await waitFor(() => expect(bridge.calls.some(([c, a]) => c === 'stats_desc_close' && a.slot === 3)).toBe(true));
  });
});
