// Gaply — the Data View grid's arithmetic (Research OS, Phase 8Q, decisions 460
// and 461; ADR-012 Part 3): what is drawn, which windows that needs, and where a
// key moves the active cell. Numbers only, and no cell.
import type { ColumnKind } from '../describeTypes';

export const ROW_HEIGHT = 24;
export const HEAD_HEIGHT = 44;
export const ROW_NUMBER_WIDTH = 72;
export const COLUMN_WIDTH = 112;
export const CATEGORICAL_WIDTH = 200;
/** Rows and columns drawn beyond what is in view. */
export const OVERSCAN_ROWS = 3;
export const OVERSCAN_COLUMNS = 2;
/** The shape of a window the grid asks for. At 512 characters a cell and four
 *  bytes a character, one holds at most 2.4 MiB, so Rust's 4 MiB cap never cuts
 *  it short (decision 458), and 16 of them hold at most 39 MiB. */
export const BLOCK_ROWS = 50;
export const BLOCK_COLUMNS = 24;
export const MAX_WINDOWS = 16;

export interface DrawnRange {
  firstRow: number;
  /** One past the last. */
  endRow: number;
  firstColumn: number;
  /** One past the last. */
  endColumn: number;
}

export interface GridCell {
  row: number;
  column: number;
}

/** A window, by its place in whole blocks. */
export interface Block {
  row: number;
  column: number;
}

export const NOTHING_DRAWN: DrawnRange = { firstRow: 0, endRow: 0, firstColumn: 0, endColumn: 0 };

/** Where each column starts, past the row numbers, and last the columns' whole
 *  width: n + 1 numbers. */
export function columnOffsets(kinds: ColumnKind[]): number[] {
  const offsets = [0];
  for (const kind of kinds) offsets.push(offsets[offsets.length - 1] + (kind === 'categorical' ? CATEGORICAL_WIDTH : COLUMN_WIDTH));
  return offsets;
}

/** The first column whose end is past `x`. */
function columnAt(offsets: number[], x: number): number {
  let low = 0;
  let high = offsets.length - 1;
  while (low < high) {
    const mid = Math.floor((low + high) / 2);
    if (offsets[mid + 1] > x) high = mid;
    else low = mid + 1;
  }
  return low;
}

export function drawnRange(box: { scrollTop: number; scrollLeft: number; clientHeight: number; clientWidth: number; rows: number; offsets: number[] }): DrawnRange {
  const columns = box.offsets.length - 1;
  if (box.clientHeight <= 0 || box.clientWidth <= 0 || columns <= 0) return NOTHING_DRAWN;
  const top = Math.max(0, box.scrollTop);
  const left = Math.max(0, box.scrollLeft);
  const firstRow = Math.floor(top / ROW_HEIGHT);
  const endRow = Math.ceil((top + Math.max(0, box.clientHeight - HEAD_HEIGHT)) / ROW_HEIGHT);
  const firstColumn = columnAt(box.offsets, left);
  const right = left + Math.max(0, box.clientWidth - ROW_NUMBER_WIDTH);
  const endColumn = Math.min(columns, columnAt(box.offsets, Math.max(left, right - 1)) + 1);
  return {
    firstRow: Math.max(0, Math.min(box.rows - 1, firstRow - OVERSCAN_ROWS)),
    endRow: Math.max(0, Math.min(box.rows, endRow + OVERSCAN_ROWS)),
    firstColumn: Math.max(0, Math.min(firstColumn, columns - 1) - OVERSCAN_COLUMNS),
    endColumn: Math.min(columns, endColumn + OVERSCAN_COLUMNS),
  };
}

export function sameRange(a: DrawnRange, b: DrawnRange): boolean {
  return a.firstRow === b.firstRow && a.endRow === b.endRow && a.firstColumn === b.firstColumn && a.endColumn === b.endColumn;
}

/** The window holding a cell. */
export function blockOf(row: number, column: number): Block {
  return { row: Math.floor(row / BLOCK_ROWS), column: Math.floor(column / BLOCK_COLUMNS) };
}

/** Each window the range touches, then the active cell's, so its line can be read
 *  when it has been scrolled out of view. Nothing drawn asks for nothing. */
export function blocksFor(range: DrawnRange, active: GridCell | null): Block[] {
  const out: Block[] = [];
  if (range.endRow <= range.firstRow || range.endColumn <= range.firstColumn) return out;
  const first = blockOf(range.firstRow, range.firstColumn);
  const last = blockOf(range.endRow - 1, range.endColumn - 1);
  for (let r = first.row; r <= last.row; r += 1) {
    for (let c = first.column; c <= last.column; c += 1) out.push({ row: r, column: c });
  }
  if (active) {
    const own = blockOf(active.row, active.column);
    if (!out.some((b) => b.row === own.row && b.column === own.column)) out.push(own);
  }
  return out;
}

/** The rows a Page key moves: those in view, less one, at least one. */
export function pageRows(clientHeight: number): number {
  return Math.max(1, Math.floor((clientHeight - HEAD_HEIGHT) / ROW_HEIGHT) - 1);
}

/** Where a key moves the active cell, or null for a key that does not move it. */
export function moveActive(key: string, modifier: boolean, at: GridCell, rows: number, columns: number, page: number): GridCell | null {
  const row = (r: number) => Math.max(0, Math.min(rows - 1, r));
  const column = (c: number) => Math.max(0, Math.min(columns - 1, c));
  switch (key) {
    case 'ArrowUp':
      return { row: row(at.row - 1), column: at.column };
    case 'ArrowDown':
      return { row: row(at.row + 1), column: at.column };
    case 'ArrowLeft':
      return { row: at.row, column: column(at.column - 1) };
    case 'ArrowRight':
      return { row: at.row, column: column(at.column + 1) };
    case 'PageUp':
      return { row: row(at.row - page), column: at.column };
    case 'PageDown':
      return { row: row(at.row + page), column: at.column };
    case 'Home':
      return modifier ? { row: 0, column: 0 } : { row: at.row, column: 0 };
    case 'End':
      return modifier ? { row: rows - 1, column: columns - 1 } : { row: at.row, column: columns - 1 };
    default:
      return null;
  }
}

/** Where to scroll so the whole cell shows, clear of the sticky head and row
 *  numbers; where it is, if it already does. */
export function reveal(at: GridCell, box: { scrollTop: number; scrollLeft: number; clientHeight: number; clientWidth: number }, offsets: number[]): { top: number; left: number } {
  const rowTop = at.row * ROW_HEIGHT;
  const viewHeight = Math.max(ROW_HEIGHT, box.clientHeight - HEAD_HEIGHT);
  let top = box.scrollTop;
  if (rowTop < top) top = rowTop;
  else if (rowTop + ROW_HEIGHT > top + viewHeight) top = rowTop + ROW_HEIGHT - viewHeight;
  const start = offsets[at.column];
  const end = offsets[at.column + 1];
  const viewWidth = Math.max(0, box.clientWidth - ROW_NUMBER_WIDTH);
  let left = box.scrollLeft;
  if (start < left) left = start;
  else if (end > left + viewWidth) left = end - viewWidth;
  return { top, left };
}
