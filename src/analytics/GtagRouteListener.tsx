import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { isTauri } from '../utils/isTauri';

// Never in the desktop app (Research OS, decision 155): a route there can hold
// a project's id, and the desktop app sends no route analytics at all. With no
// id, neither the script nor any page_view is requested.
const GA_ID = isTauri ? undefined : process.env.REACT_APP_GA_MEASUREMENT_ID;

let gtagScriptRequested = false;

function ensureGtag(): void {
  if (!GA_ID || typeof document === 'undefined' || gtagScriptRequested) return;
  gtagScriptRequested = true;

  const loader = document.createElement('script');
  loader.async = true;
  loader.src = `https://www.googletagmanager.com/gtag/js?id=${encodeURIComponent(GA_ID)}`;
  document.head.appendChild(loader);

  window.dataLayer = window.dataLayer || [];
  window.gtag = function gtag(...args: unknown[]) {
    window.dataLayer!.push(args);
  };
  window.gtag('js', new Date());
  window.gtag('config', GA_ID, { send_page_view: false });
}

/**
 * Optional GA4: set REACT_APP_GA_MEASUREMENT_ID at build time. Sends page_view on route change.
 */
export function GtagRouteListener(): null {
  const location = useLocation();

  useEffect(() => {
    ensureGtag();
  }, []);

  useEffect(() => {
    if (!GA_ID || typeof window.gtag !== 'function') return;
    const path = `${location.pathname}${location.search}`;
    window.gtag('event', 'page_view', {
      page_path: path,
      page_title: typeof document !== 'undefined' ? document.title : '',
    });
  }, [location.pathname, location.search]);

  return null;
}
