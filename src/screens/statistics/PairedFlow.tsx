// Gaply — the paired-samples t-test (Research OS, Phase 8D-2c, decision 271):
// an analysis of the user's own data, over the paired bridge.
//
// Import → Profile → Settings → Validate → Run → Results → Export, as on the
// one-sample t-test's screen, built from the parts the screens share
// (flowParts.tsx, flowHooks.ts). Rust owns every decision: this screen passes
// the path once (to open), the slot, the run id and the user's choices, and
// shows what comes back.
//
// The choices: a first and a second column, each from the numeric columns,
// with no default; the difference is first minus second. The second list
// keeps the first's choice: the same column twice is sent, and Rust refuses
// it with same_column. Each candidate missing code of either column is asked
// once, with both columns' counts; a ticked code leaves out every pair in
// which either column holds it, as Rust does.
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
import { pairedBridge, type PairedBridge } from './onesampleBridge';
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
import type { OneSampleRunStateView, OneSampleValidationView, PairedResultView, PairedSpec } from './onesampleTypes';
import { Stage, STATUS_SENTENCES } from './describeWording';
import { shownNumber, TTEST_DIGITS_NOTE } from './ttestWording';
import {
  oneSampleRefusalSentence,
  oneSampleRunEndedSentence,
  oneSampleSentence,
  PAIRED_DIFFERENCE_LINE,
  PAIRED_ENTRY_TEXT,
  PAIRED_EXPORT_CHOICES,
  PAIRED_EXPORT_TEXT,
  PAIRED_ROWS_USED_NOTE,
  PAIRED_STAGE_HEADINGS,
  pairedMissingCodeQuestion,
  type OneSampleNames,
} from './onesampleWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface PairedFlowProps {
  bridge: PairedBridge;
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

/** One candidate missing code of either chosen column, with each column's
 *  count (0 where the code is absent). */
interface OfferedCode {
  code: number;
  first: number;
  second: number;
}

function buildSpec(view: DatasetView, first: string, second: string, missingCodes: number[], operationId: number): PairedSpec {
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.paired_t',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: { first, second, missingCodes },
    randomSeed: null,
    confirmationRefs: [],
  };
}

const PairedFlow: React.FC<PairedFlowProps> = ({
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
  const [first, setFirst] = useState('');
  const [second, setSecond] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);
  const [report, setReport] = useState<OneSampleValidationView | null>(null);
  const [runView, setRunView] = useState<OneSampleRunStateView | null>(null);
  const [result, setResult] = useState<PairedResultView | null>(null);
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

  const firstColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === first), [view, first]);
  const secondColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === second), [view, second]);

  /** Each code of the first column, then each of the second's not already
   *  offered, once, with both columns' counts. */
  const offered = useMemo<OfferedCode[]>(() => {
    if (!firstColumn || !secondColumn) return [];
    const cellsIn = (codes: Array<{ code: number; cells: number }>, code: number) => codes.find((s) => s.code === code)?.cells ?? 0;
    const out: OfferedCode[] = [];
    for (const s of [...firstColumn.suspectMissingCodes, ...secondColumn.suspectMissingCodes]) {
      if (out.some((o) => o.code === s.code)) continue;
      out.push({ code: s.code, first: cellsIn(firstColumn.suspectMissingCodes, s.code), second: cellsIn(secondColumn.suspectMissingCodes, s.code) });
    }
    return out;
  }, [firstColumn, secondColumn]);
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
    return buildSpec(view as DatasetView, first, second, missingCodes, operation.current);
  }, [view, first, second, missingCodes]);

  // While a run is unfinished, its state is read every `pollMs`.
  useRunPolling(runView, setRunView, bridge, setError, oneSampleRefusalSentence, pollMs);

  const act = actWith(setBusy, setError, oneSampleRefusalSentence);

  const reset = () => {
    setFirst('');
    setSecond('');
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
      const choice = PAIRED_EXPORT_CHOICES.find((c) => c.format === format) ?? PAIRED_EXPORT_CHOICES[0];
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
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="pt-stage-heading">
      {PAIRED_STAGE_HEADINGS[stage]}
    </h3>
  );

  // Check the settings needs both columns; the same one twice is Rust's to
  // refuse (decision 271).
  const ready = first !== '' && second !== '';

  const picker = (which: 'first' | 'second', label: string, value: string, set: (v: string) => void) => (
    <label>
      {label}{' '}
      <select value={value} data-testid={`pt-${which}`} aria-describedby="pt-difference-line" onChange={(e) => set(e.target.value)}>
        <option value="">Choose a column…</option>
        {numericColumns(view as DatasetView).map((c) => (
          <option key={c.alias} value={c.alias}>
            {c.alias} {c.header}
          </option>
        ))}
      </select>
    </label>
  );

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="pt-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <StageNav prefix="pt" stage={stage} />
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="pt-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && <FileSummary prefix="pt" view={view} />}
      <OpenProgressLine prefix="pt" rowsRead={openProgress.rowsRead} />
      {error && (
        <p role="alert" data-testid="pt-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="pt-entry-text">
            {PAIRED_ENTRY_TEXT}
          </p>
          <LimitsLine prefix="pt" limits={methodLimits} />
          <Button onClick={() => void choose()} disabled={busy} data-testid="pt-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <ProfileTable prefix="pt" view={view} />
          <Button onClick={() => setStage('settings')} data-testid="pt-to-settings">
            Continue to settings
          </Button>
        </Card>
      )}

      {stage === 'settings' && view && (
        <Card>
          {title}
          <div style={{ display: 'grid', gap: 8 }}>
            {picker('first', 'First column (a numeric column)', first, setFirst)}
            {picker('second', 'Second column (a numeric column)', second, setSecond)}
            <p id="pt-difference-line" data-testid="pt-difference-line" style={{ margin: 0 }}>
              {PAIRED_DIFFERENCE_LINE}
            </p>
            {firstColumn && secondColumn && offered.length > 0 && (
              <fieldset data-testid="pt-missing-codes">
                <legend>Candidate missing codes</legend>
                {offered.map((s) => (
                  <label key={s.code} style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={ticked.includes(s.code)}
                      data-testid={`pt-code-${s.code}`}
                      onChange={(e) => toggleCode(s.code, e.target.checked)}
                    />{' '}
                    {pairedMissingCodeQuestion(
                      s.code,
                      { header: firstColumn.header, cells: s.first },
                      { header: secondColumn.header, cells: s.second }
                    )}
                  </label>
                ))}
              </fieldset>
            )}
          </div>
          <SettingsActions prefix="pt" busy={busy} ready={ready} onBack={() => setStage('profile')} onCheck={() => void check()} />
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="pt-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} sentence={say} testId="pt-validation-errors" />
          <DiagnosticList items={report.missingInformation} sentence={say} testId="pt-validation-questions" />
          <DiagnosticList items={report.warnings} sentence={say} testId="pt-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="pt-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="pt-run"
            >
              Run the t-test
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <RunCard
          prefix="pt"
          title={title}
          runView={runView}
          endedSentence={oneSampleRunEndedSentence}
          diagnostics={<DiagnosticList items={runView.diagnostics} sentence={say} testId="pt-run-diagnostics" />}
          busy={busy}
          onCancel={() => void cancel()}
          onSeeResults={() => void seeResults()}
          onBack={() => setStage('settings')}
        />
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <PairedResultCard result={result} names={names} />
          <SaveToProject runId={result.runId} testId="pt-project-save" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="pt-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="pt-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <ExportCard
          prefix="pt"
          title={title}
          text={PAIRED_EXPORT_TEXT}
          choices={PAIRED_EXPORT_CHOICES}
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
 *  and the manifest. No verdict, no stars. Its rows name the columns by role,
 *  never by header, so the saved-result view, which draws a stored paired
 *  result with it too, reads as the live one. */
export const PairedResultCard: React.FC<{ result: PairedResultView; names: OneSampleNames }> = ({ result, names }) => {
  const r = result.result;
  const n = shownNumber;
  const say = (d: { code: string; params?: Record<string, unknown>; count?: number }) => oneSampleSentence(d, names);
  return (
    <div data-testid="pt-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={say} testId="pt-result-notes" />
      <dl data-testid="pt-result-numbers" style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
        <dt>Pairs</dt>
        <dd className="gds-mono" data-testid="pt-n-pairs">
          {r.nPairs}
        </dd>
        <dt>First column: mean</dt>
        <dd className="gds-mono" data-testid="pt-first-mean">
          {n(r.first.mean)}
        </dd>
        <dt>First column: SD</dt>
        <dd className="gds-mono" data-testid="pt-first-sd">
          {n(r.first.sd)}
        </dd>
        <dt>Second column: mean</dt>
        <dd className="gds-mono" data-testid="pt-second-mean">
          {n(r.second.mean)}
        </dd>
        <dt>Second column: SD</dt>
        <dd className="gds-mono" data-testid="pt-second-sd">
          {n(r.second.sd)}
        </dd>
        <dt>Mean difference (first minus second)</dt>
        <dd className="gds-mono" data-testid="pt-mean-difference">
          {n(r.meanDifference)}
        </dd>
        <dt>SD of the differences</dt>
        <dd className="gds-mono" data-testid="pt-sd-differences">
          {n(r.sdDifferences)}
        </dd>
        <dt>Standard error</dt>
        <dd className="gds-mono" data-testid="pt-se">
          {n(r.standardError)}
        </dd>
        <dt>{Math.round(r.confidenceLevel * 100)}% confidence interval</dt>
        <dd className="gds-mono" data-testid="pt-ci">
          {n(r.ciLower)} to {n(r.ciUpper)}
        </dd>
        <dt>t</dt>
        <dd className="gds-mono" data-testid="pt-t">
          {n(r.t)}
        </dd>
        <dt>Degrees of freedom</dt>
        <dd className="gds-mono" data-testid="pt-df">
          {n(r.df)}
        </dd>
        <dt>p (two-sided)</dt>
        <dd className="gds-mono" data-testid="pt-p">
          {n(r.p)}
        </dd>
        <dt>Cohen&apos;s d_z</dt>
        <dd>
          <span className="gds-mono" data-testid="pt-dz">
            {n(r.cohensDz)}
          </span>
          <br />
          <span data-testid="pt-d-no-interval">{oneSampleSentence({ code: 'stats.onesample.cohens_d_has_no_interval' })}</span>
        </dd>
      </dl>
      <section aria-label="Rows left out" data-testid="pt-exclusions">
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
      <section aria-label="What this does not assess" data-testid="pt-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{oneSampleSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer">{TTEST_DIGITS_NOTE}</p>
      <ManifestPanel prefix="pt" manifest={result.manifest} rowsUsedNote={PAIRED_ROWS_USED_NOTE} />
    </div>
  );
};

export default PairedFlow;

/** /app/statistics/paired — the page around the flow. */
export const PairedPage: React.FC<{ bridge?: PairedBridge }> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? pairedBridge(), [bridge]);
  return (
    <MethodPage testId="paired-page" header="Paired-samples t-test" panelTitle="Paired-samples t-test on a data file">
      <PairedFlow bridge={b} />
    </MethodPage>
  );
};
