// Gaply — the descriptive summary (Research OS, Phase 2B): the first screen
// of the Statistics workspace's operation contract.
//
// Import → Profile → Settings → Validate → Run → Results → Export, over the
// describe bridge. Rust owns every decision: this screen passes the path once
// (to open), the slot, the run id and the user's choices, and shows what
// comes back.
//
// What lives here, and for how long: the file's base name, the headers, the
// level labels, the candidate missing-code values and the summary numbers,
// in this component's state only. Never a row, never another cell, never a
// path, and never browser storage. The file is closed, and Rust drops its
// rows, when the user closes it or the screen unmounts.
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { pickManuscriptPath } from '../common/pickFile';
import { saveTextFile } from '../../utils/saveTextFile';
import { describeBridge, type DescribeBridge } from './describeBridge';
import { savableOnScreen, SaveToProject } from './projects/SaveToProject';
import type {
  ColumnProfile,
  ColumnResult,
  DatasetView,
  Diagnostic,
  ExportFormat,
  OperationSpec,
  ResultBundleView,
  RunStateView,
  Stat,
  TreatAs,
  ValidationReportView,
} from './describeTypes';
import {
  COULD_NOT_COMPUTE,
  diagnosticSentence,
  computeTimeSentence,
  elapsedSentence,
  ENTRY_TEXT,
  EXPORT_CHOICES,
  EXPORT_TEXT,
  KIND_NAMES,
  LEVELS_RULE,
  missingCodeQuestion,
  notComputableReason,
  platformName,
  progressSentence,
  refusalSentence,
  reproducibilitySentence,
  ROWS_USED_NOTE,
  RUN_STATE_NAMES,
  runEndedSentence,
  shortHash,
  shown,
  SHOWN_DIGITS_NOTE,
  Stage,
  STAGE_HEADINGS,
  STAGES,
  STATUS_SENTENCES,
} from './describeWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface DescribeFlowProps {
  bridge: DescribeBridge;
  /** The native file picker: an absolute path, or null when cancelled. The
   *  path goes to `open` and is never kept. */
  pickPath?: () => Promise<string | null>;
  /** Writes the export through the save dialog; the chosen path, or null
   *  when the dialog was closed. */
  save?: (name: string, text: string, mime: string) => Promise<string | null>;
  /** How often a run's state is read while it runs. */
  pollMs?: number;
}

interface Pick {
  include: boolean;
  treatAs: TreatAs;
}

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

function defaultPicks(view: DatasetView): Record<string, Pick> {
  const picks: Record<string, Pick> = {};
  for (const c of view.profile.columns) {
    picks[c.alias] = { include: true, treatAs: c.kind === 'categorical' ? 'categorical' : 'numeric' };
  }
  return picks;
}

/** The candidate missing codes of the columns chosen as numeric, each with
 *  the columns that hold it. */
function offeredCodes(view: DatasetView, picks: Record<string, Pick>): Array<[number, Array<{ header: string; cells: number }>]> {
  const byCode = new Map<number, Array<{ header: string; cells: number }>>();
  for (const c of view.profile.columns) {
    const p = picks[c.alias];
    if (!p?.include || p.treatAs !== 'numeric') continue;
    for (const s of c.suspectMissingCodes) {
      byCode.set(s.code, [...(byCode.get(s.code) ?? []), { header: c.header, cells: s.cells }]);
    }
  }
  return Array.from(byCode.entries()).sort((a, b) => a[0] - b[0]);
}

function buildSpec(view: DatasetView, picks: Record<string, Pick>, missingCodes: number[], operationId: number): OperationSpec {
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.describe',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      columns: view.profile.columns.filter((c) => picks[c.alias]?.include).map((c) => ({ alias: c.alias, treatAs: picks[c.alias].treatAs })),
      missingCodes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

/** A statistic's cell: its value, or "Could not compute" and why, styled
 *  like any other cell. */
const StatCell: React.FC<{ stat: Stat }> = ({ stat }) => {
  if ('value' in stat) return <td className="gds-mono">{shown(stat.value)}</td>;
  return (
    <td data-tone="neutral" data-testid="desc-not-computable">
      {COULD_NOT_COMPUTE}
      <br />
      <small>{notComputableReason(stat.not_computable)}</small>
    </td>
  );
};

const DiagnosticList: React.FC<{ items: Diagnostic[]; name: (a: number) => string; testId: string }> = ({ items, name, testId }) =>
  items.length === 0 ? null : (
    <ul data-testid={testId} style={{ margin: 0 }}>
      {items.map((d, i) => (
        <li key={i}>{diagnosticSentence(d, name)}</li>
      ))}
    </ul>
  );

/** The summary's tables: one for the numeric columns, one per categorical
 *  column. */
function resultTable(columns: ColumnResult[]) {
  const numeric = columns.flatMap((c) => ('numeric' in c ? [c.numeric] : []));
  const categorical = columns.flatMap((c) => ('categorical' in c ? [c.categorical] : []));
  return (
    <>
      {numeric.length > 0 && (
        <table data-testid="desc-numeric-table" style={{ borderCollapse: 'collapse' }}>
          <caption style={{ textAlign: 'left' }}>Numeric columns</caption>
          <thead>
            <tr>
              {['Column', 'n', 'Missing', 'Mean', 'SD', 'Min', 'Q1', 'Median', 'Q3', 'Max'].map((h) => (
                <th key={h} scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {numeric.map((s) => (
              <tr key={s.alias}>
                <th scope="row" style={{ textAlign: 'left' }}>
                  {s.header}
                </th>
                <td className="gds-mono">{s.n}</td>
                <td className="gds-mono">{s.missing}</td>
                {[s.mean, s.sd, s.min, s.q1, s.median, s.q3, s.max].map((st, i) => (
                  <StatCell key={i} stat={st} />
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {categorical.map((s) => (
        <table key={s.alias} data-testid="desc-levels-table" style={{ borderCollapse: 'collapse' }}>
          <caption style={{ textAlign: 'left' }}>
            {s.header}: n {s.n}, missing {s.missing}
          </caption>
          <thead>
            <tr>
              <th scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                Level
              </th>
              <th scope="col" style={{ textAlign: 'left' }}>
                Count
              </th>
            </tr>
          </thead>
          <tbody>
            {s.levels.map((l) => (
              <tr key={l.label}>
                <td>{l.label}</td>
                <td className="gds-mono">{l.count}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ))}
    </>
  );
}

/** The summary's result: its notes, the tables, the digits note and the run
 *  manifest. The live results screen renders it, and so does a saved result
 *  (Research OS, Phase 5B-3b, decision 150), from the same props. */
export const DescribeResult: React.FC<{ result: ResultBundleView; name: (alias: number) => string }> = ({ result, name }) => (
  <>
    <DiagnosticList items={result.diagnostics} name={name} testId="desc-result-notes" />
    {resultTable(result.columns)}
    <p className="gds-jc__disclaimer">{SHOWN_DIGITS_NOTE}</p>
    <aside aria-label="Run manifest" data-testid="desc-manifest">
      <h4 style={{ margin: '8px 0 4px' }}>Run manifest</h4>
      <dl style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
        <dt>Data file (SHA-256)</dt>
        <dd className="gds-mono" data-testid="desc-manifest-input">
          {result.manifest.inputs.map(shortHash).join(', ')}
        </dd>
        <dt>Capability</dt>
        <dd data-testid="desc-manifest-capability">
          {result.manifest.capabilityId}, version {result.manifest.capabilityVersion}
        </dd>
        <dt>Engine version</dt>
        <dd data-testid="desc-manifest-engine">{result.manifest.engineVersion}</dd>
        <dt>Rows read</dt>
        <dd className="gds-mono" data-testid="desc-manifest-rows-read">
          {result.manifest.rowsRead}
        </dd>
        <dt>Rows used</dt>
        <dd>
          <span className="gds-mono" data-testid="desc-manifest-rows-used">
            {result.manifest.rowsUsed}
          </span>
          <br />
          <span data-testid="desc-manifest-rows-used-note">{ROWS_USED_NOTE}</span>
        </dd>
        <dt>Time to compute</dt>
        <dd data-testid="desc-manifest-compute-time">{computeTimeSentence(result.manifest.elapsedMs)}</dd>
        <dt>Platform</dt>
        <dd data-testid="desc-manifest-platform">{platformName(result.manifest.platform)}</dd>
        <dt>Reproducibility</dt>
        <dd data-testid="desc-manifest-reproducibility">{reproducibilitySentence(result.manifest.reproducibility)}</dd>
        <dt>Result (SHA-256)</dt>
        <dd className="gds-mono" data-testid="desc-manifest-output">
          {shortHash(result.manifest.outputHash)}
        </dd>
      </dl>
    </aside>
  </>
);

const DescribeFlow: React.FC<DescribeFlowProps> = ({
  bridge,
  pickPath = () => pickManuscriptPath(DATA_EXTENSIONS, 'Data file'),
  save = saveTextFile,
  pollMs = 250,
}) => {
  const [stage, setStage] = useState<Stage>('import');
  const [view, setView] = useState<DatasetView | null>(null);
  const [picks, setPicks] = useState<Record<string, Pick>>({});
  const [ticked, setTicked] = useState<number[]>([]);
  const [report, setReport] = useState<ValidationReportView | null>(null);
  const [runView, setRunView] = useState<RunStateView | null>(null);
  const [result, setResult] = useState<ResultBundleView | null>(null);
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);
  const slot = useRef<number | null>(null);

  // Focus moves to each stage's heading when the stage changes.
  const heading = useRef<HTMLHeadingElement>(null);
  const shownStage = useRef<Stage>(stage);
  useEffect(() => {
    if (shownStage.current === stage) return;
    shownStage.current = stage;
    heading.current?.focus();
  }, [stage]);

  // The file is closed, and its rows dropped in Rust, when the screen goes.
  useEffect(
    () => () => {
      if (slot.current !== null) void bridge.close(slot.current).catch(() => undefined);
    },
    [bridge]
  );

  const name = useCallback(
    (alias: number) => {
      const c = view?.profile.columns.find((col) => aliasNumber(col.alias) === alias);
      return c ? `"${c.header}"` : `C${alias}`;
    },
    [view]
  );
  const offered = useMemo(() => (view ? offeredCodes(view, picks) : []), [view, picks]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some(([code]) => code === t)), [ticked, offered]);
  const spec = useCallback(() => {
    operation.current += 1;
    return buildSpec(view as DatasetView, picks, missingCodes, operation.current);
  }, [view, picks, missingCodes]);

  // While a run is unfinished, its state is read every `pollMs`.
  useEffect(() => {
    if (!runView || runView.terminal) return;
    let live = true;
    const t = setTimeout(() => {
      bridge
        .status(runView.runId)
        .then((s) => live && setRunView(s))
        .catch((e) => live && setError(refusalSentence(e)));
    }, pollMs);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [runView, bridge, pollMs]);

  const act = async (f: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(refusalSentence(e));
    } finally {
      setBusy(false);
    }
  };

  const choose = () =>
    act(async () => {
      const path = await pickPath();
      if (!path) return;
      const opened = await bridge.open(path);
      if (slot.current !== null) void bridge.close(slot.current).catch(() => undefined);
      slot.current = opened.slot;
      setView(opened);
      setPicks(defaultPicks(opened));
      setTicked([]);
      setReport(null);
      setRunView(null);
      setResult(null);
      setStage('profile');
    });

  const closeFile = () =>
    act(async () => {
      if (slot.current !== null) await bridge.close(slot.current);
      slot.current = null;
      setView(null);
      setReport(null);
      setRunView(null);
      setResult(null);
      setStage('import');
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
      const choice = EXPORT_CHOICES.find((c) => c.format === format) ?? EXPORT_CHOICES[0];
      const out = await bridge.export(result.runId, format);
      const bytes = Uint8Array.from(out.bytes);
      const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
      // What is written must be the bytes Rust returned, and their hash.
      const again = new TextEncoder().encode(text);
      if (again.length !== bytes.length || again.some((b, i) => b !== bytes[i])) {
        setError('The export could not be saved exactly as Gaply made it, so it was not saved.');
        return;
      }
      const where = await save(choice.file, text, choice.mime);
      setExportNote(where ? `Saved. SHA-256 of the file: ${shortHash(out.sha256)}` : 'Not saved: the save dialog was closed.');
    });

  const toggleCode = (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code)));

  const stageNav = (
    <nav aria-label="Steps" data-testid="desc-steps" style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
      {STAGES.map((s) => (
        <span key={s.id} aria-current={s.id === stage ? 'step' : undefined} style={{ fontWeight: s.id === stage ? 700 : 400 }}>
          {s.label}
        </span>
      ))}
    </nav>
  );

  const title = (
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="desc-stage-heading">
      {STAGE_HEADINGS[stage]}
    </h3>
  );

  const profileRow = (c: ColumnProfile) => {
    const levels = c.levels ?? [];
    return (
      <tr key={c.alias}>
        <td>
          <span className="gds-mono">{c.alias}</span> {c.header}
        </td>
        <td>{KIND_NAMES[c.kind]}</td>
        <td className="gds-mono">{c.n}</td>
        <td className="gds-mono">{c.missing}</td>
        <td className="gds-mono">{c.distinct}</td>
        <td>
          {levels.slice(0, 12).map((l) => `${l.label} (${l.count})`).join(', ')}
          {levels.length > 12 ? `, and ${levels.length - 12} more` : ''}
        </td>
        <td className="gds-mono">{c.suspectMissingCodes.map((s) => s.code).join(', ')}</td>
      </tr>
    );
  };


  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="desc-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        {stageNav}
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="desc-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && (
        <p data-testid="desc-file-summary" style={{ margin: 0 }}>
          <span className="gds-mono">{view.displayName}</span>: {view.profile.rows} rows, {view.profile.columns.length}{' '}
          {view.profile.columns.length === 1 ? 'column' : 'columns'}.
        </p>
      )}
      {error && (
        <p role="alert" data-testid="desc-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="desc-entry-text">
            {ENTRY_TEXT}
          </p>
          <Button onClick={() => void choose()} disabled={busy} data-testid="desc-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <p data-testid="desc-levels-rule">{LEVELS_RULE}</p>
          <table data-testid="desc-profile" style={{ borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                {['Column', 'Kind', 'n', 'Missing', 'Distinct', 'Levels', 'Candidate missing codes'].map((h) => (
                  <th key={h} scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>{view.profile.columns.map(profileRow)}</tbody>
          </table>
          <Button onClick={() => setStage('settings')} data-testid="desc-to-settings">
            Continue to settings
          </Button>
        </Card>
      )}

      {stage === 'settings' && view && (
        <Card>
          {title}
          <table data-testid="desc-settings" style={{ borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                <th scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                  Include
                </th>
                <th scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                  Column
                </th>
                <th scope="col" style={{ textAlign: 'left' }}>
                  Summarise as
                </th>
              </tr>
            </thead>
            <tbody>
              {view.profile.columns.map((c) => {
                const p = picks[c.alias];
                return (
                  <tr key={c.alias}>
                    <td>
                      <input
                        type="checkbox"
                        checked={p?.include ?? false}
                        aria-label={`Include ${c.header}`}
                        data-testid={`desc-include-${c.alias}`}
                        onChange={(e) => setPicks({ ...picks, [c.alias]: { ...p, include: e.target.checked } })}
                      />
                    </td>
                    <td>
                      <span className="gds-mono">{c.alias}</span> {c.header} <small>({KIND_NAMES[c.kind]})</small>
                    </td>
                    <td>
                      <select
                        value={p?.treatAs}
                        aria-label={`Summarise ${c.header} as`}
                        data-testid={`desc-kind-${c.alias}`}
                        disabled={!p?.include}
                        onChange={(e) => setPicks({ ...picks, [c.alias]: { ...p, treatAs: e.target.value as TreatAs } })}
                      >
                        <option value="numeric">Numeric</option>
                        <option value="categorical">Categorical</option>
                      </select>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {offered.length > 0 && (
            <fieldset data-testid="desc-missing-codes">
              <legend>Candidate missing codes</legend>
              {offered.map(([code, where]) => (
                <label key={code} style={{ display: 'block' }}>
                  <input
                    type="checkbox"
                    checked={ticked.includes(code)}
                    data-testid={`desc-code-${code}`}
                    onChange={(e) => toggleCode(code, e.target.checked)}
                  />{' '}
                  {missingCodeQuestion(code, where)}
                </label>
              ))}
            </fieldset>
          )}
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('profile')} data-testid="desc-back-profile">
              Back to the profile
            </Button>
            <Button onClick={() => void check()} disabled={busy} data-testid="desc-check">
              Check the settings
            </Button>
          </div>
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="desc-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} name={name} testId="desc-validation-errors" />
          <DiagnosticList items={report.missingInformation} name={name} testId="desc-validation-questions" />
          <DiagnosticList items={report.warnings} name={name} testId="desc-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="desc-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="desc-run"
            >
              Run the summary
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <Card>
          {title}
          <div role="status" aria-live="polite" data-testid="desc-run-status">
            <p style={{ margin: 0 }} data-testid="desc-run-state">
              {RUN_STATE_NAMES[runView.state]}
            </p>
            <p style={{ margin: 0 }} data-testid="desc-progress">
              {progressSentence(runView.columnsDone, runView.columnsTotal)}
            </p>
            <progress value={runView.columnsDone} max={Math.max(runView.columnsTotal, 1)} aria-label="Columns summarised" />
            <p style={{ margin: 0 }} data-testid="desc-elapsed">
              {elapsedSentence(runView.elapsedMs)}
            </p>
          </div>
          {runView.terminal && runView.state !== 'succeeded' && (
            <>
              <p data-testid="desc-run-ended">{runEndedSentence(runView.state)}</p>
              <DiagnosticList items={runView.diagnostics} name={name} testId="desc-run-diagnostics" />
              {/* A failed run is saved where its failure is shown (decisions 136, 140). */}
              {runView.terminal && savableOnScreen(runView) && <SaveToProject runId={runView.runId} testId="desc-project-save" />}
            </>
          )}
          <div style={{ display: 'flex', gap: 8 }}>
            {!runView.terminal && (
              <Button variant="secondary" onClick={() => void cancel()} disabled={runView.cancelRequested} data-testid="desc-cancel">
                {runView.cancelRequested ? 'Cancelling…' : 'Cancel'}
              </Button>
            )}
            {runView.terminal && runView.state === 'succeeded' && (
              <Button onClick={() => void seeResults()} disabled={busy} data-testid="desc-see-results">
                See the results
              </Button>
            )}
            {runView.terminal && (
              <Button variant="secondary" onClick={() => setStage('settings')} data-testid="desc-run-back">
                Back to settings
              </Button>
            )}
          </div>
        </Card>
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <DescribeResult result={result} name={name} />
          <SaveToProject runId={result.runId} testId="desc-project-save" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="desc-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="desc-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer">{EXPORT_TEXT}</p>
          <fieldset data-testid="desc-export-choice">
            <legend>What to save</legend>
            {EXPORT_CHOICES.map((c) => (
              <label key={c.format} style={{ display: 'block' }}>
                <input
                  type="radio"
                  name="desc-export"
                  value={c.format}
                  checked={format === c.format}
                  data-testid={`desc-export-${c.format}`}
                  onChange={() => setFormat(c.format)}
                />{' '}
                {c.label}
              </label>
            ))}
          </fieldset>
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('results')} data-testid="desc-export-back">
              Back to the results
            </Button>
            <Button onClick={() => void saveExport()} disabled={busy} data-testid="desc-save">
              Save…
            </Button>
          </div>
          {exportNote && (
            <p role="status" data-testid="desc-export-note">
              {exportNote}
            </p>
          )}
        </Card>
      )}
    </div>
  );
};

export default DescribeFlow;

const Rail: React.FC<{ navigate: (p: string) => void }> = ({ navigate }) => (
  <NavRail
    items={[
      { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
      { id: 'describe', label: 'Descriptive summary', icon: 'Σ' },
    ]}
    activeId="describe"
    brand={<GaplyGlobe scale="mark" />}
  />
);

/** /app/statistics/describe — the page around the flow. */
export const DescribePage: React.FC<{ bridge?: DescribeBridge }> = ({ bridge }) => {
  const navigate = useNavigate();
  const b = useMemo(() => bridge ?? describeBridge(), [bridge]);
  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="describe-page">
      <AppShell rail={<Rail navigate={navigate} />} header={<HeaderBar title="Descriptive summary" />}>
        <Panel title="Descriptive summary of a data file">
          <DescribeFlow bridge={b} />
        </Panel>
      </AppShell>
    </div>
  );
};
