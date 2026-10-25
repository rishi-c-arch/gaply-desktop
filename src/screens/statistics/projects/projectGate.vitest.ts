// REACT_APP_FEATURE_PROJECTS and the one helper that decides whether the
// project screens show (Research OS, Phase 5B-3a; decisions 119 and 136):
// off by default, and on only with REACT_APP_FEATURE_WORKSPACES on too, and
// only in the desktop app.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it, vi } from 'vitest';

import { projectsShown } from './projectGate';

const flags = vi.hoisted(() => ({ projects: false, workspaces: false }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => (name === 'projects' && flags.projects) || (name === 'workspaces' && flags.workspaces),
}));

describe('REACT_APP_FEATURE_PROJECTS', () => {
  it('is off by default', async () => {
    const real = await vi.importActual<typeof import('../../../config/featureFlags')>('../../../config/featureFlags');
    expect(process.env.REACT_APP_FEATURE_PROJECTS).toBeUndefined();
    expect(real.FEATURE_FLAGS.projects).toBe(false);
    expect(real.isFeatureEnabled('projects')).toBe(false);
  });

  it('is set by no committed .env file', () => {
    const root = path.join(__dirname, '../../../..');
    const envs = fs.readdirSync(root).filter((f) => f.startsWith('.env'));
    for (const f of envs) {
      expect(fs.readFileSync(path.join(root, f), 'utf8'), f).not.toMatch(/REACT_APP_FEATURE_PROJECTS\s*=\s*(true|1)/);
    }
  });
});

describe('projectsShown', () => {
  it.each([
    [false, false, false, false],
    [true, false, true, false],
    [false, true, true, false],
    [true, true, false, false],
    [false, false, true, false],
    [true, true, true, true],
  ])('projects=%s workspaces=%s desktop=%s gives %s', (projects, workspaces, desktop, shown) => {
    flags.projects = projects;
    flags.workspaces = workspaces;
    expect(projectsShown(desktop)).toBe(shown);
  });
});
