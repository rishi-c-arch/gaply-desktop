// The timing counter (Research OS, Phase 8Q, decision 462): off by default, shown
// only under statsLayout, in memory only, 300 frames at most.
import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { FEATURE_FLAGS } from '../../../config/featureFlags';
import { GridTiming, percentile95, timingShown } from './gridTiming';
import { TimingLine } from './TimingLine';

const flags = vi.hoisted(() => ({ on: new Set<string>() }));
vi.mock('../../../config/featureFlags', async (original) => ({
  ...(await original<typeof import('../../../config/featureFlags')>()),
  isFeatureEnabled: (name: string) => flags.on.has(name),
}));

afterEach(() => {
  cleanup();
  flags.on.clear();
});

describe('the statsTiming flag', () => {
  it('is off by default', () => {
    expect(FEATURE_FLAGS.statsTiming).toBe(false);
  });

  it('shows only with statsLayout on too', () => {
    expect(timingShown()).toBe(false);
    flags.on.add('statsTiming');
    expect(timingShown()).toBe(false);
    flags.on.clear();
    flags.on.add('statsLayout');
    expect(timingShown()).toBe(false);
    flags.on.add('statsTiming');
    expect(timingShown()).toBe(true);
  });
});

describe('the counter', () => {
  it('the 95th percentile of what it holds', () => {
    expect(percentile95([])).toBeNull();
    expect(percentile95([5])).toBe(5);
    expect(percentile95(Array.from({ length: 100 }, (_, i) => i + 1))).toBe(95);
  });

  it('holds at most the last 300 frames', () => {
    const t = new GridTiming();
    for (let i = 0; i < 1000; i += 1) t.noteFrame(i);
    expect(t.frameCount()).toBe(300);
    expect(t.read().frameP95).toBe(700 + Math.ceil(0.95 * 300) - 1);
  });

  it('keeps the last window\'s round trip, and one first paint per table', () => {
    const t = new GridTiming();
    expect(t.read()).toEqual({ windowMs: null, paintMs: null, frameP95: null });
    t.noteWindow(12);
    t.noteWindow(8);
    t.notePaint('a', 40);
    t.notePaint('a', 90);
    expect(t.read()).toEqual({ windowMs: 8, paintMs: 40, frameP95: null });
    t.notePaint('b', 70);
    expect(t.read().paintMs).toBe(70);
  });
});

describe('the line', () => {
  it('gives the three figures in ms, and says when one is not read yet', () => {
    const t = new GridTiming();
    vi.useFakeTimers();
    try {
      render(<TimingLine timing={t} />);
      expect(screen.getByTestId('sw-timing').textContent).toBe('Window: not read. First paint: not read. Frames, 95th percentile: not read.');
      t.noteWindow(3.26);
      t.notePaint('a', 41.5);
      t.noteFrame(16.7);
      act(() => {
        vi.advanceTimersByTime(600);
      });
      expect(screen.getByTestId('sw-timing').textContent).toBe('Window: 3.3 ms. First paint: 41.5 ms. Frames, 95th percentile: 16.7 ms.');
    } finally {
      vi.useRealTimers();
    }
  });
});
