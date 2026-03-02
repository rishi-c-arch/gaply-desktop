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
        background: '#020617',
        borderRadius: 16,
        border: '1px solid rgba(30, 64, 175, 0.4)',
        padding: 24,
        flex: 1,
        minWidth: 0,
        boxShadow: '0 16px 40px rgba(15,23,42,0.7)',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
        <span style={{ fontSize: 14, color: '#9ca3af', fontWeight: 500 }}>{title}</span>
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
      <div style={{ fontSize: 24, fontWeight: 700, color: '#e5e7eb' }}>{value}</div>
    </div>
  );
};

export default StatsCard;
