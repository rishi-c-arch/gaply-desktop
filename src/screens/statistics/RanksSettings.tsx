// Gaply — the rank tests' settings (Research OS, Phase 8Y-2, decision 619), on
// AnovaSettings.tsx's: the one copy, rendered by the Statistics window's dialog
// (window/RanksDialog.tsx); the method has no page of its own (ADR-012:233-241). None
// is made for the user, but for one that follows their choice: once the group is
// chosen, every level starts ticked (decision 329). A prepared score may be the outcome
// and is never listed as a group, and Rust's refusal stays the boundary. What is held
// here is the user's choices, in this component's state only, and never browser storage.
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { ANOVA_GROUP_LIMIT_SENTENCE, anovaMissingCodeQuestion, LEVELS_QUESTION, levelsTooFewSentence, MAX_GROUP_LEVELS } from './anovaWording';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { RanksBridge } from './ranksBridge';
import type { RanksSpec } from './ranksTypes';
import { RANKS_GROUP_LABEL, RANKS_OUTCOME_LABEL, ranksSentence, type RanksNames } from './ranksWording';
import type { GroupLevel } from './ttestTypes';
import { columnLabel, columnNames } from './window/windowWording';

const outcomes = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

const groups = (view: DatasetView) =>
  view.profile.columns.filter((c: ColumnProfile) => c.prepared !== true && c.distinct >= 2 && c.distinct <= MAX_GROUP_LEVELS);

/** Decision 619: levels null when every level is ticked, otherwise the ticked
 *  indexes, ascending; the codes ascending and once each; the view's preparation
 *  exactly when it has one (decision 490). */
export function buildRanksSpec(
  view: DatasetView,
  outcome: string,
  group: string,
  all: GroupLevel[],
  ticked: number[],
  missingCodes: number[],
  operationId: number
): RanksSpec {
  const included = all.map((l) => l.index).filter((i) => ticked.includes(i));
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.independent_ranks',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      outcome,
      group,
      levels: included.length === all.length ? null : [...included].sort((a, b) => a - b),
      missingCodes: codes,
    },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

export interface RanksSettingsOptions {
  bridge: Pick<RanksBridge, 'groupLevels'>;
  /** The open slot's number, when the group's levels are read. */
  slot: () => number | null;
  /** How the caller runs an action: its busy state, and where a refusal goes. */
  act: (f: () => Promise<void>) => Promise<void>;
}

export function useRanksSettings(view: DatasetView | null, { bridge, slot, act }: RanksSettingsOptions) {
  const [outcome, setOutcomeAlias] = useState('');
  const [group, setGroup] = useState('');
  const [groupLevels, setGroupLevels] = useState<GroupLevel[]>([]);
  const [tickedLevels, setTickedLevels] = useState<number[]>([]);
  const [ticked, setTicked] = useState<number[]>([]);
  // The newest levels request: an answer to an earlier one, success or refusal, for a
  // group since replaced or a form since reset, changes nothing (decision 635).
  const levelsAsked = useRef(0);

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: RanksNames = useMemo(
    () => ({
      column: columnNames(view),
      level: (index: number) => groupLevels.find((l) => l.index === index)?.label ?? `level ${index + 1}`,
    }),
    [view, groupLevels]
  );

  const spec = useCallback(
    (operationId: number) => buildRanksSpec(view as DatasetView, outcome, group, groupLevels, tickedLevels, missingCodes, operationId),
    [view, outcome, group, groupLevels, tickedLevels, missingCodes]
  );

  const reset = useCallback(() => {
    levelsAsked.current += 1;
    setOutcomeAlias('');
    setGroup('');
    setGroupLevels([]);
    setTickedLevels([]);
    setTicked([]);
  }, []);

  // A code is ticked for one outcome: another that offers the same code starts unticked (decision 635).
  const setOutcome = (alias: string) => {
    setOutcomeAlias(alias);
    setTicked([]);
  };

  const chooseGroup = (alias: string) =>
    act(async () => {
      levelsAsked.current += 1;
      const asked = levelsAsked.current;
      setGroup(alias);
      setGroupLevels([]);
      setTickedLevels([]);
      const at = slot();
      if (alias === '' || at === null) return;
      // The levels in the core's own order; their indexes are what the spec
      // sends. Every level starts ticked (decision 329).
      let read;
      try {
        read = await bridge.groupLevels(at, alias);
      } catch (e) {
        // A refusal for a group since replaced, or a form since reset, is not this form's.
        if (asked !== levelsAsked.current) return;
        throw e;
      }
      if (asked !== levelsAsked.current) return;
      setGroupLevels(read.levels);
      setTickedLevels(read.levels.map((l) => l.index));
    });

  return {
    outcome,
    group,
    groupLevels,
    tickedLevels,
    ticked,
    outcomeColumn,
    offered,
    names,
    sameColumn: outcome !== '' && outcome === group,
    ready: outcome !== '' && group !== '' && outcome !== group && groupLevels.length > 0 && tickedLevels.length >= 2,
    spec,
    reset,
    setOutcome,
    chooseGroup,
    toggleLevel: (index: number, on: boolean) => setTickedLevels((t) => (on ? [...t.filter((x) => x !== index), index] : t.filter((x) => x !== index))),
    toggleCode: (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

export type RanksSettingsState = ReturnType<typeof useRanksSettings>;

/** The controls. */
export const RanksSettings: React.FC<{ view: DatasetView; settings: RanksSettingsState }> = ({ view, settings: s }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    <label>
      {RANKS_OUTCOME_LABEL}{' '}
      <select value={s.outcome} data-testid="rk-outcome" onChange={(e) => s.setOutcome(e.target.value)}>
        <option value="">Choose a column…</option>
        {outcomes(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <label>
      {RANKS_GROUP_LABEL}{' '}
      <select value={s.group} data-testid="rk-group" onChange={(e) => void s.chooseGroup(e.target.value)}>
        <option value="">Choose a column…</option>
        {groups(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <p style={{ margin: 0 }} data-testid="rk-group-limit">
      <small>{ANOVA_GROUP_LIMIT_SENTENCE}</small>
    </p>
    {s.sameColumn && (
      <p role="status" data-testid="rk-same-column">
        {ranksSentence({ code: 'stats.ranks.same_column' })}
      </p>
    )}
    {s.groupLevels.length > 0 && (
      <fieldset data-testid="rk-levels">
        <legend>{LEVELS_QUESTION}</legend>
        {/* Rust's own order; each box is the index Rust gave, and the label is text only. */}
        {s.groupLevels.map((l) => (
          <label key={l.index} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.tickedLevels.includes(l.index)}
              data-testid={`rk-level-${l.index}`}
              onChange={(e) => s.toggleLevel(l.index, e.target.checked)}
            />{' '}
            {l.label} ({l.count})
          </label>
        ))}
        {s.tickedLevels.length < 2 && (
          <p role="status" data-testid="rk-levels-too-few">
            {levelsTooFewSentence(s.tickedLevels.length)}
          </p>
        )}
      </fieldset>
    )}
    {s.outcomeColumn && s.offered.length > 0 && (
      <fieldset data-testid="rk-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map((o) => (
          <label key={o.code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(o.code)}
              data-testid={`rk-code-${o.code}`}
              onChange={(e) => s.toggleCode(o.code, e.target.checked)}
            />{' '}
            {anovaMissingCodeQuestion(o.code, columnLabel(s.outcomeColumn as ColumnProfile), o.cells)}
          </label>
        ))}
      </fieldset>
    )}
  </div>
);
