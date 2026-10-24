// GaplyGlobe keeps a failed globe to itself (Research OS, Phase 5B-4a; the
// hand test's finding h). drei's useTexture rethrows a failed image load
// through R3F's Canvas into the React tree; with only a Suspense around it,
// the first boundary above was the app's root one, whose fallback replaces
// the whole screen with "Something went wrong". The brand mark is in the rail
// of every AppShell screen, so one failed texture took any of them down.
//
// ThreeJSGlobe is mocked to throw (the pattern in src/screens/screens.vitest.tsx);
// the test waits for the lazy import to resolve and the mock to render.
import React from 'react';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AppErrorBoundary } from '../components/AppErrorBoundary';
import { GaplyGlobe } from './GaplyGlobe';

const globe = vi.hoisted(() => ({ attempts: 0 }));

vi.mock('../components/ThreeJSGlobe', () => ({
  default: () => {
    globe.attempts += 1;
    throw new Error('GLOBE_PROBE: Could not load the texture');
  },
}));

let quiet: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  globe.attempts = 0;
  // React reports a caught render error on the console; keep the run's output readable.
  quiet = vi.spyOn(console, 'error').mockImplementation(() => {});
});
afterEach(() => {
  quiet.mockRestore();
  cleanup();
});

const Screen: React.FC = () => (
  <AppErrorBoundary>
    <nav>
      <GaplyGlobe scale="mark" />
    </nav>
    <main data-testid="rest-of-screen">HTSCREEN_body</main>
  </AppErrorBoundary>
);

describe('GaplyGlobe', () => {
  it('a globe that throws leaves the rest of the screen in place, with nothing in its own box', async () => {
    render(<Screen />);
    await waitFor(() => expect(globe.attempts).toBeGreaterThan(0));
    // React 19 holds the commit after a Suspense retry for up to
    // FALLBACK_THROTTLE_MS (300 in react-dom 19.2.0), so the DOM read too early
    // is the one from before the throw. Measured: with a 50 ms wait this test
    // passed at HEAD, where the root fallback had not yet been committed; with
    // 1000 ms it failed there, as it must.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 1000));
    });

    expect(screen.queryByText('Something went wrong')).toBeNull();
    expect(screen.getByTestId('rest-of-screen').textContent).toBe('HTSCREEN_body');
    const box = screen.getByTestId('gds-globe-mark');
    expect(box.childElementCount).toBe(0);
    expect(box.textContent).toBe('');
  });
});
