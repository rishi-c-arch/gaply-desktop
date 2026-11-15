// Gaply — Analyze, Structural equations, Structural equation model… (Research OS, Phase 9C-5,
// decisions 814 and 815; ADR-012 Part 6): MethodDialog around the structural equation model's
// model editor, on CfaDialog.tsx's pattern. It owns no file, and its bridge's type has no open
// and no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { SemBridge } from '../semBridge';
import { SemSettings, useSemSettings } from '../SemSettings';
import type { SemDiagnostic, SemRunStateView, SemSpec } from '../semTypes';
import { SM_RULE, semRefusalSentence, semSentence } from '../semWording';
import { MethodDialog } from './MethodDialog';
import { SEM_DIALOG_TITLE, semSettingsLine } from './windowWording';

export type SemDialogBridge = Omit<SemBridge, 'open' | 'close'>;

export const SemDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: SemDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: SemRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useSemSettings(view);
  const operation = useRef(0);

  const started = async (spec: SemSpec, runId: number) => {
    const first: SemRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: semSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<SemSpec, SemDiagnostic>
      title={SEM_DIALOG_TITLE}
      prefix="sm"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => semSentence(d, settings.names)}
      refusalSentence={semRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="sm-rule">{SM_RULE}</p>
      <SemSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
