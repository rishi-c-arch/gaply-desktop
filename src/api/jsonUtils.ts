/**
 * Parse JSON from a Response body without throwing; useful after apiFetch when
 * the server may return HTML or plain text on errors.
 */
export async function readJsonSafe<T = unknown>(
  res: Response,
): Promise<{ ok: true; data: T } | { ok: false; error: string }> {
  const text = await res.text();
  const trimmed = text.trim();
  if (!trimmed) {
    return { ok: true, data: null as T };
  }
  try {
    return { ok: true, data: JSON.parse(trimmed) as T };
  } catch {
    return { ok: false, error: 'Invalid JSON response' };
  }
}
