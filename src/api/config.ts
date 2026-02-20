// Central API configuration for backend base URL
// In production we ALWAYS use the hardcoded Railway backend; env vars only affect local dev.
// This avoids stale Vercel envs accidentally pointing to the wrong backend.
const PRODUCTION_BACKEND = 'https://gaply-backend-production.up.railway.app';
// In production only try production URLs; never localhost (avoids confusing ERR_CONNECTION_REFUSED in console).
const FALLBACK_URLS = [
  PRODUCTION_BACKEND,
  'https://gaply-backend-gaply.up.railway.app',
  'https://backend.gaply.in',
].filter(u => u !== PRODUCTION_BACKEND);
const PRODUCTION_URLS = [PRODUCTION_BACKEND, ...FALLBACK_URLS];

const isLocalhost =
  typeof window !== 'undefined' &&
  (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1');

// CRA uses process.env.REACT_APP_* (inlined at build time). We only respect this on localhost.
const envApiUrl =
  typeof process !== 'undefined' &&
  ((process as any)?.env?.REACT_APP_API_BASE_URL || (process as any)?.env?.REACT_APP_API_URL);
export const API_BASE_URL: string = isLocalhost
  ? (envApiUrl || 'http://localhost:8080')
  : PRODUCTION_BACKEND;

export function buildApiUrl(path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${API_BASE_URL}${normalizedPath}`;
}

export async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const path = input.startsWith('http') ? input : buildApiUrl(input);
  const isFormData = typeof FormData !== 'undefined' && init?.body instanceof FormData;
  const headers: HeadersInit = {
    ...(isFormData ? {} : { 'Content-Type': 'application/json' }),
    ...(init?.headers || {}),
  };

  // Try primary, then fallback (production: only production URLs; dev: localhost first)
  const urlsToTry = input.startsWith('http')
    ? [path]
    : (isLocalhost
        ? [API_BASE_URL]
        : PRODUCTION_URLS
      ).map(base => `${base}${input.startsWith('/') ? input : `/${input}`}`);
  let lastError: any;
  for (const url of urlsToTry) {
    for (let attempt = 0; attempt < 3; attempt += 1) {
      try {
        // Use the signal from init if provided (for timeout control)
        const fetchOptions: RequestInit = { 
          ...init, 
          headers, 
          mode: 'cors',
          signal: init?.signal, // Preserve abort signal for timeout
        };
        const res = await fetch(url, fetchOptions);
        if (res.ok || res.status >= 400) return res; // return even 4xx/5xx to caller
      } catch (err: any) {
        lastError = err;
        const message = String(err?.message || '');
        const transient =
          message.includes('ERR_NETWORK_IO_SUSPENDED') ||
          message.includes('ERR_NETWORK_CHANGED') ||
          message.includes('ERR_CONNECTION_RESET') ||
          message.includes('NetworkError') ||
          message.includes('Failed to fetch') ||
          message.includes('aborted');
        if (!transient || attempt === 2) {
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 800 + attempt * 800));
      }
    }
  }
  throw lastError ?? new Error('All API endpoints unreachable');
}

