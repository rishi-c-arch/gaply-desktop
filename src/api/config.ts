// Central API configuration for backend base URL
// Priority: env var → public env → hardcoded domain
export const API_BASE_URL: string =
  (import.meta as any)?.env?.VITE_API_BASE_URL ||
  (typeof process !== 'undefined' && (process as any)?.env?.REACT_APP_API_BASE_URL) ||
  'https://backend.gaply.in';

export function buildApiUrl(path: string): string {
  const normalizedPath = path.startsWith('/') ? path : `/${path}`;
  return `${API_BASE_URL}${normalizedPath}`;
}

export async function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  const url = input.startsWith('http') ? input : buildApiUrl(input);
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...(init?.headers || {}),
  };
  return fetch(url, { ...init, headers, mode: 'cors' });
}

