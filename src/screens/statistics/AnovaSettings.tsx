// Gaply — the one-way ANOVA's settings (Research OS, Phase 8S, decisions 481 and 484;
// ADR-012 Part 6): the one copy, rendered by the old page (AnovaFlow.tsx) and by the
// Statistics window's dialog (window/AnovaDialog.tsx). None is made for the user, but
// for one that follows their choice: once the group is chosen, every level starts
// ticked (decision 329). A prepared score is never listed as a group, and Rust's refusal
// stays the boundary. What is held here is the user's choices, in this component's state
// only, and never browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import type { AnovaBridge } from './anovaBridge';
import { POST_HOC_TESTS, type AnovaSpec, type PostHoc } from './anovaTypes';
import {
  ANOVA_GROUP_LIMIT_SENTENCE,
  anovaMissingCodeQuestion,
  LEVELS_QUESTION,
  levelsTooFewSentence,
  MAX_GROUP_LEVELS,
  POST_HOC_CHOICES,
  POST_HOC_QUESTION,
  type AnovaNames,
} from './anovaWording';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { GroupLevel } from './ttestTypes';
import { columnLabel, columnNames } from './window/windowWording';

const outcomes = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

const groups = (view: DatasetView) =>
  view.profile.columns.filter((c: ColumnProfile) => c.prepared !== true && c.distinct >= 2 && c.distinct <= MAX_GROUP_LEVELS);

/** Decision 329: null when every level is ticked; otherwise the ticked
 *  indexes, ascending. The post hoc tests in the method's order. */
export function buildAnovaSpec(
  view: DatasetView,
  outcome: string,
  group: string,
  all: GroupLevel[],
  ticked: number[],
  postHoc: PostHoc[],
  missingCodes: number[],
  operationId: number
): AnovaSpec {
  const included = all.map((l) => l.index).filter((i) => ticked.includes(i));
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.one_way_anova',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      outcome,
      group,
      levels: included.length === all.length ? null : [...included].sort((a, b) => a - b),
      postHoc: POST_HOC_TESTS.filter((t) => postHoc.includes(t)),
      missingCodes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

export interface AnovaSettingsOptions {
  bridge: Pick<AnovaBridge, 'groupLevels'>;
  /** The open slot's number, when the group's levels are read. */
  slot: () => number | null;
  /** How the caller runs an action: its busy state, and where a refusal goes. */
  act: (f: () => Promise<void>) => Promise<void>;
}

export function useAnovaSettings(view: DatasetView | null, { bridge, slot, act }: AnovaSettingsOptions) {
  const [outcome, setOutcome] = useState('');
  const [group, setGroup] = useState('');
  const [groupLevels, setGroupLevels] = useState<GroupLevel[]>([]);
  const [tickedLevels, setTickedLevels] = useState<number[]>([]);
  const [postHoc, setPostHoc] = useState<PostHoc[]>([]);
  const [ticked, setTicked] = useState<number[]>([]);

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: AnovaNames = useMemo(
    () => ({
      column: columnNames(view),
      level: (index: number) => groupLevels.find((l) => l.index === index)?.label ?? `level ${index + 1}`,
    }),
    [view, groupLevels]
  );

  const spec = useCallback(
    (operationId: number) => buildAnovaSpec(view as DatasetView, outcome, group, groupLevels, tickedLevels, postHoc, missingCodes, operationId),
    [view, outcome, group, groupLevels, tickedLevels, postHoc, missingCodes]
  );

  const reset = useCallback(() => {
    setOutcome('');
    setGroup('');
    setGroupLevels([]);
    setTickedLevels([]);
    setPostHoc([]);
    setTicked([]);
  }, []);

  const chooseGroup = (alias: string) =>
    act(async () => {
      setGroup(alias);
      setGroupLevels([]);
      setTickedLevels([]);
      const at = slot();
      if (alias === '' || at === null) return;
      // The levels in the core's own order; their indexes are what the spec
      // sends. Every level starts ticked (decision 329).
      const read = await bridge.groupLevels(at, alias);
      setGroupLevels(read.levels);
      setTickedLevels(read.levels.map((l) => l.index));
    });

  return {
    outcome,
    group,
    groupLevels,
    tickedLevels,
    postHoc,
    ticked,
    outcomeColumn,
    offered,
    names,
    ready: outcome !== '' && group !== '' && groupLevels.length > 0 && tickedLevels.length >= 2,
    spec,
    reset,
    setOutcome,
    chooseGroup,
    toggleLevel: (index: number, on: boolean) => setTickedLevels((t) => (on ? [...t.filter((x) => x !== index), index] : t.filter((x) => x !== index))),
    togglePostHoc: (test: PostHoc, on: boolean) => setPostHoc((t) => (on ? [...t.filter((x) => x !== test), test] : t.filter((x) => x !== test))),
    toggleCode: (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

export type AnovaSettingsState = ReturnType<typeof useAnovaSettings>;

/** The controls, the page's and the dialog's alike. */
export const AnovaSettings: React.FC<{ view: DatasetView; settings: AnovaSettingsState }> = ({ view, settings: s }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    <label>
      Outcome (a numeric column){' '}
      <select value={s.outcome} data-testid="an-outcome" onChange={(e) => s.setOutcome(e.target.value)}>
        <option value="">Choose a column…</option>
        {outcomes(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <label>
      Group (a column with 2 to {MAX_GROUP_LEVELS} levels){' '}
      <select value={s.group} data-testid="an-group" onChange={(e) => void s.chooseGroup(e.target.value)}>
        <option value="">Choose a column…</option>
        {groups(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <p style={{ margin: 0 }} data-testid="an-group-limit">
      <small>{ANOVA_GROUP_LIMIT_SENTENCE}</small>
    </p>
    {s.groupLevels.length > 0 && (
      <fieldset data-testid="an-levels">
        <legend>{LEVELS_QUESTION}</legend>
        {/* Rust's own order; each box is the index Rust gave. */}
        {s.groupLevels.map((l) => (
          <label key={l.index} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.tickedLevels.includes(l.index)}
              data-testid={`an-level-${l.index}`}
              onChange={(e) => s.toggleLevel(l.index, e.target.checked)}
            />{' '}
            {l.label} ({l.count})
          </label>
        ))}
        {s.tickedLevels.length < 2 && (
          <p role="status" data-testid="an-levels-too-few">
            {levelsTooFewSentence(s.tickedLevels.length)}
          </p>
        )}
      </fieldset>
    )}
    <fieldset data-testid="an-post-hoc">
      <legend>{POST_HOC_QUESTION}</legend>
      {POST_HOC_CHOICES.map((c) => (
        <div key={c.test}>
          <label style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.postHoc.includes(c.test)}
              data-testid={`an-post-hoc-${c.test}`}
              aria-describedby={`an-post-hoc-line-${c.test}`}
              onChange={(e) => s.togglePostHoc(c.test, e.target.checked)}
            />{' '}
            {c.label}
          </label>
          <p id={`an-post-hoc-line-${c.test}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
            {c.line}
          </p>
        </div>
      ))}
    </fieldset>
    {s.outcomeColumn && s.offered.length > 0 && (
      <fieldset data-testid="an-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map((o) => (
          <label key={o.code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(o.code)}
              data-testid={`an-code-${o.code}`}
              onChange={(e) => s.toggleCode(o.code, e.target.checked)}
            />{' '}
            {anovaMissingCodeQuestion(o.code, columnLabel(s.outcomeColumn as ColumnProfile), o.cells)}
          </label>
        ))}
      </fieldset>
    )}
  </div>
);
