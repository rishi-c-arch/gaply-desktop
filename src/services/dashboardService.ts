import { apiFetch } from '../api/config';

export interface DashboardStats {
  publishready_used: number;
  publishready_total: number;
  datamaestro_used: number;
  datamaestro_total: number;
  total_projects: number;
}

export interface ChartMonth {
  month: string;
  publishready: number;
  datamaestro: number;
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
  if (!res.ok) {
    const err = await res.json().catch(() => ({}));
    throw new Error((err as { error?: string }).error || `Failed to load dashboard (${res.status})`);
  }
  return res.json();
}
