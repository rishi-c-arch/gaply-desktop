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
import { refusalOf } from '../../statsverifier/l1/l1Wording';
import type { DatasetView, ExportView } from '../describeTypes';
import { actWith, saveExactly, useMethodLimits } from '../flowHooks';
import { FileSummary, LimitsLine, OpenProgressLine } from '../flowParts';
import type { MethodKind } from '../methods';
import { prepBridge, type PrepBridge } from '../prepBridge';
import { prepRefusalSentence } from '../prepWording';
import { projectBridge, type ProjectBridge } from '../projects/projectBridge';
import { projectsShown } from '../projects/projectGate';
import { EXPORTS } from '../projects/projectExport';
import { ProjectExportDialog } from '../projects/ProjectExportDialog';
import { projectPagePath } from '../projects/projectRoute';
import { noteCurrent, noteWorkspaceState as noteProjectWorkspaceState, useOpenProject } from '../projects/projectSession';
import { readMethodLimits } from '../statsInvoke';
import { useGeneration } from '../projects/useGeneration';
import { capabilityBridge, type CapabilityBridge } from '../ttestBridge';
import type { CapabilityView } from '../ttestTypes';
import './StatisticsWindow.css';
import { DataGrid } from './DataGrid';
import { GridTiming, timingShown } from './gridTiming';
import { HelpDialog } from './HelpDialog';
import { METHOD_DIALOGS, type StartedRun } from './methodOutput';
import { OpenProjectDialog } from './OpenProjectDialog';
import { useRerunAll } from './rerunAll';
import { RerunAllDialog } from './RerunAllDialog';
import { AskFirst } from './MethodDialog';
import { entryHeadingId, Output, OutputWatchers } from './Output';
import { ScoreDialog } from './ScoreDialog';
import { holdsUnsavedWork, MAX_OUTPUT, runIsGoing, statisticsSession, useStatisticsSession, type NewOutputEntry } from './statisticsSession';
import { TimingLine } from './TimingLine';
import { useWindowFile } from './useWindowFile';
import { VariableView } from './VariableView';
import { openBlocked, windowMenus, type HelpTopic } from './windowMenus';
import {
  ASK_CLOSE,
  ASK_EXPORT_PREPARED,
  ASK_OPEN,
  EXPORT_PREPARED_NAME,
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
  /** Makes, reads and clears the slot's scores; prepBridge() by default (decision 505). */
  prep?: PrepBridge;
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

/** The open project's line (decision 523): with a project open, a button
 *  opening its page; with none, plain text. */
const ProjectLine: React.FC<{ onOpen: (projectId: string) => void }> = ({ onOpen }) => {
  const open = useOpenProject();
  return (
    <p className="sw-project" data-testid="sw-project-line" title={open?.name}>
      {open ? (
        <button type="button" className="sw-project__open" onClick={() => onOpen(open.projectId)} data-testid="sw-project-open">
          {PROJECT_LINE_PREFIX}
          <bdi>{open.name}</bdi>
        </button>
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
  prep: givenPrep,
}) => {
  const navigate = useNavigate();
  const workspace = useMemo(() => givenWorkspace ?? workspaceBridge(), [givenWorkspace]);
  const files = useMemo(() => givenFiles ?? describeBridge(), [givenFiles]);
  const capabilities = useMemo(() => givenCapabilities ?? capabilityBridge(), [givenCapabilities]);
  const projects = useMemo(() => givenProjects ?? projectBridge(), [givenProjects]);
  const runs = useMemo(() => defaultRunBridges(givenRuns), [givenRuns]);
  const forget = useMemo(() => givenForget ?? describeBridge(), [givenForget]);
  const prep = useMemo(() => givenPrep ?? prepBridge(), [givenPrep]);
  const timing = useMemo(() => (timingShown() ? new GridTiming() : undefined), []);
  const session = useStatisticsSession();
  const { view } = session;
  const [message, setMessage] = useState<string | null>(null);
  const [runGoing, setRunGoing] = useState(false);
  const [list, setList] = useState<CapabilityView[] | null>(null);
  const [listUnread, setListUnread] = useState(false);
  const [help, setHelp] = useState<HelpTopic | null>(null);
  const [dialogKind, setDialogKind] = useState<MethodKind | null>(null);
  const [asking, setAsking] = useState<'close' | 'open' | 'export' | null>(null);
  // The export's own error (decision 514), shown in the one sw-error paragraph after useWindowFile's.
  const [exportError, setExportError] = useState<string | null>(null);
  // The slot the score dialog was opened for: it shows only while that slot is the session's.
  const [scoreFor, setScoreFor] = useState<number | null>(null);
  const [tabs, setTabs] = useState<{ key: number; first?: string }>({ key: 0 });
  const [outputStatus, setOutputStatus] = useState<string | null>(null);
  // Project, Open a project…'s dialog (decision 525).
  const [openingProject, setOpeningProject] = useState(false);
  // Project, Export the project report…'s dialog, for the open project (decision 526).
  const [exportingReport, setExportingReport] = useState(false);
  const openProject = useOpenProject();
  const reportGen = useGeneration();
  // Project, Run every saved result again…'s dialog, and the list it starts (decision 538).
  const [rerunAllOpen, setRerunAllOpen] = useState(false);
  const rerunList = useRerunAll();
  const rerunsGoing = rerunList.status === 'going';
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

  // An export's error belongs to the file it was asked of.
  useEffect(() => {
    setExportError(null);
  }, [view]);

  // Rust's word, or an entry of this window's whose run has not ended.
  const going = runGoing || session.output.some(runIsGoing) || rerunsGoing;
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

  /** File, Export the prepared data (decision 514): csv for the slot the
   *  question was asked of, then the save dialog, each answer taken only while
   *  the session's generation and slot are what they were (8T-2a's check, written
   *  again here, since the score dialog's is its own). run.unknown_input means
   *  the slot is gone; from csv it can also mean the slot holds no recipe, which
   *  the item's guard keeps from happening. */
  const exportAct = actWith(statisticsSession.setBusy, setExportError, (e) => prepRefusalSentence(e, view));
  const exportPrepared = (slot: number | null) =>
    exportAct(async () => {
      if (slot === null) return;
      const generation = statisticsSession.generation();
      const still = () => statisticsSession.generation() === generation && statisticsSession.get().slot === slot;
      let out: ExportView;
      try {
        out = await prep.csv(slot);
      } catch (e) {
        if (!still()) return;
        if (refusalOf(e)?.code !== 'run.unknown_input') throw e;
        statisticsSession.slotGone();
        return;
      }
      if (!still()) return;
      const saved = await saveExactly(out, { file: EXPORT_PREPARED_NAME, mime: 'text/csv' }, save);
      if ('error' in saved) setExportError(saved.error);
      else setMessage(saved.note);
    });

  const menus = windowMenus({
    fileOpen: view !== null,
    prepared: view?.preparation !== undefined,
    projectOpen: openProject !== null,
    rerunsGoing,
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
      score: () => setScoreFor(session.slot),
      exportPrepared: () => setAsking('export'),
      openProject: () => setOpeningProject(true),
      exportReport: () => setExportingReport(true),
      rerunAll: () => setRerunAllOpen(true),
    },
  });

  const openWhy = openBlocked({ runGoing: going, busy: session.busy, rerunsGoing });
  // The list's project while it goes, else the open one.
  const rerunAllProject = rerunsGoing ? rerunList.projectId : (openProject?.projectId ?? null);
  const MethodDialogFor = dialogKind ? METHOD_DIALOGS[dialogKind] : undefined;

  const body = (content: React.ReactNode, full?: 'grid' | 'output') => (
    <>
      <AboveTheGrid view={view} />
      <div className={full ? `sw-body sw-body--${full}` : 'sw-body'} data-testid="sw-body" tabIndex={full ? undefined : 0}>
        <p className="sw-message" role="status" aria-live="polite" data-testid="sw-message">
          {message ?? ''}
        </p>
        {(error ?? exportError) && (
          <p className="sw-message" role="alert" data-testid="sw-error">
            {error ?? exportError}
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
        {showProjects && <ProjectLine onOpen={(id) => navigate(projectPagePath(id))} />}
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
      {scoreFor !== null && scoreFor === session.slot && view && (
        <ScoreDialog
          view={view}
          slot={scoreFor}
          prep={prep}
          onCancel={() => setScoreFor(null)}
          onDone={(sentence) => {
            setScoreFor(null);
            if (sentence !== null) setMessage(sentence);
          }}
        />
      )}
      {asking === 'export' && (
        <AskFirst
          title={ASK_EXPORT_PREPARED.title}
          sentence={ASK_EXPORT_PREPARED.sentence}
          testId="sw-ask-export"
          onCancel={() => setAsking(null)}
          onContinue={() => {
            setAsking(null);
            void exportPrepared(session.slot);
          }}
        />
      )}
      {(asking === 'close' || asking === 'open') && (
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
      {openingProject && (
        <OpenProjectDialog
          projects={projects}
          workspace={workspace}
          onClose={() => setOpeningProject(false)}
          onOpened={(id) => {
            setOpeningProject(false);
            navigate(projectPagePath(id));
          }}
        />
      )}
      {exportingReport && openProject && (
        <ProjectExportDialog
          kind="report"
          projectId={openProject.projectId}
          bridge={projects}
          gen={reportGen}
          save={(n, t, m) => save(n, t, m ?? EXPORTS.report.mime)}
          openerAfter={() => null}
          onDone={(said) => {
            setExportingReport(false);
            if (said !== null) setMessage(said);
          }}
        />
      )}
      {rerunAllOpen && rerunAllProject !== null && (
        <RerunAllDialog projectId={rerunAllProject} projects={projects} runs={runs} pollMs={runPollMs} fileOpen={view !== null} onClose={() => setRerunAllOpen(false)} />
      )}
      {switchDialog}
    </div>
  );
};

export default StatisticsWindow;
