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
      style={{
        background: 'var(--dashboard-card-bg)',
        borderRadius: 12,
        border: '1px solid var(--dashboard-border)',
        padding: 20,
        flex: 1,
        minWidth: 0,
        transition: 'all 150ms ease',
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
