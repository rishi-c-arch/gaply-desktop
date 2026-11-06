// The one-way ANOVA's dialog (Research OS, Phase 8S, decisions 481 and 484; ADR-012 Part
// 6), over a mock bridge. The twins of AnovaFlow.vitest.tsx's tests at :112 and :140.
// Every assertion on its test ids and its spec is that file's. What differs:
// * toSettingsWithGroup becomes the dialog's opening and the same two choices, and
//   an-check its Run;
// * :112 checks twice through an-back-settings, and a dialog starts a run at its first
//   valid press, so it is written as two openings of the dialog.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import AnovaFlow from '../AnovaFlow';
import { makeMockAnovaBridge } from '../anovaBridge';
import type { AnovaRunStateView, AnovaSpec, AnovaValidationView } from '../anovaTypes';
import type { DatasetView } from '../describeTypes';
import { AnovaDialog } from './AnovaDialog';
import { OUTPUT_FULL } from './windowWording';

vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const view: DatasetView = {
  slot: 3,
  displayName: 'trial.csv',
  profile: {
    rows: 16,
    columns: [
      { alias: 'C1', header: 'score', kind: 'numeric', n: 16, missing: 0, distinct: 15, levels: null, suspectMissingCodes: [{ code: 99, cells: 2 }], nonFinite: 0 },
      {
        alias: 'C2',
        header: 'arm',
        kind: 'categorical',
        n: 16,
        missing: 0,
        distinct: 4,
        levels: [
          { label: 'a', count: 4 },
          { label: 'b', count: 4 },
          { label: 'c', count: 4 },
          { label: 'd', count: 4 },
        ],
        suspectMissingCodes: [],
        nonFinite: 0,
      },
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 16, columns: 2, semanticRole: 'table' },
};

const report = (status: AnovaValidationView['status']): AnovaValidationView => ({ status, errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 16, columns: 2, cells: 32 } });
const running: AnovaRunStateView = { runId: 5, state: 'running', terminal: false, elapsedMs: 1200, cancelRequested: false, stepsDone: 3, stepsTotal: 46, diagnostics: [] };
const levels = ({ slot, column }: Record<string, unknown>) => ({ slot, column, levels: ['a', 'b', 'c', 'd'].map((label, index) => ({ index, label, count: 4 })) });

function setup(responses: Record<string, unknown>, over: { outputFull?: boolean; v?: DatasetView } = {}) {
  const bridge = makeMockAnovaBridge({ stats_anova_group_levels: levels, stats_anova_run: { runId: 5 }, stats_anova_status: running, ...responses });
  const onStarted = vi.fn();
  const onCancel = vi.fn();
  const utils = render(<AnovaDialog view={over.v ?? view} slot={3} bridge={bridge} outputFull={over.outputFull ?? false} onStarted={onStarted} onCancel={onCancel} />);
  return { bridge, onStarted, onCancel, ...utils };
}

const specs = (bridge: ReturnType<typeof makeMockAnovaBridge>, cmd: string) => bridge.calls.filter(([c]) => c === cmd).map(([, a]) => a.spec as AnovaSpec);
const box = (id: string) => screen.getByTestId(id) as HTMLInputElement;
const run = () => screen.getByTestId('an-run') as HTMLButtonElement;
async function withGroup() {
  fireEvent.change(screen.getByTestId('an-outcome'), { target: { value: 'C1' } });
  fireEvent.change(screen.getByTestId('an-group'), { target: { value: 'C2' } });
  await screen.findByTestId('an-level-3');
}

describe('the twins of AnovaFlow.vitest.tsx', () => {
  it('ticks every level and no post hoc test, and sends one spec per choice: as they stand', async () => {
    const { bridge } = setup({ stats_anova_validate: report('valid') });
    await withGroup();
    for (const i of [0, 1, 2, 3]) expect(box(`an-level-${i}`).checked, `level ${i}`).toBe(true);
    for (const t of ['tukey', 'games_howell', 'bonferroni']) expect(box(`an-post-hoc-${t}`).checked, t).toBe(false);
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_anova_validate')).toHaveLength(1));
    expect(specs(bridge, 'stats_anova_validate')[0].parameters).toEqual({ outcome: 'C1', group: 'C2', levels: null, postHoc: [], missingCodes: [] });
  });

  it('…a level unticked and two tests chosen', async () => {
    const { bridge } = setup({ stats_anova_validate: report('valid') });
    await withGroup();
    fireEvent.click(screen.getByTestId('an-level-1'));
    fireEvent.click(screen.getByTestId('an-post-hoc-bonferroni'));
    fireEvent.click(screen.getByTestId('an-post-hoc-tukey'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_anova_validate')).toHaveLength(1));
    const p = specs(bridge, 'stats_anova_validate')[0].parameters;
    expect(p.levels).toEqual([0, 2, 3]);
    expect(p.postHoc).toEqual(['tukey', 'bonferroni']);
  });

  it('keeps Run off with fewer than two levels ticked, and says so', async () => {
    setup({});
    await withGroup();
    for (const i of [0, 1, 2]) fireEvent.click(screen.getByTestId(`an-level-${i}`));
    expect(screen.getByTestId('an-levels-too-few').textContent).toBe('1 level is ticked. Tick at least two.');
    expect(run().disabled).toBe(true);
    fireEvent.click(screen.getByTestId('an-level-0'));
    expect(screen.queryByTestId('an-levels-too-few')).toBeNull();
    expect(run().disabled).toBe(false);
  });
});

describe('with nothing chosen', () => {
  it('opens with no outcome, no group, no level and no post hoc test, Run off, and its levels rule shown', () => {
    setup({});
    expect((screen.getByTestId('an-outcome') as HTMLSelectElement).value).toBe('');
    expect((screen.getByTestId('an-group') as HTMLSelectElement).value).toBe('');
    expect(screen.queryByTestId('an-levels')).toBeNull();
    for (const t of ['tukey', 'games_howell', 'bonferroni']) expect(box(`an-post-hoc-${t}`).checked, t).toBe(false);
    expect(run().disabled).toBe(true);
    expect(screen.getByTestId('an-levels-rule').textContent).not.toBe('');
  });
});

describe('a prepared score (decision 484)', () => {
  const withScore: DatasetView = {
    ...view,
    profile: {
      ...view.profile,
      columns: [...view.profile.columns, { alias: 'C3', header: 'C3', scoreName: 'Total anxiety', prepared: true, kind: 'numeric', n: 16, missing: 0, distinct: 4, levels: null, suspectMissingCodes: [], nonFinite: 0 }],
    },
  };

  it('is offered as an outcome, by the name the user typed, and never as a group', () => {
    setup({}, { v: withScore });
    expect(Array.from((screen.getByTestId('an-outcome') as HTMLSelectElement).options).find((o) => o.value === 'C3')?.textContent).toBe('C3 Total anxiety');
    expect(Array.from((screen.getByTestId('an-group') as HTMLSelectElement).options).map((o) => o.value)).not.toContain('C3');
  });

  it('nor on the page', async () => {
    const page = makeMockAnovaBridge({ stats_desc_open: withScore, stats_desc_close: true });
    render(<AnovaFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('an-choose'));
    fireEvent.click(await screen.findByTestId('an-to-settings'));
    const group = (await screen.findByTestId('an-group')) as HTMLSelectElement;
    expect(Array.from(group.options).map((o) => o.value)).not.toContain('C3');
  });

  it("Rust's refusal reading a group's levels shows as the dialog's refusal, and nothing runs", async () => {
    const refused = () => {
      throw new Error(JSON.stringify({ code: 'stats.anova.score_as_group' }));
    };
    const { bridge, onStarted } = setup({ stats_anova_group_levels: refused });
    fireEvent.change(screen.getByTestId('an-group'), { target: { value: 'C2' } });
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).not.toBe('');
    expect(bridge.calls.filter(([c]) => c === 'stats_anova_run')).toEqual([]);
    expect(onStarted).not.toHaveBeenCalled();
  });
});

describe('one set of choices gives one spec', () => {
  it("equal to the page's for the same choices", async () => {
    const { bridge, unmount } = setup({ stats_anova_validate: report('valid') });
    await withGroup();
    fireEvent.click(screen.getByTestId('an-level-1'));
    fireEvent.click(screen.getByTestId('an-post-hoc-tukey'));
    fireEvent.click(screen.getByTestId('an-code-99'));
    fireEvent.click(run());
    await waitFor(() => expect(specs(bridge, 'stats_anova_validate')).toHaveLength(1));
    const fromDialog = specs(bridge, 'stats_anova_validate')[0];
    unmount();

    const page = makeMockAnovaBridge({ stats_desc_open: view, stats_desc_close: true, stats_anova_group_levels: levels, stats_anova_validate: report('valid') });
    render(<AnovaFlow bridge={page} pickPath={async () => '/x.csv'} save={vi.fn()} pollMs={10} limits={async () => null} />);
    fireEvent.click(screen.getByTestId('an-choose'));
    fireEvent.click(await screen.findByTestId('an-to-settings'));
    await screen.findByTestId('an-outcome');
    await withGroup();
    fireEvent.click(screen.getByTestId('an-level-1'));
    fireEvent.click(screen.getByTestId('an-post-hoc-tukey'));
    fireEvent.click(screen.getByTestId('an-code-99'));
    fireEvent.click(screen.getByTestId('an-check'));
    await waitFor(() => expect(specs(page, 'stats_anova_validate')).toHaveLength(1));
    expect({ ...fromDialog, operationId: 0 }).toEqual({ ...specs(page, 'stats_anova_validate')[0], operationId: 0 });
  });
});

describe("MethodDialog's rule, for the ANOVA", () => {
  it('valid: hands the run to its parent, with its own settings line', async () => {
    const { onStarted } = setup({ stats_anova_validate: report('valid') });
    await withGroup();
    fireEvent.click(screen.getByTestId('an-level-1'));
    fireEvent.click(screen.getByTestId('an-post-hoc-tukey'));
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalledTimes(1));
    const s = onStarted.mock.calls[0][0];
    expect(s.runId).toBe(5);
    expect(s.settings).toContain('Outcome: C1 score. Group: C2 arm. Levels: a, c, d.');
    expect(s.settings).toContain('Missing codes: none.');
  });

  it('with the Output full, Run says so and nothing is validated or run', async () => {
    const { bridge, onStarted } = setup({}, { outputFull: true });
    await withGroup();
    const before = bridge.calls.length;
    fireEvent.click(run());
    expect((await screen.findByTestId('sw-dialog-refusal')).textContent).toBe(OUTPUT_FULL);
    expect(bridge.calls).toHaveLength(before);
    expect(onStarted).not.toHaveBeenCalled();
  });

  it('never closes the slot: not on mount, Cancel, Run or unmount', async () => {
    const { bridge, onStarted, unmount } = setup({ stats_anova_validate: report('valid') });
    const closes = () => bridge.calls.filter(([c]) => c === 'stats_desc_close');
    expect(closes()).toEqual([]);
    await withGroup();
    fireEvent.click(run());
    await waitFor(() => expect(onStarted).toHaveBeenCalled());
    expect(closes()).toEqual([]);
    fireEvent.click(screen.getByTestId('an-dialog-cancel'));
    expect(closes()).toEqual([]);
    unmount();
    expect(closes()).toEqual([]);
  });
});

describe('one copy of the settings', () => {
  const src = (f: string) => fs.readFileSync(path.join(__dirname, f), 'utf8');

  it('the page and the dialog each render AnovaSettings, by import, and neither holds a control', () => {
    for (const f of ['../AnovaFlow.tsx', 'AnovaDialog.tsx']) {
      expect(src(f), f).toMatch(/import \{[^}]*\bAnovaSettings\b[^}]*\buseAnovaSettings\b[^}]*\} from '[^']*AnovaSettings'/);
      expect(src(f), f).toMatch(/<AnovaSettings\b/);
      expect(src(f), f).not.toMatch(/data-testid="an-outcome"|data-testid="an-group"|an-level-\$\{|an-post-hoc-\$\{|an-code-/);
    }
    expect(src('../AnovaSettings.tsx')).toMatch(/an-level-/);
    expect(src('MethodDialog.tsx')).not.toMatch(/an-level-/);
  });

  it('its bridge type omits open and close, keeps groupLevels, and it names no close', () => {
    expect(src('AnovaDialog.tsx')).toMatch(/Omit<AnovaBridge, 'open' \| 'close'>/);
    for (const f of ['AnovaDialog.tsx', 'MethodDialog.tsx']) expect(src(f), f).not.toMatch(/\.close\(|stats_desc_close/);
  });
});
