import { readJsonSafe } from './jsonUtils';

function mockResponse(body: string, contentType = 'application/json'): Response {
  return new Response(body, {
    status: 200,
    headers: { 'Content-Type': contentType },
  });
}

describe('readJsonSafe', () => {
  it('returns data for valid JSON object', async () => {
    const res = mockResponse('{"a":1}');
    const out = await readJsonSafe<{ a: number }>(res);
    expect(out.ok).toBe(true);
    if (out.ok) expect(out.data).toEqual({ a: 1 });
  });

  it('returns null data for empty body', async () => {
    const res = mockResponse('');
    const out = await readJsonSafe(res);
    expect(out.ok).toBe(true);
    if (out.ok) expect(out.data).toBeNull();
  });

  it('returns error for non-JSON body', async () => {
    const res = mockResponse('<!DOCTYPE html><html></html>', 'text/html');
    const out = await readJsonSafe(res);
    expect(out.ok).toBe(false);
    if (!out.ok) expect(out.error).toBe('Invalid JSON response');
  });
});
