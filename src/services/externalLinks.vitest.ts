// Part C follow-up, item 7: every external link the webview opens goes
// through the app's own Rust command, `open_external_link`, which rechecks
// it. The opener plugin's own click handler and its webview permission to
// open URLs are switched off, so a page cannot open a link around the check.
import { afterEach, describe, expect, it, vi } from 'vitest';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn(async (_cmd: string, _args?: unknown) => undefined) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

import { installExternalLinkClicks, openExternal } from './externalLinks';

function anchor(href: string, target?: string): HTMLAnchorElement {
  const a = document.createElement('a');
  a.href = href;
  if (target) a.target = target;
  a.textContent = 'link';
  document.body.appendChild(a);
  return a;
}

/** Long enough for any open that was started to have reached `invoke`. */
const settle = () => new Promise((r) => setTimeout(r, 20));

function click(el: Element, init: MouseEventInit = {}): MouseEvent {
  const e = new MouseEvent('click', { bubbles: true, cancelable: true, button: 0, ...init });
  el.dispatchEvent(e);
  return e;
}

describe('external links', () => {
  let uninstall: () => void = () => {};
  afterEach(() => {
    uninstall();
    document.body.innerHTML = '';
    invoke.mockClear();
  });

  it('opens an https link with target=_blank through the Rust command, and nothing else', async () => {
    uninstall = installExternalLinkClicks(window);
    const e = click(anchor('https://example.org/guide', '_blank'));
    expect(e.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
    expect(invoke).toHaveBeenCalledWith('open_external_link', { url: 'https://example.org/guide' });
    expect(invoke.mock.calls.some((c) => String(c[0]).startsWith('plugin:opener'))).toBe(false);
  });

  it('opens a ctrl- or shift-clicked https link, as the plugin did', async () => {
    uninstall = installExternalLinkClicks(window);
    click(anchor('https://example.org/a'), { ctrlKey: true });
    click(anchor('https://example.org/b'), { shiftKey: true });
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    expect(invoke.mock.calls.map((c) => (c[1] as { url: string }).url)).toEqual(['https://example.org/a', 'https://example.org/b']);
  });

  it('stops http and tel links leaving the webview, and opens neither (part C follow-up 2)', async () => {
    uninstall = installExternalLinkClicks(window);
    for (const [href, target] of [['http://example.org/', '_blank'], ['tel:+100', '_blank'], ['http://example.org/', undefined], ['tel:+100', undefined]] as const) {
      const e = click(anchor(href, target));
      expect(e.defaultPrevented, href).toBe(true);
    }
    await settle();
    expect(invoke).not.toHaveBeenCalled();
  });

  it('sends a mailto link to the checked command whatever its target, and Rust decides', async () => {
    uninstall = installExternalLinkClicks(window);
    const plain = click(anchor('mailto:helloresearcher@gaply.in'));
    const self = click(anchor('mailto:helloresearcher@gaply.in?subject=Hi', '_self'));
    expect(plain.defaultPrevented).toBe(true);
    expect(self.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
    expect(invoke.mock.calls.map((c) => c[0])).toEqual(['open_external_link', 'open_external_link']);
    expect(invoke.mock.calls.map((c) => (c[1] as { url: string }).url)).toEqual(['mailto:helloresearcher@gaply.in', 'mailto:helloresearcher@gaply.in?subject=Hi']);
  });

  it('opens an https link with no target through the command instead of navigating the app window', async () => {
    uninstall = installExternalLinkClicks(window);
    const e = click(anchor('https://example.org/plain'));
    expect(e.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('open_external_link', { url: 'https://example.org/plain' }));
  });

  it('leaves alone an ordinary in-app link, a cancelled click and a non-primary button', async () => {
    uninstall = installExternalLinkClicks(window);
    const inApp = click(anchor('#/stats-verifier'));
    expect(inApp.defaultPrevented).toBe(false);
    // The app's own origin, even over http (dev, and Windows' tauri.localhost).
    const sameOrigin = click(anchor(`${window.location.origin}/#/home`, '_blank'));
    expect(sameOrigin.defaultPrevented).toBe(false);
    const a = anchor('https://example.org/', '_blank');
    a.addEventListener('click', (e) => e.preventDefault());
    click(a);
    click(anchor('https://example.org/', '_blank'), { button: 1 });
    click(anchor('https://example.org/', '_blank'), { metaKey: true });
    await settle();
    expect(invoke).not.toHaveBeenCalled();
  });

  it('openExternal sends only an https or mailto URL, and reports a refusal', async () => {
    await expect(openExternal('https://example.supabase.co/auth/v1/authorize?x=1')).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('open_external_link', { url: 'https://example.supabase.co/auth/v1/authorize?x=1' });
    invoke.mockClear();
    await expect(openExternal('javascript:alert(1)')).rejects.toThrow();
    await expect(openExternal('tel:+100')).rejects.toThrow();
    await expect(openExternal('http://gaply.in/')).rejects.toThrow();
    expect(invoke).not.toHaveBeenCalled();
    await expect(openExternal('mailto:helloresearcher@gaply.in')).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('open_external_link', { url: 'mailto:helloresearcher@gaply.in' });
  });
});
