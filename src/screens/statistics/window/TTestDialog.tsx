// Gaply — Analyze, Compare means, Independent-samples t-test… (Research OS, Phase 8S,
// decision 483; ADR-012 Part 6): MethodDialog around the t-test's one copy of its
// settings. It owns no file, and its bridge's type has no open and no close; it keeps
// groupLevels, which the settings read.
import React, { useRef, useState } from 'react';
import type { DatasetView } from '../describeTypes';
import { actWith } from '../flowHooks';
import type { TTestBridge } from '../ttestBridge';
import { TTestSettings, useTTestSettings } from '../TTestSettings';
import type { TTestDiagnostic, TTestRunStateView, TTestSpec } from '../ttestTypes';
import { TTEST_LEVELS_RULE, ttestRefusalSentence, ttestSentence } from '../ttestWording';
import { MethodDialog } from './MethodDialog';
import { TTEST_DIALOG_TITLE, ttestSettingsLine } from './windowWording';

export type TTestDialogBridge = Omit<TTestBridge, 'open' | 'close'>;

export const TTestDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: TTestDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: TTestRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const act = actWith(setBusy, setRefusal, ttestRefusalSentence);
  const settings = useTTestSettings(view, { bridge, slot: () => slot, act });
  const operation = useRef(0);

  const started = async (spec: TTestSpec, runId: number) => {
    const first: TTestRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: ttestSettingsLine(spec.parameters, view, settings.groupLevels), run });
  };

  return (
    <MethodDialog<TTestSpec, TTestDiagnostic>
      title={TTEST_DIALOG_TITLE}
      prefix="tt"
      slot={slot}
      ready={settings.ready && !busy}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => ttestSentence(d, settings.names)}
      refusalSentence={ttestRefusalSentence}
      settingsRefusal={refusal}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="tt-levels-rule">{TTEST_LEVELS_RULE}</p>
      <TTestSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
