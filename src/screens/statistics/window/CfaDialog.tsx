// Gaply — Analyze, Structural equations, Confirmatory factor analysis… (Research OS, Phase
// 9C-3, decisions 782 and 783; ADR-012 Part 6): MethodDialog around the confirmatory factor
// analysis's model editor, on FactorDialog.tsx's pattern. It owns no file, and its bridge's
// type has no open and no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { CfaBridge } from '../cfaBridge';
import { CfaSettings, useCfaSettings } from '../CfaSettings';
import type { CfaDiagnostic, CfaRunStateView, CfaSpec } from '../cfaTypes';
import { CF_RULE, cfaRefusalSentence, cfaSentence } from '../cfaWording';
import { MethodDialog } from './MethodDialog';
import { CFA_DIALOG_TITLE, cfaSettingsLine } from './windowWording';

export type CfaDialogBridge = Omit<CfaBridge, 'open' | 'close'>;

export const CfaDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: CfaDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: CfaRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useCfaSettings(view);
  const operation = useRef(0);

  const started = async (spec: CfaSpec, runId: number) => {
    const first: CfaRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: cfaSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<CfaSpec, CfaDiagnostic>
      title={CFA_DIALOG_TITLE}
      prefix="cf"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => cfaSentence(d, settings.names)}
      refusalSentence={cfaRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="cf-rule">{CF_RULE}</p>
      <CfaSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
