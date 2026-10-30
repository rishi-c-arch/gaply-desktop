// Gaply — the one-sample t-test (Research OS, Phase 8D-2b, decision 268): an
// analysis of the user's own data, over the one-sample bridge.
//
// Import → Profile → Settings → Validate → Run → Results → Export, as on the
// independent t-test's screen, built from the parts both screens share
// (flowParts.tsx, flowHooks.ts). Rust owns every decision: this screen passes
// the path once (to open), the slot, the run id and the user's choices, and
// shows what comes back.
//
// The choices: the column (a numeric one), the test value and each candidate
// missing code in the column, asked as a question. The test value is a text
// field, empty at first, and Gaply never fills it in: the spec sends the
// number typed when it is a finite number, and null otherwise, which Rust
// answers with test_value_not_chosen.
//
// What lives here, and for how long: the file's base name, the headers, the
// candidate missing-code values, the chosen settings and the result numbers,
// in this component's state only. Never a row, never another cell, never a
// path, and never browser storage. The file is closed, and Rust drops its
// rows, when the user closes it or the screen unmounts.
//
// The wording is neutral (decision 66): the card reports the numbers and
// what they do not assess, and passes no verdict on them.
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../design-system';
import { pickManuscriptPath } from '../common/pickFile';
import { saveTextFile } from '../../utils/saveTextFile';
import { oneSampleBridge, type OneSampleBridge } from './onesampleBridge';
import { SaveToProject } from './projects/SaveToProject';
import {
  DiagnosticList,
  ExportCard,
  FileSummary,
  LimitsLine,
  ManifestPanel,
  MethodPage,
  OpenProgressLine,
  ProfileTable,
  RunCard,
  SettingsActions,
  StageNav,
} from './flowParts';
import {
  actWith,
  fileActions,
  saveExactly,
  useCloseOnUnmount,
  useMethodLimits,
  useOpenProgress,
  useRunPolling,
  useStageFocus,
} from './flowHooks';
import { readMethodLimits } from './statsInvoke';
import type { DatasetView, ExportFormat } from './describeTypes';
import type { OneSampleResultView, OneSampleRunStateView, OneSampleSpec, OneSampleValidationView } from './onesampleTypes';
import { Stage, STATUS_SENTENCES } from './describeWording';
import { shownNumber, TTEST_DIGITS_NOTE } from './ttestWording';
import {
  ONE_SAMPLE_ENTRY_TEXT,
  ONE_SAMPLE_EXPORT_CHOICES,
  ONE_SAMPLE_EXPORT_TEXT,
  ONE_SAMPLE_ROWS_USED_NOTE,
  ONE_SAMPLE_STAGE_HEADINGS,
  oneSampleMissingCodeQuestion,
  oneSampleRefusalSentence,
  oneSampleRunEndedSentence,
  oneSampleSentence,
  TEST_VALUE_LABEL,
  TEST_VALUE_LINE,
  type OneSampleNames,
} from './onesampleWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface OneSampleFlowProps {
  bridge: OneSampleBridge;
  /** The native file picker: an absolute path, or null when cancelled. The
   *  path goes to `open` and is never kept. */
  pickPath?: () => Promise<string | null>;
  /** Writes the export through the save dialog; the chosen path, or null
   *  when the dialog was closed. */
  save?: (name: string, text: string, mime: string) => Promise<string | null>;
  /** How often a run's state is read while it runs. */
  pollMs?: number;
  /** This computer's limits for the methods, read once (Research OS, Phase
   *  8F-2, decision 289); stats_desc_limits by default. */
  limits?: () => Promise<unknown>;
}

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

/** The columns the test can read: numeric ones. */
const numericColumns = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** The test value the spec sends: the number typed when the trimmed text is
 *  not empty and is a finite number; null otherwise. Never a default. */
export function testValueOf(text: string): number | null {
  if (text.trim() === '') return null;
  const v = Number(text);
  return Number.isFinite(v) ? v : null;
}

function buildSpec(view: DatasetView, outcome: string, testValue: number | null, missingCodes: number[], operationId: number): OneSampleSpec {
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.one_sample_t',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { outcome, testValue, missingCodes },
    randomSeed: null,
    confirmationRefs: [],
  };
}

const OneSampleFlow: React.FC<OneSampleFlowProps> = ({
  bridge,
  pickPath = () => pickManuscriptPath(DATA_EXTENSIONS, 'Data file'),
  save = saveTextFile,
  pollMs = 250,
  limits = readMethodLimits,
}) => {
  const methodLimits = useMethodLimits(limits);
  const openProgress = useOpenProgress();
  const [stage, setStage] = useState<Stage>('import');
  const [view, setView] = useState<DatasetView | null>(null);
  const [outcome, setOutcome] = useState('');
  /** The test value as typed; empty at first. */
  const [testValueText, setTestValueText] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);
  const [report, setReport] = useState<OneSampleValidationView | null>(null);
  const [runView, setRunView] = useState<OneSampleRunStateView | null>(null);
  const [result, setResult] = useState<OneSampleResultView | null>(null);
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);
  const slot = useRef<number | null>(null);

  // Focus moves to each stage's heading when the stage changes.
  const heading = useStageFocus(stage);
  // The file is closed, and its rows dropped in Rust, when the screen goes.
  useCloseOnUnmount(bridge, slot);

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: OneSampleNames = useMemo(
    () => ({
      column: (alias: number) => {
        const c = view?.profile.columns.find((col) => aliasNumber(col.alias) === alias);
        return c ? `"${c.header}"` : `C${alias}`;
      },
    }),
    [view]
  );

  const spec = useCallback(() => {
    operation.current += 1;
    return buildSpec(view as DatasetView, outcome, testValueOf(testValueText), missingCodes, operation.current);
  }, [view, outcome, testValueText, missingCodes]);

  // While a run is unfinished, its state is read every `pollMs`.
  useRunPolling(runView, setRunView, bridge, setError, oneSampleRefusalSentence, pollMs);

  const act = actWith(setBusy, setError, oneSampleRefusalSentence);

  const reset = () => {
    setOutcome('');
    setTestValueText('');
    setTicked([]);
    setReport(null);
    setRunView(null);
    setResult(null);
  };

  const { choose, closeFile } = fileActions({
    bridge,
    track: openProgress.track,
    slot,
    pickPath,
    act,
    onOpened: (opened) => {
      setView(opened);
      reset();
      setStage('profile');
    },
    onClosed: () => {
      setView(null);
      reset();
      setStage('import');
    },
  });

  const check = () =>
    act(async () => {
      setReport(await bridge.validate(slot.current as number, spec()));
      setStage('validate');
    });

  const run = () =>
    act(async () => {
      const started = await bridge.run(slot.current as number, spec());
      setResult(null);
      setRunView(await bridge.status(started.runId));
      setStage('run');
    });

  const cancel = () =>
    act(async () => {
      if (runView) setRunView(await bridge.cancel(runView.runId));
    });

  const seeResults = () =>
    act(async () => {
      if (!runView) return;
      setResult(await bridge.result(runView.runId));
      setStage('results');
    });

  const saveExport = () =>
    act(async () => {
      if (!result) return;
      setExportNote(null);
      const choice = ONE_SAMPLE_EXPORT_CHOICES.find((c) => c.format === format) ?? ONE_SAMPLE_EXPORT_CHOICES[0];
      const out = await bridge.export(result.runId, format);
      const saved = await saveExactly(out, choice, save);
      if ('error' in saved) {
        setError(saved.error);
        return;
      }
      setExportNote(saved.note);
    });

  const toggleCode = (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code)));

  const say = (d: { code: string; params?: Record<string, unknown>; count?: number }) => oneSampleSentence(d, names);

  const title = (
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="os-stage-heading">
      {ONE_SAMPLE_STAGE_HEADINGS[stage]}
    </h3>
  );

  // Check the settings needs the column only; the test value is Rust's to
  // ask for (decision 268).
  const ready = outcome !== '';

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="os-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <StageNav prefix="os" stage={stage} />
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="os-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && <FileSummary prefix="os" view={view} />}
      <OpenProgressLine prefix="os" rowsRead={openProgress.rowsRead} />
      {error && (
        <p role="alert" data-testid="os-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="os-entry-text">
            {ONE_SAMPLE_ENTRY_TEXT}
          </p>
          <LimitsLine prefix="os" limits={methodLimits} />
          <Button onClick={() => void choose()} disabled={busy} data-testid="os-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <ProfileTable prefix="os" view={view} />
          <Button onClick={() => setStage('settings')} data-testid="os-to-settings">
            Continue to settings
          </Button>
        </Card>
      )}

      {stage === 'settings' && view && (
        <Card>
          {title}
          <div style={{ display: 'grid', gap: 8 }}>
            <label>
              Column (a numeric column){' '}
              <select value={outcome} data-testid="os-outcome" onChange={(e) => setOutcome(e.target.value)}>
                <option value="">Choose a column…</option>
                {numericColumns(view).map((c) => (
                  <option key={c.alias} value={c.alias}>
                    {c.alias} {c.header}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {TEST_VALUE_LABEL}{' '}
              <input
                type="text"
                inputMode="decimal"
                value={testValueText}
                data-testid="os-test-value"
                aria-describedby="os-test-value-line"
                onChange={(e) => setTestValueText(e.target.value)}
              />
            </label>
            <p id="os-test-value-line" data-testid="os-test-value-line" style={{ margin: 0 }}>
              {TEST_VALUE_LINE}
            </p>
            {outcomeColumn && offered.length > 0 && (
              <fieldset data-testid="os-missing-codes">
                <legend>Candidate missing codes</legend>
                {offered.map((s) => (
                  <label key={s.code} style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={ticked.includes(s.code)}
                      data-testid={`os-code-${s.code}`}
                      onChange={(e) => toggleCode(s.code, e.target.checked)}
                    />{' '}
                    {oneSampleMissingCodeQuestion(s.code, outcomeColumn.header, s.cells)}
                  </label>
                ))}
              </fieldset>
            )}
          </div>
          <SettingsActions prefix="os" busy={busy} ready={ready} onBack={() => setStage('profile')} onCheck={() => void check()} />
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="os-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} sentence={say} testId="os-validation-errors" />
          <DiagnosticList items={report.missingInformation} sentence={say} testId="os-validation-questions" />
          <DiagnosticList items={report.warnings} sentence={say} testId="os-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="os-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="os-run"
            >
              Run the t-test
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <RunCard
          prefix="os"
          title={title}
          runView={runView}
          endedSentence={oneSampleRunEndedSentence}
          diagnostics={<DiagnosticList items={runView.diagnostics} sentence={say} testId="os-run-diagnostics" />}
          busy={busy}
          onCancel={() => void cancel()}
          onSeeResults={() => void seeResults()}
          onBack={() => setStage('settings')}
        />
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <OneSampleResultCard result={result} names={names} />
          <SaveToProject runId={result.runId} testId="os-project-save" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="os-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="os-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <ExportCard
          prefix="os"
          title={title}
          text={ONE_SAMPLE_EXPORT_TEXT}
          choices={ONE_SAMPLE_EXPORT_CHOICES}
          format={format}
          onFormat={setFormat}
          busy={busy}
          onBack={() => setStage('results')}
          onSave={() => void saveExport()}
          note={exportNote}
        />
      )}
    </div>
  );
};

/** The neutral result card (decision 66): the numbers, what is not assessed,
 *  and the manifest. No verdict, no stars. The saved-result view draws a
 *  stored one-sample result with it too. */
export const OneSampleResultCard: React.FC<{ result: OneSampleResultView; names: OneSampleNames }> = ({ result, names }) => {
  const r = result.result;
  const n = shownNumber;
  const say = (d: { code: string; params?: Record<string, unknown>; count?: number }) => oneSampleSentence(d, names);
  return (
    <div data-testid="os-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={say} testId="os-result-notes" />
      <dl data-testid="os-result-numbers" style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
        <dt>n</dt>
        <dd className="gds-mono" data-testid="os-n">
          {r.n}
        </dd>
        <dt>Mean</dt>
        <dd className="gds-mono" data-testid="os-mean">
          {n(r.mean)}
        </dd>
        <dt>SD</dt>
        <dd className="gds-mono" data-testid="os-sd">
          {n(r.sd)}
        </dd>
        <dt>{TEST_VALUE_LABEL}</dt>
        <dd className="gds-mono" data-testid="os-test-value-shown">
          {n(r.testValue)}
        </dd>
        <dt>Mean difference (the mean minus the test value)</dt>
        <dd className="gds-mono" data-testid="os-mean-difference">
          {n(r.meanDifference)}
        </dd>
        <dt>Standard error</dt>
        <dd className="gds-mono" data-testid="os-se">
          {n(r.standardError)}
        </dd>
        <dt>{Math.round(r.confidenceLevel * 100)}% confidence interval</dt>
        <dd className="gds-mono" data-testid="os-ci">
          {n(r.ciLower)} to {n(r.ciUpper)}
        </dd>
        <dt>t</dt>
        <dd className="gds-mono" data-testid="os-t">
          {n(r.t)}
        </dd>
        <dt>Degrees of freedom</dt>
        <dd className="gds-mono" data-testid="os-df">
          {n(r.df)}
        </dd>
        <dt>p (two-sided)</dt>
        <dd className="gds-mono" data-testid="os-p">
          {n(r.p)}
        </dd>
        <dt>Cohen&apos;s d</dt>
        <dd>
          <span className="gds-mono" data-testid="os-d">
            {n(r.cohensD)}
          </span>
          <br />
          <span data-testid="os-d-no-interval">{oneSampleSentence({ code: 'stats.onesample.cohens_d_has_no_interval' })}</span>
        </dd>
      </dl>
      <section aria-label="Rows left out" data-testid="os-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e) => (
              <li key={e.code}>{say(e)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="os-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{oneSampleSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer">{TTEST_DIGITS_NOTE}</p>
      <ManifestPanel prefix="os" manifest={result.manifest} rowsUsedNote={ONE_SAMPLE_ROWS_USED_NOTE} />
    </div>
  );
};

export default OneSampleFlow;

/** /app/statistics/one-sample — the page around the flow. */
export const OneSamplePage: React.FC<{ bridge?: OneSampleBridge }> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? oneSampleBridge(), [bridge]);
  return (
    <MethodPage testId="one-sample-page" header="One-sample t-test" panelTitle="One-sample t-test on a data file">
      <OneSampleFlow bridge={b} />
    </MethodPage>
  );
};
