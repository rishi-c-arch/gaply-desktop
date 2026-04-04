import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Document, Page, pdfjs } from 'react-pdf';

pdfjs.GlobalWorkerOptions.workerSrc = `https://unpkg.com/pdfjs-dist@${pdfjs.version}/build/pdf.worker.min.mjs`;

export interface EthicalAiPdfDeckProps {
  fileUrl: string;
  /** If the PDF fails to load or parse, switch parent to PPTX iframe. */
  onUnavailable: () => void;
}

const EthicalAiPdfDeck: React.FC<EthicalAiPdfDeckProps> = ({ fileUrl, onUnavailable }) => {
  const stageRef = useRef<HTMLDivElement>(null);
  const touchStartX = useRef<number | null>(null);
  const [numPages, setNumPages] = useState(0);
  const [page, setPage] = useState(1);
  const [pageWidth, setPageWidth] = useState(720);

  useEffect(() => {
    const el = stageRef.current;
    if (!el) return undefined;
    const ro = new ResizeObserver(() => {
      const w = el.clientWidth;
      if (w > 0) setPageWidth(Math.min(w - 8, 920));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const goPrev = useCallback(() => {
    setPage((p) => Math.max(1, p - 1));
  }, []);

  const goNext = useCallback(() => {
    setPage((p) => (numPages ? Math.min(numPages, p + 1) : p + 1));
  }, [numPages]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        goPrev();
      }
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        goNext();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [goPrev, goNext]);

  const onTouchStart = (e: React.TouchEvent) => {
    touchStartX.current = e.touches[0].clientX;
  };

  const onTouchEnd = (e: React.TouchEvent) => {
    if (touchStartX.current == null) return;
    const dx = e.changedTouches[0].clientX - touchStartX.current;
    touchStartX.current = null;
    if (dx > 56) goPrev();
    else if (dx < -56) goNext();
  };

  const onDocumentLoadSuccess = useCallback(({ numPages: n }: { numPages: number }) => {
    setNumPages(n);
    setPage(1);
  }, []);

  const onDocumentLoadError = useCallback(() => {
    onUnavailable();
  }, [onUnavailable]);

  const requestFs = () => {
    const el = stageRef.current;
    if (!el?.requestFullscreen) return;
    el.requestFullscreen().catch(() => {});
  };

  const jumpTo = (raw: string) => {
    const n = parseInt(raw, 10);
    if (!Number.isFinite(n) || n < 1) return;
    if (numPages) setPage(Math.min(n, numPages));
    else setPage(n);
  };

  return (
    <div className="ethical-ai-pdf-deck">
      <div className="ethical-ai-pdf-toolbar" role="toolbar" aria-label="Slide controls">
        <button type="button" onClick={goPrev} disabled={page <= 1}>
          Previous
        </button>
        <span className="ethical-ai-pdf-counter" aria-live="polite">
          Slide {page}
          {numPages ? ` / ${numPages}` : ''}
        </span>
        <button type="button" onClick={goNext} disabled={numPages === 0 || page >= numPages}>
          Next
        </button>
        <label className="ethical-ai-pdf-jump">
          Go to
          <input
            type="number"
            min={1}
            max={numPages || undefined}
            placeholder="#"
            aria-label="Slide number (press Enter)"
            onKeyDown={(e) => {
              if (e.key === 'Enter') jumpTo((e.target as HTMLInputElement).value);
            }}
          />
        </label>
        <button type="button" className="ethical-ai-pdf-fs" onClick={requestFs}>
          Full screen
        </button>
      </div>

      <div
        ref={stageRef}
        className="ethical-ai-pdf-stage"
        onTouchStart={onTouchStart}
        onTouchEnd={onTouchEnd}
        role="region"
        aria-label="Presentation slides — swipe or use arrow keys"
      >
        <Document
          file={fileUrl}
          onLoadSuccess={onDocumentLoadSuccess}
          onLoadError={onDocumentLoadError}
          loading={<div className="ethical-ai-pdf-loading">Opening slides…</div>}
          className="ethical-ai-pdf-document"
        >
          <div key={page} className="ethical-ai-pdf-page-layer">
            <Page
              pageNumber={page}
              width={pageWidth}
              renderTextLayer={false}
              renderAnnotationLayer={false}
              className="ethical-ai-pdf-page"
            />
          </div>
        </Document>
      </div>

      <p className="ethical-ai-pdf-hint">
        Tip: keyboard ← → or swipe.{' '}
        <a href={fileUrl} download className="ethical-ai-pdf-dl">
          Download PDF
        </a>
      </p>
    </div>
  );
};

export default EthicalAiPdfDeck;
