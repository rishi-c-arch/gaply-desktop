// Central API configuration for backend base URL
// Priority: env var → public env → hardcoded domain
const FALLBACK_URLS = [
  'https://backend.gaply.in',
  'https://srv-d3cl1tmmcj7s73dmq9eg.onrender.com',
];

export const API_BASE_URL: string =
  (import.meta as any)?.env?.VITE_API_BASE_URL ||
  (typeof process !== 'undefined' && (process as any)?.env?.REACT_APP_API_BASE_URL) ||
  FALLBACK_URLS[0];

export function buildApiUrl(path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${API_BASE_URL}${normalizedPath}`;
}

export async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const path = input.startsWith('http') ? input : buildApiUrl(input);
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...(init?.headers || {}),
  };

  // Try primary, then fallback
  const urlsToTry = input.startsWith('http') ? [path] : [API_BASE_URL, ...FALLBACK_URLS.filter(u => u !== API_BASE_URL)].map(base => `${base}${input.startsWith('/') ? input : `/${input}`}`);
  let lastError: any;
  for (const url of urlsToTry) {
    try {
      const res = await fetch(url, { ...init, headers, mode: 'cors' });
      if (res.ok || res.status >= 400) return res; // return even 4xx/5xx to caller
    } catch (err) {
      lastError = err;
    }
  }
  throw lastError ?? new Error('All API endpoints unreachable');
}

