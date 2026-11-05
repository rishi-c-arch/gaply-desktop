// Gaply — a method's dialog (Research OS, Phase 8R, decision 469; ADR-012 Part 6):
// one frame and one rule for every method, on the design system's ProjectDialog,
// which keeps the focus inside, closes on Escape and returns the focus.
//
// It opens with nothing chosen and holds the settings while open: the settings
// live in the method's own component, mounted with the dialog, so Cancel and
// Escape drop them and reopening starts empty. Run stays disabled until the
// method's readiness rule holds. Run validates: valid runs at once; a warning
// stays open and shows it, Run says so and a second press runs it; anything else,
// or a refused run, stays open with the sentences and nothing runs. Changing a
// setting takes back what the last check said, so a warning is only ever pressed
// through for the settings it was about.
//
// A dialog closes nothing and owns no file. It gets the slot's number as a prop,
// and the bridge it is given has no open and no close.
import React, { useRef, useState } from 'react';
import { Button } from '../../../design-system';
import { ProjectDialog } from '../../../design-system/ProjectDialog';
import type { RunStartedView, ValidationStatus } from '../describeTypes';
import { STATUS_SENTENCES } from '../describeWording';
import { DiagnosticList } from '../flowParts';
import './MethodDialog.css';
import { ASK_CANCEL, ASK_CONTINUE, DIALOG_CANCEL, OUTPUT_FULL, RUN_WITH_WARNINGS, RUN_WORD } from './windowWording';

interface Report<D> {
  status: ValidationStatus;
  errors: D[];
  warnings: D[];
  missingInformation: D[];
}

export interface MethodDialogProps<S, D> {
  title: string;
  /** The method's test-id prefix. */
  prefix: string;
  slot: number;
  /** The method's own readiness rule: Run stays disabled until it holds. */
  ready: boolean;
  outputFull: boolean;
  /** What these settings are, so a report is shown only for the settings it was about. */
  settingsKey: string;
  buildSpec: () => S;
  bridge: {
    validate(slot: number, spec: S): Promise<Report<D>>;
    run(slot: number, spec: S): Promise<RunStartedView>;
  };
  sentence: (d: D) => string;
  refusalSentence: (e: unknown) => string;
  onStarted: (spec: S, runId: number) => void | Promise<void>;
  onCancel: () => void;
  /** Where the focus goes when it closes: a new entry's heading after a run. */
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
  children: React.ReactNode;
}

export function MethodDialog<S, D>({ title, prefix, slot, ready, outputFull, settingsKey, buildSpec, bridge, sentence, refusalSentence, onStarted, onCancel, returnFocus, children }: MethodDialogProps<S, D>) {
  const [pending, setPending] = useState(false);
  const [checked, setChecked] = useState<{ key: string; report: Report<D> } | null>(null);
  const [refused, setRefused] = useState<{ key: string; sentence: string } | null>(null);
  const inFlight = useRef(false);

  const report = checked !== null && checked.key === settingsKey ? checked.report : null;
  const refusal = refused !== null && refused.key === settingsKey ? refused.sentence : null;
  const warned = report?.status === 'valid_with_warnings';
  const cannotRun = report !== null && report.status !== 'valid' && !warned;

  const press = async () => {
    if (inFlight.current || !ready) return;
    setRefused(null);
    if (outputFull) {
      setRefused({ key: settingsKey, sentence: OUTPUT_FULL });
      return;
    }
    inFlight.current = true;
    setPending(true);
    try {
      const spec = buildSpec();
      if (!warned) {
        const next = await bridge.validate(slot, spec);
        setChecked({ key: settingsKey, report: next });
        if (next.status !== 'valid') return;
      }
      const started = await bridge.run(slot, spec);
      await onStarted(spec, started.runId);
    } catch (e) {
      setRefused({ key: settingsKey, sentence: refusalSentence(e) });
    } finally {
      inFlight.current = false;
      setPending(false);
    }
  };

  const cancel = () => {
    if (!pending) onCancel();
  };

  return (
    <ProjectDialog title={title} onClose={cancel} returnFocus={returnFocus} testId="sw-method-dialog">
      <div className="sw-dialog__scroll" data-testid="sw-dialog-scroll" tabIndex={0}>
        {children}
        {report && (
          <div className="sw-dialog__report">
            <p role="status" data-testid={`${prefix}-validation-status`}>
              {STATUS_SENTENCES[report.status]}
            </p>
            <DiagnosticList items={report.errors} sentence={sentence} testId={`${prefix}-validation-errors`} />
            <DiagnosticList items={report.missingInformation} sentence={sentence} testId={`${prefix}-validation-questions`} />
            <DiagnosticList items={report.warnings} sentence={sentence} testId={`${prefix}-validation-warnings`} />
          </div>
        )}
        {refusal && (
          <p role="alert" data-testid="sw-dialog-refusal">
            {refusal}
          </p>
        )}
      </div>
      <div className="sw-dialog__actions">
        <Button variant="secondary" onClick={cancel} disabled={pending} data-testid={`${prefix}-dialog-cancel`}>
          {DIALOG_CANCEL}
        </Button>
        <Button onClick={() => void press()} disabled={pending || !ready || cannotRun} data-testid={`${prefix}-run`}>
          {warned ? RUN_WITH_WARNINGS : RUN_WORD}
        </Button>
      </div>
    </ProjectDialog>
  );
}

/** A sentence before something is lost, with Continue and Cancel (decision 472). */
export const AskFirst: React.FC<{ title: string; sentence: string; testId: string; onContinue: () => void; onCancel: () => void }> = ({ title, sentence, testId, onContinue, onCancel }) => {
  return (
    <ProjectDialog title={title} onClose={onCancel} testId={testId}>
      <p data-testid={`${testId}-sentence`}>{sentence}</p>
      <div className="sw-dialog__actions">
        <Button variant="secondary" onClick={onCancel} data-testid={`${testId}-cancel`}>
          {ASK_CANCEL}
        </Button>
        <Button onClick={onContinue} data-testid={`${testId}-continue`}>
          {ASK_CONTINUE}
        </Button>
      </div>
    </ProjectDialog>
  );
};
