// actWith's busy count (Research OS, Phase 8Z-1, decision 635). A screen calls actWith on
// every render, so two overlapping actions can come from two different closures: the
// count belongs to the stable setBusy, and busy goes false only when the last call on it
// ends. An action's refusal is still shown.
//
// Decision 652: statisticsSession.setBusy is one module-level setter shared by three callers,
// so a call that never settles would hold the count for the module's life. A clear of the
// session forgets the calls then running: busy goes false at once, and such a call, when
// it ends, changes neither the count nor busy, and its refusal is not shown.
import { afterEach, describe, expect, it, vi } from 'vitest';

import { actWith } from './flowHooks';
import { statisticsSession } from './window/statisticsSession';

function deferred() {
  let resolve: () => void = () => undefined;
  let reject: (e: unknown) => void = () => undefined;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
const flush = () => new Promise((r) => setTimeout(r, 0));

describe('actWith', () => {
  it('is busy while one action runs, then not', async () => {
    const setBusy = vi.fn();
    const d = deferred();
    const run = actWith(setBusy, vi.fn(), String)(() => d.promise);
    expect(setBusy.mock.calls).toEqual([[true]]);
    d.resolve();
    await run;
    expect(setBusy.mock.calls).toEqual([[true], [false]]);
  });

  it('stays busy until the last of two overlapping calls ends, whichever closure made them', async () => {
    const setBusy = vi.fn();
    const first = deferred();
    const second = deferred();
    // Two renders, two closures, one setBusy.
    const a = actWith(setBusy, vi.fn(), String)(() => first.promise);
    const b = actWith(setBusy, vi.fn(), String)(() => second.promise);
    first.resolve();
    await a;
    await flush();
    expect(setBusy.mock.calls.filter((c) => c[0] === false)).toEqual([]);
    second.resolve();
    await b;
    expect(setBusy.mock.calls[setBusy.mock.calls.length - 1]).toEqual([false]);
    expect(setBusy.mock.calls.filter((c) => c[0] === false)).toHaveLength(1);
  });

  it('keeps counting when a call is refused, and shows its refusal', async () => {
    const setBusy = vi.fn();
    const setError = vi.fn();
    const first = deferred();
    const second = deferred();
    const a = actWith(setBusy, setError, (e) => String(e))(() => first.promise);
    const b = actWith(setBusy, setError, (e) => String(e))(() => second.promise);
    first.reject('refused');
    await a;
    expect(setError).toHaveBeenLastCalledWith('refused');
    expect(setBusy.mock.calls.filter((c) => c[0] === false)).toEqual([]);
    second.resolve();
    await b;
    expect(setBusy.mock.calls.filter((c) => c[0] === false)).toHaveLength(1);
  });

  it("never lets one setter's calls hold another's busy", async () => {
    const one = vi.fn();
    const two = vi.fn();
    const first = deferred();
    const second = deferred();
    const a = actWith(one, vi.fn(), String)(() => first.promise);
    const b = actWith(two, vi.fn(), String)(() => second.promise);
    first.resolve();
    await a;
    expect(one).toHaveBeenLastCalledWith(false);
    expect(two.mock.calls.filter((c) => c[0] === false)).toEqual([]);
    second.resolve();
    await b;
    expect(two).toHaveBeenLastCalledWith(false);
  });
});

describe('a clear of the session forgets the calls running on its setter (decision 652)', () => {
  afterEach(() => statisticsSession.clear());
  const start = (f: () => Promise<void>, setError: (s: string | null) => void = vi.fn()) => actWith(statisticsSession.setBusy, setError, (e) => String(e))(f);
  const busy = () => statisticsSession.get().busy;

  it('a call that never settles, then clear(): busy is false at once, before any new call', () => {
    void start(() => new Promise<void>(() => undefined));
    expect(busy()).toBe(true);
    statisticsSession.clear();
    expect(busy()).toBe(false);
  });

  it('also on the path that holds a file', () => {
    statisticsSession.setFile({ slot: 1, displayName: 'x', profile: { rows: 0, columns: [] }, inputRef: { contentHash: '', rows: 0, columns: 0, semanticRole: 'table' } } as never);
    void start(() => new Promise<void>(() => undefined));
    expect(busy()).toBe(true);
    statisticsSession.clear();
    expect(busy()).toBe(false);
    expect(statisticsSession.get().slot).toBeNull();
  });

  it('then a new call that ends leaves busy false', async () => {
    void start(() => new Promise<void>(() => undefined));
    statisticsSession.clear();
    const d = deferred();
    const next = start(() => d.promise);
    expect(busy()).toBe(true);
    d.resolve();
    await next;
    expect(busy()).toBe(false);
  });

  it('the old call made to end after that changes neither busy, nor the count, nor the refusal', async () => {
    const old = deferred();
    const oldError = vi.fn();
    const stale = start(() => old.promise, oldError);
    oldError.mockClear();
    statisticsSession.clear();
    const d = deferred();
    const next = start(() => d.promise);
    d.resolve();
    await next;
    expect(busy()).toBe(false);
    old.reject('refused');
    await stale;
    expect(busy()).toBe(false);
    expect(oldError).not.toHaveBeenCalled();
    // The count did not move: one more call still takes busy to true and back to false.
    const after = deferred();
    const third = start(() => after.promise);
    expect(busy()).toBe(true);
    after.resolve();
    await third;
    expect(busy()).toBe(false);
  });

  it('a new call started after clear() and still pending when the old one ends keeps busy true', async () => {
    const old = deferred();
    const stale = start(() => old.promise);
    statisticsSession.clear();
    const d = deferred();
    const next = start(() => d.promise);
    old.resolve();
    await stale;
    await flush();
    expect(busy()).toBe(true);
    d.resolve();
    await next;
    expect(busy()).toBe(false);
  });
});
