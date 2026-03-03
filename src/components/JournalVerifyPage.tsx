import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { apiFetch } from '../api/config';
import { useAuth } from '../contexts/AuthContext';
import { authService } from '../services/authService';
import { premiumService } from '../services/premiumService';
import './JournalVerifyPage.css';

declare global {
  interface Window {
    Razorpay: any;
  }
}

type VerifyResult = {
  success: boolean;
  is_real: boolean;
  confidence: string;
  summary: string;
  details: string;
  parameters: Record<string, unknown>;
  red_flags?: string[];
  recommendations?: string[];
  error?: string;
};

const JournalVerifyPage: React.FC = () => {
  const { user, isAuthenticated, token, refreshSubscription } = useAuth();
  const navigate = useNavigate();
  const [url, setUrl] = useState('');
  const [doi, setDoi] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<VerifyResult | null>(null);
  const [remainingUses, setRemainingUses] = useState(0);
  const [showPurchaseModal, setShowPurchaseModal] = useState(false);
  const [purchaseLoading, setPurchaseLoading] = useState(false);
  const [syncLoading, setSyncLoading] = useState(false);
  const [paymentSuccess, setPaymentSuccess] = useState(false);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
      return;
    }
    checkJournalCheckAccess();
  }, [isAuthenticated, user, navigate]);

  const checkJournalCheckAccess = async () => {
    if (!user) return;
    try {
      const access = await authService.checkFeatureAccess(user.id, 'journal_check');
      setRemainingUses(access.remaining_uses);
    } catch {
      setRemainingUses(0);
    }
  };

  const handleSyncPayment = async () => {
    if (!token) return;
    setSyncLoading(true);
    setError(null);
    try {
      const res = await apiFetch('/v1/payments/razorpay/sync-entitlements', { method: 'POST' });
      const data = await res.json().catch(() => ({}));
      if (res.ok && data.success) {
        await checkJournalCheckAccess();
        const processed = (data as { orders_processed?: number }).orders_processed ?? 0;
        if (processed > 0) {
          setError(null);
          setPaymentSuccess(true);
          setTimeout(() => setPaymentSuccess(false), 4000);
        }
      }
    } catch {
      setError('Failed to sync. Please try again.');
    } finally {
      setSyncLoading(false);
    }
  };

  const handleVerify = async () => {
    const trimmedUrl = url.trim();
    const trimmedDoi = doi.trim();
    if (!trimmedUrl && !trimmedDoi) {
      setError('Please enter a journal URL or DOI');
      return;
    }
    if (!token) {
      setError('Please log in to verify journals');
      navigate('/login');
      return;
    }
    if (remainingUses <= 0) {
      setShowPurchaseModal(true);
      return;
    }

    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const consumeRes = await apiFetch('/api/dashboard/consume-journal-check', { method: 'POST' });
      if (!consumeRes.ok) {
        const errData = await consumeRes.json().catch(() => ({}));
        if (consumeRes.status === 403) {
          setError('No remaining journal checks. Purchase more or upgrade to Premium.');
          setShowPurchaseModal(true);
          await checkJournalCheckAccess();
          setLoading(false);
          return;
        }
        throw new Error((errData as { error?: string }).error || 'Failed to start verification');
      }

      const verifyRes = await apiFetch('/api/journal-verify', {
        method: 'POST',
        body: JSON.stringify({ url: trimmedUrl || undefined, doi: trimmedDoi || undefined }),
      });
      if (!verifyRes.ok) {
        const errData = await verifyRes.json().catch(() => ({}));
        throw new Error((errData as { error?: string }).error || 'Verification failed');
      }
      const data = (await verifyRes.json()) as VerifyResult;
      setResult(data);
      await checkJournalCheckAccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Verification failed');
    } finally {
      setLoading(false);
    }
  };

  const handlePurchase = async () => {
    if (!user || !isAuthenticated) {
      navigate('/login');
      return;
    }
    if (user.email === 'testadmin@gaply.com') {
      alert('Test Account: Payment bypassed! Refreshing...');
      setShowPurchaseModal(false);
      await refreshSubscription();
      await checkJournalCheckAccess();
      return;
    }

    setPurchaseLoading(true);
    setError(null);
    try {
      const orderResult = await premiumService.createPaymentOrder('PLAN-JOURNAL-CHECK');
      if (!orderResult.success || !orderResult.data) {
        setError(orderResult.error || 'Failed to create payment order');
        setPurchaseLoading(false);
        return;
      }
      const { order, key_id: keyId } = orderResult.data;
      const openRazorpay = () => {
        const options = {
          key: keyId,
          amount: order.amount,
          currency: order.currency,
          name: 'GAPLY',
          description: 'Journal Verification — 2 checks',
          order_id: order.id,
          prefill: { name: `${user.first_name || ''} ${user.last_name || ''}`.trim() || user.email, email: user.email },
          theme: { color: '#007AFF' },
          handler: async function (response: any) {
            try {
              setShowPurchaseModal(false);
              const verifyResult = await premiumService.verifyPayment(
                response.razorpay_order_id,
                response.razorpay_payment_id,
                response.razorpay_signature
              );
              if (verifyResult.success) {
                setError(null);
                setPaymentSuccess(true);
                await refreshSubscription();
                await checkJournalCheckAccess();
                if (user) {
                  const access = await authService.checkFeatureAccess(user.id, 'journal_check');
                  if (access.remaining_uses === 0) {
                    const syncRes = await apiFetch('/v1/payments/razorpay/sync-entitlements', { method: 'POST' });
                    if (syncRes.ok) await checkJournalCheckAccess();
                  }
                }
                setTimeout(() => setPaymentSuccess(false), 4500);
              } else {
                setError(verifyResult.error || 'Payment verification failed');
                setShowPurchaseModal(true);
              }
            } catch {
              setError('Payment verification failed. Use "I paid — refresh access" if payment went through.');
              setShowPurchaseModal(true);
            } finally {
              setPurchaseLoading(false);
            }
          },
          modal: { ondismiss: () => setPurchaseLoading(false) },
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
      setError(err instanceof Error ? err.message : 'Purchase failed');
    } finally {
      setPurchaseLoading(false);
    }
  };

  if (!isAuthenticated || !user) return null;

  return (
    <div className="journal-verify-page">
      <div className="jv-hero">
        <div className="jv-hero-badge">Journal Authenticity</div>
        <h1 className="jv-hero-title">Verify Your Journal is Real</h1>
        <p className="jv-hero-subtitle">
          AI-powered analysis of journal URLs and DOIs. Detect predatory journals and get detailed authenticity reports.
        </p>
      </div>

      <div className="jv-card">
        <div className="jv-usage-badge">
          {remainingUses > 0 ? (
            <span className="jv-usage-ok">{remainingUses} check{remainingUses !== 1 ? 's' : ''} remaining</span>
          ) : (
            <span className="jv-usage-none">No checks remaining — purchase below</span>
          )}
          {remainingUses <= 0 && (
            <button
              type="button"
              className="jv-sync-btn"
              onClick={handleSyncPayment}
              disabled={syncLoading}
            >
              {syncLoading ? 'Syncing…' : 'I paid — refresh access'}
            </button>
          )}
        </div>

        <div className="jv-input-group">
          <label>Journal URL</label>
          <input
            type="url"
            placeholder="https://example.com/journal"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            disabled={loading}
          />
        </div>
        <div className="jv-input-group">
          <label>DOI (optional if URL provided)</label>
          <input
            type="text"
            placeholder="10.1234/example.2024.001"
            value={doi}
            onChange={(e) => setDoi(e.target.value)}
            disabled={loading}
          />
        </div>

        {error && <div className="jv-error">{error}</div>}

        <button
          className="jv-btn jv-btn-primary"
          onClick={handleVerify}
          disabled={loading || (remainingUses <= 0 && !showPurchaseModal)}
        >
          {loading ? (
            <span className="jv-spinner" />
          ) : remainingUses <= 0 ? (
            'Purchase to Verify'
          ) : (
            'Verify Journal'
          )}
        </button>

        {remainingUses <= 0 && (
          <button className="jv-btn jv-btn-secondary" onClick={() => setShowPurchaseModal(true)}>
            Get 2 Checks for ₹99
          </button>
        )}
      </div>

      {result && (
        <div className={`jv-result-card ${result.is_real ? 'jv-result-real' : 'jv-result-fake'}`}>
          <div className="jv-result-header">
            <span className={`jv-verdict-badge ${result.is_real ? 'real' : 'fake'}`}>
              {result.is_real ? '✓ Legitimate' : '⚠ Verify Further'}
            </span>
            <span className={`jv-confidence confidence-${result.confidence}`}>{result.confidence} confidence</span>
          </div>
          <p className="jv-summary">{result.summary}</p>
          <div className="jv-details">{result.details}</div>
          {result.parameters && Object.keys(result.parameters).length > 0 && (
            <div className="jv-parameters">
              <h4>Key Parameters</h4>
              <dl>
                {Object.entries(result.parameters).map(([k, v]) => (
                  <div key={k}>
                    <dt>{k.replace(/_/g, ' ')}</dt>
                    <dd>{String(v)}</dd>
                  </div>
                ))}
              </dl>
            </div>
          )}
          {result.red_flags && result.red_flags.length > 0 && (
            <div className="jv-red-flags">
              <h4>Red Flags</h4>
              <ul>
                {result.red_flags.map((f, i) => (
                  <li key={i}>{f}</li>
                ))}
              </ul>
            </div>
          )}
          {result.recommendations && result.recommendations.length > 0 && (
            <div className="jv-recommendations">
              <h4>Recommendations</h4>
              <ul>
                {result.recommendations.map((r, i) => (
                  <li key={i}>{r}</li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {paymentSuccess && (
        <div className="jv-success-overlay">
          <div className="jv-success-card">
            <div className="jv-success-icon">✓</div>
            <h3>Payment Successful!</h3>
            <p>Enjoy your premium feature — you now have 2 journal verification checks.</p>
            <div className="jv-success-confetti" aria-hidden="true" />
          </div>
        </div>
      )}

      {showPurchaseModal && (
        <div className="jv-modal-overlay" onClick={() => !purchaseLoading && setShowPurchaseModal(false)}>
          <div className="jv-modal" onClick={(e) => e.stopPropagation()}>
            <h3>Get Journal Verification</h3>
            <p>2 journal checks for ₹99. One-time purchase.</p>
            <p className="jv-modal-note">Premium subscribers get 5 free checks included.</p>
            <div className="jv-modal-actions">
              <button className="jv-btn jv-btn-primary" onClick={handlePurchase} disabled={purchaseLoading}>
                {purchaseLoading ? 'Processing…' : 'Pay ₹99'}
              </button>
              <button className="jv-btn jv-btn-ghost" onClick={() => setShowPurchaseModal(false)} disabled={purchaseLoading}>
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default JournalVerifyPage;
