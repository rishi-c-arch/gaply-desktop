import { apiFetch } from '../api/config';

export interface DashboardStats {
  publishready_used: number;
  publishready_total: number;
  datamaestro_used: number;
  datamaestro_total: number;
  journal_check_used: number;
  journal_check_total: number;
  total_projects: number;
}

export interface ChartMonth {
  month: string;
  publishready: number;
  datamaestro: number;
  journal_check?: number;
}

export interface DashboardProject {
  id: string;
  name: string;
  date: string;
  status: string;
}

export interface DashboardOverview {
  success: boolean;
  stats: DashboardStats;
  chart_data: ChartMonth[];
  projects: DashboardProject[];
}

export interface DashboardOverviewState {
  data: DashboardOverview | null;
  loading: boolean;
  error: string | null;
}

export async function fetchDashboardOverview(
  token: string | null
): Promise<DashboardOverview> {
  if (!token) {
    throw new Error('Authentication required');
  }
  const res = await apiFetch('/api/dashboard/overview', {
    method: 'GET',
    headers: {
      Authorization: `Bearer ${token}`,
    },
  });
  const text = await res.text();
  // Treat an empty successful response as \"no data yet\" instead of a hard error.
  if ((!text || !text.trim()) && res.ok) {
    return {
      success: true,
      stats: {
        publishready_used: 0,
        publishready_total: 0,
        datamaestro_used: 0,
        datamaestro_total: 0,
        journal_check_used: 0,
        journal_check_total: 0,
        total_projects: 0,
      },
      chart_data: [],
      projects: [],
    };
  }
  let raw: DashboardOverview | { error?: string };
  try {
    raw = JSON.parse(text) as DashboardOverview | { error?: string };
  } catch {
    throw new Error(res.ok ? 'Dashboard unavailable (invalid response)' : `Failed to load dashboard (${res.status})`);
  }
  if (!res.ok) {
    throw new Error((raw as { error?: string }).error || `Failed to load dashboard (${res.status})`);
  }
  return raw as DashboardOverview;
}

export interface BillingTransactionRow {
  id: string;
  date: string;
  description: string;
  amount: string;
  status: string;
}

export interface BillingResponse {
  success: boolean;
  transactions: BillingTransactionRow[];
}

export async function fetchBillingTransactions(
  token: string | null
): Promise<BillingTransactionRow[]> {
  if (!token) {
    throw new Error('Authentication required');
  }
  const res = await apiFetch('/api/dashboard/billing', {
    method: 'GET',
    headers: {
      Authorization: `Bearer ${token}`,
    },
  });
  const text = await res.text();
  // If backend returns 200 with empty body, treat as \"no transactions yet\".
  if ((!text || !text.trim()) && res.ok) {
    return [];
  }
  let data: BillingResponse & { error?: string };
  try {
    data = JSON.parse(text) as BillingResponse & { error?: string };
  } catch {
    throw new Error(res.ok ? 'Billing unavailable (invalid response)' : `Failed to load billing (${res.status})`);
  }
  if (!res.ok) {
    throw new Error(data.error || `Failed to load billing (${res.status})`);
  }
  return data.transactions || [];
}

/** Create a report record for Recent Projects when PublishReady or DataMaestro completes. Fire-and-forget; does not throw. */
export async function createReport(
  token: string | null,
  sourceName: string,
  sourceType?: 'publishready' | 'datamaestro'
): Promise<void> {
  if (!token || !sourceName?.trim()) return;
  try {
    await apiFetch('/api/dashboard/report', {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
      body: JSON.stringify({
        source_name: sourceName.trim().slice(0, 500),
        source_type: sourceType || undefined,
      }),
    });
  } catch {
    /* fire-and-forget; do not surface to user */
  }
}
