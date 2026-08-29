// Gaply — pdf.js worker setup for the DESKTOP app (§11 D43).
//
// The marketing pages (src/components/seo-guides/EthicalAiPdfDeck.tsx) point
// `workerSrc` at unpkg.com. That is fine on the web and wrong twice here:
//
//   1. the desktop CSP is `script-src 'self'` / `worker-src 'self' blob:`,
//      so a CDN worker is blocked outright; and
//   2. R4 — the AI layer makes no network calls. A viewer that fetched its
//      worker from a CDN would make READING A LOCAL FILE require the network.
//
// So the worker is resolved from the installed `pdfjs-dist` and bundled. This
// module is the single place that decides it; nothing else may set workerSrc.
import { pdfjs } from 'react-pdf';

let configured = false;

/** Idempotent: several components may mount, only one worker is configured. */
export function configurePdfWorker(): void {
  if (configured) return;
  // `new URL(..., import.meta.url)` is what lets the bundler emit the worker as
  // a local asset instead of leaving a bare specifier at runtime.
  pdfjs.GlobalWorkerOptions.workerSrc = new URL(
    'pdfjs-dist/build/pdf.worker.min.mjs',
    import.meta.url,
  ).toString();
  configured = true;
}

/** Test seam: assert the worker is local, never a CDN. */
export function currentWorkerSrc(): string {
  return String(pdfjs.GlobalWorkerOptions.workerSrc ?? '');
}
