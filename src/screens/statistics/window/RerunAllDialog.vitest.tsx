// Run every saved result again…'s dialog (Research OS, Phase 8U-2, decisions
// 537, 538 and 546): read the runs once; the sentence, with what applies; Run
// again starting the list once; the lines as they come, Stop and Close; the
// count; the focus at each change; Escape closing without stopping; and
// nothing started by itself. Every bridge is a mock, and no project command is
// named as a string (projectGuards.vitest.ts reads this folder).
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { defaultRunBridges } from '../../workspace/useWorkspaceSwitch';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockProjectBridge, PROJECT_COMMANDS as C } from '../projects/projectBridge';
import type { StoredRunView } from '../projects/projectTypes';
import { codeSentence, PROJECT_TEXT } from '../projects/projectWording';
import { rerunAllStore } from './rerunAll';
import { RerunAllDialog } from './RerunAllDialog';
import { statisticsSession } from './statisticsSession';
import {
  RERUN_ALL_GOING_NOTE,
  RERUN_ALL_LEFT_OUT,
  RERUN_ALL_MEMORY,
  RERUN_ALL_PREPARED,
  RERUN_ALL_STOP,
  RERUN_ALL_TITLE,
  rerunAllSentence,
} from './windowWording';

afterEach(() => {
  cleanup();
  statisticsSession.clear();
});

const P = 'p'.repeat(32);
const stored = (id: number, savedAt: number, over: Partial<StoredRunView> = {}): StoredRunView => ({
  storedRunId: id,
  capabilityId: 'stats.describe',
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
const TWO = [stored(1, 1792396100), stored(2, 1792396200)];

/** A project bridge whose reruns stay going until `finish` is called. */
function world(runs: StoredRunView[] | (() => StoredRunView[] | Promise<StoredRunView[]>), opts: { readRunsFails?: boolean } = {}) {
  let going: { runId: number; rerunOf: number } | null = null;
  let ended: { runId: number; rerunOf: number; storedRunId: number } | null = null;
  let ids = 10;
  let all = typeof runs === 'function' ? null : runs.slice();
  const projects = makeMockProjectBridge({
    [C.runs]: () => {
      if (opts.readRunsFails) throw new Error('unread');
      if (typeof runs === 'function') return runs();
      return (all as StoredRunView[]).slice();
    },
    [C.current]: () => ({ projectId: P, rerun: going ? { ...going, status: 'going', storedRunId: null, code: null } : ended ? { ...ended, status: 'saved', code: null } : null }),
    [C.rerun]: (a: Record<string, unknown>) => {
      going = { runId: ++ids, rerunOf: a.runId as number };
      return { runId: ids, kind: 'describe' };
    },
  });
  const describe = makeMockDescribeBridge({ stats_desc_cancel: { runId: 0 } });
  const finish = () => {
    if (!going) return;
    const id = 100 + going.rerunOf;
    all = [...(all ?? []), stored(id, 1792397000 + id, { rerunOf: going.rerunOf, comparison: 'identical' })];
    ended = { ...going, storedRunId: id };
    going = null;
  };
  return { projects, bridges: defaultRunBridges({ describe }), describe, finish };
}

function dialog(w: ReturnType<typeof world>, fileOpen = false) {
  const onClose = vi.fn();
  const ui = <RerunAllDialog projectId={P} projects={w.projects} runs={w.bridges} pollMs={2} fileOpen={fileOpen} onClose={onClose} />;
  const view = render(ui);
  return { onClose, view, ui };
}
const reruns = (w: ReturnType<typeof world>) => w.projects.calls.filter(([c]) => c === C.rerun);
const reads = (w: ReturnType<typeof world>) => w.projects.calls.filter(([c]) => c === C.runs);

describe('before the list (decision 538)', () => {
  it('reads the runs once: while reading, the loading sentence with Cancel', async () => {
    let release: (r: StoredRunView[]) => void = () => undefined;
    const w = world(() => new Promise<StoredRunView[]>((res) => (release = res)));
    dialog(w);
    expect(screen.getByRole('dialog', { name: RERUN_ALL_TITLE })).toBeTruthy();
    expect(screen.getByTestId('sw-rerun-all-message').textContent).toBe(PROJECT_TEXT.runsLoading);
    expect(screen.getByTestId('sw-rerun-all-cancel').textContent).toBe(PROJECT_TEXT.cancel);
    expect(screen.queryByTestId('sw-rerun-all-run')).toBeNull();
    await act(async () => release(TWO));
    expect(await screen.findByTestId('sw-rerun-all-sentence')).toBeTruthy();
    expect(reads(w)).toHaveLength(1);
  });

  it.each([
    ['a failed read', world([], { readRunsFails: true }), PROJECT_TEXT.runsUnread],
    ['no saved run', world([]), PROJECT_TEXT.runsEmpty],
    ['saved runs, none in the list', world([stored(1, 1, { state: 'failed' }), stored(2, 2, { rerunOf: 1 })]), codeSentence('project.rerun_not_succeeded')],
  ])('%s: its sentence, with Close and no Run again', async (_n, w, said) => {
    const { onClose } = dialog(w);
    await waitFor(() => expect(screen.getByTestId('sw-rerun-all-message').textContent).toBe(said));
    expect(screen.getByTestId('sw-rerun-all-close').textContent).toBe(PROJECT_TEXT.closeDialog);
    expect(screen.queryByTestId('sw-rerun-all-run')).toBeNull();
    fireEvent.click(screen.getByTestId('sw-rerun-all-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('the sentence for n, with Cancel focused and Run again, and nothing started', async () => {
    const w = world(TWO);
    dialog(w);
    expect((await screen.findByTestId('sw-rerun-all-sentence')).textContent).toBe(rerunAllSentence(2));
    expect(document.activeElement).toBe(screen.getByTestId('sw-rerun-all-cancel'));
    expect(screen.getByTestId('sw-rerun-all-run').textContent).toBe(PROJECT_TEXT.runAgain);
    expect(screen.queryByTestId('sw-rerun-all-prepared')).toBeNull();
    expect(screen.queryByTestId('sw-rerun-all-left-out')).toBeNull();
    expect(screen.queryByTestId('sw-rerun-all-memory')).toBeNull();
    expect(reruns(w)).toEqual([]);
    expect(rerunAllStore.get().status).toBe('idle');
  });

  it('adds the prepared and left-out sentences when they apply', async () => {
    const w = world([stored(1, 1, { prepared: true }), stored(2, 2, { rerunOf: 1 })]);
    dialog(w);
    expect((await screen.findByTestId('sw-rerun-all-sentence')).textContent).toBe(rerunAllSentence(1));
    expect(screen.getByTestId('sw-rerun-all-prepared').textContent).toBe(RERUN_ALL_PREPARED);
    expect(screen.getByTestId('sw-rerun-all-left-out').textContent).toBe(RERUN_ALL_LEFT_OUT);
  });

  it('the memory sentence only with a data file open in the window (decision 546)', async () => {
    const w = world(TWO);
    dialog(w, true);
    expect((await screen.findByTestId('sw-rerun-all-memory')).textContent).toBe(RERUN_ALL_MEMORY);
    cleanup();
    dialog(world(TWO), false);
    await screen.findByTestId('sw-rerun-all-sentence');
    expect(screen.queryByTestId('sw-rerun-all-memory')).toBeNull();
  });

  it('opened and opened again, it starts nothing', async () => {
    const w = world(TWO);
    const { view } = dialog(w);
    await screen.findByTestId('sw-rerun-all-sentence');
    view.unmount();
    dialog(w);
    await screen.findByTestId('sw-rerun-all-sentence');
    expect(reruns(w)).toEqual([]);
  });
});

describe('the list in the dialog (decision 538)', () => {
  it('Run again starts it once, even clicked twice; then the log, the note, Stop focused and Close', async () => {
    const w = world(TWO);
    dialog(w);
    const run = await screen.findByTestId('sw-rerun-all-run');
    fireEvent.click(run);
    fireEvent.click(run);
    await waitFor(() => expect(screen.getByTestId('sw-rerun-all-stop')).toBeTruthy());
    expect(reruns(w)).toHaveLength(1);
    const log = screen.getByTestId('sw-rerun-all-log');
    expect(log.getAttribute('role')).toBe('log');
    expect(log.getAttribute('aria-live')).toBe('polite');
    expect(screen.getByTestId('sw-rerun-all-going-note').textContent).toBe(RERUN_ALL_GOING_NOTE);
    expect(screen.getByTestId('sw-rerun-all-stop').textContent).toBe(RERUN_ALL_STOP);
    await waitFor(() => expect(document.activeElement).toBe(screen.getByTestId('sw-rerun-all-stop')));
    await waitFor(() => expect(log.textContent).toContain(PROJECT_TEXT.runningAgain));
  });

  it('ends with the count, Close focused; Close then empties the store, so the next opening reads the runs again', async () => {
    const w = world(TWO);
    const { onClose } = dialog(w);
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(reruns(w)).toHaveLength(1));
    w.finish();
    await waitFor(() => expect(reruns(w)).toHaveLength(2));
    w.finish();
    await waitFor(() => expect(rerunAllStore.get().status).toBe('done'));
    expect(screen.getByTestId('sw-rerun-all-log').textContent).toContain('2 saved results: 2 identical.');
    expect(screen.queryByTestId('sw-rerun-all-stop')).toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByTestId('sw-rerun-all-close')));
    fireEvent.click(screen.getByTestId('sw-rerun-all-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(rerunAllStore.get().status).toBe('idle');
  });

  it('Stop, once: disabled once pressed, the kind\'s cancel once', async () => {
    const w = world(TWO);
    dialog(w);
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(rerunAllStore.get().going?.phase).toBe('running'));
    const stop = screen.getByTestId('sw-rerun-all-stop') as HTMLButtonElement;
    fireEvent.click(stop);
    fireEvent.click(stop);
    expect(stop.disabled).toBe(true);
    await waitFor(() => expect(w.describe.calls.filter(([c]) => c === 'stats_desc_cancel')).toHaveLength(1));
  });

  it('Escape closes it without stopping the list, and opened again it shows the list going', async () => {
    const w = world(TWO);
    const { onClose } = dialog(w);
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(rerunAllStore.get().status).toBe('going'));
    fireEvent.keyDown(screen.getByTestId('sw-rerun-all-dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(rerunAllStore.get().status).toBe('going');
    expect(rerunAllStore.get().stopped).toBe(false);
    cleanup();
    dialog(w);
    expect(screen.getByTestId('sw-rerun-all-log')).toBeTruthy();
    expect(screen.queryByTestId('sw-rerun-all-run')).toBeNull();
    expect(reads(w)).toHaveLength(1);
  });

  it('Close while it goes only closes the dialog', async () => {
    const w = world(TWO);
    const { onClose } = dialog(w);
    fireEvent.click(await screen.findByTestId('sw-rerun-all-run'));
    await waitFor(() => expect(screen.getByTestId('sw-rerun-all-close')).toBeTruthy());
    fireEvent.click(screen.getByTestId('sw-rerun-all-close'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(rerunAllStore.get().status).toBe('going');
  });
});
