// The design system's Button with a ref, so focus can return to the control
// that opened a panel when the panel closes.
import React from 'react';
import type { ButtonProps } from '../../../design-system';

export const FocusButton = React.forwardRef<HTMLButtonElement, ButtonProps>(({ variant = 'primary', className = '', ...rest }, ref) => (
  <button ref={ref} type="button" className={`gds-btn gds-btn--${variant} ${className}`} {...rest} />
));
FocusButton.displayName = 'FocusButton';
