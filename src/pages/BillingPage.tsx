import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import DashboardLayout from '../components/Layout/DashboardLayout';
import { fetchBillingTransactions } from '../services/dashboardService';
import { useAuth } from '../contexts/AuthContext';

const BillingPage: React.FC = () => {
  const { isAuthenticated, user, token } = useAuth();
  const navigate = useNavigate();
  const [transactions, setTransactions] = useState<Awaited<ReturnType<typeof fetchBillingTransactions>>>([]);
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

  return (
    <DashboardLayout pageTitle="Billing">
      <div style={{ maxWidth: 720, margin: '0 auto' }}>
        <div
          style={{
            background: 'var(--dashboard-card-bg)',
            borderRadius: 12,
            border: '1px solid var(--dashboard-border)',
            padding: 24,
          }}
        >
          <h2 style={{ fontSize: 18, fontWeight: 600, color: 'var(--dashboard-text)', margin: '0 0 16px 0' }}>
            Billing history
          </h2>
          {loading && (
            <p style={{ color: 'var(--dashboard-text-muted)', margin: 0 }}>Loading transactions…</p>
          )}
          {error && !loading && (
            <p style={{ color: 'var(--dashboard-danger)', margin: 0 }}>{error}</p>
          )}
          {!loading && !error && transactions.length === 0 && (
            <p style={{ color: 'var(--dashboard-text-muted)', margin: 0 }}>
              No transactions yet. When you upgrade or renew, they will appear here.
            </p>
          )}
          {!loading && !error && transactions.length > 0 && (
            <ul style={{ listStyle: 'none', padding: 0, margin: 0 }}>
              {transactions.map((tx) => (
                <li
                  key={tx.id}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    padding: '12px 0',
                    borderBottom: '1px solid var(--dashboard-border)',
                    gap: 16,
                  }}
                >
                  <div>
                    <div style={{ fontWeight: 500, color: 'var(--dashboard-text)' }}>{tx.description}</div>
                    <div style={{ fontSize: 13, color: 'var(--dashboard-text-muted)' }}>{tx.date}</div>
                  </div>
                  <div style={{ textAlign: 'right' }}>
                    <div style={{ fontWeight: 600, color: 'var(--dashboard-text)' }}>{tx.amount}</div>
                    <span
                      style={{
                        fontSize: 12,
                        padding: '2px 8px',
                        borderRadius: 6,
                        background:
                          tx.status === 'completed' || tx.status === 'paid' || tx.status === 'billed'
                            ? 'rgba(34, 197, 94, 0.2)'
                            : tx.status === 'pending'
                            ? 'rgba(234, 179, 8, 0.2)'
                            : 'rgba(239, 68, 68, 0.2)',
                        color:
                          tx.status === 'completed' || tx.status === 'paid' || tx.status === 'billed'
                            ? 'var(--dashboard-success)'
                            : tx.status === 'pending'
                            ? '#eab308'
                            : 'var(--dashboard-danger)',
                      }}
                    >
                      {tx.status}
                    </span>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </DashboardLayout>
  );
};

export default BillingPage;
