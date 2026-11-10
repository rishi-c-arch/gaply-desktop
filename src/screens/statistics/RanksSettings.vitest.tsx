// The rank tests' settings (Research OS, Phase 8Y-2, decision 619), on
// AnovaSettings.tsx's: the outcome a numeric column, a scale score among them; the
// group a column that is not a score with 2 to MAX_GROUP_LEVELS distinct values;
// choosing a group fetches its levels and ticks every one, each keyed by its index
// with its label as text; ready when an outcome and a group are chosen, they differ,
// the levels are loaded and at least two are ticked; the codes the outcome's suspect
// codes, ticked by the user, sent ascending and unique.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { MAX_GROUP_LEVELS } from './anovaWording';
import { makeMockRanksBridge } from './ranksBridge';
import { RanksSettings, useRanksSettings, type RanksSettingsState } from './RanksSettings';

afterEach(cleanup);

const col = (alias: string, header: string, distinct: number, over: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias,
  header,
  kind: 'categorical',
  n: 100,
  missing: 0,
  distinct,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

const view: DatasetView = {
  slot: 3,
  displayName: 'survey.csv',
  profile: {
    rows: 100,
    columns: [
      col('C1', 'arm', 3),
      col('C2', 'one level', 1),
      col('C3', 'fifty', MAX_GROUP_LEVELS),
      col('C4', 'fifty-one', MAX_GROUP_LEVELS + 1),
      col('C5', 'age', 40, { kind: 'numeric', suspectMissingCodes: [{ code: 99, cells: 2 }, { code: -9, cells: 1 }] }),
      col('C6', 'total', 9, { kind: 'numeric', prepared: true, scoreName: 'Total' }),
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 100, columns: 6, semanticRole: 'table' },
};

const LEVELS = [
  { index: 0, label: 'LVL_a', count: 4 },
  { index: 1, label: 'LVL_b', count: 5 },
  { index: 2, label: 'LVL_c', count: 6 },
];

let state: RanksSettingsState | null = null;
const bridgeFor = () => makeMockRanksBridge({ stats_ranks_group_levels: { slot: 3, column: 'C1', levels: LEVELS } });
const Harness: React.FC<{ bridge?: ReturnType<typeof bridgeFor>; v?: DatasetView }> = ({ bridge = bridgeFor(), v = view }) => {
  const s = useRanksSettings(v, { bridge, slot: () => 3, act: (f) => f() });
  state = s;
  return <RanksSettings view={v} settings={s} />;
};

const options = (id: string) => Array.from((screen.getByTestId(id) as HTMLSelectElement).options).map((o) => o.value);
const pick = (id: string, value: string) => fireEvent.change(screen.getByTestId(id), { target: { value } });
const pickGroup = async (value: string) => {
  pick('rk-group', value);
  await waitFor(() => expect(screen.getByTestId('rk-levels')).toBeTruthy());
};

describe('the pickers', () => {
  it("offers the numeric columns as the outcome, a score among them, and only a file's own column with 2 to 50 distinct values as the group", () => {
    render(<Harness />);
    expect(options('rk-outcome')).toEqual(['', 'C5', 'C6']);
    expect(options('rk-group')).toEqual(['', 'C1', 'C3', 'C5']);
    expect(screen.getByTestId('rk-group-limit').textContent).toBe(`A column with more than ${MAX_GROUP_LEVELS} distinct values is not offered as a group.`);
  });

  it('shows no score as a group, by name either', () => {
    render(<Harness />);
    const group = screen.getByTestId('rk-group') as HTMLSelectElement;
    expect(Array.from(group.options).some((o) => o.textContent?.includes('Total'))).toBe(false);
  });
});

describe('the levels list', () => {
  it('reads the levels through the rank tests command and ticks every one, each keyed by its index', async () => {
    const bridge = bridgeFor();
    render(<Harness bridge={bridge} />);
    await pickGroup('C1');
    expect(bridge.calls).toEqual([['stats_ranks_group_levels', { slot: 3, column: 'C1' }]]);
    expect(state?.tickedLevels).toEqual([0, 1, 2]);
    expect(Array.from(screen.getByTestId('rk-levels').querySelectorAll('input')).map((i) => i.getAttribute('data-testid'))).toEqual(['rk-level-0', 'rk-level-1', 'rk-level-2']);
    expect(screen.getByTestId('rk-level-1').closest('label')?.textContent?.trim()).toBe('LVL_b (5)');
  });

  it('holds no label in an attribute', async () => {
    const { container } = render(<Harness />);
    await pickGroup('C1');
    const attrs = Array.from(container.querySelectorAll('*')).flatMap((el) => Array.from(el.attributes).map((a) => a.value));
    expect(attrs.filter((a) => a.includes('LVL_'))).toEqual([]);
  });

  it('says so when fewer than two are ticked', async () => {
    render(<Harness />);
    await pickGroup('C1');
    fireEvent.click(screen.getByTestId('rk-level-1'));
    fireEvent.click(screen.getByTestId('rk-level-2'));
    expect(screen.getByTestId('rk-levels-too-few').textContent).toBe('1 level is ticked. Tick at least two.');
  });

  it("keeps the levels of the group chosen last when an earlier group's answer comes later", async () => {
    const answers: Record<string, (v: unknown) => void> = {};
    const bridge = {
      ...bridgeFor(),
      groupLevels: (_slot: number, column: string) => new Promise((resolve) => (answers[column] = resolve)),
    } as unknown as ReturnType<typeof bridgeFor>;
    render(<Harness bridge={bridge} />);
    await act(async () => {
      pick('rk-group', 'C1');
    });
    await act(async () => {
      pick('rk-group', 'C3');
    });
    const later = [
      { index: 0, label: 'third', count: 1 },
      { index: 1, label: 'fourth', count: 2 },
    ];
    await act(async () => answers.C3({ slot: 3, column: 'C3', levels: later }));
    await act(async () => answers.C1({ slot: 3, column: 'C1', levels: LEVELS }));
    expect(state?.group).toBe('C3');
    expect(state?.groupLevels).toEqual(later);
    expect(state?.tickedLevels).toEqual([0, 1]);
  });

  it('ignores an answer that comes after a reset', async () => {
    let answer: (v: unknown) => void = () => undefined;
    const bridge = { ...bridgeFor(), groupLevels: () => new Promise((resolve) => (answer = resolve)) } as unknown as ReturnType<typeof bridgeFor>;
    render(<Harness bridge={bridge} />);
    await act(async () => {
      pick('rk-group', 'C1');
    });
    act(() => state?.reset());
    await act(async () => answer({ slot: 3, column: 'C1', levels: LEVELS }));
    expect(state?.groupLevels).toEqual([]);
    expect(state?.tickedLevels).toEqual([]);
  });

  it('drops the levels when the group is cleared', async () => {
    render(<Harness />);
    await pickGroup('C1');
    await act(async () => {
      pick('rk-group', '');
    });
    expect(screen.queryByTestId('rk-levels')).toBeNull();
    expect(state?.tickedLevels).toEqual([]);
  });
});

describe('readiness', () => {
  it('needs an outcome, a group, the levels loaded and two levels ticked', async () => {
    render(<Harness />);
    expect(state?.ready).toBe(false);
    pick('rk-outcome', 'C5');
    expect(state?.ready).toBe(false);
    await pickGroup('C1');
    expect(state?.ready).toBe(true);
    fireEvent.click(screen.getByTestId('rk-level-0'));
    expect(state?.ready).toBe(true);
    fireEvent.click(screen.getByTestId('rk-level-1'));
    expect(state?.ready).toBe(false);
  });

  it('is not ready with the outcome as the group, and says so', async () => {
    const bridge = makeMockRanksBridge({ stats_ranks_group_levels: { slot: 3, column: 'C5', levels: LEVELS } });
    render(<Harness bridge={bridge} />);
    pick('rk-outcome', 'C5');
    await pickGroup('C5');
    expect(state?.ready).toBe(false);
    expect(screen.getByTestId('rk-same-column').textContent).toBe('The outcome and the group are the same column. Choose two different columns.');
  });
});

describe('the missing codes', () => {
  it("offers the outcome's suspect codes and sends the ticked ones ascending and once each", async () => {
    render(<Harness />);
    expect(screen.queryByTestId('rk-missing-codes')).toBeNull();
    pick('rk-outcome', 'C5');
    await pickGroup('C1');
    fireEvent.click(screen.getByTestId('rk-code-99'));
    fireEvent.click(screen.getByTestId('rk-code--9'));
    expect(state?.spec(1).parameters).toEqual({ outcome: 'C5', group: 'C1', levels: null, missingCodes: [-9, 99] });
    expect(screen.getByTestId('rk-code-99').closest('label')?.textContent?.trim()).toBe(
      'Is 99 a code for a missing value? It appears in the outcome "age" (2 cells). If you tick it, every row whose outcome is 99 is left out.'
    );
  });

  it('sends a choice of levels, a score as the outcome, and no code the new outcome does not hold', async () => {
    render(<Harness />);
    pick('rk-outcome', 'C5');
    await pickGroup('C1');
    fireEvent.click(screen.getByTestId('rk-code-99'));
    fireEvent.click(screen.getByTestId('rk-level-1'));
    expect(state?.spec(1).parameters.levels).toEqual([0, 2]);
    pick('rk-outcome', 'C6');
    expect(state?.spec(1).parameters).toEqual({ outcome: 'C6', group: 'C1', levels: [0, 2], missingCodes: [] });
  });

  it('reset drops every choice', async () => {
    render(<Harness />);
    pick('rk-outcome', 'C5');
    await pickGroup('C1');
    act(() => state?.reset());
    expect(state?.outcome).toBe('');
    expect(state?.group).toBe('');
    expect(state?.groupLevels).toEqual([]);
    expect(state?.ticked).toEqual([]);
  });

  it('never fetches levels without a slot', async () => {
    const bridge = bridgeFor();
    const act2 = vi.fn((f: () => Promise<void>) => f());
    const NoSlot: React.FC = () => {
      const s = useRanksSettings(view, { bridge, slot: () => null, act: act2 });
      state = s;
      return <RanksSettings view={view} settings={s} />;
    };
    render(<NoSlot />);
    await act(async () => {
      pick('rk-group', 'C1');
    });
    expect(bridge.calls).toEqual([]);
    expect(state?.groupLevels).toEqual([]);
  });
});
