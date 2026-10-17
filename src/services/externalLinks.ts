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
// `installExternalLinkClicks` keeps the plugin's click rules exactly (primary
// button, no Meta or Alt, not already cancelled, `target="_blank"` or Ctrl or
// Shift held; http, https, mailto and tel are kept out of the webview), and
// opens only https through the command. Level 1's model-supplied sources keep
// their own stricter command, `stats_open_source_link`.

import { invoke } from '@tauri-apps/api/core';

const KEPT_OUT = ['http:', 'https:', 'mailto:', 'tel:'];

/** Open an https URL in the system browser through the checked command.
 *  Rejects, without sending, anything that is not https. */
export async function openExternal(url: string): Promise<void> {
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    throw new Error('That link could not be read, so it was not opened.');
  }
  if (parsed.protocol !== 'https:') throw new Error('Only https links are opened.');
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
    if (!a || !a.href || (a.target !== '_blank' && !e.ctrlKey && !e.shiftKey)) return;
    let url: URL;
    try {
      url = new URL(a.href);
    } catch {
      return;
    }
    if (!KEPT_OUT.includes(url.protocol)) return;
    e.preventDefault();
    if (url.protocol === 'https:') {
      // A refused or failed open changes nothing on the page.
      openExternal(url.href).catch(() => undefined);
    }
  };
  win.addEventListener('click', onClick);
  return () => win.removeEventListener('click', onClick);
}
