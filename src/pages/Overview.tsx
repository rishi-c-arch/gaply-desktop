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
    journal_check: d.journal_check ?? 0,
    research_deep: 0,
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
      <div className="overview-page overview-hud">
        {/* HUD Stats Row */}
        <div className="overview-hud-stats">
          <div className="overview-hud-stat">
            <span className="overview-hud-stat-label">Manuscripts Analyzed</span>
            <div className="overview-hud-stat-value-row">
              <span className="overview-hud-stat-value overview-hud-stat-primary">
                {stats ? stats.publishready_used + stats.datamaestro_used : 0}
                <span className="overview-hud-stat-unit">UNITS</span>
              </span>
              <span className="overview-hud-stat-badge">Active</span>
            </div>
            <div className="overview-hud-stat-bar">
              <div
                className="overview-hud-stat-bar-fill overview-hud-stat-bar-primary"
                style={{ width: stats ? `${Math.min(100, ((stats.publishready_used + stats.datamaestro_used) / 20) * 100)}%` : '0%' }}
              />
            </div>
          </div>
          <div className="overview-hud-stat">
            <span className="overview-hud-stat-label">Journal Match Accuracy</span>
            <div className="overview-hud-stat-value-row">
              <span className="overview-hud-stat-value">99.8<span className="overview-hud-stat-unit">%</span></span>
              <span className="material-symbols-outlined overview-hud-stat-icon">trending_up</span>
            </div>
            <div className="overview-hud-stat-dots">
              {[1, 2, 3, 4, 5].map((i) => (
                <div key={i} className={`overview-hud-stat-dot ${i <= 4 ? 'active' : ''}`} />
              ))}
            </div>
          </div>
          <div className="overview-hud-stat">
            <span className="overview-hud-stat-label">Active Researchers</span>
            <div className="overview-hud-stat-value-row">
              <span className="overview-hud-stat-value">{stats ? stats.total_projects : 0}</span>
              <span className="overview-hud-stat-meta">+12 / MIN</span>
            </div>
            <div className="overview-hud-stat-pulse">
              <span className="overview-hud-stat-pulse-dot" />
              <span className="overview-hud-stat-pulse-label">Active</span>
            </div>
          </div>
          <div className="overview-hud-stat overview-hud-stat-highlight">
            <span className="overview-hud-stat-label overview-hud-stat-label-primary">Total Data Verified</span>
            <div className="overview-hud-stat-value-row">
              <span className="overview-hud-stat-value">85.4<span className="overview-hud-stat-unit">PB</span></span>
              <span className="overview-hud-stat-peak">PEAK</span>
            </div>
            <div className="overview-hud-stat-chart">
              {[1, 2, 3, 4, 5].map((i) => (
                <div key={i} className="overview-hud-stat-chart-bar" style={{ height: `${20 + i * 15}%` }} />
              ))}
            </div>
          </div>
        </div>

        {/* Content Grid */}
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
      </div>
    </DashboardLayout>
  );
};

export default Overview;
