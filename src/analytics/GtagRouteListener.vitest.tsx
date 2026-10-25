// Route analytics never run in the desktop app (Research OS, Phase 5B-3b
// report, decision 155). With REACT_APP_GA_MEASUREMENT_ID set at build time:
//
// * in the desktop app, no gtag script is requested, no dataLayer or gtag
//   exists, and no page_view is sent, however the route changes (a project's
//   page has its id in the route, decision 149);
// * in the browser build, the behaviour is as before: the script is
//   requested once, and each route change sends one page_view with its path
//   and the document's title.
//
// The listener reads the measurement id and the platform when its module
// loads, so each case loads it afresh with its own environment.
import React from 'react';
import { MemoryRouter, useNavigate } from 'react-router-dom';
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const ID = 'G-TEST0000';
const env = vi.hoisted(() => ({ desktop: false }));
vi.mock('../utils/isTauri', () => ({
  get isTauri() {
    return env.desktop;
  },
  get default() {
    return env.desktop;
  },
}));

type W = Window & { gtag?: (...a: unknown[]) => void; dataLayer?: unknown[] };
const w = window as W;
const scripts = () => Array.from(document.head.querySelectorAll('script')).filter((s) => s.src.includes('googletagmanager'));

let go: (to: string) => void = () => undefined;
const Driver: React.FC = () => {
  const navigate = useNavigate();
  go = (to) => navigate(to);
  return null;
};

async function mount() {
  vi.resetModules();
  const { GtagRouteListener } = await import('./GtagRouteListener');
  render(
    <MemoryRouter initialEntries={['/app']}>
      <Driver />
      <GtagRouteListener />
    </MemoryRouter>
  );
  act(() => go('/app/statistics/projects/0123456789abcdef0123456789abcdef'));
  act(() => go('/app/statistics'));
}

const before = process.env.REACT_APP_GA_MEASUREMENT_ID;
beforeEach(() => {
  process.env.REACT_APP_GA_MEASUREMENT_ID = ID;
  delete w.gtag;
  delete w.dataLayer;
  scripts().forEach((s) => s.remove());
});
afterEach(() => {
  cleanup();
  process.env.REACT_APP_GA_MEASUREMENT_ID = before;
  delete w.gtag;
  delete w.dataLayer;
  scripts().forEach((s) => s.remove());
});

describe('route analytics with REACT_APP_GA_MEASUREMENT_ID set', () => {
  it('make no call at all in the desktop app', async () => {
    env.desktop = true;
    await mount();
    expect(scripts()).toHaveLength(0);
    expect(w.gtag).toBeUndefined();
    expect(w.dataLayer).toBeUndefined();
  });

  it('in the browser build, request the script once and send one page_view per route', async () => {
    env.desktop = false;
    await mount();
    expect(scripts()).toHaveLength(1);
    expect(scripts()[0].src).toContain(encodeURIComponent(ID));
    const views = (w.dataLayer ?? []).map((a) => Array.from(a as ArrayLike<unknown>)).filter((a) => a[0] === 'event' && a[1] === 'page_view');
    expect(views.map((a) => (a[2] as { page_path: string }).page_path)).toEqual([
      '/app',
      '/app/statistics/projects/0123456789abcdef0123456789abcdef',
      '/app/statistics',
    ]);
    expect(views.every((a) => (a[2] as { page_title: string }).page_title === document.title)).toBe(true);
    const config = (w.dataLayer ?? []).map((a) => Array.from(a as ArrayLike<unknown>)).filter((a) => a[0] === 'config');
    expect(config).toEqual([['config', ID, { send_page_view: false }]]);
  });
});
