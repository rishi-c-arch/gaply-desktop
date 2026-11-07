// The save and rerun sentences for a run on prepared data (Research OS, Phase
// 8T-2b, decisions 515 to 517; ADR-011's disk list). Each new sentence word for
// word beside the old one, which stays; SaveToProject showing one or the other
// by its prepared prop; RerunDialog by the stored run's prepared, which Rust
// sends only when true; and every new sentence of this step read for the words a
// verdict would use, with alpha's and decision 66's lists copied.
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { ASK_EXPORT_PREPARED, EXPORT_PREPARED_NAME, NO_SCORE } from '../window/windowWording';
import { makeMockProjectBridge } from './projectBridge';
import { ProjectPage } from './ProjectPage';
import { openProjectStore } from './projectSession';
import type { StoredRunView } from './projectTypes';
import { RERUN_SENTENCE, RERUN_SENTENCE_PREPARED, SAVE_SENTENCE, SAVE_SENTENCE_PREPARED } from './projectWording';
import { SaveToProject } from './SaveToProject';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));

beforeEach(() => openProjectStore.clear());
afterEach(cleanup);

/** alphaWording.vitest.ts:20, copied: the words a verdict on alpha would use (decision 384). */
const VERDICT = /\b(acceptable|unacceptable|good|excellent|questionable|poor|reliable|unreliable|adequate|sufficient)\b|0\.7\b|[*★☆]|\b(should|recommend|drop|remove)\b/i;

/** PairedFlow.vitest.tsx:135, copied: decision 66's words. */
const VERDICT_66 = /significan|[*★☆]|\breject|\bconfirm(s|ed)? that|\bproves?\b|null hypothesis|\b(t-)?test (passed|failed)\b|\bfail(s|ed)? the test\b/i;

describe('the two new sentences, word for word (decisions 515 and 516)', () => {
  it("the save sentence for a run on prepared data, decision 133's beside it unchanged", () => {
    expect(SAVE_SENTENCE_PREPARED).toBe(
      "Saving keeps a copy of the data file, exactly as it is, and the recipe of its scale scores, with the names you typed for them, in this app's folder on this computer. Gaply uploads none of it. You can delete the project at any time."
    );
    expect(SAVE_SENTENCE).toBe("Saving keeps a copy of the data file, exactly as it is, in this app's folder on this computer. Gaply uploads none of it. You can delete the project at any time.");
  });

  it("the rerun sentence for a prepared run, decision 191's beside it unchanged", () => {
    expect(RERUN_SENTENCE_PREPARED).toBe(
      'Run this again? This result was made on data with scale scores. Gaply makes the scores again from the recipe kept in this project, on the copy of the data file kept in this project, then runs the same method with the same settings. When it finishes, the new run is saved in this project, beside the saved one, which does not change.'
    );
    expect(RERUN_SENTENCE).toBe(
      'Run this again? Gaply runs the same method, with the same settings, on the copy of the data file kept in this project. When it finishes, the new run is saved in this project, beside the saved one, which does not change.'
    );
  });

  it("no new sentence of this step carries a verdict's word: alpha's list or decision 66's", () => {
    const all = [SAVE_SENTENCE_PREPARED, RERUN_SENTENCE_PREPARED, NO_SCORE, ASK_EXPORT_PREPARED.title, ASK_EXPORT_PREPARED.sentence, EXPORT_PREPARED_NAME];
    expect(all).toHaveLength(6);
    for (const s of all) {
      expect(s, s).not.toMatch(VERDICT);
      expect(s, s).not.toMatch(VERDICT_66);
    }
  });
});

/* ---- SaveToProject -------------------------------------------------------------- */

function saveDialog(prepared?: boolean) {
  const bridge = makeMockProjectBridge({ project_current: { projectId: null }, project_list: [] });
  render(prepared === undefined ? <SaveToProject runId={5} bridge={bridge} desktop /> : <SaveToProject runId={5} bridge={bridge} desktop prepared={prepared} />);
  fireEvent.click(screen.getByTestId('project-save-button'));
  return screen.findByTestId('project-save-sentence');
}

describe('SaveToProject, by prepared (decision 515)', () => {
  it("with no prepared prop, decision 133's sentence", async () => {
    expect((await saveDialog()).textContent).toBe(SAVE_SENTENCE);
  });

  it("with prepared false, decision 133's sentence", async () => {
    expect((await saveDialog(false)).textContent).toBe(SAVE_SENTENCE);
  });

  it('with prepared true, the sentence that says the recipe and its names are kept', async () => {
    expect((await saveDialog(true)).textContent).toBe(SAVE_SENTENCE_PREPARED);
  });
});

/* ---- RerunDialog ---------------------------------------------------------------- */

const A = '0123456789abcdef0123456789abcdef';
const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

const storedRun = (id: number, over: Partial<StoredRunView> = {}): StoredRunView => ({
  storedRunId: id,
  capabilityId: 'stats.analyze.independent_t',
  capabilityVersion: 1,
  state: 'succeeded',
  endCode: null,
  startedAt: 1792396290,
  endedAt: 1792396291,
  savedAt: 1792396300 + id,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
  ...over,
});

function page(runs: StoredRunView[]) {
  const bridge = makeMockProjectBridge({
    project_current: { projectId: A, rerun: null },
    project_list: [{ projectId: A, name: 'HT8T2', bytes: 40000, savedAt: 1792396351, runCount: runs.length, refused: null }],
    project_sizes: { totalBytes: 40000, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
    project_runs: runs,
    project_close: { closed: true },
  });
  render(
    <ProjectPage
      projectId={A}
      bridge={bridge}
      workspace={makeMockWorkspaceBridge({ workspace_state: ACTIVE })}
      describe={makeMockDescribeBridge()}
      ttest={makeMockTTestBridge()}
      desktop
      save={async () => null}
      pollMs={5}
      onClosed={vi.fn()}
      onBack={vi.fn()}
    />
  );
}

describe('RerunDialog, by the stored run\'s prepared (decision 516)', () => {
  it('a prepared run: the sentence that says the scores are made again from the recipe; an unprepared one beside it: the old one', async () => {
    page([storedRun(1), storedRun(2, { prepared: true })]);
    await screen.findByTestId('project-runs-list');

    fireEvent.click(screen.getByTestId('project-run-rerun-2'));
    expect(screen.getByTestId('project-rerun-sentence').textContent).toBe(RERUN_SENTENCE_PREPARED);
    fireEvent.click(screen.getByTestId('project-rerun-dialog-cancel'));

    fireEvent.click(screen.getByTestId('project-run-rerun-1'));
    expect(screen.getByTestId('project-rerun-sentence').textContent).toBe(RERUN_SENTENCE);
  });
});
