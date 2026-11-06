// Gaply — the Statistics window (Research OS, Phase 8P, decisions 449 to 453;
// ADR-012 Parts 1, 2, 4 and 5). A full-window screen of its own, behind
// RequireWorkspace and the statsLayout flag, which hides unfinished UI and is
// never a boundary: Rust checks every command whatever this shows. It holds no
// business logic. It reads Rust's state, and shows what comes back.
//
// Nothing here is kept but in memory, logged or sent. A path goes to the open
// once and is never held. A column's name and the file's base name are shown
// on the user's own screen only.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Menu, Tabs } from '../../../design-system';
import { isFeatureEnabled } from '../../../config/featureFlags';
import { pickManuscriptPath } from '../../common/pickFile';
import { defaultRunBridges, useWorkspaceSwitch, type RunBridges } from '../../workspace/useWorkspaceSwitch';
import { workspaceBridge, type WorkspaceBridge } from '../../workspace/workspaceBridge';
import { describeBridge, type DescribeBridge } from '../describeBridge';
import { saveTextFile } from '../../../utils/saveTextFile';
import type { DatasetView } from '../describeTypes';
import { useMethodLimits } from '../flowHooks';
import { FileSummary, LimitsLine, OpenProgressLine } from '../flowParts';
import type { MethodKind } from '../methods';
import { projectBridge, type ProjectBridge } from '../projects/projectBridge';
import { projectsShown } from '../projects/projectGate';
import { noteCurrent, noteWorkspaceState as noteProjectWorkspaceState, useOpenProject } from '../projects/projectSession';
import { readMethodLimits } from '../statsInvoke';
import { capabilityBridge, type CapabilityBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import './StatisticsWindow.css';
import { DataGrid } from './DataGrid';
import { GridTiming, timingShown } from './gridTiming';
import { HelpDialog } from './HelpDialog';
import { METHOD_DIALOGS, type StartedRun } from './methodOutput';
import { AskFirst } from './MethodDialog';
import { entryHeadingId, Output, OutputWatchers } from './Output';
import { holdsUnsavedWork, MAX_OUTPUT, runIsGoing, statisticsSession, useStatisticsSession, type NewOutputEntry } from './statisticsSession';
import { TimingLine } from './TimingLine';
import { useWindowFile } from './useWindowFile';
import { VariableView } from './VariableView';
import { openBlocked, windowMenus, type HelpTopic } from './windowMenus';
import {
  ASK_CLOSE,
  ASK_OPEN,
  AS_READ_LINE,
  FOOTER_PHRASES,
  MENU_WORDS,
  NO_PROJECT_LINE,
  OPEN_BUTTON,
  outputShownSentence,
  preparedNote,
  PROJECT_LINE_PREFIX,
  STATE_UNREAD,
  TABS,
  VARIABLE_EMPTY,
  WINDOW_TITLE,
} from './windowWording';

/** The eighth copy of the picker's list: each flow's is private, a vitest pins
 *  this one equal to each, and the copies merge in 8V (decision 450). */
export const DATA_EXTENSIONS = ['csv', 'tsv', 'xlsx', 'xls', 'xlsb', 'xlsm', 'ods', 'sav', 'mat'];

export interface StatisticsWindowProps {
  workspace?: WorkspaceBridge;
  /** Opens and closes the session's file. */
  files?: Pick<DescribeBridge, 'open' | 'close'>;
  /** Reads the grid's windows of cells; describeBridge's by default. */
  cells?: Pick<DescribeBridge, 'cells'>;
  capabilities?: CapabilityBridge;
  projects?: ProjectBridge;
  /** Reads and cancels a run that blocks a switch, each through its kind's. */
  runs?: Partial<RunBridges>;
  /** The native file picker: an absolute path, or null when cancelled. The
   *  path goes to the open and is never kept. */
  pickPath?: () => Promise<string | null>;
  /** This computer's limits, read once; stats_desc_limits by default. */
  limits?: () => Promise<unknown>;
  /** Forgets a removed result's record; describeBridge's by default. */
  forget?: Pick<DescribeBridge, 'forgetRun'>;
  /** Writes an export through the save dialog. */
  save?: (name: string, text: string, mime: string) => Promise<string | null>;
  /** How often a run's state is read while it runs. */
  runPollMs?: number;
  /** How often the workspace's state is read. */
  pollMs?: number;
}

const AboveTheGrid: React.FC<{ view: DatasetView | null }> = ({ view }) => {
  const note = view ? preparedNote(view.profile.columns.filter((c) => c.prepared === true).length) : null;
  return (
    <div className="sw-line" data-testid="sw-line">
      {view && (
        <div className="sw-line__file" title={view.displayName}>
          <FileSummary prefix="sw" view={view} />
          {note && <span data-testid="sw-prepared-note"> {note}</span>}
        </div>
      )}
      <p className="sw-line__read" data-testid="sw-as-read">
        {AS_READ_LINE}
      </p>
    </div>
  );
};

const ProjectLine: React.FC = () => {
  const open = useOpenProject();
  return (
    <p className="sw-project" data-testid="sw-project-line" title={open?.name}>
      {open ? (
        <>
          {PROJECT_LINE_PREFIX}
          <bdi>{open.name}</bdi>
        </>
      ) : (
        NO_PROJECT_LINE
      )}
    </p>
  );
};

const defaultPick = () => pickManuscriptPath(DATA_EXTENSIONS, 'Data file');

export const StatisticsWindow: React.FC<StatisticsWindowProps> = ({
  workspace: givenWorkspace,
  files: givenFiles,
  cells,
  capabilities: givenCapabilities,
  projects: givenProjects,
  runs: givenRuns,
  pickPath = defaultPick,
  limits: loadLimits = readMethodLimits,
  pollMs = 1000,
  forget: givenForget,
  save = saveTextFile,
  runPollMs = 250,
}) => {
  const navigate = useNavigate();
  const workspace = useMemo(() => givenWorkspace ?? workspaceBridge(), [givenWorkspace]);
  const files = useMemo(() => givenFiles ?? describeBridge(), [givenFiles]);
  const capabilities = useMemo(() => givenCapabilities ?? capabilityBridge(), [givenCapabilities]);
  const projects = useMemo(() => givenProjects ?? projectBridge(), [givenProjects]);
  const runs = useMemo(() => defaultRunBridges(givenRuns), [givenRuns]);
  const forget = useMemo(() => givenForget ?? describeBridge(), [givenForget]);
  const timing = useMemo(() => (timingShown() ? new GridTiming() : undefined), []);
  const session = useStatisticsSession();
  const { view } = session;
  const [message, setMessage] = useState<string | null>(null);
  const [runGoing, setRunGoing] = useState(false);
  const [list, setList] = useState<CapabilityView[] | null>(null);
  const [listUnread, setListUnread] = useState(false);
  const [help, setHelp] = useState<HelpTopic | null>(null);
  const [dialogKind, setDialogKind] = useState<MethodKind | null>(null);
  const [asking, setAsking] = useState<'close' | 'open' | null>(null);
  const [tabs, setTabs] = useState<{ key: number; first?: string }>({ key: 0 });
  const [outputStatus, setOutputStatus] = useState<string | null>(null);
  // Where the focus goes as a dialog closes: the new entry's heading after a run, else its opener.
  const started = useRef<number | null>(null);
  const afterDialog = useRef<(() => HTMLElement | null) | null>(() => (started.current === null ? null : document.getElementById(entryHeadingId(started.current))));
  const limits = useMethodLimits(loadLimits);
  const showProjects = projectsShown();
  const { choose, closeFile, rowsRead, error } = useWindowFile({ bridge: files, pickPath });
  const { leave, dialog: switchDialog } = useWorkspaceSwitch({ bridge: workspace, runs, pollMs, setMessage });

  // What the window knows of the workspace, when it appears and every second:
  // whether a run is going, and that Statistics is still active (decision 450).
  useEffect(() => {
    let live = true;
    const read = async () => {
      try {
        const s = await workspace.state();
        statisticsSession.noteWorkspaceState(s);
        noteProjectWorkspaceState(s);
        if (!live) return;
        setRunGoing(s.activeRun !== null);
        setMessage((m) => (m === STATE_UNREAD ? null : m));
      } catch {
        if (live) setMessage(STATE_UNREAD);
      }
    };
    void read();
    const t = setInterval(read, pollMs);
    return () => {
      live = false;
      clearInterval(t);
    };
  }, [workspace, pollMs]);

  // What Rust says may run, as the home reads it (StatisticsHome.tsx).
  useEffect(() => {
    let live = true;
    capabilities.list().then(
      (l) => {
        if (live) setList(l);
      },
      () => {
        if (live) setListUnread(true);
      }
    );
    return () => {
      live = false;
    };
  }, [capabilities]);

  // Which project is open: Rust's answer, asked when the window appears
  // (decision 148), through the project bridge.
  useEffect(() => {
    if (!showProjects) return;
    let live = true;
    void Promise.allSettled([projects.current(), projects.list()]).then(([current, entries]) => {
      if (live) noteCurrent(current.status === 'fulfilled' ? current.value : null, entries.status === 'fulfilled' ? entries.value : null);
    });
    return () => {
      live = false;
    };
  }, [projects, showProjects]);

  // Rust's word, or an entry of this window's whose run has not ended.
  const going = runGoing || session.output.some(runIsGoing);
  const outputFull = session.output.length >= MAX_OUTPUT;

  const onStarted = (s: StartedRun) => {
    if (dialogKind === null || view === null || session.slot === null) return;
    const number = statisticsSession.addEntry({ kind: dialogKind, view, slot: session.slot, settings: s.settings, runId: s.runId, run: s.run } as NewOutputEntry);
    if (number === null) return;
    started.current = number;
    setOutputStatus(outputShownSentence(number));
    setTabs((t) => ({ key: t.key + 1, first: 'output' }));
    setDialogKind(null);
  };

  const menus = windowMenus({
    fileOpen: view !== null,
    runGoing: going,
    outputFull,
    busy: session.busy,
    capabilities: list,
    projects: showProjects,
    statsCheck: isFeatureEnabled('statsCheck'),
    workspaces: isFeatureEnabled('workspaces'),
    actions: {
      open: () => (view !== null && holdsUnsavedWork(session) ? setAsking('open') : void choose()),
      close: () => (holdsUnsavedWork(session) ? setAsking('close') : void closeFile()),
      go: (path) => navigate(path),
      switchWorkspace: () => void leave(),
      help: setHelp,
      method: (kind) => {
        started.current = null;
        setDialogKind(kind as MethodKind);
      },
    },
  });

  const openWhy = openBlocked({ runGoing: going, busy: session.busy });
  const MethodDialogFor = dialogKind ? METHOD_DIALOGS[dialogKind] : undefined;

  const body = (content: React.ReactNode, full?: 'grid' | 'output') => (
    <>
      <AboveTheGrid view={view} />
      <div className={full ? `sw-body sw-body--${full}` : 'sw-body'} data-testid="sw-body" tabIndex={full ? undefined : 0}>
        <p className="sw-message" role="status" aria-live="polite" data-testid="sw-message">
          {message ?? ''}
        </p>
        {error && (
          <p className="sw-message" role="alert" data-testid="sw-error">
            {error}
          </p>
        )}
        {session.notice && (
          <p className="sw-message" role="status" data-testid="sw-notice">
            {session.notice}
          </p>
        )}
        <OpenProgressLine prefix="sw" rowsRead={rowsRead} />
        {content}
      </div>
    </>
  );

  const dataTab = view ? (
    <DataGrid view={view} cells={cells} timing={timing} />
  ) : (
    <section data-testid="sw-data-empty" style={{ display: 'grid', gap: 8, justifyItems: 'start' }}>
      <Button data-testid="sw-open" onClick={() => void choose()} disabled={openWhy !== undefined}>
        {OPEN_BUTTON}
      </Button>
      {openWhy && <p data-testid="sw-open-blocked">{openWhy}</p>}
      <LimitsLine prefix="sw" limits={limits} />
    </section>
  );

  return (
    <div className="gds-root sw-window" data-testid="statistics-window">
      <h1 className="sw-sr">{WINDOW_TITLE}</h1>
      <header className="sw-top" data-testid="sw-top">
        <Menu label={MENU_WORDS.bar} menus={menus} />
        {showProjects && <ProjectLine />}
      </header>
      <main className="sw-main">
        <Tabs
          key={tabs.key}
          defaultTabId={tabs.first}
          tabs={[
            { id: 'data', label: TABS.data, content: body(dataTab, view !== null ? 'grid' : undefined) },
            { id: 'variable', label: TABS.variable, content: body(view ? <VariableView view={view} /> : <p data-testid="sw-variable-empty">{VARIABLE_EMPTY}</p>) },
            {
              id: 'output',
              label: TABS.output(session.output.length),
              content: body(<Output session={session} bridges={runs} forget={forget} projects={projects} save={save} status={outputStatus} />, session.output.length > 0 ? 'output' : undefined),
            },
          ]}
        />
      </main>
      <footer className="sw-foot" data-testid="sw-foot">
        {FOOTER_PHRASES.map((p) => (
          <span key={p}>{p}</span>
        ))}
        {timing && <TimingLine timing={timing} />}
      </footer>
      {help && <HelpDialog topic={help} capabilities={list} unread={listUnread} limits={limits} onClose={() => setHelp(null)} />}
      <OutputWatchers entries={session.output} bridges={runs} pollMs={runPollMs} />
      {MethodDialogFor && view && session.slot !== null && (
        <MethodDialogFor view={view} slot={session.slot} bridges={runs} outputFull={outputFull} onStarted={onStarted} onCancel={() => setDialogKind(null)} returnFocus={afterDialog} />
      )}
      {asking && (
        <AskFirst
          title={asking === 'close' ? ASK_CLOSE.title : ASK_OPEN.title}
          sentence={asking === 'close' ? ASK_CLOSE.sentence : ASK_OPEN.sentence}
          testId="sw-ask"
          onCancel={() => setAsking(null)}
          onContinue={() => {
            const act = asking === 'close' ? closeFile : choose;
            setAsking(null);
            void act();
          }}
        />
      )}
      {switchDialog}
    </div>
  );
};

export default StatisticsWindow;
