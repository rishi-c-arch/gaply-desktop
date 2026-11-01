// Gaply — the one-way ANOVA (Research OS, Phase 8G-2, decisions 318, 329 and
// 331): an analysis of the user's own data, over the ANOVA's bridge.
//
// Import → Profile → Settings → Validate → Run → Results → Export, as on the
// t-test's screen. Rust owns every decision: this screen passes the path
// once (to open), the slot, the run id and the user's choices, and shows
// what comes back.
//
// The choices: the outcome (a numeric column), the group (any column with 2
// to 50 levels when read as categorical), the levels included (every level
// ticked, at least two kept, sent as null when all are ticked and otherwise
// as the ticked indexes ascending, so one choice gives one spec: decision
// 329), the post hoc tests (none ticked; Gaply does not choose one) and each
// candidate missing code in the outcome, asked as a question.
//
// The results: the group table, the ANOVA table, Welch's ANOVA, Levene's two
// lines, the effect sizes, then each chosen post hoc test's table, in full,
// one test at a time, under a heading that states its row count (decision
// 331). A statistic that is not computable shows its sentence where its
// number would be, and a p below the floor shows as "below" the floor's
// value from the result. Six figures on screen; the CSV keeps every digit.
// No verdict words and no stars (decision 66).
//
// What lives here, and for how long: the file's base name, the headers, the
// level labels, the candidate missing-code values, the chosen settings and
// the result numbers, in this component's state only. Never a row, never
// another cell, never a path, and never browser storage. No label goes into
// a spec. The file is closed, and Rust drops its rows, when the user closes
// it or the screen unmounts.
//
// Behind REACT_APP_FEATURE_STATS_DESCRIBE, which hides unfinished UI and is
// not an authorization boundary.
import React, { useCallback, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../design-system';
import { CELL_PADDING } from '../../design-system/primitives';
import { pickManuscriptPath } from '../common/pickFile';
import { saveTextFile } from '../../utils/saveTextFile';
import { anovaBridge, type AnovaBridge } from './anovaBridge';
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
import type { ColumnProfile, DatasetView, ExportFormat } from './describeTypes';
import type { GroupLevel } from './ttestTypes';
import {
  POST_HOC_TESTS,
  type AnovaResultView,
  type AnovaRunStateView,
  type AnovaSpec,
  type AnovaValidationView,
  type FOutcome,
  type PostHoc,
  type PostHocTable,
} from './anovaTypes';
import { Stage, STATUS_SENTENCES } from './describeWording';
import { shownNumber } from './ttestWording';
import {
  ANOVA_DIGITS_NOTE,
  ANOVA_ENTRY_TEXT,
  ANOVA_EXPORT_CHOICES,
  ANOVA_EXPORT_TEXT,
  ANOVA_GROUP_LIMIT_SENTENCE,
  ANOVA_LEVELS_RULE,
  ANOVA_ROWS_USED_NOTE,
  ANOVA_STAGE_HEADINGS,
  anovaMissingCodeQuestion,
  anovaRefusalSentence,
  anovaRunEndedSentence,
  anovaSentence,
  anovaStatText,
  LEVELS_QUESTION,
  LEVENE_NOTE,
  levelsTooFewSentence,
  MAX_GROUP_LEVELS,
  POST_HOC_CHOICES,
  POST_HOC_QUESTION,
  POST_HOC_STATISTIC,
  postHocHeading,
  postHocPText,
  stepsSentence,
  type AnovaNames,
} from './anovaWording';

const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface AnovaFlowProps {
  bridge: AnovaBridge;
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

const outcomes = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

const groups = (view: DatasetView) =>
  view.profile.columns.filter((c: ColumnProfile) => c.distinct >= 2 && c.distinct <= MAX_GROUP_LEVELS);

/** Decision 329: null when every level is ticked; otherwise the ticked
 *  indexes, ascending. The post hoc tests in the method's order. */
export function buildAnovaSpec(
  view: DatasetView,
  outcome: string,
  group: string,
  all: GroupLevel[],
  ticked: number[],
  postHoc: PostHoc[],
  missingCodes: number[],
  operationId: number
): AnovaSpec {
  const included = all.map((l) => l.index).filter((i) => ticked.includes(i));
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.one_way_anova',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      outcome,
      group,
      levels: included.length === all.length ? null : [...included].sort((a, b) => a - b),
      postHoc: POST_HOC_TESTS.filter((t) => postHoc.includes(t)),
      missingCodes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

const AnovaFlow: React.FC<AnovaFlowProps> = ({
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
  const [group, setGroup] = useState('');
  const [groupLevels, setGroupLevels] = useState<GroupLevel[]>([]);
  const [tickedLevels, setTickedLevels] = useState<number[]>([]);
  const [postHoc, setPostHoc] = useState<PostHoc[]>([]);
  const [ticked, setTicked] = useState<number[]>([]);
  const [report, setReport] = useState<AnovaValidationView | null>(null);
  const [runView, setRunView] = useState<AnovaRunStateView | null>(null);
  const [result, setResult] = useState<AnovaResultView | null>(null);
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const operation = useRef(0);
  const slot = useRef<number | null>(null);

  const heading = useStageFocus(stage);
  useCloseOnUnmount(bridge, slot);

  const outcomeColumn = useMemo(() => view?.profile.columns.find((c) => c.alias === outcome), [view, outcome]);
  const offered = useMemo(() => outcomeColumn?.suspectMissingCodes ?? [], [outcomeColumn]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const names: AnovaNames = useMemo(
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
    return buildAnovaSpec(view as DatasetView, outcome, group, groupLevels, tickedLevels, postHoc, missingCodes, operation.current);
  }, [view, outcome, group, groupLevels, tickedLevels, postHoc, missingCodes]);

  useRunPolling(runView, setRunView, bridge, setError, anovaRefusalSentence, pollMs);

  const act = actWith(setBusy, setError, anovaRefusalSentence);

  const reset = () => {
    setOutcome('');
    setGroup('');
    setGroupLevels([]);
    setTickedLevels([]);
    setPostHoc([]);
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
      const choice = ANOVA_EXPORT_CHOICES.find((c) => c.format === format) ?? ANOVA_EXPORT_CHOICES[0];
      const out = await bridge.export(result.runId, format);
      const saved = await saveExactly(out, choice, save);
      if ('error' in saved) {
        setError(saved.error);
        return;
      }
      setExportNote(saved.note);
    });

  const chooseGroup = (alias: string) =>
    act(async () => {
      setGroup(alias);
      setGroupLevels([]);
      setTickedLevels([]);
      if (alias === '' || slot.current === null) return;
      // The levels in the core's own order; their indexes are what the spec
      // sends. Every level starts ticked (decision 329).
      const read = await bridge.groupLevels(slot.current, alias);
      setGroupLevels(read.levels);
      setTickedLevels(read.levels.map((l) => l.index));
    });
  const toggleLevel = (index: number, on: boolean) =>
    setTickedLevels((t) => (on ? [...t.filter((x) => x !== index), index] : t.filter((x) => x !== index)));
  const togglePostHoc = (test: PostHoc, on: boolean) =>
    setPostHoc((t) => (on ? [...t.filter((x) => x !== test), test] : t.filter((x) => x !== test)));
  const toggleCode = (code: number, on: boolean) => setTicked((t) => (on ? [...t.filter((x) => x !== code), code] : t.filter((x) => x !== code)));

  const title = (
    <h3 className="gds-card__title" ref={heading} tabIndex={-1} data-testid="an-stage-heading">
      {ANOVA_STAGE_HEADINGS[stage]}
    </h3>
  );

  const ready = outcome !== '' && group !== '' && groupLevels.length > 0 && tickedLevels.length >= 2;

  return (
    <div style={{ display: 'grid', gap: 12 }} data-testid="an-flow">
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
        <StageNav prefix="an" stage={stage} />
        {view && (
          <Button variant="secondary" onClick={() => void closeFile()} data-testid="an-close">
            Close this file
          </Button>
        )}
      </div>
      {view && stage !== 'import' && <FileSummary prefix="an" view={view} />}
      <OpenProgressLine prefix="an" rowsRead={openProgress.rowsRead} />
      {error && (
        <p role="alert" data-testid="an-error">
          {error}
        </p>
      )}

      {stage === 'import' && (
        <Card>
          {title}
          <p className="gds-jc__disclaimer" data-testid="an-entry-text">
            {ANOVA_ENTRY_TEXT}
          </p>
          <LimitsLine prefix="an" limits={methodLimits} />
          <Button onClick={() => void choose()} disabled={busy} data-testid="an-choose">
            Choose a data file
          </Button>
        </Card>
      )}

      {stage === 'profile' && view && (
        <Card>
          {title}
          <p data-testid="an-levels-rule">{ANOVA_LEVELS_RULE}</p>
          <ProfileTable prefix="an" view={view} />
          <Button onClick={() => setStage('settings')} data-testid="an-to-settings">
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
              <select value={outcome} data-testid="an-outcome" onChange={(e) => setOutcome(e.target.value)}>
                <option value="">Choose a column…</option>
                {outcomes(view).map((c) => (
                  <option key={c.alias} value={c.alias}>
                    {c.alias} {c.header}
                  </option>
                ))}
              </select>
            </label>
            <label>
              Group (a column with 2 to {MAX_GROUP_LEVELS} levels){' '}
              <select value={group} data-testid="an-group" onChange={(e) => void chooseGroup(e.target.value)}>
                <option value="">Choose a column…</option>
                {groups(view).map((c) => (
                  <option key={c.alias} value={c.alias}>
                    {c.alias} {c.header}
                  </option>
                ))}
              </select>
            </label>
            <p style={{ margin: 0 }} data-testid="an-group-limit">
              <small>{ANOVA_GROUP_LIMIT_SENTENCE}</small>
            </p>
            {groupLevels.length > 0 && (
              <fieldset data-testid="an-levels">
                <legend>{LEVELS_QUESTION}</legend>
                {/* Rust's own order; each box is the index Rust gave. */}
                {groupLevels.map((l) => (
                  <label key={l.index} style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={tickedLevels.includes(l.index)}
                      data-testid={`an-level-${l.index}`}
                      onChange={(e) => toggleLevel(l.index, e.target.checked)}
                    />{' '}
                    {l.label} ({l.count})
                  </label>
                ))}
                {tickedLevels.length < 2 && (
                  <p role="status" data-testid="an-levels-too-few">
                    {levelsTooFewSentence(tickedLevels.length)}
                  </p>
                )}
              </fieldset>
            )}
            <fieldset data-testid="an-post-hoc">
              <legend>{POST_HOC_QUESTION}</legend>
              {POST_HOC_CHOICES.map((c) => (
                <div key={c.test}>
                  <label style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={postHoc.includes(c.test)}
                      data-testid={`an-post-hoc-${c.test}`}
                      aria-describedby={`an-post-hoc-line-${c.test}`}
                      onChange={(e) => togglePostHoc(c.test, e.target.checked)}
                    />{' '}
                    {c.label}
                  </label>
                  <p id={`an-post-hoc-line-${c.test}`} className="gds-jc__disclaimer" style={{ margin: '0 0 0.5em 1.5em' }}>
                    {c.line}
                  </p>
                </div>
              ))}
            </fieldset>
            {outcomeColumn && offered.length > 0 && (
              <fieldset data-testid="an-missing-codes">
                <legend>Candidate missing codes</legend>
                {offered.map((s) => (
                  <label key={s.code} style={{ display: 'block' }}>
                    <input
                      type="checkbox"
                      checked={ticked.includes(s.code)}
                      data-testid={`an-code-${s.code}`}
                      onChange={(e) => toggleCode(s.code, e.target.checked)}
                    />{' '}
                    {anovaMissingCodeQuestion(s.code, outcomeColumn.header, s.cells)}
                  </label>
                ))}
              </fieldset>
            )}
          </div>
          <SettingsActions prefix="an" busy={busy} ready={ready} onBack={() => setStage('profile')} onCheck={() => void check()} />
        </Card>
      )}

      {stage === 'validate' && report && (
        <Card>
          {title}
          <p role="status" data-testid="an-validation-status">
            {STATUS_SENTENCES[report.status]}
          </p>
          <DiagnosticList items={report.errors} sentence={(d) => anovaSentence(d, names)} testId="an-validation-errors" />
          <DiagnosticList items={report.missingInformation} sentence={(d) => anovaSentence(d, names)} testId="an-validation-questions" />
          <DiagnosticList items={report.warnings} sentence={(d) => anovaSentence(d, names)} testId="an-validation-warnings" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="an-back-settings">
              Back to settings
            </Button>
            <Button
              onClick={() => void run()}
              disabled={busy || !(report.status === 'valid' || report.status === 'valid_with_warnings')}
              data-testid="an-run"
            >
              Run the one-way ANOVA
            </Button>
          </div>
        </Card>
      )}

      {stage === 'run' && runView && (
        <RunCard
          prefix="an"
          title={title}
          runView={runView}
          endedSentence={anovaRunEndedSentence}
          progress={
            <p style={{ margin: 0 }} data-testid="an-progress">
              {stepsSentence(runView.stepsDone, runView.stepsTotal)}
            </p>
          }
          diagnostics={<DiagnosticList items={runView.diagnostics} sentence={(d) => anovaSentence(d, names)} testId="an-run-diagnostics" />}
          busy={busy}
          onCancel={() => void cancel()}
          onSeeResults={() => void seeResults()}
          onBack={() => setStage('settings')}
        />
      )}

      {stage === 'results' && result && (
        <Card>
          {title}
          <AnovaResultCard result={result} names={names} />
          <SaveToProject runId={result.runId} testId="an-project-save" />
          <div style={{ display: 'flex', gap: 8 }}>
            <Button variant="secondary" onClick={() => setStage('settings')} data-testid="an-results-back">
              Change the settings
            </Button>
            <Button onClick={() => setStage('export')} data-testid="an-to-export">
              Continue to export
            </Button>
          </div>
        </Card>
      )}

      {stage === 'export' && result && (
        <ExportCard
          prefix="an"
          title={title}
          text={ANOVA_EXPORT_TEXT}
          choices={ANOVA_EXPORT_CHOICES}
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

/** An F test's line: its four numbers, or its sentence where they would be. */
const FTestRow: React.FC<{ label: string; test: FOutcome; testId: string }> = ({ label, test, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
      {label}
    </th>
    {'computed' in test ? (
      <>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.f)}</td>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.df1)}</td>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.df2)}</td>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.p)}</td>
      </>
    ) : (
      <td colSpan={4} style={CELL_PADDING} data-testid={`${testId}-not-computable`}>
        {anovaSentence({ code: test.not_computable })}
      </td>
    )}
  </tr>
);

/** One chosen test's table, every pair, under a heading with its row count. */
const PostHocTableView: React.FC<{ table: PostHocTable; label: (level: number) => string }> = ({ table, label }) => {
  const pairs = 'computed' in table.pairs ? table.pairs.computed : null;
  return (
    <section data-testid={`an-post-hoc-table-${table.test}`} aria-label={postHocHeading(table.test, pairs ? pairs.length : 0)}>
      <h4 style={{ margin: '8px 0 4px' }} data-testid={`an-post-hoc-heading-${table.test}`}>
        {postHocHeading(table.test, pairs ? pairs.length : 0)}
      </h4>
      {pairs === null ? (
        <p data-testid={`an-post-hoc-not-computable-${table.test}`}>
          {anovaSentence({ code: (table.pairs as { not_computable: string }).not_computable })}
        </p>
      ) : (
        <table style={{ borderCollapse: 'collapse' }}>
          <thead>
            <tr>{['Pair', 'Difference', 'SE', POST_HOC_STATISTIC[table.test], 'df', 'p', '95% interval'].map(th)}</tr>
          </thead>
          <tbody>
            {pairs.map((p, i) => (
              <tr key={`${p.first}-${p.second}-${i}`}>
                <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
                  {label(p.first)} minus {label(p.second)}
                </th>
                {'computed' in p.outcome ? (
                  <>
                    <td className="gds-mono" style={CELL_PADDING}>{n(p.outcome.computed.difference)}</td>
                    <td className="gds-mono" style={CELL_PADDING}>{n(p.outcome.computed.se)}</td>
                    <td className="gds-mono" style={CELL_PADDING}>{n(p.outcome.computed.statistic)}</td>
                    <td className="gds-mono" style={CELL_PADDING}>{n(p.outcome.computed.df)}</td>
                    <td className="gds-mono" style={CELL_PADDING}>{postHocPText(p.outcome.computed.p)}</td>
                    <td className="gds-mono" style={CELL_PADDING}>
                      {n(p.outcome.computed.ciLower)} to {n(p.outcome.computed.ciUpper)}
                    </td>
                  </>
                ) : (
                  <td colSpan={6} style={CELL_PADDING}>
                    {anovaSentence({ code: p.outcome.not_computable })}
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
};

/** The neutral result card (decision 66): every table, what is not
 *  computable in its own words, what is not assessed, and the manifest. */
export const AnovaResultCard: React.FC<{ result: AnovaResultView; names: AnovaNames }> = ({ result, names }) => {
  const r = result.result;
  const label = (level: number) => r.groups.find((g) => g.level === level)?.label ?? names.level(level);
  const [shown, setShown] = useState<PostHoc | null>(r.postHoc.length > 0 ? r.postHoc[0].test : null);
  const table = r.postHoc.find((t) => t.test === shown) ?? null;
  return (
    <div data-testid="an-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => anovaSentence(d, names)} testId="an-result-notes" />
      <table data-testid="an-groups" style={{ borderCollapse: 'collapse' }}>
        <caption style={{ textAlign: 'left' }}>The groups</caption>
        <thead>
          <tr>{['Level', 'n', 'Mean', 'SD', 'SE', '95% interval of the mean', 'Min', 'Max'].map(th)}</tr>
        </thead>
        <tbody>
          {r.groups.map((g) => (
            <tr key={g.level}>
              <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>
                {g.label}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>{g.n}</td>
              <td className="gds-mono" style={CELL_PADDING}>{n(g.mean)}</td>
              <td className="gds-mono" style={CELL_PADDING}>{n(g.sd)}</td>
              <td className="gds-mono" style={CELL_PADDING}>{n(g.se)}</td>
              <td className="gds-mono" style={CELL_PADDING}>
                {n(g.ciLower)} to {n(g.ciUpper)}
              </td>
              <td className="gds-mono" style={CELL_PADDING}>{n(g.min)}</td>
              <td className="gds-mono" style={CELL_PADDING}>{n(g.max)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <section aria-label="The ANOVA table" data-testid="an-anova-table">
        <h4 style={{ margin: '8px 0 4px' }}>The ANOVA table</h4>
        {'computed' in r.table ? (
          <table style={{ borderCollapse: 'collapse' }}>
            <thead>
              <tr>{['Source', 'Sum of squares', 'df', 'Mean square', 'F', 'p'].map(th)}</tr>
            </thead>
            <tbody>
              <tr>
                <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>Between groups</th>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.ssBetween)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.dfBetween)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.msBetween)}</td>
                <td style={CELL_PADDING} data-testid="an-f">{anovaStatText(r.table.computed.f)}</td>
                <td style={CELL_PADDING} data-testid="an-p">{anovaStatText(r.table.computed.p)}</td>
              </tr>
              <tr>
                <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>Within groups</th>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.ssWithin)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.dfWithin)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.msWithin)}</td>
                <td style={CELL_PADDING} />
                <td style={CELL_PADDING} />
              </tr>
              <tr>
                <th scope="row" style={{ textAlign: 'left', ...CELL_PADDING }}>Total</th>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.ssTotal)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(r.table.computed.dfBetween + r.table.computed.dfWithin)}</td>
                <td style={CELL_PADDING} />
                <td style={CELL_PADDING} />
                <td style={CELL_PADDING} />
              </tr>
            </tbody>
          </table>
        ) : (
          <p data-testid="an-anova-table-not-computable">{anovaSentence({ code: r.table.not_computable })}</p>
        )}
      </section>
      <section aria-label="Welch's ANOVA and Levene's tests" data-testid="an-f-tests">
        <h4 style={{ margin: '8px 0 4px' }}>Welch&apos;s ANOVA and Levene&apos;s tests</h4>
        <table style={{ borderCollapse: 'collapse' }}>
          <thead>
            <tr>{['Test', 'F', 'df1', 'df2', 'p'].map(th)}</tr>
          </thead>
          <tbody>
            <FTestRow label="Welch's ANOVA" test={r.welch} testId="an-welch" />
            <FTestRow label="Levene's test, based on the mean" test={r.leveneMean} testId="an-levene-mean" />
            <FTestRow label="Levene's test, based on the median" test={r.leveneMedian} testId="an-levene-median" />
          </tbody>
        </table>
        <p style={{ margin: 0 }}>
          <small>{LEVENE_NOTE}</small>
        </p>
      </section>
      <dl data-testid="an-effect-sizes" style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
        <dt>Eta squared</dt>
        <dd data-testid="an-eta">{anovaStatText(r.etaSquared)}</dd>
        <dt>Omega squared</dt>
        <dd data-testid="an-omega">{anovaStatText(r.omegaSquared)}</dd>
      </dl>
      {r.postHoc.length > 0 && (
        <section aria-label="Post hoc tests" data-testid="an-post-hoc-results">
          <h4 style={{ margin: '8px 0 4px' }}>Post hoc tests</h4>
          {r.postHoc.length > 1 && (
            <div role="group" aria-label="Which post hoc test to show" style={{ display: 'flex', gap: 8 }}>
              {r.postHoc.map((t) => (
                <Button
                  key={t.test}
                  variant={t.test === shown ? 'primary' : 'secondary'}
                  aria-pressed={t.test === shown}
                  onClick={() => setShown(t.test)}
                  data-testid={`an-show-${t.test}`}
                >
                  {postHocHeading(t.test, 'computed' in t.pairs ? t.pairs.computed.length : 0)}
                </Button>
              ))}
            </div>
          )}
          {table && <PostHocTableView table={table} label={label} />}
        </section>
      )}
      {r.notes.length > 0 && (
        <ul data-testid="an-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{anovaSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="an-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e) => (
              <li key={e.code}>{anovaSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="an-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{anovaSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer">{ANOVA_DIGITS_NOTE}</p>
      <ManifestPanel prefix="an" manifest={result.manifest} rowsUsedNote={ANOVA_ROWS_USED_NOTE} />
    </div>
  );
};

export default AnovaFlow;

/** /app/statistics/anova — the page around the flow. */
export const AnovaPage: React.FC<{ bridge?: AnovaBridge }> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? anovaBridge(), [bridge]);
  return (
    <MethodPage testId="anova-page" header="One-way ANOVA" panelTitle="One-way ANOVA on a data file">
      <AnovaFlow bridge={b} />
    </MethodPage>
  );
};
