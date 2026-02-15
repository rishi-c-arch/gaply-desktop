import React from 'react';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { Calendar, ChevronDown } from 'lucide-react';
import { chartData as fallbackChartData } from '../../data/mockData';
import type { ChartDataPoint } from '../../types/dashboard';

interface ChartCardProps {
  data?: ChartDataPoint[];
}

const ChartCard: React.FC<ChartCardProps> = ({ data }) => {
  const chartData = (data ?? fallbackChartData).map((d) => ({
    month: d.month,
    value: d.publishready ?? d.value ?? 0,
    value2: d.datamaestro ?? d.value2 ?? 0,
  }));
  const maxVal = Math.max(
    ...chartData.flatMap((d) => [d.value, d.value2]),
    1
  );
  const yDomain = [0, Math.ceil(maxVal * 1.2) || 2.5];
  return (
    <div
      style={{
        background: 'var(--dashboard-card-bg)',
        borderRadius: 12,
        border: '1px solid var(--dashboard-border)',
        padding: 24,
        marginBottom: 24,
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
        <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dashboard-text)', margin: 0 }}>
          Feature Usage Overview (PublishReady & DataMaestro)
        </h3>
        <button
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            background: 'rgba(59, 130, 246, 0.1)',
            border: '1px solid var(--dashboard-border)',
            borderRadius: 8,
            padding: '8px 12px',
            color: 'var(--dashboard-text-muted)',
            fontSize: 14,
            cursor: 'pointer',
          }}
        >
          <Calendar size={16} />
          Activity
          <ChevronDown size={16} />
        </button>
      </div>

      <div style={{ height: 280 }}>
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={chartData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
            <defs>
              <linearGradient id="colorValue" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#3B82F6" stopOpacity={0.3} />
                <stop offset="95%" stopColor="#3B82F6" stopOpacity={0} />
              </linearGradient>
              <linearGradient id="colorValue2" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#6B7280" stopOpacity={0.2} />
                <stop offset="95%" stopColor="#6B7280" stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="var(--dashboard-chart-grid)" vertical={false} />
            <XAxis
              dataKey="month"
              axisLine={false}
              tickLine={false}
              tick={{ fill: 'var(--dashboard-text-muted)', fontSize: 12 }}
            />
            <YAxis
              domain={yDomain}
              axisLine={false}
              tickLine={false}
              tick={{ fill: 'var(--dashboard-text-muted)', fontSize: 12 }}
            />
            <Tooltip
              contentStyle={{
                background: 'var(--dashboard-card-bg)',
                border: '1px solid var(--dashboard-border)',
                borderRadius: 8,
                color: 'var(--dashboard-text)',
              }}
              labelStyle={{ color: 'var(--dashboard-text-muted)' }}
            />
            <Area
              type="monotone"
              dataKey="value"
              stroke="#3B82F6"
              strokeWidth={2}
              fill="url(#colorValue)"
            />
            <Area
              type="monotone"
              dataKey="value2"
              stroke="#6B7280"
              strokeWidth={2}
              fill="url(#colorValue2)"
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};

export default ChartCard;
