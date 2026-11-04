// The grid's windows of cells (Research OS, Phase 8Q, decision 460): the newest
// request wins, a window for another table is dropped, at most 16 are held, and
// nothing thrown or said holds a cell.
import { describe, expect, it, vi } from 'vitest';
import type { CellWindowView } from '../describeTypes';
import { GridCells, GRID_MALFORMED } from './gridCells';
import { GRID_UNREADABLE } from './windowWording';

const HASH = 'ab'.repeat(32);
const OTHER = 'cd'.repeat(32);

type Ask = { firstRow: number; firstColumn: number; resolve: (w: unknown) => void; reject: (e: unknown) => void };

function setUp() {
  const asks: Ask[] = [];
  const source = {
    cells: vi.fn((_slot: number, firstRow: number, _rows: number, firstColumn: number, _columns: number) => new Promise<CellWindowView>((resolve, reject) => {
      asks.push({ firstRow, firstColumn, resolve: resolve as (w: unknown) => void, reject });
    })),
  };
  const on = { changed: vi.fn(), slotGone: vi.fn(), refused: vi.fn(), roundTrip: vi.fn() };
  const store = new GridCells(source, 7, HASH, on);
  store.attach();
  return { asks, source, on, store };
}

const win = (firstRow: number, firstColumn: number, value = 'v', over: Partial<CellWindowView> = {}): CellWindowView => ({
  slot: 7,
  inputHash: HASH,
  rows: 100_000,
  columns: 128,
  firstRow,
  firstColumn,
  cells: [[value, null]],
  ...over,
});
const tick = () => new Promise((r) => setTimeout(r, 0));

describe('asking', () => {
  it('asks once for each window it lacks, in blocks', async () => {
    const { asks, source, store } = setUp();
    store.want([{ row: 0, column: 0 }, { row: 2, column: 1 }]);
    store.want([{ row: 0, column: 0 }, { row: 2, column: 1 }]);
    expect(source.cells).toHaveBeenCalledTimes(2);
    expect(source.cells).toHaveBeenCalledWith(7, 0, 50, 0, 24);
    expect(source.cells).toHaveBeenCalledWith(7, 100, 50, 24, 24);
    asks[0].resolve(win(0, 0));
    await tick();
    store.want([{ row: 0, column: 0 }]);
    expect(source.cells).toHaveBeenCalledTimes(2);
  });

  it('a cell is undefined until its window is here, then its string, or null for missing', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    expect(store.value(0, 0)).toBeUndefined();
    asks[0].resolve(win(0, 0, '1.5'));
    await tick();
    expect(on.changed).toHaveBeenCalled();
    expect(store.value(0, 0)).toBe('1.5');
    expect(store.value(0, 1)).toBeNull();
    expect(store.value(1, 0)).toBeUndefined();
    expect(store.value(0, 2)).toBeUndefined();
  });
});

describe('dropping', () => {
  it('drops a window whose inputHash is not the open view\'s', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].resolve(win(0, 0, 'stale', { inputHash: OTHER }));
    await tick();
    expect(store.value(0, 0)).toBeUndefined();
    expect(store.size()).toBe(0);
    expect(on.changed).not.toHaveBeenCalled();
  });

  it('drops a window for another slot', async () => {
    const { asks, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].resolve(win(0, 0, 'stale', { slot: 8 }));
    await tick();
    expect(store.value(0, 0)).toBeUndefined();
  });

  it('the newest request wins: an answer no longer wanted is dropped, and one wanted again is asked again', async () => {
    const { asks, source, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    store.want([{ row: 4, column: 0 }]);
    asks[0].resolve(win(0, 0, 'old'));
    asks[1].resolve(win(200, 0, 'new'));
    await tick();
    expect(store.value(0, 0)).toBeUndefined();
    expect(store.value(200, 0)).toBe('new');
    store.want([{ row: 0, column: 0 }]);
    expect(source.cells).toHaveBeenCalledTimes(3);
  });

  it('holds nothing and calls nothing once detached', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    store.detach();
    asks[0].resolve(win(0, 0));
    await tick();
    expect(store.size()).toBe(0);
    expect(on.changed).not.toHaveBeenCalled();
  });
});

describe('the cache', () => {
  it('holds at most 16 windows, the oldest going first and never one in view', async () => {
    const { asks, store } = setUp();
    for (let i = 0; i < 30; i += 1) {
      store.want([{ row: i, column: 0 }]);
      asks[i].resolve(win(i * 50, 0, `v${i}`));
      await tick();
      expect(store.size()).toBeLessThanOrEqual(16);
      expect(store.value(i * 50, 0), 'the one in view').toBe(`v${i}`);
    }
    expect(store.size()).toBe(16);
    expect(store.value(0, 0)).toBeUndefined();
    expect(store.value(29 * 50, 0)).toBe('v29');
    expect(store.value(14 * 50, 0)).toBe('v14');
  });

  it('asks for at most 16 at once', () => {
    const { source, store } = setUp();
    store.want(Array.from({ length: 40 }, (_, i) => ({ row: i, column: 0 })));
    expect(source.cells).toHaveBeenCalledTimes(16);
  });
});

describe('a refusal', () => {
  it('run.unknown_input says the slot is gone, and nothing else', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].reject('{"code":"run.unknown_input"}');
    await tick();
    expect(on.slotGone).toHaveBeenCalledTimes(1);
    expect(on.refused).not.toHaveBeenCalled();
  });

  it('any other gives its code\'s sentence, and the cells stay blank', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].reject('{"code":"run.run_busy"}');
    await tick();
    expect(on.slotGone).not.toHaveBeenCalled();
    expect(on.refused).toHaveBeenCalledTimes(1);
    expect(on.refused.mock.calls[0][0]).toMatch(/\S/);
    expect(store.value(0, 0)).toBeUndefined();
  });

  it('a later window clears the sentence', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].reject('{"code":"run.run_busy"}');
    await tick();
    store.want([{ row: 0, column: 0 }]);
    asks[1].resolve(win(0, 0));
    await tick();
    expect(on.refused).toHaveBeenLastCalledWith(null);
  });
});

describe('a malformed answer', () => {
  const SENTINEL = 'CELL_SENTINEL_q9';
  it.each([
    ['a number as a cell', { cells: [[1.5, SENTINEL]] }],
    ['an object as a cell', { cells: [[{ a: SENTINEL }]] }],
    ['a row that is not an array', { cells: [SENTINEL] }],
    ['no cells', { cells: SENTINEL }],
  ])('%s: a closed code and a fixed sentence, never the cell', async (_what, over) => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].resolve(win(0, 0, 'x', over as unknown as Partial<CellWindowView>));
    await tick();
    expect(store.size()).toBe(0);
    expect(on.refused).toHaveBeenCalledWith(GRID_UNREADABLE);
    expect(JSON.stringify(on.refused.mock.calls)).not.toContain(SENTINEL);
    expect(GRID_MALFORMED).toMatch(/^[a-z_.]+$/);
  });
});

describe('the round trip', () => {
  it('is reported as a number', async () => {
    const { asks, on, store } = setUp();
    store.want([{ row: 0, column: 0 }]);
    asks[0].resolve(win(0, 0));
    await tick();
    expect(on.roundTrip).toHaveBeenCalledTimes(1);
    expect(typeof on.roundTrip.mock.calls[0][0]).toBe('number');
  });
});
