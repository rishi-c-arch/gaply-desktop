// Gaply — the Data View's timing counter (Research OS, Phase 8Q, decision 462),
// read by Rishi in the real-app check against ADR-012's stops. Behind statsTiming,
// shown only under statsLayout. Times in ms, in memory only: it holds no cell, and
// it reads only performance.now() and requestAnimationFrame.
import { isFeatureEnabled } from '../../../config/featureFlags';

const FRAMES_KEPT = 300;
/** How long after a scroll event its frames still count as scrolling. */
const SCROLLING_MS = 200;

export const timingShown = (): boolean => isFeatureEnabled('statsTiming') && isFeatureEnabled('statsLayout');

export function percentile95(values: number[]): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.ceil(0.95 * sorted.length) - 1];
}

export interface TimingFigures {
  windowMs: number | null;
  paintMs: number | null;
  frameP95: number | null;
}

export class GridTiming {
  private windowMs: number | null = null;
  private paintMs: number | null = null;
  private paintedFor: string | null = null;
  private frames: number[] = [];
  private lastScroll = 0;
  private lastFrame: number | null = null;
  private loop: number | null = null;

  noteWindow(ms: number) {
    this.windowMs = ms;
  }

  /** The first paint of a table: a later paint of the same one is a return to
   *  the tab, not an open. */
  notePaint(table: string, ms: number) {
    if (this.paintedFor === table) return;
    this.paintedFor = table;
    this.paintMs = ms;
  }

  noteFrame(ms: number) {
    this.frames.push(ms);
    if (this.frames.length > FRAMES_KEPT) this.frames.shift();
  }

  frameCount(): number {
    return this.frames.length;
  }

  /** A scroll happened: frames count until it has been quiet for a moment. */
  scrolled() {
    this.lastScroll = performance.now();
    if (this.loop === null) {
      this.lastFrame = null;
      this.loop = requestAnimationFrame(this.frame);
    }
  }

  private frame = (now: number) => {
    if (this.lastFrame !== null) this.noteFrame(now - this.lastFrame);
    this.lastFrame = now;
    this.loop = performance.now() - this.lastScroll > SCROLLING_MS ? null : requestAnimationFrame(this.frame);
  };

  stop() {
    if (this.loop !== null) cancelAnimationFrame(this.loop);
    this.loop = null;
  }

  read(): TimingFigures {
    return { windowMs: this.windowMs, paintMs: this.paintMs, frameP95: percentile95(this.frames) };
  }
}
