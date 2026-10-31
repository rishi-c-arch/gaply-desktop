// The workspace wire, from the TypeScript side (Research OS, Phase 3). Each
// JSON fixture is the exact serialisation of a Rust view or refusal
// (src-tauri/tests/workspace_wire.rs checks the bytes); here each is
// deep-compared with a copy typed `satisfies` its TypeScript type, so tsc
// catches a renamed or missing key and this run catches a value that differs.
// The code list Rust pins is exactly the codes the wording has a sentence for.
import { describe, expect, it } from 'vitest';

import listJson from './fixtures/workspaceList.json';
import stateJson from './fixtures/workspaceStateView.json';
import noneJson from './fixtures/workspaceStateViewNone.json';
import runActiveJson from './fixtures/refusalRunActive.json';
import illegalJson from './fixtures/refusalIllegalTransition.json';
import codesJson from './workspaceCodes.json';
import type { WorkspaceDescriptorView, WorkspaceRefusal, WorkspaceStateView } from './workspaceTypes';
import { workspaceRefusal } from './workspaceTypes';
import { WORDED_CODES, workspaceSentence } from './workspaceWording';

const missing = (id: 'compute' | 'design') =>
  ({
    id,
    version: 0,
    displayNameCode: `workspace.name.${id}`,
    availability: 'missing',
    activationPolicy: 'single_active',
    capabilityIds: [],
    supportsOffline: false,
    supportedPlatforms: [],
    entitled: null,
  }) satisfies WorkspaceDescriptorView;

const list = [
  {
    id: 'statistics',
    version: 1,
    displayNameCode: 'workspace.name.statistics',
    availability: 'available',
    activationPolicy: 'single_active',
    capabilityIds: [
      'stats.check',
      'stats.verify.independent_t',
      'stats.verify.paired_t',
      'stats.verify.one_way_anova',
      'stats.verify.pearson',
      'stats.verify.spearman',
      'stats.verify.chi_square_independence',
      'stats.verify.ols',
      'stats.describe',
      'stats.analyze.independent_t',
      'stats.analyze.one_sample_t',
      'stats.analyze.paired_t',
      'stats.analyze.one_way_anova',
    ],
    supportsOffline: true,
    supportedPlatforms: ['macos', 'windows', 'linux'],
    entitled: null,
  },
  missing('compute'),
  missing('design'),
] satisfies WorkspaceDescriptorView[];

// Phase 4B-2 (decision 80): each run names its kind.
const state = {
  state: { phase: 'active', workspace: 'statistics' },
  activeRun: { runId: 3, kind: 'ttest' },
  latestRun: { runId: 3, kind: 'ttest' },
} satisfies WorkspaceStateView;
const none = { state: { phase: 'no_workspace' }, activeRun: null, latestRun: null } satisfies WorkspaceStateView;
const runActive = {
  code: 'workspace.run_active',
  params: { workspace: 'statistics', run: 3 },
  severity: 'error',
} satisfies WorkspaceRefusal;
const illegal = {
  code: 'workspace.illegal_transition',
  params: { from: 'active', event: 'activate', workspace: 'statistics' },
  severity: 'error',
} satisfies WorkspaceRefusal;

describe('the workspace wire', () => {
  it.each([
    ['workspace_list', list, listJson],
    ['WorkspaceStateView (active)', state, stateJson],
    ['WorkspaceStateView (none)', none, noneJson],
    ['refusal run_active', runActive, runActiveJson],
    ['refusal illegal_transition', illegal, illegalJson],
  ])('%s: the typed copy equals the JSON Rust serialises', (_n, typed, json) => {
    expect(typed).toEqual(json);
  });

  it('has a sentence for exactly the codes Rust pins', () => {
    expect(WORDED_CODES).toEqual([...codesJson].sort());
  });

  it('reads a refusal out of a command error, and words it from closed values only', () => {
    const err = { code: 'validation', message: JSON.stringify(runActive) };
    expect(workspaceRefusal(err)).toEqual(runActive);
    expect(workspaceSentence(runActive)).toBe('Run 3 is still queued or running, so the workspace cannot be closed yet.');
    expect(workspaceSentence(illegal)).toBe('Statistics is open. Close it first.');
    // F15: while another activation is opening, nothing can be closed.
    const opening = { ...illegal, params: { ...illegal.params, from: 'activating' as const } };
    expect(workspaceSentence(opening)).toBe('Statistics is still opening. Wait for it to open, then try again.');
    expect(workspaceRefusal({ message: JSON.stringify({ code: 'run.unknown_run' }) })).toBeNull();
    expect(workspaceRefusal(new Error('not json'))).toBeNull();
  });
});
