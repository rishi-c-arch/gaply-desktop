// Gaply — Analyze, Descriptive statistics, Crosstabs… (Research OS, Phase 8X-2,
// decisions 588 and 589; ADR-012 Part 6): MethodDialog around crosstabs' one copy of
// its settings, on AnovaDialog.tsx's pattern. It owns no file, and its bridge's type
// has no open and no close; crosstabs has no levels command to keep.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { XtabBridge } from '../xtabBridge';
import { useXtabSettings, XtabSettings } from '../XtabSettings';
import type { XtabDiagnostic, XtabRunStateView, XtabSpec } from '../xtabTypes';
import { XTAB_LEVELS_RULE, xtabRefusalSentence, xtabSentence } from '../xtabWording';
import { MethodDialog } from './MethodDialog';
import { XTAB_DIALOG_TITLE, xtabSettingsLine } from './windowWording';

export type XtabDialogBridge = Omit<XtabBridge, 'open' | 'close'>;

export const XtabDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: XtabDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: XtabRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useXtabSettings(view);
  const operation = useRef(0);

  const started = async (spec: XtabSpec, runId: number) => {
    const first: XtabRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: xtabSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<XtabSpec, XtabDiagnostic>
      title={XTAB_DIALOG_TITLE}
      prefix="xt"
      slot={slot}
      ready={settings.ready}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => xtabSentence(d, settings.names)}
      refusalSentence={xtabRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="xt-levels-rule">{XTAB_LEVELS_RULE}</p>
      <XtabSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
