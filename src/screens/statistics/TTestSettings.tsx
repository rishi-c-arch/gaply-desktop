// Gaply — the independent-samples t-test's settings (Research OS, Phase 8S, decisions
// 481 and 483; ADR-012 Part 6): the one copy, rendered by the old page (TTestFlow.tsx)
// and by the Statistics window's dialog (window/TTestDialog.tsx). None is made for the
// user, but for one that is no choice: a group of exactly two levels selects them,
// since the user's choice was the column. A prepared score is never listed as a
// group, and Rust's refusal stays the boundary. What is held here is the user's
// choices, in this component's state only, and never browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { GroupLevel, TTestSpec, Variant } from './ttestTypes';
import type { TTestBridge } from './ttestBridge';
import {
  GROUP_LIMIT_SENTENCE,
  levelsQuestion,
  MAX_GROUP_LEVELS,
  ttestMissingCodeQuestion,
  VARIANT_CHOICES,
  VARIANT_QUESTION,
  type TTestNames,
} from './ttestWording';
import { columnLabel, columnNames } from './window/windowWording';

/** The columns that can be an outcome: numeric ones. */
const outcomes = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** The columns that can be a group: any with 2 to MAX_GROUP_LEVELS levels
 *  when read as categorical, numeric ones included (decisions 81, 87), and never a
 *  prepared score (decision 483). The profile's `distinct` counts them for every
 *  kind; the levels themselves come from stats_ttest_group_levels, which refuses a
 *  column with more, and a score. */
const groups = (view: DatasetView) =>
  view.profile.columns.filter((c: ColumnProfile) => c.prepared !== true && c.distinct >= 2 && c.distinct <= MAX_GROUP_LEVELS);

/** The two chosen levels, by position in the group-levels list. */
type LevelPicks = [number | null, number | null];

function defaultLevels(levels: GroupLevel[]): LevelPicks {
  return levels.length === 2 ? [0, 1] : [null, null];
}

export function buildTTestSpec(
  view: DatasetView,
  outcome: string,
  group: string,
  levels: LevelPicks,
  variant: Variant | null,
  missingCodes: number[],
  operationId: number
): TTestSpec {
  const [a, b] = levels;
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.independent_t',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      outcome,
      group,
      // Positions, never labels (decision 78). Both or neither.
      levels: a !== null && b !== null ? [a, b] : null,
      variant,
      missingCodes,
    },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

export interface TTestSettingsOptions {
  bridge: Pick<TTestBridge, 'groupLevels'>;
  /** The open slot's number, when the group's levels are read. */
  slot: () => number | null;
  /** How the caller runs an action: its busy state, and where a refusal goes. */
  act: (f: () => Promise<void>) => Promise<void>;
}

export function useTTestSettings(view: DatasetView | null, { bridge, slot, act }: TTestSettingsOptions) {
  const [outcome, setOutcome] = useState('');
  const [group, setGroup] = useState('');
  /** The chosen group's levels, as Rust indexes them. */
  const [groupLevels, setGroupLevels] = useState<GroupLevel[]>([]);
  const [levels, setLevels] = useState<LevelPicks>([null, null]);
  const [variant, setVariant] = useState<Variant | null>(null);
  const [ticked, setTicked] = useState<number[]>([]);

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: TTestNames = useMemo(
    () => ({
      column: columnNames(view),
      level: (index: number) => groupLevels.find((l) => l.index === index)?.label ?? `level ${index + 1}`,
    }),
    [view, groupLevels]
  );

  const spec = useCallback(
    (operationId: number) => buildTTestSpec(view as DatasetView, outcome, group, levels, variant, missingCodes, operationId),
    [view, outcome, group, levels, variant, missingCodes]
  );

  const reset = useCallback(() => {
    setOutcome('');
    setGroup('');
    setGroupLevels([]);
    setLevels([null, null]);
    setVariant(null);
    setTicked([]);
  }, []);

  const chooseGroup = (alias: string) =>
    act(async () => {
      setGroup(alias);
      setGroupLevels([]);
      setLevels([null, null]);
      const at = slot();
      if (alias === '' || at === null) return;
      // The levels in the core's own order (decision 81); positions in this
      // list are what the spec sends.
      const read = await bridge.groupLevels(at, alias);
      setGroupLevels(read.levels);
      setLevels(defaultLevels(read.levels));
    });
  const chooseLevel = (which: 0 | 1, value: string) =>
    setLevels((l) => {
      const next: LevelPicks = [l[0], l[1]];
      next[which] = value === '' ? null : Number(value);
      return next;
    });
  const toggleCode = (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code)));

  return {
    outcome,
    group,
    groupLevels,
    levels,
    variant,
    ticked,
    outcomeColumn,
    offered,
    names,
    ready: outcome !== '' && group !== '',
    spec,
    reset,
    setOutcome,
    setVariant,
    chooseGroup,
    chooseLevel,
    toggleCode,
  };
}

export type TTestSettingsState = ReturnType<typeof useTTestSettings>;

/** The controls, the page's and the dialog's alike. */
export const TTestSettings: React.FC<{ view: DatasetView; settings: TTestSettingsState }> = ({ view, settings: s }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    <label>
      Outcome (a numeric column){' '}
      <select value={s.outcome} data-testid="tt-outcome" onChange={(e) => s.setOutcome(e.target.value)}>
        <option value="">Choose a column…</option>
        {outcomes(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <label>
      Group (a column with at least two levels){' '}
      <select value={s.group} data-testid="tt-group" onChange={(e) => void s.chooseGroup(e.target.value)}>
        <option value="">Choose a column…</option>
        {groups(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <p style={{ margin: 0 }} data-testid="tt-group-limit">
      <small>{GROUP_LIMIT_SENTENCE}</small>
    </p>
    {s.groupLevels.length > 0 && (
      <fieldset data-testid="tt-levels">
        <legend>The two groups</legend>
        {s.groupLevels.length > 2 && <p data-testid="tt-levels-question">{levelsQuestion(s.groupLevels.length)}</p>}
        {([0, 1] as const).map((which) => (
          <label key={which} style={{ display: 'block' }}>
            {which === 0 ? 'Group 1' : 'Group 2'}{' '}
            <select
              value={s.levels[which] === null ? '' : String(s.levels[which])}
              data-testid={`tt-level-${which + 1}`}
              onChange={(e) => s.chooseLevel(which, e.target.value)}
            >
              <option value="">Choose a level…</option>
              {/* Rust's own order; the value is the index Rust gave. */}
              {s.groupLevels.map((l) => (
                <option key={l.index} value={String(l.index)}>
                  {l.label} ({l.count})
                </option>
              ))}
            </select>
          </label>
        ))}
        <p style={{ margin: 0 }}>The mean difference is group 1 minus group 2.</p>
      </fieldset>
    )}
    <fieldset data-testid="tt-variant">
      <legend>{VARIANT_QUESTION}</legend>
      {VARIANT_CHOICES.map((c) => (
        <div key={c.variant}>
          <label style={{ display: 'block' }}>
            <input
              type="radio"
              name="tt-variant"
              value={c.variant}
              checked={s.variant === c.variant}
              data-testid={`tt-variant-${c.variant}`}
              aria-describedby={`tt-variant-line-${c.variant}`}
              onChange={() => s.setVariant(c.variant)}
            />{' '}
            {c.label}
          </label>
          {/* Decision 93: what this test assumes, as information. */}
          <p id={`tt-variant-line-${c.variant}`} data-testid={`tt-variant-line-${c.variant}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
            {c.line}
          </p>
        </div>
      ))}
    </fieldset>
    {s.outcomeColumn && s.offered.length > 0 && (
      <fieldset data-testid="tt-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map((o) => (
          <label key={o.code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(o.code)}
              data-testid={`tt-code-${o.code}`}
              onChange={(e) => s.toggleCode(o.code, e.target.checked)}
            />{' '}
            {ttestMissingCodeQuestion(o.code, columnLabel(s.outcomeColumn as ColumnProfile), o.cells)}
          </label>
        ))}
      </fieldset>
    )}
  </div>
);
