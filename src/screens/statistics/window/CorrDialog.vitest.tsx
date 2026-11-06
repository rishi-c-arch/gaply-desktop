// The correlations' dialog (Research OS, Phase 8S, decisions 481 and 484; ADR-012 Part 6),
// over a mock bridge. The twins of CorrFlow.vitest.tsx's tests at :119, :144, :168 and
// :183. Every assertion on its test ids and its spec is that file's. What differs:
// * toSettings becomes the dialog's opening, and cr-check its Run;
// * a line on the stage heading becomes "the dialog is still open" (:183).
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import CorrFlow from '../CorrFlow';
import { makeMockCorrBridge } from '../corrBridge';
import type { CorrRunStateView, CorrSpec, CorrValidationView } from '../corrTypes';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { CorrDialog } from './CorrDialog';
import { OUTPUT_FULL } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const col = (alias: string, header: string, codes: Array<[number, number]> = [], kind: ColumnProfile['kind'] = 'numeric'): ColumnProfile => ({
  alias,
  header,
  kind,
  n: 200,
  missing: 0,
  distinct: kind === 'numeric' ? 150 : 3,
  levels: kind === 'numeric' ? null : [{ label: 'x', count: 200 }],
  suspectMissingCodes: codes.map(([code, cells]) => ({ code, cells })),
  nonFinite: 0,
});

const view: DatasetView = {
  slot: 3,
  displayName: 'thesis.csv',
  profile: {
    rows: 200,
    columns: [
      col('C1', 'Trust', [[-9, 2]]),
      col('C2', 'Group', [], 'categorical'),
      col('C3', 'Loyalty', [[99, 4]]),
      col('C4', 'Price', [
        [99, 1],
        [-99, 3],
      ]),
      col('C5', 'Quality'),
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 200, columns: 5, semanticRole: 'table' },
};

const report = (status: CorrValidationView['status'], errors: CorrValidationView['errors'] = []): CorrValidationView => ({ status, errors, warnings: [], missingInformation: [], estimatedResources: { rows: 200, columns: 4, cells: 800 } });
const running: CorrRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 1200, cancelRequested: false, stepsDone: 3, stepsTotal: 19, diagnostics: [] };

function setup(responses: Record<string, unknown>, over: { outputFull?: boolean } = {}) {
  const bridge = makeMockCorrBridge({ stats_corr_run: { runId: 5 }, stats_corr_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<CorrDialog view={view} slot={3} bridge={bridge} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockCorrBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as CorrSpec);
const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const run = () => screen.getByTestId('cr-run') as HTMLButtonElement;

describe('the twins of CorrFlow.vitest.tsx', () => {
  it('opens with no column, no coefficient and no handling chosen, and Run off with what is missing', () => {
    setup({});
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`cr-variable-${a}`).checked, a).toBe(false);
    expect(screen.queryByTestId('cr-variable-C2'), 'a text column is not offered').toBeNull();
    for (const c of ['pearson', 'spearman', 'kendall']) expect(box(`cr-coefficient-${c}`).checked, c).toBe(false);
    for (const m of ['pairwise', 'listwise']) expect(box(`cr-missing-${m}`).checked, m).toBe(false);
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('cr-settings-missing').textContent).toBe('To check the settings, choose two variables, a coefficient and how missing values are handled.');
    expect(screen.getByTestId('cr-missing').textContent).toContain('Each pair uses every row in which both of its columns have a value');
    expect(screen.getByTestId('cr-missing').textContent).toContain('Every pair uses only the rows in which every ticked column has a value');
    fireEvent.click(screen.getByTestId('cr-variable-C3'));
    expect(screen.getByTestId('cr-settings-missing').textContent).toBe('To check the settings, choose one more variable, a coefficient and how missing values are handled.');
    fireEvent.click(screen.getByTestId('cr-variable-C1'));
    fireEvent.click(screen.getByTestId('cr-coefficient-kendall'));
    expect(run().disabled).toBe(true);
    fireEvent.click(screen.getByTestId('cr-missing-listwise'));
    expect(screen.queryByTestId('cr-settings-missing')).toBeNull();
    expect(run().disabled).toBe(false);
  });

  it('ticks and unticks every numeric column only when asked, and sends one spec per choice', async () => {
    const { bridge } = setup({ stats_corr_validate: report('valid') });
    fireEvent.click(screen.getByTestId('cr-tick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`cr-variable-${a}`).checked, a).toBe(true);
    fireEvent.click(screen.getByTestId('cr-untick-all'));
    for (const a of ['C1', 'C3', 'C4', 'C5']) expect(box(`cr-variable-${a}`).checked, a).toBe(false);
    for (const a of ['C5', 'C1', 'C3']) fireEvent.click(screen.getByTestId(`cr-variable-${a}`));
    for (const c of ['kendall', 'pearson']) fireEvent.click(screen.getByTestId(`cr-coefficient-${c}`));
    fireEvent.click(screen.getByTestId('cr-missing-pairwise'));
    fireEvent.click(screen.getByTestId('cr-code-99'));
    fireEvent.click(screen.getByTestId('cr-code--9'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_corr_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_corr_validate')[0].parameters).toEqual({
      variables: ['C1', 'C3', 'C5'],
      coefficients: ['pearson', 'kendall'],
      missing: 'pairwise',
      missingCodes: [-9, 99],
    });
  });

  it('offers the candidate codes of the ticked columns only, ascending, each with where it appears', () => {
    setup({});
    expect(screen.queryByTestId('cr-missing-codes')).toBeNull();
    fireEvent.click(screen.getByTestId('cr-variable-C4'));
    fireEvent.click(screen.getByTestId('cr-variable-C3'));
    const offered = Array.from(screen.getByTestId('cr-missing-codes').querySelectorAll('input')).map((i) => i.getAttribute('data-testid'));
    expect(offered).toEqual(['cr-code--99', 'cr-code-99']);
    expect(screen.getByTestId('cr-missing-codes').textContent).toContain('It appears in "Loyalty" (4 cells), "Price" (1 cell).');
    fireEvent.click(screen.getByTestId('cr-code--99'));
    fireEvent.click(screen.getByTestId('cr-variable-C4'));
    expect(screen.queryByTestId('cr-code--99')).toBeNull();
  });

  it('shows the budget sentence with the pairs and the rows at Run, stays open, and never runs', async () => {
    const { bridge, onStarted } = setup({
      stats_corr_validate: report('invalid', [{ code: 'stats.corr.over_budget', params: { count: 2340, found: 100000 }, severity: 'error' }]),
    });
    fireEvent.click(screen.getByTestId('cr-tick-all'));
    for (const c of ['pearson', 'spearman', 'kendall']) fireEvent.click(screen.getByTestId(`cr-coefficient-${c}`));
    fireEvent.click(screen.getByTestId('cr-missing-pairwise'));
    fireEvent.click(run());
    await screen.findByTestId('cr-validation-errors');
    expect(screen.getByTestId('sw-method-dialog')).toBeTruthy();
    expect(screen.getByTestId('cr-validation-errors').textContent).toContain(
      'These settings are refused before they run: 2,340 pairs over 100,000 rows are estimated to take longer than 15 seconds on this computer. Choose fewer variables or fewer coefficients.'
    );
    expect(run().disabled).toBe(true);
    fireEvent.click(run());
    expect(bridge.calls.filter(([c]) => c === 'stats_corr_run')).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe('its two buttons (decision 484)', () => {
  it('read as alpha\'s do, in the dialog and on the page', async () => {
    const { unmount } = setup({});
    expect(screen.getByTestId('cr-tick-all').textContent).toBe('Add every numeric column');
    expect(screen.getByTestId('cr-untick-all').textContent).toBe('Remove all');
    unmount();
    const page = makeMockCorrBridge({ stats_desc_open: view, stats_desc_close: true });
    render(<CorrFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('cr-choose'));
    fireEvent.click(await screen.findByTestId('cr-to-settings'));
    expect((await screen.findByTestId('cr-tick-all')).textContent).toBe('Add every numeric column');
    expect(screen.getByTestId('cr-untick-all').textContent).toBe('Remove all');
  });
});

describe('one set of choices gives one spec', () => {
  const choose = () => {
    for (const a of ['C5', 'C1']) fireEvent.click(screen.getByTestId(`cr-variable-${a}`));
    fireEvent.click(screen.getByTestId('cr-coefficient-kendall'));
    fireEvent.click(screen.getByTestId('cr-missing-listwise'));
    fireEvent.click(screen.getByTestId('cr-code--9'));
  };

  it("equal to the page's for the same choices", async () => {
    const { bridge, unmount } = setup({ stats_corr_validate: report('valid') });
    choose();
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_corr_validate')).toHaveLength(1));
    const fromDialog = specs(bridge, 'stats_corr_validate')[0];
    unmount();

    const page = makeMockCorrBridge({ stats_desc_open: view, stats_desc_close: true, stats_corr_validate: report('valid') });
    render(<CorrFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('cr-choose'));
    fireEvent.click(await screen.findByTestId('cr-to-settings'));
    await screen.findByTestId('cr-variable-C1');
    choose();
    fireEvent.click(screen.getByTestId('cr-check'));
    await waitFor(() => expect(specs(page, 'stats_corr_validate')).toHaveLength(1));
    expect({ ...fromDialog, operationId: 0 }).toEqual({ ...specs(page, 'stats_corr_validate')[0], operationId: 0 });
  });
});

describe("MethodDialog's rule, for the correlations", () => {
  const choices = () => {
    for (const a of ['C1', 'C5']) fireEvent.click(screen.getByTestId(`cr-variable-${a}`));
    fireEvent.click(screen.getByTestId('cr-coefficient-pearson'));
    fireEvent.click(screen.getByTestId('cr-missing-pairwise'));
  };

  it('valid: hands the run to its parent, with its own settings line', async () => {
    const { onStarted } = setup({ stats_corr_validate: report('valid') });
    choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const s = onStarted.mock.calls[0][0];
    expect(s.runId).toBe(5);
    expect(s.settings).toContain('Variables: C1 Trust, C5 Quality.');
    expect(s.settings).toContain('Missing codes: none.');
  });

  it('with the Output full, Run says so and nothing is validated or run', async () => {
    const { bridge, onStarted } = setup({}, { outputFull: true });
    choices();
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe(OUTPUT_FULL);
    expect(bridge.calls).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('never closes the slot: not on mount, Cancel, Run or unmount', async () => {
    const { bridge, onStarted, unmount } = setup({ stats_corr_validate: report('valid') });
    const closes = () => bridge.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    choices();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('cr-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render CorrSettings, by import, and neither holds a control', () => {
    for (const f of ['../CorrFlow.tsx', 'CorrDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bCorrSettings\b[^}]*\buseCorrSettings\b[^}]*\} from '[^']*CorrSettings'/);
      expect(src(f), f).toMatch(/<CorrSettings\b/);
      expect(src(f), f).not.toMatch(/cr-variable-|cr-coefficient-\$\{|cr-tick-all|cr-code-/);
    }
    expect(src('../CorrSettings.tsx')).toMatch(/cr-variable-/);
    expect(src('MethodDialog.tsx')).not.toMatch(/cr-variable-/);
  });

  it('its bridge type omits open and close, and it names no close', () => {
    expect(src('CorrDialog.tsx')).toMatch(/Omit<CorrBridge, 'open' \| 'close'>/);
    for (const f of ['CorrDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });
});
