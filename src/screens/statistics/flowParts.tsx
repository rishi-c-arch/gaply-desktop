// Gaply — the parts of a method's screen on the user's own data that more
// than one method shows (Research OS, Phase 8D-2b, decision 268), moved here
// from the independent t-test's screen (TTestFlow.tsx) one at a time, not
// copied. Each takes its test-id prefix and its sentences as props, so the
// t-test keeps its markup and its tt- ids, and a new method passes its own.
//
// What they show is what the method's screen gives them: the file's base
// name, its headers and its profile, a run's state and codes, the manifest's
// hashes and counts. Nothing here is kept, logged or sent.
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { CELL_PADDING } from '../../design-system/primitives';
import {
  computeTimeSentence,
  elapsedSentence,
  platformName,
  reproducibilitySentence,
  RUN_STATE_NAMES,
  shortHash,
  STAGES,
  type Stage,
} from './describeWording';
import type { DatasetView, ExportFormat, MethodLimitsView, Platform, Reproducibility, RunState } from './describeTypes';
import { megabytes } from '../statsverifier/l1/l1Wording';
import { savableOnScreen, SaveToProject } from './projects/SaveToProject';

const TIERS = ['8gb', '16gb', '32gb'];
const LIMIT_NUMBERS = ['maxRows', 'maxColumns', 'maxCells', 'maxFileBytes', 'maxTextBytes', 'maxWorkbookBytes'] as const;

/** Whether an answer of stats_desc_limits has MethodLimitsView's shape: a
 *  tier word, a boolean, and six whole numbers (Research OS, Phase 8F-2,
 *  decision 289). A refusal, null or anything else is not one. */
export function isMethodLimitsView(v: unknown): v is MethodLimitsView {
  if (!v || typeof v !== 'object') return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.tier === 'string' &&
    TIERS.includes(o.tier) &&
    typeof o.memoryKnown === 'boolean' &&
    LIMIT_NUMBERS.every((k) => typeof o[k] === 'number' && Number.isInteger(o[k]) && (o[k] as number) >= 0)
  );
}

/** The limits line's words (decision 289): the numbers as raw integers, as
 *  the refusal sentences write them, and the file's bytes in megabytes. */
export function limitsSentence(l: MethodLimitsView): string {
  const line =
    `On this computer, Gaply opens a file of up to ${l.maxRows} rows, ${l.maxColumns} columns and ${l.maxCells} cells ` +
    `(rows times columns), and of up to ${megabytes(l.maxFileBytes)}. A file near these limits takes a few seconds to open.`;
  return l.memoryKnown ? line : `${line} Gaply could not read this computer's memory, so it uses its smallest limits.`;
}

/** The open's progress line's words (decision 290): a count only. */
export function openProgressSentence(rowsRead: number): string {
  return `Opening the file: ${rowsRead} rows read. After the last row, Gaply builds the table and its profile, which takes a few more seconds.`;
}

/** This computer's limits, on a method's Import stage; nothing when there is
 *  no view. Test id `${prefix}-limits`. */
export const LimitsLine: React.FC<{ prefix: string; limits: MethodLimitsView | null }> = ({ prefix, limits }) =>
  limits ? (
    <p className="gds-jc__disclaimer" data-testid={`${prefix}-limits`}>
      {limitsSentence(limits)}
    </p>
  ) : null;

/** The rows read by the open in flight; nothing before a count arrives or
 *  once the open has settled. Test id `${prefix}-open-progress`. */
export const OpenProgressLine: React.FC<{ prefix: string; rowsRead: number | null }> = ({ prefix, rowsRead }) =>
  rowsRead === null ? null : (
    <p role="status" data-testid={`${prefix}-open-progress`} style={{ margin: 0 }}>
      {openProgressSentence(rowsRead)}
    </p>
  );

/** A list of findings, each in its method's own sentence; nothing for none. */
export function DiagnosticList<T>({
  items,
  sentence,
  testId,
}: {
  items: T[];
  sentence: (d: T) => string;
  testId: string;
}): React.ReactElement | null {
  return items.length === 0 ? null : (
    <ul data-testid={testId} style={{ margin: 0 }}>
      {items.map((d, i) => (
        <li key={i}>{sentence(d)}</li>
      ))}
    </ul>
  );
}

/** The seven steps, the current one marked. Test id `${prefix}-steps`. */
export const StageNav: React.FC<{ prefix: string; stage: Stage }> = ({ prefix, stage }) => (
  <nav aria-label="Steps" data-testid={`${prefix}-steps`} style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
    {STAGES.map((s) => (
      <span key={s.id} aria-current={s.id === stage ? 'step' : undefined} style={{ fontWeight: s.id === stage ? 700 : 400 }}>
        {s.label}
      </span>
    ))}
  </nav>
);

/** The open file's line: its base name, rows and columns. Never a path.
 *  Test id `${prefix}-file-summary`. */
export const FileSummary: React.FC<{ prefix: string; view: DatasetView }> = ({ prefix, view }) => (
  <p data-testid={`${prefix}-file-summary`} style={{ margin: 0 }}>
    <span className="gds-mono">{view.displayName}</span>: {view.profile.rows} rows, {view.profile.columns.length}{' '}
    {view.profile.columns.length === 1 ? 'column' : 'columns'}.
  </p>
);

/** The file's profile: each column's alias and header, kind, counts and up to
 *  twelve levels. Test id `${prefix}-profile`. */
export const ProfileTable: React.FC<{ prefix: string; view: DatasetView }> = ({ prefix, view }) => (
  <table data-testid={`${prefix}-profile`} style={{ borderCollapse: 'collapse' }}>
    <thead>
      <tr>
        {['Column', 'Kind', 'n', 'Missing', 'Levels'].map((h) => (
          <th key={h} scope="col" style={{ textAlign: 'left', ...CELL_PADDING }}>
            {h}
          </th>
        ))}
      </tr>
    </thead>
    <tbody>
      {view.profile.columns.map((c) => (
        <tr key={c.alias}>
          <td style={CELL_PADDING}>
            <span className="gds-mono">{c.alias}</span> {c.header}
          </td>
          <td style={CELL_PADDING}>{c.kind === 'numeric' ? 'Numeric' : c.kind === 'categorical' ? 'Categorical' : 'Empty'}</td>
          <td className="gds-mono" style={CELL_PADDING}>{c.n}</td>
          <td className="gds-mono" style={CELL_PADDING}>{c.missing}</td>
          <td style={CELL_PADDING}>
            {(c.levels ?? []).slice(0, 12).map((l) => `${l.label} (${l.count})`).join(', ')}
            {(c.levels?.length ?? 0) > 12 ? `, and ${(c.levels?.length ?? 0) - 12} more` : ''}
          </td>
        </tr>
      ))}
    </tbody>
  </table>
);

/** The settings' two buttons. Check the settings waits for the method's own
 *  readiness rule, `ready`, and for no action to be going. Test ids
 *  `${prefix}-back-profile` and `${prefix}-check`. */
export const SettingsActions: React.FC<{
  prefix: string;
  busy: boolean;
  ready: boolean;
  onBack: () => void;
  onCheck: () => void;
}> = ({ prefix, busy, ready, onBack, onCheck }) => (
  <div style={{ display: 'flex', gap: 8 }}>
    <Button variant="secondary" onClick={onBack} data-testid={`${prefix}-back-profile`}>
      Back to the profile
    </Button>
    <Button onClick={onCheck} disabled={busy || !ready} data-testid={`${prefix}-check`}>
      Check the settings
    </Button>
  </div>
);

/** The run's card: its state and time; once it has ended without a result,
 *  why, the method's diagnostics and, for a failed run, Save to a project…
 *  (decision 140); Cancel while it goes, See the results once it succeeded.
 *  Test ids `${prefix}-run-status`, `-run-state`, `-elapsed`, `-run-ended`,
 *  `-project-save`, `-cancel`, `-see-results` and `-run-back`. */
export const RunCard: React.FC<{
  prefix: string;
  title: React.ReactNode;
  runView: {
    runId: number;
    state: RunState;
    terminal: boolean;
    elapsedMs: number;
    cancelRequested: boolean;
    diagnostics: Array<{ severity: string }>;
  };
  endedSentence: (state: string) => string;
  diagnostics: React.ReactNode;
  busy: boolean;
  onCancel: () => void;
  onSeeResults: () => void;
  onBack: () => void;
}> = ({ prefix, title, runView, endedSentence, diagnostics, busy, onCancel, onSeeResults, onBack }) => (
  <Card>
    {title}
    <div role="status" aria-live="polite" data-testid={`${prefix}-run-status`}>
      <p style={{ margin: 0 }} data-testid={`${prefix}-run-state`}>
        {RUN_STATE_NAMES[runView.state]}
      </p>
      <p style={{ margin: 0 }} data-testid={`${prefix}-elapsed`}>
        {elapsedSentence(runView.elapsedMs)}
      </p>
    </div>
    {runView.terminal && runView.state !== 'succeeded' && (
      <>
        <p data-testid={`${prefix}-run-ended`}>{endedSentence(runView.state)}</p>
        {diagnostics}
        {/* A failed run is saved where its failure is shown (decision 140). */}
        {runView.terminal && savableOnScreen(runView) && <SaveToProject runId={runView.runId} testId={`${prefix}-project-save`} />}
      </>
    )}
    <div style={{ display: 'flex', gap: 8 }}>
      {!runView.terminal && (
        <Button variant="secondary" onClick={onCancel} disabled={runView.cancelRequested} data-testid={`${prefix}-cancel`}>
          {runView.cancelRequested ? 'Cancelling…' : 'Cancel'}
        </Button>
      )}
      {runView.terminal && runView.state === 'succeeded' && (
        <Button onClick={onSeeResults} disabled={busy} data-testid={`${prefix}-see-results`}>
          See the results
        </Button>
      )}
      {runView.terminal && (
        <Button variant="secondary" onClick={onBack} data-testid={`${prefix}-run-back`}>
          Back to settings
        </Button>
      )}
    </div>
  </Card>
);

/** The export's card: the method's own text and file choices, the choice,
 *  and Save… through the save dialog. Test ids `${prefix}-export-choice`,
 *  `-export-{format}`, `-export-back`, `-save` and `-export-note`. */
export const ExportCard: React.FC<{
  prefix: string;
  title: React.ReactNode;
  text: string;
  choices: ReadonlyArray<{ format: ExportFormat; label: string }>;
  format: ExportFormat;
  onFormat: (format: ExportFormat) => void;
  busy: boolean;
  onBack: () => void;
  onSave: () => void;
  note: string | null;
}> = ({ prefix, title, text, choices, format, onFormat, busy, onBack, onSave, note }) => (
  <Card>
    {title}
    <p className="gds-jc__disclaimer">{text}</p>
    <fieldset data-testid={`${prefix}-export-choice`}>
      <legend>What to save</legend>
      {choices.map((c) => (
        <label key={c.format} style={{ display: 'block' }}>
          <input
            type="radio"
            name={`${prefix}-export`}
            value={c.format}
            checked={format === c.format}
            data-testid={`${prefix}-export-${c.format}`}
            onChange={() => onFormat(c.format)}
          />{' '}
          {c.label}
        </label>
      ))}
    </fieldset>
    <div style={{ display: 'flex', gap: 8 }}>
      <Button variant="secondary" onClick={onBack} data-testid={`${prefix}-export-back`}>
        Back to the results
      </Button>
      <Button onClick={onSave} disabled={busy} data-testid={`${prefix}-save`}>
        Save…
      </Button>
    </div>
    {note && (
      <p role="status" data-testid={`${prefix}-export-note`}>
        {note}
      </p>
    )}
  </Card>
);

/** A run manifest: hashes, versions and counts, never a header, label, cell,
 *  name or path. The method says what its rows used are. Test ids
 *  `${prefix}-manifest` and `-manifest-input`, `-capability`, `-engine`,
 *  `-rows-read`, `-rows-used`, `-compute-time`, `-platform`,
 *  `-reproducibility` and `-output`. */
export const ManifestPanel: React.FC<{
  prefix: string;
  manifest: {
    inputs: string[];
    capabilityId: string;
    capabilityVersion: number;
    engineVersion: number;
    rowsRead: number;
    rowsUsed: number;
    elapsedMs: number;
    platform: Platform;
    reproducibility: Reproducibility;
    outputHash: string;
  };
  rowsUsedNote: string;
}> = ({ prefix, manifest: m, rowsUsedNote }) => (
  <aside aria-label="Run manifest" data-testid={`${prefix}-manifest`}>
    <h4 style={{ margin: '8px 0 4px' }}>Run manifest</h4>
    <dl style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
      <dt>Data file (SHA-256)</dt>
      <dd className="gds-mono" data-testid={`${prefix}-manifest-input`}>
        {m.inputs.map(shortHash).join(', ')}
      </dd>
      <dt>Capability</dt>
      <dd data-testid={`${prefix}-manifest-capability`}>
        {m.capabilityId}, version {m.capabilityVersion}
      </dd>
      <dt>Engine version</dt>
      <dd data-testid={`${prefix}-manifest-engine`}>{m.engineVersion}</dd>
      <dt>Rows read</dt>
      <dd className="gds-mono" data-testid={`${prefix}-manifest-rows-read`}>
        {m.rowsRead}
      </dd>
      <dt>Rows used</dt>
      <dd>
        <span className="gds-mono" data-testid={`${prefix}-manifest-rows-used`}>
          {m.rowsUsed}
        </span>
        <br />
        <span>{rowsUsedNote}</span>
      </dd>
      <dt>Time to compute</dt>
      <dd data-testid={`${prefix}-manifest-compute-time`}>{computeTimeSentence(m.elapsedMs)}</dd>
      <dt>Platform</dt>
      <dd data-testid={`${prefix}-manifest-platform`}>{platformName(m.platform)}</dd>
      <dt>Reproducibility</dt>
      <dd data-testid={`${prefix}-manifest-reproducibility`}>{reproducibilitySentence(m.reproducibility)}</dd>
      <dt>Result (SHA-256)</dt>
      <dd className="gds-mono" data-testid={`${prefix}-manifest-output`}>
        {shortHash(m.outputHash)}
      </dd>
    </dl>
  </aside>
);

/** The page around a method's flow, at its own route: the rail back to the
 *  home and to Statistics, the method's header and panel titles. */
export const MethodPage: React.FC<{ testId: string; header: string; panelTitle: string; children: React.ReactNode }> = ({
  testId,
  header,
  panelTitle,
  children,
}) => {
  const navigate = useNavigate();
  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid={testId}>
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
        header={<HeaderBar title={header} />}
      >
        <Panel title={panelTitle}>{children}</Panel>
      </AppShell>
    </div>
  );
};
