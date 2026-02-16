import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import './index.css';
import { ErrorBoundary } from './components/ErrorBoundary';
import reportWebVitals from './reportWebVitals';

// Suppress WebGL context lost - prevents blank page when GPU/context is lost
if (typeof window !== 'undefined') {
  window.addEventListener('webglcontextlost', (e) => { e.preventDefault(); }, false);
}

// Catch uncaught errors and show them (helps debug blank page)
const showError = (title: string, detail: string) => {
  const root = document.getElementById('root');
  if (root && !root.innerHTML.includes('Something went wrong')) {
    root.innerHTML = `<div style="padding:48px 24px;font-family:system-ui;max-width:600px;margin:0 auto;color:#333;background:#fff">
      <h1 style="color:#c00">${title}</h1>
      <pre style="background:#f5f5f5;padding:16px;overflow:auto;font-size:12px;max-height:300px">${detail}</pre>
      <p><a href="/" style="color:#007AFF">Go home</a> · <a href="javascript:location.reload()" style="color:#007AFF">Retry</a></p>
    </div>`;
  }
};
window.addEventListener('error', (e) => {
  showError('Something went wrong', (e.error && e.error.stack) || String(e.message));
});
window.addEventListener('unhandledrejection', (e) => {
  showError('Loading error', String(e.reason));
});

// Suppress non-critical warnings (source map errors, 403 errors from external resources)
if (process.env.NODE_ENV === 'development') {
  const originalError = console.error;
  const originalWarn = console.warn;

  console.error = (...args: any[]) => {
    const message = args.join(' ');
    // Suppress mediapipe source map warnings (transitive dependency issue, non-critical)
    if (message.includes('Failed to parse source map') && message.includes('mediapipe')) {
      return;
    }
    // Suppress 403 Forbidden warnings (external resources, non-critical)
    if (message.includes('Failed to load resource') && message.includes('403')) {
      return;
    }
    originalError.apply(console, args);
  };

  console.warn = (...args: any[]) => {
    const message = args.join(' ');
    // Suppress mediapipe source map warnings
    if (message.includes('Failed to parse source map') && message.includes('mediapipe')) {
      return;
    }
    originalWarn.apply(console, args);
  };
}

const root = ReactDOM.createRoot(
  document.getElementById('root') as HTMLElement
);

// Path-based entry: /final-orchestrator loads a minimal bundle with NO Three.js/WebGL
// (fixes "Context Lost" blank page on mobile/incognito)
const path = window.location.pathname;
if (path === '/final-orchestrator' || path.startsWith('/final-orchestrator/')) {
  import('./FinalOrchestratorApp');
} else {
  import('./App').then(({ default: App }) => {
    root.render(
      <React.StrictMode>
        <ErrorBoundary fallback={
          <div style={{ padding: 48, fontFamily: 'system-ui', maxWidth: 600, margin: '0 auto', color: '#333' }}>
            <h1 style={{ color: '#c00' }}>Something went wrong</h1>
            <p><a href="/" style={{ color: '#007AFF' }}>Go home</a></p>
          </div>
        }>
          <BrowserRouter>
            <App />
          </BrowserRouter>
        </ErrorBoundary>
      </React.StrictMode>
    );
  });
}

// Hide loading/fallback when we have visible content (or after timeout to avoid stuck "Loading...")
const hideFallbackWhenReady = () => {
  const el = document.getElementById('static-fallback');
  const loadingEl = document.getElementById('gaply-loading');
  const root = document.getElementById('root');
  if (!el) return;
  const path = window.location.pathname;
  const hasManuscriptContent = root?.querySelector('.manuscript-page, .manuscript-hero, .manuscript-grid');
  const hasHomeContent = root?.querySelector('.apple-hero, .apple-hero__title');
  const hasGenericContent = root && root.innerHTML.trim().length > 500;
  const appMounted = root?.getAttribute('data-app-mounted') === 'true';
  const hasAnyContent = hasManuscriptContent || hasHomeContent || hasGenericContent || (appMounted && root?.children?.length);
  const shouldHide =
    (path === '/final-orchestrator' && (!!hasManuscriptContent || appMounted)) ||
    (path === '/' && (!!hasHomeContent || appMounted)) ||
    ((path !== '/' && path !== '/final-orchestrator') && (hasGenericContent || !!root?.querySelector('[class]') || appMounted));
  if (shouldHide) {
    el.style.display = 'none';
    if (loadingEl) loadingEl.style.display = 'none';
  }
};
window.addEventListener('gaply-app-mounted', hideFallbackWhenReady);
requestAnimationFrame(() => requestAnimationFrame(hideFallbackWhenReady));
setTimeout(hideFallbackWhenReady, 300);
setTimeout(hideFallbackWhenReady, 800);
setTimeout(hideFallbackWhenReady, 1500);
setTimeout(hideFallbackWhenReady, 2500);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
