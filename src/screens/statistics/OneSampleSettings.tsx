// Gaply — the one-sample t-test's settings (Research OS, Phase 8S, decisions 481 and
// 483; ADR-012 Part 6): the one copy, rendered by the old page (OneSampleFlow.tsx) and by
// the Statistics window's dialog (window/OneSampleDialog.tsx). The column and the test
// value, none chosen for the user: the value is Rust's to ask for (decision 268). What is
// held here is the user's choices, in this component's state only, and never browser
// storage.
import React, { useCallback, useMemo, useState } from 'react';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { OneSampleSpec } from './onesampleTypes';
import { oneSampleMissingCodeQuestion, TEST_VALUE_LABEL, TEST_VALUE_LINE, type OneSampleNames } from './onesampleWording';
import { columnLabel, columnNames } from './window/windowWording';

/** The columns the test can read: numeric ones. */
const numericColumns = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** The test value the spec sends: the number typed when the trimmed text is
 *  not empty and is a finite number; null otherwise. Never a default. */
export function testValueOf(text: string): number | null {
  if (text.trim() === '') return null;
  const v = Number(text);
  return Number.isFinite(v) ? v : null;
}

export function buildOneSampleSpec(view: DatasetView, outcome: string, testValue: number | null, missingCodes: number[], operationId: number): OneSampleSpec {
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.one_sample_t',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { outcome, testValue, missingCodes },
    randomSeed: null,
    confirmationRefs: [],
  };
}

export function useOneSampleSettings(view: DatasetView | null) {
  const [outcome, setOutcome] = useState('');
  /** The test value as typed; empty at first. */
  const [testValueText, setTestValueText] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  const names: OneSampleNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback(
    (operationId: number) => buildOneSampleSpec(view as DatasetView, outcome, testValueOf(testValueText), missingCodes, operationId),
    [view, outcome, testValueText, missingCodes]
  );

  const reset = useCallback(() => {
    setOutcome('');
    setTestValueText('');
    setTicked([]);
  }, []);

  return {
    outcome,
    testValueText,
    ticked,
    outcomeColumn,
    offered,
    names,
    // Check the settings needs the column only; the test value is Rust's to ask for (decision 268).
    ready: outcome !== '',
    spec,
    reset,
    setOutcome,
    setTestValueText,
    toggleCode: (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

export type OneSampleSettingsState = ReturnType<typeof useOneSampleSettings>;

/** The controls, the page's and the dialog's alike. */
export const OneSampleSettings: React.FC<{ view: DatasetView; settings: OneSampleSettingsState }> = ({ view, settings: s }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    <label>
      Column (a numeric column){' '}
      <select value={s.outcome} data-testid="os-outcome" onChange={(e) => s.setOutcome(e.target.value)}>
        <option value="">Choose a column…</option>
        {numericColumns(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
    <label>
      {TEST_VALUE_LABEL}{' '}
      <input
        type="text"
        inputMode="decimal"
        value={s.testValueText}
        data-testid="os-test-value"
        aria-describedby="os-test-value-line"
        onChange={(e) => s.setTestValueText(e.target.value)}
      />
    </label>
    <p id="os-test-value-line" data-testid="os-test-value-line" style={{ margin: 0 }}>
      {TEST_VALUE_LINE}
    </p>
    {s.outcomeColumn && s.offered.length > 0 && (
      <fieldset data-testid="os-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map((o) => (
          <label key={o.code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(o.code)}
              data-testid={`os-code-${o.code}`}
              onChange={(e) => s.toggleCode(o.code, e.target.checked)}
            />{' '}
            {oneSampleMissingCodeQuestion(o.code, columnLabel(s.outcomeColumn as ColumnProfile), o.cells)}
          </label>
        ))}
      </fieldset>
    )}
  </div>
);
