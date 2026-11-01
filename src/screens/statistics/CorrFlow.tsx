// Gaply — the correlations (Research OS, Phase 8H-2, decision 359): an
// analysis of the user's own data, over the correlations' bridge.
//
// Import → Profile → Settings → Validate → Run → Results → Export, as on the
// ANOVA's screen. Rust owns every decision: this screen passes the path once
// (to open), the slot, the run id and the user's choices, and shows what
// comes back.
//
// The choices, none made for the user: the numeric columns (none ticked, at
// least two; "Tick every numeric column" and "Untick all" are actions the
// user takes), the coefficients (none ticked, at least one), the missing
// handling (pairwise or listwise, neither chosen), and each candidate missing
// code in the ticked columns, asked as a question. The spec lists the columns
// in the file's order, the coefficients in Pearson, Spearman, Kendall order
// and the codes ascending, so one choice gives one spec (decision 338). A
// choice over the budget is refused by Rust at Check the settings, with its
// pairs and rows, and never runs.
//
// The results: the descriptives; one button per chosen coefficient with its
// pair count, one coefficient at a time; for 20 variables or fewer, a lower
// triangle of the coefficients; then every pair, in pages of 1,225 rows. A
// statistic that is not computable shows its sentence where its number would
// be; a perfect Pearson or Spearman t shows its sentence and its p shows 0.
// Six figures on screen; the CSV keeps every digit. No verdict words and no
// stars (decision 66).
//
// What lives here, and for how long: the file's base name, the headers, the
// candidate missing-code values, the chosen settings and the result numbers,
// in this component's state only. Never a row, never another cell, never a
// path, and never browser storage. The result names columns by alias; the
// headers come from the open file's own profile, on this screen only. The
// file is closed, and Rust drops its rows, when the user closes it or the
// screen unmounts.
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../design-system';
import { CELL_PADDING } from '../../design-system/primitives';
import { pickManuscriptPath } from '../common/pickFile';
import { saveTextFile } from '../../utils/saveTextFile';
import { corrBridge, type CorrBridge } from './corrBridge';
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
import {
  COEFFICIENTS,
  type CoefficientTable,
  type Coefficient,
  type CorrResultView,
  type CorrRunStateView,
  type CorrSpec,
  type CorrValidationView,
  type MissingHandling,
  type Pair,
} from './corrTypes';
import { Stage, STATUS_SENTENCES } from './describeWording';
import { shownNumber } from './ttestWording';
import {
  boundedText,
  COEFFICIENT_CHOICES,
  COEFFICIENTS_QUESTION,
  coefficientHeading,
  CORR_DIGITS_NOTE,
  CORR_ENTRY_TEXT,
  CORR_EXPORT_CHOICES,
  CORR_EXPORT_TEXT,
  CORR_ROWS_USED_NOTE,
  CORR_STAGE_HEADINGS,
  corrMissingCodeQuestion,
  corrRefusalSentence,
  corrRunEndedSentence,
  corrSentence,
  corrStatText,
  MATRIX_MAX_VARIABLES,
  MISSING_CHOICES,
  MISSING_QUESTION,
  PAIRS_PAGE,
  pairsShownSentence,
  settingsMissingSentence,
  stepsSentence,
  VARIABLES_QUESTION,
  type CorrNames,
} from './corrWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface CorrFlowProps {
  bridge: CorrBridge;
  /** The native file picker: an absolute path, or null when cancelled. */
  pickPath?: () => Promise<string | null>;
  /** Writes the export through the save dialog. */
  save?: (name: string, text: string, mime: string) => Promise<string | null>;
  /** How often a run's state is read while it runs. */
  pollMs?: number;
  /** This computer's limits for the methods, read once. */
  limits?: () => Promise<unknown>;
}

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** Decision 338: the ticked columns in the file's column order, the
 *  coefficients in COEFFICIENTS' order, the codes ascending and once each. */
export function buildCorrSpec(
  view: DatasetView,
  variables: string[],
  coefficients: Coefficient[],
  missing: MissingHandling,
  missingCodes: number[],
  operationId: number
): CorrSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.correlations',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      variables: view.profile.columns.map((c) => c.alias).filter((a) => variables.includes(a)),
      coefficients: COEFFICIENTS.filter((c) => coefficients.includes(c)),
      missing,
      missingCodes: codes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

const CorrFlow: React.FC<CorrFlowProps> = ({
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
  const [variables, setVariables] = useState<string[]>([]);
  const [coefficients, setCoefficients] = useState<Coefficient[]>([]);
  const [missing, setMissing] = useState<MissingHandling | null>(null);
  const [ticked, setTicked] = useState<number[]>([]);
  const [report, setReport] = useState<CorrValidationView | null>(null);
  const [runView, setRunView] = useState<CorrRunStateView | null>(null);
  const [result, setResult] = useState<CorrResultView | null>(null);
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);
  const slot = useRef<number | null>(null);

  const heading = useStageFocus(stage);
  useCloseOnUnmount(bridge, slot);

  /** The candidate codes of the ticked columns, ascending, with where each
   *  appears (decision 359: the union over the ticked columns). */
  const offered = useMemo(() => {
    const where = new Map<number, Array<{ header: string; cells: number }>>();
    for (const c of view?.profile.columns ?? []) {
      if (!variables.includes(c.alias)) continue;
      for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: c.header, cells: s.cells }]);
    }
    return Array.from(where.keys())
      .sort((a, b) => a - b)
      .map((code) => ({ code, where: where.get(code) ?? [] }));
  }, [view, variables]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: CorrNames = useMemo(
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
    return buildCorrSpec(view as DatasetView, variables, coefficients, missing as MissingHandling, missingCodes, operation.current);
  }, [view, variables, coefficients, missing, missingCodes]);

  useRunPolling(runView, setRunView, bridge, setError, corrRefusalSentence, pollMs);

  const act = actWith(setBusy, setError, corrRefusalSentence);

  const reset = () => {
    setVariables([]);
    setCoefficients([]);
    setMissing(null);
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
      const choice = CORR_EXPORT_CHOICES.find((c) => c.format === format) ?? CORR_EXPORT_CHOICES[0];
      const out = await bridge.export(result.runId, format);
      const saved = await saveExactly(out, choice, save);
      if ('error' in saved) {
        setError(saved.error);
        return;
      }
      setExportNote(saved.note);
    });

  const toggle = <T,>(set: React.Dispatch<React.SetStateAction<T[]>>, x: T, on: boolean) =>
    set((t) => (on ? [...t.filter((y) => y !== x), x] : t.filter((y) => y !== x)));

  const title = (
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="cr-stage-heading">
      {CORR_STAGE_HEADINGS[stage]}
    </h3>
  );

  const stillMissing = settingsMissingSentence(variables.length, coefficients.length, missing !== null);

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="cr-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <StageNav prefix="cr" stage={stage} />
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="cr-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && <FileSummary prefix="cr" view={view} />}
      <OpenProgressLine prefix="cr" rowsRead={openProgress.rowsRead} />
      {error && (
        <p role="alert" data-testid="cr-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="cr-entry-text">
            {CORR_ENTRY_TEXT}
          </p>
          <LimitsLine prefix="cr" limits={methodLimits} />
          <Button onClick={() => void choose()} disabled={busy} data-testid="cr-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <ProfileTable prefix="cr" view={view} />
          <Button onClick={() => setStage('settings')} data-testid="cr-to-settings">
            Continue to settings
          </Button>
        </Card>
      )}

      {stage === 'settings' && view && (
        <Card>
          {title}
          <div style={{ display: 'grid', gap: 8 }}>
            <fieldset data-testid="cr-variables">
              <legend>{VARIABLES_QUESTION}</legend>
              <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
                <Button variant="secondary" onClick={() => setVariables(numeric(view).map((c) => c.alias))} data-testid="cr-tick-all">
                  Tick every numeric column
                </Button>
                <Button variant="secondary" onClick={() => setVariables([])} data-testid="cr-untick-all">
                  Untick all
                </Button>
              </div>
              {numeric(view).map((c) => (
                <label key={c.alias} style={{ display: 'block' }}>
                  <input
                    type="checkbox"
                    checked={variables.includes(c.alias)}
                    data-testid={`cr-variable-${c.alias}`}
                    onChange={(e) => toggle(setVariables, c.alias, e.target.checked)}
                  />{' '}
                  {c.alias} {c.header}
                </label>
              ))}
            </fieldset>
            <fieldset data-testid="cr-coefficients">
              <legend>{COEFFICIENTS_QUESTION}</legend>
              {COEFFICIENT_CHOICES.map((c) => (
                <div key={c.coefficient}>
                  <label style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={coefficients.includes(c.coefficient)}
                      data-testid={`cr-coefficient-${c.coefficient}`}
                      aria-describedby={`cr-coefficient-line-${c.coefficient}`}
                      onChange={(e) => toggle(setCoefficients, c.coefficient, e.target.checked)}
                    />{' '}
                    {c.label}
                  </label>
                  <p id={`cr-coefficient-line-${c.coefficient}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
                    {c.line}
                  </p>
                </div>
              ))}
            </fieldset>
            <fieldset data-testid="cr-missing">
              <legend>{MISSING_QUESTION}</legend>
              {MISSING_CHOICES.map((c) => (
                <div key={c.missing}>
                  <label style={{ display: 'block' }}>
                    <input
                      type="radio"
                      name="cr-missing"
                      checked={missing === c.missing}
                      data-testid={`cr-missing-${c.missing}`}
                      aria-describedby={`cr-missing-line-${c.missing}`}
                      onChange={() => setMissing(c.missing)}
                    />{' '}
                    {c.label}
                  </label>
                  <p id={`cr-missing-line-${c.missing}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
                    {c.line}
                  </p>
                </div>
              ))}
            </fieldset>
            {offered.length > 0 && (
              <fieldset data-testid="cr-missing-codes">
                <legend>Candidate missing codes</legend>
                {offered.map((s) => (
                  <label key={s.code} style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={ticked.includes(s.code)}
                      data-testid={`cr-code-${s.code}`}
                      onChange={(e) => toggle(setTicked, s.code, e.target.checked)}
                    />{' '}
                    {corrMissingCodeQuestion(s.code, s.where)}
                  </label>
                ))}
              </fieldset>
            )}
            {stillMissing && (
              <p role="status" data-testid="cr-settings-missing">
                {stillMissing}
              </p>
            )}
          </div>
          <SettingsActions prefix="cr" busy={busy} ready={stillMissing === null} onBack={() => setStage('profile')} onCheck={() => void check()} />
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="cr-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} sentence={(d) => corrSentence(d, names)} testId="cr-validation-errors" />
          <DiagnosticList items={report.missingInformation} sentence={(d) => corrSentence(d, names)} testId="cr-validation-questions" />
          <DiagnosticList items={report.warnings} sentence={(d) => corrSentence(d, names)} testId="cr-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="cr-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="cr-run"
            >
              Run the correlations
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <RunCard
          prefix="cr"
          title={title}
          runView={runView}
          endedSentence={corrRunEndedSentence}
          progress={
            <p style={{ margin: 0 }} data-testid="cr-progress">
              {stepsSentence(runView.stepsDone, runView.stepsTotal)}
            </p>
          }
          diagnostics={<DiagnosticList items={runView.diagnostics} sentence={(d) => corrSentence(d, names)} testId="cr-run-diagnostics" />}
          busy={busy}
          onCancel={() => void cancel()}
          onSeeResults={() => void seeResults()}
          onBack={() => setStage('settings')}
        />
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <CorrResultCard result={result} names={names} />
          <SaveToProject runId={result.runId} testId="cr-project-save" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="cr-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="cr-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <ExportCard
          prefix="cr"
          title={title}
          text={CORR_EXPORT_TEXT}
          choices={CORR_EXPORT_CHOICES}
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

const n = shownNumber;
const th = (label: string) => (
  <th key={label} scope="col" style={{ textAlign: 'left', ...CELL_PADDING }}>
    {label}
  </th>
);

/** Each coefficient's own symbol. */
const SYMBOL: Record<Coefficient, string> = { pearson: 'r', spearman: 'rho', kendall: 'tau-b' };

/** A pair's coefficient: six figures, or its sentence where it would be. */
const estimateText = (p: Pair) =>
  'computed' in p.outcome ? n(p.outcome.computed.estimate) : corrSentence({ code: p.outcome.not_computable });

/** The lower triangle of one coefficient, coefficients only (decision 359). */
const CorrMatrix: React.FC<{ table: CoefficientTable; variables: string[]; label: (alias: string) => string }> = ({
  table,
  variables,
  label,
}) => {
  const at = new Map(table.pairs.map((p) => [`${p.first} ${p.second}`, p]));
  return (
    <table data-testid="cr-matrix" style={{ borderCollapse: 'collapse' }}>
      <caption style={{ textAlign: 'left' }}>{`${SYMBOL[table.coefficient]} for each pair, below the diagonal`}</caption>
      <thead>
        <tr>
          <td />
          {variables.slice(0, -1).map((v) => th(label(v)))}
        </tr>
      </thead>
      <tbody>
        {variables.slice(1).map((row, i) => (
          <tr key={row}>
            <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
              {label(row)}
            </th>
            {variables.slice(0, i + 1).map((col) => {
              const p = at.get(`${col} ${row}`);
              return (
                <td key={col} className="gds-mono" style={CELL_PADDING} data-testid={`cr-matrix-${col}-${row}`}>
                  {p ? estimateText(p) : ''}
                </td>
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
};

/** One coefficient's pairs, in pages of PAIRS_PAGE rows (decision 359). */
const PairsTable: React.FC<{ table: CoefficientTable; label: (alias: string) => string }> = ({ table, label }) => {
  const [page, setPage] = useState(0);
  const total = table.pairs.length;
  const pages = Math.max(1, Math.ceil(total / PAIRS_PAGE));
  const first = page * PAIRS_PAGE;
  const shown = table.pairs.slice(first, first + PAIRS_PAGE);
  const kendall = table.coefficient === 'kendall';
  const heads = kendall
    ? ['First', 'Second', 'n', SYMBOL[table.coefficient], 'z', 'p', '95% interval']
    : ['First', 'Second', 'n', SYMBOL[table.coefficient], 't', 'df', 'p', '95% interval'];
  return (
    <section data-testid={`cr-pairs-${table.coefficient}`} aria-label={coefficientHeading(table.coefficient, total)}>
      {pages > 1 && (
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <Button variant="secondary" disabled={page === 0} onClick={() => setPage(page - 1)} data-testid="cr-pairs-previous">
            Previous
          </Button>
          <Button variant="secondary" disabled={page >= pages - 1} onClick={() => setPage(page + 1)} data-testid="cr-pairs-next">
            Next
          </Button>
          <p role="status" style={{ margin: 0 }} data-testid="cr-pairs-shown">
            {pairsShownSentence(first + 1, first + shown.length, total)}
          </p>
        </div>
      )}
      <table style={{ borderCollapse: 'collapse' }}>
        <thead>
          <tr>{heads.map(th)}</tr>
        </thead>
        <tbody>
          {shown.map((p) => (
            <tr key={`${p.first}-${p.second}`} data-testid={`cr-pair-${p.first}-${p.second}`}>
              <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
                {label(p.first)}
              </th>
              <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
                {label(p.second)}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>{p.n}</td>
              {'computed' in p.outcome ? (
                <>
                  <td className="gds-mono" style={CELL_PADDING}>{n(p.outcome.computed.estimate)}</td>
                  {'t' in p.outcome.computed.test ? (
                    <>
                      <td style={CELL_PADDING} data-testid="cr-t">{boundedText(p.outcome.computed.test.t.t)}</td>
                      <td className="gds-mono" style={CELL_PADDING}>{n(p.outcome.computed.test.t.df)}</td>
                    </>
                  ) : (
                    <td className="gds-mono" style={CELL_PADDING} data-testid="cr-z">{n(p.outcome.computed.test.z)}</td>
                  )}
                  <td className="gds-mono" style={CELL_PADDING} data-testid="cr-p">{n(p.outcome.computed.p)}</td>
                  <td style={CELL_PADDING} data-testid="cr-interval">
                    {'computed' in p.outcome.computed.interval
                      ? `${n(p.outcome.computed.interval.computed.lower)} to ${n(p.outcome.computed.interval.computed.upper)}`
                      : corrSentence({ code: p.outcome.computed.interval.not_computable })}
                  </td>
                </>
              ) : (
                <td colSpan={heads.length - 3} style={CELL_PADDING} data-testid="cr-pair-not-computable">
                  {corrSentence({ code: p.outcome.not_computable })}
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
};

/** The neutral result card (decision 66): the descriptives, each chosen
 *  coefficient one at a time, what is not computable in its own words, what
 *  is not assessed, and the manifest. */
export const CorrResultCard: React.FC<{ result: CorrResultView; names: CorrNames }> = ({ result, names }) => {
  const r = result.result;
  const label = (alias: string) => names.column(aliasNumber(alias));
  const variables = r.descriptives.map((d) => d.variable);
  const [shown, setShown] = useState<Coefficient | null>(r.coefficients.length > 0 ? r.coefficients[0].coefficient : null);
  const table = r.coefficients.find((t) => t.coefficient === shown) ?? null;
  return (
    <div data-testid="cr-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => corrSentence(d, names)} testId="cr-result-notes" />
      <table data-testid="cr-descriptives" style={{ borderCollapse: 'collapse' }}>
        <caption style={{ textAlign: 'left' }}>The variables, over the rows used</caption>
        <thead>
          <tr>{['Variable', 'n', 'Mean', 'SD'].map(th)}</tr>
        </thead>
        <tbody>
          {r.descriptives.map((d) => (
            <tr key={d.variable}>
              <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
                {label(d.variable)}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>{d.n}</td>
              <td style={CELL_PADDING}>{corrStatText(d.mean)}</td>
              <td style={CELL_PADDING}>{corrStatText(d.sd)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {r.coefficients.length > 0 && (
        <section aria-label="The coefficients" data-testid="cr-coefficient-results">
          <div role="group" aria-label="Which coefficient to show" style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
            {r.coefficients.map((t) => (
              <Button
                key={t.coefficient}
                variant={t.coefficient === shown ? 'primary' : 'secondary'}
                aria-pressed={t.coefficient === shown}
                onClick={() => setShown(t.coefficient)}
                data-testid={`cr-show-${t.coefficient}`}
              >
                {coefficientHeading(t.coefficient, t.pairs.length)}
              </Button>
            ))}
          </div>
          {table && variables.length <= MATRIX_MAX_VARIABLES && (
            <CorrMatrix key={`m-${table.coefficient}`} table={table} variables={variables} label={label} />
          )}
          {table && <PairsTable key={table.coefficient} table={table} label={label} />}
        </section>
      )}
      <section aria-label="Rows left out" data-testid="cr-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e, i) => (
              <li key={`${e.code}-${i}`}>{corrSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="cr-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{corrSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer">{CORR_DIGITS_NOTE}</p>
      <ManifestPanel prefix="cr" manifest={result.manifest} rowsUsedNote={CORR_ROWS_USED_NOTE} />
    </div>
  );
};

export default CorrFlow;

/** /app/statistics/correlations — the page around the flow. */
export const CorrPage: React.FC<{ bridge?: CorrBridge }> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? corrBridge(), [bridge]);
  return (
    <MethodPage testId="correlations-page" header="Correlations" panelTitle="Correlations on a data file">
      <CorrFlow bridge={b} />
    </MethodPage>
  );
};
