// Gaply — Analyze, Nonparametric tests, Independent samples… (Research OS, Phase 8Y-2,
// decisions 619 and 620; ADR-012 Part 6): MethodDialog around the rank tests' one copy
// of their settings, on AnovaDialog.tsx's pattern. It owns no file, and its bridge's
// type has no open and no close; it keeps groupLevels, which the settings read.
import React, { useRef, useState } from 'react';
import type { DatasetView } from '../describeTypes';
import { actWith } from '../flowHooks';
import type { RanksBridge } from '../ranksBridge';
import { RanksSettings, useRanksSettings } from '../RanksSettings';
import type { RanksDiagnostic, RanksRunStateView, RanksSpec } from '../ranksTypes';
import { RANKS_LEVELS_RULE, ranksRefusalSentence, ranksSentence } from '../ranksWording';
import { MethodDialog } from './MethodDialog';
import { RANKS_DIALOG_TITLE, ranksSettingsLine } from './windowWording';

export type RanksDialogBridge = Omit<RanksBridge, 'open' | 'close'>;

export const RanksDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: RanksDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: RanksRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const act = actWith(setBusy, setRefusal, ranksRefusalSentence);
  const settings = useRanksSettings(view, { bridge, slot: () => slot, act });
  const operation = useRef(0);

  const started = async (spec: RanksSpec, runId: number) => {
    const first: RanksRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: ranksSettingsLine(spec.parameters, view, settings.groupLevels), run });
  };

  return (
    <MethodDialog<RanksSpec, RanksDiagnostic>
      title={RANKS_DIALOG_TITLE}
      prefix="rk"
      slot={slot}
      ready={settings.ready && !busy}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => ranksSentence(d, settings.names)}
      refusalSentence={ranksRefusalSentence}
      settingsRefusal={refusal}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="rk-levels-rule">{RANKS_LEVELS_RULE}</p>
      <RanksSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
