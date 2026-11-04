// The grid's arithmetic (Research OS, Phase 8Q, decisions 460 and 461): what is
// drawn, which windows that needs, and where the keys move the active cell.
import { describe, expect, it } from 'vitest';
import {
  BLOCK_COLUMNS,
  BLOCK_ROWS,
  blocksFor,
  CATEGORICAL_WIDTH,
  COLUMN_WIDTH,
  columnOffsets,
  drawnRange,
  HEAD_HEIGHT,
  MAX_WINDOWS,
  moveActive,
  OVERSCAN_COLUMNS,
  OVERSCAN_ROWS,
  pageRows,
  reveal,
  ROW_HEIGHT,
  ROW_NUMBER_WIDTH,
} from './gridRange';

const flat = (n: number) => columnOffsets(Array.from({ length: n }, () => 'numeric' as const));

describe('the columns', () => {
  it('are 112 px, and 200 px for categorical', () => {
    expect([COLUMN_WIDTH, CATEGORICAL_WIDTH, ROW_NUMBER_WIDTH, ROW_HEIGHT, HEAD_HEIGHT]).toEqual([112, 200, 72, 24, 44]);
    expect(columnOffsets(['numeric', 'categorical', 'empty'])).toEqual([0, 112, 312, 424]);
  });
});

describe('the drawn range', () => {
  const box = { scrollTop: 0, scrollLeft: 0, clientHeight: 560, clientWidth: 1000, rows: 2000, offsets: flat(20) };

  it('is bounded by the box, at any scroll', () => {
    const bound = (Math.ceil((560 - HEAD_HEIGHT) / ROW_HEIGHT) + 1 + 2 * OVERSCAN_ROWS) * (Math.ceil((1000 - ROW_NUMBER_WIDTH) / COLUMN_WIDTH) + 1 + 2 * OVERSCAN_COLUMNS);
    for (const [scrollTop, scrollLeft] of [
      [0, 0],
      [1, 1],
      [24 * 1000 + 7, 500],
      [24 * 2000, 112 * 20],
    ]) {
      const r = drawnRange({ ...box, scrollTop, scrollLeft });
      const drawn = (r.endRow - r.firstRow) * (r.endColumn - r.firstColumn);
      expect(drawn).toBeGreaterThan(0);
      expect(drawn).toBeLessThanOrEqual(bound);
    }
  });

  it('holds what is in view, plus a few beyond, inside the table', () => {
    expect(drawnRange(box)).toEqual({ firstRow: 0, endRow: 22 + OVERSCAN_ROWS, firstColumn: 0, endColumn: 9 + OVERSCAN_COLUMNS });
    const mid = drawnRange({ ...box, scrollTop: 24 * 1000, scrollLeft: 112 * 5 });
    expect(mid.firstRow).toBe(1000 - OVERSCAN_ROWS);
    expect(mid.endRow).toBe(1000 + 22 + OVERSCAN_ROWS);
    expect(mid.firstColumn).toBe(5 - OVERSCAN_COLUMNS);
    const end = drawnRange({ ...box, scrollTop: 24 * 2000, scrollLeft: 112 * 20 });
    expect(end.endRow).toBe(2000);
    expect(end.endColumn).toBe(20);
  });

  it('finds the columns by their own widths', () => {
    const offsets = columnOffsets(['categorical', 'categorical', 'numeric', 'numeric', 'numeric', 'numeric', 'numeric']);
    const r = drawnRange({ ...box, offsets, rows: 10, clientWidth: 72 + 250, scrollLeft: 0 });
    expect(r.firstColumn).toBe(0);
    expect(r.endColumn).toBe(2 + OVERSCAN_COLUMNS);
    const on = drawnRange({ ...box, offsets, rows: 10, clientWidth: 72 + 112, scrollLeft: 400 });
    expect(on.firstColumn).toBe(Math.max(0, 2 - OVERSCAN_COLUMNS));
  });

  it('is empty for a box with no size, a table with no rows or no columns', () => {
    for (const b of [{ ...box, clientHeight: 0 }, { ...box, clientWidth: 0 }, { ...box, rows: 0 }, { ...box, offsets: [0] }]) {
      const r = drawnRange(b);
      expect((r.endRow - r.firstRow) * (r.endColumn - r.firstColumn)).toBe(0);
    }
  });
});

describe('the windows it needs', () => {
  it('a window is never cut short by the text cap, and stays inside the row and column caps', () => {
    // 512 characters at most, four bytes each at most (supplementary.rs:55).
    expect(BLOCK_ROWS * BLOCK_COLUMNS * 512 * 4).toBeLessThanOrEqual(4 * 1024 * 1024);
    expect(BLOCK_ROWS).toBeLessThanOrEqual(200);
    expect(BLOCK_COLUMNS).toBeLessThanOrEqual(64);
  });

  it('names each block the range touches, and the active cell\'s', () => {
    const range = { firstRow: 48, endRow: 52, firstColumn: 0, endColumn: 2 };
    expect(blocksFor(range, { row: 0, column: 0 })).toEqual([
      { row: 0, column: 0 },
      { row: 1, column: 0 },
    ]);
    expect(blocksFor(range, { row: 5000, column: 30 })).toEqual([
      { row: 0, column: 0 },
      { row: 1, column: 0 },
      { row: 100, column: 1 },
    ]);
  });

  it('fits the cache on a very large window', () => {
    // 5,120 x 2,880 px of narrow columns and rows, at the worst alignment.
    const r = drawnRange({ scrollTop: 24 * 49, scrollLeft: 112 * 23, clientHeight: 2880, clientWidth: 5120, rows: 100_000, offsets: flat(128) });
    expect(blocksFor(r, { row: 0, column: 0 }).length).toBeLessThanOrEqual(MAX_WINDOWS);
    expect(MAX_WINDOWS).toBe(16);
  });
});

describe('the keys', () => {
  const at = { row: 10, column: 5 };
  const move = (key: string, mod = false, from = at) => moveActive(key, mod, from, 100, 20, 21);

  it('the arrows move one cell and stop at the edge', () => {
    expect(move('ArrowDown')).toEqual({ row: 11, column: 5 });
    expect(move('ArrowUp')).toEqual({ row: 9, column: 5 });
    expect(move('ArrowLeft')).toEqual({ row: 10, column: 4 });
    expect(move('ArrowRight')).toEqual({ row: 10, column: 6 });
    expect(move('ArrowUp', false, { row: 0, column: 0 })).toEqual({ row: 0, column: 0 });
    expect(move('ArrowLeft', false, { row: 0, column: 0 })).toEqual({ row: 0, column: 0 });
    expect(move('ArrowDown', false, { row: 99, column: 19 })).toEqual({ row: 99, column: 19 });
    expect(move('ArrowRight', false, { row: 99, column: 19 })).toEqual({ row: 99, column: 19 });
  });

  it('Page Up and Down move a screen and stop at the edge', () => {
    expect(move('PageDown')).toEqual({ row: 31, column: 5 });
    expect(move('PageUp', false, { row: 30, column: 5 })).toEqual({ row: 9, column: 5 });
    expect(move('PageUp')).toEqual({ row: 0, column: 5 });
    expect(move('PageDown', false, { row: 90, column: 5 })).toEqual({ row: 99, column: 5 });
  });

  it('Home and End move along the row; with Cmd or Ctrl, to the file\'s ends', () => {
    expect(move('Home')).toEqual({ row: 10, column: 0 });
    expect(move('End')).toEqual({ row: 10, column: 19 });
    expect(move('Home', true)).toEqual({ row: 0, column: 0 });
    expect(move('End', true)).toEqual({ row: 99, column: 19 });
  });

  it('any other key does not move it', () => {
    expect(move('Tab')).toBeNull();
    expect(move('a')).toBeNull();
    expect(move('Enter')).toBeNull();
  });

  it('a page is the rows in view, less one, and at least one', () => {
    expect(pageRows(560)).toBe(20);
    expect(pageRows(44)).toBe(1);
    expect(pageRows(0)).toBe(1);
  });
});

describe('scrolling to the active cell', () => {
  const offsets = flat(20);
  const box = { scrollTop: 2400, scrollLeft: 560, clientHeight: 560, clientWidth: 1000 };

  it('stays put when the cell is in view', () => {
    expect(reveal({ row: 105, column: 6 }, box, offsets)).toEqual({ top: 2400, left: 560 });
  });

  it('clears the sticky head and the row numbers on the way up and left', () => {
    expect(reveal({ row: 50, column: 2 }, box, offsets)).toEqual({ top: 1200, left: 224 });
  });

  it('shows the whole cell on the way down and right', () => {
    const r = reveal({ row: 200, column: 19 }, box, offsets);
    expect(r.top).toBe(201 * 24 - (560 - 44));
    expect(r.left).toBe(20 * 112 - (1000 - 72));
  });
});
