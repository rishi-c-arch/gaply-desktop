// Gaply — Analyze, Descriptive statistics, Summary… (Research OS, Phase 8S, decision
// 482; ADR-012 Part 6): MethodDialog around the summary's one copy of its settings,
// opening with no column included. It owns no file, and its bridge's type has no open
// and no close.
import React, { useMemo, useRef } from 'react';
import type { DescribeBridge } from '../describeBridge';
import { DescribeSettings, noPicks, useDescribeSettings } from '../DescribeSettings';
import type { DatasetView, Diagnostic, OperationSpec, RunStateView } from '../describeTypes';
import { diagnosticSentence, LEVELS_RULE, refusalSentence } from '../describeWording';
import { MethodDialog } from './MethodDialog';
import { columnNames, DESCRIBE_DIALOG_TITLE, DESCRIBE_NOT_READY, describeSettingsLine } from './windowWording';

export type DescribeDialogBridge = Omit<DescribeBridge, 'open' | 'close'>;

export const DescribeDialog: React.FC<{
  view: DatasetView;
  slot: number;
  bridge: DescribeDialogBridge;
  outputFull: boolean;
  onStarted: (started: { runId: number; settings: string; run: RunStateView }) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}> = ({ view, slot, bridge, outputFull, onStarted, onCancel, returnFocus }) => {
  const settings = useDescribeSettings(view, noPicks);
  const operation = useRef(0);
  const name = useMemo(() => columnNames(view), [view]);

  const started = async (spec: OperationSpec, runId: number) => {
    const first: RunStateView = { runId, state: 'queued', terminal: false, columnsDone: 0, columnsTotal: spec.parameters.columns.length, elapsedMs: 0, cancelRequested: false, diagnostics: [] };
    const run = await bridge.status(runId).catch(() => first);
    onStarted({ runId, settings: describeSettingsLine(spec.parameters, view), run });
  };

  return (
    <MethodDialog<OperationSpec, Diagnostic>
      title={DESCRIBE_DIALOG_TITLE}
      prefix="desc"
      slot={slot}
      ready={settings.anyIncluded}
      outputFull={outputFull}
      settingsKey={JSON.stringify(settings.spec(0).parameters)}
      buildSpec={() => {
        operation.current += 1;
        return settings.spec(operation.current);
      }}
      bridge={bridge}
      sentence={(d) => diagnosticSentence(d, name)}
      refusalSentence={refusalSentence}
      onStarted={started}
      onCancel={onCancel}
      returnFocus={returnFocus}
    >
      <p data-testid="desc-levels-rule">{LEVELS_RULE}</p>
      <DescribeSettings view={view} settings={settings} />
      {!settings.anyIncluded && (
        <p role="status" data-testid="desc-settings-missing">
          {DESCRIBE_NOT_READY}
        </p>
      )}
    </MethodDialog>
  );
};
