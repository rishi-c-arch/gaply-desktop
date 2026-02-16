/**
 * Minimal app for /final-orchestrator - NO Three.js/WebGL.
 * Loaded only when path is /final-orchestrator to avoid "Context Lost" blank page.
 */
import React, { useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider } from './contexts/ThemeContext';
import { AuthProvider } from './contexts/AuthContext';
import { ErrorBoundary } from './components/ErrorBoundary';
import ManuscriptOrchestratorPage from './components/ManuscriptOrchestratorPage';
import SEO from './components/SEO';
import './index.css';
import './App.css';
import './components/ManuscriptOrchestratorPage.css';

const FinalOrchestratorRoot: React.FC = () => {
  useEffect(() => {
    const root = document.getElementById('root');
    if (root) {
      root.setAttribute('data-app-mounted', 'true');
      window.dispatchEvent(new CustomEvent('gaply-app-mounted'));
    }
  }, []);
  return (
    <div className="App" style={{ minHeight: '100vh', background: '#ffffff' }}>
      <SEO
        title="Final Analysis Suite | Gaply"
        description="Upload manuscript files, add journal links, and generate a clean report with Gaply chat support."
        keywords="final analysis suite, manuscript upload, journal submission assistant, gaply chat"
      />
      <div className="app-content-below-header" style={{ paddingTop: 0, minHeight: '100vh' }}>
        <ManuscriptOrchestratorPage />
      </div>
    </div>
  );
};

const root = document.getElementById('root');
if (root) {
  const r = ReactDOM.createRoot(root);
  r.render(
    <React.StrictMode>
      <ErrorBoundary fallback={
        <div style={{ padding: '80px 24px 48px', maxWidth: 600, margin: '0 auto', textAlign: 'center', color: '#1d1d1f' }}>
          <h1 style={{ fontSize: '24px', marginBottom: 16 }}>Something went wrong</h1>
          <p style={{ marginBottom: 24, color: '#6e6e73' }}>The page couldn&apos;t load. Try refreshing or <a href="/" style={{ color: '#007AFF' }}>go home</a>.</p>
        </div>
      }>
        <BrowserRouter>
          <ThemeProvider>
            <AuthProvider>
              <FinalOrchestratorRoot />
            </AuthProvider>
          </ThemeProvider>
        </BrowserRouter>
      </ErrorBoundary>
    </React.StrictMode>
  );
}
