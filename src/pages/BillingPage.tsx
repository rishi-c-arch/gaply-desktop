import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Receipt, CheckCircle } from 'lucide-react';
import DashboardLayout from '../components/Layout/DashboardLayout';
import { fetchBillingTransactions } from '../services/dashboardService';
import type { BillingTransactionRow } from '../services/dashboardService';
import { useAuth } from '../contexts/AuthContext';

const BillingPage: React.FC = () => {
  const { isAuthenticated, user, token } = useAuth();
  const navigate = useNavigate();
  const [transactions, setTransactions] = useState<BillingTransactionRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
    }
  }, [isAuthenticated, user, navigate]);

  useEffect(() => {
    if (!token) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    fetchBillingTransactions(token)
      .then(setTransactions)
      .catch((err) => setError(err instanceof Error ? err.message : 'Failed to load billing'))
      .finally(() => setLoading(false));
  }, [token]);

  if (!isAuthenticated || !user) {
    return null;
  }

  if (loading) {
    return (
      <DashboardLayout pageTitle="Billing">
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-text-muted)' }}>Loading transactions…</div>
      </DashboardLayout>
    );
  }

  if (error) {
    return (
      <DashboardLayout pageTitle="Billing">
        <div style={{ padding: 48, textAlign: 'center', color: 'var(--dashboard-danger)' }}>{error}</div>
      </DashboardLayout>
    );
  }

  return (
    <DashboardLayout pageTitle="Billing">
      <div style={{ maxWidth: 720, margin: '0 auto' }}>
        <div
          style={{
            background: 'var(--dashboard-card-bg)',
            borderRadius: 16,
            border: '1px solid var(--dashboard-border)',
            overflow: 'hidden',
          }}
        >
          <div
            style={{
              padding: '24px 28px',
              borderBottom: '1px solid var(--dashboard-border)',
              display: 'flex',
              alignItems: 'center',
              gap: 12,
            }}
          >
            <div
              style={{
                width: 44,
                height: 44,
                borderRadius: 12,
                background: 'linear-gradient(135deg, rgba(59, 130, 246, 0.15) 0%, rgba(139, 92, 246, 0.15) 100%)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              <Receipt size={22} color="var(--dashboard-accent)" />
            </div>
            <div>
              <h2 style={{ fontSize: 18, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 4px 0' }}>
                Transaction history
              </h2>
              <p style={{ fontSize: 13, color: 'var(--dashboard-text-muted)', margin: 0 }}>
                All your past payments and invoices
              </p>
            </div>
          </div>

          <div style={{ padding: '8px 0' }}>
            {transactions.length === 0 ? (
              <div style={{ padding: 32, textAlign: 'center', color: 'var(--dashboard-text-muted)', fontSize: 14 }}>
                No transactions yet
              </div>
            ) : (
              transactions.map((tx, index) => (
                <div
                  key={tx.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '18px 28px',
                    borderBottom: index < transactions.length - 1 ? '1px solid var(--dashboard-border)' : 'none',
                    gap: 16,
                    transition: 'background 120ms ease',
                  }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: 16, minWidth: 0 }}>
                    <div
                      style={{
                        width: 40,
                        height: 40,
                        borderRadius: 10,
                        background: 'var(--dashboard-sidebar-active-bg)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        flexShrink: 0,
                      }}
                    >
                      <Receipt size={18} color="var(--dashboard-text-muted)" />
                    </div>
                    <div style={{ minWidth: 0 }}>
                      <div style={{ fontSize: 15, fontWeight: 500, color: 'var(--dashboard-text)', marginBottom: 2 }}>
                        {tx.description}
                      </div>
                      <div style={{ fontSize: 13, color: 'var(--dashboard-text-muted)' }}>{tx.date}</div>
                    </div>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 16, flexShrink: 0 }}>
                    <span style={{ fontSize: 15, fontWeight: 600, color: 'var(--dashboard-text)' }}>{tx.amount}</span>
                    <span
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 6,
                        fontSize: 12,
                        fontWeight: 600,
                        color: tx.status === 'billed' || tx.status === 'paid' ? 'var(--dashboard-success)' : 'var(--dashboard-text-muted)',
                        background: tx.status === 'billed' || tx.status === 'paid' ? 'rgba(34, 197, 94, 0.12)' : 'var(--dashboard-sidebar-active-bg)',
                        padding: '6px 12px',
                        borderRadius: 8,
                      }}
                    >
                      <CheckCircle size={14} />
                      {tx.status === 'billed' || tx.status === 'paid' ? 'Billed' : tx.status}
                    </span>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </DashboardLayout>
  );
};

export default BillingPage;
