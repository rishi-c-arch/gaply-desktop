import React, { useEffect, useState } from 'react';

/**
 * Non-intrusive notice when the browser reports offline; does not change layout when online.
 */
export const OfflineHintBanner: React.FC = () => {
  const [offline, setOffline] = useState(
    () => typeof navigator !== 'undefined' && !navigator.onLine,
  );

  useEffect(() => {
    const onOnline = () => setOffline(false);
    const onOffline = () => setOffline(true);
    window.addEventListener('online', onOnline);
    window.addEventListener('offline', onOffline);
    return () => {
      window.removeEventListener('online', onOnline);
      window.removeEventListener('offline', onOffline);
    };
  }, []);

  if (!offline) return null;

  return (
    <div
      role="status"
      aria-live="polite"
      style={{
        position: 'fixed',
        bottom: 0,
        left: 0,
        right: 0,
        zIndex: 9998,
        padding: '10px 16px',
        textAlign: 'center',
        background: 'var(--bg-secondary)',
        color: 'var(--text-primary)',
        borderTop: '1px solid var(--border-glow)',
        fontSize: 14,
        fontFamily: 'var(--font-body)',
      }}
    >
      You appear to be offline. Some actions may not work until you reconnect.
    </div>
  );
};
