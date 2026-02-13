import React, { useState, useEffect } from 'react';
import { useAuth } from '../contexts/AuthContext';
import { premiumService } from '../services/premiumService';

declare global {
  interface Window {
    Razorpay: any;
  }
}

interface PackageSelectionProps {
  onClose: () => void;
}

const PLANS = [
  {
    id: 'PLAN-PREMIUM',
    name: 'Gaply Premium',
    price: '₹2,499',
    description: 'PublishReady + DataMaestro bundled access with priority-quality outputs',
    features: [
      'PublishReady (2 uses)',
      'DataMaestro (1 use)',
      'Unlimited chat with Gaply.AI in both features',
      'Priority evaluation queue',
      'Valid for 365 days',
    ],
    popular: true,
  },
];

const PackageSelection: React.FC<PackageSelectionProps> = ({ onClose }) => {
  const { user, subscription, isAuthenticated, refreshSubscription } = useAuth();
  const [currentView, setCurrentView] = useState<'plans' | 'account'>('plans');
  const [processing, setProcessing] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    const stored = localStorage.getItem('gaply_theme');
    return stored === 'light' || stored === 'dark' ? stored : 'dark';
  });

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const isLight = theme === 'light';
  const bg = isLight ? '#F2F0EF' : '#000000';
  const cardBg = isLight ? 'rgba(255,255,255,0.9)' : 'rgba(255,255,255,0.05)';
  const text = isLight ? '#0b0b0b' : '#ffffff';
  const muted = isLight ? '#666' : '#888';
  const border = isLight ? 'rgba(15,23,42,0.12)' : 'rgba(255,255,255,0.1)';
  const accent = '#007AFF';

  const handlePackageSelect = async (planId: string) => {
    if (!isAuthenticated || !user) {
      setError('Please log in to purchase a plan');
      window.location.href = '/login';
      return;
    }

    if (user.email === 'testadmin@gaply.com') {
      alert('Test Account: Payment bypassed! Redirecting to premium page...');
      window.location.href = '/premium';
      return;
    }

    try {
      setProcessing(planId);
      setError('');

      const orderResult = await premiumService.createPaymentOrder(planId);
      if (!orderResult.success || !orderResult.data) {
        setError(orderResult.error || 'Failed to create payment order');
        return;
      }

      const { order, key_id: keyId } = orderResult.data;

      const openRazorpay = () => {
        const options = {
          key: keyId,
          amount: order.amount,
          currency: order.currency,
          name: 'GAPLY',
          description: `Premium Plan - ${order.receipt}`,
          order_id: order.id,
          prefill: {
            name: user ? `${user.first_name || ''} ${user.last_name || ''}`.trim() || user.email : '',
            email: user?.email || '',
          },
          theme: { color: accent },
          handler: async function (response: any) {
            try {
              const verifyResult = await premiumService.verifyPayment(
                response.razorpay_order_id,
                response.razorpay_payment_id,
                response.razorpay_signature
              );
              if (verifyResult.success) {
                await refreshSubscription();
                alert('Payment successful! Your premium features are now active.');
                window.location.reload();
              } else {
                setError(verifyResult.error || 'Payment verification failed');
              }
            } catch (err) {
              setError('Payment verification failed. Please contact support.');
            } finally {
              setProcessing(null);
            }
          },
          modal: { ondismiss: () => setProcessing(null) },
        };
        const rzp = new window.Razorpay(options);
        rzp.open();
      };

      if (!window.Razorpay) {
        const script = document.createElement('script');
        script.src = 'https://checkout.razorpay.com/v1/checkout.js';
        script.onload = openRazorpay;
        script.onerror = () => setError('Failed to load payment gateway');
        document.head.appendChild(script);
      } else {
        openRazorpay();
      }
    } catch (err) {
      setError('An error occurred. Please try again.');
      setProcessing(null);
    }
  };

  const currentPlanName = subscription?.package_name || 'No active plan';
  const gapFinderRemaining = subscription?.remaining_uses?.gap_finder ?? 0;
  const deepEvalRemaining = subscription?.remaining_uses?.deep_eval ?? 0;

  return (
    <div
      style={{
        position: 'fixed',
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        background: bg,
        zIndex: 1000,
        fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif',
        overflow: 'auto',
      }}
    >
      <div style={{ position: 'relative', zIndex: 2, minHeight: '100vh', padding: '20px' }}>
        {/* Header */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: '30px',
            paddingBottom: '20px',
            borderBottom: `1px solid ${border}`,
          }}
        >
          <button
            onClick={onClose}
            style={{
              background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
              border: 'none',
              borderRadius: '8px',
              padding: '8px 16px',
              cursor: 'pointer',
              fontSize: '14px',
              fontWeight: '500',
              color: text,
            }}
          >
            ← Back
          </button>

          <div style={{ textAlign: 'center' }}>
            <h1 style={{ fontSize: '32px', fontWeight: '600', color: text, margin: '0 0 4px 0' }}>
              Choose your plan.
            </h1>
            <p style={{ fontSize: '16px', color: muted, margin: 0 }}>
              Select the research plan that fits your needs.
            </p>
          </div>

          <button
            onClick={() => {
              const next = theme === 'dark' ? 'light' : 'dark';
              setTheme(next);
              localStorage.setItem('gaply_theme', next);
              document.documentElement.setAttribute('data-theme', next);
            }}
            style={{
              background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
              border: 'none',
              borderRadius: '8px',
              padding: '8px 16px',
              cursor: 'pointer',
              fontSize: '14px',
              color: text,
            }}
          >
            {theme === 'dark' ? 'Light Mode' : 'Dark Mode'}
          </button>
        </div>

        {/* View Toggle */}
        <div style={{ display: 'flex', justifyContent: 'center', marginBottom: '30px' }}>
          <div
            style={{
              background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
              borderRadius: '12px',
              padding: '4px',
              display: 'flex',
              gap: '4px',
            }}
          >
            <button
              onClick={() => setCurrentView('plans')}
              style={{
                background: currentView === 'plans' ? accent : 'transparent',
                color: currentView === 'plans' ? 'white' : text,
                border: 'none',
                borderRadius: '8px',
                padding: '8px 20px',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
              }}
            >
              Plans
            </button>
            <button
              onClick={() => setCurrentView('account')}
              style={{
                background: currentView === 'account' ? accent : 'transparent',
                color: currentView === 'account' ? 'white' : text,
                border: 'none',
                borderRadius: '8px',
                padding: '8px 20px',
                fontSize: '14px',
                fontWeight: '500',
                cursor: 'pointer',
              }}
            >
              My Account
            </button>
          </div>
        </div>

        {error && (
          <div
            style={{
              background: 'rgba(255,59,48,0.15)',
              color: '#ff3b30',
              padding: '12px 16px',
              borderRadius: '8px',
              marginBottom: '20px',
              textAlign: 'center',
            }}
          >
            {error}
          </div>
        )}

        {/* Plans View */}
        {currentView === 'plans' && (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
              gap: '20px',
              maxWidth: '700px',
              margin: '0 auto 30px',
            }}
          >
            {PLANS.map((pkg) => (
              <div
                key={pkg.id}
                style={{
                  background: cardBg,
                  borderRadius: '16px',
                  padding: '24px',
                  border: pkg.popular ? `2px solid ${accent}` : `1px solid ${border}`,
                  position: 'relative',
                  transition: 'all 0.3s ease',
                  cursor: processing ? 'not-allowed' : 'pointer',
                  opacity: processing ? 0.7 : 1,
                }}
                onClick={() => !processing && handlePackageSelect(pkg.id)}
              >
                {pkg.popular && (
                  <div
                    style={{
                      position: 'absolute',
                      top: '-12px',
                      left: '50%',
                      transform: 'translateX(-50%)',
                      background: accent,
                      color: 'white',
                      padding: '6px 16px',
                      borderRadius: '16px',
                      fontSize: '12px',
                      fontWeight: '600',
                    }}
                  >
                    Most Popular
                  </div>
                )}

                <h3 style={{ fontSize: '20px', fontWeight: '600', color: text, margin: '0 0 8px 0' }}>
                  {pkg.name}
                </h3>
                <div style={{ fontSize: '32px', fontWeight: '700', color: accent, margin: '0 0 12px 0' }}>
                  {pkg.price}
                </div>
                <p style={{ fontSize: '14px', color: muted, margin: '0 0 20px 0', lineHeight: 1.4 }}>
                  {pkg.description}
                </p>
                <ul style={{ listStyle: 'none', padding: 0, margin: '0 0 20px 0' }}>
                  {pkg.features.map((feature, i) => (
                    <li
                      key={i}
                      style={{
                        fontSize: '13px',
                        color: muted,
                        margin: '0 0 8px 0',
                        display: 'flex',
                        alignItems: 'center',
                        gap: '8px',
                      }}
                    >
                      <span style={{ color: '#30D158', fontWeight: '600' }}>✓</span>
                      {feature}
                    </li>
                  ))}
                </ul>
                <button
                  disabled={!!processing}
                  style={{
                    width: '100%',
                    background: pkg.popular ? accent : (isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)'),
                    color: pkg.popular ? 'white' : text,
                    border: 'none',
                    borderRadius: '8px',
                    padding: '12px 20px',
                    fontSize: '14px',
                    fontWeight: '600',
                    cursor: processing ? 'not-allowed' : 'pointer',
                  }}
                >
                  {processing === pkg.id ? 'Processing...' : `Get ${pkg.name}`}
                </button>
              </div>
            ))}
          </div>
        )}

        {/* Account View - Current Plan */}
        {currentView === 'account' && (
          <div
            style={{
              maxWidth: '500px',
              margin: '0 auto',
              background: cardBg,
              borderRadius: '16px',
              padding: '30px',
              border: `1px solid ${border}`,
            }}
          >
            <h2 style={{ fontSize: '24px', fontWeight: '600', color: text, margin: '0 0 20px 0' }}>
              Current Plan
            </h2>
            <div
              style={{
                background: `${accent}20`,
                borderRadius: '12px',
                padding: '20px',
                marginBottom: '20px',
                border: `1px solid ${accent}40`,
              }}
            >
              <h3 style={{ fontSize: '18px', fontWeight: '600', color: accent, margin: '0 0 8px 0' }}>
                {currentPlanName}
              </h3>
              {subscription?.expires_at && (
                <p style={{ fontSize: '14px', color: muted, margin: 0 }}>
                  Valid until: {new Date(subscription.expires_at).toLocaleDateString()}
                </p>
              )}
            </div>

            <h3 style={{ fontSize: '20px', fontWeight: '600', color: text, margin: '0 0 16px 0' }}>
              Usage
            </h3>
            <div style={{ marginBottom: '16px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                <span style={{ fontSize: '14px', color: text }}>PublishReady (Gap Finder)</span>
                <span style={{ fontSize: '14px', color: muted }}>{gapFinderRemaining} uses left</span>
              </div>
              <div
                style={{
                  background: isLight ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.1)',
                  borderRadius: '8px',
                  height: '6px',
                  overflow: 'hidden',
                }}
              >
                <div
                  style={{
                    background: `linear-gradient(90deg, ${accent}, #30D158)`,
                    height: '100%',
                    width: `${Math.min(100, (gapFinderRemaining / 2) * 100)}%`,
                  }}
                />
              </div>
            </div>
            <div style={{ marginBottom: '20px' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '4px' }}>
                <span style={{ fontSize: '14px', color: text }}>DataMaestro (Deep Analysis)</span>
                <span style={{ fontSize: '14px', color: muted }}>{deepEvalRemaining} uses left</span>
              </div>
              <div
                style={{
                  background: isLight ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.1)',
                  borderRadius: '8px',
                  height: '6px',
                  overflow: 'hidden',
                }}
              >
                <div
                  style={{
                    background: `linear-gradient(90deg, ${accent}, #30D158)`,
                    height: '100%',
                    width: `${Math.min(100, deepEvalRemaining * 100)}%`,
                  }}
                />
              </div>
            </div>

            <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
              <button
                onClick={() => (window.location.href = '/premium')}
                style={{
                  background: accent,
                  color: 'white',
                  border: 'none',
                  borderRadius: '8px',
                  padding: '12px 20px',
                  fontSize: '14px',
                  fontWeight: '600',
                  cursor: 'pointer',
                  flex: 1,
                  minWidth: '120px',
                }}
              >
                View Dashboard
              </button>
              <button
                onClick={() => setCurrentView('plans')}
                style={{
                  background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
                  color: text,
                  border: 'none',
                  borderRadius: '8px',
                  padding: '12px 20px',
                  fontSize: '14px',
                  fontWeight: '600',
                  cursor: 'pointer',
                  flex: 1,
                  minWidth: '120px',
                }}
              >
                Upgrade Package
              </button>
            </div>
          </div>
        )}

        {/* Footer */}
        <div
          style={{
            textAlign: 'center',
            marginTop: '30px',
            paddingTop: '20px',
            borderTop: `1px solid ${border}`,
          }}
        >
          <p style={{ fontSize: '12px', color: muted, margin: '0 0 4px 0' }}>
            All plans include 30-day money-back guarantee
          </p>
          <p style={{ fontSize: '12px', color: muted, margin: 0 }}>
            Need help choosing? Contact our support team
          </p>
        </div>
      </div>
    </div>
  );
};

export default PackageSelection;
