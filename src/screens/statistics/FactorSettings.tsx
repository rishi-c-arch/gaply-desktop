// Gaply — the factor analysis's settings (Research OS, Phase 9B-3, decisions 729 and 730).
// The items are AlphaSettings.tsx's two panes: the numeric columns, scores included, on the
// left in the file's order; the chosen items on the right, drawn in the file's order whatever
// order they were ticked in. None is chosen for the user. Then the extraction (principal
// components first and chosen at the start), the number of factors (Kaiser's rule first and
// chosen at the start, or a fixed number the user types), the rotation (none first and chosen
// at the start, as SPSS's dialog starts), the iterations (25 at the start) and the convergence
// (0.001 at the start), each typed on OneSampleSettings.tsx's pattern. Each candidate missing
// code of the drawn items is asked as a question; a tick is dropped with the last item that
// offered it, and a view change prunes the ticks (decision 698).
//
// The spec lists the items in the file's order, factors null for Kaiser's rule, and the codes
// ascending and once each. What is held here is the user's choices, in this component's state
// only.
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button } from '../../design-system';
import type { DatasetView } from './describeTypes';
import type { Extraction, FactorSpec, Rotation } from './factorTypes';
import {
  DEFAULT_CONVERGENCE,
  DEFAULT_ITERATIONS,
  FA_CHOOSE_ITEMS,
  FA_CODES_LEGEND,
  FA_CONVERGENCE_LABEL,
  FA_CONVERGENCE_RANGE,
  FA_EXTRACTION_LABELS,
  FA_EXTRACTION_LEGEND,
  FA_FACTORS_FIXED,
  FA_FACTORS_INPUT,
  FA_FACTORS_KAISER,
  FA_FACTORS_LEGEND,
  FA_ITEMS_LEGEND,
  FA_ITERATIONS_LABEL,
  FA_ITERATIONS_RANGE,
  FA_ROTATION_LABELS,
  FA_ROTATION_LEGEND,
  MAX_CONVERGENCE,
  MAX_ITEMS,
  MAX_ITERATIONS,
  MIN_CONVERGENCE,
  MIN_ITEMS,
  MIN_ITERATIONS,
  factorFactorsRange,
  factorMissingCodeQuestion,
  factorTooManyItems,
  type FactorNames,
} from './factorWording';
import { offeredBy } from './orderedColumns';
import { SCALE_FIELD_STYLE } from './AlphaSettings';
import { ALPHA_ADD_ALL, ALPHA_COLUMNS_HEADING, ALPHA_ITEMS_HEADING, ALPHA_ITEMS_NONE, ALPHA_REMOVE_ALL, ALPHA_REMOVE_ITEM, columnLabel, columnNames } from './window/windowWording';

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

const EXTRACTIONS: Extraction[] = ['principal_components', 'principal_axis'];
const ROTATIONS: Rotation[] = ['none', 'varimax', 'promax'];

/** A typed whole number, or null when the trimmed text is not one. */
export function wholeNumberOf(text: string): number | null {
  const t = text.trim();
  return /^\d+$/.test(t) ? Number(t) : null;
}

/** A typed finite number, or null. Never a default. */
export function typedNumberOf(text: string): number | null {
  if (text.trim() === '') return null;
  const v = Number(text);
  return Number.isFinite(v) ? v : null;
}

/** The most factors these drawn items allow: the items for principal components, one fewer
 *  for principal axis (stats_factor's factors_out_of_range). */
export function maxFactors(extraction: Extraction, items: number): number {
  return extraction === 'principal_components' ? items : items - 1;
}

/** Decision 729: the items in the file's column order, factors null for Kaiser's rule, the
 *  codes ascending and once each, and the view's preparation exactly when it has one. */
export function buildFactorSpec(
  view: DatasetView,
  items: string[],
  extraction: Extraction,
  factors: number | null,
  rotation: Rotation,
  iterations: number,
  convergence: number,
  missingCodes: number[],
  operationId: number
): FactorSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  const inOrder = view.profile.columns.map((c) => c.alias).filter((a) => items.includes(a));
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.factor',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { items: inOrder, extraction, factors, rotation, iterations, convergence, missingCodes: codes },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

export interface FactorSettingsState {
  /** The chosen items as drawn: those still among the view's numeric columns, in the file's
   *  order (decision 668's drawn-list rule). */
  items: string[];
  extraction: Extraction;
  fixed: boolean;
  factorsText: string;
  rotation: Rotation;
  iterationsText: string;
  convergenceText: string;
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  names: FactorNames;
  /** What is still missing, in the method's words; null once it can be checked. */
  stillMissing: string | null;
  /** The spec these choices give. */
  spec: (operationId: number) => FactorSpec;
  reset: () => void;
  toggleItem: (alias: string, on: boolean) => void;
  addAll: () => void;
  removeAll: () => void;
  setExtraction: (e: Extraction) => void;
  setFixed: (fixed: boolean) => void;
  setFactorsText: (text: string) => void;
  setRotation: (r: Rotation) => void;
  setIterationsText: (text: string) => void;
  setConvergenceText: (text: string) => void;
  toggleCode: (code: number, on: boolean) => void;
}

/** The user's choices, over the open file's view. */
export function useFactorSettings(view: DatasetView | null): FactorSettingsState {
  const [chosen, setChosen] = useState<string[]>([]);
  const [extraction, setExtraction] = useState<Extraction>('principal_components');
  const [fixed, setFixed] = useState(false);
  const [factorsText, setFactorsText] = useState('');
  const [rotation, setRotation] = useState<Rotation>('none');
  const [iterationsText, setIterationsText] = useState(String(DEFAULT_ITERATIONS));
  const [convergenceText, setConvergenceText] = useState(String(DEFAULT_CONVERGENCE));
  const [ticked, setTicked] = useState<number[]>([]);

  // Decision 668: everything below comes from the items as drawn, in the file's order.
  const shown = useMemo(() => (view ? numeric(view).map((c) => c.alias) : []), [view]);
  const items = useMemo(() => shown.filter((a) => chosen.includes(a)), [chosen, shown]);
  const offered = useMemo(() => offeredBy(view, items), [view, items]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  // Decision 698: a view change prunes the ticks to the codes then offered.
  useEffect(() => {
    setTicked((t) => {
      const kept = t.filter((c) => offered.some((s) => s.code === c));
      return kept.length === t.length ? t : kept;
    });
  }, [offered]);
  const names: FactorNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const factors = fixed ? wholeNumberOf(factorsText) : null;
  const iterations = wholeNumberOf(iterationsText);
  const convergence = typedNumberOf(convergenceText);
  const top = maxFactors(extraction, items.length);

  const spec = useCallback(
    (operationId: number) =>
      buildFactorSpec(view as DatasetView, items, extraction, fixed ? factors ?? Number.NaN : null, rotation, iterations ?? Number.NaN, convergence ?? Number.NaN, missingCodes, operationId),
    [view, items, extraction, fixed, factors, rotation, iterations, convergence, missingCodes]
  );

  const reset = useCallback(() => {
    setChosen([]);
    setExtraction('principal_components');
    setFixed(false);
    setFactorsText('');
    setRotation('none');
    setIterationsText(String(DEFAULT_ITERATIONS));
    setConvergenceText(String(DEFAULT_CONVERGENCE));
    setTicked([]);
  }, []);

  /** The list after a change: a tick goes with the last chosen item that offered it. */
  const listIs = (next: string[]) => {
    setChosen(next);
    const still = offeredBy(view, next);
    setTicked((t) => t.filter((c) => still.some((s) => s.code === c)));
  };

  const stillMissing =
    items.length < MIN_ITEMS
      ? FA_CHOOSE_ITEMS
      : items.length > MAX_ITEMS
        ? factorTooManyItems(items.length)
        : fixed && (factors === null || factors < 1 || factors > top)
          ? factorFactorsRange(top)
          : iterations === null || iterations < MIN_ITERATIONS || iterations > MAX_ITERATIONS
            ? FA_ITERATIONS_RANGE
            : convergence === null || convergence < MIN_CONVERGENCE || convergence > MAX_CONVERGENCE
              ? FA_CONVERGENCE_RANGE
              : null;

  return {
    items,
    extraction,
    fixed,
    factorsText,
    rotation,
    iterationsText,
    convergenceText,
    ticked,
    offered,
    names,
    stillMissing,
    spec,
    reset,
    toggleItem: (alias, on) => listIs(on ? [...chosen.filter((a) => a !== alias), alias] : chosen.filter((a) => a !== alias)),
    addAll: () => listIs(numeric(view as DatasetView).map((c) => c.alias)),
    removeAll: () => listIs([]),
    setExtraction,
    setFixed,
    setFactorsText,
    setRotation,
    setIterationsText,
    setConvergenceText,
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

/** The controls: the dialog's. */
export const FactorSettings: React.FC<{ view: DatasetView; settings: FactorSettingsState }> = ({ view, settings: s }) => {
  const columns = numeric(view);
  const chosen = columns.filter((c) => s.items.includes(c.alias));
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="fa-settings">
      <fieldset data-testid="fa-item-choices">
        <legend>{FA_ITEMS_LEGEND}</legend>
        <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
          <Button variant="secondary" onClick={s.addAll} data-testid="fa-add-all">
            {ALPHA_ADD_ALL}
          </Button>
          <Button variant="secondary" onClick={s.removeAll} data-testid="fa-remove-all">
            {ALPHA_REMOVE_ALL}
          </Button>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
          <div data-testid="fa-columns">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_COLUMNS_HEADING}</h4>
            {columns.map((c) => (
              <label key={c.alias} style={{ display: 'block' }}>
                <input type="checkbox" checked={s.items.includes(c.alias)} data-testid={`fa-column-${c.alias}`} onChange={(e) => s.toggleItem(c.alias, e.target.checked)} />{' '}
                {c.alias} <bdi>{columnLabel(c)}</bdi>
              </label>
            ))}
          </div>
          <div data-testid="fa-list">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_ITEMS_HEADING}</h4>
            {chosen.length === 0 && (
              <p style={{ margin: 0 }} data-testid="fa-none">
                {ALPHA_ITEMS_NONE}
              </p>
            )}
            {chosen.map((c) => (
              <div key={c.alias} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }} data-testid={`fa-row-${c.alias}`}>
                <span data-testid="fa-row-name">
                  {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} ${c.alias}`} onClick={() => s.toggleItem(c.alias, false)} data-testid={`fa-remove-${c.alias}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
              </div>
            ))}
          </div>
        </div>
      </fieldset>
      <fieldset data-testid="fa-extraction">
        <legend>{FA_EXTRACTION_LEGEND}</legend>
        {EXTRACTIONS.map((e) => (
          <label key={e} style={{ display: 'block' }}>
            <input type="radio" name="fa-extraction" checked={s.extraction === e} data-testid={`fa-extraction-${e}`} onChange={() => s.setExtraction(e)} />{' '}
            {FA_EXTRACTION_LABELS[e]}
          </label>
        ))}
      </fieldset>
      <fieldset data-testid="fa-factors">
        <legend>{FA_FACTORS_LEGEND}</legend>
        <label style={{ display: 'block' }}>
          <input type="radio" name="fa-factors" checked={!s.fixed} data-testid="fa-factors-kaiser" onChange={() => s.setFixed(false)} /> {FA_FACTORS_KAISER}
        </label>
        <label style={{ display: 'block' }}>
          <input type="radio" name="fa-factors" checked={s.fixed} data-testid="fa-factors-fixed" onChange={() => s.setFixed(true)} /> {FA_FACTORS_FIXED}
        </label>
        {s.fixed && (
          <label style={{ display: 'block', marginLeft: '1.5em' }}>
            <span>{FA_FACTORS_INPUT}</span>{' '}
            <input
              type="text"
              inputMode="numeric"
              value={s.factorsText}
              data-testid="fa-factors-number"
              style={SCALE_FIELD_STYLE}
              onChange={(e) => s.setFactorsText(e.target.value)}
            />
          </label>
        )}
      </fieldset>
      <fieldset data-testid="fa-rotation">
        <legend>{FA_ROTATION_LEGEND}</legend>
        {ROTATIONS.map((r) => (
          <label key={r} style={{ display: 'block' }}>
            <input type="radio" name="fa-rotation" checked={s.rotation === r} data-testid={`fa-rotation-${r}`} onChange={() => s.setRotation(r)} /> {FA_ROTATION_LABELS[r]}
          </label>
        ))}
      </fieldset>
      <label>
        <span>{FA_ITERATIONS_LABEL}</span>{' '}
        <input
          type="text"
          inputMode="numeric"
          value={s.iterationsText}
          data-testid="fa-iterations"
          style={SCALE_FIELD_STYLE}
          onChange={(e) => s.setIterationsText(e.target.value)}
        />
      </label>
      <label>
        <span>{FA_CONVERGENCE_LABEL}</span>{' '}
        <input
          type="text"
          inputMode="decimal"
          value={s.convergenceText}
          data-testid="fa-convergence"
          style={SCALE_FIELD_STYLE}
          onChange={(e) => s.setConvergenceText(e.target.value)}
        />
      </label>
      {s.offered.length > 0 && (
        <fieldset data-testid="fa-missing-codes">
          <legend>{FA_CODES_LEGEND}</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input type="checkbox" checked={s.ticked.includes(o.code)} data-testid={`fa-code-${o.code}`} onChange={(e) => s.toggleCode(o.code, e.target.checked)} />{' '}
              {factorMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
      {s.stillMissing && (
        <p role="status" data-testid="fa-settings-missing">
          {s.stillMissing}
        </p>
      )}
    </div>
  );
};
