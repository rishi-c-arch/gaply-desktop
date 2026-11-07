// The window's menus as a table (Research OS, Phase 8P, decision 451; ADR-012
// Part 5): every item in decision 441's words and order, and each item's state
// with no file, with a file and with a run going. Rust's capability list
// decides what may run; "Coming later" is a closed list of plain labels.
import { describe, expect, it, vi } from 'vitest';

import type { MenuDefinition, MenuItem } from '../../../design-system';
import { METHODS } from '../methods';
import type { CapabilityView } from '../ttestTypes';
import { COMING_LATER, LIVE_METHODS, SCALE_SCORE_CAPABILITY, windowMenus, type WindowMenuContext } from './windowMenus';
import { LATER_STEP, NOT_RUNNABLE, NO_FILE, RUN_GOING } from './windowWording';

const capability = (id: string, runnable = true): CapabilityView => ({
  id,
  version: 1,
  lifecycle: 'beta',
  titleCode: `${id}.title`,
  descriptionCode: `${id}.description`,
  licenseNotes: [],
  knownLimitations: [],
  runnable,
});
const ALL = [...METHODS.map((m) => capability(m.capabilityId)), capability(SCALE_SCORE_CAPABILITY)];

function context(over: Partial<WindowMenuContext> = {}): WindowMenuContext {
  return {
    fileOpen: false,
    runGoing: false,
    busy: false,
    capabilities: ALL,
    projects: true,
    statsCheck: true,
    workspaces: true,
    actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn() },
    ...over,
  };
}

const menu = (ms: MenuDefinition[], id: string) => {
  const m = ms.find((x) => x.id === id);
  if (!m) throw new Error(`no menu ${id}`);
  return m;
};
const items = (m: MenuDefinition): MenuItem[] => m.groups.reduce<MenuItem[]>((out, g) => out.concat(g.items), []);
const item = (ms: MenuDefinition[], menuId: string, label: string): MenuItem => {
  const i = items(menu(ms, menuId)).find((x) => x.label === label);
  if (!i) throw new Error(`no item ${label}`);
  return i;
};
const shape = (m: MenuDefinition) => m.groups.map((g) => [g.heading ?? null, g.items.map((i) => i.label)]);

describe('every item, in decision 441\'s words and order', () => {
  const ms = windowMenus(context());

  it('the menus', () => {
    expect(ms.map((m) => m.label)).toEqual(['Gaply', 'File', 'Transform', 'Analyze', 'Project', 'Help']);
    expect(menu(ms, 'gaply').after).toBe('Statistics');
  });

  it('File', () => {
    expect(shape(menu(ms, 'file'))).toEqual([[null, ['Open a data file…', 'Close this file', 'Export the prepared data (CSV)…']]]);
  });

  it('Transform', () => {
    expect(shape(menu(ms, 'transform'))).toEqual([
      [null, ['Scale score…']],
      ['Coming later', ['Recode values…', 'Compute from a formula…', 'Select cases…']],
    ]);
  });

  it('Analyze', () => {
    expect(shape(menu(ms, 'analyze'))).toEqual([
      ['Descriptive statistics', ['Summary…']],
      ['Compare means', ['Independent-samples t-test…', 'One-sample t-test…', 'Paired t-test…', 'One-way ANOVA…']],
      ['Correlate', ['Correlations…']],
      ['Scale', ["Reliability (Cronbach's alpha)…"]],
      ['Coming later', ['Crosstabs and chi-square', 'Regression', 'Factor analysis', 'Nonparametric tests', 'CFA and SEM']],
    ]);
  });

  it('Project', () => {
    expect(shape(menu(ms, 'project'))).toEqual([[null, ['Open a project…', 'Run every saved result again', 'Export the project report…']]]);
  });

  it('Help', () => {
    expect(shape(menu(ms, 'help'))).toEqual([[null, ['Methods and limits', 'How Gaply reads a file', 'Keyboard']]]);
  });

  it('Gaply', () => {
    expect(shape(menu(ms, 'gaply'))).toEqual([[null, ['Home', 'Statistical Analysis Check', 'Statistical Analysis Verifier', 'Settings', 'Switch workspace']]]);
  });
});

describe('what the flags hide', () => {
  it('the Stats Check goes while its flag is off', () => {
    expect(items(menu(windowMenus(context({ statsCheck: false })), 'gaply')).map((i) => i.label)).toEqual(['Home', 'Statistical Analysis Verifier', 'Settings', 'Switch workspace']);
  });

  it('Switch workspace goes while the workspaces flag is off', () => {
    expect(items(menu(windowMenus(context({ workspaces: false })), 'gaply')).map((i) => i.label)).toEqual(['Home', 'Statistical Analysis Check', 'Statistical Analysis Verifier', 'Settings']);
  });

  it('the Project menu goes when projectsShown says no', () => {
    expect(windowMenus(context({ projects: false })).map((m) => m.label)).toEqual(['Gaply', 'File', 'Transform', 'Analyze', 'Help']);
  });
});

describe("each live item's state", () => {
  const states: Array<[string, Partial<WindowMenuContext>, string | undefined, string | undefined]> = [
    ['no file', {}, undefined, NO_FILE],
    ['a file', { fileOpen: true }, undefined, undefined],
    ['a file and a run going', { fileOpen: true, runGoing: true }, RUN_GOING, RUN_GOING],
    ['no file and a run going', { runGoing: true }, RUN_GOING, NO_FILE],
  ];

  it.each(states)('with %s', (_name, over, open, close) => {
    const ms = windowMenus(context(over));
    expect(item(ms, 'file', 'Open a data file…').description).toBe(open);
    expect(item(ms, 'file', 'Open a data file…').disabled === true).toBe(open !== undefined);
    expect(item(ms, 'file', 'Close this file').description).toBe(close);
    expect(item(ms, 'file', 'Close this file').disabled === true).toBe(close !== undefined);
  });

  it('the sentences are the ADR\'s, word for word', () => {
    expect(NO_FILE).toBe('Open a data file first.');
    expect(RUN_GOING).toBe('A run is going. Wait for it, or cancel it in the Output.');
    expect(NOT_RUNNABLE).toBe('This version of Gaply cannot run this method.');
    expect(LATER_STEP).toBe('This comes in a later step of the build.');
  });

  it('while a picker or an open is going, Open and Close wait', () => {
    const ms = windowMenus(context({ fileOpen: true, busy: true }));
    expect(item(ms, 'file', 'Open a data file…').disabled).toBe(true);
    expect(item(ms, 'file', 'Close this file').disabled).toBe(true);
    expect(item(ms, 'file', 'Open a data file…').description).toBeTruthy();
  });

  it('Open and Close call what they name', () => {
    const c = context({ fileOpen: true });
    const ms = windowMenus(c);
    item(ms, 'file', 'Open a data file…').onSelect?.();
    item(ms, 'file', 'Close this file').onSelect?.();
    expect(c.actions.open).toHaveBeenCalledTimes(1);
    expect(c.actions.close).toHaveBeenCalledTimes(1);
  });

  it('Help and Gaply stay live with no file and with a run going', () => {
    for (const over of [{}, { runGoing: true }]) {
      const c = context(over);
      const ms = windowMenus(c);
      for (const i of [...items(menu(ms, 'help')), ...items(menu(ms, 'gaply'))]) {
        expect(i.disabled, i.label).toBeFalsy();
        expect(i.description, i.label).toBeUndefined();
        expect(typeof i.onSelect, i.label).toBe('function');
      }
    }
  });

  it('Gaply goes where each item says, and Help to its topic', () => {
    const c = context();
    const ms = windowMenus(c);
    const go = c.actions.go as ReturnType<typeof vi.fn>;
    item(ms, 'gaply', 'Home').onSelect?.();
    item(ms, 'gaply', 'Statistical Analysis Check').onSelect?.();
    item(ms, 'gaply', 'Statistical Analysis Verifier').onSelect?.();
    item(ms, 'gaply', 'Settings').onSelect?.();
    expect(go.mock.calls.map(([p]) => p)).toEqual(['/app', '/app/check/stats', '/app/statsverifier', '/app/settings']);
    item(ms, 'gaply', 'Switch workspace').onSelect?.();
    expect(c.actions.switchWorkspace).toHaveBeenCalledTimes(1);
    item(ms, 'help', 'Keyboard').onSelect?.();
    item(ms, 'help', 'How Gaply reads a file').onSelect?.();
    item(ms, 'help', 'Methods and limits').onSelect?.();
    expect((c.actions.help as ReturnType<typeof vi.fn>).mock.calls.map(([t]) => t)).toEqual(['keyboard', 'file', 'methods']);
  });
});

describe('the items that come in a later step', () => {
  const LATER: Array<[string, string]> = [
    ['file', 'Export the prepared data (CSV)…'],
    ['project', 'Open a project…'],
    ['project', 'Run every saved result again'],
    ['project', 'Export the project report…'],
  ];

  it.each(LATER)('%s, %s: disabled, with the later-step sentence, and no handler, in every state', (m, label) => {
    for (const over of [{}, { fileOpen: true }, { fileOpen: true, runGoing: true }]) {
      const i = item(windowMenus(context(over)), m, label);
      expect(i.disabled).toBe(true);
      expect(i.description).toBe(LATER_STEP);
      expect(i.onSelect).toBeUndefined();
    }
  });
});

describe('what Rust says may run', () => {
  const ROWS: Array<[string, string, string]> = [
    ...METHODS.map((m): [string, string, string] => ['analyze', m.menuLabel, m.capabilityId]),
    ['transform', 'Scale score…', SCALE_SCORE_CAPABILITY],
  ];

  // A method's item is live once its dialog exists (decision 485), and has its tests below.
  const notLive = (r: [string, string, string]) => r[2] !== SCALE_SCORE_CAPABILITY && !METHODS.some((m) => m.capabilityId === r[2] && LIVE_METHODS.includes(m.kind));
  it.each(ROWS.filter(notLive))('%s, %s: Rust calling it runnable shows the later-step sentence', (m, label) => {
    const i = item(windowMenus(context({ fileOpen: true })), m, label);
    expect(i.disabled).toBe(true);
    expect(i.description).toBe(LATER_STEP);
    expect(i.onSelect).toBeUndefined();
  });

  it.each(ROWS)('%s, %s: Rust not calling it runnable shows the not-runnable sentence, which wins', (m, label, id) => {
    const notRunnable = ALL.map((c) => (c.id === id ? { ...c, runnable: false } : c));
    expect(item(windowMenus(context({ capabilities: notRunnable })), m, label).description).toBe(NOT_RUNNABLE);
  });

  it.each(ROWS)('%s, %s: not in Rust\'s list, or the list unread, is not runnable', (m, label, id) => {
    expect(item(windowMenus(context({ capabilities: ALL.filter((c) => c.id !== id) })), m, label).description).toBe(NOT_RUNNABLE);
    expect(item(windowMenus(context({ capabilities: null })), m, label).description).toBe(NOT_RUNNABLE);
  });
});

describe('Coming later', () => {
  const ms = windowMenus(context({ fileOpen: true }));
  const later = (id: string) => {
    const g = menu(ms, id).groups.find((x) => x.heading === 'Coming later');
    if (!g) throw new Error('no heading');
    return g.items;
  };

  it('is the closed list decision 441 gives, and the table is what the menus show', () => {
    expect(COMING_LATER.analyze).toEqual(['Crosstabs and chi-square', 'Regression', 'Factor analysis', 'Nonparametric tests', 'CFA and SEM']);
    expect(COMING_LATER.transform).toEqual(['Recode values…', 'Compute from a formula…', 'Select cases…']);
    expect(later('analyze').map((i) => i.label)).toEqual(COMING_LATER.analyze);
    expect(later('transform').map((i) => i.label)).toEqual(COMING_LATER.transform);
  });

  it.each(['analyze', 'transform'])('%s: each is a plain label: disabled, with no id, handler, sentence or capability', (id) => {
    for (const i of later(id)) {
      expect(Object.keys(i).sort(), i.label).toEqual(['disabled', 'label']);
      expect(i.disabled, i.label).toBe(true);
    }
  });

  it.each(['analyze', 'transform'])('%s: none carries a date or a promise', (id) => {
    const NO = /\b(19|20)\d\d\b|\b\d{1,2}\/\d{1,2}\b|\bq[1-4]\b|\b(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\b|\b(soon|next|planned|upcoming|release|version|beta|stats\.)\b/i;
    for (const i of later(id)) expect(i.label).not.toMatch(NO);
  });
});

describe("alpha's item, live (decision 473)", () => {
  const LABEL = "Reliability (Cronbach's alpha)…";

  it('no file, a run going, a file busy, the Output full: disabled, each with its sentence', () => {
    const said = (over: Partial<WindowMenuContext>) => item(windowMenus(context(over)), 'analyze', LABEL);
    expect(said({ fileOpen: false }).description).toBe(NO_FILE);
    expect(said({ fileOpen: true, runGoing: true }).description).toBe(RUN_GOING);
    expect(said({ fileOpen: true, outputFull: true }).description).toBe('The Output is full. Remove a result you have kept, then run this.');
    for (const o of [{ fileOpen: false }, { fileOpen: true, runGoing: true }, { fileOpen: true, outputFull: true }]) expect(said(o).disabled).toBe(true);
  });

  it('with a file, it opens the dialog of its own kind', () => {
    const method = vi.fn();
    const i = item(windowMenus(context({ fileOpen: true, actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), method } })), 'analyze', LABEL);
    expect(i.disabled).toBeFalsy();
    i.onSelect?.();
    expect(method).toHaveBeenCalledWith('cronbach_alpha');
  });

  it('Rust not calling it runnable wins, with a file and an empty Output', () => {
    const notRunnable = ALL.map((c) => (c.id === 'stats.analyze.cronbach_alpha' ? { ...c, runnable: false } : c));
    expect(item(windowMenus(context({ fileOpen: true, capabilities: notRunnable })), 'analyze', LABEL).description).toBe(NOT_RUNNABLE);
  });
});

describe('every method, live (decision 485)', () => {
  it('LIVE_METHODS holds all seven', () => {
    expect([...LIVE_METHODS].sort()).toEqual(METHODS.map((m) => m.kind).sort());
  });

  it.each(METHODS.map((m) => [m.menuLabel, m.kind] as const))('%s opens the dialog of its own kind, with a file', (label, kind) => {
    const method = vi.fn();
    const i = item(windowMenus(context({ fileOpen: true, actions: { open: vi.fn(), close: vi.fn(), go: vi.fn(), switchWorkspace: vi.fn(), help: vi.fn(), method } })), 'analyze', label);
    expect(i.disabled).toBeFalsy();
    i.onSelect?.();
    expect(method).toHaveBeenCalledWith(kind);
    expect(item(windowMenus(context({ fileOpen: false })), 'analyze', label).description).toBe(NO_FILE);
  });

  it("Project's items and the prepared data's export keep the later-step sentence, and Transform's scale score is live (decision 506)", () => {
    const ms = windowMenus(context({ fileOpen: true }));
    expect([item(ms, 'transform', 'Scale score…').disabled, item(ms, 'transform', 'Scale score…').description, typeof item(ms, 'transform', 'Scale score…').onSelect]).toEqual([undefined, undefined, 'function']);
    expect(item(ms, 'file', 'Export the prepared data (CSV)…').description).toBe(LATER_STEP);
    for (const label of ['Open a project…', 'Run every saved result again', 'Export the project report…']) expect(item(ms, 'project', label).description).toBe(LATER_STEP);
  });
});
