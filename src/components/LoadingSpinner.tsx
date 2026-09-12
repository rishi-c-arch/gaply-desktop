// Loading Spinner Component for GAPLY Frontend
// Production-ready loading states with Web3 styling

import React from 'react';

interface LoadingSpinnerProps {
  size?: 'small' | 'medium' | 'large';
  color?: 'primary' | 'white' | 'dark';
  text?: string;
  fullScreen?: boolean;
}

export const LoadingSpinner: React.FC<LoadingSpinnerProps> = ({
  size = 'medium',
  color = 'primary',
  text,
  fullScreen = false
}) => {
  const sizeClasses = {
    small: 'loading-spinner-small',
    medium: 'loading-spinner-medium',
    large: 'loading-spinner-large'
  };

  const colorClasses = {
    primary: 'loading-spinner-primary',
    white: 'loading-spinner-white',
    dark: 'loading-spinner-dark'
  };

  const spinnerContent = (
    <div className={`loading-spinner ${sizeClasses[size]} ${colorClasses[color]}`}>
      <div className="spinner-ring">
        <div className="spinner-ring-inner"></div>
      </div>
      {text && <p className="spinner-text">{text}</p>}
    </div>
  );

  if (fullScreen) {
    return (
      <div className="loading-overlay">
        <div className="loading-backdrop"></div>
        {spinnerContent}
      </div>
    );
  }

  return spinnerContent;
};

// Loading States Hook
export const useLoading = (initialState: boolean = false) => {
  const [isLoading, setIsLoading] = React.useState(initialState);

  const startLoading = React.useCallback(() => {
    setIsLoading(true);
  }, []);

  const stopLoading = React.useCallback(() => {
    setIsLoading(false);
  }, []);

  const withLoading = React.useCallback(
    async (asyncFunction: () => Promise<any>) => {
      try {
        startLoading();
        return await asyncFunction();
      } finally {
        stopLoading();
      }
    }, [startLoading, stopLoading]);

  return {
    isLoading,
    startLoading,
    stopLoading,
    withLoading
  };
};

// CSS Styles (to be added to your CSS file)
// Built and never wired, in a component nothing imports — see the lint
// step's note in .github/workflows/frontend-build.yml. Kept rather than
// deleted so the unfinished work stays visible.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const loadingStyles = `
.loading-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
}

.loading-backdrop {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(4px);
}

.loading-spinner {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  z-index: 10000;
}

.spinner-ring {
  position: relative;
  border-radius: 50%;
  border: 3px solid transparent;
  border-top: 3px solid currentColor;
  animation: spin 1s linear infinite;
}

.loading-spinner-small .spinner-ring {
  width: 24px;
  height: 24px;
}

.loading-spinner-medium .spinner-ring {
  width: 40px;
  height: 40px;
}

.loading-spinner-large .spinner-ring {
  width: 60px;
  height: 60px;
}

.spinner-ring-inner {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  border: 2px solid transparent;
  border-top: 2px solid currentColor;
  animation: spin 0.8s linear infinite reverse;
}

.loading-spinner-small .spinner-ring-inner {
  width: 12px;
  height: 12px;
}

.loading-spinner-medium .spinner-ring-inner {
  width: 20px;
  height: 20px;
}

.loading-spinner-large .spinner-ring-inner {
  width: 30px;
  height: 30px;
}

.loading-spinner-primary {
  color: #ff6b35;
}

.loading-spinner-white {
  color: white;
}

.loading-spinner-dark {
  color: #2c3e50;
}

.spinner-text {
  margin-top: 15px;
  font-size: 16px;
  font-weight: 500;
  color: currentColor;
  text-align: center;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

/* Web3 Enhanced Spinner */
.loading-spinner-web3 {
  position: relative;
}

.loading-spinner-web3::before {
  content: '';
  position: absolute;
  top: -5px;
  left: -5px;
  right: -5px;
  bottom: -5px;
  border-radius: 50%;
  background: linear-gradient(45deg, 
    rgba(255, 107, 53, 0.1) 0%, 
    rgba(247, 147, 30, 0.1) 25%, 
    rgba(255, 107, 53, 0.1) 50%, 
    rgba(247, 147, 30, 0.1) 75%, 
    rgba(255, 107, 53, 0.1) 100%);
  animation: rotate 2s linear infinite;
}

@keyframes rotate {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

/* Responsive adjustments */
@media (max-width: 768px) {
  .spinner-text {
    font-size: 14px;
  }
  
  .loading-spinner-large .spinner-ring {
    width: 50px;
    height: 50px;
  }
  
  .loading-spinner-large .spinner-ring-inner {
    width: 25px;
    height: 25px;
  }
}
`;

export default LoadingSpinner;
