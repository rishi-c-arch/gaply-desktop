// Gaply — online/offline connectivity indicator (the F1 header affordance,
// realized in F14). Users must always be able to tell whether cloud features
// are reachable; the five local suites work either way.
import React, { useEffect, useState } from 'react';

export function useOnlineStatus(): boolean {
  const [online, setOnline] = useState(typeof navigator === 'undefined' ? true : navigator.onLine);
  useEffect(() => {
    const up = () => setOnline(true);
    const down = () => setOnline(false);
    window.addEventListener('online', up);
    window.addEventListener('offline', down);
    return () => {
      window.removeEventListener('online', up);
      window.removeEventListener('offline', down);
    };
  }, []);
  return online;
}

export const ConnectivityIndicator: React.FC = () => {
  const online = useOnlineStatus();
  return (
    <span
      className="gds-conn"
      data-online={online}
      data-testid="connectivity"
      title={online ? 'Cloud features available' : 'Offline — local suites keep working'}
    >
      <span className="gds-conn__dot" aria-hidden="true" />
      {online ? 'online' : 'offline — local only'}
    </span>
  );
};
