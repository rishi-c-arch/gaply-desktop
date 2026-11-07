// Gaply — the correlations' settings (Research OS, Phase 8S, decisions 481 and 484;
// ADR-012 Part 6): the one copy, rendered by the old page (CorrFlow.tsx) and by the
// Statistics window's dialog (window/CorrDialog.tsx). None is made for the user: "Add
// every numeric column" and "Remove all" are actions they take. What is held here is the
// user's choices, in this component's state only, and never browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import { Button } from '../../design-system';
import { COEFFICIENTS, type Coefficient, type CorrSpec, type MissingHandling } from './corrTypes';
import {
  COEFFICIENT_CHOICES,
  COEFFICIENTS_QUESTION,
  corrMissingCodeQuestion,
  MISSING_CHOICES,
  MISSING_QUESTION,
  settingsMissingSentence,
  VARIABLES_QUESTION,
  type CorrNames,
} from './corrWording';
import type { DatasetView } from './describeTypes';
import { ALPHA_ADD_ALL, ALPHA_REMOVE_ALL, columnLabel, columnNames } from './window/windowWording';

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** Decision 338: the ticked columns in the file's column order, the
 *  coefficients in COEFFICIENTS' order, the codes ascending and once each. */
export function buildCorrSpec(
  view: DatasetView,
  variables: string[],
  coefficients: Coefficient[],
  missing: MissingHandling,
  missingCodes: number[],
  operationId: number
): CorrSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.correlations',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      variables: view.profile.columns.map((c) => c.alias).filter((a) => variables.includes(a)),
      coefficients: COEFFICIENTS.filter((c) => coefficients.includes(c)),
      missing,
      missingCodes: codes,
    },
    randomSeed: null,
    confirmationRefs: [],
    // Decision 490: the view's preparation exactly when it has one; absent, never null, on a file's own table.
    ...(view.preparation ? { preparation: view.preparation } : {}),
  };
}

const toggled = <T,>(t: T[], x: T, on: boolean) => (on ? [...t.filter((y) => y !== x), x] : t.filter((y) => y !== x));

export function useCorrSettings(view: DatasetView | null) {
  const [variables, setVariables] = useState<string[]>([]);
  const [coefficients, setCoefficients] = useState<Coefficient[]>([]);
  const [missing, setMissing] = useState<MissingHandling | null>(null);
  const [ticked, setTicked] = useState<number[]>([]);

  /** The candidate codes of the ticked columns, ascending, with where each
   *  appears (decision 359: the union over the ticked columns). */
  const offered = useMemo(() => {
    const where = new Map<number, Array<{ header: string; cells: number }>>();
    for (const c of view?.profile.columns ?? []) {
      if (!variables.includes(c.alias)) continue;
      for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
    }
    return Array.from(where.keys())
      .sort((a, b) => a - b)
      .map((code) => ({ code, where: where.get(code) ?? [] }));
  }, [view, variables]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);
  const names: CorrNames = useMemo(() => ({ column: columnNames(view) }), [view]);

  const spec = useCallback(
    (operationId: number) => buildCorrSpec(view as DatasetView, variables, coefficients, missing as MissingHandling, missingCodes, operationId),
    [view, variables, coefficients, missing, missingCodes]
  );

  const reset = useCallback(() => {
    setVariables([]);
    setCoefficients([]);
    setMissing(null);
    setTicked([]);
  }, []);

  return {
    variables,
    coefficients,
    missing,
    ticked,
    offered,
    names,
    stillMissing: settingsMissingSentence(variables.length, coefficients.length, missing !== null),
    spec,
    reset,
    setMissing,
    addAll: () => setVariables(numeric(view as DatasetView).map((c) => c.alias)),
    removeAll: () => setVariables([]),
    toggleVariable: (alias: string, on: boolean) => setVariables((t) => toggled(t, alias, on)),
    toggleCoefficient: (c: Coefficient, on: boolean) => setCoefficients((t) => toggled(t, c, on)),
    toggleCode: (code: number, on: boolean) => setTicked((t) => toggled(t, code, on)),
  };
}

export type CorrSettingsState = ReturnType<typeof useCorrSettings>;

/** The controls, the page's and the dialog's alike. */
export const CorrSettings: React.FC<{ view: DatasetView; settings: CorrSettingsState }> = ({ view, settings: s }) => (
  <div style={{ display: 'grid', gap: 8 }}>
    <fieldset data-testid="cr-variables">
      <legend>{VARIABLES_QUESTION}</legend>
      <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
        <Button variant="secondary" onClick={s.addAll} data-testid="cr-tick-all">
          {ALPHA_ADD_ALL}
        </Button>
        <Button variant="secondary" onClick={s.removeAll} data-testid="cr-untick-all">
          {ALPHA_REMOVE_ALL}
        </Button>
      </div>
      {numeric(view).map((c) => (
        <label key={c.alias} style={{ display: 'block' }}>
          <input
            type="checkbox"
            checked={s.variables.includes(c.alias)}
            data-testid={`cr-variable-${c.alias}`}
            onChange={(e) => s.toggleVariable(c.alias, e.target.checked)}
          />{' '}
          {c.alias} {columnLabel(c)}
        </label>
      ))}
    </fieldset>
    <fieldset data-testid="cr-coefficients">
      <legend>{COEFFICIENTS_QUESTION}</legend>
      {COEFFICIENT_CHOICES.map((c) => (
        <div key={c.coefficient}>
          <label style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.coefficients.includes(c.coefficient)}
              data-testid={`cr-coefficient-${c.coefficient}`}
              aria-describedby={`cr-coefficient-line-${c.coefficient}`}
              onChange={(e) => s.toggleCoefficient(c.coefficient, e.target.checked)}
            />{' '}
            {c.label}
          </label>
          <p id={`cr-coefficient-line-${c.coefficient}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
            {c.line}
          </p>
        </div>
      ))}
    </fieldset>
    <fieldset data-testid="cr-missing">
      <legend>{MISSING_QUESTION}</legend>
      {MISSING_CHOICES.map((c) => (
        <div key={c.missing}>
          <label style={{ display: 'block' }}>
            <input
              type="radio"
              name="cr-missing"
              checked={s.missing === c.missing}
              data-testid={`cr-missing-${c.missing}`}
              aria-describedby={`cr-missing-line-${c.missing}`}
              onChange={() => s.setMissing(c.missing)}
            />{' '}
            {c.label}
          </label>
          <p id={`cr-missing-line-${c.missing}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
            {c.line}
          </p>
        </div>
      ))}
    </fieldset>
    {s.offered.length > 0 && (
      <fieldset data-testid="cr-missing-codes">
        <legend>Candidate missing codes</legend>
        {s.offered.map((o) => (
          <label key={o.code} style={{ display: 'block' }}>
            <input
              type="checkbox"
              checked={s.ticked.includes(o.code)}
              data-testid={`cr-code-${o.code}`}
              onChange={(e) => s.toggleCode(o.code, e.target.checked)}
            />{' '}
            {corrMissingCodeQuestion(o.code, o.where)}
          </label>
        ))}
      </fieldset>
    )}
    {s.stillMissing && (
      <p role="status" data-testid="cr-settings-missing">
        {s.stillMissing}
      </p>
    )}
  </div>
);
