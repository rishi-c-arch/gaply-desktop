// Gaply — the Data View grid's windows of cells (Research OS, Phase 8Q, decision
// 460; ADR-012 Part 3). One of these belongs to one open view, its slot and its
// inputHash, and holds at most 16 windows, in memory, for as long as the grid
// shows that view. It reaches Rust through stats_desc_cells alone. The newest
// request wins, and a window for another table is dropped, so a cell on screen is
// never a stale one.
//
// A thrown error can reach an error boundary's log, so a malformed answer throws
// one closed code, and nothing here ever puts a cell in an error or a sentence.
import { refusalOf } from '../../statsverifier/l1/l1Wording';
import type { DescribeBridge } from '../describeBridge';
import type { CellWindowView } from '../describeTypes';
import { refusalSentence } from '../describeWording';
import { BLOCK_COLUMNS, BLOCK_ROWS, blockOf, MAX_WINDOWS, type Block } from './gridRange';
import { GRID_UNREADABLE } from './windowWording';

export const GRID_MALFORMED = 'stats.grid.malformed_window';

export interface GridCellsEvents {
  /** A window landed, so what is drawn may differ. */
  changed: () => void;
  /** Rust no longer holds the slot. */
  slotGone: () => void;
  /** A refusal's sentence, or null when a later window has landed. */
  refused: (sentence: string | null) => void;
  /** How long the last window took, in ms; given only while the counter is on. */
  roundTrip?: (ms: number) => void;
}

const keyOf = (b: Block) => `${b.row}:${b.column}`;

function checked(w: CellWindowView): CellWindowView {
  const wellFormed =
    typeof w === 'object' &&
    w !== null &&
    Array.isArray(w.cells) &&
    w.cells.every((row) => Array.isArray(row) && row.every((cell) => cell === null || typeof cell === 'string'));
  if (!wellFormed) throw new Error(GRID_MALFORMED);
  return w;
}

export class GridCells {
  /** Oldest first: a window is moved to the back when it is wanted again. */
  private held = new Map<string, CellWindowView>();
  private asked = new Set<string>();
  private wanted = new Set<string>();
  private attached = false;

  constructor(
    private readonly source: Pick<DescribeBridge, 'cells'>,
    private readonly slot: number,
    private readonly inputHash: string,
    private readonly on: GridCellsEvents
  ) {}

  attach() {
    this.attached = true;
  }

  detach() {
    this.attached = false;
    this.held.clear();
  }

  /** These windows are what is wanted now: any held stay, the rest are asked for. */
  want(blocks: Block[]) {
    const now = blocks.slice(0, MAX_WINDOWS);
    this.wanted = new Set(now.map(keyOf));
    for (const block of now) {
      const key = keyOf(block);
      const window = this.held.get(key);
      if (window) {
        this.held.delete(key);
        this.held.set(key, window);
      } else if (!this.asked.has(key)) {
        this.ask(key, block);
      }
    }
  }

  private ask(key: string, block: Block) {
    this.asked.add(key);
    const started = this.on.roundTrip ? performance.now() : 0;
    this.source
      .cells(this.slot, block.row * BLOCK_ROWS, BLOCK_ROWS, block.column * BLOCK_COLUMNS, BLOCK_COLUMNS)
      .then((answer) => {
        this.asked.delete(key);
        if (!this.attached) return;
        const window = checked(answer);
        if (window.slot !== this.slot || window.inputHash !== this.inputHash) return;
        this.on.roundTrip?.(performance.now() - started);
        if (!this.wanted.has(key)) return;
        this.held.set(key, window);
        this.evict();
        this.on.refused(null);
        this.on.changed();
      })
      .catch((e: unknown) => {
        this.asked.delete(key);
        if (!this.attached) return;
        if (e instanceof Error && e.message === GRID_MALFORMED) {
          this.on.refused(GRID_UNREADABLE);
        } else if (refusalOf(e)?.code === 'run.unknown_input') {
          this.on.slotGone();
        } else {
          this.on.refused(refusalSentence(e));
        }
      });
  }

  /** The oldest window not in view goes first; one in view only if every one is. */
  private evict() {
    while (this.held.size > MAX_WINDOWS) {
      const keys = Array.from(this.held.keys());
      this.held.delete(keys.find((k) => !this.wanted.has(k)) ?? keys[0]);
    }
  }

  /** A cell's string, null for a missing one, or undefined while its window is
   *  not here. */
  value(row: number, column: number): string | null | undefined {
    const window = this.held.get(keyOf(blockOf(row, column)));
    if (!window) return undefined;
    return window.cells[row - window.firstRow]?.[column - window.firstColumn];
  }

  size(): number {
    return this.held.size;
  }
}
