import React from 'react';
import { useNavigate } from 'react-router-dom';
import { usePremiumSession } from '../hooks/usePremiumSession';

interface PremiumFeatureGuardProps {
  featureKey: string;
  children: React.ReactNode;
}

/**
 * Wraps premium feature content. Shows upgrade modal when access=false.
 * Renders children when access=true.
 */
export const PremiumFeatureGuard: React.FC<PremiumFeatureGuardProps> = ({
  featureKey,
  children,
}) => {
  const navigate = useNavigate();
  const { access, creditsRemaining, loading } = usePremiumSession(featureKey);

  if (loading) {
    return (
      <div style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        minHeight: '40vh',
        color: 'var(--section-text)',
      }}>
        Loading...
      </div>
    );
  }

  if (!access && !loading) {
    return (
      <div style={{
        minHeight: '60vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '40px',
      }}>
        <div style={{
          maxWidth: '480px',
          textAlign: 'center',
          background: 'var(--card-bg)',
          borderRadius: '24px',
          padding: '48px',
          border: '1px solid var(--card-border)',
        }}>
          <h2 style={{ fontSize: '1.5rem', fontWeight: '600', marginBottom: '16px' }}>
            Premium Pro Required
          </h2>
          <p style={{ color: 'var(--muted-text)', marginBottom: '24px' }}>
            Upgrade to Premium Pro to access this feature. Credits never expire.
          </p>
          <button
            onClick={() => navigate('/pricing')}
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
            View Pricing
          </button>
        </div>
      </div>
    );
  }

  return (
    <>
      {creditsRemaining >= 0 && (
        <div style={{
          padding: '8px 16px',
          marginBottom: '8px',
          fontSize: '0.9rem',
          color: 'var(--muted-text)',
          textAlign: 'right',
        }}>
          {creditsRemaining} credit{creditsRemaining !== 1 ? 's' : ''} remaining
        </div>
      )}
      {children}
    </>
  );
};
