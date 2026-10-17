// External links in the Note Creator — do they actually reach the opener?
//
// HOW THESE LINKS WORK, because it is not obvious and the audit got it wrong.
// They are plain `<a target="_blank">`. Tauri's NATIVE new-window path is not
// what carries them: `new_window_handler` starts as None, its only setter is
// `WebviewBuilder::on_new_window`, and the app never calls it — so wry's
// WKWebView UI delegate falls through to `else { None }` and a native
// new-window request would be dropped. They work anyway because a listener on
// WINDOW catches a left-click whose composedPath contains an `<a>` with
// target="_blank" and an http/https/mailto/tel href, calls preventDefault(),
// and opens it. Until the Stats Verifier's part C follow-up that listener was
// tauri-plugin-opener's injected `init-iife.js`, which opened any such URL
// with no check in Rust; now it is the app's own
// (`src/services/externalLinks.ts`), with the same rules, and it opens https
// links through the checked command `open_external_link`. The click never
// reaches the native path at all.
//
// THE CONSEQUENCE THIS FILE PINS. The shim is on `window`, which is ABOVE
// React's root container. React dispatches synthetic events from that
// container, and SyntheticEvent.stopPropagation() also stops the NATIVE event —
// so any handler calling it anywhere in an external link's ancestry silently
// kills the link. No error, no navigation, nothing. ScaffoldPicker had exactly
// that on its "Verify current requirements" anchor, guarding a card click
// handler that does not exist, and it cost all eleven scaffolds their link.
//
// Measured in the running app on 12 Sep 2026: a plain anchor opened in the
// default browser; the same anchor under an ancestor that stops propagation
// produced no request at all.
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'fs';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn(async (_cmd: string, _args?: unknown) => undefined) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
import { installExternalLinkClicks } from '../../services/externalLinks';

import ScaffoldPicker from './ScaffoldPicker';
import RecommendedToolsPanel from './RecommendedToolsPanel';
import { Scaffold } from './manuscriptModel';

const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };

/** The app's real window-level listener, with only the Rust command mocked:
 *  `opened` is every URL it sent to `open_external_link`. If a click reaches
 *  it, the app opens the URL; if it doesn't, the link does nothing. */
const installOpenerShim = () => {
  invoke.mockClear();
  const remove = installExternalLinkClicks(window);
  return {
    get opened(): string[] {
      return invoke.mock.calls.filter((c) => c[0] === 'open_external_link').map((c) => (c[1] as { url: string }).url);
    },
    remove,
  };
};

let shim: ReturnType<typeof installOpenerShim>;
beforeEach(() => { shim = installOpenerShim(); });
afterEach(() => { shim.remove(); cleanup(); });

describe('scaffold picker — “Verify current requirements”', () => {
  const renderPicker = () =>
    render(<ScaffoldPicker scaffolds={CAT.scaffolds} notice="n" onPick={vi.fn()} />);

  it('a click REACHES the window-level opener, for every scaffold', () => {
    renderPicker();
    for (const s of CAT.scaffolds) {
      fireEvent.click(screen.getByTestId(`sp-verify-${s.id}`));
    }
    // the regression: this was 0 of 11 while a stopPropagation sat on the anchor
    expect(shim.opened).toHaveLength(CAT.scaffolds.length);
    expect(shim.opened[0]).toBe(CAT.scaffolds[0].publisherAuthorUrl);
  });

  it('no ancestor of a verify link stops propagation', () => {
    renderPicker();
    const link = screen.getByTestId('sp-verify-ieee');
    fireEvent.click(link);
    expect(shim.opened).toContain(CAT.scaffolds.find((s) => s.id === 'ieee')!.publisherAuthorUrl);
  });

  it('the pick button still works — removing the guard broke nothing', () => {
    const onPick = vi.fn();
    render(<ScaffoldPicker scaffolds={CAT.scaffolds} notice="n" onPick={onPick} />);
    fireEvent.click(screen.getByTestId('sp-pick-ieee'));
    expect(onPick).toHaveBeenCalledTimes(1);
    expect(onPick.mock.calls[0][0].id).toBe('ieee');
    expect(shim.opened).toHaveLength(0); // and it is not mistaken for a link
  });
});

describe('recommended tools panel', () => {
  it('both third-party links reach the opener', () => {
    render(<RecommendedToolsPanel />);
    fireEvent.click(screen.getByTestId('tool-notebooklm'));
    fireEvent.click(screen.getByTestId('tool-gpai'));
    expect(shim.opened).toEqual(['https://notebooklm.google/', 'https://gpai.app/']);
  });
});

describe('the shim’s own limits, recorded so they are not rediscovered', () => {
  it('a stopPropagation anywhere in the ancestry silently kills the link', () => {
    // This is the exact shape that was measured dead in the running app.
    render(
      // eslint-disable-next-line jsx-a11y/no-static-element-interactions, jsx-a11y/click-events-have-key-events
      <div onClick={(e) => e.stopPropagation()}>
        <a href="https://example.org/x" target="_blank" rel="noreferrer noopener" data-testid="trapped">x</a>
      </div>
    );
    fireEvent.click(screen.getByTestId('trapped'));
    expect(shim.opened).toHaveLength(0);
  });

  it('⌘-click is NOT carried by the shim (it bails on metaKey)', () => {
    render(<ScaffoldPicker scaffolds={CAT.scaffolds} notice="n" onPick={vi.fn()} />);
    fireEvent.click(screen.getByTestId('sp-verify-ieee'), { metaKey: true });
    // Documented, not a defect to fix here: the native new-window path the
    // modifier would use is unhandled, so ⌘-click does nothing in the desktop app.
    expect(shim.opened).toHaveLength(0);
  });
});
