// Gaply — Project, Run every saved result again… : the driver (Research OS,
// Phase 8U-2, decisions 536, 537 and 545; ADR-012 Part 9). A module store of
// its own, beside the session's, on openProjectStore's pattern, so leaving the
// window does not stop the list. It runs the open project's saved results
// again, one after another, through today's commands only: the bridge's
// current, rerun and runs, and the kind's own cancel. It writes nothing: each
// rerun's save is Rust's, as a Run again click's is.
//
// Its token holds the session's generation at the start. Whenever the session
// has been cleared since (the window's poll, or a workspace switch), the list
// is stale: it starts nothing more, adds nothing, and the store reads as idle.
//
// Memory only: no browser storage, no log, and no name of a project, a file or
// a column. A line names a run by its method and its saved time.
import { useSyncExternalStore } from 'react';
import type { ProjectBridge } from '../projects/projectBridge';
import { currentId } from '../projects/projectSession';
import type { ProjectCode, StoredRunView } from '../projects/projectTypes';
import { capabilityName, codeSentence, PROJECT_TEXT, projectRefusal, projectSentence, RERUN_WORDS, rerunLines } from '../projects/projectWording';
import { statisticsSession } from './statisticsSession';
import { RERUN_ALL_FAILED, RERUN_ALL_NOT_RUN, RERUN_ALL_REST, RERUN_ALL_STOPPED, rerunAllCount, type RerunAllCounts } from './windowWording';

export interface RerunAllDeps {
  bridge: Pick<ProjectBridge, 'current' | 'rerun' | 'runs'>;
  /** Each kind's cancel, as runKinds gives them (useWorkspaceSwitch.tsx). */
  kinds: Record<string, { cancel: (runId: number) => Promise<unknown> }>;
  /** How often the going rerun is read: the window's runPollMs. */
  pollMs: number;
}

export type RerunAllPhase = 'starting' | 'running' | 'cancelling';

export interface RerunAllGoing {
  /** Its place in the list, from 1. */
  place: number;
  stored: StoredRunView;
  runId: number | null;
  kind: string | null;
  phase: RerunAllPhase;
}

export interface RerunAllState {
  status: 'idle' | 'going' | 'done';
  projectId: string | null;
  total: number;
  lines: string[];
  going: RerunAllGoing | null;
  counts: RerunAllCounts;
  /** Stop was pressed in this list. */
  stopped: boolean;
}

const ZERO: RerunAllCounts = { identical: 0, different: 0, notCompared: 0, failed: 0, savedNotRead: 0, notSaved: 0, notKnown: 0, notRun: 0, notStarted: 0 };
const IDLE: RerunAllState = { status: 'idle', projectId: null, total: 0, lines: [], going: null, counts: ZERO, stopped: false };

/** The codes that end the list (decision 536, point 6). */
const ENDING: ReadonlySet<string> = new Set([
  'project.project_cap',
  'project.all_projects_cap',
  'project.disk_full',
  'project.rerun_busy',
  'project.rerun_in_progress',
  'project.not_open',
  'project.run_cancelled',
]);

let state: RerunAllState = IDLE;
/** The list's token: a number of its own and the session's generation at its start. */
let token: { n: number; generation: number } | null = null;
let lists = 0;
/** The list's deps, for Stop's cancel. */
let listDeps: RerunAllDeps | null = null;
const listeners = new Set<() => void>();

function emit() {
  listeners.forEach((l) => l());
}

/** The list is no longer this session's: a clear has moved the generation. */
const staleNow = () => token !== null && token.generation !== statisticsSession.generation();

function set(next: Partial<RerunAllState>) {
  state = { ...state, ...next };
  emit();
}

/** The runs the list reruns: no rerunOf, and Succeeded, since Rust refuses any
 *  other; oldest saved first, then by number. */
export function rerunList(runs: StoredRunView[]): StoredRunView[] {
  return runs
    .filter((r) => r.rerunOf === null && r.state === 'succeeded')
    .slice()
    .sort((a, b) => a.savedAt - b.savedAt || a.storedRunId - b.storedRunId);
}

const lineOf = (place: number, run: StoredRunView, sentences: string[]) => `${place}. ${capabilityName(run.capabilityId)}: ${sentences.join(' ')}`;

/** The going run's line, with where it stands. */
export function goingLine(g: RerunAllGoing): string {
  const word = g.phase === 'starting' ? PROJECT_TEXT.starting : g.phase === 'cancelling' ? PROJECT_TEXT.cancelling : PROJECT_TEXT.runningAgain;
  return lineOf(g.place, g.stored, [RERUN_WORDS.repeatsOn(g.stored.savedAt), word]);
}

const sleep = (ms: number) => new Promise<void>((res) => setTimeout(res, ms));

/** The kind's cancel, by an own key (as ProjectPage looks a kind up), the
 *  summary's for a kind this screen does not know, as runKinds' commandsFor
 *  does. Its answer is not read: the run ends by itself either way. */
function cancelRun(deps: RerunAllDeps, kind: string, runId: number) {
  const own = Object.prototype.hasOwnProperty.call(deps.kinds, kind);
  const k = own ? deps.kinds[kind] : deps.kinds.describe;
  void Promise.resolve()
    .then(() => k?.cancel(runId))
    .catch(() => undefined);
}

async function drive(n: number, projectId: string, all: StoredRunView[], list: StoredRunView[], deps: RerunAllDeps) {
  // Stale lists stop here, and say so to their readers: a clear of a session
  // that held nothing tells no one (statisticsSession.ts), so the store does.
  const mine = () => {
    const ok = token !== null && token.n === n && !staleNow();
    if (!ok && staleNow()) emit();
    return ok;
  };
  const counts: RerunAllCounts = { ...ZERO };
  const lines: string[] = [];
  const add = (line: string) => {
    lines.push(line);
    set({ lines: lines.slice(), counts: { ...counts } });
  };
  /** Ends the list: the ending line, if any, with the rest; the rest counted
   *  not started; then the count. */
  const end = (left: number, reason: string | null) => {
    counts.notStarted += left;
    const words = [reason, left > 0 ? RERUN_ALL_REST : null].filter((w): w is string => w !== null);
    if (words.length > 0) lines.push(words.join(' '));
    lines.push(rerunAllCount(list.length, counts));
    set({ status: 'done', lines: lines.slice(), counts: { ...counts }, going: null });
  };

  for (let i = 0; i < list.length; i += 1) {
    const run = list[i];
    const place = i + 1;
    const left = list.length - i;
    if (!mine()) return;
    if (state.stopped) return end(left, RERUN_ALL_STOPPED);

    // 3. The list's project, and no other rerun, before each start.
    let answer: unknown;
    try {
      answer = await deps.bridge.current();
    } catch {
      if (mine()) end(left, PROJECT_TEXT.pageUnread);
      return;
    }
    if (!mine()) return;
    if (currentId(answer) !== projectId) return end(left, codeSentence('project.not_open'));
    const elsewhere = (answer as { rerun?: { status?: unknown } | null }).rerun;
    if (elsewhere && elsewhere.status === 'going') return end(left, codeSentence('project.rerun_in_progress'));
    if (state.stopped) return end(left, RERUN_ALL_STOPPED);

    // 4. One rerun, once.
    set({ going: { place, stored: run, runId: null, kind: null, phase: 'starting' } });
    let started: { runId: number; kind: string };
    try {
      started = await deps.bridge.rerun(run.storedRunId);
    } catch (e) {
      if (!mine()) return;
      counts.notRun += 1;
      set({ going: null });
      add(lineOf(place, run, [RERUN_WORDS.repeatsOn(run.savedAt), RERUN_ALL_NOT_RUN, projectSentence(e)]));
      const r = projectRefusal(e);
      if (r === null || ENDING.has(r.code)) return end(left - 1, state.stopped ? RERUN_ALL_STOPPED : null);
      continue;
    }
    if (!mine()) return;
    const stopping = state.stopped;
    set({ going: { place, stored: run, runId: started.runId, kind: started.kind, phase: stopping ? 'cancelling' : 'running' } });
    if (stopping) cancelRun(deps, started.kind, started.runId);

    // 5. Read it until it is no longer going.
    let sentences: string[] | null = null;
    let endsWith: ProjectCode | null = null;
    let notKnown = false;
    for (;;) {
      // eslint-disable-next-line no-await-in-loop
      await sleep(deps.pollMs);
      if (!mine()) return;
      let now: unknown;
      try {
        // eslint-disable-next-line no-await-in-loop
        now = await deps.bridge.current();
      } catch {
        notKnown = true;
        break;
      }
      if (!mine()) return;
      const r = (now as { rerun?: { runId?: unknown; status?: unknown; storedRunId?: unknown; code?: unknown } | null }).rerun;
      if (!r || currentId(now) !== projectId || r.runId !== started.runId) {
        notKnown = true;
        break;
      }
      if (r.status === 'going') continue;
      if (r.status === 'saved') {
        let after: StoredRunView[] | null;
        try {
          // eslint-disable-next-line no-await-in-loop
          after = await deps.bridge.runs(projectId);
        } catch {
          after = null;
        }
        if (!mine()) return;
        const found = after?.find((x) => x.storedRunId === r.storedRunId) ?? null;
        const repeated = all.find((x) => x.storedRunId === run.storedRunId) ?? run;
        if (found === null) {
          counts.savedNotRead += 1;
          sentences = [RERUN_WORDS.repeatsOn(run.savedAt), RERUN_WORDS.saved, PROJECT_TEXT.runsUnread];
        } else if (found.state === 'succeeded') {
          const said = rerunLines(found, repeated);
          if (found.comparison === 'identical') counts.identical += 1;
          else if (found.comparison === 'different') counts.different += 1;
          else counts.notCompared += 1;
          sentences = said.length > 0 ? said : [RERUN_WORDS.repeatsOn(run.savedAt)];
        } else {
          counts.failed += 1;
          sentences = [rerunLines(found, repeated)[0] ?? RERUN_WORDS.repeatsOn(run.savedAt), RERUN_ALL_FAILED];
        }
      } else if (r.status === 'not_saved') {
        counts.notSaved += 1;
        endsWith = (typeof r.code === 'string' ? r.code : null) as ProjectCode | null;
        sentences = [RERUN_WORDS.repeatsOn(run.savedAt), RERUN_WORDS.notSaved, codeSentence(endsWith)];
      } else {
        notKnown = true;
      }
      break;
    }
    set({ going: null });
    if (notKnown) {
      counts.notKnown += 1;
      add(lineOf(place, run, [RERUN_WORDS.repeatsOn(run.savedAt), PROJECT_TEXT.pageUnread]));
      return end(left - 1, state.stopped ? RERUN_ALL_STOPPED : null);
    }
    add(lineOf(place, run, sentences ?? []));
    // 7. Stop pressed in this list, or a cancel from elsewhere: Stopped.
    if (state.stopped || endsWith === 'project.run_cancelled') return end(left - 1, RERUN_ALL_STOPPED);
    if (endsWith !== null && ENDING.has(endsWith)) return end(left - 1, null);
  }
  if (mine()) end(0, null);
}

export const rerunAllStore = {
  get(): RerunAllState {
    return staleNow() ? IDLE : state;
  },
  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },
  /** Starts the list, from idle or done only: false otherwise, so a second
   *  click starts nothing. `runs` are the project's saved runs as read. */
  start(projectId: string, runs: StoredRunView[], deps: RerunAllDeps): boolean {
    const now = rerunAllStore.get();
    if (now.status === 'going') return false;
    const list = rerunList(runs);
    lists += 1;
    listDeps = deps;
    token = { n: lists, generation: statisticsSession.generation() };
    state = { ...IDLE, status: 'going', projectId, total: list.length };
    emit();
    void drive(lists, projectId, runs, list, deps);
    return true;
  },
  /** Stop, once: no more starts, and the going run's cancel, if it has one. */
  stop() {
    if (rerunAllStore.get().status !== 'going' || state.stopped) return;
    const g = state.going;
    set({ stopped: true, going: g && g.runId !== null ? { ...g, phase: 'cancelling' } : g });
    if (g && g.runId !== null && g.kind !== null && listDeps !== null) cancelRun(listDeps, g.kind, g.runId);
  },
  /** Empties a list that is done, so the next opening reads the runs again. */
  clear() {
    if (rerunAllStore.get().status === 'going') return;
    state = IDLE;
    token = null;
    emit();
  },
};

const subscribeBoth = (listener: () => void) => {
  const a = rerunAllStore.subscribe(listener);
  const b = statisticsSession.subscribe(listener);
  return () => {
    a();
    b();
  };
};

/** The store, as a component reads it: it follows the session too, so a clear
 *  turns a going list idle on screen. */
export function useRerunAll(): RerunAllState {
  return useSyncExternalStore(subscribeBoth, rerunAllStore.get, rerunAllStore.get);
}
