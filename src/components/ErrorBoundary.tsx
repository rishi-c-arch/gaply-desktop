// Error Boundary Component for GAPLY Frontend
// Production-ready error handling with fallback UI

import React, { Component, ErrorInfo, ReactNode } from 'react';

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: ErrorInfo) => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): State {
    // Update state so the next render will show the fallback UI
    return { hasError: true, error, errorInfo: null };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    // Log error to console in development
    if (process.env.NODE_ENV === 'development') {
      console.error('Error caught by boundary:', error, errorInfo);
    }

    // Call custom error handler if provided
    if (this.props.onError) {
      this.props.onError(error, errorInfo);
    }

    // Send to error tracking service in production
    if (process.env.NODE_ENV === 'production') {
      // Example: Send to Sentry or other error tracking service
      // Sentry.captureException(error, { extra: errorInfo });
    }

    this.setState({
      error,
      errorInfo,
    });
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null, errorInfo: null });
  };

  render() {
    if (this.state.hasError) {
      // Custom fallback UI
      if (this.props.fallback) {
        return this.props.fallback;
      }

      // Default fallback UI
      return <ErrorFallback error={this.state.error} onRetry={this.handleRetry} />;
    }

    return this.props.children;
  }
}

interface ErrorFallbackProps {
  error: Error | null;
  onRetry: () => void;
}

const ErrorFallback: React.FC<ErrorFallbackProps> = ({ error, onRetry }) => {
  return (
    <div className="error-fallback">
      <div className="error-container">
        <div className="error-icon">⚠️</div>
        <h2 className="error-title">Something went wrong</h2>
        <p className="error-message">
          We're sorry, but something unexpected happened. Please try again.
        </p>
        
        {process.env.NODE_ENV === 'development' && error && (
          <details className="error-details">
            <summary>Error Details</summary>
            <pre className="error-stack">{error.stack}</pre>
          </details>
        )}
        
        <div className="error-actions">
          <button onClick={onRetry} className="retry-button">
            Try Again
          </button>
          <button 
            onClick={() => window.location.reload()} 
            className="reload-button"
          >
            Reload Page
          </button>
        </div>
      </div>
      
      <style>{`
        .error-fallback {
          min-height: 100vh;
          display: flex;
          align-items: center;
          justify-content: center;
          background: linear-gradient(135deg, #ff6b35 0%, #f7931e 100%);
          padding: 20px;
        }
        
        .error-container {
          background: white;
          border-radius: 20px;
          padding: 40px;
          max-width: 500px;
          width: 100%;
          text-align: center;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.2);
        }
        
        .error-icon {
          font-size: 48px;
          margin-bottom: 20px;
        }
        
        .error-title {
          color: #2c3e50;
          font-size: 24px;
          font-weight: 700;
          margin-bottom: 15px;
        }
        
        .error-message {
          color: #666;
          font-size: 16px;
          line-height: 1.5;
          margin-bottom: 30px;
        }
        
        .error-details {
          text-align: left;
          margin-bottom: 30px;
          padding: 15px;
          background: #f8f9fa;
          border-radius: 8px;
          border: 1px solid #e9ecef;
        }
        
        .error-stack {
          font-family: 'Courier New', monospace;
          font-size: 12px;
          color: #dc3545;
          white-space: pre-wrap;
          overflow-x: auto;
        }
        
        .error-actions {
          display: flex;
          gap: 15px;
          justify-content: center;
        }
        
        .retry-button,
        .reload-button {
          padding: 12px 24px;
          border: none;
          border-radius: 8px;
          font-size: 16px;
          font-weight: 600;
          cursor: pointer;
          transition: all 0.3s ease;
        }
        
        .retry-button {
          background: linear-gradient(135deg, #ff6b35 0%, #f7931e 100%);
          color: white;
        }
        
        .retry-button:hover {
          transform: translateY(-2px);
          box-shadow: 0 8px 25px rgba(255, 107, 53, 0.3);
        }
        
        .reload-button {
          background: #6c757d;
          color: white;
        }
        
        .reload-button:hover {
          background: #5a6268;
          transform: translateY(-2px);
        }
        
        @media (max-width: 768px) {
          .error-container {
            padding: 30px 20px;
          }
          
          .error-actions {
            flex-direction: column;
          }
        }
      `}</style>
    </div>
  );
};

export default ErrorBoundary;
