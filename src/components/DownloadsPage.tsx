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
      icon: '🍎',
      file: 'Gaply-mac-arm64.dmg',
    },
    {
      id: 'windows',
      title: 'Windows',
      subtitle: 'Windows 10 & 11',
      href: DOWNLOAD_LINKS.windows,
      icon: '🪟',
      file: 'Gaply-windows.exe',
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
          maxWidth: '800px',
          margin: '0 auto',
          padding: '60px 24px 80px',
        }}
      >
        <h2
          style={{
            fontSize: 'clamp(32px, 5vw, 48px)',
            fontWeight: '700',
            letterSpacing: '-0.03em',
            marginBottom: '16px',
            lineHeight: 1.1,
          }}
        >
          Download Gaply Desktop
        </h2>
        <p
          style={{
            fontSize: '18px',
            color: 'rgba(255, 255, 255, 0.7)',
            lineHeight: 1.6,
            marginBottom: '48px',
          }}
        >
          Use Gaply on your Mac or Windows laptop. Same features, same experience — works offline after the first load.
        </p>

        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            gap: '20px',
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
                gap: '24px',
                padding: '24px 28px',
                background: 'rgba(255, 255, 255, 0.05)',
                border: '1px solid rgba(255, 255, 255, 0.12)',
                borderRadius: '16px',
                textDecoration: 'none',
                color: '#fff',
                transition: 'all 0.2s ease',
              }}
              onMouseOver={(e) => {
                e.currentTarget.style.background = 'rgba(255, 255, 255, 0.08)';
                e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.2)';
              }}
              onMouseOut={(e) => {
                e.currentTarget.style.background = 'rgba(255, 255, 255, 0.05)';
                e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.12)';
              }}
            >
              <span style={{ fontSize: '40px' }}>{d.icon}</span>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: '18px', fontWeight: '600', marginBottom: '4px' }}>
                  {d.title}
                </div>
                <div style={{ fontSize: '14px', color: 'rgba(255, 255, 255, 0.6)' }}>
                  {d.subtitle}
                </div>
              </div>
              <span
                style={{
                  padding: '10px 20px',
                  background: 'rgba(0, 122, 255, 0.2)',
                  borderRadius: '8px',
                  fontSize: '14px',
                  fontWeight: '600',
                }}
              >
                Download
              </span>
            </a>
          ))}
        </div>

        <p
          style={{
            marginTop: '40px',
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
            marginTop: '24px',
            fontSize: '14px',
            color: 'rgba(255, 255, 255, 0.6)',
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
