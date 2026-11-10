// Gaply — the related rank tests' settings (Research OS, Phase 8Z-2, decision 653), on
// AlphaSettings.tsx's two panes. The order the user chooses is kept and shown: it fixes the
// Wilcoxon differences, the first less the second. The numeric columns, scores included, on
// the left in the file's order; the chosen columns on the right in the list's order, each
// numbered, with ↑, ↓ and Remove. None is chosen for the user. Each candidate missing code
// of the chosen columns is asked as a question; a tick is dropped with the last column
// that offered it, so re-adding the column shows it unticked.
//
// The spec keeps the columns in the list's order, never sorted, and the codes ascending and
// once each. What is held here is the user's choices, in this component's state only.
import React, { useCallback, useMemo, useState } from 'react';
import { Button } from '../../design-system';
import type { DatasetView } from './describeTypes';
import type { RelatedRanksSpec } from './relatedRanksTypes';
import {
  MAX_RELATED_COLUMNS,
  RELATED_RANKS_CODES_LEGEND,
  RELATED_RANKS_LEGEND,
  RELATED_RANKS_LIST_HEADING,
  RELATED_RANKS_TOO_FEW,
  relatedRanksDifferenceSentence,
  relatedRanksMissingCodeQuestion,
  relatedRanksMoveDown,
  relatedRanksMoveUp,
  relatedRanksTooManyColumns,
  type RelatedRanksNames,
} from './relatedRanksWording';
import { ALPHA_ADD_ALL, ALPHA_COLUMNS_HEADING, ALPHA_ITEMS_NONE, ALPHA_REMOVE_ALL, ALPHA_REMOVE_ITEM, columnLabel, columnNames } from './window/windowWording';

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** Decision 653: the columns in the list's order, never sorted, the codes ascending and
 *  once each, and the view's preparation exactly when it has one. */
export function buildRelatedRanksSpec(view: DatasetView, columns: string[], missingCodes: number[], operationId: number): RelatedRanksSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.related_ranks',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { columns, missingCodes: codes },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

/** The candidate codes of `columns`, ascending, with where each appears. */
function offeredBy(view: DatasetView | null, columns: readonly string[]) {
  const where = new Map<number, Array<{ header: string; cells: number }>>();
  for (const c of view?.profile.columns ?? []) {
    if (!columns.includes(c.alias)) continue;
    for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
  }
  return Array.from(where.keys())
    .sort((a, b) => a - b)
    .map((code) => ({ code, where: where.get(code) ?? [] }));
}

export interface RelatedRanksSettingsState {
  /** The chosen aliases, in the list's order. */
  columns: string[];
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  names: RelatedRanksNames;
  /** What is still missing, in the method's words; null once it can be checked. */
  stillMissing: string | null;
  /** The spec these choices give. */
  spec: (operationId: number) => RelatedRanksSpec;
  reset: () => void;
  toggleColumn: (alias: string, on: boolean) => void;
  moveColumn: (alias: string, by: -1 | 1) => void;
  addAll: () => void;
  removeAll: () => void;
  toggleCode: (code: number, on: boolean) => void;
}

/** The user's choices, over the open file's view. */
export function useRelatedRanksSettings(view: DatasetView | null): RelatedRanksSettingsState {
  const [columns, setColumns] = useState<string[]>([]);
  const [ticked, setTicked] = useState<number[]>([]);

  const offered = useMemo(() => offeredBy(view, columns), [view, columns]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  const names: RelatedRanksNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback((operationId: number) => buildRelatedRanksSpec(view as DatasetView, columns, missingCodes, operationId), [view, columns, missingCodes]);

  const reset = useCallback(() => {
    setColumns([]);
    setTicked([]);
  }, []);

  /** The list after a change: a tick goes with the last chosen column that offered it. */
  const listIs = (next: string[]) => {
    setColumns(next);
    const still = offeredBy(view, next);
    setTicked((t) => t.filter((c) => still.some((s) => s.code === c)));
  };

  const stillMissing = columns.length < 2 ? RELATED_RANKS_TOO_FEW : columns.length > MAX_RELATED_COLUMNS ? relatedRanksTooManyColumns(columns.length) : null;

  return {
    columns,
    ticked,
    offered,
    names,
    stillMissing,
    spec,
    reset,
    toggleColumn: (alias, on) => listIs(on ? [...columns.filter((c) => c !== alias), alias] : columns.filter((c) => c !== alias)),
    moveColumn: (alias, by) => {
      const at = columns.indexOf(alias);
      const to = at + by;
      if (at < 0 || to < 0 || to >= columns.length) return;
      const next = [...columns];
      next[at] = next[to];
      next[to] = alias;
      setColumns(next);
    },
    addAll: () => listIs([...columns, ...numeric(view as DatasetView).map((c) => c.alias).filter((a) => !columns.includes(a))]),
    removeAll: () => listIs([]),
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

/** The controls: the dialog's. */
export const RelatedRanksSettings: React.FC<{ view: DatasetView; settings: RelatedRanksSettingsState }> = ({ view, settings: s }) => {
  const columns = numeric(view);
  const chosen = s.columns.map((alias) => columns.find((c) => c.alias === alias)).filter((c): c is (typeof columns)[number] => c !== undefined);
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="rr-settings">
      <fieldset data-testid="rr-column-choices">
        <legend>{RELATED_RANKS_LEGEND}</legend>
        <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
          <Button variant="secondary" onClick={s.addAll} data-testid="rr-add-all">
            {ALPHA_ADD_ALL}
          </Button>
          <Button variant="secondary" onClick={s.removeAll} data-testid="rr-remove-all">
            {ALPHA_REMOVE_ALL}
          </Button>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
          <div data-testid="rr-columns">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_COLUMNS_HEADING}</h4>
            {columns.map((c) => (
              <label key={c.alias} style={{ display: 'block' }}>
                <input type="checkbox" checked={s.columns.includes(c.alias)} data-testid={`rr-column-${c.alias}`} onChange={(e) => s.toggleColumn(c.alias, e.target.checked)} /> {c.alias}{' '}
                <bdi>{columnLabel(c)}</bdi>
              </label>
            ))}
          </div>
          <div data-testid="rr-list">
            <h4 style={{ margin: '4px 0' }}>{RELATED_RANKS_LIST_HEADING}</h4>
            {chosen.length === 0 && (
              <p style={{ margin: 0 }} data-testid="rr-none">
                {ALPHA_ITEMS_NONE}
              </p>
            )}
            {chosen.map((c, i) => (
              <div key={c.alias} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }} data-testid={`rr-row-${c.alias}`}>
                <span data-testid="rr-row-name">
                  {i + 1}. {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>
                <Button variant="ghost" aria-label={relatedRanksMoveUp(c.alias)} disabled={i === 0} onClick={() => s.moveColumn(c.alias, -1)} data-testid={`rr-up-${c.alias}`}>
                  ↑
                </Button>
                <Button variant="ghost" aria-label={relatedRanksMoveDown(c.alias)} disabled={i === chosen.length - 1} onClick={() => s.moveColumn(c.alias, 1)} data-testid={`rr-down-${c.alias}`}>
                  ↓
                </Button>
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} ${c.alias}`} onClick={() => s.toggleColumn(c.alias, false)} data-testid={`rr-remove-${c.alias}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
              </div>
            ))}
          </div>
        </div>
      </fieldset>
      {chosen.length === 2 && (
        <p style={{ margin: 0 }} data-testid="rr-difference-line">
          {relatedRanksDifferenceSentence(columnLabel(chosen[0]), columnLabel(chosen[1]))}
        </p>
      )}
      {s.offered.length > 0 && (
        <fieldset data-testid="rr-missing-codes">
          <legend>{RELATED_RANKS_CODES_LEGEND}</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input type="checkbox" checked={s.ticked.includes(o.code)} data-testid={`rr-code-${o.code}`} onChange={(e) => s.toggleCode(o.code, e.target.checked)} />{' '}
              {relatedRanksMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
      {s.stillMissing && (
        <p role="status" data-testid="rr-settings-missing">
          {s.stillMissing}
        </p>
      )}
    </div>
  );
};
