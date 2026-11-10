// Each method from the Analyze menu, through its dialog, into the Output, with its card,
// its Save and its Export (Research OS, Phase 8S, decisions 481 to 486), in the window,
// over mock bridges. One table, one row per METHODS row. Nothing touches Rust.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
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
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import { makeMockCapabilityBridge, makeMockTTestBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import { makeMockXtabBridge } from '../xtabBridge';
import { makeMockRanksBridge } from '../ranksBridge';
import { METHOD_DIALOGS, METHOD_OUTPUT } from './methodOutput';
import { statisticsSession } from './statisticsSession';
import { StatisticsWindow } from './StatisticsWindow';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

const num = (alias: string, header: string) => ({ alias, header, kind: 'numeric' as const, n: 9, missing: 0, distinct: 7, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const view: DatasetView = {
  slot: 1,
  displayName: 'f.csv',
  profile: {
    rows: 9,
    columns: [num('C1', 'Height'), num('C2', 'Weight'), { alias: 'C3', header: 'Arm', kind: 'categorical', n: 9, missing: 0, distinct: 2, levels: [{ label: 'a', count: 5 }, { label: 'b', count: 4 }], suspectMissingCodes: [], nonFinite: 0 }],
  },
  inputRef: { contentHash: 'a'.repeat(64), rows: 9, columns: 3, semanticRole: 'table' },
};

const valid = { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 2, cells: 18 } };
const done = { runId: 5, state: 'succeeded', terminal: true, elapsedMs: 10, cancelRequested: false, stepsDone: 7, stepsTotal: 7, columnsDone: 1, columnsTotal: 1, diagnostics: [] };
const levels = ({ slot, column }: Record<string, unknown>) => ({ slot, column, levels: ['a', 'b'].map((label, index) => ({ index, label, count: 4 })) });
const fill = (prefix: string, result: unknown, extra: Record<string, unknown> = {}) => ({
  [`${prefix}_validate`]: valid,
  [`${prefix}_run`]: { runId: 5 },
  [`${prefix}_status`]: done,
  [`${prefix}_result`]: result,
  [`${prefix}_export`]: exportView,
  ...extra,
});
const select = (id: string, value: string) => fireEvent.change(screen.getByTestId(id), { target: { value } });
const click = (id: string) => fireEvent.click(screen.getByTestId(id));

const ROWS = [
  {
    kind: 'describe',
    run: 'desc-run',
    card: 'desc-manifest',
    prefix: 'stats_desc',
    choose: async () => click('desc-include-C1'),
    line: 'Columns: C1 Height as numeric. Missing codes: none.',
  },
  {
    kind: 'ttest',
    run: 'tt-run',
    card: 'tt-result-card',
    prefix: 'stats_ttest',
    choose: async () => {
      select('tt-outcome', 'C1');
      select('tt-group', 'C3');
      await screen.findByTestId('tt-levels');
      click('tt-variant-welch');
    },
    line: "Outcome: C1 Height. Group: C3 Arm. Group 1: a. Group 2: b. Test: Welch's t-test. Missing codes: none.",
  },
  {
    kind: 'one_sample',
    run: 'os-run',
    card: 'os-result-card',
    prefix: 'stats_onesample',
    choose: async () => {
      select('os-outcome', 'C1');
      select('os-test-value', '3');
    },
    line: 'Column: C1 Height. Test value: 3. Missing codes: none.',
  },
  {
    kind: 'paired',
    run: 'pt-run',
    card: 'pt-result-card',
    prefix: 'stats_paired',
    choose: async () => {
      select('pt-first', 'C1');
      select('pt-second', 'C2');
    },
    line: 'First column: C1 Height. Second column: C2 Weight. Missing codes: none.',
  },
  {
    kind: 'anova',
    run: 'an-run',
    card: 'an-result-card',
    prefix: 'stats_anova',
    choose: async () => {
      select('an-outcome', 'C1');
      select('an-group', 'C3');
      await screen.findByTestId('an-level-1');
    },
    line: 'Outcome: C1 Height. Group: C3 Arm. Levels: all. Post hoc: none. Missing codes: none.',
  },
  {
    kind: 'correlations',
    run: 'cr-run',
    card: 'cr-result-card',
    prefix: 'stats_corr',
    choose: async () => {
      click('cr-tick-all');
      click('cr-coefficient-pearson');
      click('cr-missing-pairwise');
    },
    line: null,
  },
  {
    kind: 'cronbach_alpha',
    run: 'al-run',
    card: 'al-result-card',
    prefix: 'stats_alpha',
    choose: async () => click('al-tick-all'),
    line: null,
  },
  // Research OS, Phase 8X-2 (decision 591).
  {
    kind: 'crosstabs',
    run: 'xt-run',
    card: 'xt-result-card',
    prefix: 'stats_xtab',
    choose: async () => {
      select('xt-rows', 'C3');
      select('xt-columns', 'C1');
    },
    line: 'Rows: C3 Arm. Columns: C1 Height. Missing codes: none.',
  },
  // Research OS, Phase 8Y-2 (decision 622).
  {
    kind: 'independent_ranks',
    run: 'rk-run',
    card: 'rk-result-card',
    prefix: 'stats_ranks',
    choose: async () => {
      select('rk-outcome', 'C1');
      select('rk-group', 'C3');
      await screen.findByTestId('rk-level-1');
    },
    line: 'Outcome: C1 Height. Group: C3 Arm. Levels: all. Missing codes: none.',
  },
] as const;

function setUp() {
  const describeB = makeMockDescribeBridge({ stats_desc_open: view, stats_desc_close: true, stats_desc_forget_run: null, ...fill('stats_desc', resultBundleView) });
  const cells = makeMockDescribeBridge({ stats_desc_cells: (a: Record<string, unknown>) => ({ slot: a.slot, inputHash: 'a'.repeat(64), rows: 9, columns: 3, firstRow: a.firstRow, firstColumn: a.firstColumn, cells: [] }) });
  const runs = {
    describe: describeB,
    ttest: makeMockTTestBridge({ stats_ttest_group_levels: levels, ...fill('stats_ttest', ttestResultView) }),
    oneSample: makeMockOneSampleBridge(fill('stats_onesample', onesampleResult)),
    paired: makeMockPairedBridge(fill('stats_paired', pairedResult)),
    anova: makeMockAnovaBridge({ stats_anova_group_levels: levels, ...fill('stats_anova', anovaResultView) }),
    corr: makeMockCorrBridge(fill('stats_corr', corrResultView)),
    alpha: makeMockAlphaBridge(fill('stats_alpha', alphaResultView)),
    xtab: makeMockXtabBridge(fill('stats_xtab', xtabResultView)),
    ranks: makeMockRanksBridge({ stats_ranks_group_levels: levels, ...fill('stats_ranks', ranksResultView) }),
  };
  const capabilities: CapabilityView[] = METHODS.map((m) => ({ id: m.capabilityId, version: 1, lifecycle: 'beta', titleCode: 'stats.capability.describe.title', descriptionCode: 'stats.capability.describe.description', licenseNotes: [], knownLimitations: [], runnable: true }));
  const save = vi.fn().mockResolvedValue('/chosen/out.csv');
  render(
    <MemoryRouter initialEntries={['/w']}>
      <Routes>
        <Route
          path="/w"
          element={
            <StatisticsWindow
              workspace={makeMockWorkspaceBridge({ workspace_state: { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null } })}
              files={describeB}
              forget={describeB}
              cells={cells}
              capabilities={makeMockCapabilityBridge({ capability_list: capabilities })}
              projects={makeMockProjectBridge({ [C.current]: { projectId: null, rerun: null }, [C.list]: [], [C.saveRun]: { projectId: 'p'.repeat(32), storedRunId: 1, created: true } })}
              runs={runs}
              save={save}
              pickPath={() => Promise.resolve('/x/f.csv')}
              limits={() => Promise.resolve(null)}
              pollMs={15}
              runPollMs={5}
            />
          }
        />
      </Routes>
    </MemoryRouter>
  );
  return { runs, save, describeB };
}

const openFromMenu = async (kind: string) => {
  for (let i = 0; i < 200; i += 1) {
    fireEvent.click(screen.getByTestId('gds-menu-analyze'));
    const item = screen.getByTestId(`gds-menu-item-analyze-${kind}`);
    if (item.getAttribute('aria-disabled') !== 'true') {
      fireEvent.click(item);
      return;
    }
    fireEvent.click(screen.getByTestId('gds-menu-analyze'));
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });
  }
  throw new Error(`${kind} never came on`);
};

describe('the tables', () => {
  it('have a dialog and an Output row for every kind', () => {
    expect(Object.keys(METHOD_DIALOGS).sort()).toEqual(METHODS.map((m) => m.kind).sort());
    expect(Object.keys(METHOD_OUTPUT).sort()).toEqual(METHODS.map((m) => m.kind).sort());
    expect(ROWS.map((r) => r.kind).slice().sort()).toEqual(METHODS.map((m) => m.kind).sort());
  });
});

describe.each(ROWS)('$kind, from the menu to the Output', ({ kind, run, card, prefix, choose, line }) => {
  it('opens over the data with Run off, runs, closes, and its entry has its card, its Save and its Export', async () => {
    const { save, describeB } = setUp();
    fireEvent.click(await screen.findByTestId('sw-open'));
    await screen.findByTestId('sw-file-summary');
    const opens = describeB.calls.filter(([c]) => c === 'stats_desc_open').length;
    await openFromMenu(kind);
    await screen.findByTestId('sw-method-dialog');
    expect(screen.getByTestId('sw-grid'), 'the data stays behind').toBeTruthy();
    expect((screen.getByTestId(run) as HTMLButtonElement).disabled).toBe(true);

    await choose();
    await waitFor(() => expect((screen.getByTestId(run) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByTestId(run));
    await waitFor(() => expect(screen.queryByTestId('sw-method-dialog')).toBeNull());

    expect(screen.getByRole('tab', { name: /Output \(1\)/ }).getAttribute('aria-selected')).toBe('true');
    expect(await within(await screen.findByTestId('sw-entry-1')).findByTestId(card)).toBeTruthy();
    expect(statisticsSession.get().output[0].kind).toBe(kind);
    expect(screen.getByTestId('sw-entry-1-heading').textContent).toBe(`1 ${METHODS.find((m) => m.kind === kind)?.name}`);
    if (line) expect(screen.getByTestId('sw-entry-1-settings').textContent).toBe(`Settings: ${line}`);
    expect(screen.getByTestId('sw-entry-1-project-button')).toBeTruthy();

    fireEvent.click(screen.getByTestId('sw-entry-1-export'));
    fireEvent.click(screen.getByTestId('sw-entry-1-save'));
    await screen.findByTestId('sw-entry-1-export-note');
    expect(save).toHaveBeenCalledTimes(1);
    expect(prefix.startsWith('stats_')).toBe(true);

    // The dialog neither opened nor closed anything.
    expect(describeB.calls.filter(([c]) => c === 'stats_desc_open')).toHaveLength(opens);
    expect(describeB.calls.filter(([c]) => c === 'stats_desc_close')).toEqual([]);
  });

  it('Cancel drops the settings, and reopening starts empty', async () => {
    setUp();
    fireEvent.click(await screen.findByTestId('sw-open'));
    await screen.findByTestId('sw-file-summary');
    await openFromMenu(kind);
    await screen.findByTestId('sw-method-dialog');
    await choose();
    fireEvent.keyDown(screen.getByTestId('sw-method-dialog'), { key: 'Escape' });
    await waitFor(() => expect(screen.queryByTestId('sw-method-dialog')).toBeNull());
    await openFromMenu(kind);
    await screen.findByTestId('sw-method-dialog');
    expect((screen.getByTestId(run) as HTMLButtonElement).disabled).toBe(true);
    expect(statisticsSession.get().output).toHaveLength(0);
  });
});
