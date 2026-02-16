import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import './index.css';
import App from './App';
import { ErrorBoundary } from './components/ErrorBoundary';
import reportWebVitals from './reportWebVitals';

// Remove loading overlay only after React has painted (prevents blank flash)
const removeLoadingScreen = () => {
  const el = document.getElementById('loading-screen');
  if (el) el.remove();
};

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

// Remove loading screen after React has painted (prevents blank page)
requestAnimationFrame(() => {
  requestAnimationFrame(() => removeLoadingScreen());
});
// Fallback: remove after 2s in case rAF doesn't fire (e.g. tab in background)
setTimeout(removeLoadingScreen, 2000);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
