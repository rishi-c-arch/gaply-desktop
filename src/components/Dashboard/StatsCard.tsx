import React from 'react';
import { Info } from 'lucide-react';

interface StatsCardProps {
  title: string;
  value: string;
  showInfo?: boolean;
}

const StatsCard: React.FC<StatsCardProps> = ({ title, value, showInfo }) => {
  return (
    <div
      className="dashboard-card"
      style={{
        background: 'var(--dashboard-card-bg)',
        borderRadius: 12,
        border: '1px solid var(--dashboard-border)',
        padding: 24,
        flex: 1,
        minWidth: 0,
        transition: 'border-color 0.2s ease, box-shadow 0.2s ease, transform 0.2s ease',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
        <span style={{ fontSize: 14, color: 'var(--dashboard-text-muted)', fontWeight: 500 }}>{title}</span>
        {showInfo && (
          <button
            style={{
              background: 'transparent',
              border: 'none',
              cursor: 'pointer',
              padding: 4,
              color: 'var(--dashboard-text-muted)',
            }}
            title="Info"
          >
            <Info size={16} />
          </button>
        )}
      </div>
      <div style={{ fontSize: 24, fontWeight: 700, color: 'var(--dashboard-text)' }}>{value}</div>
    </div>
  );
};

export default StatsCard;
