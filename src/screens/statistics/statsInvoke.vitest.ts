// The open's progress (Research OS, Phase 8F-2, decision 290):
// openWithProgress opens with a progressId from one shared counter, polls
// stats_desc_open_progress from 250 ms on, every 250 ms, passes each count
// (never a null) on, and stops when the open returns or is refused. And the
// limits read (decision 289): one command, its answer as it came. Tauri's
// invoke is reached as projectScreens.vitest.tsx reaches it, through
// window.__TAURI_INTERNALS__, with fake timers.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { openWithProgress, readMethodLimits } from './statsInvoke';

vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

const tauri = vi.hoisted(() => ({
  calls: [] as Array<[string, Record<string, unknown>]>,
  answers: {} as Record<string, (args: Record<string, unknown>) => unknown>,
}));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args: Record<string, unknown> = {}) => {
    tauri.calls.push([cmd, args]);
    const a = tauri.answers[cmd];
    return a ? Promise.resolve().then(() => a(args)) : Promise.resolve(null);
  },
}));

/** An open that stays pending until `settle` is called. */
function pendingOpen() {
  let settle: (ok: boolean) => void = () => undefined;
  const done = new Promise<unknown>((resolve, reject) => {
    settle = (ok) => (ok ? resolve({ slot: 1 }) : reject(new Error('{"code":"malformed"}')));
  });
  tauri.answers.stats_desc_open = () => done;
  return (ok: boolean) => settle(ok);
}

const polls = () => tauri.calls.filter(([c]) => c === 'stats_desc_open_progress');

beforeEach(() => {
  vi.useFakeTimers();
  tauri.calls = [];
  tauri.answers = {};
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
    invoke: () => Promise.resolve(null),
  };
});
afterEach(() => {
  vi.useRealTimers();
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

describe('openWithProgress', () => {
  it('opens with a progressId, polls from 250 ms every 250 ms, and passes each count on', async () => {
    const settle = pendingOpen();
    let n = 0;
    tauri.answers.stats_desc_open_progress = () => ({ rowsRead: (n += 1000) });
    const seen: number[] = [];
    const open = openWithProgress<{ slot: number }>('/data.csv', (r) => seen.push(r));
    await vi.advanceTimersByTimeAsync(0);
    const [cmd, args] = tauri.calls[0];
    expect(cmd).toBe('stats_desc_open');
    expect(Object.keys(args).sort()).toEqual(['path', 'progressId']);
    expect(typeof args.progressId).toBe('number');

    await vi.advanceTimersByTimeAsync(249);
    expect(polls()).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(1);
    expect(polls()).toHaveLength(1);
    expect(polls()[0][1]).toEqual({ progressId: args.progressId });
    await vi.advanceTimersByTimeAsync(250);
    expect(polls()).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(250);
    expect(polls()).toHaveLength(3);
    expect(seen).toEqual([1000, 2000, 3000]);

    settle(true);
    await expect(open).resolves.toEqual({ slot: 1 });
    await vi.advanceTimersByTimeAsync(2000);
    expect(polls()).toHaveLength(3);
    expect(seen).toEqual([1000, 2000, 3000]);
  });

  it('never passes a null on, and stops polling when the open is refused', async () => {
    const settle = pendingOpen();
    tauri.answers.stats_desc_open_progress = () => null;
    const seen: unknown[] = [];
    const open = openWithProgress('/data.csv', (r) => seen.push(r));
    await vi.advanceTimersByTimeAsync(750);
    expect(polls()).toHaveLength(3);
    settle(false);
    await expect(open).rejects.toThrow('malformed');
    await vi.advanceTimersByTimeAsync(2000);
    expect(polls()).toHaveLength(3);
    expect(seen).toEqual([]);
  });

  it('a count that arrives after the open has settled is not passed on', async () => {
    const settle = pendingOpen();
    let answer: (v: unknown) => void = () => undefined;
    tauri.answers.stats_desc_open_progress = () => new Promise((r) => (answer = r));
    const seen: number[] = [];
    const open = openWithProgress('/data.csv', (r) => seen.push(r));
    await vi.advanceTimersByTimeAsync(250);
    expect(polls()).toHaveLength(1);
    settle(true);
    await open;
    answer({ rowsRead: 5000 });
    await vi.advanceTimersByTimeAsync(0);
    expect(seen).toEqual([]);
  });

  it('an open that returns in under 250 ms is never polled', async () => {
    tauri.answers.stats_desc_open = () => ({ slot: 1 });
    const seen: number[] = [];
    await openWithProgress('/data.csv', (r) => seen.push(r));
    await vi.advanceTimersByTimeAsync(1000);
    expect(polls()).toHaveLength(0);
    expect(seen).toEqual([]);
  });

  it('takes each open its own progressId from one counter', async () => {
    tauri.answers.stats_desc_open = () => ({ slot: 1 });
    await openWithProgress('/a.csv', () => undefined);
    await openWithProgress('/b.csv', () => undefined);
    const ids = tauri.calls.filter(([c]) => c === 'stats_desc_open').map(([, a]) => a.progressId as number);
    expect(ids).toHaveLength(2);
    expect(ids[1]).toBeGreaterThan(ids[0]);
  });
});

describe('readMethodLimits', () => {
  it('calls stats_desc_limits with no arguments and answers what came back', async () => {
    tauri.answers.stats_desc_limits = () => ({ tier: '8gb' });
    await expect(readMethodLimits()).resolves.toEqual({ tier: '8gb' });
    expect(tauri.calls).toEqual([['stats_desc_limits', {}]]);
  });
});
