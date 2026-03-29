import { useCallback, useEffect, useState } from 'react';
import { apiFetch } from '../../api/config';
import type { ConferencesIndiaResponse } from './types';

export function useConferencesIndia() {
  const [data, setData] = useState<ConferencesIndiaResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch('/api/v1/conferences-india');
      if (!res.ok) {
        const j = await res.json().catch(() => ({}));
        setError((j as { details?: string }).details || res.statusText || 'Failed to load');
        setData(null);
        return;
      }
      const json = (await res.json()) as ConferencesIndiaResponse;
      setData(json);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : 'Network error');
      setData(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  return { data, loading, error, reload: load };
}
