// Gaply — Analyze, Compare means, One-sample t-test… (Research OS, Phase 8S, decision
// 483; ADR-012 Part 6): MethodDialog around the test's one copy of its settings. It owns
// no file, and its bridge's type has no open and no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { OneSampleBridge } from '../onesampleBridge';
import { OneSampleSettings, useOneSampleSettings } from '../OneSampleSettings';
import type { OneSampleDiagnostic, OneSampleRunStateView, OneSampleSpec } from '../onesampleTypes';
import { oneSampleRefusalSentence, oneSampleSentence } from '../onesampleWording';
import { MethodDialog } from './MethodDialog';
import { ONE_SAMPLE_DIALOG_TITLE, oneSampleSettingsLine } from './windowWording';

export type OneSampleDialogBridge = Omit<OneSampleBridge, 'open' | 'close'>;

export const OneSampleDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: OneSampleDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: OneSampleRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useOneSampleSettings(view);
  const operation = useRef(0);

  const started = async (spec: OneSampleSpec, runId: number) => {
    const first: OneSampleRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: oneSampleSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<OneSampleSpec, OneSampleDiagnostic>
      title={ONE_SAMPLE_DIALOG_TITLE}
      prefix="os"
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
      <OneSampleSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
