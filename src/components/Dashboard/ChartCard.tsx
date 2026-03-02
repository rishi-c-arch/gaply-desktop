import React, { useState, useRef, useEffect } from 'react';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts';
import { Calendar, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react';
import type { ChartDataPoint } from '../../types/dashboard';

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

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
    value: d.publishready ?? d.value ?? 0,
    value2: d.datamaestro ?? d.value2 ?? 0,
    monthIndex: i,
  }));

  const displayData =
    selectedMonth != null
      ? chartData.filter((d) => d.monthIndex === selectedMonth)
      : chartData;
  const maxVal = Math.max(
    ...displayData.flatMap((d) => [d.value, d.value2]),
    1
  );
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
      className="dashboard-card"
      style={{
        background: '#020617',
        borderRadius: 16,
        border: '1px solid rgba(30, 64, 175, 0.4)',
        padding: 24,
        marginBottom: 24,
        boxShadow: '0 18px 45px rgba(15, 23, 42, 0.65)',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 24 }}>
        <h3 style={{ fontSize: 16, fontWeight: 600, color: '#e5e7eb', margin: 0 }}>
          Feature Usage Overview (PublishReady & DataMaestro)
        </h3>
        <div ref={calendarRef} style={{ position: 'relative' }}>
          <button
            type="button"
            onClick={() => setCalendarOpen((o) => !o)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 8,
              background: 'rgba(59, 130, 246, 0.14)',
              border: '1px solid rgba(30, 64, 175, 0.5)',
              borderRadius: 8,
              padding: '8px 12px',
              color: '#e5e7eb',
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
                background: '#020617',
                border: '1px solid rgba(30, 64, 175, 0.5)',
                borderRadius: 14,
                boxShadow: '0 18px 45px rgba(15,23,42,0.85)',
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

      <div style={{ width: '100%', minWidth: 0, height: 280, minHeight: 280 }}>
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={displayData} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
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
            <CartesianGrid strokeDasharray="3 3" stroke="rgba(51,65,85,0.7)" vertical={false} />
            <XAxis
              dataKey="month"
              axisLine={false}
              tickLine={false}
              tick={{ fill: '#9ca3af', fontSize: 12 }}
            />
            <YAxis
              domain={yDomain}
              axisLine={false}
              tickLine={false}
              tick={{ fill: '#9ca3af', fontSize: 12 }}
            />
            <Tooltip
              contentStyle={{
                background: '#020617',
                border: '1px solid rgba(30,64,175,0.6)',
                borderRadius: 8,
                color: '#e5e7eb',
              }}
              labelStyle={{ color: '#9ca3af' }}
              formatter={(value: number | undefined) => [value ?? 0, '']}
              labelFormatter={(label) => `${label} ${selectedYear}`}
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
