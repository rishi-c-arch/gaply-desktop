// File, Open a data file… and File, Close this file (Research OS, Phase 8P,
// decision 450; ADR-012 Part 2): today's fileActions over the session's slot.
// A cancelled picker or a refused open changes nothing, the old slot closes
// only after the new one has opened, and leaving the window closes nothing.
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { makeMockDescribeBridge } from '../describeBridge';
import type { DatasetView } from '../describeTypes';
import { statisticsSession } from './statisticsSession';
import { useWindowFile } from './useWindowFile';

const view = (slot: number): DatasetView => ({
  slot,
  displayName: `f${slot}.csv`,
  profile: { rows: 3, columns: [] },
  inputRef: { contentHash: 'ab'.repeat(32), rows: 3, columns: 0, semanticRole: 'table' },
});

afterEach(() => {
  cleanup();
  statisticsSession.clear();
});

const refusal = JSON.stringify({ code: 'run.too_many_rows', params: {}, severity: 'error' });

function setUp(responses: Record<string, unknown>, pick: () => Promise<string | null> = () => Promise.resolve('/x/y.csv')) {
  const bridge = makeMockDescribeBridge(responses);
  const hook = renderHook(() => useWindowFile({ bridge, pickPath: pick }));
  return { bridge, hook, commands: () => bridge.calls.map(([c]) => c) };
}

describe('Open a data file…', () => {
  it('with none open: the picker, the open, and the store takes the view; nothing closes', async () => {
    const { hook, commands } = setUp({ stats_desc_open: view(1) });
    await act(() => hook.result.current.choose());
    expect(statisticsSession.get().slot).toBe(1);
    expect(commands()).toEqual(['stats_desc_open']);
  });

  it('with one open: the old slot closes only after the new one has opened', async () => {
    statisticsSession.setFile(view(1));
    const { hook, commands, bridge } = setUp({ stats_desc_open: view(2), stats_desc_close: true });
    await act(() => hook.result.current.choose());
    expect(commands()).toEqual(['stats_desc_open', 'stats_desc_close']);
    expect(bridge.calls[1][1]).toEqual({ slot: 1 });
    expect(statisticsSession.get().slot).toBe(2);
  });

  it('with one open: a cancelled picker changes nothing, and sends nothing', async () => {
    const v = view(1);
    statisticsSession.setFile(v);
    const { hook, commands } = setUp({}, () => Promise.resolve(null));
    await act(() => hook.result.current.choose());
    expect(commands()).toEqual([]);
    expect(statisticsSession.get().view).toBe(v);
    expect(hook.result.current.error).toBeNull();
  });

  it('with one open: a refused open changes nothing, closes nothing, and says why', async () => {
    const v = view(1);
    statisticsSession.setFile(v);
    const { hook, commands } = setUp({
      stats_desc_open: () => {
        throw new Error(refusal);
      },
    });
    await act(() => hook.result.current.choose());
    expect(commands()).toEqual(['stats_desc_open']);
    expect(statisticsSession.get().view).toBe(v);
    expect(hook.result.current.error).toBeTruthy();
  });

  it('is busy from the picker until the open has settled', async () => {
    let finish: (p: string | null) => void = () => undefined;
    const { hook } = setUp({ stats_desc_open: view(1) }, () => new Promise((r) => (finish = r)));
    let done: Promise<void> = Promise.resolve();
    act(() => {
      done = hook.result.current.choose();
    });
    expect(statisticsSession.get().busy).toBe(true);
    await act(async () => {
      finish('/x/y.csv');
      await done;
    });
    expect(statisticsSession.get().busy).toBe(false);
  });

  it('a file that finishes opening after the session was cleared is closed, not kept', async () => {
    let finish: (p: string | null) => void = () => undefined;
    const { hook, bridge, commands } = setUp({ stats_desc_open: view(3), stats_desc_close: true }, () => new Promise((r) => (finish = r)));
    let done: Promise<void> = Promise.resolve();
    act(() => {
      done = hook.result.current.choose();
    });
    statisticsSession.clear();
    await act(async () => {
      finish('/x/y.csv');
      await done;
    });
    expect(statisticsSession.get().view).toBeNull();
    await waitFor(() => expect(commands()).toEqual(['stats_desc_open', 'stats_desc_close']));
    expect(bridge.calls[1][1]).toEqual({ slot: 3 });
  });
});

describe('Close this file', () => {
  it.each([[true], [false]])('sends the close, and the file goes whether Rust held the slot (%s) or not', async (held) => {
    statisticsSession.setFile(view(1));
    const { hook, bridge, commands } = setUp({ stats_desc_close: held });
    await act(() => hook.result.current.closeFile());
    expect(commands()).toEqual(['stats_desc_close']);
    expect(bridge.calls[0][1]).toEqual({ slot: 1 });
    expect(statisticsSession.get().view).toBeNull();
  });

  it('a refused close says why, and the file stays', async () => {
    const v = view(1);
    statisticsSession.setFile(v);
    const { hook } = setUp({
      stats_desc_close: () => {
        throw new Error(refusal);
      },
    });
    await act(() => hook.result.current.closeFile());
    expect(statisticsSession.get().view).toBe(v);
    expect(hook.result.current.error).toBeTruthy();
  });
});

describe('leaving the window', () => {
  it('closes nothing: the store keeps the file', async () => {
    const { hook, commands } = setUp({ stats_desc_open: view(1) });
    await act(() => hook.result.current.choose());
    hook.unmount();
    expect(commands()).toEqual(['stats_desc_open']);
    expect(statisticsSession.get().slot).toBe(1);
  });
});
