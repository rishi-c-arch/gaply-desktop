import React from 'react';
import './PaymentSuccessOverlay.css';

interface PaymentSuccessOverlayProps {
  show: boolean;
  title?: string;
  message?: string;
  onAutoDismiss?: () => void;
  /** Auto-dismiss after ms; 0 = no auto-dismiss */
  autoDismissMs?: number;
}

/**
 * Full-screen overlay shown only when payment verification succeeds.
 * Displays "Payment Successful! Enjoy your premium features." (or custom message).
 */
const PaymentSuccessOverlay: React.FC<PaymentSuccessOverlayProps> = ({
  show,
  title = 'Payment Successful!',
  message = 'Enjoy your premium features.',
  onAutoDismiss,
  autoDismissMs = 4500,
}) => {
  React.useEffect(() => {
    if (!show || autoDismissMs <= 0 || !onAutoDismiss) return;
    const t = setTimeout(onAutoDismiss, autoDismissMs);
    return () => clearTimeout(t);
  }, [show, autoDismissMs, onAutoDismiss]);

  if (!show) return null;

  return (
    <div className="payment-success-overlay" role="alert" aria-live="polite">
      <div className="payment-success-card">
        <div className="payment-success-icon">✓</div>
        <h3>{title}</h3>
        <p>{message}</p>
        <div className="payment-success-confetti" aria-hidden="true" />
      </div>
    </div>
  );
};

export default PaymentSuccessOverlay;
