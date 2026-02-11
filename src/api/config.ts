// Central API configuration for backend base URL
// Priority: env var → public env → hardcoded domain
const FALLBACK_URLS = [
  'http://localhost:8080',
  'https://gaply-backend-gaply.up.railway.app',
  'https://gaply-backend-production.up.railway.app',
  'https://backend.gaply.in',
];

const isLocalhost =
  typeof window !== 'undefined' &&
  (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1');

export const API_BASE_URL: string =
  (import.meta as any)?.env?.VITE_API_BASE_URL ||
  (typeof process !== 'undefined' && (process as any)?.env?.REACT_APP_API_BASE_URL) ||
  (isLocalhost ? 'http://localhost:8080' : FALLBACK_URLS[1]);

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

  // Try primary, then fallback
  const urlsToTry = input.startsWith('http')
    ? [path]
    : (isLocalhost
        ? [API_BASE_URL]
        : [API_BASE_URL, ...FALLBACK_URLS.filter(u => u !== API_BASE_URL)]
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

