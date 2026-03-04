import React, { useEffect, useState } from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { DOWNLOAD_LINKS } from '../config/downloads';

const STORAGE_KEY = 'gaply_download_popup_dismissed';
const DISMISS_DAYS = 7;

const isDismissed = (): boolean => {
  try {
    const val = localStorage.getItem(STORAGE_KEY);
    if (!val) return false;
    const parsed = JSON.parse(val);
    const age = (Date.now() - parsed.ts) / (1000 * 60 * 60 * 24);
    return age < DISMISS_DAYS;
  } catch {
    return false;
  }
};

const setDismissed = () => {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ ts: Date.now() }));
  } catch {}
};

const DownloadPopUp: React.FC = () => {
  const [visible, setVisible] = useState(false);
  const [mounted, setMounted] = useState(false);
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    setMounted(true);
  }, []);

  useEffect(() => {
    if (!mounted || isDismissed() || location.pathname === '/download') return;
    const t = setTimeout(() => setVisible(true), 3000);
    return () => clearTimeout(t);
  }, [mounted, location.pathname]);

  const handleDismiss = () => {
    setDismissed();
    setVisible(false);
  };

  const handleDownload = (url: string) => {
    setDismissed();
    setVisible(false);
    window.open(url, '_blank');
  };

  const handleViewAll = () => {
    setDismissed();
    setVisible(false);
    navigate('/download');
  };

  if (!visible) return null;

  return (
    <>
      <div
        role="presentation"
        onClick={handleDismiss}
        style={{
          position: 'fixed',
          inset: 0,
          background: 'rgba(0,0,0,0.6)',
          backdropFilter: 'blur(8px)',
          WebkitBackdropFilter: 'blur(8px)',
          zIndex: 9998,
          animation: 'fadeIn 0.25s ease-out',
        }}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby="download-popup-title"
        style={{
          position: 'fixed',
          top: '50%',
          left: '50%',
          transform: 'translate(-50%, -50%)',
          zIndex: 9999,
          width: 'min(420px, 92vw)',
          background: 'linear-gradient(145deg, #0d0d0d 0%, #1a1a1a 100%)',
          borderRadius: '20px',
          border: '1px solid rgba(255,255,255,0.1)',
          boxShadow: '0 24px 80px rgba(0,0,0,0.5), 0 0 0 1px rgba(255,255,255,0.05)',
          overflow: 'hidden',
          animation: 'slideUp 0.35s cubic-bezier(0.16, 1, 0.3, 1)',
        }}
      >
        <style>{`
          @keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
          @keyframes slideUp { from { opacity: 0; transform: translate(-50%, -48%); } to { opacity: 1; transform: translate(-50%, -50%); } }
        `}</style>

        <div style={{ padding: '28px 24px 24px' }}>
          <button
            onClick={handleDismiss}
            aria-label="Close"
            style={{
              position: 'absolute',
              top: 16,
              right: 16,
              background: 'none',
              border: 'none',
              color: 'rgba(255,255,255,0.5)',
              cursor: 'pointer',
              padding: 8,
              borderRadius: 8,
            }}
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>

          <div style={{ marginBottom: 20 }}>
            <div
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 8,
                padding: '6px 12px',
                background: 'rgba(0, 122, 255, 0.15)',
                borderRadius: 20,
                fontSize: 12,
                fontWeight: 600,
                color: '#007AFF',
                marginBottom: 16,
              }}
            >
              Desktop app available
            </div>
            <h2
              id="download-popup-title"
              style={{
                fontSize: 22,
                fontWeight: 700,
                color: '#fff',
                margin: 0,
                lineHeight: 1.3,
              }}
            >
              Get Gaply on your device
            </h2>
            <p
              style={{
                fontSize: 15,
                color: 'rgba(255,255,255,0.65)',
                margin: '8px 0 0',
                lineHeight: 1.5,
              }}
            >
              Same features, works offline after first load.
            </p>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <button
              onClick={() => handleDownload(DOWNLOAD_LINKS.macArm64)}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: 14,
                padding: '14px 18px',
                background: 'rgba(255,255,255,0.08)',
                border: '1px solid rgba(255,255,255,0.12)',
                borderRadius: 12,
                color: '#fff',
                fontSize: 15,
                fontWeight: 600,
                cursor: 'pointer',
                textAlign: 'left',
              }}
            >
              <span>Mac (Apple Silicon)</span>
            </button>
            <button
              onClick={() => handleDownload(DOWNLOAD_LINKS.windows)}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                gap: 14,
                padding: '14px 18px',
                background: 'rgba(255,255,255,0.08)',
                border: '1px solid rgba(255,255,255,0.12)',
                borderRadius: 12,
                color: '#fff',
                fontSize: 15,
                fontWeight: 600,
                cursor: 'pointer',
                textAlign: 'left',
              }}
            >
              <span>Windows</span>
            </button>
          </div>

          <div style={{ display: 'flex', gap: 10, marginTop: 20 }}>
            <button
              onClick={handleViewAll}
              style={{
                flex: 1,
                padding: '12px 16px',
                background: 'transparent',
                border: '1px solid rgba(255,255,255,0.2)',
                borderRadius: 10,
                color: 'rgba(255,255,255,0.9)',
                fontSize: 14,
                fontWeight: 500,
                cursor: 'pointer',
              }}
            >
              View all options
            </button>
            <button
              onClick={handleDismiss}
              style={{
                flex: 1,
                padding: '12px 16px',
                background: 'rgba(255,255,255,0.06)',
                border: 'none',
                borderRadius: 10,
                color: 'rgba(255,255,255,0.7)',
                fontSize: 14,
                cursor: 'pointer',
              }}
            >
              Maybe later
            </button>
          </div>
        </div>
      </div>
    </>
  );
};

export default DownloadPopUp;
