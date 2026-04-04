import React, { Suspense, useEffect, useState } from 'react';
import EthicalAiPptxIframe from './EthicalAiPptxIframe';
import { ETHICAL_AI_PDF_PATH, ETHICAL_AI_PPTX_PATH } from './ethicalAiGuideTopics';

const PRODUCTION_ORIGIN = 'https://www.gaply.in';

const EthicalAiPdfDeck = React.lazy(() => import('./EthicalAiPdfDeck'));

export type DeckMode = 'checking' | 'pdf' | 'pptx';

/**
 * Prefer in-page PDF (smooth prev/next). If no PDF is on the server, fall back to Office/Google iframe for .pptx.
 */
const EthicalAiDeckExperience: React.FC = () => {
  const [urls, setUrls] = useState<{ pdfUrl: string; pptxUrl: string } | null>(null);
  const [mode, setMode] = useState<DeckMode>('checking');

  useEffect(() => {
    const host = window.location.hostname;
    const useProd =
      host === 'localhost' || host === '127.0.0.1' || host.endsWith('.local');
    const origin = useProd ? PRODUCTION_ORIGIN : window.location.origin;
    const pdfFromEnv = process.env.REACT_APP_ETHICAL_AI_PDF_URL?.trim();
    const pptxFromEnv = process.env.REACT_APP_ETHICAL_AI_PPTX_URL?.trim();
    setUrls({
      pdfUrl: pdfFromEnv || `${origin}${ETHICAL_AI_PDF_PATH}`,
      pptxUrl: pptxFromEnv || `${origin}${ETHICAL_AI_PPTX_PATH}`,
    });
  }, []);

  useEffect(() => {
    if (!urls?.pdfUrl) return undefined;
    let cancelled = false;
    (async () => {
      try {
        const r = await fetch(urls.pdfUrl, { method: 'HEAD', cache: 'no-store' });
        if (cancelled) return;
        setMode(r.ok ? 'pdf' : 'pptx');
      } catch {
        if (!cancelled) setMode('pptx');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [urls?.pdfUrl]);

  if (!urls) {
    return <div className="ethical-ai-deck-skeleton" aria-busy="true" />;
  }

  if (mode === 'checking') {
    return <div className="ethical-ai-deck-skeleton" aria-busy="true" />;
  }

  if (mode === 'pdf') {
    return (
      <Suspense fallback={<div className="ethical-ai-deck-skeleton" aria-busy="true" />}>
        <EthicalAiPdfDeck fileUrl={urls.pdfUrl} onUnavailable={() => setMode('pptx')} />
      </Suspense>
    );
  }

  return <EthicalAiPptxIframe absoluteUrl={urls.pptxUrl} />;
};

export default EthicalAiDeckExperience;
