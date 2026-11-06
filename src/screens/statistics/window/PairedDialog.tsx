// Gaply — Analyze, Compare means, Paired t-test… (Research OS, Phase 8S, decision 483;
// ADR-012 Part 6): MethodDialog around the test's one copy of its settings. It owns no
// file, and its bridge's type has no open and no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { PairedBridge } from '../onesampleBridge';
import { PairedSettings, usePairedSettings } from '../PairedSettings';
import type { OneSampleDiagnostic, OneSampleRunStateView, PairedSpec } from '../onesampleTypes';
import { oneSampleRefusalSentence, oneSampleSentence } from '../onesampleWording';
import { MethodDialog } from './MethodDialog';
import { PAIRED_DIALOG_TITLE, pairedSettingsLine } from './windowWording';

export type PairedDialogBridge = Omit<PairedBridge, 'open' | 'close'>;

export const PairedDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: PairedDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: OneSampleRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = usePairedSettings(view);
  const operation = useRef(0);

  const started = async (spec: PairedSpec, runId: number) => {
    const first: OneSampleRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: pairedSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<PairedSpec, OneSampleDiagnostic>
      title={PAIRED_DIALOG_TITLE}
      prefix="pt"
      slot={slot}
      ready={settings.ready}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => oneSampleSentence(d, settings.names)}
      refusalSentence={oneSampleRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <PairedSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
