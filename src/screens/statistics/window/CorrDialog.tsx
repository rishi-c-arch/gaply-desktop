// Gaply — Analyze, Correlate, Correlations… (Research OS, Phase 8S, decision 484; ADR-012
// Part 6): MethodDialog around the correlations' one copy of their settings. A budget
// refusal stays in the dialog with its sentence, as any refusal does. It owns no file,
// and its bridge's type has no open and no close.
import React, { useRef } from 'react';
import type { CorrBridge } from '../corrBridge';
import { CorrSettings, useCorrSettings } from '../CorrSettings';
import type { CorrDiagnostic, CorrRunStateView, CorrSpec } from '../corrTypes';
import { corrRefusalSentence, corrSentence } from '../corrWording';
import type { DatasetView } from '../describeTypes';
import { MethodDialog } from './MethodDialog';
import { CORR_DIALOG_TITLE, corrSettingsLine } from './windowWording';

export type CorrDialogBridge = Omit<CorrBridge, 'open' | 'close'>;

export const CorrDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: CorrDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: CorrRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useCorrSettings(view);
  const operation = useRef(0);

  const started = async (spec: CorrSpec, runId: number) => {
    const first: CorrRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: corrSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<CorrSpec, CorrDiagnostic>
      title={CORR_DIALOG_TITLE}
      prefix="cr"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => corrSentence(d, settings.names)}
      refusalSentence={corrRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <CorrSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
