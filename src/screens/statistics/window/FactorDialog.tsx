// Gaply — Analyze, Dimension reduction, Factor… (Research OS, Phase 9B-3, decisions 729 and
// 730; ADR-012 Part 6): MethodDialog around the factor analysis's settings, on
// LinearRegressionDialog.tsx's pattern. It owns no file, and its bridge's type has no open and
// no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { FactorBridge } from '../factorBridge';
import { FactorSettings, useFactorSettings } from '../FactorSettings';
import type { FactorDiagnostic, FactorRunStateView, FactorSpec } from '../factorTypes';
import { FA_RULE, factorRefusalSentence, factorSentence } from '../factorWording';
import { MethodDialog } from './MethodDialog';
import { FACTOR_DIALOG_TITLE, factorSettingsLine } from './windowWording';

export type FactorDialogBridge = Omit<FactorBridge, 'open' | 'close'>;

export const FactorDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: FactorDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: FactorRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useFactorSettings(view);
  const operation = useRef(0);

  const started = async (spec: FactorSpec, runId: number) => {
    const first: FactorRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: factorSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<FactorSpec, FactorDiagnostic>
      title={FACTOR_DIALOG_TITLE}
      prefix="fa"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      // The parameters, and which number-of-factors radio is chosen with its text: an empty
      // fixed number sends NaN, which JSON writes as null, Kaiser's rule's value too.
      settingsKey={JSON.stringify([settings.spec(0).parameters, settings.fixed, settings.factorsText])}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => factorSentence(d, settings.names)}
      refusalSentence={factorRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="fa-rule">{FA_RULE}</p>
      <FactorSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
