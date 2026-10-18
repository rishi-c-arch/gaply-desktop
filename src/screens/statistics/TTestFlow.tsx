// Gaply — the independent-samples t-test (Research OS, Phase 4B-2): an
// analysis of the user's own data, over the t-test bridge.
//
// Import → Profile → Settings → Validate → Run → Results → Export, as on the
// descriptive summary's screen. Rust owns every decision: this screen passes
// the path once (to open), the slot, the run id and the user's choices, and
// shows what comes back.
//
// The choices: the outcome (a numeric column), the group (any column with at
// least two levels when read as categorical, numeric columns included:
// decision 81), the two levels when there are more, chosen by label and
// sent as their positions in the list stats_ttest_group_levels returns,
// which is the core's own indexing (decisions 78, 81: the screen never sorts
// labels), the variant, asked with no default (decision 64), and each
// candidate missing code in the outcome, asked as a question.
//
// What lives here, and for how long: the file's base name, the headers, the
// level labels, the candidate missing-code values, the chosen settings and
// the result numbers, in this component's state only. Never a row, never
// another cell, never a path, and never browser storage. No label goes into
// a spec. The file is closed, and Rust drops its rows, when the user closes
// it or the screen unmounts.
//
// The wording is neutral (decision 66): the card reports the numbers and
// what they do not assess, and passes no verdict on them.
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { pickManuscriptPath } from '../common/pickFile';
import { saveTextFile } from '../../utils/saveTextFile';
import { ttestBridge, type TTestBridge } from './ttestBridge';
import type { ColumnProfile, DatasetView, ExportFormat } from './describeTypes';
import type {
  GroupLevel,
  TTestDiagnostic,
  TTestResultView,
  TTestRunStateView,
  TTestSpec,
  TTestValidationReportView,
  Variant,
} from './ttestTypes';
import {
  computeTimeSentence,
  elapsedSentence,
  platformName,
  reproducibilitySentence,
  RUN_STATE_NAMES,
  shortHash,
  Stage,
  STAGES,
  STATUS_SENTENCES,
} from './describeWording';
import {
  GROUP_LIMIT_SENTENCE,
  levelsQuestion,
  MAX_GROUP_LEVELS,
  SD_RATIO_NOTE,
  shownNumber,
  STANDARDIZER_NAMES,
  statSentence,
  TTEST_DIGITS_NOTE,
  TTEST_ENTRY_TEXT,
  TTEST_EXPORT_CHOICES,
  TTEST_EXPORT_TEXT,
  TTEST_LEVELS_RULE,
  TTEST_ROWS_USED_NOTE,
  TTEST_STAGE_HEADINGS,
  ttestMissingCodeQuestion,
  ttestRefusalSentence,
  ttestRunEndedSentence,
  ttestSentence,
  type TTestNames,
  VARIANT_CHOICES,
  VARIANT_NAMES,
  VARIANT_QUESTION,
} from './ttestWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface TTestFlowProps {
  bridge: TTestBridge;
  /** The native file picker: an absolute path, or null when cancelled. The
   *  path goes to `open` and is never kept. */
  pickPath?: () => Promise<string | null>;
  /** Writes the export through the save dialog; the chosen path, or null
   *  when the dialog was closed. */
  save?: (name: string, text: string, mime: string) => Promise<string | null>;
  /** How often a run's state is read while it runs. */
  pollMs?: number;
}

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

/** The columns that can be an outcome: numeric ones. */
const outcomes = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** The columns that can be a group: any with 2 to MAX_GROUP_LEVELS levels
 *  when read as categorical, numeric ones included (decisions 81, 87). The
 *  profile's `distinct` counts them for every kind; the levels themselves
 *  come from stats_ttest_group_levels, which refuses a column with more. */
const groups = (view: DatasetView) =>
  view.profile.columns.filter((c: ColumnProfile) => c.distinct >= 2 && c.distinct <= MAX_GROUP_LEVELS);

/** The two chosen levels, by position in the group-levels list. */
type LevelPicks = [number | null, number | null];

function defaultLevels(levels: GroupLevel[]): LevelPicks {
  return levels.length === 2 ? [0, 1] : [null, null];
}

function buildSpec(
  view: DatasetView,
  outcome: string,
  group: string,
  levels: LevelPicks,
  variant: Variant | null,
  missingCodes: number[],
  operationId: number
): TTestSpec {
  const [a, b] = levels;
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.independent_t',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      outcome,
      group,
      // Positions, never labels (decision 78). Both or neither.
      levels: a !== null && b !== null ? [a, b] : null,
      variant,
      missingCodes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

const DiagnosticList: React.FC<{ items: TTestDiagnostic[]; names: TTestNames; testId: string }> = ({ items, names, testId }) =>
  items.length === 0 ? null : (
    <ul data-testid={testId} style={{ margin: 0 }}>
      {items.map((d, i) => (
        <li key={i}>{ttestSentence(d, names)}</li>
      ))}
    </ul>
  );

const TTestFlow: React.FC<TTestFlowProps> = ({
  bridge,
  pickPath = () => pickManuscriptPath(DATA_EXTENSIONS, 'Data file'),
  save = saveTextFile,
  pollMs = 250,
}) => {
  const [stage, setStage] = useState<Stage>('import');
  const [view, setView] = useState<DatasetView | null>(null);
  const [outcome, setOutcome] = useState('');
  const [group, setGroup] = useState('');
  /** The chosen group's levels, as Rust indexes them. */
  const [groupLevels, setGroupLevels] = useState<GroupLevel[]>([]);
  const [levels, setLevels] = useState<LevelPicks>([null, null]);
  const [variant, setVariant] = useState<Variant | null>(null);
  const [ticked, setTicked] = useState<number[]>([]);
  const [report, setReport] = useState<TTestValidationReportView | null>(null);
  const [runView, setRunView] = useState<TTestRunStateView | null>(null);
  const [result, setResult] = useState<TTestResultView | null>(null);
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

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: TTestNames = useMemo(
    () => ({
      column: (alias: number) => {
        const c = view?.profile.columns.find((col) => aliasNumber(col.alias) === alias);
        return c ? `"${c.header}"` : `C${alias}`;
      },
      level: (index: number) => groupLevels.find((l) => l.index === index)?.label ?? `level ${index + 1}`,
    }),
    [view, groupLevels]
  );

  const spec = useCallback(() => {
    operation.current += 1;
    return buildSpec(view as DatasetView, outcome, group, levels, variant, missingCodes, operation.current);
  }, [view, outcome, group, levels, variant, missingCodes]);

  // While a run is unfinished, its state is read every `pollMs`.
  useEffect(() => {
    if (!runView || runView.terminal) return;
    let live = true;
    const t = setTimeout(() => {
      bridge
        .status(runView.runId)
        .then((s) => live && setRunView(s))
        .catch((e) => live && setError(ttestRefusalSentence(e)));
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
      setError(ttestRefusalSentence(e));
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    setOutcome('');
    setGroup('');
    setGroupLevels([]);
    setLevels([null, null]);
    setVariant(null);
    setTicked([]);
    setReport(null);
    setRunView(null);
    setResult(null);
  };

  const choose = () =>
    act(async () => {
      const path = await pickPath();
      if (!path) return;
      const opened = await bridge.open(path);
      if (slot.current !== null) void bridge.close(slot.current).catch(() => undefined);
      slot.current = opened.slot;
      setView(opened);
      reset();
      setStage('profile');
    });

  const closeFile = () =>
    act(async () => {
      if (slot.current !== null) await bridge.close(slot.current);
      slot.current = null;
      setView(null);
      reset();
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
      const choice = TTEST_EXPORT_CHOICES.find((c) => c.format === format) ?? TTEST_EXPORT_CHOICES[0];
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

  const chooseGroup = (alias: string) =>
    act(async () => {
      setGroup(alias);
      setGroupLevels([]);
      setLevels([null, null]);
      if (alias === '' || slot.current === null) return;
      // The levels in the core's own order (decision 81); positions in this
      // list are what the spec sends.
      const read = await bridge.groupLevels(slot.current, alias);
      setGroupLevels(read.levels);
      setLevels(defaultLevels(read.levels));
    });
  const chooseLevel = (which: 0 | 1, value: string) =>
    setLevels((l) => {
      const next: LevelPicks = [l[0], l[1]];
      next[which] = value === '' ? null : Number(value);
      return next;
    });
  const toggleCode = (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code)));

  const stageNav = (
    <nav aria-label="Steps" data-testid="tt-steps" style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
      {STAGES.map((s) => (
        <span key={s.id} aria-current={s.id === stage ? 'step' : undefined} style={{ fontWeight: s.id === stage ? 700 : 400 }}>
          {s.label}
        </span>
      ))}
    </nav>
  );

  const title = (
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="tt-stage-heading">
      {TTEST_STAGE_HEADINGS[stage]}
    </h3>
  );

  const ready = outcome !== '' && group !== '';

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="tt-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        {stageNav}
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="tt-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && (
        <p data-testid="tt-file-summary" style={{ margin: 0 }}>
          <span className="gds-mono">{view.displayName}</span>: {view.profile.rows} rows, {view.profile.columns.length}{' '}
          {view.profile.columns.length === 1 ? 'column' : 'columns'}.
        </p>
      )}
      {error && (
        <p role="alert" data-testid="tt-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="tt-entry-text">
            {TTEST_ENTRY_TEXT}
          </p>
          <Button onClick={() => void choose()} disabled={busy} data-testid="tt-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <p data-testid="tt-levels-rule">{TTEST_LEVELS_RULE}</p>
          <table data-testid="tt-profile" style={{ borderCollapse: 'collapse' }}>
            <thead>
              <tr>
                {['Column', 'Kind', 'n', 'Missing', 'Levels'].map((h) => (
                  <th key={h} scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {view.profile.columns.map((c) => (
                <tr key={c.alias}>
                  <td>
                    <span className="gds-mono">{c.alias}</span> {c.header}
                  </td>
                  <td>{c.kind === 'numeric' ? 'Numeric' : c.kind === 'categorical' ? 'Categorical' : 'Empty'}</td>
                  <td className="gds-mono">{c.n}</td>
                  <td className="gds-mono">{c.missing}</td>
                  <td>
                    {(c.levels ?? []).slice(0, 12).map((l) => `${l.label} (${l.count})`).join(', ')}
                    {(c.levels?.length ?? 0) > 12 ? `, and ${(c.levels?.length ?? 0) - 12} more` : ''}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <Button onClick={() => setStage('settings')} data-testid="tt-to-settings">
            Continue to settings
          </Button>
        </Card>
      )}

      {stage === 'settings' && view && (
        <Card>
          {title}
          <div style={{ display: 'grid', gap: 8 }}>
            <label>
              Outcome (a numeric column){' '}
              <select value={outcome} data-testid="tt-outcome" onChange={(e) => setOutcome(e.target.value)}>
                <option value="">Choose a column…</option>
                {outcomes(view).map((c) => (
                  <option key={c.alias} value={c.alias}>
                    {c.alias} {c.header}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Group (a column with at least two levels){' '}
              <select value={group} data-testid="tt-group" onChange={(e) => void chooseGroup(e.target.value)}>
                <option value="">Choose a column…</option>
                {groups(view).map((c) => (
                  <option key={c.alias} value={c.alias}>
                    {c.alias} {c.header}
                  </option>
                ))}
              </select>
            </label>
            <p style={{ margin: 0 }} data-testid="tt-group-limit">
              <small>{GROUP_LIMIT_SENTENCE}</small>
            </p>
            {groupLevels.length > 0 && (
              <fieldset data-testid="tt-levels">
                <legend>The two groups</legend>
                {groupLevels.length > 2 && <p data-testid="tt-levels-question">{levelsQuestion(groupLevels.length)}</p>}
                {([0, 1] as const).map((which) => (
                  <label key={which} style={{ display: 'block' }}>
                    {which === 0 ? 'Group 1' : 'Group 2'}{' '}
                    <select
                      value={levels[which] === null ? '' : String(levels[which])}
                      data-testid={`tt-level-${which + 1}`}
                      onChange={(e) => chooseLevel(which, e.target.value)}
                    >
                      <option value="">Choose a level…</option>
                      {/* Rust's own order; the value is the index Rust gave. */}
                      {groupLevels.map((l) => (
                        <option key={l.index} value={String(l.index)}>
                          {l.label} ({l.count})
                        </option>
                      ))}
                    </select>
                  </label>
                ))}
                <p style={{ margin: 0 }}>The mean difference is group 1 minus group 2.</p>
              </fieldset>
            )}
            <fieldset data-testid="tt-variant">
              <legend>{VARIANT_QUESTION}</legend>
              {VARIANT_CHOICES.map((c) => (
                <label key={c.variant} style={{ display: 'block' }}>
                  <input
                    type="radio"
                    name="tt-variant"
                    value={c.variant}
                    checked={variant === c.variant}
                    data-testid={`tt-variant-${c.variant}`}
                    onChange={() => setVariant(c.variant)}
                  />{' '}
                  {c.label}
                </label>
              ))}
            </fieldset>
            {outcomeColumn && offered.length > 0 && (
              <fieldset data-testid="tt-missing-codes">
                <legend>Candidate missing codes</legend>
                {offered.map((s) => (
                  <label key={s.code} style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={ticked.includes(s.code)}
                      data-testid={`tt-code-${s.code}`}
                      onChange={(e) => toggleCode(s.code, e.target.checked)}
                    />{' '}
                    {ttestMissingCodeQuestion(s.code, outcomeColumn.header, s.cells)}
                  </label>
                ))}
              </fieldset>
            )}
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('profile')} data-testid="tt-back-profile">
              Back to the profile
            </Button>
            <Button onClick={() => void check()} disabled={busy || !ready} data-testid="tt-check">
              Check the settings
            </Button>
          </div>
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="tt-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} names={names} testId="tt-validation-errors" />
          <DiagnosticList items={report.missingInformation} names={names} testId="tt-validation-questions" />
          <DiagnosticList items={report.warnings} names={names} testId="tt-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="tt-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="tt-run"
            >
              Run the t-test
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <Card>
          {title}
          <div role="status" aria-live="polite" data-testid="tt-run-status">
            <p style={{ margin: 0 }} data-testid="tt-run-state">
              {RUN_STATE_NAMES[runView.state]}
            </p>
            <p style={{ margin: 0 }} data-testid="tt-elapsed">
              {elapsedSentence(runView.elapsedMs)}
            </p>
          </div>
          {runView.terminal && runView.state !== 'succeeded' && (
            <>
              <p data-testid="tt-run-ended">{ttestRunEndedSentence(runView.state)}</p>
              <DiagnosticList items={runView.diagnostics} names={names} testId="tt-run-diagnostics" />
            </>
          )}
          <div style={{ display: 'flex', gap: 8 }}>
            {!runView.terminal && (
              <Button variant="secondary" onClick={() => void cancel()} disabled={runView.cancelRequested} data-testid="tt-cancel">
                {runView.cancelRequested ? 'Cancelling…' : 'Cancel'}
              </Button>
            )}
            {runView.terminal && runView.state === 'succeeded' && (
              <Button onClick={() => void seeResults()} disabled={busy} data-testid="tt-see-results">
                See the results
              </Button>
            )}
            {runView.terminal && (
              <Button variant="secondary" onClick={() => setStage('settings')} data-testid="tt-run-back">
                Back to settings
              </Button>
            )}
          </div>
        </Card>
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <ResultCard result={result} names={names} />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="tt-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="tt-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer">{TTEST_EXPORT_TEXT}</p>
          <fieldset data-testid="tt-export-choice">
            <legend>What to save</legend>
            {TTEST_EXPORT_CHOICES.map((c) => (
              <label key={c.format} style={{ display: 'block' }}>
                <input
                  type="radio"
                  name="tt-export"
                  value={c.format}
                  checked={format === c.format}
                  data-testid={`tt-export-${c.format}`}
                  onChange={() => setFormat(c.format)}
                />{' '}
                {c.label}
              </label>
            ))}
          </fieldset>
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('results')} data-testid="tt-export-back">
              Back to the results
            </Button>
            <Button onClick={() => void saveExport()} disabled={busy} data-testid="tt-save">
              Save…
            </Button>
          </div>
          {exportNote && (
            <p role="status" data-testid="tt-export-note">
              {exportNote}
            </p>
          )}
        </Card>
      )}
    </div>
  );
};

/** The neutral result card (decision 66): the numbers, what d used, what is
 *  not assessed, and the manifest. No verdict, no stars. */
export const ResultCard: React.FC<{ result: TTestResultView; names: TTestNames }> = ({ result, names }) => {
  const r = result.result;
  const m = result.manifest;
  const [g1, g2] = r.groups;
  const n = shownNumber;
  return (
    <div data-testid="tt-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} names={names} testId="tt-result-notes" />
      <dl data-testid="tt-result-numbers" style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
        <dt>Test</dt>
        <dd data-testid="tt-variant-name">{VARIANT_NAMES[r.variant]}</dd>
        <dt>t</dt>
        <dd className="gds-mono" data-testid="tt-t">
          {n(r.t)}
        </dd>
        <dt>Degrees of freedom</dt>
        <dd className="gds-mono" data-testid="tt-df">
          {n(r.df)}
        </dd>
        <dt>p (two-sided)</dt>
        <dd className="gds-mono" data-testid="tt-p">
          {n(r.p)}
        </dd>
        <dt>Mean difference ({g1.label} minus {g2.label})</dt>
        <dd className="gds-mono" data-testid="tt-mean-difference">
          {n(r.meanDifference)}
        </dd>
        <dt>Standard error</dt>
        <dd className="gds-mono" data-testid="tt-se">
          {n(r.standardError)}
        </dd>
        <dt>{Math.round(r.confidenceLevel * 100)}% confidence interval</dt>
        <dd className="gds-mono" data-testid="tt-ci">
          {n(r.ciLower)} to {n(r.ciUpper)}
        </dd>
        <dt>Cohen&apos;s d</dt>
        <dd>
          <span className="gds-mono" data-testid="tt-d">
            {n(r.cohensD)}
          </span>
          <br />
          <span data-testid="tt-d-standardizer">
            Divided by {STANDARDIZER_NAMES[r.dStandardizer]}: <span className="gds-mono">{n(r.dStandardizerValue)}</span>
          </span>
          <br />
          <span data-testid="tt-d-no-interval">{ttestSentence({ code: 'stats.ttest.cohens_d_has_no_interval' })}</span>
        </dd>
        <dt>SD ratio</dt>
        <dd>
          <span className="gds-mono" data-testid="tt-sd-ratio">
            {statSentence(r.sdRatio)}
          </span>
          <br />
          <small>{SD_RATIO_NOTE}</small>
        </dd>
      </dl>
      <table data-testid="tt-groups" style={{ borderCollapse: 'collapse' }}>
        <caption style={{ textAlign: 'left' }}>The two groups</caption>
        <thead>
          <tr>
            {['Group', 'Level', 'n', 'Mean', 'SD'].map((h) => (
              <th key={h} scope="col" style={{ textAlign: 'left', paddingRight: 12 }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {r.groups.map((g, i) => (
            <tr key={g.level}>
              <th scope="row" style={{ textAlign: 'left' }}>
                {i + 1}
              </th>
              <td>{g.label}</td>
              <td className="gds-mono">{g.n}</td>
              <td className="gds-mono">{n(g.mean)}</td>
              <td className="gds-mono">{n(g.sd)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <section aria-label="Rows left out" data-testid="tt-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e) => (
              <li key={e.code}>{ttestSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="tt-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{ttestSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer">{TTEST_DIGITS_NOTE}</p>
      <aside aria-label="Run manifest" data-testid="tt-manifest">
        <h4 style={{ margin: '8px 0 4px' }}>Run manifest</h4>
        <dl style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
          <dt>Data file (SHA-256)</dt>
          <dd className="gds-mono" data-testid="tt-manifest-input">
            {m.inputs.map(shortHash).join(', ')}
          </dd>
          <dt>Capability</dt>
          <dd data-testid="tt-manifest-capability">
            {m.capabilityId}, version {m.capabilityVersion}
          </dd>
          <dt>Engine version</dt>
          <dd data-testid="tt-manifest-engine">{m.engineVersion}</dd>
          <dt>Rows read</dt>
          <dd className="gds-mono" data-testid="tt-manifest-rows-read">
            {m.rowsRead}
          </dd>
          <dt>Rows used</dt>
          <dd>
            <span className="gds-mono" data-testid="tt-manifest-rows-used">
              {m.rowsUsed}
            </span>
            <br />
            <span>{TTEST_ROWS_USED_NOTE}</span>
          </dd>
          <dt>Time to compute</dt>
          <dd data-testid="tt-manifest-compute-time">{computeTimeSentence(m.elapsedMs)}</dd>
          <dt>Platform</dt>
          <dd data-testid="tt-manifest-platform">{platformName(m.platform)}</dd>
          <dt>Reproducibility</dt>
          <dd data-testid="tt-manifest-reproducibility">{reproducibilitySentence(m.reproducibility)}</dd>
          <dt>Result (SHA-256)</dt>
          <dd className="gds-mono" data-testid="tt-manifest-output">
            {shortHash(m.outputHash)}
          </dd>
        </dl>
      </aside>
    </div>
  );
};

export default TTestFlow;

/** /app/statistics/ttest — the page around the flow. */
export const TTestPage: React.FC<{ bridge?: TTestBridge }> = ({ bridge }) => {
  const navigate = useNavigate();
  const b = useMemo(() => bridge ?? ttestBridge(), [bridge]);
  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="ttest-page">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'statistics', label: 'Statistics', icon: 'Σ', onSelect: () => navigate('/app/statistics') },
            ]}
            activeId="statistics"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title="Independent-samples t-test" />}
      >
        <Panel title="Independent-samples t-test on a data file">
          <TTestFlow bridge={b} />
        </Panel>
      </AppShell>
    </div>
  );
};
