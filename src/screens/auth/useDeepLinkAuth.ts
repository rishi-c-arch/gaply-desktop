// Gaply — useDeepLinkAuth: completes desktop Google OAuth. The system browser
// finishes consent at https://<domain>/auth-success, which 302s to
// gaply://auth/callback?code=…  (or ?error=…&error_description=… on failure).
// Tauri delivers that URL to the running app via the deep-link plugin's
// onOpenUrl (WARM start: app already running); we ALSO check getCurrent() for
// the COLD start case (the app was launched *by* the deep link, so the URL
// arrived before any listener was bound). We pull the `code` and exchange it
// for a Supabase session, or surface an honest error via `onResult`.
//
// IMPORTANT: deep links only fire in a BUNDLED/INSTALLED build (the OS resolves
// the gaply:// scheme to the installed app). They do NOT work under
// `tauri dev`. This hook no-ops safely on the web build (isTauri false).
import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { isTauri } from '../../utils/isTauri';
import { useGaplySession } from '../session/SessionProvider';

export interface DeepLinkAuthResult {
  ok: boolean;
  /** Present on failure — the provider's error_description (preferred) or code. */
  error?: string;
}

/** Parse a gaply://auth/callback URL. Returns the OAuth `code` on success, or an
 *  `error` (+ human `errorDescription`) when the provider handed back a failure
 *  (e.g. ?error=server_error&error_description=Unable+to+exchange+external+code). */
export function parseCallback(url: string): { code?: string; error?: string; errorDescription?: string } {
  try {
    // custom-scheme URLs parse fine with the URL constructor
    const q = new URL(url).searchParams;
    return {
      code: q.get('code') ?? undefined,
      error: q.get('error') ?? undefined,
      errorDescription: q.get('error_description') ?? undefined,
    };
  } catch {
    // Permissive fallback if the runtime's URL rejects the custom scheme.
    const pick = (k: string): string | undefined => {
      const m = url.match(new RegExp(`[?&]${k}=([^&]+)`));
      return m ? decodeURIComponent(m[1].replace(/\+/g, ' ')) : undefined;
    };
    return { code: pick('code'), error: pick('error'), errorDescription: pick('error_description') };
  }
}

/** Redacted STRUCTURAL summary of a callback URL, for diagnostics only. Reports
 *  the scheme/host/path and the presence + LOCATION (query vs fragment) of the
 *  auth params — never their secret values. Manual splitting (not `new URL`) so
 *  it's robust to custom schemes the runtime's URL parser may reject. */
export function summarizeCallback(url: string): {
  scheme: string;
  host: string;
  path: string;
  hasCode: boolean;
  hasError: boolean;
  hasAccessToken: boolean;
  hasRefreshToken: boolean;
  paramLocation: 'query' | 'fragment' | 'both' | 'none';
} {
  const hashIdx = url.indexOf('#');
  const fragment = hashIdx >= 0 ? url.slice(hashIdx + 1) : '';
  const beforeHash = hashIdx >= 0 ? url.slice(0, hashIdx) : url;
  const qIdx = beforeHash.indexOf('?');
  const query = qIdx >= 0 ? beforeHash.slice(qIdx + 1) : '';
  const base = qIdx >= 0 ? beforeHash.slice(0, qIdx) : beforeHash;
  let scheme = '';
  let host = '';
  let path = '';
  const m = base.match(/^([^:]+):\/\/([^/]*)(\/.*)?$/);
  if (m) { scheme = m[1]; host = m[2]; path = m[3] ?? ''; } else { path = base; }
  const qp = new URLSearchParams(query);
  const fp = new URLSearchParams(fragment);
  const has = (k: string): boolean => qp.has(k) || fp.has(k);
  const anyQuery = Array.from(qp.keys()).length > 0;
  const anyFragment = Array.from(fp.keys()).length > 0;
  const paramLocation = anyQuery && anyFragment ? 'both' : anyQuery ? 'query' : anyFragment ? 'fragment' : 'none';
  return {
    scheme,
    host,
    path,
    hasCode: has('code'),
    hasError: has('error'),
    hasAccessToken: has('access_token'),
    hasRefreshToken: has('refresh_token'),
    paramLocation,
  };
}

export function useDeepLinkAuth(onResult?: (r: DeepLinkAuthResult) => void): void {
  const { auth } = useGaplySession();
  const navigate = useNavigate();
  // Keep the latest callback in a ref so an inline handler from the caller does
  // not re-run this effect (which would re-register the listener every render).
  const onResultRef = useRef(onResult);
  useEffect(() => { onResultRef.current = onResult; }, [onResult]);

  useEffect(() => {
    if (!isTauri) return;
    const unlisteners: Array<() => void> = [];
    let done = false; // process exactly one callback (cold OR warm, not both)

    // Handle one callback URL. Returns true if the URL WAS an auth callback
    // (code or error), so the caller can stop processing further URLs.
    const handle = async (url: string): Promise<boolean> => {
      // Redacted diagnostic log (structure only — never the code/token VALUES),
      // so a "silently stuck" callback is visible in terminal/Console. Wrapped so
      // logging can never break the auth flow.
      const shape = summarizeCallback(url);
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('log_auth_callback', {
          scheme: shape.scheme,
          host: shape.host,
          path: shape.path,
          hasCode: shape.hasCode,
          hasError: shape.hasError,
          hasAccessToken: shape.hasAccessToken,
          hasRefreshToken: shape.hasRefreshToken,
          paramLocation: shape.paramLocation,
        });
      } catch {
        /* logging is best-effort */
      }

      const { code, error, errorDescription } = parseCallback(url);
      if (error) {
        onResultRef.current?.({ ok: false, error: errorDescription || error });
        return true;
      }
      if (code && auth.exchangeCodeForSession) {
        const res = await auth.exchangeCodeForSession(code);
        onResultRef.current?.({ ok: res.ok, error: res.error });
        if (res.ok) navigate('/app');
        return true;
      }
      // It IS our OAuth callback but carried no usable credentials — surface it
      // honestly instead of failing silently (item 2). Non-auth deep links fall
      // through to `return false` so they pass by untouched.
      const isAuthCallback = shape.host === 'auth' || /auth\/callback/.test(url);
      if (isAuthCallback) {
        onResultRef.current?.({ ok: false, error: 'no credentials were returned' });
        return true;
      }
      return false;
    };

    const runAll = async (urls: string[]): Promise<void> => {
      for (const url of urls) {
        if (done) break;
        if (await handle(url)) { done = true; break; }
      }
    };

    (async () => {
      try {
        const deepLink = await import('@tauri-apps/plugin-deep-link');
        // COLD start: the URL that launched the app. getCurrent() returns it even
        // though it arrived before onOpenUrl was bound. No-op if unsupported.
        try {
          const initial = await deepLink.getCurrent();
          if (initial && initial.length) await runAll(initial);
        } catch {
          /* getCurrent unavailable on this platform/version — ignore */
        }
        // WARM start (macOS + any platform where the plugin emits natively).
        unlisteners.push(await deepLink.onOpenUrl((urls: string[]) => { void runAll(urls); }));
      } catch {
        /* plugin unavailable (e.g. dev) — deep link simply won't fire */
      }

      // WARM start on Windows/Linux: the Rust single-instance handler forwards
      // the argv URL as a 'deep-link-url' event (see lib.rs). Belt-and-suspenders
      // with onOpenUrl; the `done` guard keeps it to exactly one exchange.
      try {
        const { listen } = await import('@tauri-apps/api/event');
        unlisteners.push(await listen<string>('deep-link-url', (e) => { void runAll([e.payload]); }));
      } catch {
        /* event API unavailable — ignore */
      }
    })();

    return () => { for (const u of unlisteners) u(); };
  }, [auth, navigate]);
}
