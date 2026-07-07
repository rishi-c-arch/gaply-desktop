// Gaply DS — Toast (provider + hook; auto-dismiss; portal viewport).
import React, { createContext, useCallback, useContext, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import './primitives.css';
import type { BadgeStatus } from './primitives';

export interface ToastItem {
  id: number;
  message: string;
  status: BadgeStatus;
}

interface ToastContextValue {
  toast: (message: string, status?: BadgeStatus, durationMs?: number) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export const useToast = (): ToastContextValue => {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error('useToast must be used inside <ToastProvider>');
  return ctx;
};

export const ToastProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [items, setItems] = useState<ToastItem[]>([]);
  const nextId = useRef(1);

  const toast = useCallback((message: string, status: BadgeStatus = 'neutral', durationMs = 4000) => {
    const id = nextId.current++;
    setItems((xs) => [...xs, { id, message, status }]);
    window.setTimeout(() => setItems((xs) => xs.filter((x) => x.id !== id)), durationMs);
  }, []);

  return (
    <ToastContext.Provider value={{ toast }}>
      {children}
      {createPortal(
        <div className="gds-toast-viewport" data-testid="gds-toast-viewport">
          {items.map((t) => (
            <div key={t.id} className={`gds-toast gds-toast--${t.status}`} role="status">
              {t.message}
            </div>
          ))}
        </div>,
        document.body
      )}
    </ToastContext.Provider>
  );
};
