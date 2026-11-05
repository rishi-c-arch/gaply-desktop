// Gaply — Analyze, Reliability (Cronbach's alpha)… (Research OS, Phase 8R, decision
// 470; ADR-012 Part 6): MethodDialog around alpha's one copy of its settings. It
// owns no file, and its bridge's type has no open and no close, so a dialog that
// closed the shared slot would not compile.
import React, { useMemo, useRef } from 'react';
import type { AlphaBridge } from '../alphaBridge';
import { AlphaSettings, alphaNames, useAlphaSettings } from '../AlphaSettings';
import type { AlphaRunStateView, AlphaSpec, AlphaValidationView } from '../alphaTypes';
import { alphaRefusalSentence, alphaSentence } from '../alphaWording';
import type { DatasetView } from '../describeTypes';
import { MethodDialog } from './MethodDialog';
import { ALPHA_DIALOG_TITLE, alphaSettingsLine } from './windowWording';

export type AlphaDialogBridge = Omit<AlphaBridge, 'open' | 'close'>;

export interface StartedAlpha {
  runId: number;
  settings: string;
  run: AlphaRunStateView;
}

export const AlphaDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: AlphaDialogBridge;
  outputFull: boolean;
  /** The run has started: its parent makes the entry and closes this. */
  onStarted: (started: StartedAlpha) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useAlphaSettings(view);
  const operation = useRef(0);
  const names = useMemo(() => alphaNames(view), [view]);
  const settingsKey = JSON.stringify(settings.spec(0).parameters);

  const buildSpec = () => {
    operation.current += 1;
    return settings.spec(operation.current);
  };

  const started = async (spec: AlphaSpec, runId: number) => {
    // Before the first poll: nothing is known yet but that it was accepted.
    const first: AlphaRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: alphaSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<AlphaSpec, AlphaValidationView['errors'][number]>
      title={ALPHA_DIALOG_TITLE}
      prefix="al"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      settingsKey={settingsKey}
      buildSpec={buildSpec}
      bridge={bridge}
      sentence={(d) => alphaSentence(d, names)}
      refusalSentence={alphaRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <AlphaSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
