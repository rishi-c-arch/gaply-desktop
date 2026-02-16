import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import './index.css';
import App from './App';
import { ErrorBoundary } from './components/ErrorBoundary';
import reportWebVitals from './reportWebVitals';

// Catch uncaught errors and show them (helps debug blank page)
window.addEventListener('error', (e) => {
  const root = document.getElementById('root');
  if (root && root.innerHTML.length < 200) {
    root.innerHTML = `<div style="padding:48px 24px;font-family:system-ui;max-width:600px;margin:0 auto;color:#333">
      <h1 style="color:#c00">Something went wrong</h1>
      <pre style="background:#f5f5f5;padding:16px;overflow:auto;font-size:12px">${(e.error && e.error.stack) || e.message}</pre>
      <p><a href="/" style="color:#007AFF">Go home</a></p>
    </div>`;
  }
});
window.addEventListener('unhandledrejection', (e) => {
  const root = document.getElementById('root');
  if (root && root.innerHTML.length < 200) {
    root.innerHTML = `<div style="padding:48px 24px;font-family:system-ui;max-width:600px;margin:0 auto;color:#333">
      <h1 style="color:#c00">Loading error</h1>
      <pre style="background:#f5f5f5;padding:16px;overflow:auto;font-size:12px">${String(e.reason)}</pre>
      <p><a href="/" style="color:#007AFF">Go home</a></p>
    </div>`;
  }
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

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
