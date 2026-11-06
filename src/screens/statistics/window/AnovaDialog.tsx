// Gaply — Analyze, Compare means, One-way ANOVA… (Research OS, Phase 8S, decision 484;
// ADR-012 Part 6): MethodDialog around the ANOVA's one copy of its settings. It owns no
// file, and its bridge's type has no open and no close; it keeps groupLevels, which the
// settings read.
import React, { useRef, useState } from 'react';
import type { AnovaBridge } from '../anovaBridge';
import { AnovaSettings, useAnovaSettings } from '../AnovaSettings';
import type { AnovaDiagnostic, AnovaRunStateView, AnovaSpec } from '../anovaTypes';
import { ANOVA_LEVELS_RULE, anovaRefusalSentence, anovaSentence } from '../anovaWording';
import type { DatasetView } from '../describeTypes';
import { actWith } from '../flowHooks';
import { MethodDialog } from './MethodDialog';
import { ANOVA_DIALOG_TITLE, anovaSettingsLine } from './windowWording';

export type AnovaDialogBridge = Omit<AnovaBridge, 'open' | 'close'>;

export const AnovaDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: AnovaDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: AnovaRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const act = actWith(setBusy, setRefusal, anovaRefusalSentence);
  const settings = useAnovaSettings(view, { bridge, slot: () => slot, act });
  const operation = useRef(0);

  const started = async (spec: AnovaSpec, runId: number) => {
    const first: AnovaRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: anovaSettingsLine(spec.parameters, view, settings.groupLevels), run });
  };

  return (
    <MethodDialog<AnovaSpec, AnovaDiagnostic>
      title={ANOVA_DIALOG_TITLE}
      prefix="an"
      slot={slot}
      ready={settings.ready && !busy}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => anovaSentence(d, settings.names)}
      refusalSentence={anovaRefusalSentence}
      settingsRefusal={refusal}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="an-levels-rule">{ANOVA_LEVELS_RULE}</p>
      <AnovaSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
