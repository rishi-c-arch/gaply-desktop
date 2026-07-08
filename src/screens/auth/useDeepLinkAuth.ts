// Gaply — useDeepLinkAuth: completes desktop Google OAuth. The system browser
// finishes consent at https://<domain>/auth-success, which 302s to
// gaply://auth/callback?code=…  Tauri delivers that URL to the running app via
// the deep-link plugin's onOpenUrl; we pull the `code` and exchange it for a
// Supabase session.
//
// IMPORTANT: deep links only fire in a BUNDLED/INSTALLED build (the OS resolves
// the gaply:// scheme to the installed app). They do NOT work under
// `tauri dev`. This hook no-ops safely on the web build (isTauri false).
import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { isTauri } from '../../utils/isTauri';
import { useGaplySession } from '../session/SessionProvider';

/** Extract the OAuth `code` from a gaply://auth/callback?code=…&state=… URL. */
export function parseCallbackCode(url: string): string | null {
  try {
    // custom-scheme URLs parse fine with the URL constructor
    const q = new URL(url).searchParams;
    return q.get('code');
  } catch {
    // fall back to a permissive regex if the runtime's URL rejects the scheme
    const m = url.match(/[?&]code=([^&]+)/);
    return m ? decodeURIComponent(m[1]) : null;
  }
}

export function useDeepLinkAuth(onDone?: (ok: boolean) => void): void {
  const { auth } = useGaplySession();
  const navigate = useNavigate();

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      try {
        const { onOpenUrl } = await import('@tauri-apps/plugin-deep-link');
        unlisten = await onOpenUrl(async (urls: string[]) => {
          for (const url of urls) {
            const code = parseCallbackCode(url);
            if (!code || !auth.exchangeCodeForSession) continue;
            const res = await auth.exchangeCodeForSession(code);
            onDone?.(res.ok);
            if (res.ok) navigate('/app');
          }
        });
      } catch {
        /* plugin unavailable (e.g. dev) — deep link simply won't fire */
      }
    })();

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
      void cancelled;
    };
  }, [auth, navigate, onDone]);
}
