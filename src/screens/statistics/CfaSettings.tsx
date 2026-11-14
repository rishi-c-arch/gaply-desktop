// Gaply — the confirmatory factor analysis's settings, its model editor (Research OS, Phase
// 9C-3, decisions 782 and 783). On the left, the view's numeric columns, scores included, in
// the file's order, each with a select: "Not in the model" (chosen at the start), "Factor 1" to
// "Factor k" for the k factors drawn, and "A new factor" while k is below the most factors. On
// the right, the factors in their numbered order, each listing its columns in the file's order,
// its first column marked as the one whose loading is fixed at 1. Then whether the factors are
// correlated (ticked at the start; disabled, its value kept, with fewer than two factors), and
// each candidate missing code of the model's columns as a question.
//
// The factors are held by ids that do not change as the user edits, and shown numbered by their
// first column in the file's order: when a factor's first column leaves it, its number may
// change, and the right pane and every select show the new numbers at once. A factor whose last
// column leaves it is gone, and a view change that removes a column takes it out of its factor,
// as the codes' prune does (decision 698). Nothing is chosen for the user.
//
// The spec lists the factors in the order of their first columns, each its columns in the file's
// order, so Rust's order rules (items_out_of_order, factors_out_of_order) always hold; the codes
// ascending and once each. The rule that an uncorrelated factor of two columns is not identified
// stays Rust's: validation says so after Run. What is held here is the user's choices, in this
// component's state only.
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { DatasetView } from './describeTypes';
import type { CfaSpec } from './cfaTypes';
import {
  CF_AT_CAP,
  CF_CODES_LEGEND,
  CF_COLUMNS_HEADING,
  CF_CORRELATED_LABEL,
  CF_CORRELATED_LINE,
  CF_FACTORS_HEADING,
  CF_FIXED_MARK,
  CF_NEW_FACTOR,
  CF_NO_FACTOR,
  CF_NOT_IN_MODEL,
  CF_PUT_COLUMNS,
  MAX_FACTORS,
  MAX_ITEMS,
  MIN_FACTOR_ITEMS,
  MIN_ITEMS,
  cfaFactorHeading,
  cfaFactorOption,
  cfaFactorTooFew,
  cfaMissingCodeQuestion,
  type CfaNames,
} from './cfaWording';
import { offeredBy } from './orderedColumns';
import { columnLabel, columnNames } from './window/windowWording';

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** Decision 782: the factors in the order of their first columns in the file, each its columns
 *  in the file's order, the codes ascending and once each, and the view's preparation exactly
 *  when it has one. A column the view does not have, and an empty factor, are left out. */
export function buildCfaSpec(view: DatasetView, factors: string[][], correlatedFactors: boolean, missingCodes: number[], operationId: number): CfaSpec {
  const order = view.profile.columns.map((c) => c.alias);
  const at = (a: string) => order.indexOf(a);
  const indicators = factors
    .map((f) => order.filter((a) => f.includes(a)))
    .filter((f) => f.length > 0)
    .sort((a, b) => at(a[0]) - at(b[0]));
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.cfa',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { indicators, correlatedFactors, missingCodes: codes },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

/** A factor as drawn: its stable id, its number (from 1, by its first column in the file) and
 *  its columns in the file's order. */
export interface DrawnFactor {
  id: number;
  number: number;
  columns: string[];
}

/** Where a select puts its column: out of the model, into a factor by id, or a new factor. */
export type CfaTarget = 'none' | 'new' | number;

interface Model {
  /** Each column in the model, by alias, to its factor's id. */
  assigned: Record<string, number>;
  /** The next id a new factor takes. */
  next: number;
}

export interface CfaSettingsState {
  /** The factors as drawn, in their numbered order. */
  factors: DrawnFactor[];
  /** Every column in the model, in the file's order. */
  columns: string[];
  correlated: boolean;
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  names: CfaNames;
  /** The model has the most columns Gaply fits, so a column not in it offers only "Not in the
   *  model". */
  atCap: boolean;
  /** What is still missing, in the method's words; null once it can be checked. */
  stillMissing: string | null;
  /** The factor a column is in, or null. */
  factorOf: (alias: string) => DrawnFactor | null;
  /** The spec these choices give. */
  spec: (operationId: number) => CfaSpec;
  reset: () => void;
  assign: (alias: string, target: CfaTarget) => void;
  setCorrelated: (on: boolean) => void;
  toggleCode: (code: number, on: boolean) => void;
}

const EMPTY: Model = { assigned: {}, next: 1 };

/** The user's choices, over the open file's view. */
export function useCfaSettings(view: DatasetView | null): CfaSettingsState {
  const [model, setModel] = useState<Model>(EMPTY);
  const [correlated, setCorrelated] = useState(true);
  const [ticked, setTicked] = useState<number[]>([]);

  const shown = useMemo(() => (view ? numeric(view).map((c) => c.alias) : []), [view]);
  // A view change that removes a column takes it out of its factor (decision 698's prune).
  useEffect(() => {
    setModel((m) => {
      const kept = Object.keys(m.assigned).filter((a) => shown.includes(a));
      if (kept.length === Object.keys(m.assigned).length) return m;
      return { ...m, assigned: Object.fromEntries(kept.map((a) => [a, m.assigned[a]])) };
    });
  }, [shown]);

  // Everything below comes from the columns as drawn, in the file's order.
  const columns = useMemo(() => shown.filter((a) => model.assigned[a] !== undefined), [shown, model]);
  const factors = useMemo<DrawnFactor[]>(() => {
    const byId = new Map<number, string[]>();
    for (const a of columns) byId.set(model.assigned[a], [...(byId.get(model.assigned[a]) ?? []), a]);
    // A Map keeps its keys in the order first set, and `columns` is in the file's order, so
    // the factors come in the order of their first columns.
    return Array.from(byId.entries()).map(([id, cols], i) => ({ id, number: i + 1, columns: cols }));
  }, [columns, model]);
  const offered = useMemo(() => offeredBy(view, columns), [view, columns]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  // Decision 698: the ticks are pruned to the codes then offered.
  useEffect(() => {
    setTicked((t) => {
      const kept = t.filter((c) => offered.some((s) => s.code === c));
      return kept.length === t.length ? t : kept;
    });
  }, [offered]);
  const names: CfaNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback(
    (operationId: number) =>
      buildCfaSpec(
        view as DatasetView,
        factors.map((f) => f.columns),
        correlated,
        missingCodes,
        operationId
      ),
    [view, factors, correlated, missingCodes]
  );

  const reset = useCallback(() => {
    setModel(EMPTY);
    setCorrelated(true);
    setTicked([]);
  }, []);

  const assign = useCallback(
    (alias: string, target: CfaTarget) =>
      setModel((m) => {
        const assigned = { ...m.assigned };
        if (target === 'none') {
          delete assigned[alias];
          return { ...m, assigned };
        }
        if (target === 'new') {
          assigned[alias] = m.next;
          return { assigned, next: m.next + 1 };
        }
        assigned[alias] = target;
        return { ...m, assigned };
      }),
    []
  );

  const short = factors.find((f) => f.columns.length < MIN_FACTOR_ITEMS);
  const stillMissing =
    factors.length === 0 || columns.length < MIN_ITEMS
      ? CF_PUT_COLUMNS
      : short
        ? cfaFactorTooFew(short.number, short.columns.length, MIN_FACTOR_ITEMS)
        : null;

  return {
    factors,
    columns,
    correlated,
    ticked,
    offered,
    names,
    atCap: columns.length >= MAX_ITEMS,
    stillMissing,
    factorOf: (alias) => factors.find((f) => f.columns.includes(alias)) ?? null,
    spec,
    reset,
    assign,
    setCorrelated,
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

/** A select's value for a target. */
const valueOf = (t: CfaTarget) => (t === 'none' ? 'none' : t === 'new' ? 'new' : `f${t}`);
const targetOf = (v: string): CfaTarget => (v === 'none' ? 'none' : v === 'new' ? 'new' : Number(v.slice(1)));

/** The controls: the dialog's. */
export const CfaSettings: React.FC<{ view: DatasetView; settings: CfaSettingsState }> = ({ view, settings: s }) => {
  const columns = numeric(view);
  const byAlias = new Map(columns.map((c) => [c.alias, c]));
  const label = (alias: string) => {
    const c = byAlias.get(alias);
    return c ? columnLabel(c) : alias;
  };
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="cf-settings">
      <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
        <div data-testid="cf-columns">
          <h4 style={{ margin: '4px 0' }}>{CF_COLUMNS_HEADING}</h4>
          {s.atCap && (
            <p role="status" style={{ margin: '0 0 4px' }} data-testid="cf-at-cap">
              {CF_AT_CAP}
            </p>
          )}
          {columns.map((c) => {
            const f = s.factorOf(c.alias);
            const locked = f === null && s.atCap;
            return (
              <label key={c.alias} style={{ display: 'block' }}>
                <span>
                  {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>{' '}
                <select
                  value={valueOf(f === null ? 'none' : f.id)}
                  data-testid={`cf-assign-${c.alias}`}
                  onChange={(e) => s.assign(c.alias, targetOf(e.target.value))}
                >
                  <option value="none">{CF_NOT_IN_MODEL}</option>
                  {!locked &&
                    s.factors.map((g) => (
                      <option key={g.id} value={valueOf(g.id)}>
                        {cfaFactorOption(g.number)}
                      </option>
                    ))}
                  {!locked && s.factors.length < MAX_FACTORS && <option value="new">{CF_NEW_FACTOR}</option>}
                </select>
              </label>
            );
          })}
        </div>
        <div data-testid="cf-factors">
          <h4 style={{ margin: '4px 0' }}>{CF_FACTORS_HEADING}</h4>
          {s.factors.length === 0 && (
            <p style={{ margin: 0 }} data-testid="cf-none">
              {CF_NO_FACTOR}
            </p>
          )}
          {s.factors.map((f) => (
            <section key={f.id} data-testid={`cf-factor-${f.number}`}>
              <h5 style={{ margin: '4px 0' }}>{cfaFactorHeading(f.number)}</h5>
              <ul style={{ margin: 0 }}>
                {f.columns.map((a, i) => (
                  <li key={a} data-testid={`cf-factor-${f.number}-${a}`}>
                    {a} <bdi>{label(a)}</bdi>
                    {i === 0 && <> {CF_FIXED_MARK}</>}
                  </li>
                ))}
              </ul>
              {f.columns.length < MIN_FACTOR_ITEMS && (
                <p style={{ margin: 0 }} data-testid={`cf-factor-${f.number}-short`}>
                  {cfaFactorTooFew(f.number, f.columns.length, MIN_FACTOR_ITEMS)}
                </p>
              )}
            </section>
          ))}
        </div>
      </div>
      <label style={{ display: 'block' }}>
        <input
          type="checkbox"
          checked={s.correlated}
          disabled={s.factors.length < 2}
          data-testid="cf-correlated"
          onChange={(e) => s.setCorrelated(e.target.checked)}
        />{' '}
        {CF_CORRELATED_LABEL}
      </label>
      <p style={{ margin: 0 }} data-testid="cf-correlated-line">
        {CF_CORRELATED_LINE}
      </p>
      {s.offered.length > 0 && (
        <fieldset data-testid="cf-missing-codes">
          <legend>{CF_CODES_LEGEND}</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input type="checkbox" checked={s.ticked.includes(o.code)} data-testid={`cf-code-${o.code}`} onChange={(e) => s.toggleCode(o.code, e.target.checked)} />{' '}
              {cfaMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
      {s.stillMissing && (
        <p role="status" data-testid="cf-settings-missing">
          {s.stillMissing}
        </p>
      )}
    </div>
  );
};
