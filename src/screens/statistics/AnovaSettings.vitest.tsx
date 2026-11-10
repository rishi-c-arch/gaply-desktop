// The one-way ANOVA's settings against stale answers and a change of outcome (Research OS, Phase
// 8Z-1, decision 635), on RanksSettings.vitest.tsx's harness, but with an act built by
// actWith, since a bare act can observe neither a refusal nor busy. Each race holds the
// levels' answers back and lets them land in the order the test chooses.
import React, { useState } from 'react';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import type { ColumnProfile, DatasetView } from './describeTypes';
import { actWith } from './flowHooks';

import { AnovaSettings, useAnovaSettings, type AnovaSettingsState } from './AnovaSettings';

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
      col('C1', 'arm', 2),
      col('C3', 'site', 3),
      col('C5', 'age', 40, { kind: 'numeric', suspectMissingCodes: [{ code: 99, cells: 2 }] }),
      col('C6', 'weight', 41, { kind: 'numeric', suspectMissingCodes: [{ code: 99, cells: 4 }] }),
    ],
  },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 100, columns: 4, semanticRole: 'table' },
};

const FIRST = [
  { index: 0, label: 'a', count: 4 },
  { index: 1, label: 'b', count: 5 },
];
const NEWER = [
  { index: 0, label: 'x', count: 1 },
  { index: 1, label: 'y', count: 2 },
  { index: 2, label: 'z', count: 3 },
];

let state: AnovaSettingsState | null = null;
const seen: { busy: boolean; refusal: string | null } = { busy: false, refusal: null };
const asked: Record<string, { resolve: (v: unknown) => void; reject: (e: unknown) => void }> = {};
const bridge = {
  groupLevels: (_slot: number, column: string) => new Promise((resolve, reject) => (asked[column] = { resolve, reject })),
} as never;

const Harness: React.FC = () => {
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const run = actWith(setBusy, setRefusal, (e) => (e as Error).message);
  const s = useAnovaSettings(view, { bridge, slot: () => 3, act: run });
  state = s;
  seen.busy = busy;
  seen.refusal = refusal;
  return <AnovaSettings view={view} settings={s} />;
};

const pick = (id: string, value: string) => fireEvent.change(screen.getByTestId(id), { target: { value } });
const choose = async (alias: string) => {
  await act(async () => {
    pick('an-group', alias);
  });
};
const levels = (rows: typeof FIRST) => ({ slot: 3, column: 'C', levels: rows });

describe('a stale answer changes nothing', () => {
  it("keeps the levels of the group chosen last when an earlier group's answer comes later", async () => {
    render(<Harness />);
    await choose('C1');
    await choose('C3');
    await act(async () => asked.C3.resolve(levels(NEWER)));
    await act(async () => asked.C1.resolve(levels(FIRST)));
    expect(state?.group).toBe('C3');
    expect(state?.groupLevels).toEqual(NEWER);
    expect(seen.refusal).toBeNull();
    expect(seen.busy).toBe(false);
  });

  it("shows no refusal when an earlier group's request is refused later", async () => {
    render(<Harness />);
    await choose('C1');
    await choose('C3');
    await act(async () => asked.C3.resolve(levels(NEWER)));
    await act(async () => asked.C1.reject(new Error('too many levels')));
    expect(state?.group).toBe('C3');
    expect(state?.groupLevels).toEqual(NEWER);
    expect(seen.refusal).toBeNull();
    expect(seen.busy).toBe(false);
  });

  it('ignores an answer, or a refusal, that comes after a reset', async () => {
    render(<Harness />);
    await choose('C1');
    act(() => state?.reset());
    await act(async () => asked.C1.resolve(levels(FIRST)));
    expect(state?.groupLevels).toEqual([]);
    await choose('C3');
    act(() => state?.reset());
    await act(async () => asked.C3.reject(new Error('too many levels')));
    expect(state?.groupLevels).toEqual([]);
    expect(seen.refusal).toBeNull();
  });

  it("stays busy when a stale answer ends before the newer one, success or refusal", async () => {
    render(<Harness />);
    await choose('C1');
    await choose('C3');
    expect(seen.busy).toBe(true);
    await act(async () => asked.C1.resolve(levels(FIRST)));
    expect(seen.busy).toBe(true);
    expect(state?.groupLevels).toEqual([]);
    await act(async () => asked.C3.resolve(levels(NEWER)));
    expect(seen.busy).toBe(false);
    expect(state?.groupLevels).toEqual(NEWER);
    await choose('C1');
    await choose('C3');
    await act(async () => asked.C1.reject(new Error('too many levels')));
    expect(seen.busy).toBe(true);
    expect(seen.refusal).toBeNull();
    await act(async () => asked.C3.resolve(levels(NEWER)));
    expect(seen.busy).toBe(false);
  });

  it("still shows the refusal of the group chosen last", async () => {
    render(<Harness />);
    await choose('C1');
    await act(async () => asked.C1.reject(new Error('too many levels')));
    expect(seen.refusal).toBe('too many levels');
    expect(seen.busy).toBe(false);
  });
});

describe('the missing codes', () => {
  it('are cleared when the outcome changes to another that offers the same code', async () => {
    render(<Harness />);
    pick('an-outcome', 'C5');
    fireEvent.click(screen.getByTestId('an-code-99'));
    expect(state?.ticked).toEqual([99]);
    expect(state?.spec(1).parameters.missingCodes).toEqual([99]);
    pick('an-outcome', 'C6');
    expect(state?.ticked).toEqual([]);
    expect(state?.spec(1).parameters.missingCodes).toEqual([]);
    expect((screen.getByTestId('an-code-99') as HTMLInputElement).checked).toBe(false);
  });
});
