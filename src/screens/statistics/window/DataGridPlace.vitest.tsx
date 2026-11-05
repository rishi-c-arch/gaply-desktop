// The grid keeps its place across a change of tab (Research OS, Phase 8R, decision
// 473): its scroll and active cell, in the session beside the view, restored in a
// layout effect, keyed by the slot and the hash as the grid is, and lost when the
// view changes. jsdom has no layout, so the box's size is stubbed.
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import { act } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { makeMockDescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { DataGrid } from './DataGrid';
import { statisticsSession } from './statisticsSession';

afterEach(() => {
  cleanup();
  statisticsSession.clear();
  vi.restoreAllMocks();
});

const column = (i: number): ColumnProfile => ({ alias: `C${i + 1}`, header: `v${i + 1}`, kind: 'numeric', n: 2000, missing: 0, distinct: 2, levels: null, suspectMissingCodes: [], nonFinite: 0 });
const viewOf = (slot: number, hash: string): DatasetView => ({
  slot,
  displayName: 'a.csv',
  profile: { rows: 2000, columns: Array.from({ length: 30 }, (_, i) => column(i)) },
  inputRef: { contentHash: hash.repeat(64).slice(0, 64), rows: 2000, columns: 30, semanticRole: 'table' },
});
function stubBox() {
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(560);
  vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(1000);
}
const source = (v: DatasetView) =>
  makeMockDescribeBridge({
    stats_desc_cells: (a: Record<string, unknown>) => ({ slot: v.slot, inputHash: v.inputRef.contentHash, rows: 2000, columns: 30, firstRow: a.firstRow, firstColumn: a.firstColumn, cells: [] }),
  });
const grid = () => screen.getByTestId('sw-grid');

/** A view, mounted as the window mounts it, moved by the keyboard and a scroll. */
function placed(v: DatasetView) {
  stubBox();
  act(() => statisticsSession.setFile(v));
  const utils = render(<DataGrid view={v} cells={source(v)} />);
  fireEvent.keyDown(grid(), { key: 'ArrowDown' });
  fireEvent.keyDown(grid(), { key: 'ArrowRight' });
  fireEvent.scroll(grid(), { target: { scrollTop: 24 * 500, scrollLeft: 112 * 5 } });
  return utils;
}
const activeRow = () => screen.getByTestId('sw-active-cell').textContent;

describe("the grid's place", () => {
  it('comes back after the grid was unmounted, as a change of tab does', () => {
    const v = viewOf(1, 'a');
    const { unmount } = placed(v);
    expect(activeRow()).toContain('Row 2, C2');
    unmount();
    render(<DataGrid view={v} cells={source(v)} />);
    expect(grid().scrollTop).toBe(24 * 500);
    expect(grid().scrollLeft).toBe(112 * 5);
    expect(activeRow()).toContain('Row 2, C2');
    expect(screen.getByTestId(`sw-rownum-${500}`)).toBeTruthy();
  });

  it('is lost when the view changes: a make keeps the slot and changes the hash', () => {
    const v = viewOf(1, 'a');
    const { unmount } = placed(v);
    unmount();
    const made = viewOf(1, 'b');
    act(() => statisticsSession.setFile(made));
    render(<DataGrid view={made} cells={source(made)} />);
    expect(grid().scrollTop).toBe(0);
    expect(activeRow()).toContain('Row 1, C1');
  });

  it('is lost on a new file, a close and a clear', () => {
    for (const lose of [() => statisticsSession.setFile(viewOf(2, 'c')), () => statisticsSession.clearFile(), () => statisticsSession.clear()]) {
      const v = viewOf(1, 'a');
      const { unmount } = placed(v);
      unmount();
      expect(statisticsSession.get().gridPlace).not.toBeNull();
      act(lose);
      expect(statisticsSession.get().gridPlace).toBeNull();
      act(() => statisticsSession.clear());
      cleanup();
    }
  });

  it('a grid of a view that is no longer the session\'s writes no place', () => {
    const v = viewOf(1, 'a');
    const { unmount } = placed(v);
    act(() => statisticsSession.setFile(viewOf(2, 'c')));
    unmount();
    expect(statisticsSession.get().gridPlace).toBeNull();
  });

  it('holds numbers only, never a cell', () => {
    const v = viewOf(1, 'a');
    const { unmount } = placed(v);
    unmount();
    const p = statisticsSession.get().gridPlace;
    expect(p && Object.keys(p).sort()).toEqual(['active', 'key', 'left', 'top']);
    expect(p?.key).toBe(`1:${v.inputRef.contentHash}`);
  });
});
