// Gaply — crosstabs' settings (Research OS, Phase 8X-2, decision 588), on
// AnovaSettings.tsx's: the one copy, rendered by the Statistics window's dialog
// (window/XtabDialog.tsx); crosstabs has no page of its own (ADR-012:233-241). None is
// made for the user. Two pickers, rows and columns, each offering the columns that are
// not scale scores and have 2 to MAX_GROUP_LEVELS distinct values in the profile, as
// the ANOVA's group (AnovaSettings.tsx:27-28). Rust counts a column's levels among the
// included rows (decision 575), so a column of 51 distinct values one of which is a
// code would be taken there; it is not offered here (recorded, not built). The
// candidate missing codes are the union of both columns' suspect codes, ascending and
// once each, as the correlations' are (CorrSettings.tsx:63-75); a ticked code applies
// to both columns. Rust's refusal stays the boundary. What is held here is the user's
// choices, in this component's state only, and never browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import type { ColumnProfile, DatasetView } from './describeTypes';
import type { XtabSpec } from './xtabTypes';
import { MAX_GROUP_LEVELS } from './anovaWording';
import {
  XTAB_COLUMN_LIMIT_SENTENCE,
  XTAB_COLUMNS_LABEL,
  XTAB_ROWS_LABEL,
  xtabMissingCodeQuestion,
  xtabSentence,
  type XtabNames,
} from './xtabWording';
import { columnLabel, columnNames } from './window/windowWording';

/** The columns either picker offers: a file's own, with 2 to MAX_GROUP_LEVELS
 *  distinct values. */
export const xtabColumns = (view: DatasetView): ColumnProfile[] =>
  view.profile.columns.filter((c) => c.prepared !== true && c.distinct >= 2 && c.distinct <= MAX_GROUP_LEVELS);

/** The spec: rows and columns by alias, the codes ascending and once each, the
 *  view's preparation exactly when it has one (decision 490). */
export function buildXtabSpec(view: DatasetView, rows: string, columns: string, missingCodes: number[], operationId: number): XtabSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.crosstabs',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { rows, columns, missingCodes: codes },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

export function useXtabSettings(view: DatasetView | null) {
  const [rows, setRows] = useState('');
  const [columns, setColumns] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);

  /** The candidate codes of both chosen columns, ascending, with where each
   *  appears. */
  const offered = useMemo(() => {
    const where = new Map<number, Array<{ header: string; cells: number }>>();
    const chosen = [rows, columns].filter((a, i, all) => a !== '' && all.indexOf(a) === i);
    for (const alias of chosen) {
      const c = view?.profile.columns.find((x) => x.alias === alias);
      if (!c) continue;
      for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
    }
    return Array.from(where.keys())
      .sort((a, b) => a - b)
      .map((code) => ({ code, where: where.get(code) ?? [] }));
  }, [view, rows, columns]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  const names: XtabNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback(
    (operationId: number) => buildXtabSpec(view as DatasetView, rows, columns, missingCodes, operationId),
    [view, rows, columns, missingCodes]
  );

  const reset = useCallback(() => {
    setRows('');
    setColumns('');
    setTicked([]);
  }, []);

  return {
    rows,
    columns,
    ticked,
    offered,
    names,
    ready: rows !== '' && columns !== '' && rows !== columns,
    spec,
    reset,
    setRows,
    setColumns,
    toggleCode: (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

export type XtabSettingsState = ReturnType<typeof useXtabSettings>;

const Picker: React.FC<{ label: string; value: string; testId: string; view: DatasetView; onChange: (alias: string) => void }> = ({ label, value, testId, view, onChange }) => (
  <label>
    {label}{' '}
    <select value={value} data-testid={testId} onChange={(e) => onChange(e.target.value)}>
      <option value="">Choose a column…</option>
      {xtabColumns(view).map((c) => (
        <option key={c.alias} value={c.alias}>
          {c.alias} {columnLabel(c)}
        </option>
      ))}
    </select>
  </label>
);

/** The controls. */
export const XtabSettings: React.FC<{ view: DatasetView; settings: XtabSettingsState }> = ({ view, settings: s }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    <Picker label={XTAB_ROWS_LABEL} value={s.rows} testId="xt-rows" view={view} onChange={s.setRows} />
    <Picker label={XTAB_COLUMNS_LABEL} value={s.columns} testId="xt-columns" view={view} onChange={s.setColumns} />
    <p style={{ margin: 0 }} data-testid="xt-column-limit">
      <small>{XTAB_COLUMN_LIMIT_SENTENCE}</small>
    </p>
    {s.rows !== '' && s.rows === s.columns && (
      <p role="status" data-testid="xt-same-column">
        {xtabSentence({ code: 'stats.xtab.same_column' })}
      </p>
    )}
    {s.offered.length > 0 && (
      <fieldset data-testid="xt-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map((o) => (
          <label key={o.code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(o.code)}
              data-testid={`xt-code-${o.code}`}
              onChange={(e) => s.toggleCode(o.code, e.target.checked)}
            />{' '}
            {xtabMissingCodeQuestion(o.code, o.where)}
          </label>
        ))}
      </fieldset>
    )}
  </div>
);
