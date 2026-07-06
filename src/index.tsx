import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter, HashRouter } from 'react-router-dom';
import './fonts';
import './index.css';
import App from './App';
import reportWebVitals from './reportWebVitals';
import * as serviceWorkerRegistration from './serviceWorkerRegistration';
import isTauri from './utils/isTauri';

// Defense-in-depth against prototype pollution: freeze Object.prototype so a
// malicious payload (e.g. from an uploaded spreadsheet parsed by SheetJS)
// cannot inject shared properties onto every object. Runs after module imports
// have initialized, so it only blocks runtime writes — legitimate library
// setup at import time is unaffected.
Object.freeze(Object.prototype);

// Tauri: history-API routing doesn't survive the custom protocol, so the
// desktop build uses HashRouter. Full-page navigations elsewhere in the app
// (window.location.href = '/login') land on a path URL — convert it to the
// equivalent hash route before React mounts.
if (isTauri) {
  const { pathname, search } = window.location;
  if (pathname !== '/' && pathname !== '/index.html' && !window.location.hash) {
    window.location.replace(`/#${pathname}${search}`);
  }
}

const Router = isTauri ? HashRouter : BrowserRouter;

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
    <Router>
      <App />
    </Router>
  </React.StrictMode>
);

reportWebVitals();

// Service workers are unsupported/unreliable on Tauri's custom protocol and
// would cache stale desktop builds — web only.
if (isTauri) {
  serviceWorkerRegistration.unregister();
} else {
  serviceWorkerRegistration.register();
}
