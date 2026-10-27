// A saved paired-samples t-test result (Research OS, Phase 8D-2c, decision
// 272), read from the two snapshots Rust wrote (src-tauri/tests/fixtures/
// projects/stored/paired.json and paired_failed.json, by
// stored_paired_results_have_the_layout_of_their_fixtures): the shape check
// accepts each as its own case; the saved result renders through the live
// screen's PairedResultCard with the same text as the live results screen,
// and with no button; and the failed run shows differences_constant's
// sentence. In savedOneSample.vitest.tsx's shape, and here, under projects/,
// because projectGuards.vitest.ts lets only this folder's tests name a
// project command (project_run_result). Each fixture is read inside its case.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import PairedFlow from '../PairedFlow';
import { makeMockPairedBridge } from '../onesampleBridge';
import type { DatasetView } from '../describeTypes';
import { oneSampleSentence } from '../onesampleWording';
import type { OneSampleDiagnostic } from '../onesampleTypes';
import { makeMockProjectBridge } from './projectBridge';
import type { StoredRunView } from './projectTypes';
import { SAVED_TEXT } from './projectWording';
import { SavedResult } from './SavedResult';
import { checkStored } from './storedShape';
import { useGeneration } from './useGeneration';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
type Stored = Record<string, unknown> & { bundleJson: string | null; failureJson: string | null };
const stored = (name: string): Stored => JSON.parse(fs.readFileSync(path.join(STORED, name), 'utf8')) as Stored;
const ID = '0123456789abcdef0123456789abcdef';

const RUN = (state: 'succeeded' | 'failed', storedRunId: number): StoredRunView => ({
  storedRunId,
  capabilityId: 'stats.analyze.paired_t',
  capabilityVersion: 1,
  state,
  endCode: null,
  startedAt: 1792396000,
  endedAt: 1792396001,
  savedAt: 1792396100,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
});

/** Two numeric columns, for the live screen's pickers. */
const view: DatasetView = {
  slot: 4,
  displayName: 'pairs.csv',
  profile: {
    rows: 4,
    columns: [
      { alias: 'C1', header: 'a', kind: 'numeric', n: 4, missing: 0, distinct: 4, levels: null, suspectMissingCodes: [], nonFinite: 0 },
      { alias: 'C2', header: 'b', kind: 'numeric', n: 4, missing: 0, distinct: 4, levels: null, suspectMissingCodes: [], nonFinite: 0 },
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 4, columns: 2, semanticRole: 'table' },
};

function saved(result: unknown, run: StoredRunView) {
  const bridge = makeMockProjectBridge({ project_run_result: result });
  const Host: React.FC = () => {
    const gen = useGeneration();
    return <SavedResult projectId={ID} run={run} bridge={bridge} gen={gen} />;
  };
  render(<Host />);
}

/** The live results screen's card for the same result, made from the stored
 *  bundle as Rust's stats_onesample_commands::result_paired makes it. */
async function liveText(bundle: Record<string, unknown>): Promise<string> {
  const live = { runId: bundle.runId, status: bundle.status, result: bundle.primaryOutputs, diagnostics: bundle.diagnostics, manifest: bundle.manifest };
  const bridge = makeMockPairedBridge({
    stats_desc_open: view,
    stats_desc_close: true,
    stats_paired_validate: { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 4, columns: 2, cells: 8 } },
    stats_paired_run: { runId: 1 },
    stats_paired_status: { runId: 1, state: 'succeeded', terminal: true, elapsedMs: 0, cancelRequested: false, diagnostics: [] },
    stats_paired_result: live,
  });
  render(<PairedFlow bridge={bridge} pickPath={() => Promise.resolve('/x/pairs.csv')} pollMs={5} />);
  fireEvent.click(screen.getByTestId('pt-choose'));
  fireEvent.click(await screen.findByTestId('pt-to-settings'));
  fireEvent.change(await screen.findByTestId('pt-first'), { target: { value: 'C1' } });
  fireEvent.change(screen.getByTestId('pt-second'), { target: { value: 'C2' } });
  fireEvent.click(screen.getByTestId('pt-check'));
  fireEvent.click(await screen.findByTestId('pt-run'));
  fireEvent.click(await screen.findByTestId('pt-see-results'));
  const text = (await screen.findByTestId('pt-result-card')).textContent ?? '';
  cleanup();
  return text;
}

describe('a saved paired-samples t-test', () => {
  it('the shape check accepts both snapshots, each as its own case', () => {
    expect(checkStored(stored('paired.json')).kind).toBe('paired');
    const failed = checkStored(stored('paired_failed.json'));
    expect(failed.kind).toBe('failed');
    expect(failed.kind === 'failed' && failed.capability).toBe('paired');
    // A string where a parameter's number belongs is refused: no string
    // parameters for this method (decision 272).
    const s = stored('paired_failed.json');
    const v = JSON.parse(s.failureJson as string);
    v.failure.params = { column: 'C1' };
    expect(checkStored({ ...s, failureJson: JSON.stringify(v) }).kind).toBe('badShape');
  });

  it('renders through PairedResultCard, with the same text as the live results screen', async () => {
    const s = stored('paired.json');
    const live = await liveText(JSON.parse(s.bundleJson as string));
    expect(live.length).toBeGreaterThan(100);
    saved(s, RUN('succeeded', 1));
    expect((await screen.findByTestId('pt-result-card')).textContent).toBe(live);
    const section = screen.getByTestId('saved-result');
    expect(screen.getByTestId('saved-result-kind').textContent).toBe('Paired-samples t-test');
    // Read only: no Save, re-run, edit or AI control.
    expect(within(section).queryAllByRole('button')).toHaveLength(0);
    expect(screen.queryByTestId('pt-project-save')).toBeNull();
  });

  it("a failed run shows differences_constant's sentence", async () => {
    const s = stored('paired_failed.json');
    saved(s, RUN('failed', 2));
    const failure = JSON.parse(s.failureJson as string).failure as OneSampleDiagnostic;
    expect(failure.code).toBe('stats.onesample.differences_constant');
    expect((await screen.findByTestId('saved-result-failed')).textContent).toBe(`${SAVED_TEXT.failed} ${oneSampleSentence(failure)}`);
    expect(screen.getByTestId('saved-result-failed').textContent).toContain(
      'Every pair differs by the same amount, so the standard error of the differences is exactly 0 and t is not defined.'
    );
  });
});
