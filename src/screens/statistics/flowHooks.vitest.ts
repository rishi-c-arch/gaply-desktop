// actWith's busy count (Research OS, Phase 8Z-1, decision 635). A screen calls actWith on
// every render, so two overlapping actions can come from two different closures: the
// count belongs to the stable setBusy, and busy goes false only when the last call on it
// ends. An action's refusal is still shown.
import { describe, expect, it, vi } from 'vitest';

import { actWith } from './flowHooks';

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

  it('counts each setBusy on its own', async () => {
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
