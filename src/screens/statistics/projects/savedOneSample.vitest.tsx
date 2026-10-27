// A saved one-sample t-test result (Research OS, Phase 8D-2b, decision 269),
// read from the two snapshots Rust wrote (src-tauri/tests/fixtures/projects/
// stored/one_sample.json and one_sample_failed.json, by
// stored_one_sample_results_have_the_layout_of_their_fixtures): the shape
// check accepts each as its own case; the saved result renders through the
// live screen's OneSampleResultCard with the same text as the live results
// screen; and the failed run shows its code's sentence. The shape check's
// field-by-field rejections for these two are SavedResult.vitest.tsx's
// tables. Here, under projects/, because projectGuards.vitest.ts lets only
// this folder's tests name a project command (project_run_result).
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import OneSampleFlow from '../OneSampleFlow';
import { makeMockOneSampleBridge } from '../onesampleBridge';
import { datasetView } from '../fixtures/describeFixtures';
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
  capabilityId: 'stats.analyze.one_sample_t',
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

function saved(view: unknown, run: StoredRunView) {
  const bridge = makeMockProjectBridge({ project_run_result: view });
  const Host: React.FC = () => {
    const gen = useGeneration();
    return <SavedResult projectId={ID} run={run} bridge={bridge} gen={gen} />;
  };
  render(<Host />);
}

/** The live results screen's card for the same result, made from the stored
 *  bundle as Rust's stats_onesample_commands::result_one_sample makes it. */
async function liveText(bundle: Record<string, unknown>): Promise<string> {
  const live = { runId: bundle.runId, status: bundle.status, result: bundle.primaryOutputs, diagnostics: bundle.diagnostics, manifest: bundle.manifest };
  const bridge = makeMockOneSampleBridge({
    stats_desc_open: datasetView,
    stats_desc_close: true,
    stats_onesample_validate: { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 9, columns: 1, cells: 9 } },
    stats_onesample_run: { runId: 1 },
    stats_onesample_status: { runId: 1, state: 'succeeded', terminal: true, elapsedMs: 0, cancelRequested: false, diagnostics: [] },
    stats_onesample_result: live,
  });
  render(<OneSampleFlow bridge={bridge} pickPath={() => Promise.resolve('/x/data.csv')} pollMs={5} />);
  fireEvent.click(screen.getByTestId('os-choose'));
  fireEvent.click(await screen.findByTestId('os-to-settings'));
  fireEvent.change(await screen.findByTestId('os-outcome'), { target: { value: 'C2' } });
  fireEvent.change(screen.getByTestId('os-test-value'), { target: { value: '3' } });
  fireEvent.click(screen.getByTestId('os-check'));
  fireEvent.click(await screen.findByTestId('os-run'));
  fireEvent.click(await screen.findByTestId('os-see-results'));
  const text = (await screen.findByTestId('os-result-card')).textContent ?? '';
  cleanup();
  return text;
}

describe('a saved one-sample t-test', () => {
  it('the shape check accepts both snapshots, each as its own case', () => {
    expect(checkStored(stored('one_sample.json')).kind).toBe('one_sample');
    const failed = checkStored(stored('one_sample_failed.json'));
    expect(failed.kind).toBe('failed');
    expect(failed.kind === 'failed' && failed.capability).toBe('one_sample');
    // A string where a parameter's number belongs is refused: no string
    // parameters for this method (decision 269).
    const s = stored('one_sample_failed.json');
    const v = JSON.parse(s.failureJson as string);
    v.failure.params = { column: 'C1' };
    expect(checkStored({ ...s, failureJson: JSON.stringify(v) }).kind).toBe('badShape');
  });

  it('renders through OneSampleResultCard, with the same text as the live results screen', async () => {
    const s = stored('one_sample.json');
    const live = await liveText(JSON.parse(s.bundleJson as string));
    expect(live.length).toBeGreaterThan(100);
    saved(s, RUN('succeeded', 1));
    expect((await screen.findByTestId('os-result-card')).textContent).toBe(live);
    const section = screen.getByTestId('saved-result');
    expect(screen.getByTestId('saved-result-kind').textContent).toBe('One-sample t-test');
    // Read only: no Save, re-run, edit or AI control.
    expect(within(section).queryAllByRole('button')).toHaveLength(0);
    expect(screen.queryByTestId('os-project-save')).toBeNull();
  });

  it("a failed run shows its code's sentence", async () => {
    const s = stored('one_sample_failed.json');
    saved(s, RUN('failed', 2));
    const failure = JSON.parse(s.failureJson as string).failure as OneSampleDiagnostic;
    expect(failure.code).toBe('stats.onesample.values_constant');
    expect((await screen.findByTestId('saved-result-failed')).textContent).toBe(`${SAVED_TEXT.failed} ${oneSampleSentence(failure)}`);
    expect(screen.getByTestId('saved-result-failed').textContent).toContain(
      'Every value is the same, so the standard error is exactly 0 and t is not defined.'
    );
  });
});
