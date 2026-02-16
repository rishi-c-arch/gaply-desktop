import React, { useState, useEffect } from 'react';
import { useAuth } from '../contexts/AuthContext';
import { useNavigate } from 'react-router-dom';

const UserDashboard: React.FC = () => {
  const { user, subscription, isAuthenticated } = useAuth();
  const navigate = useNavigate();
  const [theme, setTheme] = useState<'light' | 'dark'>(() => {
    const stored = localStorage.getItem('gaply_theme');
    return stored === 'light' || stored === 'dark' ? stored : 'light';
  });

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    if (!isAuthenticated || !user) {
      navigate('/login');
      return;
    }
  }, [isAuthenticated, user, navigate]);

  const isLight = theme === 'light';
  const bg = isLight ? '#F8F9FA' : '#0a0a0a';
  const cardBg = isLight ? '#ffffff' : 'rgba(255,255,255,0.04)';
  const text = isLight ? '#1a1a1a' : '#ffffff';
  const muted = isLight ? '#6b7280' : '#9ca3af';
  const border = isLight ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.08)';
  const accent = '#007AFF';
  const accentGreen = '#10B981';

  const gapFinderRemaining = subscription?.remaining_uses?.gap_finder ?? 0;
  const deepEvalRemaining = subscription?.remaining_uses?.deep_eval ?? 0;
  const currentPlanName = subscription?.package_name || 'No active plan';
  const maxGapFinder = 2;
  const maxDeepEval = 1;

  const toggleTheme = () => {
    const next = theme === 'dark' ? 'light' : 'dark';
    setTheme(next);
    localStorage.setItem('gaply_theme', next);
  };

  if (!isAuthenticated || !user) {
    return null;
  }

  return (
    <div
      style={{
        minHeight: '100vh',
        background: bg,
        fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
        transition: 'background 0.3s ease',
      }}
    >
      {/* Header */}
      <header
        style={{
          position: 'sticky',
          top: 0,
          zIndex: 50,
          background: isLight ? 'rgba(255,255,255,0.9)' : 'rgba(10,10,10,0.9)',
          backdropFilter: 'blur(12px)',
          borderBottom: `1px solid ${border}`,
          padding: '16px 24px',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '24px' }}>
          <button
            onClick={() => navigate('/packages')}
            style={{
              background: 'transparent',
              border: 'none',
              color: muted,
              fontSize: '14px',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
            }}
          >
            ← Back to Plans
          </button>
          <div>
            <h1 style={{ fontSize: '20px', fontWeight: '600', color: text, margin: 0 }}>
              My Account
            </h1>
            <p style={{ fontSize: '13px', color: muted, margin: '2px 0 0 0' }}>
              {user.email}
            </p>
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div
            style={{
              background: `${accent}15`,
              color: accent,
              padding: '6px 12px',
              borderRadius: '8px',
              fontSize: '13px',
              fontWeight: '600',
            }}
          >
            {currentPlanName}
          </div>
          <button
            onClick={toggleTheme}
            style={{
              background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
              border: 'none',
              borderRadius: '8px',
              padding: '8px 14px',
              fontSize: '13px',
              color: text,
              cursor: 'pointer',
            }}
          >
            {theme === 'dark' ? '☀️ Light' : '🌙 Dark'}
          </button>
        </div>
      </header>

      {/* Main Content */}
      <main style={{ maxWidth: '900px', margin: '0 auto', padding: '40px 24px' }}>
        <section style={{ marginBottom: '40px' }}>
          <h2
            style={{
              fontSize: '15px',
              fontWeight: '600',
              color: muted,
              textTransform: 'uppercase',
              letterSpacing: '0.05em',
              margin: '0 0 20px 0',
            }}
          >
            Premium Features
          </h2>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
              gap: '24px',
            }}
          >
            {/* PublishReady Card */}
            <div
              style={{
                background: cardBg,
                borderRadius: '20px',
                padding: '28px',
                border: `1px solid ${border}`,
                boxShadow: isLight ? '0 1px 3px rgba(0,0,0,0.06)' : 'none',
                transition: 'all 0.2s ease',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  justifyContent: 'space-between',
                  marginBottom: '20px',
                }}
              >
                <div>
                  <div
                    style={{
                      width: '48px',
                      height: '48px',
                      borderRadius: '12px',
                      background: 'linear-gradient(135deg, #007AFF 0%, #5856D6 100%)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: '24px',
                      marginBottom: '12px',
                    }}
                  >
                    📄
                  </div>
                  <h3 style={{ fontSize: '20px', fontWeight: '600', color: text, margin: '0 0 4px 0' }}>
                    PublishReady
                  </h3>
                  <p style={{ fontSize: '14px', color: muted, margin: 0, lineHeight: 1.4 }}>
                    Research gap finder & journal-fit analysis
                  </p>
                </div>
                <span
                  style={{
                    fontSize: '24px',
                    fontWeight: '700',
                    color: accent,
                  }}
                >
                  {gapFinderRemaining}/{maxGapFinder}
                </span>
              </div>
              <div style={{ marginBottom: '20px' }}>
                <div
                  style={{
                    height: '8px',
                    borderRadius: '4px',
                    background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
                    overflow: 'hidden',
                  }}
                >
                  <div
                    style={{
                      height: '100%',
                      width: `${(gapFinderRemaining / maxGapFinder) * 100}%`,
                      background: `linear-gradient(90deg, ${accent}, #5856D6)`,
                      borderRadius: '4px',
                      transition: 'width 0.4s ease',
                    }}
                  />
                </div>
                <p style={{ fontSize: '12px', color: muted, margin: '6px 0 0 0' }}>
                  uses remaining
                </p>
              </div>
              <button
                onClick={() => navigate(gapFinderRemaining > 0 ? '/final-orchestrator' : '/premium')}
                style={{
                  width: '100%',
                  background: gapFinderRemaining > 0 ? accent : (isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)'),
                  color: gapFinderRemaining > 0 ? 'white' : muted,
                  border: 'none',
                  borderRadius: '10px',
                  padding: '12px 20px',
                  fontSize: '14px',
                  fontWeight: '600',
                  cursor: gapFinderRemaining > 0 ? 'pointer' : 'default',
                  opacity: gapFinderRemaining > 0 ? 1 : 0.7,
                }}
              >
                {gapFinderRemaining > 0 ? 'Use PublishReady' : 'Upgrade for more'}
              </button>
            </div>

            {/* DataMaestro Card */}
            <div
              style={{
                background: cardBg,
                borderRadius: '20px',
                padding: '28px',
                border: `1px solid ${border}`,
                boxShadow: isLight ? '0 1px 3px rgba(0,0,0,0.06)' : 'none',
                transition: 'all 0.2s ease',
              }}
            >
              <div
                style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  justifyContent: 'space-between',
                  marginBottom: '20px',
                }}
              >
                <div>
                  <div
                    style={{
                      width: '48px',
                      height: '48px',
                      borderRadius: '12px',
                      background: 'linear-gradient(135deg, #10B981 0%, #059669 100%)',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'center',
                      fontSize: '24px',
                      marginBottom: '12px',
                    }}
                  >
                    📊
                  </div>
                  <h3 style={{ fontSize: '20px', fontWeight: '600', color: text, margin: '0 0 4px 0' }}>
                    DataMaestro
                  </h3>
                  <p style={{ fontSize: '14px', color: muted, margin: 0, lineHeight: 1.4 }}>
                    Deep paper analysis & statistical evaluation
                  </p>
                </div>
                <span
                  style={{
                    fontSize: '24px',
                    fontWeight: '700',
                    color: accentGreen,
                  }}
                >
                  {deepEvalRemaining}/{maxDeepEval}
                </span>
              </div>
              <div style={{ marginBottom: '20px' }}>
                <div
                  style={{
                    height: '8px',
                    borderRadius: '4px',
                    background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
                    overflow: 'hidden',
                  }}
                >
                  <div
                    style={{
                      height: '100%',
                      width: `${(deepEvalRemaining / maxDeepEval) * 100}%`,
                      background: `linear-gradient(90deg, ${accentGreen}, #059669)`,
                      borderRadius: '4px',
                      transition: 'width 0.4s ease',
                    }}
                  />
                </div>
                <p style={{ fontSize: '12px', color: muted, margin: '6px 0 0 0' }}>
                  uses remaining
                </p>
              </div>
              <button
                onClick={() => navigate(deepEvalRemaining > 0 ? '/statistical-research' : '/premium')}
                style={{
                  width: '100%',
                  background: deepEvalRemaining > 0 ? accentGreen : (isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)'),
                  color: deepEvalRemaining > 0 ? 'white' : muted,
                  border: 'none',
                  borderRadius: '10px',
                  padding: '12px 20px',
                  fontSize: '14px',
                  fontWeight: '600',
                  cursor: deepEvalRemaining > 0 ? 'pointer' : 'default',
                  opacity: deepEvalRemaining > 0 ? 1 : 0.7,
                }}
              >
                {deepEvalRemaining > 0 ? 'Use DataMaestro' : 'Upgrade for more'}
              </button>
            </div>
          </div>
        </section>

        {/* Plan Summary */}
        <section
          style={{
            background: cardBg,
            borderRadius: '20px',
            padding: '24px',
            border: `1px solid ${border}`,
            boxShadow: isLight ? '0 1px 3px rgba(0,0,0,0.06)' : 'none',
          }}
        >
          <h2
            style={{
              fontSize: '15px',
              fontWeight: '600',
              color: muted,
              textTransform: 'uppercase',
              letterSpacing: '0.05em',
              margin: '0 0 16px 0',
            }}
          >
            Plan Summary
          </h2>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: '16px' }}>
            <div>
              <p style={{ fontSize: '16px', fontWeight: '600', color: text, margin: '0 0 4px 0' }}>
                {currentPlanName}
              </p>
              {subscription?.expires_at && (
                <p style={{ fontSize: '13px', color: muted, margin: 0 }}>
                  Valid until {new Date(subscription.expires_at).toLocaleDateString()}
                </p>
              )}
            </div>
            <button
              onClick={() => navigate('/packages')}
              style={{
                background: isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.1)',
                color: text,
                border: 'none',
                borderRadius: '10px',
                padding: '10px 20px',
                fontSize: '14px',
                fontWeight: '600',
                cursor: 'pointer',
              }}
            >
              Upgrade Plan
            </button>
          </div>
        </section>
      </main>
    </div>
  );
};

export default UserDashboard;
