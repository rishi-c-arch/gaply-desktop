// Gaply — Analyze, Regression, Linear… (Research OS, Phase 9A-2, decisions 684 and 685;
// ADR-012 Part 6): MethodDialog around the linear regression's settings, on
// RelatedRanksDialog.tsx's pattern. It owns no file, and its bridge's type has no open and
// no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { LinearRegressionBridge } from '../linearRegressionBridge';
import { LinearRegressionSettings, useLinearRegressionSettings } from '../LinearRegressionSettings';
import type { LinearRegressionDiagnostic, LinearRegressionRunStateView, LinearRegressionSpec } from '../linearRegressionTypes';
import { LR_RULE, linearRegressionRefusalSentence, linearRegressionSentence } from '../linearRegressionWording';
import { MethodDialog } from './MethodDialog';
import { LINEAR_REGRESSION_DIALOG_TITLE, linearRegressionSettingsLine } from './windowWording';

export type LinearRegressionDialogBridge = Omit<LinearRegressionBridge, 'open' | 'close'>;

export const LinearRegressionDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: LinearRegressionDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: LinearRegressionRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useLinearRegressionSettings(view);
  const operation = useRef(0);

  const started = async (spec: LinearRegressionSpec, runId: number) => {
    const first: LinearRegressionRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: linearRegressionSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<LinearRegressionSpec, LinearRegressionDiagnostic>
      title={LINEAR_REGRESSION_DIALOG_TITLE}
      prefix="lr"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => linearRegressionSentence(d, settings.names)}
      refusalSentence={linearRegressionRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="lr-rule">{LR_RULE}</p>
      <LinearRegressionSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
