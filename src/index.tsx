import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import './index.css';
import App from './App';
import reportWebVitals from './reportWebVitals';

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
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>
);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();
