// Gaply — the paired t-test's settings (Research OS, Phase 8S, decisions 481 and 483;
// ADR-012 Part 6): the one copy, rendered by the old page (PairedFlow.tsx) and by the
// Statistics window's dialog (window/PairedDialog.tsx). The two columns, neither chosen
// for the user: the same one twice is Rust's to refuse (decision 271). What is held here
// is the user's choices, in this component's state only, and never browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { PairedSpec } from './onesampleTypes';
import { PAIRED_DIFFERENCE_LINE, pairedMissingCodeQuestion, type OneSampleNames } from './onesampleWording';
import { columnLabel, columnNames } from './window/windowWording';

/** The columns the test can read: numeric ones. */
const numericColumns = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** One candidate missing code of either chosen column, with each column's
 *  count (0 where the code is absent). */
interface OfferedCode {
  code: number;
  first: number;
  second: number;
}

export function buildPairedSpec(view: DatasetView, first: string, second: string, missingCodes: number[], operationId: number): PairedSpec {
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.paired_t',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { first, second, missingCodes },
    randomSeed: null,
    confirmationRefs: [],
  };
}

export function usePairedSettings(view: DatasetView | null) {
  const [first, setFirst] = useState('');
  const [second, setSecond] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);

  const firstColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === first), [view, first]);
  const secondColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === second), [view, second]);

  /** Each code of the first column, then each of the second's not already
   *  offered, once, with both columns' counts. */
  const offered = useMemo<OfferedCode[]>(() => {
    if (!firstColumn || !secondColumn) return [];
    const cellsIn = (codes: Array<{ code: number; cells: number }>, code: number) => codes.find((s) => s.code === code)?.cells ?? 0;
    const out: OfferedCode[] = [];
    for (const s of [...firstColumn.suspectMissingCodes, ...secondColumn.suspectMissingCodes]) {
      if (out.some((o) => o.code === s.code)) continue;
      out.push({ code: s.code, first: cellsIn(firstColumn.suspectMissingCodes, s.code), second: cellsIn(secondColumn.suspectMissingCodes, s.code) });
    }
    return out;
  }, [firstColumn, secondColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  const names: OneSampleNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback((operationId: number) => buildPairedSpec(view as DatasetView, first, second, missingCodes, operationId), [view, first, second, missingCodes]);

  const reset = useCallback(() => {
    setFirst('');
    setSecond('');
    setTicked([]);
  }, []);

  return {
    first,
    second,
    ticked,
    firstColumn,
    secondColumn,
    offered,
    names,
    // Check the settings needs both columns; the same one twice is Rust's to refuse (decision 271).
    ready: first !== '' && second !== '',
    spec,
    reset,
    setFirst,
    setSecond,
    toggleCode: (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

export type PairedSettingsState = ReturnType<typeof usePairedSettings>;

/** The controls, the page's and the dialog's alike. */
export const PairedSettings: React.FC<{ view: DatasetView; settings: PairedSettingsState }> = ({ view, settings: s }) => {
  const picker = (which: 'first' | 'second', label: string, value: string, set: (v: string) => void) => (
    <label>
      {label}{' '}
      <select value={value} data-testid={`pt-${which}`} aria-describedby="pt-difference-line" onChange={(e) => set(e.target.value)}>
        <option value="">Choose a column…</option>
        {numericColumns(view).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {columnLabel(c)}
          </option>
        ))}
      </select>
    </label>
  );
  return (
    <div style={{ display: 'grid', gap: 8 }}>
      {picker('first', 'First column (a numeric column)', s.first, s.setFirst)}
      {picker('second', 'Second column (a numeric column)', s.second, s.setSecond)}
      <p id="pt-difference-line" data-testid="pt-difference-line" style={{ margin: 0 }}>
        {PAIRED_DIFFERENCE_LINE}
      </p>
      {s.firstColumn && s.secondColumn && s.offered.length > 0 && (
        <fieldset data-testid="pt-missing-codes">
          <legend>Candidate missing codes</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input
                type="checkbox"
                checked={s.ticked.includes(o.code)}
                data-testid={`pt-code-${o.code}`}
                onChange={(e) => s.toggleCode(o.code, e.target.checked)}
              />{' '}
              {pairedMissingCodeQuestion(
                o.code,
                { header: columnLabel(s.firstColumn as ColumnProfile), cells: o.first },
                { header: columnLabel(s.secondColumn as ColumnProfile), cells: o.second }
              )}
            </label>
          ))}
        </fieldset>
      )}
    </div>
  );
};
