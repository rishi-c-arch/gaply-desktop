// Gaply — Cronbach's alpha (Research OS, Phase 8I-2, decision 384): an
// analysis of the user's own data, over alpha's bridge.
//
// Import → Profile → Settings → Validate → Run → Results → Export, as on the
// correlations' screen. Rust owns every decision: this screen passes the path
// once (to open), the slot, the run id and the user's choices, and shows what
// comes back.
//
// The choices, none made for the user, in AlphaSettings.tsx (decision 470): the numeric columns as items (none
// ticked, at least two; "Add every numeric column" and "Remove all" are
// actions the user takes), a Reverse-score box beside each ticked item (none
// ticked; unticking an item unticks it), the scale's lowest and highest
// values when an item is reverse-scored (two empty fields, never filled in
// and never taken from the data), and each candidate missing code in the
// ticked items, asked as a question. The spec lists the items and reversed
// items in the file's order and the codes ascending, and has no scale when
// nothing is reverse-scored, so one choice gives one spec (decision 338). A
// reversed value outside the ends is refused by Rust at Check the settings,
// one sentence per column, and never runs.
//
// The results: the rows used and the items; alpha with its 95% interval;
// standardized alpha; the mean inter-item correlation with its smallest and
// largest; the rows left out; the negative-alpha note; then one row per item.
// A statistic that is not computable shows its sentence where its number
// would be. Six figures on screen; the CSV keeps every digit. No verdict, no
// threshold, no stars and no advice on which item to drop or reverse
// (decision 66).
//
// What lives here, and for how long: the file's base name, the headers, the
// candidate missing-code values, the chosen settings, the typed scale ends and
// the result numbers, in this component's state only. Never a row, never
// another cell, never a path, and never browser storage. The result names
// columns by alias; the headers come from the open file's own profile, on this
// screen only. The file is closed, and Rust drops its rows, when the user
// closes it or the screen unmounts.
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../design-system';
import { CELL_PADDING } from '../../design-system/primitives';
import { pickManuscriptPath } from '../common/pickFile';
import { saveTextFile } from '../../utils/saveTextFile';
import { alphaBridge, type AlphaBridge } from './alphaBridge';
import { AlphaSettings, alphaNames, buildAlphaSpec, useAlphaSettings } from './AlphaSettings';
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
import type { AlphaResultView, AlphaRunStateView, AlphaValidationView } from './alphaTypes';
import { Stage, STATUS_SENTENCES } from './describeWording';
import {
  ALPHA_DIGITS_NOTE,
  ALPHA_ENTRY_TEXT,
  ALPHA_EXPORT_CHOICES,
  ALPHA_EXPORT_TEXT,
  ALPHA_ROWS_USED_NOTE,
  ALPHA_STAGE_HEADINGS,
  alphaIntervalText,
  alphaRefusalSentence,
  alphaRunEndedSentence,
  alphaSentence,
  alphaStatText,
  stepsSentence,
  type AlphaNames,
} from './alphaWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface AlphaFlowProps {
  bridge: AlphaBridge;
  /** The native file picker: an absolute path, or null when cancelled. */
  pickPath?: () => Promise<string | null>;
  /** Writes the export through the save dialog. */
  save?: (name: string, text: string, mime: string) => Promise<string | null>;
  /** How often a run's state is read while it runs. */
  pollMs?: number;
  /** This computer's limits for the methods, read once. */
  limits?: () => Promise<unknown>;
}

// So alphaTypes.vitest.ts's import holds (decision 470).
export { buildAlphaSpec };

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

const AlphaFlow: React.FC<AlphaFlowProps> = ({
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
  const settings = useAlphaSettings(view);
  const [report, setReport] = useState<AlphaValidationView | null>(null);
  const [runView, setRunView] = useState<AlphaRunStateView | null>(null);
  const [result, setResult] = useState<AlphaResultView | null>(null);
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);
  const slot = useRef<number | null>(null);

  const heading = useStageFocus(stage);
  useCloseOnUnmount(bridge, slot);

  const names: AlphaNames = useMemo(() => alphaNames(view), [view]);

  const spec = useCallback(() => {
    operation.current += 1;
    return settings.spec(operation.current);
  }, [settings]);

  useRunPolling(runView, setRunView, bridge, setError, alphaRefusalSentence, pollMs);

  const act = actWith(setBusy, setError, alphaRefusalSentence);

  const reset = () => {
    settings.reset();
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
      const choice = ALPHA_EXPORT_CHOICES.find((c) => c.format === format) ?? ALPHA_EXPORT_CHOICES[0];
      const out = await bridge.export(result.runId, format);
      const saved = await saveExactly(out, choice, save);
      if ('error' in saved) {
        setError(saved.error);
        return;
      }
      setExportNote(saved.note);
    });

  const title = (
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="al-stage-heading">
      {ALPHA_STAGE_HEADINGS[stage]}
    </h3>
  );


  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="al-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <StageNav prefix="al" stage={stage} />
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="al-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && <FileSummary prefix="al" view={view} />}
      <OpenProgressLine prefix="al" rowsRead={openProgress.rowsRead} />
      {error && (
        <p role="alert" data-testid="al-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="al-entry-text">
            {ALPHA_ENTRY_TEXT}
          </p>
          <LimitsLine prefix="al" limits={methodLimits} />
          <Button onClick={() => void choose()} disabled={busy} data-testid="al-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <ProfileTable prefix="al" view={view} />
          <Button onClick={() => setStage('settings')} data-testid="al-to-settings">
            Continue to settings
          </Button>
        </Card>
      )}

      {stage === 'settings' && view && (
        <Card>
          {title}
          <AlphaSettings view={view} settings={settings} />
          <SettingsActions prefix="al" busy={busy} ready={settings.stillMissing === null} onBack={() => setStage('profile')} onCheck={() => void check()} />
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="al-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} sentence={(d) => alphaSentence(d, names)} testId="al-validation-errors" />
          <DiagnosticList items={report.missingInformation} sentence={(d) => alphaSentence(d, names)} testId="al-validation-questions" />
          <DiagnosticList items={report.warnings} sentence={(d) => alphaSentence(d, names)} testId="al-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="al-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="al-run"
            >
              Run Cronbach&apos;s alpha
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <RunCard
          prefix="al"
          title={title}
          runView={runView}
          endedSentence={alphaRunEndedSentence}
          progress={
            <p style={{ margin: 0 }} data-testid="al-progress">
              {stepsSentence(runView.stepsDone, runView.stepsTotal)}
            </p>
          }
          diagnostics={<DiagnosticList items={runView.diagnostics} sentence={(d) => alphaSentence(d, names)} testId="al-run-diagnostics" />}
          busy={busy}
          onCancel={() => void cancel()}
          onSeeResults={() => void seeResults()}
          onBack={() => setStage('settings')}
        />
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <AlphaResultCard result={result} names={names} />
          <SaveToProject runId={result.runId} testId="al-project-save" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="al-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="al-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <ExportCard
          prefix="al"
          title={title}
          text={ALPHA_EXPORT_TEXT}
          choices={ALPHA_EXPORT_CHOICES}
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

const th = (label: string) => (
  <th key={label} scope="col" style={{ textAlign: 'left', ...CELL_PADDING }}>
    {label}
  </th>
);

/** One line of the summary: its label and its value, or its sentence. */
const SummaryRow: React.FC<{ label: string; testId: string; children: React.ReactNode }> = ({ label, testId, children }) => (
  <tr>
    <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
      {label}
    </th>
    <td style={CELL_PADDING} data-testid={testId}>
      {children}
    </td>
  </tr>
);

const ITEM_HEADS = [
  'Item',
  'Reverse-scored',
  'Mean',
  'SD',
  'Scale mean if deleted',
  'Scale variance if deleted',
  'Corrected item-total correlation',
  'Alpha if deleted',
];

/** The neutral result card (decision 66): the summary, the rows left out,
 *  the note, each item, what is not computable in its own words, what is not
 *  assessed, and the manifest. */
export const AlphaResultCard: React.FC<{ result: AlphaResultView; names: AlphaNames }> = ({ result, names }) => {
  const r = result.result;
  const label = (alias: string) => names.column(aliasNumber(alias));
  const ii = r.interItem;
  const interItem =
    'value' in ii.mean && 'value' in ii.min && 'value' in ii.max
      ? `${alphaStatText(ii.mean)} (smallest ${alphaStatText(ii.min)}, largest ${alphaStatText(ii.max)})`
      : alphaStatText('value' in ii.mean ? ('value' in ii.min ? ii.max : ii.min) : ii.mean);
  return (
    <div data-testid="al-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => alphaSentence(d, names)} testId="al-result-notes" />
      <table data-testid="al-summary" style={{ borderCollapse: 'collapse' }}>
        <caption style={{ textAlign: 'left' }}>Over the rows used</caption>
        <tbody>
          <SummaryRow label="Rows used (n)" testId="al-n">
            {r.rowsUsed}
          </SummaryRow>
          <SummaryRow label="Items (k)" testId="al-k">
            {r.k}
          </SummaryRow>
          <SummaryRow label="Cronbach's alpha" testId="al-alpha">
            {alphaStatText(r.alpha)}
          </SummaryRow>
          <SummaryRow label="95% interval for alpha" testId="al-interval">
            {alphaIntervalText(r.interval)}
          </SummaryRow>
          <SummaryRow label="Standardized alpha" testId="al-standardized">
            {alphaStatText(r.standardized)}
          </SummaryRow>
          <SummaryRow label="Mean inter-item correlation" testId="al-inter-item">
            {interItem}
          </SummaryRow>
        </tbody>
      </table>
      {r.notes.map((code) => (
        <p key={code} role="note" data-testid="al-note">
          {alphaSentence({ code })}
        </p>
      ))}
      <section aria-label="Rows left out" data-testid="al-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e, i) => (
              <li key={`${e.code}-${i}`}>{alphaSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <table data-testid="al-items" style={{ borderCollapse: 'collapse' }}>
        <caption style={{ textAlign: 'left' }}>Each item, after reverse-scoring</caption>
        <thead>
          <tr>{ITEM_HEADS.map(th)}</tr>
        </thead>
        <tbody>
          {r.items.map((it) => (
            <tr key={it.item} data-testid={`al-item-row-${it.item}`}>
              <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
                {label(it.item)}
              </th>
              <td style={CELL_PADDING}>{it.reversed ? 'Reversed' : ''}</td>
              <td style={CELL_PADDING}>{alphaStatText(it.mean)}</td>
              <td style={CELL_PADDING}>{alphaStatText(it.sd)}</td>
              <td style={CELL_PADDING}>{alphaStatText(it.scaleMeanIfDeleted)}</td>
              <td style={CELL_PADDING}>{alphaStatText(it.scaleVarianceIfDeleted)}</td>
              <td style={CELL_PADDING}>{alphaStatText(it.correctedItemTotal)}</td>
              <td style={CELL_PADDING}>{alphaStatText(it.alphaIfDeleted)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <section aria-label="What this does not assess" data-testid="al-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{alphaSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer">{ALPHA_DIGITS_NOTE}</p>
      <ManifestPanel prefix="al" manifest={result.manifest} rowsUsedNote={ALPHA_ROWS_USED_NOTE} />
    </div>
  );
};

export default AlphaFlow;

/** /app/statistics/alpha — the page around the flow. */
export const AlphaPage: React.FC<{ bridge?: AlphaBridge }> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? alphaBridge(), [bridge]);
  return (
    <MethodPage testId="alpha-page" header="Cronbach's alpha" panelTitle="Cronbach's alpha on a data file">
      <AlphaFlow bridge={b} />
    </MethodPage>
  );
};
