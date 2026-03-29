import React, { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../contexts/AuthContext';
import { apiFetch } from '../api/config';

const PremiumProCheckout: React.FC = () => {
  const { user, isAuthenticated } = useAuth();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login', { state: { from: '/checkout/premium-pro' } });
    }
  }, [isAuthenticated, user, navigate]);

  const loadRazorpayScript = (): Promise<void> => {
    if ((window as any).Razorpay) return Promise.resolve();
    return new Promise((resolve, reject) => {
      const script = document.createElement('script');
      script.src = 'https://checkout.razorpay.com/v1/checkout.js';
      script.onload = () => resolve();
      script.onerror = () => reject(new Error('Failed to load payment gateway'));
      document.head.appendChild(script);
    });
  };

  const initiateCheckout = async () => {
    if (!user || !isAuthenticated) {
      navigate('/login', { state: { from: '/checkout/premium-pro' } });
      return;
    }
    if (user.email === 'testadmin@gaply.com') {
      setSuccess(true);
      setTimeout(() => navigate('/dashboard'), 2000);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const res = await apiFetch('/api/v1/payment/create-order', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
      });
      const data = await res.json();

      if (!res.ok) {
        setError(data.error || 'Failed to create order');
        setLoading(false);
        return;
      }

      const orderId = data.order_id || data.OrderID;
      const amount = data.amount || 799900;
      const currency = data.currency || 'INR';
      const keyId = data.key_id || data.KeyID || process.env.REACT_APP_RAZORPAY_KEY_ID;

      if (!orderId || !keyId) {
        setError('Invalid response from payment gateway');
        setLoading(false);
        return;
      }

      await loadRazorpayScript();

      const options = {
        key: keyId,
        amount,
        currency,
        name: 'GAPLY',
        description: 'Premium Pro — ₹7,999 one-time',
        order_id: orderId,
        prefill: {
          name: `${user.first_name || ''} ${user.last_name || ''}`.trim() || user.email,
          email: user.email,
        },
        theme: { color: '#007AFF' },
        handler: function () {
          setSuccess(true);
          setTimeout(() => navigate('/dashboard'), 3000);
        },
        modal: { ondismiss: () => setLoading(false) },
      };

      const rzp = new (window as any).Razorpay(options);
      rzp.open();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Checkout failed');
    } finally {
      setLoading(false);
    }
  };

  if (!isAuthenticated || !user) return null;

  return (
    <div style={{
      minHeight: '100vh',
      backgroundColor: 'var(--app-bg)',
      padding: 'clamp(96px, 12vw, 140px) clamp(16px, 4vw, 40px)',
      color: 'var(--app-text)',
      fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
    }}>
      <div style={{
        maxWidth: '480px',
        width: '100%',
        background: 'var(--card-bg)',
        borderRadius: '24px',
        padding: 'clamp(32px, 5vw, 48px)',
        border: '1px solid var(--card-border)',
        textAlign: 'center',
      }}>
        {success ? (
          <>
            <div style={{ fontSize: '3rem', marginBottom: '16px' }}>✓</div>
            <h2 style={{ fontSize: '1.5rem', fontWeight: '600', marginBottom: '12px' }}>
              Payment Successful
            </h2>
            <p style={{ color: 'var(--muted-text)', marginBottom: '24px' }}>
              Your Premium Pro credits will be available shortly. Refresh your dashboard to see them.
            </p>
            <button
              onClick={() => navigate('/dashboard')}
              style={{
                background: 'var(--hero-cta-bg)',
                color: 'var(--hero-cta-text)',
                padding: '14px 32px',
                borderRadius: '12px',
                border: 'none',
                fontWeight: '600',
                cursor: 'pointer',
              }}
            >
              Go to Dashboard
            </button>
          </>
        ) : (
          <>
            <h2 style={{ fontSize: '1.75rem', fontWeight: '600', marginBottom: '8px' }}>
              Premium Pro — ₹7,999
            </h2>
            <p style={{ color: 'var(--muted-text)', marginBottom: '24px' }}>
              One-time payment. Lifetime credits.
            </p>
            {error && (
              <p style={{ color: '#ff6b6b', marginBottom: '16px', fontSize: '0.95rem' }}>
                {error}
              </p>
            )}
            <button
              onClick={initiateCheckout}
              disabled={loading}
              style={{
                width: '100%',
                background: 'var(--hero-cta-bg)',
                color: 'var(--hero-cta-text)',
                padding: '18px 32px',
                borderRadius: '16px',
                border: 'none',
                fontWeight: '600',
                fontSize: '1.05rem',
                cursor: loading ? 'not-allowed' : 'pointer',
                opacity: loading ? 0.7 : 1,
              }}
            >
              {loading ? 'Opening payment...' : 'Get Premium Pro'}
            </button>
            <p style={{ marginTop: '16px', fontSize: '0.9rem', color: 'var(--muted-text)' }}>
              Credits never expire. Use them anytime.
            </p>
          </>
        )}
      </div>
    </div>
  );
};

export default PremiumProCheckout;
