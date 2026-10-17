// Gaply — external links in the desktop app (Phase 2 evaluation fixes, part
// C follow-up, item 7).
//
// The opener plugin used to open links by itself: its injected click handler
// sent any <a target="_blank"> to `plugin:opener|open_url`, and the window's
// permission allowed every http(s), mailto and tel URL, with no check in
// Rust. Both are off now (`src-tauri/src/lib.rs`, `capabilities/default.json`).
// Every external link goes through `open_external_link`, which rechecks it
// (https only, no credentials, bounded; `src-tauri/src/external_links.rs`).
//
// `installExternalLinkClicks` keeps the plugin's rules for which clicks it
// takes (primary button, no Meta or Alt, not already cancelled), and since
// part C's follow-up 2 takes every http, https, mailto and tel anchor whatever
// its target that is not on the app's own origin, so no external link
// navigates the app window itself. It opens
// https and mailto through the command (Rust allows only listed mailto
// addresses, with at most a subject) and opens no http or tel link. Level 1's
// model-supplied sources keep their own stricter command,
// `stats_open_source_link`.

import { invoke } from '@tauri-apps/api/core';

const KEPT_OUT = ['http:', 'https:', 'mailto:', 'tel:'];
const OPENED = ['https:', 'mailto:'];

/** Open an https or mailto URL through the checked command. Rejects,
 *  without sending, any other scheme. */
export async function openExternal(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error('That link could not be read, so it was not opened.');
  }
  if (!OPENED.includes(parsed.protocol)) throw new Error('Only https and mailto links are opened.');
  await invoke('open_external_link', { url });
}

/** Handle clicks on external links for the whole window. Returns the
 *  function that removes the handler. */
export function installExternalLinkClicks(win: Window): () => void {
  const onClick = (e: MouseEvent) => {
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.altKey) return;
    const a = e.composedPath().find((n) => n instanceof Node && n.nodeName.toUpperCase() === 'A') as
      | HTMLAnchorElement
      | undefined;
    if (!a || !a.href) return;
    let url: URL;
    try {
      url = new URL(a.href);
    } catch {
      return;
    }
    if (!KEPT_OUT.includes(url.protocol)) return;
    // The app's own pages are left to the router: in development and on
    // Windows the app itself is served over http (localhost:3000,
    // tauri.localhost).
    if (url.origin === win.location.origin) return;
    e.preventDefault();
    if (OPENED.includes(url.protocol)) {
      // A refused or failed open changes nothing on the page.
      openExternal(url.href).catch(() => undefined);
    }
  };
  win.addEventListener('click', onClick);
  return () => win.removeEventListener('click', onClick);
}
