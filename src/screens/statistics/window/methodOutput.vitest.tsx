// The Output for every method (Research OS, Phase 8S, decision 479; ADR-012 Part
// 7): one table keyed by the method's kind, whose row gives each entry its bridge,
// names, sentences, progress, result card and export. Every bridge is a mock.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockAlphaBridge } from '../alphaBridge';
import { makeMockAnovaBridge } from '../anovaBridge';
import { makeMockCorrBridge } from '../corrBridge';
import { makeMockDescribeBridge } from '../describeBridge';
import type { DatasetView } from '../describeTypes';
import { alphaResultView } from '../fixtures/alphaFixtures';
import { anovaResultView } from '../fixtures/anovaFixtures';
import { corrResultView } from '../fixtures/corrFixtures';
import { exportView, resultBundleView } from '../fixtures/describeFixtures';
import onesampleResult from '../fixtures/onesampleResultView.json';
import pairedResult from '../fixtures/pairedResultView.json';
import { ttestResultView } from '../fixtures/ttestFixtures';
import { xtabResultView } from '../fixtures/xtabFixtures';
import { ranksResultView } from '../fixtures/ranksFixtures';
import { METHODS } from '../methods';
import { makeMockOneSampleBridge, makeMockPairedBridge } from '../onesampleBridge';
import { makeMockProjectBridge } from '../projects/projectBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { makeMockXtabBridge } from '../xtabBridge';
import { makeMockRanksBridge } from '../ranksBridge';
import { METHOD_OUTPUT } from './methodOutput';
import { Output, OutputWatchers } from './Output';
import { statisticsSession, type NewOutputEntry, type OutputEntry } from './statisticsSession';

vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
});

const view: DatasetView = {
  slot: 1,
  displayName: 'f.csv',
  profile: {
    rows: 9,
    columns: [
      { alias: 'C1', header: 'Height', kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 },
      { alias: 'C2', header: 'C2', scoreName: 'Total anxiety', prepared: true, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: 'a'.repeat(64), rows: 9, columns: 2, semanticRole: 'table' },
};

const common = { runId: 5, state: 'running', terminal: false, elapsedMs: 10, cancelRequested: false, diagnostics: [] };
const running = { ...common, stepsDone: 3, stepsTotal: 7 };
const summaryRunning = { ...common, columnsDone: 1, columnsTotal: 2 };

/** One row per kind: its bridge's key, its commands' prefix, a result, a run, its card. */
const KINDS = [
  { kind: 'describe', key: 'describe', prefix: 'stats_desc', result: resultBundleView, run: summaryRunning, card: 'desc-manifest', progress: '1 of 2 columns summarised.' },
  { kind: 'ttest', key: 'ttest', prefix: 'stats_ttest', result: ttestResultView, run: common, card: 'tt-result-card', progress: null },
  { kind: 'one_sample', key: 'oneSample', prefix: 'stats_onesample', result: onesampleResult, run: common, card: 'os-result-card', progress: null },
  { kind: 'paired', key: 'paired', prefix: 'stats_paired', result: pairedResult, run: common, card: 'pt-result-card', progress: null },
  { kind: 'anova', key: 'anova', prefix: 'stats_anova', result: anovaResultView, run: running, card: 'an-result-card', progress: '3 of 7 steps' },
  { kind: 'correlations', key: 'corr', prefix: 'stats_corr', result: corrResultView, run: running, card: 'cr-result-card', progress: '3 of 7 steps' },
  { kind: 'cronbach_alpha', key: 'alpha', prefix: 'stats_alpha', result: alphaResultView, run: running, card: 'al-result-card', progress: '3 of 7 steps' },
  // Research OS, Phase 8X-2 (decisions 588 and 591): one check point, so no steps sentence.
  { kind: 'crosstabs', key: 'xtab', prefix: 'stats_xtab', result: xtabResultView, run: { ...common, stepsDone: 0, stepsTotal: 1 }, card: 'xt-result-card', progress: null },
  // Research OS, Phase 8Y-2 (decision 622): one check point, so no steps sentence.
  { kind: 'independent_ranks', key: 'ranks', prefix: 'stats_ranks', result: ranksResultView, run: { ...common, stepsDone: 0, stepsTotal: 1 }, card: 'rk-result-card', progress: null },
] as const;

function bridgesWith(prefix: string, responses: Record<string, unknown>) {
  const scoped = Object.fromEntries(Object.entries(responses).map(([k, v]) => [`${prefix}_${k}`, v]));
  return {
    describe: makeMockDescribeBridge(prefix === 'stats_desc' ? scoped : {}),
    ttest: makeMockTTestBridge(prefix === 'stats_ttest' ? scoped : {}),
    oneSample: makeMockOneSampleBridge(prefix === 'stats_onesample' ? scoped : {}),
    paired: makeMockPairedBridge(prefix === 'stats_paired' ? scoped : {}),
    anova: makeMockAnovaBridge(prefix === 'stats_anova' ? scoped : {}),
    corr: makeMockCorrBridge(prefix === 'stats_corr' ? scoped : {}),
    alpha: makeMockAlphaBridge(prefix === 'stats_alpha' ? scoped : {}),
    xtab: makeMockXtabBridge(prefix === 'stats_xtab' ? scoped : {}),
    ranks: makeMockRanksBridge(prefix === 'stats_ranks' ? scoped : {}),
  };
}

function shown(entry: Partial<OutputEntry> & NewOutputEntry, bridges: ReturnType<typeof bridgesWith>, save = vi.fn().mockResolvedValue('/chosen')) {
  const Harness: React.FC = () => {
    const [, force] = React.useState(0);
    React.useEffect(() => statisticsSession.subscribe(() => force((x) => x + 1)), []);
    return (
      <>
        <OutputWatchers entries={statisticsSession.get().output} bridges={bridges} pollMs={5} />
        <Output session={statisticsSession.get()} bridges={bridges} forget={makeMockDescribeBridge()} projects={makeMockProjectBridge()} save={save} status={null} />
      </>
    );
  };
  act(() => {
    statisticsSession.setFile(view);
    const { result, ...fresh } = entry;
    const n = statisticsSession.addEntry(fresh as NewOutputEntry) as number;
    if (result) statisticsSession.updateEntry(n, { result });
  });
  render(<Harness />);
  return { save };
}

describe('METHOD_OUTPUT', () => {
  it('has a row for every METHODS row and no other', () => {
    expect(Object.keys(METHOD_OUTPUT).sort()).toEqual(METHODS.map((m) => m.kind).sort());
    expect(KINDS.map((k) => k.kind).slice().sort()).toEqual(METHODS.map((m) => m.kind).sort());
  });

  it.each(KINDS)("$kind's row takes its own bridge from the window's", ({ kind, key, prefix }) => {
    const bridges = bridgesWith(prefix, {});
    expect(METHOD_OUTPUT[kind].bridge(bridges)).toBe(bridges[key]);
  });

  it('names no open and no close, and writes no bridge of its own', () => {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const src = require('node:fs').readFileSync(require('node:path').join(__dirname, 'methodOutput.tsx'), 'utf8') as string;
    expect(src).not.toMatch(/\.(open|close)\(|invoke\(|stats_[a-z]+_/);
  });
});

describe.each(KINDS)('an entry of $kind', ({ kind, prefix, result, run, card, progress }) => {
  const entry = (over: Record<string, unknown> = {}) =>
    ({ kind, view, slot: 1, settings: 'S.', runId: 5, run, ...over } as unknown as NewOutputEntry & Partial<OutputEntry>);

  it('while it runs: its state, its progress in what it counts, and Cancel through its own bridge', async () => {
    let asked = false;
    const bridges = bridgesWith(prefix, {
      status: () => ({ ...run, cancelRequested: asked }),
      cancel: () => {
        asked = true;
        return { ...run, cancelRequested: true };
      },
    });
    shown(entry(), bridges);
    const e = await screen.findByTestId('sw-entry-1');
    if (progress) expect(within(e).getByTestId('sw-entry-1-progress').textContent).toContain(progress);
    else expect(within(e).queryByTestId('sw-entry-1-progress')).toBeNull();
    fireEvent.click(within(e).getByTestId('sw-entry-1-cancel'));
    const calls = (METHOD_OUTPUT[kind].bridge(bridges) as unknown as { calls: Array<[string, unknown]> }).calls;
    await waitFor(() => expect(calls.some(([c]) => c === `${prefix}_cancel`)).toBe(true));
    await waitFor(() => expect(statisticsSession.get().output[0].run.cancelRequested).toBe(true));
  });

  it('reads its run through its own status and its result through its own result, whichever tab is shown', async () => {
    const done = { ...run, state: 'succeeded', terminal: true };
    const bridges = bridgesWith(prefix, { status: done, result });
    shown(entry(), bridges);
    await waitFor(() => expect(statisticsSession.get().output[0].result).not.toBeNull());
    expect(await screen.findByTestId(card)).toBeTruthy();
  });

  it('draws its own card, and Export CSV… goes through its own export and saveExactly', async () => {
    const bridges = bridgesWith(prefix, { export: exportView });
    const { save } = shown(entry({ run: { ...run, state: 'succeeded', terminal: true }, result }), bridges);
    expect(await screen.findByTestId(card)).toBeTruthy();
    fireEvent.click(screen.getByTestId('sw-entry-1-export'));
    expect((screen.getByTestId('sw-entry-1-export-csv') as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId('sw-entry-1-export-manifestJson')).toBeTruthy();
    fireEvent.click(screen.getByTestId('sw-entry-1-save'));
    await screen.findByTestId('sw-entry-1-export-note');
    const calls = (METHOD_OUTPUT[kind].bridge(bridges) as unknown as { calls: Array<[string, unknown]> }).calls;
    expect(calls.filter(([c]) => c === `${prefix}_export`)).toEqual([[`${prefix}_export`, { runId: 5, format: 'csv' }]]);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it('a failed run says so in its own words, with the Save its code allows', async () => {
    const failed = { ...run, state: 'failed', terminal: true, diagnostics: [{ code: `${prefix}.nothing`, severity: 'error', params: {} }] };
    const bridges = bridgesWith(prefix, {});
    shown(entry({ run: failed }), bridges);
    const said = await screen.findByTestId('sw-entry-1-run-ended');
    expect(said.textContent).toBe(METHOD_OUTPUT[kind].runEnded('failed'));
    expect(screen.getByTestId('sw-entry-1-diagnostics')).toBeTruthy();
    expect(screen.getByTestId('sw-entry-1-project-button')).toBeTruthy();
  });
});

describe('names in a card and a sentence', () => {
  const failing = (kind: string, code: string, params: Record<string, number>) =>
    ({ kind, view, slot: 1, settings: 'S.', runId: 5, run: { ...common, state: 'failed', terminal: true, diagnostics: [{ code, severity: 'error', params }] } } as unknown as NewOutputEntry);

  it("a column by the view's own name, a score by what the user typed", () => {
    expect(METHOD_OUTPUT.correlations.sentence({ code: 'stats.corr.unknown_column', params: { column: 2 }, severity: 'error' } as never, view)).toContain('"Total anxiety"');
    expect(METHOD_OUTPUT.correlations.sentence({ code: 'stats.corr.unknown_column', params: { column: 1 }, severity: 'error' } as never, view)).toContain('"Height"');
  });

  it('a level before any result reads "level n+1", as a saved failure does', async () => {
    shown(failing('anova', 'stats.anova.group_too_small', { index: 2, count: 1 }), bridgesWith('stats_anova', {}));
    const list = await screen.findByTestId('sw-entry-1-diagnostics');
    expect(list.textContent).toContain('level 3');
  });

  it("a t-test's and an ANOVA's card name levels by the result's own labels", async () => {
    const label = ttestResultView.result.groups[0].label;
    shown({ kind: 'ttest', view, slot: 1, settings: 'S.', runId: 5, run: { ...common, state: 'succeeded', terminal: true }, result: ttestResultView } as never, bridgesWith('stats_ttest', {}));
    expect((await screen.findByTestId('tt-result-card')).textContent).toContain(label);
  });
});
