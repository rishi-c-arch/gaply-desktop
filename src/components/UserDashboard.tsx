import React, { useState, useEffect } from 'react';
import { buildApiUrl } from '../api/config';

interface UserAccount {
  planId: string;
  planName: string;
  gapFinderUsesRemaining: number;
  deepEvalUsesRemaining: number;
  hasSupport: boolean;
  expiresAt: string;
  createdAt: string;
}

interface PaymentHistory {
  id: string;
  package_name: string;
  amount: number;
  status: string;
  created_at: string;
}

const UserDashboard: React.FC = () => {
  const [userAccount, setUserAccount] = useState<UserAccount | null>(null);
  const [paymentHistory, setPaymentHistory] = useState<PaymentHistory[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'overview' | 'usage' | 'payments'>('overview');

  useEffect(() => {
    loadUserData();
  }, []);

  const loadUserData = async () => {
    const token = localStorage.getItem('gaply_token');
    if (!token) {
      window.location.href = '/login';
      return;
    }

    try {
      // Load user account
      const accountResponse = await fetch(buildApiUrl('/api/v1/user/account'), {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        }
      });

      if (accountResponse.ok) {
        const accountData = await accountResponse.json();
        setUserAccount(accountData);
      }

      // Load payment history
      const paymentsResponse = await fetch(buildApiUrl('/api/v1/payment/history'), {
        headers: {
          'Authorization': `Bearer ${token}`,
          'Content-Type': 'application/json'
        }
      });

      if (paymentsResponse.ok) {
        const paymentsData = await paymentsResponse.json();
        setPaymentHistory(paymentsData.data || []);
      }
    } catch (error) {
      console.error('Failed to load user data:', error);
    } finally {
      setLoading(false);
    }
  };

  const handleLogout = () => {
    localStorage.removeItem('gaply_token');
    localStorage.removeItem('user');
    window.location.href = '/';
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleDateString('en-IN', {
      year: 'numeric',
      month: 'long',
      day: 'numeric'
    });
  };

  const formatCurrency = (amount: number) => {
    return new Intl.NumberFormat('en-IN', {
      style: 'currency',
      currency: 'INR'
    }).format(amount);
  };

  if (loading) {
    return (
      <div style={{ 
        display: 'flex', 
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        background: '#0A0A0A',
        color: '#cecece'
      }}>
        <div>Loading...</div>
      </div>
    );
  }

  return (
    <div style={{
      minHeight: '100vh',
      background: 'linear-gradient(135deg, #0A0A0A 0%, #1a1a1a 100%)',
      color: '#cecece',
      padding: '80px 20px'
    }}>
      <div style={{ maxWidth: '1200px', margin: '0 auto' }}>
        {/* Header */}
        <div style={{ 
          display: 'flex', 
          justifyContent: 'space-between', 
          alignItems: 'center',
          marginBottom: '40px'
        }}>
          <h1 style={{ 
            fontSize: '2.5rem', 
            fontWeight: 'bold',
            background: 'linear-gradient(45deg, #ff7a1a, #ffffff)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            margin: 0
          }}>
            Dashboard
          </h1>
          <button
            onClick={handleLogout}
            style={{
              background: 'transparent',
              color: '#cecece',
              border: '1px solid #cecece',
              padding: '10px 20px',
              borderRadius: '20px',
              fontSize: '1rem',
              cursor: 'pointer',
              transition: 'all 0.3s ease'
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
            }}
          >
            Logout
          </button>
        </div>

        {/* Navigation Tabs */}
        <div style={{
          display: 'flex',
          gap: '20px',
          marginBottom: '40px',
          borderBottom: '1px solid rgba(255, 255, 255, 0.1)'
        }}>
          {[
            { id: 'overview', label: 'Overview' },
            { id: 'usage', label: 'Usage' },
            { id: 'payments', label: 'Payments' }
          ].map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as any)}
              style={{
                background: 'transparent',
                color: activeTab === tab.id ? '#ff7a1a' : '#cecece',
                border: 'none',
                padding: '15px 25px',
                fontSize: '1rem',
                fontWeight: '600',
                cursor: 'pointer',
                borderBottom: activeTab === tab.id ? '2px solid #ff7a1a' : '2px solid transparent',
                transition: 'all 0.3s ease'
              }}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Overview Tab */}
        {activeTab === 'overview' && userAccount && (
          <div>
            <div style={{
              background: 'rgba(255, 255, 255, 0.05)',
              borderRadius: '20px',
              padding: '30px',
              marginBottom: '30px'
            }}>
              <h2 style={{ color: '#ff7a1a', marginBottom: '20px' }}>Current Plan</h2>
              <div style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
                gap: '20px'
              }}>
                <div>
                  <h3 style={{ margin: '0 0 10px 0' }}>{userAccount.planName}</h3>
                  <p style={{ opacity: 0.7, margin: 0 }}>
                    Plan ID: {userAccount.planId}
                  </p>
                </div>
                <div>
                  <h4 style={{ margin: '0 0 10px 0' }}>Expires</h4>
                  <p style={{ opacity: 0.7, margin: 0 }}>
                    {formatDate(userAccount.expiresAt)}
                  </p>
                </div>
                <div>
                  <h4 style={{ margin: '0 0 10px 0' }}>Support</h4>
                  <p style={{ opacity: 0.7, margin: 0 }}>
                    {userAccount.hasSupport ? '✅ Included' : '❌ Not included'}
                  </p>
                </div>
              </div>
            </div>

            <div style={{
              background: 'rgba(255, 255, 255, 0.05)',
              borderRadius: '20px',
              padding: '30px'
            }}>
              <h2 style={{ color: '#ff7a1a', marginBottom: '20px' }}>Quick Actions</h2>
              <div style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
                gap: '20px'
              }}>
                <button
                  onClick={() => window.location.href = '/premium'}
                  style={{
                    background: 'linear-gradient(45deg, #ff7a1a, #ff9500)',
                    color: 'white',
                    border: 'none',
                    padding: '15px',
                    borderRadius: '15px',
                    fontSize: '1rem',
                    fontWeight: '600',
                    cursor: 'pointer',
                    transition: 'all 0.3s ease'
                  }}
                  onMouseEnter={(e) => e.currentTarget.style.transform = 'translateY(-2px)'}
                  onMouseLeave={(e) => e.currentTarget.style.transform = 'translateY(0)'}
                >
                  Upgrade Plan
                </button>
                <button
                  onClick={() => window.location.href = '/contact'}
                  style={{
                    background: 'transparent',
                    color: '#cecece',
                    border: '1px solid #cecece',
                    padding: '15px',
                    borderRadius: '15px',
                    fontSize: '1rem',
                    fontWeight: '600',
                    cursor: 'pointer',
                    transition: 'all 0.3s ease'
                  }}
                  onMouseEnter={(e) => {
                    e.currentTarget.style.background = 'rgba(255, 255, 255, 0.1)';
                  }}
                  onMouseLeave={(e) => {
                    e.currentTarget.style.background = 'transparent';
                  }}
                >
                  Contact Support
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Usage Tab */}
        {activeTab === 'usage' && userAccount && (
          <div style={{
            background: 'rgba(255, 255, 255, 0.05)',
            borderRadius: '20px',
            padding: '30px'
          }}>
            <h2 style={{ color: '#ff7a1a', marginBottom: '30px' }}>Feature Usage</h2>
            
            <div style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
              gap: '30px'
            }}>
              <div style={{
                background: 'rgba(255, 122, 26, 0.1)',
                border: '1px solid rgba(255, 122, 26, 0.3)',
                borderRadius: '15px',
                padding: '25px',
                textAlign: 'center'
              }}>
                <h3 style={{ color: '#ff7a1a', marginBottom: '15px' }}>
                  🔍 Gap Finder
                </h3>
                <div style={{ fontSize: '2rem', fontWeight: 'bold', marginBottom: '10px' }}>
                  {userAccount.gapFinderUsesRemaining}
                </div>
                <p style={{ opacity: 0.7, margin: 0 }}>
                  uses remaining
                </p>
              </div>

              <div style={{
                background: 'rgba(255, 122, 26, 0.1)',
                border: '1px solid rgba(255, 122, 26, 0.3)',
                borderRadius: '15px',
                padding: '25px',
                textAlign: 'center'
              }}>
                <h3 style={{ color: '#ff7a1a', marginBottom: '15px' }}>
                  📊 Deep Evaluation
                </h3>
                <div style={{ fontSize: '2rem', fontWeight: 'bold', marginBottom: '10px' }}>
                  {userAccount.deepEvalUsesRemaining}
                </div>
                <p style={{ opacity: 0.7, margin: 0 }}>
                  uses remaining
                </p>
              </div>
            </div>

            <div style={{ marginTop: '30px', textAlign: 'center' }}>
              <p style={{ opacity: 0.7 }}>
                Need more uses? <a href="/premium" style={{ color: '#ff7a1a' }}>Upgrade your plan</a>
              </p>
            </div>
          </div>
        )}

        {/* Payments Tab */}
        {activeTab === 'payments' && (
          <div style={{
            background: 'rgba(255, 255, 255, 0.05)',
            borderRadius: '20px',
            padding: '30px'
          }}>
            <h2 style={{ color: '#ff7a1a', marginBottom: '30px' }}>Payment History</h2>
            
            {paymentHistory.length === 0 ? (
              <div style={{ textAlign: 'center', opacity: 0.7 }}>
                <p>No payment history found.</p>
              </div>
            ) : (
              <div style={{ overflowX: 'auto' }}>
                <table style={{ width: '100%', borderCollapse: 'collapse' }}>
                  <thead>
                    <tr style={{ borderBottom: '1px solid rgba(255, 255, 255, 0.1)' }}>
                      <th style={{ padding: '15px', textAlign: 'left' }}>Plan</th>
                      <th style={{ padding: '15px', textAlign: 'left' }}>Amount</th>
                      <th style={{ padding: '15px', textAlign: 'left' }}>Status</th>
                      <th style={{ padding: '15px', textAlign: 'left' }}>Date</th>
                    </tr>
                  </thead>
                  <tbody>
                    {paymentHistory.map((payment) => (
                      <tr key={payment.id} style={{ borderBottom: '1px solid rgba(255, 255, 255, 0.05)' }}>
                        <td style={{ padding: '15px' }}>{payment.package_name}</td>
                        <td style={{ padding: '15px' }}>{formatCurrency(payment.amount)}</td>
                        <td style={{ padding: '15px' }}>
                          <span style={{
                            background: payment.status === 'success' ? 'rgba(34, 197, 94, 0.2)' : 'rgba(239, 68, 68, 0.2)',
                            color: payment.status === 'success' ? '#22c55e' : '#ef4444',
                            padding: '5px 10px',
                            borderRadius: '10px',
                            fontSize: '0.9rem'
                          }}>
                            {payment.status}
                          </span>
                        </td>
                        <td style={{ padding: '15px' }}>{formatDate(payment.created_at)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default UserDashboard;
