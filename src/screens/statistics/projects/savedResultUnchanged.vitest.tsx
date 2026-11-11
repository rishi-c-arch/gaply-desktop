// A saved result with no scores renders as it did at 8eb00b7c, byte for byte
// (Research OS, Phase 8U-3, decision 554). Each stored fixture's body was
// rendered by 8eb00b7c's SavedResult.tsx and DescribeFlow.tsx, before this
// step changed either, and its innerHTML's SHA-256 written here; the run with
// no scores, with an empty list and with scores at no column of the result
// must each give the same bytes.
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { act, cleanup, render, screen } from '@testing-library/react';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { makeMockWorkspaceBridge } from '../../workspace/workspaceBridge';
import type { WorkspaceStateView } from '../../workspace/workspaceTypes';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { makeMockProjectBridge } from './projectBridge';
import { ProjectPage } from './ProjectPage';
import { SavedResultBody } from './SavedResult';
import { checkStored } from './storedShape';
import type { RunResultView, StoredRunView } from './projectTypes';

const ACTIVE: WorkspaceStateView = { state: { phase: 'active', workspace: 'statistics' }, activeRun: null, latestRun: null };

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

const TZ = process.env.TZ;
beforeAll(() => {
  process.env.TZ = 'UTC';
});
afterAll(() => {
  process.env.TZ = TZ;
});
afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
const FILES = fs.readdirSync(STORED).filter((f) => f.endsWith('.json')).sort();
const view = (f: string) => JSON.parse(fs.readFileSync(path.join(STORED, f), 'utf8')) as RunResultView;

/** Rendered by 8eb00b7c's code, before this step changed it. */
const AT_8EB00B7C: Record<string, string> = {
  'alpha.json': 'c3b41691abdd22833a61e66e53ddf511554179430a94434cc63f5f61911cbcb0',
  'alpha_reversed.json': 'afc8803a3c6e3b5efc86bd463eef4c480bf946de2e6dfe9089f48a4b209f99bc',
  'anova.json': 'c8ff774c5672b7652d443951cd477baa8dc41853913d5d8df575cac2121e4e4c',
  'anova_post_hoc.json': '0ccd254123bffdee0d1f3cec365ac6f8c287f7cea0eeddcfcfa4402c5ad8ae73',
  'corr_listwise.json': 'b758a2f470ec39cedf279d7a648c5242dd4672ca04f321a5beb57f8c52d006ae',
  'corr_pairwise.json': 'dbaf926719d33ef06d3e9f80d7ec344e6cf40d2035b03aebfc0ac7261ffd464e',
  'one_sample.json': 'e89dd355446a8e2d5129bd7b58dc7329b6aba975715c029c1c28f4ae3f305691',
  'one_sample_failed.json': '3782643db6fb760f162b9a371e3fb215357d427d47d556fb04d5c509b419e55b',
  'paired.json': 'c765325d614416b8681c06bb0512edeb636d686588011b261aa6aff177e14e0e',
  'paired_failed.json': '615142d171a3468257d6e65e3f07747e41de2dfc3b1d69c7328fec09297ce493',
  'summary.json': 'c06d214d87624e1a5a037789341652fb0e358f60266abd2ea6b2ffc2e25a3cfb',
  'summary_prepared.json': '5eb3778db7abfd8f120483e5f606de91b672eb7421b887a46d9d55b0279c0849',
  'ttest.json': '712b856b511cb1f499d13e5f1adb6dce78f93baf0e1e022c662331151b92fd00',
  'ttest_failed.json': 'ef79fd3d7b752ea4ca7a4c0fe54e76497a4ca923bdefce655a7ab10ae1b793cd',
};

/** Research OS, Phase 8X-2 (decision 591): the stored fixtures 8X-2 added, which
 *  8eb00b7c's code never rendered. Captured from 8X-2's own render, the record a
 *  later step keeps its own result against; AT_8EB00B7C is unchanged. */
const AT_8X2: Record<string, string> = {
  'xtab_2x2.json': '96a21e30a4827ebb65310a04d4a38c106bfe16567a3800d39759fcbf55278194',
  'xtab_3x3.json': '49b8837842bda6892741d30d1e01cc791a261452d8a2148d16f084dd4b125e82',
};

/** Research OS, Phase 8Y-2 (decision 622): the stored fixtures 8Y-2 added, captured
 *  from 8Y-2's own render, the record a later step keeps its own result against;
 *  AT_8EB00B7C and AT_8X2 are unchanged. */
const AT_8Y2: Record<string, string> = {
  'ranks_kw.json': '6b89c3bb900b06b8e9f8fd0720bea255fdd2db719e53e905253623f8dd8a9ab8',
  'ranks_mw.json': 'a33969b70729d288bb4a3c453f059b4607d32ae4139cd37ca8b603c37118926e',
};

/** Research OS, Phase 8Z-2 (decision 656): the stored fixtures 8Z-2 added, captured
 *  from 8Z-2's own render, the record a later step keeps its own result against;
 *  AT_8EB00B7C, AT_8X2 and AT_8Y2 are unchanged. */
const AT_8Z2: Record<string, string> = {
  'related_ranks_f.json': '7517b1ced822003af9515284d3a66cd4dc2f25f6a31581551fdd9eeb4a072df4',
  'related_ranks_w.json': '042d1a8da0c8422c171ebf3bea7f7c8df17e10c0ed3bce2710cc141303637c3f',
};

/** Research OS, Phase 9A-2 (decision 687): the stored fixtures 9A-2 added, captured from
 *  9A-2's own render, the record a later step keeps its own result against; AT_8EB00B7C,
 *  AT_8X2, AT_8Y2 and AT_8Z2 are unchanged. */
const AT_9A2: Record<string, string> = {
  'linear_regression.json': '0b8f6199d942731d03f5286f18ef70564058e9ca3f37c7d28b7396908fdaa12a',
  'linear_regression_failed.json': '11568990c75972feb761f316cee1b71173dce6103e167cd382fe618e715a5607',
  'linear_regression_perfect.json': '9f67d0a5a516850099d2d6cc4f5f9bb08f964f51ffa56d46dc0187d2e131bf4a',
};

/** Research OS, Phase 9B-1 (decision 698): the regression's edge cases saved, captured from
 *  9B-1's own render, the record a later step keeps its own result against; AT_8EB00B7C,
 *  AT_8X2, AT_8Y2, AT_8Z2 and AT_9A2 are unchanged. */
const AT_9B1: Record<string, string> = {
  'linear_regression_between_factors.json': '1fe8e48b3fb5ec62f3ecab503a304a5ddf657061f3686c7a4940ddb30af00bab',
  'linear_regression_ms_subnormal.json': '1b838563410774cc3d17438fac4c2d828082c2c2b45f4712c69716cee490493c',
  'linear_regression_mss_zero.json': '61c1822d25c1ef3747e4f9a5a895fb6027085866d5f1913bdc4da120eab87ed6',
  'linear_regression_offset_no_spread.json': '60bc55de5a0aa8ea4c562e297c5c3e00aeef388e832c1c5d785b411d1dee14e6',
  'linear_regression_sigma2_subnormal.json': '0113284e6464fa0ef1d6af1458797651ab9ee4981bdf4eb9b1588a9ccdcbf1a0',
  'linear_regression_split_se.json': '26f65a45c72fb5616843697ab9eda94c5692af498878ed50152af2e335a66595',
  'linear_regression_tiny_perfect.json': '0795dcb1b546a9e2fb7e39a141d605c3152712099f7c55cc3601b324d72c4d51',
};

/** Each file's record: 8eb00b7c's, or that of the step that added the file. */
const RECORDED: Record<string, string> = { ...AT_8EB00B7C, ...AT_8X2, ...AT_8Y2, ...AT_8Z2, ...AT_9A2, ...AT_9B1 };

const sha = (s: string) => createHash('sha256').update(s).digest('hex');

function rendered(f: string, scores?: Parameters<typeof SavedResultBody>[0] extends { scores?: infer S } ? S : never): string {
  const props = { checked: checkStored(view(f)), ...(scores === undefined ? {} : { scores }) } as Parameters<typeof SavedResultBody>[0];
  const { container } = render(<SavedResultBody {...props} />);
  const html = container.innerHTML;
  cleanup();
  return html;
}

/** A project page with no prepared run, its list read: rendered by 8eb00b7c's
 *  ProjectPage.tsx, before this step changed it. */
const PAGE_AT_8EB00B7C = 'f8326e127586aa414fb3b497e0aaa5e5870a9e593e5b99f98af8af3928da5c57';

const A = '0123456789abcdef0123456789abcdef';
const run = (id: number, capabilityId: string, state: StoredRunView['state'], savedAt: number): StoredRunView => ({
  storedRunId: id,
  capabilityId,
  capabilityVersion: 1,
  state,
  endCode: state === 'failed' ? 'project.run_interrupted' : null,
  startedAt: savedAt - 10,
  endedAt: savedAt - 9,
  savedAt,
  agentVersion: '0.1.0',
  rerunOf: id === 4 ? 1 : null,
  comparison: id === 4 ? 'identical' : null,
  engineVersionDiffers: id === 4 ? false : null,
});

describe('a project page with no prepared run, byte for byte as at 8eb00b7c', () => {
  it('reads what it read, and renders what it rendered', async () => {
    const bridge = makeMockProjectBridge({
      project_current: { projectId: A, rerun: null },
      project_list: [{ projectId: A, name: 'Chapter 3', bytes: 40000, savedAt: 1792396351, runCount: 4, refused: null }],
      project_sizes: { totalBytes: 40000, projectCount: 1, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 },
      project_runs: [
        run(1, 'stats.describe', 'succeeded', 1792396100),
        run(2, 'stats.analyze.independent_t', 'failed', 1792396200),
        run(3, 'stats.analyze.correlations', 'succeeded', 1792396300),
        run(4, 'stats.describe', 'succeeded', 1792396400),
      ],
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
    await screen.findByTestId('project-runs-list');
    await act(async () => new Promise((r) => setTimeout(r, 30)));
    expect(bridge.calls.map(([c]) => c)).toEqual(['project_current', 'project_list', 'project_sizes', 'project_runs']);
    expect({ page: sha(screen.getByTestId('project-page').outerHTML) }).toEqual({ page: PAGE_AT_8EB00B7C });
  });
});

describe('a saved result with no scores, byte for byte as at 8eb00b7c', () => {
  it('covers every stored fixture', () => {
    expect(FILES.length).toBeGreaterThanOrEqual(20);
    // The six records name disjoint files, and together the whole directory.
    expect(Object.keys(AT_8EB00B7C).filter((k) => k in AT_8X2 || k in AT_8Y2 || k in AT_8Z2 || k in AT_9A2 || k in AT_9B1)).toEqual([]);
    expect(Object.keys(AT_8X2).filter((k) => k in AT_8Y2 || k in AT_8Z2 || k in AT_9A2 || k in AT_9B1)).toEqual([]);
    expect(Object.keys(AT_8Y2).filter((k) => k in AT_8Z2 || k in AT_9A2 || k in AT_9B1)).toEqual([]);
    expect(Object.keys(AT_8Z2).filter((k) => k in AT_9A2 || k in AT_9B1)).toEqual([]);
    expect(Object.keys(AT_9A2).filter((k) => k in AT_9B1)).toEqual([]);
    expect([...Object.keys(AT_8EB00B7C), ...Object.keys(AT_8X2), ...Object.keys(AT_8Y2), ...Object.keys(AT_8Z2), ...Object.keys(AT_9A2), ...Object.keys(AT_9B1)].sort()).toEqual(FILES);
  });

  it.each(FILES)('%s', (f) => {
    const html = rendered(f);
    expect(html.length).toBeGreaterThan(40);
    expect({ [f]: sha(html) }).toEqual({ [f]: RECORDED[f] });
  });
});
