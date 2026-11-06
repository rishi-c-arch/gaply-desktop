// Gaply — the descriptive summary's settings (Research OS, Phase 8S, decisions 481
// and 482; ADR-012 Part 6): the one copy, rendered by the old page (DescribeFlow.tsx)
// and by the Statistics window's dialog (window/DescribeDialog.tsx). Its starting
// picks are its caller's: the page includes every column, as it always has; the
// dialog includes none, so nothing is chosen for the user. A column the user includes
// starts treated as its kind says, which they may change: that follows the file.
//
// What is held here is the user's choices, in this component's state only, and never
// browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import type { DatasetView, OperationSpec, TreatAs } from './describeTypes';
import { KIND_NAMES, missingCodeQuestion } from './describeWording';
import { columnLabel } from './window/windowWording';

export interface DescribePick {
  include: boolean;
  treatAs: TreatAs;
}

export type DescribePicks = Record<string, DescribePick>;

/** Every column, as the page has always started. */
export function defaultPicks(view: DatasetView): DescribePicks {
  const picks: DescribePicks = {};
  for (const c of view.profile.columns) {
    picks[c.alias] = { include: true, treatAs: c.kind === 'categorical' ? 'categorical' : 'numeric' };
  }
  return picks;
}

/** No column, each treated as its kind says once the user includes it. */
export function noPicks(view: DatasetView): DescribePicks {
  const picks = defaultPicks(view);
  for (const alias of Object.keys(picks)) picks[alias] = { ...picks[alias], include: false };
  return picks;
}

/** The candidate missing codes of the columns chosen as numeric, each with
 *  the columns that hold it. */
function offeredCodes(view: DatasetView, picks: DescribePicks): Array<[number, Array<{ header: string; cells: number }>]> {
  const byCode = new Map<number, Array<{ header: string; cells: number }>>();
  for (const c of view.profile.columns) {
    const p = picks[c.alias];
    if (!p?.include || p.treatAs !== 'numeric') continue;
    for (const s of c.suspectMissingCodes) {
      byCode.set(s.code, [...(byCode.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
    }
  }
  return Array.from(byCode.entries()).sort((a, b) => a[0] - b[0]);
}

export function buildDescribeSpec(view: DatasetView, picks: DescribePicks, missingCodes: number[], operationId: number): OperationSpec {
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.describe',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      columns: view.profile.columns.filter((c) => picks[c.alias]?.include).map((c) => ({ alias: c.alias, treatAs: picks[c.alias].treatAs })),
      missingCodes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

export interface DescribeSettingsState {
  picks: DescribePicks;
  ticked: number[];
  offered: ReturnType<typeof offeredCodes>;
  /** Whether a column is included: the dialog's readiness. */
  anyIncluded: boolean;
  spec: (operationId: number) => OperationSpec;
  /** A new file: the caller's starting picks, and no code. */
  reset: (view: DatasetView) => void;
  setPick: (alias: string, pick: DescribePick) => void;
  toggleCode: (code: number, on: boolean) => void;
}

export function useDescribeSettings(view: DatasetView | null, start: (view: DatasetView) => DescribePicks): DescribeSettingsState {
  const [picks, setPicks] = useState<DescribePicks>(() => (view ? start(view) : {}));
  const [ticked, setTicked] = useState<number[]>([]);
  const offered = useMemo(() => (view ? offeredCodes(view, picks) : []), [view, picks]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some(([code]) => code === t)), [ticked, offered]);
  const spec = useCallback((operationId: number) => buildDescribeSpec(view as DatasetView, picks, missingCodes, operationId), [view, picks, missingCodes]);
  return {
    picks,
    ticked,
    offered,
    anyIncluded: Object.values(picks).some((p) => p.include),
    spec,
    reset: (opened) => {
      setPicks(start(opened));
      setTicked([]);
    },
    setPick: (alias, pick) => setPicks((now) => ({ ...now, [alias]: pick })),
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

/** The controls, the page's and the dialog's alike. */
export const DescribeSettings: React.FC<{ view: DatasetView; settings: DescribeSettingsState }> = ({ view, settings: s }) => (
  <>
    <table data-testid="desc-settings" style={{ borderCollapse: 'collapse' }}>
      <thead>
        <tr>
          <th scope="col" style={{ textAlign: 'left', ...CELL_PADDING }}>
            Include
          </th>
          <th scope="col" style={{ textAlign: 'left', ...CELL_PADDING }}>
            Column
          </th>
          <th scope="col" style={{ textAlign: 'left', ...CELL_PADDING }}>
            Summarise as
          </th>
        </tr>
      </thead>
      <tbody>
        {view.profile.columns.map((c) => {
          const p = s.picks[c.alias];
          const name = columnLabel(c);
          return (
            <tr key={c.alias}>
              <td style={CELL_PADDING}>
                <input
                  type="checkbox"
                  checked={p?.include ?? false}
                  aria-label={`Include ${name}`}
                  data-testid={`desc-include-${c.alias}`}
                  onChange={(e) => s.setPick(c.alias, { ...p, include: e.target.checked })}
                />
              </td>
              <td style={CELL_PADDING}>
                <span className="gds-mono">{c.alias}</span> {name} <small>({KIND_NAMES[c.kind]})</small>
              </td>
              <td style={CELL_PADDING}>
                <select
                  value={p?.treatAs}
                  aria-label={`Summarise ${name} as`}
                  data-testid={`desc-kind-${c.alias}`}
                  disabled={!p?.include}
                  onChange={(e) => s.setPick(c.alias, { ...p, treatAs: e.target.value as TreatAs })}
                >
                  <option value="numeric">Numeric</option>
                  <option value="categorical">Categorical</option>
                </select>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
    {s.offered.length > 0 && (
      <fieldset data-testid="desc-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map(([code, where]) => (
          <label key={code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(code)}
              data-testid={`desc-code-${code}`}
              onChange={(e) => s.toggleCode(code, e.target.checked)}
            />{' '}
            {missingCodeQuestion(code, where)}
          </label>
        ))}
      </fieldset>
    )}
  </>
);
