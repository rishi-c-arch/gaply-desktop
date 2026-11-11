// Gaply — the linear regression's settings (Research OS, Phase 9A-2, decisions 684 and 685).
// The outcome is a select of the numeric columns, scores included, on OneSampleSettings.tsx's
// pattern. The predictors are AlphaSettings.tsx's two panes as RelatedRanksSettings.tsx draws
// them: the numeric columns other than the outcome on the left in the file's order; the
// chosen predictors on the right in the list's order, each numbered, with ↑, ↓ and Remove.
// None is chosen for the user. Choosing an outcome takes it out of the predictors and clears
// every tick (decision 652's rule for a single-column choice); choosing another outcome later
// does not put it back. Each candidate missing code of the outcome and the predictors is
// asked as a question; a tick is dropped with the last column that offered it.
//
// The spec keeps the predictors in the list's order, never sorted, and the codes ascending
// and once each. What is held here is the user's choices, in this component's state only.
import React, { useCallback, useMemo, useState } from 'react';
import { Button } from '../../design-system';
import type { DatasetView } from './describeTypes';
import type { LinearRegressionSpec } from './linearRegressionTypes';
import {
  LR_CHOOSE_OUTCOME,
  LR_CHOOSE_PREDICTOR,
  LR_CODES_LEGEND,
  LR_LIST_HEADING,
  LR_OUTCOME_EMPTY,
  LR_OUTCOME_LABEL,
  LR_PREDICTORS_LEGEND,
  MAX_PREDICTORS,
  linearRegressionMissingCodeQuestion,
  linearRegressionMoveDown,
  linearRegressionMoveUp,
  linearRegressionTooManyPredictors,
  type LinearRegressionNames,
} from './linearRegressionWording';
import { addColumn, moveColumnDown, moveColumnUp, offeredBy, removeColumn } from './orderedColumns';
import { ALPHA_ADD_ALL, ALPHA_COLUMNS_HEADING, ALPHA_ITEMS_NONE, ALPHA_REMOVE_ALL, ALPHA_REMOVE_ITEM, columnLabel, columnNames } from './window/windowWording';

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** Decision 684: the predictors in the list's order, never sorted, the codes ascending and
 *  once each, and the view's preparation exactly when it has one. */
export function buildLinearRegressionSpec(
  view: DatasetView,
  outcome: string,
  predictors: string[],
  missingCodes: number[],
  operationId: number
): LinearRegressionSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.linear_regression',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { outcome, predictors, missingCodes: codes },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

export interface LinearRegressionSettingsState {
  /** The chosen outcome when the view still has it among its numeric columns; '' otherwise. */
  outcome: string;
  /** The chosen predictors as drawn: those still among the view's numeric columns, in the
   *  list's order (decision 668). A chosen alias the view has lost stays chosen, undrawn and
   *  unsent. */
  predictors: string[];
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  names: LinearRegressionNames;
  /** What is still missing, in the method's words; null once it can be checked. */
  stillMissing: string | null;
  /** The spec these choices give. */
  spec: (operationId: number) => LinearRegressionSpec;
  reset: () => void;
  setOutcome: (alias: string) => void;
  togglePredictor: (alias: string, on: boolean) => void;
  movePredictor: (alias: string, by: -1 | 1) => void;
  addAll: () => void;
  removeAll: () => void;
  toggleCode: (code: number, on: boolean) => void;
}

/** The user's choices, over the open file's view. */
export function useLinearRegressionSettings(view: DatasetView | null): LinearRegressionSettingsState {
  const [chosenOutcome, setChosenOutcome] = useState('');
  const [chosen, setChosen] = useState<string[]>([]);
  const [ticked, setTicked] = useState<number[]>([]);

  // Decision 668: everything below comes from the outcome and the list as drawn.
  const shown = useMemo(() => (view ? numeric(view).map((c) => c.alias) : []), [view]);
  const outcome = shown.includes(chosenOutcome) ? chosenOutcome : '';
  const predictors = useMemo(() => chosen.filter((a) => shown.includes(a) && a !== outcome), [chosen, shown, outcome]);
  const columns = useMemo(() => (outcome ? [outcome, ...predictors] : predictors), [outcome, predictors]);
  const offered = useMemo(() => offeredBy(view, columns), [view, columns]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  const names: LinearRegressionNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback(
    (operationId: number) => buildLinearRegressionSpec(view as DatasetView, outcome, predictors, missingCodes, operationId),
    [view, outcome, predictors, missingCodes]
  );

  const reset = useCallback(() => {
    setChosenOutcome('');
    setChosen([]);
    setTicked([]);
  }, []);

  /** The list after a change: a tick goes with the last chosen column that offered it. */
  const listIs = (next: string[]) => {
    setChosen(next);
    const still = offeredBy(view, outcome ? [outcome, ...next] : next);
    setTicked((t) => t.filter((c) => still.some((s) => s.code === c)));
  };

  const stillMissing =
    outcome === ''
      ? LR_CHOOSE_OUTCOME
      : predictors.length === 0
        ? LR_CHOOSE_PREDICTOR
        : predictors.length > MAX_PREDICTORS
          ? linearRegressionTooManyPredictors(predictors.length)
          : null;

  return {
    outcome,
    predictors,
    ticked,
    offered,
    names,
    stillMissing,
    spec,
    reset,
    // Decision 684: the outcome leaves the list, and every tick is cleared.
    setOutcome: (alias) => {
      setChosenOutcome(alias);
      setChosen((list) => removeColumn(list, alias));
      setTicked([]);
    },
    togglePredictor: (alias, on) => listIs(on ? addColumn(chosen, alias) : removeColumn(chosen, alias)),
    movePredictor: (alias, by) => setChosen(by < 0 ? moveColumnUp(chosen, alias, predictors) : moveColumnDown(chosen, alias, predictors)),
    addAll: () => listIs(numeric(view as DatasetView).reduce((list, c) => (c.alias === outcome ? list : addColumn(list, c.alias)), chosen)),
    removeAll: () => listIs([]),
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code))),
  };
}

/** The controls: the dialog's. */
export const LinearRegressionSettings: React.FC<{ view: DatasetView; settings: LinearRegressionSettingsState }> = ({ view, settings: s }) => {
  const columns = numeric(view);
  const offeredLeft = columns.filter((c) => c.alias !== s.outcome);
  const chosen = s.predictors.map((alias) => columns.find((c) => c.alias === alias)).filter((c): c is (typeof columns)[number] => c !== undefined);
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="lr-settings">
      <label>
        {LR_OUTCOME_LABEL}{' '}
        <select value={s.outcome} data-testid="lr-outcome" onChange={(e) => s.setOutcome(e.target.value)}>
          <option value="">{LR_OUTCOME_EMPTY}</option>
          {columns.map((c) => (
            <option key={c.alias} value={c.alias}>
              {c.alias} {columnLabel(c)}
            </option>
          ))}
        </select>
      </label>
      <fieldset data-testid="lr-predictor-choices">
        <legend>{LR_PREDICTORS_LEGEND}</legend>
        <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
          <Button variant="secondary" onClick={s.addAll} data-testid="lr-add-all">
            {ALPHA_ADD_ALL}
          </Button>
          <Button variant="secondary" onClick={s.removeAll} data-testid="lr-remove-all">
            {ALPHA_REMOVE_ALL}
          </Button>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
          <div data-testid="lr-columns">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_COLUMNS_HEADING}</h4>
            {offeredLeft.map((c) => (
              <label key={c.alias} style={{ display: 'block' }}>
                <input
                  type="checkbox"
                  checked={s.predictors.includes(c.alias)}
                  data-testid={`lr-column-${c.alias}`}
                  onChange={(e) => s.togglePredictor(c.alias, e.target.checked)}
                />{' '}
                {c.alias} <bdi>{columnLabel(c)}</bdi>
              </label>
            ))}
          </div>
          <div data-testid="lr-list">
            <h4 style={{ margin: '4px 0' }}>{LR_LIST_HEADING}</h4>
            {chosen.length === 0 && (
              <p style={{ margin: 0 }} data-testid="lr-none">
                {ALPHA_ITEMS_NONE}
              </p>
            )}
            {chosen.map((c, i) => (
              <div key={c.alias} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }} data-testid={`lr-row-${c.alias}`}>
                <span data-testid="lr-row-name">
                  {i + 1}. {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>
                <Button variant="ghost" aria-label={linearRegressionMoveUp(c.alias)} disabled={i === 0} onClick={() => s.movePredictor(c.alias, -1)} data-testid={`lr-up-${c.alias}`}>
                  ↑
                </Button>
                <Button
                  variant="ghost"
                  aria-label={linearRegressionMoveDown(c.alias)}
                  disabled={i === chosen.length - 1}
                  onClick={() => s.movePredictor(c.alias, 1)}
                  data-testid={`lr-down-${c.alias}`}
                >
                  ↓
                </Button>
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} ${c.alias}`} onClick={() => s.togglePredictor(c.alias, false)} data-testid={`lr-remove-${c.alias}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
              </div>
            ))}
          </div>
        </div>
      </fieldset>
      {s.offered.length > 0 && (
        <fieldset data-testid="lr-missing-codes">
          <legend>{LR_CODES_LEGEND}</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input type="checkbox" checked={s.ticked.includes(o.code)} data-testid={`lr-code-${o.code}`} onChange={(e) => s.toggleCode(o.code, e.target.checked)} />{' '}
              {linearRegressionMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
      {s.stillMissing && (
        <p role="status" data-testid="lr-settings-missing">
          {s.stillMissing}
        </p>
      )}
    </div>
  );
};
