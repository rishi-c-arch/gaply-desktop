import React, { useEffect, useState } from 'react';
import DashboardLayout from '../components/Layout/DashboardLayout';
import ChartCard from '../components/Dashboard/ChartCard';
import './Overview.css';
import StatsCard from '../components/Dashboard/StatsCard';
import AccountCard from '../components/Dashboard/AccountCard';
import { fetchDashboardOverview } from '../services/dashboardService';
import type { ChartDataPoint } from '../types/dashboard';
import { useAuth } from '../contexts/AuthContext';

const Overview: React.FC = () => {
  const { token } = useAuth();
  const [overview, setOverview] = useState<Awaited<ReturnType<typeof fetchDashboardOverview>> | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!token) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    fetchDashboardOverview(token)
      .then(setOverview)
      .catch((err) => setError(err instanceof Error ? err.message : 'Failed to load dashboard'))
      .finally(() => setLoading(false));
  }, [token]);

  if (loading && !overview) {
    return (
      <DashboardLayout>
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-text-muted)' }}>Loading dashboard…</div>
      </DashboardLayout>
    );
  }

  if (error && !overview) {
    return (
      <DashboardLayout>
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
    {
      id: 'publishready',
      title: 'PublishReady Uses',
      value: stats ? `${stats.publishready_used}/${stats.publishready_total}` : '0/0',
      showInfo: true,
    },
    {
      id: 'datamaestro',
      title: 'DataMaestro Uses',
      value: stats ? `${stats.datamaestro_used}/${stats.datamaestro_total}` : '0/0',
      showInfo: true,
    },
    {
      id: 'projects',
      title: 'Total Projects',
      value: stats ? String(stats.total_projects) : '0',
      showInfo: false,
    },
  ];

  return (
    <DashboardLayout>
      <div className="overview-page">
        <header className="overview-page-header">
          <div className="overview-page-header__left">
            <p className="overview-page-tag">System Analytics</p>
            <h1 className="overview-page-title">Feature Usage Overview</h1>
            <p className="overview-page-subtitle">PublishReady &amp; DataMaestro activity metrics</p>
          </div>
        </header>
      </div>
      <div className="overview-root">
        <div className="overview-main">
          <ChartCard data={chartData} />
          <div className="overview-stats">
            {statsCards.map((card) => (
              <StatsCard
                key={card.id}
                title={card.title}
                value={card.value}
                showInfo={card.showInfo}
              />
            ))}
          </div>
        </div>
        <div className="overview-main">
          <AccountCard />
        </div>
      </div>
    </DashboardLayout>
  );
};

export default Overview;
