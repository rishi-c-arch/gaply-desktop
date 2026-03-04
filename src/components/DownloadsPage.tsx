import React from 'react';
import { useNavigate } from 'react-router-dom';
import { DOWNLOAD_LINKS, RELEASE_BASE } from '../config/downloads';

const DownloadsPage: React.FC = () => {
  const navigate = useNavigate();

  const downloads = [
    {
      id: 'mac-arm64',
      title: 'Mac (Apple Silicon)',
      subtitle: 'M1, M2, M3, M4',
      href: DOWNLOAD_LINKS.macArm64,
    },
    {
      id: 'windows',
      title: 'Windows',
      subtitle: 'Windows 10 & 11',
      href: DOWNLOAD_LINKS.windows,
    },
  ];

  return (
    <div
      style={{
        minHeight: '100vh',
        background: '#000000',
        color: '#ffffff',
        fontFamily: '-apple-system, BlinkMacSystemFont, "SF Pro Display", "SF Pro Text", "Helvetica Neue", Helvetica, Arial, sans-serif',
        paddingTop: '80px',
      }}
    >
      {/* Header */}
      <div
        style={{
          position: 'fixed',
          top: 0,
          left: 0,
          right: 0,
          zIndex: 1000,
          background: 'rgba(0, 0, 0, 0.8)',
          backdropFilter: 'blur(20px)',
          WebkitBackdropFilter: 'blur(20px)',
          borderBottom: '1px solid rgba(255, 255, 255, 0.1)',
          padding: '20px 40px',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <h1
          style={{
            fontSize: '24px',
            fontWeight: '600',
            margin: 0,
            letterSpacing: '-0.02em',
          }}
        >
          Gaply
        </h1>

        <button
          onClick={() => navigate('/')}
          style={{
            background: 'transparent',
            border: '1px solid rgba(255, 255, 255, 0.3)',
            color: '#fff',
            padding: '10px 20px',
            borderRadius: '8px',
            cursor: 'pointer',
            fontSize: '14px',
          }}
        >
          Back to Home
        </button>
      </div>

      {/* Main content */}
      <div
        style={{
          maxWidth: '720px',
          margin: '0 auto',
          padding: '60px 24px 80px',
        }}
      >
        <h2
          style={{
            fontSize: 'clamp(28px, 4vw, 40px)',
            fontWeight: '600',
            letterSpacing: '-0.03em',
            marginBottom: '12px',
            lineHeight: 1.2,
          }}
        >
          Download Gaply Desktop
        </h2>
        <p
          style={{
            fontSize: '16px',
            color: 'rgba(255, 255, 255, 0.65)',
            lineHeight: 1.6,
            marginBottom: '40px',
          }}
        >
          Same features as the web app — works offline after the first load.
        </p>

        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '16px',
          }}
        >
          {downloads.map((d) => (
            <a
              key={d.id}
              href={d.href}
              target="_blank"
              rel="noopener noreferrer"
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: '20px',
                padding: '20px 24px',
                background: 'rgba(255, 255, 255, 0.04)',
                border: '1px solid rgba(255, 255, 255, 0.1)',
                borderRadius: '12px',
                textDecoration: 'none',
                color: '#fff',
                transition: 'all 0.2s ease',
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.background = 'rgba(255, 255, 255, 0.07)';
                e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.18)';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.background = 'rgba(255, 255, 255, 0.04)';
                e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.1)';
              }}
            >
              <div>
                <div style={{ fontSize: '16px', fontWeight: '600', marginBottom: '2px' }}>
                  {d.title}
                </div>
                <div style={{ fontSize: '13px', color: 'rgba(255, 255, 255, 0.5)' }}>
                  {d.subtitle}
                </div>
              </div>
              <span
                style={{
                  padding: '8px 16px',
                  background: 'rgba(255, 255, 255, 0.1)',
                  borderRadius: '6px',
                  fontSize: '13px',
                  fontWeight: '500',
                }}
              >
                Download
              </span>
            </a>
          ))}
        </div>

        {/* Security notice */}
        <div
          style={{
            marginTop: '40px',
            padding: '24px',
            background: 'rgba(255, 255, 255, 0.03)',
            border: '1px solid rgba(255, 255, 255, 0.08)',
            borderRadius: '12px',
          }}
        >
          <div
            style={{
              fontSize: '14px',
              fontWeight: '600',
              marginBottom: '12px',
              color: 'rgba(255, 255, 255, 0.9)',
            }}
          >
            If your system shows a security warning
          </div>
          <p
            style={{
              fontSize: '14px',
              color: 'rgba(255, 255, 255, 0.6)',
              lineHeight: 1.7,
              marginBottom: '16px',
            }}
          >
            Gaply is a new, unfunded venture. We haven&apos;t yet paid for official code signing certificates, so Mac and Windows may show a warning when you first run the app. The app is safe — you can trust it. We&apos;re working to add proper signing in a future update.
          </p>
          <div
            style={{
              fontSize: '13px',
              color: 'rgba(255, 255, 255, 0.55)',
              lineHeight: 1.8,
            }}
          >
            <strong style={{ color: 'rgba(255, 255, 255, 0.75)' }}>Mac:</strong> Right-click the app → Open → Open again.
            <br />
            <strong style={{ color: 'rgba(255, 255, 255, 0.75)' }}>Windows:</strong> Click &quot;More info&quot; → &quot;Run anyway&quot;.
          </div>
        </div>

        <p
          style={{
            marginTop: '32px',
            fontSize: '14px',
            color: 'rgba(255, 255, 255, 0.5)',
            lineHeight: 1.6,
          }}
        >
          After downloading, open the file and follow the installer. The app connects to gaply.in — no account changes needed.
        </p>

        <a
          href={RELEASE_BASE}
          target="_blank"
          rel="noopener noreferrer"
          style={{
            display: 'inline-block',
            marginTop: '20px',
            fontSize: '14px',
            color: 'rgba(255, 255, 255, 0.55)',
            textDecoration: 'underline',
          }}
        >
          View all releases on GitHub →
        </a>
      </div>
    </div>
  );
};

export default DownloadsPage;
