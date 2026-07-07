// Gaply DS — Modal (portal; overlay click + Escape close).
import React, { useEffect } from 'react';
import { createPortal } from 'react-dom';
import './primitives.css';

export interface ModalProps {
  open: boolean;
  title?: string;
  onClose: () => void;
  children: React.ReactNode;
}

export const Modal: React.FC<ModalProps> = ({ open, title, onClose, children }) => {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;
  return createPortal(
    <div className="gds-modal-overlay" onClick={onClose} data-testid="gds-modal-overlay">
      <div
        className="gds-modal"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="gds-modal__header">
          <span>{title}</span>
          <button type="button" className="gds-btn gds-btn--ghost" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="gds-modal__body">{children}</div>
      </div>
    </div>,
    document.body
  );
};
