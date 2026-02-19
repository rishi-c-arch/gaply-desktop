import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { CreditCard, User, Mail, HelpCircle, ArrowRight, AlertTriangle } from 'lucide-react';
import { accountMenuItems, supportMenuItems } from '../../data/mockData';
import { useAuth } from '../../contexts/AuthContext';

const WHATSAPP_NUMBER = '916387144799';
const WHATSAPP_URL = `https://wa.me/${WHATSAPP_NUMBER}`;

const AccountCard: React.FC = () => {
  const navigate = useNavigate();
  const { deleteAccount } = useAuth();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleDeleteClick = () => {
    if (!confirmDelete) {
      setConfirmDelete(true);
      setError(null);
      return;
    }
    setDeleting(true);
    setError(null);
    deleteAccount()
      .then((result) => {
        if (result.success) {
          window.location.href = '/';
        } else {
          setError(result.error || 'Failed to delete account');
          setDeleting(false);
        }
      })
      .catch(() => {
        setError('Failed to delete account');
        setDeleting(false);
      });
  };

  const handleAccountClick = (itemId: string) => {
    if (itemId === 'billing') navigate('/dashboard/billing');
    else if (itemId === 'profile') navigate('/dashboard/profile');
    // plan: no navigation or could link to upgrade
  };

  const handleSupportClick = (itemId: string) => {
    if (itemId === 'contact') window.open(WHATSAPP_URL, '_blank', 'noopener,noreferrer');
    else if (itemId === 'help') navigate('/dashboard/help');
  };

  const renderAccountItem = (item: (typeof accountMenuItems)[0]) => {
    const Icon = item.icon === 'CreditCard' ? CreditCard : item.icon === 'User' ? User : null;
    const isClickable = item.id === 'billing' || item.id === 'profile';
    return (
      <button
        key={item.id}
        type="button"
        onClick={() => isClickable && handleAccountClick(item.id)}
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
          cursor: isClickable ? 'pointer' : 'default',
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
        {isClickable && <ArrowRight size={16} color="var(--dashboard-text-muted)" />}
      </button>
    );
  };

  const renderSupportItem = (item: (typeof supportMenuItems)[0]) => {
    const Icon = item.icon === 'Mail' ? Mail : HelpCircle;
    return (
      <button
        key={item.id}
        type="button"
        onClick={() => handleSupportClick(item.id)}
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
        {confirmDelete && (
          <div
            style={{
              marginTop: 16,
              padding: 12,
              background: 'rgba(239, 68, 68, 0.08)',
              border: '1px solid var(--dashboard-danger)',
              borderRadius: 8,
              marginBottom: 8,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <AlertTriangle size={18} color="var(--dashboard-danger)" />
              <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--dashboard-text)' }}>
                Permanently delete your account and all data?
              </span>
            </div>
            <p style={{ fontSize: 12, color: 'var(--dashboard-text-muted)', margin: '0 0 12px 0' }}>
              Your email, password, and all account data will be removed from our systems. This cannot be undone.
            </p>
            {error && (
              <p style={{ fontSize: 12, color: 'var(--dashboard-danger)', margin: '0 0 8px 0' }}>{error}</p>
            )}
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                type="button"
                onClick={() => { setConfirmDelete(false); setError(null); }}
                disabled={deleting}
                style={{
                  padding: '8px 16px',
                  fontSize: 13,
                  fontWeight: 500,
                  color: 'var(--dashboard-text)',
                  background: 'var(--dashboard-sidebar-active-bg)',
                  border: '1px solid var(--dashboard-border)',
                  borderRadius: 8,
                  cursor: deleting ? 'not-allowed' : 'pointer',
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleDeleteClick}
                disabled={deleting}
                style={{
                  padding: '8px 16px',
                  fontSize: 13,
                  fontWeight: 600,
                  color: '#fff',
                  background: 'var(--dashboard-danger)',
                  border: 'none',
                  borderRadius: 8,
                  cursor: deleting ? 'not-allowed' : 'pointer',
                }}
              >
                {deleting ? 'Deleting…' : 'Yes, delete my account'}
              </button>
            </div>
          </div>
        )}
        <button
          type="button"
          onClick={confirmDelete ? () => { setConfirmDelete(false); setError(null); } : handleDeleteClick}
          disabled={deleting}
          style={{
            width: '100%',
            marginTop: confirmDelete ? 0 : 16,
            padding: '12px 16px',
            background: confirmDelete ? 'var(--dashboard-sidebar-active-bg)' : 'var(--dashboard-danger)',
            border: `1px solid ${confirmDelete ? 'var(--dashboard-border)' : 'transparent'}`,
            borderRadius: 8,
            color: confirmDelete ? 'var(--dashboard-text)' : '#FFFFFF',
            fontSize: 14,
            fontWeight: 600,
            cursor: deleting ? 'not-allowed' : 'pointer',
            transition: 'all 150ms ease',
          }}
        >
          {confirmDelete ? 'Cancel' : 'Delete Account'}
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
