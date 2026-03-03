import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import DashboardLayout from '../components/Layout/DashboardLayout';
import ChartCard from '../components/Dashboard/ChartCard';
import StatsCard from '../components/Dashboard/StatsCard';
import { fetchDashboardOverview } from '../services/dashboardService';
import type { ChartDataPoint } from '../types/dashboard';
import { useAuth } from '../contexts/AuthContext';

const UsagePage: React.FC = () => {
  const { isAuthenticated, user, token } = useAuth();
  const navigate = useNavigate();
  const [overview, setOverview] = useState<Awaited<ReturnType<typeof fetchDashboardOverview>> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  useEffect(() => {
    if (!token) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    fetchDashboardOverview(token)
      .then(setOverview)
      .catch((err) => setError(err instanceof Error ? err.message : 'Failed to load usage'))
      .finally(() => setLoading(false));
  }, [token]);

  if (!isAuthenticated || !user) {
    return null;
  }

  if (loading && !overview) {
    return (
      <DashboardLayout pageTitle="Usage">
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-text-muted)' }}>Loading usage…</div>
      </DashboardLayout>
    );
  }

  if (error && !overview) {
    return (
      <DashboardLayout pageTitle="Usage">
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-danger)' }}>{error}</div>
      </DashboardLayout>
    );
  }

  const stats = overview?.stats;
  const chartData: ChartDataPoint[] = (overview?.chart_data ?? []).map((d) => ({
    month: d.month,
    publishready: d.publishready,
    datamaestro: d.datamaestro,
    value: d.publishready,
    value2: d.datamaestro,
  }));

  const statsCards = [
    { id: 'publishready', title: 'PublishReady Uses', value: stats ? `${stats.publishready_used}/${stats.publishready_total}` : '0/0', showInfo: true },
    { id: 'datamaestro', title: 'DataMaestro Uses', value: stats ? `${stats.datamaestro_used}/${stats.datamaestro_total}` : '0/0', showInfo: true },
    { id: 'journal_check', title: 'Journal Verification Uses', value: stats ? `${stats.journal_check_used || 0}/${stats.journal_check_total || 0}` : '0/0', showInfo: true },
    { id: 'projects', title: 'Total Projects', value: stats ? String(stats.total_projects) : '0', showInfo: false },
  ];

  return (
    <DashboardLayout pageTitle="Usage">
      <div style={{ maxWidth: 900, margin: '0 auto' }}>
        <ChartCard data={chartData} />
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6 mb-6">
          {statsCards.map((card) => (
            <StatsCard key={card.id} title={card.title} value={card.value} showInfo={card.showInfo} />
          ))}
        </div>
      </div>
    </DashboardLayout>
  );
};

export default UsagePage;
