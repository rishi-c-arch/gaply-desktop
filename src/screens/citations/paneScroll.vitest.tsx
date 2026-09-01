// Gaply — the detail pane must be able to scroll its own content.
//
// The bug this pins: the Citation Manager's right-hand pane is
// `sticky top-0 h-screen`, which pins it to the viewport — correct — and on its
// own also CLIPS, because a box fixed at one screen tall with no overflow rule
// has nowhere to put anything past the fold. With a citation selected, the pane
// ran preview → Edit details → metadata → Document card and then simply stopped
// at the window edge: the Document card's buttons were the last thing reachable
// and the entire AI assistance panel below it could not be scrolled to. The
// mouse wheel did nothing, because there was no scroll container under it.
//
// jsdom does not do layout, so these assert the RULE rather than a rendered
// scrollbar: the pane carries an overflow class, and nothing inside it re-pins
// the height. That is the thing that was actually missing, and it is the thing
// a future edit could silently remove.
import React from 'react';
import fs from 'node:fs';
import path from 'node:path';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import CitationManagerPage from './CitationManagerPage';
import { makeMockLocalLibrary } from './localLibrary';
import { Citation } from './citationTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const USER = { user: { id: 'u1', email: 'a@b.c' } };
function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: USER, offline: false }),
    onAuthStateChange: (cb: any) => { cb(USER); return () => {}; },
  } as any;
}

const NAIDU: Citation = {
  id: 'cite-naidu',
  csl: {
    id: 'cite-naidu',
    type: 'article-journal',
    title: 'Incidence of needlestick injury among healthcare workers in western India',
    author: [{ family: 'Naidu', given: 'Raji T' }],
    issued: { year: 2023 },
    DOI: '10.4103/ijmr.ijmr_892_23',
  } as any,
  doi: '10.4103/ijmr.ijmr_892_23',
  retracted: false,
  source: 'manual',
} as Citation;

async function renderCM() {
  const local = makeMockLocalLibrary();
  await local.upsert(NAIDU, []);
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>
        <CitationManagerPage
          citationService={{ add: vi.fn(), list: vi.fn(), remove: vi.fn() } as any}
          localLibrary={local}
        />
      </GaplySessionProvider>
    </MemoryRouter>,
  );
  return local;
}

describe('Citation Manager detail pane', () => {
  it('is a scroll container, so content below the fold is reachable', async () => {
    await renderCM();
    const pane = await screen.findByTestId('detail-pane');

    // The pane owns its scrolling, independently of the list — the list scrolls
    // in <main>, and the two must not be the same scrollbar or selecting a
    // citation would move the list under the user.
    expect(pane.className).toMatch(/\boverflow-y-auto\b/);
    // A flex child will not shrink below its content without this, which is how
    // an overflow rule ends up on a box that never actually overflows.
    expect(pane.className).toMatch(/\bmin-h-0\b/);
    // The pin is deliberate and stays.
    expect(pane.className).toMatch(/\bsticky\b/);
    expect(pane.className).toMatch(/\bh-screen\b/);
  });

  it('does not re-pin the height one level down, which would clip it again', async () => {
    // `h-full` on the inner wrapper is `height: 100%` of a box that is already
    // exactly one screen tall — the same clip, moved inside the scroll
    // container, where it is harder to see.
    await renderCM();
    await waitFor(() => expect(screen.getByText(/needlestick/i)).toBeTruthy());
    screen.getByText(/needlestick/i).click();

    const detail = await screen.findByTestId('detail');
    expect(detail.className).not.toMatch(/\bh-full\b/);
    expect(detail.closest('[data-testid="detail-pane"]')).not.toBeNull();
  });
});

describe('Settings page', () => {
  // Settings does NOT have this bug, and the reason is worth pinning: it uses
  // the design system's panel, whose body carries `overflow: auto` in CSS
  // rather than a utility class on the element. That is why the same mistake
  // did not reach it — and why a check that only looked for a Tailwind class
  // would wrongly report it as broken.
  const css = fs.readFileSync(
    path.resolve(__dirname, '../../design-system/primitives.css'),
    'utf8',
  );

  it('scrolls through the design-system panel body, which declares overflow', () => {
    const rule = css.match(/\.gds-root \.gds-panel__body \{[^}]*\}/)?.[0] ?? '';
    expect(rule, 'the .gds-panel__body rule is missing').not.toBe('');
    expect(rule).toMatch(/overflow:\s*auto/);
  });

  it('the panel can actually shrink, or the overflow rule would never engage', () => {
    // `overflow: auto` on a box that grows to fit its content never scrolls.
    // `min-height: 0` on the panel is what lets it be shorter than its content.
    const panel = css.match(/\.gds-root \.gds-panel \{[^}]*\}/)?.[0] ?? '';
    expect(panel, 'the .gds-panel rule is missing').not.toBe('');
    expect(panel).toMatch(/min-height:\s*0/);
    expect(panel).toMatch(/flex-direction:\s*column/);
  });
});
