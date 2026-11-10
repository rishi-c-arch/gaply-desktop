// Gaply — Analyze, Nonparametric tests, Related samples… (Research OS, Phase 8Z-2, decisions
// 653 and 654; ADR-012 Part 6): MethodDialog around the related rank tests' settings, on
// AlphaDialog.tsx's pattern with RanksDialog.tsx's bridge type. It owns no file, and its
// bridge's type has no open and no close.
import React, { useRef } from 'react';
import type { DatasetView } from '../describeTypes';
import type { RelatedRanksBridge } from '../relatedRanksBridge';
import { RelatedRanksSettings, useRelatedRanksSettings } from '../RelatedRanksSettings';
import type { RelatedRanksDiagnostic, RelatedRanksRunStateView, RelatedRanksSpec } from '../relatedRanksTypes';
import { RELATED_RANKS_RULE, relatedRanksRefusalSentence, relatedRanksSentence } from '../relatedRanksWording';
import { MethodDialog } from './MethodDialog';
import { RELATED_RANKS_DIALOG_TITLE, relatedRanksSettingsLine } from './windowWording';

export type RelatedRanksDialogBridge = Omit<RelatedRanksBridge, 'open' | 'close'>;

export const RelatedRanksDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: RelatedRanksDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: RelatedRanksRunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useRelatedRanksSettings(view);
  const operation = useRef(0);

  const started = async (spec: RelatedRanksSpec, runId: number) => {
    const first: RelatedRanksRunStateView = { runId, state: 'queued', terminal: false, elapsedMs: 0, cancelRequested: false, stepsDone: 0, stepsTotal: 0, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: relatedRanksSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<RelatedRanksSpec, RelatedRanksDiagnostic>
      title={RELATED_RANKS_DIALOG_TITLE}
      prefix="rr"
      slot={slot}
      ready={settings.stillMissing === null}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => relatedRanksSentence(d, settings.names)}
      refusalSentence={relatedRanksRefusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="rr-rule">{RELATED_RANKS_RULE}</p>
      <RelatedRanksSettings view={view} settings={settings} />
    </MethodDialog>
  );
};
