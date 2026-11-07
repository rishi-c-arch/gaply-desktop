// The Output's Save, by the entry's view (Research OS, Phase 8T-2b, decision
// 515): an entry whose view has a preparation shows the save sentence that says
// the recipe and its names are kept, the same test as its spec carrying a
// preparation (decision 490), so a run on a prepared file that used no score is
// told so too; any other entry shows decision 133's. Every bridge is a mock.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { defaultRunBridges } from '../../workspace/useWorkspaceSwitch';
import { makeMockAlphaBridge } from '../alphaBridge';
import type { AlphaResultView } from '../alphaTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { alphaResultView } from '../fixtures/alphaFixtures';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import { openProjectStore } from '../projects/projectSession';
import { SAVE_SENTENCE, SAVE_SENTENCE_PREPARED } from '../projects/projectWording';
import { Output } from './Output';
import { statisticsSession } from './statisticsSession';

vi.mock('../../../config/featureFlags', () => ({ isFeatureEnabled: (name: string) => name === 'workspaces' || name === 'projects' }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  openProjectStore.clear();
});

const col = (alias: string, extra: Partial<ColumnProfile> = {}): ColumnProfile => ({ alias, header: alias, kind: 'numeric', n: 9, missing: 0, distinct: 5, levels: null, suspectMissingCodes: [], nonFinite: 0, ...extra });
const fileView: DatasetView = {
  slot: 1,
  displayName: 'items.csv',
  profile: { rows: 9, columns: [col('C1'), col('C2'), col('C3')] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 9, columns: 3, semanticRole: 'table' },
};
const preparedView: DatasetView = {
  ...fileView,
  profile: { rows: 9, columns: [...fileView.profile.columns, col('C4', { prepared: true, scoreName: 'Total' })] },
  inputRef: { ...fileView.inputRef, contentHash: 'cd'.repeat(32), columns: 4 },
  preparation: { sourceHash: 'ab'.repeat(32), recipeHash: 'ef'.repeat(32), sourceColumns: 3, capabilityVersion: 1, engineVersion: 1 },
};

/** One alpha entry, finished, on `view`, and the Output drawn over the session. */
function drawn(view: DatasetView) {
  act(() => {
    statisticsSession.setFile(view);
    const n = statisticsSession.addEntry({
      kind: 'cronbach_alpha',
      view,
      slot: 1,
      settings: 'Items: C1, C2.',
      runId: 5,
      run: { runId: 5, state: 'succeeded', terminal: true, elapsedMs: 1, cancelRequested: false, stepsDone: 7, stepsTotal: 7, diagnostics: [] },
    });
    statisticsSession.updateEntry(n as number, { result: { ...alphaResultView, runId: 5 } as unknown as AlphaResultView });
  });
  const bridges = defaultRunBridges({ alpha: makeMockAlphaBridge() });
  const projects = makeMockProjectBridge({ [C.current]: { projectId: null }, [C.list]: [] });
  const Harness: React.FC = () => {
    const [, force] = React.useState(0);
    React.useEffect(() => statisticsSession.subscribe(() => force((x) => x + 1)), []);
    return <Output session={statisticsSession.get()} bridges={bridges} forget={makeMockDescribeBridge()} projects={projects} save={async () => null} status={null} />;
  };
  render(<Harness />);
  const e = screen.getByTestId('sw-entry-1');
  fireEvent.click(within(e).getByTestId('sw-entry-1-project-button'));
  return screen.findByTestId('sw-entry-1-project-sentence');
}

describe("the Output's Save, by the entry's view (decision 515)", () => {
  it('an entry on a prepared view: the sentence that says the recipe and its names are kept', async () => {
    expect((await drawn(preparedView)).textContent).toBe(SAVE_SENTENCE_PREPARED);
  });

  it("an entry on a file's own table: decision 133's sentence", async () => {
    expect((await drawn(fileView)).textContent).toBe(SAVE_SENTENCE);
  });
});
