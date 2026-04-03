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
      className="dashboard-card glass-panel hud-border"
      style={{
        background: 'var(--dashboard-glass)',
        backdropFilter: 'blur(12px)',
        borderRadius: 4,
        border: '1px solid var(--dashboard-glass-border)',
        padding: 24,
        flex: 1,
        minWidth: 0,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
        <span style={{ fontSize: 10, color: 'var(--dashboard-text-muted)', fontWeight: 500, textTransform: 'uppercase', letterSpacing: '0.05em' }}>{title}</span>
        {showInfo && (
          <button
            style={{
              background: 'transparent',
              border: 'none',
              cursor: 'pointer',
              padding: 4,
              color: '#9ca3af',
            }}
            title="Info"
          >
            <Info size={16} />
          </button>
        )}
      </div>
      <div style={{ fontSize: 28, fontWeight: 300, color: 'var(--dashboard-text)' }}>{value}</div>
    </div>
  );
};

export default StatsCard;
