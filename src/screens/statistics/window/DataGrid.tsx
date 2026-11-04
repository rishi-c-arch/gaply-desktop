// Gaply — the Data View's grid (Research OS, Phase 8Q, decisions 460 and 461;
// ADR-012 Part 3). Every row and column of the open file, read-only, drawn only
// where it is in view: one scroller over a spacer, the rows and columns in view
// absolutely placed, the head and the row numbers sticky. It holds no business
// logic: a cell is what Rust printed, and a cell carries no type, so a column's
// kind decides its alignment.
//
// A cell is always a text node. The cells reach this screen and nowhere else: no
// log, no error, no clipboard handler and no storage. One active cell and no
// range; its line under the grid is the one thing here a user may select.
import React, { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { describeBridge, type DescribeBridge } from '../describeBridge';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import './DataGrid.css';
import { GridCells } from './gridCells';
import { blocksFor, columnOffsets, drawnRange, HEAD_HEIGHT, moveActive, NOTHING_DRAWN, pageRows, reveal, ROW_HEIGHT, ROW_NUMBER_WIDTH, sameRange, type GridCell } from './gridRange';
import type { GridTiming } from './gridTiming';
import { statisticsSession } from './statisticsSession';
import { ACTIVE_WAITING, activeCellLabel, CELL_EMPTY, GRID_LABEL, PREPARED_WORD, ROW_HEAD } from './windowWording';

export interface DataGridProps {
  view: DatasetView;
  /** Reads a window of cells; describeBridge's by default. */
  cells?: Pick<DescribeBridge, 'cells'>;
  /** The counter, while statsTiming is on. */
  timing?: GridTiming;
}

const nameOf = (c: ColumnProfile): string => c.scoreName ?? c.header;

const ActiveLine: React.FC<{ at: GridCell; column: ColumnProfile; value: string | null | undefined }> = ({ at, column, value }) => (
  <p className="sw-grid__active" data-testid="sw-active-cell" tabIndex={0}>
    {activeCellLabel(at.row, column.alias)} <bdi>{nameOf(column)}</bdi>: <bdi data-testid="sw-active-value">{value === undefined ? ACTIVE_WAITING : value === null ? CELL_EMPTY : value}</bdi>
  </p>
);

const Grid: React.FC<Required<Pick<DataGridProps, 'view' | 'cells'>> & Pick<DataGridProps, 'timing'>> = ({ view, cells, timing }) => {
  const baseId = useId();
  const scroller = useRef<HTMLDivElement>(null);
  const columns = view.profile.columns;
  const rows = view.profile.rows;
  const offsets = useMemo(() => columnOffsets(columns.map((c) => c.kind)), [columns]);
  const [, setVersion] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [range, setRange] = useState(NOTHING_DRAWN);
  const [active, setActive] = useState<GridCell | null>(rows > 0 && columns.length > 0 ? { row: 0, column: 0 } : null);
  const opened = useRef(timing ? performance.now() : 0);
  const [store] = useState(
    () =>
      new GridCells(cells, view.slot, view.inputRef.contentHash, {
        changed: () => setVersion((v) => v + 1),
        slotGone: () => statisticsSession.slotGone(),
        refused: setError,
        roundTrip: timing ? (ms) => timing.noteWindow(ms) : undefined,
      })
  );

  const measure = useCallback(() => {
    const el = scroller.current;
    if (!el) return;
    const next = drawnRange({ scrollTop: el.scrollTop, scrollLeft: el.scrollLeft, clientHeight: el.clientHeight, clientWidth: el.clientWidth, rows, offsets });
    setRange((now) => (sameRange(now, next) ? now : next));
  }, [rows, offsets]);

  useEffect(() => {
    store.attach();
    return () => store.detach();
  }, [store]);

  useLayoutEffect(() => {
    measure();
    const el = scroller.current;
    if (!el || typeof ResizeObserver === 'undefined') return undefined;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [measure]);

  useEffect(() => store.want(blocksFor(range, active)), [store, range, active]);

  useEffect(() => () => timing?.stop(), [timing]);

  const here = store.value(range.firstRow, range.firstColumn) !== undefined;
  useEffect(() => {
    if (!timing || !here) return undefined;
    const frame = requestAnimationFrame(() => timing.notePaint(`${view.slot}:${view.inputRef.contentHash}`, performance.now() - opened.current));
    return () => cancelAnimationFrame(frame);
  }, [timing, here, view.slot, view.inputRef.contentHash]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const el = scroller.current;
    if (!active || !el) return;
    const next = moveActive(e.key, e.metaKey || e.ctrlKey, active, rows, columns.length, pageRows(el.clientHeight));
    if (!next) return;
    e.preventDefault();
    setActive(next);
    const to = reveal(next, el, offsets);
    el.scrollTop = to.top;
    el.scrollLeft = to.left;
    measure();
  };

  const onClick = (e: React.MouseEvent<HTMLDivElement>) => {
    const cell = (e.target as HTMLElement).closest<HTMLElement>('[data-r]');
    if (cell) setActive({ row: Number(cell.dataset.r), column: Number(cell.dataset.c) });
  };

  const cellId = (r: number, c: number) => `${baseId}-${r}-${c}`;
  const isDrawn = (at: GridCell) => at.row >= range.firstRow && at.row < range.endRow && at.column >= range.firstColumn && at.column < range.endColumn;
  const totalWidth = ROW_NUMBER_WIDTH + offsets[offsets.length - 1];
  const drawnColumns: number[] = [];
  for (let c = range.firstColumn; c < range.endColumn; c += 1) drawnColumns.push(c);
  const drawnRows: number[] = [];
  for (let r = range.firstRow; r < range.endRow; r += 1) drawnRows.push(r);

  return (
    <div className="sw-grid-area" data-testid="sw-grid-area">
      <div
        ref={scroller}
        className="sw-grid"
        data-testid="sw-grid"
        role="grid"
        tabIndex={0}
        aria-label={GRID_LABEL}
        aria-rowcount={rows + 1}
        aria-colcount={columns.length + 1}
        aria-readonly="true"
        aria-activedescendant={active && isDrawn(active) ? cellId(active.row, active.column) : undefined}
        onScroll={() => {
          timing?.scrolled();
          measure();
        }}
        onKeyDown={onKeyDown}
        onClick={onClick}
      >
        <div className="sw-grid__spacer" data-testid="sw-grid-spacer" role="presentation" style={{ width: totalWidth, height: HEAD_HEIGHT + rows * ROW_HEIGHT }}>
          <div className="sw-grid__headrow" role="row" aria-rowindex={1} style={{ width: totalWidth, height: HEAD_HEIGHT }}>
            <div className="sw-grid__corner" role="columnheader" aria-colindex={1} style={{ width: ROW_NUMBER_WIDTH }}>
              {ROW_HEAD}
            </div>
            {drawnColumns.map((c) => {
                const column = columns[c];
                return (
                  <div
                    key={column.alias}
                    className={`sw-grid__head${column.prepared === true ? ' sw-grid__head--prepared' : ''}`}
                    data-testid={`sw-head-${c}`}
                    role="columnheader"
                    aria-colindex={c + 2}
                    title={nameOf(column)}
                    style={{ left: ROW_NUMBER_WIDTH + offsets[c], width: offsets[c + 1] - offsets[c] }}
                  >
                    <span className="sw-grid__alias">
                      {column.alias}
                      {column.prepared === true && <span className="sw-grid__prepared"> {PREPARED_WORD}</span>}
                    </span>
                    <bdi className="sw-grid__name">{nameOf(column)}</bdi>
                  </div>
                );
            })}
          </div>
          {drawnRows.map((r) => (
            <div key={r} className="sw-grid__row" role="row" aria-rowindex={r + 2} style={{ top: HEAD_HEIGHT + r * ROW_HEIGHT, width: totalWidth, height: ROW_HEIGHT }}>
              <div className="sw-grid__rownum" data-testid={`sw-rownum-${r}`} role="rowheader" aria-colindex={1} style={{ width: ROW_NUMBER_WIDTH }}>
                {r + 1}
              </div>
              {drawnColumns.map((c) => {
                const value = store.value(r, c);
                const numeric = columns[c].kind === 'numeric';
                const isActive = active !== null && active.row === r && active.column === c;
                return (
                  <div
                    key={c}
                    id={cellId(r, c)}
                    className={`sw-grid__cell${numeric ? ' sw-grid__cell--number' : ''}${isActive ? ' sw-grid__cell--active' : ''}`}
                    data-testid={`sw-cell-${r}-${c}`}
                    data-r={r}
                    data-c={c}
                    role="gridcell"
                    aria-colindex={c + 2}
                    aria-selected={isActive}
                    aria-busy={value === undefined ? true : undefined}
                    aria-label={value === null ? CELL_EMPTY : undefined}
                    style={{ left: ROW_NUMBER_WIDTH + offsets[c], width: offsets[c + 1] - offsets[c] }}
                  >
                    {typeof value === 'string' && (numeric ? value : <bdi>{value}</bdi>)}
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
      {error && (
        <p className="sw-grid__error" role="alert" data-testid="sw-grid-error">
          {error}
        </p>
      )}
      {active && <ActiveLine at={active} column={columns[active.column]} value={store.value(active.row, active.column)} />}
    </div>
  );
};

export const DataGrid: React.FC<DataGridProps> = ({ view, cells, timing }) => {
  const source = useMemo(() => cells ?? describeBridge(), [cells]);
  // One Grid, and so one cache, one active cell and one scroll, per open view: a
  // make keeps the slot's number and changes the hash, so the key holds both.
  return <Grid key={`${view.slot}:${view.inputRef.contentHash}`} view={view} cells={source} timing={timing} />;
};
