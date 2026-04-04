import React from 'react';
import { useNavigate } from 'react-router-dom';
import './SeoGuidePages.css';

interface SeoGuideShellProps {
  headline: string;
  subhead: string;
  children: React.ReactNode;
  /** Match /features typography: tighter subhead, wider column. */
  variant?: 'default' | 'feature';
}

const SeoGuideShell: React.FC<SeoGuideShellProps> = ({ headline, subhead, children, variant = 'default' }) => {
  const navigate = useNavigate();
  const feature = variant === 'feature';

  return (
    <div
      className={`seo-guide-root${feature ? ' seo-guide-root--feature' : ''}`}
      style={{
        minHeight: '100vh',
        background: 'var(--app-bg)',
        color: 'var(--app-text)',
        fontFamily:
          '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
      }}
    >
      <div
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          zIndex: 1000,
          background: 'var(--header-bg)',
          backdropFilter: 'blur(20px)',
          WebkitBackdropFilter: 'blur(20px)',
          borderBottom: '1px solid var(--header-border)',
          padding: '12px 0',
        }}
      >
        <div
          style={{
            maxWidth: 1200,
            margin: '0 auto',
            padding: '0 clamp(16px, 4vw, 40px)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <div style={{ fontWeight: 800, letterSpacing: '.02em' }}>
            <div
              style={{
                margin: 0,
                fontSize: 18,
                color: 'var(--header-text)',
                fontFamily:
                  '-apple-system, BlinkMacSystemFont, "SF Pro Display", "Segoe UI", Roboto, sans-serif',
              }}
            >
              Gaply
            </div>
          </div>
          <button
            type="button"
            onClick={() => navigate('/')}
            style={{
              background: 'var(--toggle-bg)',
              color: 'var(--app-text)',
              padding: '10px 20px',
              borderRadius: '12px',
              border: '1px solid var(--toggle-border)',
              cursor: 'pointer',
              fontSize: '1rem',
              fontWeight: '500',
              transition: 'all 0.3s ease',
              backdropFilter: 'blur(10px)',
              WebkitBackdropFilter: 'blur(10px)',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--button-bg-hover)';
              e.currentTarget.style.borderColor = 'var(--button-border-hover)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'var(--toggle-bg)';
              e.currentTarget.style.borderColor = 'var(--toggle-border)';
            }}
          >
            Back to Home
          </button>
        </div>
      </div>

      <main
        style={{
          paddingTop: 'clamp(110px, 12vw, 140px)',
          paddingBottom: 'clamp(56px, 8vw, 96px)',
          paddingLeft: 'clamp(16px, 4vw, 40px)',
          paddingRight: 'clamp(16px, 4vw, 40px)',
        }}
      >
        <div className="seo-guide-inner" style={{ maxWidth: feature ? 1200 : 960, margin: '0 auto' }}>
          <header
            className={feature ? 'seo-guide-feature-header' : undefined}
            style={
              feature
                ? undefined
                : {
                    textAlign: 'center',
                    marginBottom: 'clamp(28px, 6vw, 48px)',
                  }
            }
          >
            <h1
              className={feature ? 'seo-guide-feature-header__title' : undefined}
              style={
                feature
                  ? undefined
                  : {
                      fontSize: 'clamp(2rem, 5vw, 3.25rem)',
                      fontWeight: 700,
                      marginBottom: '16px',
                      letterSpacing: '-0.02em',
                      lineHeight: 1.15,
                    }
              }
            >
              {headline}
            </h1>
            <p
              className={feature ? 'seo-guide-feature-header__sub' : undefined}
              style={
                feature
                  ? undefined
                  : {
                      fontSize: 'clamp(1.05rem, 2.5vw, 1.35rem)',
                      color: 'var(--muted-text)',
                      lineHeight: 1.6,
                      margin: 0,
                    }
              }
            >
              {subhead}
            </p>
          </header>
          {children}
        </div>
      </main>
    </div>
  );
};

export default SeoGuideShell;
