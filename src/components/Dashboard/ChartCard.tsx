import React, { useState, useRef, useEffect } from 'react';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts';
import { Calendar, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react';
import type { ChartDataPoint } from '../../types/dashboard';

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const FEATURE_COLORS = {
  publishready: '#10b981',
  datamaestro: '#3b82f6',
  journal_check: '#8b5cf6',
  research_deep: '#f59e0b',
};

interface ChartCardProps {
  data?: ChartDataPoint[];
}

const ChartCard: React.FC<ChartCardProps> = ({ data }) => {
  const [calendarOpen, setCalendarOpen] = useState(false);
  const [selectedYear, setSelectedYear] = useState(() => new Date().getFullYear());
  const [selectedMonth, setSelectedMonth] = useState<number | null>(null);
  const calendarRef = useRef<HTMLDivElement>(null);

  const baseData = data ?? [];
  const chartData = baseData.map((d, i) => ({
    month: d.month,
    PublishReady: d.publishready ?? d.value ?? 0,
    DataMaestro: d.datamaestro ?? d.value2 ?? 0,
    'Journal Verification': d.journal_check ?? 0,
    'Research Deep Analysis': d.research_deep ?? 0,
    monthIndex: i,
  }));

  const displayData =
    selectedMonth != null
      ? chartData.filter((d) => d.monthIndex === selectedMonth)
      : chartData;
  const allValues = displayData.flatMap((d) => [
    d.PublishReady,
    d.DataMaestro,
    d['Journal Verification'],
    d['Research Deep Analysis'],
  ]);
  const maxVal = Math.max(...allValues, 1);
  const yDomain = [0, Math.ceil(maxVal * 1.2) || 2.5];

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (calendarRef.current && !calendarRef.current.contains(e.target as Node)) {
        setCalendarOpen(false);
      }
    };
    if (calendarOpen) {
      document.addEventListener('mousedown', handleClickOutside);
      return () => document.removeEventListener('mousedown', handleClickOutside);
    }
  }, [calendarOpen]);

  const displayLabel = selectedMonth != null
    ? `${MONTHS[selectedMonth]} ${selectedYear}`
    : selectedYear.toString();

  return (
    <div
      className="dashboard-card glass-panel hud-border"
      style={{
        background: 'var(--dashboard-glass)',
        backdropFilter: 'blur(12px)',
        borderRadius: 4,
        border: '1px solid var(--dashboard-glass-border)',
        padding: 24,
        marginBottom: 24,
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
        <div>
          <h3 style={{ fontSize: 12, fontWeight: 700, color: 'var(--dashboard-text)', margin: 0, textTransform: 'uppercase', letterSpacing: '0.1em' }}>
            Feature Usage Overview (All 4 Premium Features)
          </h3>
          <p style={{ fontSize: 11, color: 'var(--dashboard-text-muted)', margin: '4px 0 0', fontStyle: 'italic' }}>
            PublishReady · DataMaestro · Journal Verification · Research Deep Analysis
          </p>
        </div>
        <div ref={calendarRef} style={{ position: 'relative' }}>
          <button
            type="button"
            onClick={() => setCalendarOpen((o) => !o)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              background: 'var(--dashboard-sidebar-active-bg)',
              border: '1px solid var(--dashboard-border)',
              borderRadius: 999,
              padding: '8px 12px',
              color: 'var(--dashboard-text)',
              fontSize: 14,
              cursor: 'pointer',
            }}
          >
            <Calendar size={16} />
            {displayLabel}
            <ChevronDown size={16} style={{ transform: calendarOpen ? 'rotate(180deg)' : 'none', transition: 'transform 150ms ease' }} />
          </button>

          {calendarOpen && (
            <div
              style={{
                position: 'absolute',
                top: '100%',
                right: 0,
                marginTop: 8,
                background: 'var(--dashboard-bg)',
                border: '1px solid var(--dashboard-border)',
                borderRadius: 4,
                boxShadow: '0 18px 45px rgba(0,0,0,0.4)',
                padding: 20,
                minWidth: 280,
                zIndex: 50,
              }}
            >
              <div style={{ fontSize: 13, fontWeight: 600, color: '#9ca3af', marginBottom: 12 }}>
                Choose period
              </div>

              {/* Year selector */}
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  marginBottom: 16,
                }}
              >
                <button
                  type="button"
                  onClick={() => setSelectedYear((y) => y - 1)}
                  style={{
                    width: 32,
                    height: 32,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    background: 'var(--dashboard-sidebar-active-bg)',
                    border: '1px solid var(--dashboard-border)',
                    borderRadius: 8,
                    color: 'var(--dashboard-text)',
                    cursor: 'pointer',
                  }}
                >
                  <ChevronLeft size={18} />
                </button>
                <span style={{ fontSize: 16, fontWeight: 600, color: 'var(--dashboard-text)' }}>{selectedYear}</span>
                <button
                  type="button"
                  onClick={() => setSelectedYear((y) => y + 1)}
                  style={{
                    width: 32,
                    height: 32,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    background: 'var(--dashboard-sidebar-active-bg)',
                    border: '1px solid var(--dashboard-border)',
                    borderRadius: 8,
                    color: 'var(--dashboard-text)',
                    cursor: 'pointer',
                  }}
                >
                  <ChevronRight size={18} />
                </button>
              </div>

              {/* Month grid */}
              <div
                style={{
                  display: 'grid',
                  gridTemplateColumns: 'repeat(4, 1fr)',
                  gap: 6,
                }}
              >
                {MONTHS.map((month, index) => {
                  const isSelected = selectedMonth === index;
                  return (
                    <button
                      key={month}
                      type="button"
                      onClick={() => setSelectedMonth(isSelected ? null : index)}
                      style={{
                        padding: '10px 8px',
                        fontSize: 12,
                        fontWeight: 500,
                        color: isSelected ? '#f9fafb' : '#e5e7eb',
                        background: isSelected ? '#3b82f6' : 'rgba(15,23,42,0.9)',
                        border: `1px solid ${isSelected ? '#3b82f6' : 'rgba(30,64,175,0.6)'}`,
                        borderRadius: 8,
                        cursor: 'pointer',
                      }}
                    >
                      {month}
                    </button>
                  );
                })}
              </div>

              <div style={{ marginTop: 12, fontSize: 12, color: '#9ca3af' }}>
                {selectedMonth != null
                  ? `Usage for ${MONTHS[selectedMonth]} ${selectedYear}`
                  : `Monthly usage for ${selectedYear}`}
              </div>
            </div>
          )}
        </div>
      </div>

      <div style={{ width: '100%', minWidth: 160, height: 320, minHeight: 200 }}>
        <ResponsiveContainer width="100%" height="100%" minWidth={160} minHeight={200} debounce={50}>
          <AreaChart data={displayData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
            <defs>
              <linearGradient id="colorPublishReady" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={FEATURE_COLORS.publishready} stopOpacity={0.35} />
                <stop offset="95%" stopColor={FEATURE_COLORS.publishready} stopOpacity={0} />
              </linearGradient>
              <linearGradient id="colorDataMaestro" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={FEATURE_COLORS.datamaestro} stopOpacity={0.35} />
                <stop offset="95%" stopColor={FEATURE_COLORS.datamaestro} stopOpacity={0} />
              </linearGradient>
              <linearGradient id="colorJournalVerification" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={FEATURE_COLORS.journal_check} stopOpacity={0.35} />
                <stop offset="95%" stopColor={FEATURE_COLORS.journal_check} stopOpacity={0} />
              </linearGradient>
              <linearGradient id="colorResearchDeep" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor={FEATURE_COLORS.research_deep} stopOpacity={0.35} />
                <stop offset="95%" stopColor={FEATURE_COLORS.research_deep} stopOpacity={0} />
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
                background: 'var(--dashboard-bg)',
                border: '1px solid var(--dashboard-border)',
                borderRadius: 8,
                color: 'var(--dashboard-text)',
                padding: '12px 16px',
              }}
              labelStyle={{ color: 'var(--dashboard-text-muted)', marginBottom: 8 }}
              formatter={(value: number | undefined, name?: string) => [`${value ?? 0} uses`, name ?? '']}
              labelFormatter={(label) => `${label} ${selectedYear}`}
            />
            <Legend
              wrapperStyle={{ paddingTop: 16 }}
              formatter={(value) => <span style={{ color: 'var(--dashboard-text)', fontSize: 12 }}>{value}</span>}
              iconType="circle"
              iconSize={8}
            />
            <Area
              type="monotone"
              dataKey="PublishReady"
              name="PublishReady"
              stroke={FEATURE_COLORS.publishready}
              strokeWidth={2}
              fill="url(#colorPublishReady)"
            />
            <Area
              type="monotone"
              dataKey="DataMaestro"
              name="DataMaestro"
              stroke={FEATURE_COLORS.datamaestro}
              strokeWidth={2}
              fill="url(#colorDataMaestro)"
            />
            <Area
              type="monotone"
              dataKey="Journal Verification"
              name="Journal Verification"
              stroke={FEATURE_COLORS.journal_check}
              strokeWidth={2}
              fill="url(#colorJournalVerification)"
            />
            <Area
              type="monotone"
              dataKey="Research Deep Analysis"
              name="Research Deep Analysis"
              stroke={FEATURE_COLORS.research_deep}
              strokeWidth={2}
              fill="url(#colorResearchDeep)"
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
};

export default ChartCard;
