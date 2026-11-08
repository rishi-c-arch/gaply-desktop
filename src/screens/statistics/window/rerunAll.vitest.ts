// Run every saved result again: the driver (Research OS, Phase 8U-2, decisions
// 536, 537 and 540; ADR-012 Part 9). A module store of its own, over the open
// project's saved runs, through the bridge's current, rerun and runs, one rerun
// at a time, one line each as it ends, and the count. Rust is a script here: no
// project command is named as a string (projectGuards.vitest.ts reads this
// folder), and every call is recorded by its bridge method.
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { capabilityName, codeSentence, PROJECT_TEXT, projectSentence, RERUN_WORDS } from '../projects/projectWording';
import type { ProjectCode, RerunView, StoredRunView } from '../projects/projectTypes';
import { rerunAllStore, rerunList, useRerunAll, type RerunAllDeps } from './rerunAll';
import { statisticsSession } from './statisticsSession';
import { RERUN_ALL_FAILED, RERUN_ALL_NOT_RUN, RERUN_ALL_REST, RERUN_ALL_STOPPED, rerunAllCount } from './windowWording';

afterEach(() => {
  // A clear of the session makes any list stale, so every test starts on an idle store.
  statisticsSession.clear();
});

const P = 'p'.repeat(32);
const Q = 'q'.repeat(32);
const SUMMARY = 'stats.describe';
const TTEST = 'stats.analyze.independent_t';

const stored = (id: number, savedAt: number, over: Partial<StoredRunView> = {}): StoredRunView => ({
  storedRunId: id,
  capabilityId: SUMMARY,
  capabilityVersion: 1,
  state: 'succeeded',
  endCode: null,
  startedAt: savedAt - 10,
  endedAt: savedAt - 9,
  savedAt,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
  ...over,
});

const refusal = (code: string) => ({ code: 'validation', message: JSON.stringify({ code }) });

/** How Rust ends one rerun, by the stored run it repeats. */
type Outcome =
  | { saved: 'identical' | 'different' | 'not_compared' }
  | { savedFailed: true }
  | { notSaved: ProjectCode }
  | { refuse: string }
  | { refuseRaw: unknown };

interface Script {
  runs: StoredRunView[];
  outcomes?: Record<number, Outcome>;
  /** The open project, read on each current; null for none. */
  open?: () => string | null;
  /** Fails the nth current read (1-based), counting every read. */
  failCurrentAt?: number;
  /** Answers about another run on the nth current read. */
  otherRunAt?: number;
  /** Shows another screen's rerun going on the nth current read. */
  goingElsewhereAt?: number;
  /** How many reads a rerun stays going. */
  goingReads?: number;
  /** The runs read after a save fails. */
  failRuns?: boolean;
  /** A promise the rerun call waits on before it answers. */
  holdRerun?: Promise<void>;
}

/** Rust, as a script: one rerun at a time, ended after `goingReads` reads. */
function rust(s: Script) {
  const calls: Array<[string, unknown]> = [];
  const runs = s.runs.slice();
  let reads = 0;
  let runIds = 40;
  let state: (RerunView & { left: number }) | null = null;
  let nextStored = Math.max(...runs.map((r) => r.storedRunId)) + 1;
  const kinds = {
    describe: { status: vi.fn(), cancel: vi.fn((runId: number) => { calls.push(['cancel', runId]); return Promise.resolve(); }) },
    ttest: { status: vi.fn(), cancel: vi.fn((runId: number) => { calls.push(['cancel', runId]); return Promise.resolve(); }) },
  };
  const finish = (st: RerunView & { left: number }) => {
    const o = s.outcomes?.[st.rerunOf] ?? { saved: 'identical' };
    const of = runs.find((r) => r.storedRunId === st.rerunOf) as StoredRunView;
    if ('saved' in o || 'savedFailed' in o) {
      const id = nextStored++;
      runs.push(stored(id, of.savedAt + 1000 + id, { capabilityId: of.capabilityId, rerunOf: st.rerunOf, state: 'savedFailed' in o ? 'failed' : 'succeeded', comparison: 'saved' in o ? o.saved : null }));
      return { ...st, status: 'saved' as const, storedRunId: id };
    }
    if ('notSaved' in o) return { ...st, status: 'not_saved' as const, code: o.notSaved };
    return st;
  };
  const bridge: RerunAllDeps['bridge'] = {
    current: async () => {
      reads += 1;
      calls.push(['current', reads]);
      if (reads === s.failCurrentAt) throw new Error('unread');
      const projectId = s.open ? s.open() : P;
      if (reads === s.goingElsewhereAt) return { projectId, rerun: { runId: 99, rerunOf: 1, status: 'going', storedRunId: null, code: null } };
      if (reads === s.otherRunAt) return { projectId, rerun: { runId: 98, rerunOf: 1, status: 'saved', storedRunId: 77, code: null } };
      if (state && state.status === 'going') {
        if (state.left > 0) state.left -= 1;
        else state = { ...finish(state), left: 0 };
      }
      const r = state ? { runId: state.runId, rerunOf: state.rerunOf, status: state.status, storedRunId: state.storedRunId, code: state.code } : null;
      return { projectId, rerun: r };
    },
    rerun: async (runId: number) => {
      calls.push(['rerun', runId]);
      if (s.holdRerun) await s.holdRerun;
      const o = s.outcomes?.[runId];
      if (o && 'refuse' in o) throw refusal(o.refuse);
      if (o && 'refuseRaw' in o) throw o.refuseRaw;
      const id = ++runIds;
      state = { runId: id, rerunOf: runId, status: 'going', storedRunId: null, code: null, left: s.goingReads ?? 1 };
      const of = runs.find((r) => r.storedRunId === runId) as StoredRunView;
      return { runId: id, kind: of.capabilityId === TTEST ? 'ttest' : 'describe' };
    },
    runs: async () => {
      calls.push(['runs', P]);
      if (s.failRuns) throw new Error('unread');
      return runs.slice();
    },
  };
  /** A Stop's cancel, as Rust then ends the run: not saved, run_cancelled. */
  const cancelEnds = () => {
    if (state && state.status === 'going') state = { ...state, status: 'not_saved', code: 'project.run_cancelled', left: 0 };
  };
  /** Rust ends the going rerun at its next read, as its outcome says. */
  const finishNow = () => {
    if (state && state.status === 'going') state = { ...state, left: 0 };
  };
  return { calls, bridge, kinds, cancelEnds, finishNow, runs };
}

const deps = (r: ReturnType<typeof rust>): RerunAllDeps => ({ bridge: r.bridge, kinds: r.kinds, pollMs: 1 });
const reruns = (r: ReturnType<typeof rust>) => r.calls.filter(([c]) => c === 'rerun').map(([, id]) => id);
const done = () => waitFor(() => expect(rerunAllStore.get().status).toBe('done'));

/** Three succeeded originals, saved in the order 3, 1, 2. */
const THREE = [stored(1, 1792396200), stored(2, 1792396300, { capabilityId: TTEST }), stored(3, 1792396100)];

describe('which runs, and their order (decision 536)', () => {
  it('only those with no rerunOf that succeeded, oldest saved first, then by number', () => {
    const list = rerunList([
      stored(5, 300),
      stored(2, 100),
      stored(4, 300),
      stored(6, 50, { rerunOf: 2, comparison: 'identical' }),
      stored(7, 10, { state: 'failed', endCode: null }),
      stored(8, 20, { state: 'cancelled' }),
    ]);
    expect(list.map((r) => r.storedRunId)).toEqual([2, 4, 5]);
  });
});

describe('the list, one rerun at a time (decision 536)', () => {
  it('reruns each once, in order, each after the last has ended, one line each, then the count', async () => {
    const r = rust({ runs: THREE, goingReads: 2 });
    expect(rerunAllStore.start(P, THREE, deps(r))).toBe(true);
    await done();
    expect(reruns(r)).toEqual([3, 1, 2]);
    // No rerun was called while another was going: between two reruns, a read saw the first end.
    const order = r.calls.map(([c]) => c);
    expect(order.filter((c) => c === 'rerun')).toHaveLength(3);
    const s = rerunAllStore.get();
    expect(s.lines).toEqual([
      `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_WORDS.identical}`,
      `2. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396200)} ${RERUN_WORDS.identical}`,
      `3. ${capabilityName(TTEST)}: ${RERUN_WORDS.repeatsOn(1792396300)} ${RERUN_WORDS.identical}`,
      '3 saved results: 3 identical.',
    ]);
    expect(s.total).toBe(3);
    expect(s.going).toBeNull();
  });

  it('a second start, or a start while going, starts nothing', async () => {
    const r = rust({ runs: THREE, goingReads: 1000 });
    expect(rerunAllStore.start(P, THREE, deps(r))).toBe(true);
    expect(rerunAllStore.start(P, THREE, deps(r))).toBe(false);
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('running'));
    expect(rerunAllStore.start(P, THREE, deps(r))).toBe(false);
    r.finishNow();
    await waitFor(() => expect(rerunAllStore.get().going?.place).toBe(2));
    expect(rerunAllStore.start(P, THREE, deps(r))).toBe(false);
    r.finishNow();
    await waitFor(() => expect(rerunAllStore.get().going?.place).toBe(3));
    r.finishNow();
    await done();
    expect(reruns(r)).toEqual([3, 1, 2]);
  });

  it('each saved outcome says what rerunLines says, and is counted by its comparison', async () => {
    const r = rust({ runs: THREE, outcomes: { 3: { saved: 'different' }, 1: { saved: 'not_compared' }, 2: { savedFailed: true } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    const s = rerunAllStore.get();
    expect(s.lines[0]).toBe(`1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_WORDS.different}`);
    expect(s.lines[1]).toBe(`2. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396200)} ${RERUN_WORDS.notCompared}`);
    expect(s.lines[2]).toBe(`3. ${capabilityName(TTEST)}: ${RERUN_WORDS.repeatsOn(1792396300)} ${RERUN_ALL_FAILED}`);
    expect(s.lines[3]).toBe('3 saved results: 1 not identical, 1 not compared, 1 failed.');
  });

  it("a save whose runs read fails says saved and unread, never a comparison", async () => {
    const one = [stored(1, 1792396200)];
    const r = rust({ runs: one, failRuns: true });
    rerunAllStore.start(P, one, deps(r));
    await done();
    const s = rerunAllStore.get();
    expect(s.lines[0]).toBe(`1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396200)} ${RERUN_WORDS.saved} ${PROJECT_TEXT.runsUnread}`);
    expect(s.lines[0]).not.toContain(RERUN_WORDS.identical);
    expect(s.lines[1]).toBe('1 saved result: 1 saved but not read.');
  });

  it('a one-line refusal (over limits) is that run\'s line, not run, and the list goes on', async () => {
    const r = rust({ runs: THREE, outcomes: { 1: { refuse: 'project.rerun_over_limits' } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3, 1, 2]);
    const s = rerunAllStore.get();
    expect(s.lines[1]).toBe(`2. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396200)} ${RERUN_ALL_NOT_RUN} ${projectSentence(refusal('project.rerun_over_limits'))}`);
    expect(s.lines[3]).toBe('3 saved results: 2 identical, 1 not run.');
  });

  it('a not saved with a code that is no ending is one line, counted not saved, and the list goes on', async () => {
    const r = rust({ runs: THREE, outcomes: { 3: { notSaved: 'project.run_failed_without_code' } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3, 1, 2]);
    expect(rerunAllStore.get().lines[0]).toBe(`1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_WORDS.notSaved} ${codeSentence('project.run_failed_without_code')}`);
    expect(rerunAllStore.get().lines[3]).toBe('3 saved results: 2 identical, 1 not saved.');
  });
});

describe('what ends the list (decision 536, points 3 to 7)', () => {
  it.each(['project.project_cap', 'project.all_projects_cap', 'project.disk_full', 'project.not_open'] as const)('a not saved with %s ends it, the rest not started', async (code) => {
    const r = rust({ runs: THREE, outcomes: { 3: { notSaved: code } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3]);
    const s = rerunAllStore.get();
    expect(s.lines).toEqual([
      `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_WORDS.notSaved} ${codeSentence(code)}`,
      RERUN_ALL_REST,
      '3 saved results: 1 not saved, 2 not started.',
    ]);
  });

  it.each(['project.rerun_busy', 'project.rerun_in_progress', 'project.project_cap'] as const)('a refusal at the start with %s ends it', async (code) => {
    const r = rust({ runs: THREE, outcomes: { 1: { refuse: code } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3, 1]);
    const s = rerunAllStore.get();
    expect(s.lines[1]).toBe(`2. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396200)} ${RERUN_ALL_NOT_RUN} ${projectSentence(refusal(code))}`);
    expect(s.lines.slice(2)).toEqual([RERUN_ALL_REST, '3 saved results: 1 identical, 1 not run, 1 not started.']);
  });

  it('a refusal that is no project code ends it', async () => {
    const raw = { code: 'validation', message: JSON.stringify({ code: 'workspace.inactive', params: {} }) };
    const r = rust({ runs: THREE, outcomes: { 3: { refuseRaw: raw } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().lines).toEqual([
      `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_ALL_NOT_RUN} ${projectSentence(raw)}`,
      RERUN_ALL_REST,
      '3 saved results: 1 not run, 2 not started.',
    ]);
  });

  it('a run_cancelled from elsewhere ends it with Stopped', async () => {
    const r = rust({ runs: THREE, outcomes: { 1: { notSaved: 'project.run_cancelled' } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3, 1]);
    const s = rerunAllStore.get();
    expect(s.lines.slice(2)).toEqual([`${RERUN_ALL_STOPPED} ${RERUN_ALL_REST}`, '3 saved results: 1 identical, 1 not saved, 1 not started.']);
  });

  it('another project open before a start ends it, with not open, nothing more started', async () => {
    let calls = 0;
    // Reads 1 to 3 are the first run's: its start check and two polls; read 4 is the second start's check.
    const r = rust({ runs: THREE, open: () => (++calls > 3 ? Q : P) });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().lines.slice(1)).toEqual([`${codeSentence('project.not_open')} ${RERUN_ALL_REST}`, '3 saved results: 1 identical, 2 not started.']);
  });

  it('another screen\'s rerun going before a start ends it, with in progress', async () => {
    const r = rust({ runs: THREE, goingElsewhereAt: 1 });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([]);
    expect(rerunAllStore.get().lines).toEqual([`${codeSentence('project.rerun_in_progress')} ${RERUN_ALL_REST}`, '3 saved results: 3 not started.']);
  });

  it('a failed read before a start ends it, with unread', async () => {
    const r = rust({ runs: THREE, failCurrentAt: 1 });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    expect(reruns(r)).toEqual([]);
    expect(rerunAllStore.get().lines).toEqual([`${PROJECT_TEXT.pageUnread} ${RERUN_ALL_REST}`, '3 saved results: 3 not started.']);
  });

  it('a failed poll, or one about another run, is not known and ends it', async () => {
    for (const s of [{ failCurrentAt: 2 }, { otherRunAt: 2 }]) {
      statisticsSession.clear();
      const r = rust({ runs: THREE, ...s });
      rerunAllStore.start(P, THREE, deps(r));
      // eslint-disable-next-line no-await-in-loop
      await done();
      expect(reruns(r), JSON.stringify(s)).toEqual([3]);
      expect(rerunAllStore.get().lines, JSON.stringify(s)).toEqual([
        `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${PROJECT_TEXT.pageUnread}`,
        RERUN_ALL_REST,
        '3 saved results: 1 not known, 2 not started.',
      ]);
    }
  });
});

describe('Stop (decision 536)', () => {
  it("cancels the going run once, through its kind's cancel, starts nothing more, and ends Stopped", async () => {
    const r = rust({ runs: THREE, goingReads: 1000 });
    rerunAllStore.start(P, THREE, deps(r));
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('running'));
    rerunAllStore.stop();
    rerunAllStore.stop();
    expect(rerunAllStore.get().going?.phase).toBe('cancelling');
    r.cancelEnds();
    await done();
    expect(r.calls.filter(([c]) => c === 'cancel')).toEqual([['cancel', 41]]);
    expect(r.kinds.describe.cancel).toHaveBeenCalledTimes(1);
    expect(r.kinds.describe.status).not.toHaveBeenCalled();
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().lines).toEqual([
      `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_WORDS.notSaved} ${codeSentence('project.run_cancelled')}`,
      `${RERUN_ALL_STOPPED} ${RERUN_ALL_REST}`,
      '3 saved results: 1 not saved, 2 not started.',
    ]);
  });

  it('pressed while the rerun is starting, it cancels the run that start returns, and starts nothing more', async () => {
    let release: () => void = () => undefined;
    const hold = new Promise<void>((res) => {
      release = res;
    });
    const r = rust({ runs: THREE, goingReads: 1000, holdRerun: hold });
    rerunAllStore.start(P, THREE, deps(r));
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('starting'));
    rerunAllStore.stop();
    await act(async () => {
      release();
      await hold;
    });
    await waitFor(() => expect(r.calls.filter(([c]) => c === 'cancel')).toEqual([['cancel', 41]]));
    r.cancelEnds();
    await done();
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().lines[1]).toBe(`${RERUN_ALL_STOPPED} ${RERUN_ALL_REST}`);
  });

  it('pressed between two runs, it starts no more, with no cancel', async () => {
    const r = rust({ runs: THREE, goingReads: 0 });
    // Stop is pressed the moment the first line is written, before the next start.
    const off = rerunAllStore.subscribe(() => {
      if (rerunAllStore.get().lines.length === 1) rerunAllStore.stop();
    });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    off();
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().lines.slice(1)).toEqual([`${RERUN_ALL_STOPPED} ${RERUN_ALL_REST}`, '3 saved results: 1 identical, 2 not started.']);
    expect(r.calls.filter(([c]) => c === 'cancel')).toEqual([]);
  });

  it('after Stop, a run that saved anyway keeps its line, then Stopped, whatever it ended with', async () => {
    // Rust saves the run before the cancel reaches it: the script never applies the cancel.
    const r = rust({ runs: THREE, goingReads: 1000 });
    rerunAllStore.start(P, THREE, deps(r));
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('running'));
    rerunAllStore.stop();
    r.finishNow();
    await done();
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().lines).toEqual([
      `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396100)} ${RERUN_WORDS.identical}`,
      `${RERUN_ALL_STOPPED} ${RERUN_ALL_REST}`,
      '3 saved results: 1 identical, 2 not started.',
    ]);
  });
});

describe('Stop during the last run (decision 536, point 7)', () => {
  it('a last run that saved anyway keeps its line, then Stopped alone, since no run is left', async () => {
    const one = [stored(1, 1792396200)];
    const r = rust({ runs: one, goingReads: 1000 });
    rerunAllStore.start(P, one, deps(r));
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('running'));
    rerunAllStore.stop();
    r.finishNow();
    await done();
    expect(rerunAllStore.get().lines).toEqual([
      `1. ${capabilityName(SUMMARY)}: ${RERUN_WORDS.repeatsOn(1792396200)} ${RERUN_WORDS.identical}`,
      RERUN_ALL_STOPPED,
      '1 saved result: 1 identical.',
    ]);
  });
});

describe('the counts (decision 537)', () => {
  it('always add up to the total, in their order, with the singular', () => {
    const zero = { identical: 0, different: 0, notCompared: 0, failed: 0, savedNotRead: 0, notSaved: 0, notKnown: 0, notRun: 0, notStarted: 0 };
    expect(rerunAllCount(1, { ...zero, identical: 1 })).toBe('1 saved result: 1 identical.');
    expect(rerunAllCount(9, { identical: 1, different: 1, notCompared: 1, failed: 1, savedNotRead: 1, notSaved: 1, notKnown: 1, notRun: 1, notStarted: 1 })).toBe(
      '9 saved results: 1 identical, 1 not identical, 1 not compared, 1 failed, 1 saved but not read, 1 not saved, 1 not known, 1 not run, 1 not started.'
    );
  });

  it('in a finished list, every run takes one count', async () => {
    const r = rust({ runs: THREE, outcomes: { 1: { refuse: 'project.rerun_over_limits' } } });
    rerunAllStore.start(P, THREE, deps(r));
    await done();
    const c = rerunAllStore.get().counts;
    expect(Object.values(c).reduce((a, b) => a + b, 0)).toBe(3);
  });
});

describe('the session ends it (decision 536)', () => {
  it('a clear of the session makes the list stale: it reads idle, starts nothing more, and adds nothing', async () => {
    const r = rust({ runs: THREE, goingReads: 1000 });
    rerunAllStore.start(P, THREE, deps(r));
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('running'));
    act(() => statisticsSession.clear());
    expect(rerunAllStore.get().status).toBe('idle');
    r.finishNow();
    await new Promise((res) => setTimeout(res, 40));
    expect(reruns(r)).toEqual([3]);
    expect(rerunAllStore.get().status).toBe('idle');
    expect(rerunAllStore.get().lines).toEqual([]);
  });

  it('useRerunAll follows the store and the session', async () => {
    const r = rust({ runs: THREE, goingReads: 1000 });
    const { result } = renderHook(() => useRerunAll());
    expect(result.current.status).toBe('idle');
    act(() => {
      rerunAllStore.start(P, THREE, deps(r));
    });
    await waitFor(() => expect(result.current.status).toBe('going'));
    act(() => statisticsSession.clear());
    await waitFor(() => expect(result.current.status).toBe('idle'));
  });
});
