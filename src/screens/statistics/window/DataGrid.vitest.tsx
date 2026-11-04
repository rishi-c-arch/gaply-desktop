// The Data View's grid (Research OS, Phase 8Q, decisions 460, 461 and 463). jsdom
// has no layout, so the box is stubbed; the real-app check reads what it does.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { makeMockDescribeBridge } from '../describeBridge';
import type { CellWindowView, ColumnProfile, DatasetView } from '../describeTypes';
import { refusalSentence } from '../describeWording';
import { DataGrid } from './DataGrid';
import { HEAD_HEIGHT, OVERSCAN_COLUMNS, OVERSCAN_ROWS, ROW_HEIGHT, ROW_NUMBER_WIDTH } from './gridRange';
import { SLOT_GONE, statisticsSession } from './statisticsSession';

vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

const proto = HTMLElement.prototype as unknown as Record<string, unknown>;
function stubBox(height: number, width: number) {
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => height });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => width });
}

afterEach(() => {
  cleanup();
  delete proto.clientHeight;
  delete proto.clientWidth;
  statisticsSession.clear();
});

const column = (i: number, over: Partial<ColumnProfile> = {}): ColumnProfile => ({
  alias: `C${i + 1}`,
  header: `h${i + 1}`,
  kind: 'numeric',
  n: 1,
  missing: 0,
  distinct: 1,
  levels: null,
  suspectMissingCodes: [],
  nonFinite: 0,
  ...over,
});

const viewOf = (rows: number, columns: ColumnProfile[], slot = 3, hash = 'ab'.repeat(32)): DatasetView => ({
  slot,
  displayName: 'survey.csv',
  profile: { rows, columns },
  inputRef: { contentHash: hash, rows, columns: columns.length, semanticRole: 'table' },
});
const wide = (rows: number, n: number, slot = 3, hash = 'ab'.repeat(32)) =>
  viewOf(rows, Array.from({ length: n }, (_, i) => column(i)), slot, hash);

/** Answers any window, each cell `r{row}c{column}` by `cell` unless given. */
function answering(view: DatasetView, cell: (r: number, c: number) => string | null = (r, c) => `r${r}c${c}`) {
  return makeMockDescribeBridge({
    stats_desc_cells: (a: Record<string, unknown>): CellWindowView => {
      const [fr, rc, fc, cc] = [a.firstRow, a.rowCount, a.firstColumn, a.columnCount] as number[];
      const rows = view.profile.rows;
      const cols = view.profile.columns.length;
      const cells: Array<Array<string | null>> = [];
      for (let r = fr; r < Math.min(rows, fr + rc); r += 1) {
        const row: Array<string | null> = [];
        for (let c = fc; c < Math.min(cols, fc + cc); c += 1) row.push(cell(r, c));
        cells.push(row);
      }
      return { slot: view.slot, inputHash: view.inputRef.contentHash, rows, columns: cols, firstRow: fr, firstColumn: fc, cells };
    },
  });
}

const cellAt = (r: number, c: number) => screen.findByTestId(`sw-cell-${r}-${c}`);
const grid = () => screen.getByTestId('sw-grid');

function drawn(view: DatasetView, cells = answering(view)) {
  stubBox(560, 1000);
  const utils = render(<DataGrid view={view} cells={cells} />);
  return { ...utils, cells };
}

describe('the roles', () => {
  it('a read-only grid, counting the header row and the row-number column', async () => {
    drawn(wide(2000, 20));
    await cellAt(0, 0);
    const g = screen.getByRole('grid');
    expect(g.getAttribute('aria-rowcount')).toBe('2001');
    expect(g.getAttribute('aria-colcount')).toBe('21');
    expect(g.getAttribute('aria-readonly')).toBe('true');
    expect(g.getAttribute('tabindex')).toBe('0');
    expect(g.getAttribute('aria-label')).toMatch(/\S/);
  });

  it('each drawn row, cell, head and row number says where it is', async () => {
    drawn(wide(2000, 20));
    const cell = await cellAt(4, 2);
    expect(cell.getAttribute('role')).toBe('gridcell');
    expect(cell.getAttribute('aria-colindex')).toBe('4');
    expect(cell.closest('[role="row"]')?.getAttribute('aria-rowindex')).toBe('6');
    const head = screen.getByTestId('sw-head-2');
    expect(head.getAttribute('role')).toBe('columnheader');
    expect(head.getAttribute('aria-colindex')).toBe('4');
    expect(head.closest('[role="row"]')?.getAttribute('aria-rowindex')).toBe('1');
    const number = screen.getByTestId('sw-rownum-4');
    expect(number.getAttribute('role')).toBe('rowheader');
    expect(number.getAttribute('aria-colindex')).toBe('1');
    expect(number.textContent).toBe('5');
  });

  it('focus is by aria-activedescendant, on the active cell\'s own id', async () => {
    drawn(wide(2000, 20));
    const first = await cellAt(0, 0);
    expect(grid().getAttribute('aria-activedescendant')).toBe(first.id);
    fireEvent.keyDown(grid(), { key: 'ArrowDown' });
    const next = await cellAt(1, 0);
    expect(grid().getAttribute('aria-activedescendant')).toBe(next.id);
    expect(next.getAttribute('aria-selected')).toBe('true');
    expect(first.getAttribute('aria-selected')).toBe('false');
  });
});

describe('a column\'s head', () => {
  it('two lines: its alias and its name, inside bdi', async () => {
    drawn(viewOf(5, [column(0, { header: 'age' })]));
    const head = await screen.findByTestId('sw-head-0');
    expect(head.textContent).toContain('C1');
    expect(head.querySelector('bdi')?.textContent).toBe('age');
    expect(head.textContent).not.toContain('prepared');
  });

  it('a prepared column: its typed name, the word, and an edge', async () => {
    drawn(viewOf(5, [column(0, { alias: 'C2', header: 'C2', prepared: true, scoreName: 'wellbeing' })]));
    const head = await screen.findByTestId('sw-head-0');
    expect(head.querySelector('bdi')?.textContent).toBe('wellbeing');
    expect(head.textContent).toContain('prepared');
    expect(head.className).toContain('sw-grid__head--prepared');
  });

  it('widths by kind: 112 px, 200 px for categorical', async () => {
    drawn(viewOf(5, [column(0), column(1, { kind: 'categorical', levels: [] }), column(2, { kind: 'empty' })]));
    await cellAt(0, 2);
    expect(screen.getByTestId('sw-head-0').style.width).toBe('112px');
    expect(screen.getByTestId('sw-head-1').style.width).toBe('200px');
    expect(screen.getByTestId('sw-head-2').style.width).toBe('112px');
    expect(screen.getByTestId('sw-head-1').style.left).toBe(`${ROW_NUMBER_WIDTH + 112}px`);
    expect((await cellAt(0, 1)).style.width).toBe('200px');
  });
});

describe('a cell', () => {
  it('a number as Rust wrote it, right-aligned by its column\'s kind', async () => {
    const view = viewOf(1, [column(0), column(1, { kind: 'categorical', levels: [] })]);
    drawn(view, answering(view, (_r, c) => (c === 0 ? '-0' : 'text')));
    const n = await cellAt(0, 0);
    await waitFor(() => expect(n.textContent).toBe('-0'));
    expect(n.className).toContain('sw-grid__cell--number');
    expect((await cellAt(0, 1)).className).not.toContain('sw-grid__cell--number');
  });

  it('text is inside bdi and a text node, never HTML', async () => {
    const payload = '<img src=x onerror=alert(1)>';
    const view = viewOf(1, [column(0, { kind: 'categorical', levels: [] })]);
    drawn(view, answering(view, () => payload));
    const cell = await cellAt(0, 0);
    await waitFor(() => expect(cell.textContent).toBe(payload));
    expect(cell.querySelector('img')).toBeNull();
    const bdi = cell.querySelector('bdi') as HTMLElement;
    expect(bdi.childNodes).toHaveLength(1);
    expect(bdi.firstChild?.nodeType).toBe(Node.TEXT_NODE);
    expect(document.querySelector('img')).toBeNull();
  });

  it('a missing cell is empty, and read as empty', async () => {
    const view = wide(1, 1);
    drawn(view, answering(view, () => null));
    const cell = await cellAt(0, 0);
    await waitFor(() => expect(cell.getAttribute('aria-label')).toBe('empty'));
    expect(cell.textContent).toBe('');
    expect(cell.getAttribute('aria-busy')).toBeNull();
  });

  it('a cell not yet here is blank and busy', async () => {
    const view = wide(1, 1);
    stubBox(560, 1000);
    let answer: (w: CellWindowView) => void = () => undefined;
    const cells = { cells: () => new Promise<CellWindowView>((r) => (answer = r)) };
    render(<DataGrid view={view} cells={cells} />);
    const cell = await cellAt(0, 0);
    expect(cell.textContent).toBe('');
    expect(cell.getAttribute('aria-busy')).toBe('true');
    act(() => answer({ slot: 3, inputHash: view.inputRef.contentHash, rows: 1, columns: 1, firstRow: 0, firstColumn: 0, cells: [['7']] }));
    await waitFor(() => expect(cell.textContent).toBe('7'));
    expect(cell.getAttribute('aria-busy')).toBeNull();
  });
});

describe('only what is in view is drawn', () => {
  it('at 2,000 x 20, 0 < drawn cells <= a bound from the box, before and after a scroll', async () => {
    const bound = (Math.ceil((560 - HEAD_HEIGHT) / ROW_HEIGHT) + 1 + 2 * OVERSCAN_ROWS) * (Math.ceil((1000 - ROW_NUMBER_WIDTH) / 112) + 1 + 2 * OVERSCAN_COLUMNS);
    drawn(wide(2000, 20));
    await cellAt(0, 0);
    const count = () => document.querySelectorAll('[role="gridcell"]').length;
    expect(count()).toBeGreaterThan(0);
    expect(count()).toBeLessThanOrEqual(bound);
    fireEvent.scroll(grid(), { target: { scrollTop: 24 * 1000, scrollLeft: 112 * 5 } });
    await cellAt(1000, 5);
    expect(count()).toBeGreaterThan(0);
    expect(count()).toBeLessThanOrEqual(bound);
    expect(screen.queryByTestId('sw-cell-0-0')).toBeNull();
    expect(screen.queryByTestId('sw-rownum-0')).toBeNull();
  });

  it('the spacer is rows x 24 px by the columns\' widths, past the head and the row numbers', async () => {
    drawn(viewOf(100_000, [column(0), column(1, { kind: 'categorical', levels: [] })]));
    await cellAt(0, 0);
    const spacer = screen.getByTestId('sw-grid-spacer');
    expect(spacer.style.height).toBe(`${HEAD_HEIGHT + 2_400_000}px`);
    expect(spacer.style.width).toBe(`${ROW_NUMBER_WIDTH + 312}px`);
  });

  it('a box with no size draws nothing and asks nothing', async () => {
    const view = wide(2000, 20);
    const cells = answering(view);
    render(<DataGrid view={view} cells={cells} />);
    await act(async () => undefined);
    expect(document.querySelectorAll('[role="gridcell"]')).toHaveLength(0);
    expect(cells.calls).toEqual([]);
  });

  it('a scroll asks only for the windows it lacks', async () => {
    const { cells } = drawn(wide(2000, 20));
    await cellAt(0, 0);
    const before = cells.calls.length;
    fireEvent.scroll(grid(), { target: { scrollTop: 24 } });
    await act(async () => undefined);
    expect(cells.calls.length).toBe(before);
    fireEvent.scroll(grid(), { target: { scrollTop: 24 * 500 } });
    await cellAt(500, 0);
    expect(cells.calls.length).toBeGreaterThan(before);
    for (const [, a] of cells.calls) {
      expect(a.rowCount as number).toBeLessThanOrEqual(200);
      expect(a.columnCount as number).toBeLessThanOrEqual(64);
    }
  });
});

describe('a stale window', () => {
  it('whose inputHash is not the open view\'s is dropped, and never drawn', async () => {
    const view = wide(5, 2);
    const other = wide(5, 2, 3, 'cd'.repeat(32));
    drawn(view, answering(other, () => 'STALE_VALUE'));
    const cell = await cellAt(0, 0);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(cell.textContent).toBe('');
    expect(cell.getAttribute('aria-busy')).toBe('true');
    expect(document.body.textContent).not.toContain('STALE_VALUE');
  });
});

describe('a change of the session\'s view empties the cache', () => {
  const twice = async (next: DatasetView) => {
    const first = wide(5, 2);
    const firstCells = answering(first, () => 'OLD');
    stubBox(560, 1000);
    const u = render(<DataGrid view={first} cells={firstCells} />);
    await waitFor(() => expect(screen.getByTestId('sw-cell-0-0').textContent).toBe('OLD'));
    const never = { cells: vi.fn(() => new Promise<CellWindowView>(() => undefined)) };
    u.rerender(<DataGrid view={next} cells={never} />);
    await waitFor(() => expect(never.cells).toHaveBeenCalled());
    const cell = screen.getByTestId('sw-cell-0-0');
    expect(cell.textContent).toBe('');
    expect(cell.getAttribute('aria-busy')).toBe('true');
    expect(document.body.textContent).not.toContain('OLD');
  };

  it('a make: the same slot and a new inputHash', async () => {
    await twice(wide(5, 2, 3, 'cd'.repeat(32)));
  });

  it('an open: a new slot', async () => {
    await twice(wide(5, 2, 4, 'cd'.repeat(32)));
  });

  it('an answer for the old table arriving after the change is not drawn', async () => {
    const first = wide(5, 2);
    let late: (w: CellWindowView) => void = () => undefined;
    const slow = { cells: vi.fn(() => new Promise<CellWindowView>((r) => (late = r))) };
    stubBox(560, 1000);
    const u = render(<DataGrid view={first} cells={slow} />);
    await cellAt(0, 0);
    const next = wide(5, 2, 3, 'cd'.repeat(32));
    const never = { cells: vi.fn(() => new Promise<CellWindowView>(() => undefined)) };
    u.rerender(<DataGrid view={next} cells={never} />);
    await waitFor(() => expect(never.cells).toHaveBeenCalled());
    await act(async () => {
      late({ slot: 3, inputHash: first.inputRef.contentHash, rows: 5, columns: 2, firstRow: 0, firstColumn: 0, cells: [['LATE', 'LATE']] });
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(document.body.textContent).not.toContain('LATE');
  });
});

describe('a refusal', () => {
  it('run.unknown_input: the session\'s slotGone, its first caller', async () => {
    const view = wide(5, 2);
    act(() => statisticsSession.setFile(view));
    const cells = { cells: () => Promise.reject('{"code":"run.unknown_input"}') };
    stubBox(560, 1000);
    render(<DataGrid view={view} cells={cells} />);
    await waitFor(() => expect(statisticsSession.get().notice).toBe(SLOT_GONE));
    expect(statisticsSession.get().slot).toBeNull();
  });

  it('any other: its code\'s sentence under the grid, the method screens\' wording, and the cells blank', async () => {
    const view = wide(5, 2);
    act(() => statisticsSession.setFile(view));
    const refusal = '{"code":"run.run_busy"}';
    stubBox(560, 1000);
    render(<DataGrid view={view} cells={{ cells: () => Promise.reject(refusal) }} />);
    const said = await screen.findByTestId('sw-grid-error');
    expect(said.textContent).toBe(refusalSentence(refusal));
    expect(said.getAttribute('role')).toBe('alert');
    expect((await cellAt(0, 0)).textContent).toBe('');
    expect(statisticsSession.get().slot).toBe(3);
  });
});

describe('the keyboard', () => {
  it('moves the active cell, scrolls to it, and leaves Tab alone', async () => {
    drawn(wide(2000, 20));
    await cellAt(0, 0);
    const line = () => screen.getByTestId('sw-active-cell').textContent;
    expect(line()).toContain('Row 1, C1');
    expect(fireEvent.keyDown(grid(), { key: 'ArrowRight' })).toBe(false);
    expect(line()).toContain('Row 1, C2');
    expect(fireEvent.keyDown(grid(), { key: 'End' })).toBe(false);
    expect(line()).toContain('Row 1, C20');
    expect(grid().scrollLeft).toBe(20 * 112 - (1000 - 72));
    expect(fireEvent.keyDown(grid(), { key: 'Home' })).toBe(false);
    expect(line()).toContain('Row 1, C1');
    expect(grid().scrollLeft).toBe(0);
    expect(fireEvent.keyDown(grid(), { key: 'PageDown' })).toBe(false);
    expect(line()).toContain('Row 21, C1');
    expect(fireEvent.keyDown(grid(), { key: 'PageUp' })).toBe(false);
    expect(line()).toContain('Row 1, C1');
    expect(fireEvent.keyDown(grid(), { key: 'End', metaKey: true })).toBe(false);
    expect(line()).toContain('Row 2000, C20');
    expect(grid().scrollTop).toBe(2000 * 24 - (560 - 44));
    await cellAt(1999, 19);
    expect(fireEvent.keyDown(grid(), { key: 'Home', ctrlKey: true })).toBe(false);
    expect(line()).toContain('Row 1, C1');
    expect(grid().scrollTop).toBe(0);
    expect(fireEvent.keyDown(grid(), { key: 'Tab' })).toBe(true);
    expect(line()).toContain('Row 1, C1');
  });

  it('a click makes a cell the active one', async () => {
    drawn(wide(2000, 20));
    fireEvent.click(await cellAt(3, 2));
    expect(screen.getByTestId('sw-active-cell').textContent).toContain('Row 4, C3');
  });
});

describe('the active cell\'s line', () => {
  it('holds its row, its alias and name, and its whole value', async () => {
    const whole = 'x'.repeat(512);
    const view = viewOf(2, [column(0, { kind: 'categorical', levels: [], header: 'note' })]);
    drawn(view, answering(view, () => whole));
    const line = await screen.findByTestId('sw-active-cell');
    await waitFor(() => expect(within(line).getByTestId('sw-active-value').textContent).toBe(whole));
    expect(line.textContent).toContain('Row 1, C1');
    expect(line.querySelectorAll('bdi')[0].textContent).toBe('note');
    expect(within(line).getByTestId('sw-active-value').tagName).toBe('BDI');
  });

  it('a score by its typed name, a missing cell as empty, and a window not here as waiting', async () => {
    const view = viewOf(1, [column(0, { alias: 'C2', header: 'C2', prepared: true, scoreName: 'wellbeing' })]);
    let answer: (w: CellWindowView) => void = () => undefined;
    stubBox(560, 1000);
    render(<DataGrid view={view} cells={{ cells: () => new Promise<CellWindowView>((r) => (answer = r)) }} />);
    const line = await screen.findByTestId('sw-active-cell');
    expect(line.textContent).toContain('wellbeing');
    expect(within(line).getByTestId('sw-active-value').textContent).toBe('Reading this cell.');
    act(() => answer({ slot: 3, inputHash: view.inputRef.contentHash, rows: 1, columns: 1, firstRow: 0, firstColumn: 0, cells: [[null]] }));
    await waitFor(() => expect(within(line).getByTestId('sw-active-value').textContent).toBe('empty'));
  });

  it('keeps wanting the active cell\'s window when it is scrolled far out of view', async () => {
    // The first window is still on its way when the grid scrolls away, so it lands
    // only if the active cell\'s block is still wanted; a block nobody wants is
    // dropped (decision 460).
    const view = wide(2000, 20);
    const quick = answering(view);
    let first: () => void = () => undefined;
    const cells = {
      cells: (slot: number, fr: number, rc: number, fc: number, cc: number) => {
        const answer = quick.cells(slot, fr, rc, fc, cc);
        return fr > 0 ? answer : new Promise<CellWindowView>((resolve) => (first = () => void answer.then(resolve)));
      },
    };
    stubBox(560, 1000);
    render(<DataGrid view={view} cells={cells} />);
    await cellAt(0, 0);
    fireEvent.scroll(grid(), { target: { scrollTop: 24 * 1500 } });
    await waitFor(() => expect(screen.getByTestId('sw-cell-1500-0').textContent).toBe('r1500c0'));
    await act(async () => {
      first();
      await new Promise((r) => setTimeout(r, 0));
    });
    await waitFor(() => expect(within(screen.getByTestId('sw-active-cell')).getByTestId('sw-active-value').textContent).toBe('r0c0'));
    expect(grid().getAttribute('aria-activedescendant')).toBeNull();
  });

  it('a table with no rows has no line and no active cell', async () => {
    drawn(wide(0, 2));
    await screen.findByTestId('sw-head-0');
    expect(screen.queryByTestId('sw-active-cell')).toBeNull();
    expect(grid().getAttribute('aria-activedescendant')).toBeNull();
  });
});

describe('the grid\'s source', () => {
  const read = (f: string) =>
    fs
      .readFileSync(path.join(__dirname, f), 'utf8')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .split('\n')
      .map((l) => l.replace(/(^|\s)\/\/.*$/, '$1'))
      .join('\n');
  const FILES = ['DataGrid.tsx', 'gridCells.ts', 'gridRange.ts', 'gridTiming.ts', 'TimingLine.tsx'];

  it('throws only its closed code, never a cell\'s text', () => {
    const throws = FILES.flatMap((f) => (read(f).match(/\bthrow\b[^;]*;/g) ?? []).map((t) => t.replace(/\s+/g, ' ')));
    expect(throws).toEqual(['throw new Error(GRID_MALFORMED);']);
  });

  it('has no Copy handler and no clipboard', () => {
    for (const f of FILES) expect(read(f), f).not.toMatch(/onCopy|clipboard|execCommand/i);
  });

  it('its CSS: the cells cannot be selected, the active line can, and every colour is a token', () => {
    const css = fs.readFileSync(path.join(__dirname, 'DataGrid.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const rule = (s: string) => css.slice(css.indexOf(`${s} {`), css.indexOf('}', css.indexOf(`${s} {`)));
    expect(rule('.gds-root .sw-grid__cell')).toMatch(/user-select: none;/);
    expect(rule('.gds-root .sw-grid__active')).toMatch(/user-select: text;/);
    expect(rule('.gds-root .sw-body.sw-body--grid')).toMatch(/overflow: hidden;/);
    expect(rule('.gds-root .sw-body.sw-body--grid')).toMatch(/padding: 0;/);
    expect(rule('.gds-root .sw-grid')).toMatch(/overflow: auto;/);
    expect(rule('.gds-root .sw-grid__headrow')).toMatch(/position: sticky;/);
    expect(rule('.gds-root .sw-grid__rownum')).toMatch(/position: sticky;/);
    expect(rule('.gds-root .sw-grid__head--prepared')).toMatch(/var\(--g-accent\)/);
    expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
    expect(css).not.toMatch(/rgba?\(/);
  });
});
