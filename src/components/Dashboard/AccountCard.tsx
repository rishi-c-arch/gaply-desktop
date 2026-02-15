import React from 'react';
import { CreditCard, User, Mail, HelpCircle, ArrowRight } from 'lucide-react';
import { accountMenuItems, supportMenuItems } from '../../data/mockData';

const AccountCard: React.FC = () => {
  const renderAccountItem = (item: (typeof accountMenuItems)[0]) => {
    const Icon = item.icon === 'CreditCard' ? CreditCard : item.icon === 'User' ? User : null;
    return (
      <button
        key={item.id}
        style={{
          width: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '12px 0',
          background: 'transparent',
          border: 'none',
          borderBottom: '1px solid var(--dashboard-border)',
          color: 'var(--dashboard-text)',
          cursor: 'pointer',
          fontSize: 14,
          textAlign: 'left',
          transition: 'background 150ms ease',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          {Icon && <Icon size={18} color="var(--dashboard-text-muted)" />}
          <div>
            {item.sublabel ? (
              <span>
                {item.label}:{' '}
                <span style={{ color: 'var(--dashboard-text-muted)' }}>
                  Gaply Premium
                  {item.highlight && (
                    <span style={{ color: 'var(--dashboard-accent)', marginLeft: 4 }}>(Upgrade)</span>
                  )}
                </span>
              </span>
            ) : (
              <span>{item.label}</span>
            )}
          </div>
        </div>
        <ArrowRight size={16} color="var(--dashboard-text-muted)" />
      </button>
    );
  };

  const renderSupportItem = (item: (typeof supportMenuItems)[0]) => {
    const Icon = item.icon === 'Mail' ? Mail : HelpCircle;
    return (
      <button
        key={item.id}
        style={{
          width: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '12px 0',
          background: 'transparent',
          border: 'none',
          borderBottom: item.id === 'help' ? 'none' : '1px solid var(--dashboard-border)',
          color: 'var(--dashboard-text)',
          cursor: 'pointer',
          fontSize: 14,
          textAlign: 'left',
          transition: 'background 150ms ease',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <Icon size={18} color="var(--dashboard-text-muted)" />
          <span>{item.label}</span>
        </div>
        <ArrowRight size={16} color="var(--dashboard-text-muted)" />
      </button>
    );
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
      <div
        style={{
          background: 'var(--dashboard-card-bg)',
          borderRadius: 12,
          border: '1px solid var(--dashboard-border)',
          padding: 24,
        }}
      >
        <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 16px 0' }}>
          Your Account
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column' }}>
          {accountMenuItems.map(renderAccountItem)}
        </div>
        <button
          style={{
            width: '100%',
            marginTop: 16,
            padding: '12px 16px',
            background: 'var(--dashboard-danger)',
            border: 'none',
            borderRadius: 8,
            color: '#FFFFFF',
            fontSize: 14,
            fontWeight: 600,
            cursor: 'pointer',
            transition: 'all 150ms ease',
          }}
        >
          Delete Account
        </button>
      </div>

      <div
        style={{
          background: 'var(--dashboard-card-bg)',
          borderRadius: 12,
          border: '1px solid var(--dashboard-border)',
          padding: 24,
        }}
      >
        <h3 style={{ fontSize: 16, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 16px 0' }}>
          Customer Support
        </h3>
        <div style={{ display: 'flex', flexDirection: 'column' }}>
          {supportMenuItems.map(renderSupportItem)}
        </div>
      </div>
    </div>
  );
};

export default AccountCard;
