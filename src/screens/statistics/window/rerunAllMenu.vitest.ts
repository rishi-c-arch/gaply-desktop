// Project, Run every saved result again… in the menus (Research OS, Phase
// 8U-2, decision 538): its states in their order, and, while the list goes, the
// list's sentence in place of a run's on every item that says a run is going,
// and Open a project… greyed. Pure: the menus' table, no screen.
import { describe, expect, it, vi } from 'vitest';

import type { MenuDefinition, MenuItem } from '../../../design-system';
import { METHODS } from '../methods';
import type { CapabilityView } from '../ttestTypes';
import { SCALE_SCORE_CAPABILITY, windowMenus, type WindowMenuContext } from './windowMenus';
import { FILE_BUSY, NO_PROJECT_OPEN, RERUNS_GOING, RUN_GOING } from './windowWording';

const capability = (id: string): CapabilityView => ({
  id,
  version: 1,
  lifecycle: 'beta',
  titleCode: `${id}.title`,
  descriptionCode: `${id}.description`,
  licenseNotes: [],
  knownLimitations: [],
  runnable: true,
});
const ALL = [...METHODS.map((m) => capability(m.capabilityId)), capability(SCALE_SCORE_CAPABILITY)];
const LABEL = 'Run every saved result again…';

const context = (over: Partial<WindowMenuContext> = {}): WindowMenuContext => ({
  fileOpen: true,
  runGoing: false,
  busy: false,
  capabilities: ALL,
  projects: true,
  statsCheck: true,
  workspaces: true,
  projectOpen: true,
  actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn() },
  ...over,
});
const find = (ms: MenuDefinition[], id: string): MenuItem => {
  const i = ms.flatMap((m) => m.groups.flatMap((g) => g.items)).find((x) => x.id === id);
  if (!i) throw new Error(`no ${id}`);
  return i;
};

describe('Run every saved result again… (decision 538)', () => {
  it('is labelled with its ellipsis, since it opens a dialog', () => {
    expect(find(windowMenus(context()), 'project-rerun-all').label).toBe(LABEL);
  });

  it('live with a project open and nothing going, calling rerunAll', () => {
    const rerunAll = vi.fn();
    const i = find(windowMenus(context({ actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), rerunAll } })), 'project-rerun-all');
    expect(i.disabled).toBeFalsy();
    expect(i.description).toBeUndefined();
    i.onSelect?.();
    expect(rerunAll).toHaveBeenCalledTimes(1);
  });

  it('says why not, in its order: no project, then a run going, then busy', () => {
    const said = (o: Partial<WindowMenuContext>) => find(windowMenus(context(o)), 'project-rerun-all');
    expect(said({ projectOpen: false, runGoing: true, busy: true }).description).toBe(NO_PROJECT_OPEN);
    expect(said({ projectOpen: undefined }).description).toBe(NO_PROJECT_OPEN);
    expect(said({ runGoing: true, busy: true }).description).toBe(RUN_GOING);
    expect(said({ busy: true }).description).toBe(FILE_BUSY);
    for (const o of [{ projectOpen: false }, { runGoing: true }, { busy: true }]) {
      expect(said(o).disabled).toBe(true);
      expect(said(o).onSelect).toBeUndefined();
    }
  });

  it('live while the list goes, whatever else holds, opening the dialog on the list', () => {
    const rerunAll = vi.fn();
    const i = find(windowMenus(context({ rerunsGoing: true, runGoing: true, busy: true, projectOpen: false, actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), rerunAll } })), 'project-rerun-all');
    expect(i.disabled).toBeFalsy();
    i.onSelect?.();
    expect(rerunAll).toHaveBeenCalledTimes(1);
  });
});

describe('the other items while the list goes (decision 538)', () => {
  const going = windowMenus(context({ rerunsGoing: true, runGoing: true }));
  const between = windowMenus(context({ rerunsGoing: true, runGoing: false }));

  it("every item that says a run is going says the list's sentence instead, also between two reruns", () => {
    const ids = ['file-open', 'file-close', ...METHODS.map((m) => `analyze-${m.kind}`), 'transform-scale-score'];
    for (const ms of [going, between]) {
      for (const id of ids) {
        const i = find(ms, id);
        expect(i.disabled, id).toBe(true);
        expect(i.description, id).toBe(RERUNS_GOING);
      }
    }
  });

  it('with a run going and no list, they say a run is going, as before', () => {
    const ms = windowMenus(context({ runGoing: true }));
    for (const id of ['file-open', 'file-close', 'analyze-describe', 'transform-scale-score']) expect(find(ms, id).description, id).toBe(RUN_GOING);
  });

  it('Open a project… is greyed with the list\'s sentence while it goes, and only then', () => {
    expect([find(going, 'project-open').disabled, find(going, 'project-open').description]).toEqual([true, RERUNS_GOING]);
    expect([find(between, 'project-open').disabled, find(between, 'project-open').description]).toEqual([true, RERUNS_GOING]);
    const live = find(windowMenus(context({ runGoing: true })), 'project-open');
    expect(live.disabled).toBeFalsy();
  });

  it('Export the project report… and Export the prepared data are unchanged by it', () => {
    expect(find(going, 'project-report').disabled).toBeFalsy();
    const plain = find(windowMenus(context({ prepared: true })), 'file-export');
    const withList = find(windowMenus(context({ prepared: true, rerunsGoing: true, runGoing: true })), 'file-export');
    expect([withList.disabled, withList.description]).toEqual([plain.disabled, plain.description]);
  });
});
