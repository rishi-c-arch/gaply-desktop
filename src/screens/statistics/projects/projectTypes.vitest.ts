// The project commands' wire contract, from the TypeScript side (Research OS,
// Phase 5B-3a). Rust pins each response's JSON byte for byte in
// src-tauri/tests/fixtures/projects/ (src-tauri/tests/project_commands.rs,
// every_response_serialises_to_its_fixture, the_project_code_list_is_pinned).
// Here each fixture is read from that folder and deep-compared with a copy
// typed `satisfies` its view, so tsc catches a renamed or missing key and
// this run catches a value or a shape that differs; the code list must equal
// PROJECT_CODES; and the bridge must send each command's name and argument
// keys as the Tauri wrappers read them.
import fs from 'node:fs';
import path from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { PROJECT_COMMANDS, TauriProjectBridge } from './projectBridge';
import {
  PROJECT_CODES,
  type DeleteConfirmationView,
  type ProjectClosedView,
  type ProjectCurrentView,
  type ProjectDeletedView,
  type ProjectExportView,
  type ProjectListEntryView,
  type ProjectOpenView,
  type ProjectRefusalView,
  type ProjectSizesView,
  type RerunStartedView,
  type RunResultView,
  type SavedRunView,
  type StoredRunView,
} from './projectTypes';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

const FIXTURES = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects');
const fixture = (name: string): unknown => JSON.parse(fs.readFileSync(path.join(FIXTURES, name), 'utf8'));

const ID = '0123456789abcdef0123456789abcdef';
const HASH = '5fd0af5daee4d8dfcb466411096e7f26e513bf2cadfb0e1605ff085026fe5b4a';

const typed: Array<[string, unknown]> = [
  [
    'projectList.json',
    [
      { projectId: ID, name: 'Chapter 3', bytes: 40960, savedAt: 1792396351, runCount: 2, refused: null },
      { projectId: 'fedcba9876543210fedcba9876543210', name: null, bytes: 4096, savedAt: null, runCount: null, refused: 'project.db_unreadable' },
    ] satisfies ProjectListEntryView[],
  ],
  [
    'projectSizes.json',
    { totalBytes: 45056, projectCount: 2, projectCapBytes: 104857600, allProjectsCapBytes: 10737418240 } satisfies ProjectSizesView,
  ],
  ['projectPrepareDelete.json', { projectId: ID, confirmation: HASH, bytes: 40960, expiresInMs: 300000 } satisfies DeleteConfirmationView],
  ['projectDeleted.json', { projectId: ID, closed: true } satisfies ProjectDeletedView],
  ['projectSaveRun.json', { projectId: ID, storedRunId: 1, created: true } satisfies SavedRunView],
  ['projectOpen.json', { projectId: ID, name: 'Chapter 3', createdAt: 1792396351, runCount: 2, bytes: 40960 } satisfies ProjectOpenView],
  ['projectClose.json', { closed: true } satisfies ProjectClosedView],
  [
    'projectCurrent.json',
    [
      { projectId: ID, rerun: null },
      { projectId: ID, rerun: { runId: 7, rerunOf: 1, status: 'going', storedRunId: null, code: null } },
      { projectId: ID, rerun: { runId: 7, rerunOf: 1, status: 'saved', storedRunId: 3, code: null } },
      { projectId: ID, rerun: { runId: 7, rerunOf: 1, status: 'not_saved', storedRunId: null, code: 'project.disk_full' } },
      { projectId: null, rerun: null },
    ] satisfies ProjectCurrentView[],
  ],
  [
    'projectRerun.json',
    [
      { runId: 7, kind: 'describe' },
      { runId: 8, kind: 'ttest' },
    ] satisfies RerunStartedView[],
  ],
  [
    'projectRuns.json',
    [
      {
        storedRunId: 1,
        capabilityId: 'stats.analyze.independent_t',
        capabilityVersion: 1,
        state: 'failed',
        endCode: 'project.run_interrupted',
        startedAt: 1792396340,
        endedAt: 1792396341,
        savedAt: 1792396351,
        agentVersion: '0.1.0',
        rerunOf: null,
        comparison: null,
        engineVersionDiffers: null,
      },
      {
        storedRunId: 2,
        capabilityId: 'stats.describe',
        capabilityVersion: 1,
        state: 'succeeded',
        endCode: null,
        startedAt: 1792396360,
        endedAt: 1792396361,
        savedAt: 1792396362,
        agentVersion: '0.1.0',
        rerunOf: 1,
        comparison: 'identical',
        engineVersionDiffers: false,
      },
    ] satisfies StoredRunView[],
  ],
  [
    'projectRunResult.json',
    {
      storedRunId: 1,
      capabilityId: 'stats.analyze.independent_t',
      state: 'failed',
      dataFile: { sha256: HASH, kind: 'data_file', format: 'xlsx', bytes: 2048 },
      specJson: '{"capabilityId":"stats.analyze.independent_t"}',
      bundleJson: '{"status":"succeeded"}',
      manifestJson: `{"outputHash":"${HASH}"}`,
      failureJson: '{"schemaVersion":1,"status":"failed","failure":{"code":"stats.ttest.both_groups_constant","params":{},"severity":"error"}}',
    } satisfies RunResultView,
  ],
  ['projectReport.json', { fileName: 'gaply_project_report.md', bytes: [35, 32, 84, 10], sha256: HASH } satisfies ProjectExportView],
  ['projectProvenance.json', { fileName: 'gaply_project_provenance.json', bytes: [123, 125, 10], sha256: HASH } satisfies ProjectExportView],
  [
    'projectRefusal.json',
    [{ code: 'project.project_cap', needed: 104857601, limit: 104857600 }, { code: 'project.not_open' }] satisfies ProjectRefusalView[],
  ],
];

/** Every key path in a value, arrays read through. */
function keyPaths(v: unknown, at = ''): string[] {
  if (Array.isArray(v)) return v.flatMap((x) => keyPaths(x, `${at}[]`));
  if (v && typeof v === 'object') {
    return Object.entries(v as Record<string, unknown>).flatMap(([k, x]) => [`${at}.${k}`, ...keyPaths(x, `${at}.${k}`)]);
  }
  return [];
}
const keys = (v: unknown) => Array.from(new Set(keyPaths(v))).sort();

describe('the project view fixtures Rust writes', () => {
  it('are all here: one per response, the refusal and the code list', () => {
    const files = fs.readdirSync(FIXTURES).filter((f) => f.endsWith('.json')).sort();
    expect(files).toEqual([...typed.map(([f]) => f), 'projectCodes.json'].sort());
  });

  it.each(typed)('%s: the typed copy equals the JSON Rust serialises, key for key', (file, copy) => {
    const json = fixture(file);
    expect(copy).toEqual(json);
    expect(keys(copy)).toEqual(keys(json));
  });

  it('the code list is PROJECT_CODES, in order, the three lock codes and the six rerun codes included', () => {
    const codes = fixture('projectCodes.json') as string[];
    expect(codes).toEqual([...PROJECT_CODES]);
    expect(codes).toHaveLength(55);
    expect(codes.slice(45, 48)).toEqual(['project.open_elsewhere', 'project.being_deleted', 'project.lock_failed']);
    // The six rerun codes, then F1's folder code last (5E-fix-1, decision 223).
    expect(codes.slice(-7, -1)).toEqual([
      'project.rerun_unsupported',
      'project.rerun_not_succeeded',
      'project.rerun_busy',
      'project.rerun_refused',
      'project.rerun_in_progress',
      'project.rerun_data_unreadable',
    ]);
    expect(codes.at(-1)).toBe('project.folder_unchangeable');
  });
});

describe('the project bridge', () => {
  it('sends each of the thirteen commands with the argument keys its wrapper reads', async () => {
    const b = new TauriProjectBridge();
    await b.list();
    await b.sizes();
    await b.prepareDelete(ID);
    await b.delete(ID, HASH);
    await b.current();
    await b.saveRun(5, { into: 'newProject', name: 'Chapter 3' });
    await b.saveRun(6, { into: 'openProject', projectId: ID });
    await b.open(ID);
    await b.close();
    await b.runs(ID);
    await b.runResult(ID, 1);
    await b.report(ID);
    await b.provenance(ID);
    await b.rerun(3);
    expect(invoke.mock.calls).toEqual([
      ['project_list', {}],
      ['project_sizes', {}],
      ['project_prepare_delete', { projectId: ID }],
      ['project_delete', { projectId: ID, confirmation: HASH }],
      ['project_current', {}],
      ['project_save_run', { runId: 5, target: { into: 'newProject', name: 'Chapter 3' } }],
      ['project_save_run', { runId: 6, target: { into: 'openProject', projectId: ID } }],
      ['project_open', { projectId: ID }],
      ['project_close', {}],
      ['project_runs', { projectId: ID }],
      ['project_run_result', { projectId: ID, storedRunId: 1 }],
      ['project_report', { projectId: ID }],
      ['project_provenance', { projectId: ID }],
      ['project_rerun', { runId: 3 }],
    ]);
    expect(Object.keys(PROJECT_COMMANDS)).toHaveLength(13);
  });

  it('names exactly the commands Rust registers', () => {
    const lib = fs.readFileSync(path.join(__dirname, '../../../../src-tauri/src/lib.rs'), 'utf8');
    const registered = Array.from(new Set(lib.match(/project_commands::project_[a-z_]+/g) ?? []))
      .map((p) => p.replace('project_commands::', ''))
      .sort();
    expect(registered).toEqual(Object.values(PROJECT_COMMANDS).slice().sort());
  });
});
