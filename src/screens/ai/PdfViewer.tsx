// Gaply — the in-app PDF viewer (§11 D41/D42/D43).
//
// Exists because D18's rule — AI prose never renders without its evidence, with
// an affordance to check the source — needs somewhere to check it. Nothing in
// the app could open a source file at a page before this.
//
// # Virtualized, and why that is not an optimisation
//
// A thesis is routinely 200–400 pages. `<Page>` renders to a canvas, so mounting
// every page would allocate hundreds of canvases at once and hang the webview.
// Only a WINDOW around the current page is mounted; the rest are spacer divs of
// the same height, so the scrollbar still describes the whole document.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Document, Page } from 'react-pdf';
import { Modal } from '../../design-system/Modal';
import { aiBridge } from './aiBridge';
import { configurePdfWorker } from './pdfWorker';

/** Pages mounted either side of the current one. */
export const PAGE_WINDOW = 2;

/** Fallback height for a page not yet measured, in CSS pixels. */
const ESTIMATED_PAGE_HEIGHT = 1100;

export interface PdfViewerProps {
  open: boolean;
  documentId: number;
  /** 1-based. `null` means "no particular page" — open at the start. */
  initialPage: number | null;
  title?: string;
  onClose: () => void;
  /** Test seam: supply bytes directly instead of going through IPC. */
  loadBytes?: (documentId: number) => Promise<Uint8Array>;
}

export const PdfViewer: React.FC<PdfViewerProps> = ({
  open,
  documentId,
  initialPage,
  title,
  onClose,
  loadBytes,
}) => {
  const [bytes, setBytes] = useState<Uint8Array | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [numPages, setNumPages] = useState(0);
  const [current, setCurrent] = useState(initialPage ?? 1);
  const scrollRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    configurePdfWorker();
  }, []);

  // Load on open. Not cached: a thesis is tens of megabytes and holding it
  // after close would keep that alive for a modal nobody has open.
  useEffect(() => {
    if (!open) {
      setBytes(null);
      setError(null);
      setNumPages(0);
      return;
    }
    let cancelled = false;
    const load = loadBytes ?? ((id: number) => aiBridge.documentBytes(id));
    load(documentId)
      .then((b) => {
        if (!cancelled) setBytes(b);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [open, documentId, loadBytes]);

  useEffect(() => {
    if (open) setCurrent(initialPage ?? 1);
  }, [open, initialPage]);

  /** THE api the evidence component calls. 1-based; clamped, never thrown. */
  const jumpToPage = useCallback(
    (page: number) => {
      const target = Math.min(Math.max(1, Math.floor(page)), Math.max(1, numPages || page));
      setCurrent(target);
      const el = scrollRef.current?.querySelector<HTMLElement>(`[data-page-anchor="${target}"]`);
      // Optional call: scrollIntoView is absent in jsdom, and a viewer that
      // threw while merely selecting a page would be worse than one that does
      // not smooth-scroll under test.
      el?.scrollIntoView?.({ block: 'start' });
    },
    [numPages],
  );

  // Land on the requested page as soon as the document reports its length.
  useEffect(() => {
    if (numPages > 0 && initialPage) jumpToPage(initialPage);
  }, [numPages, initialPage, jumpToPage]);

  const windowed = useMemo(() => {
    const lo = Math.max(1, current - PAGE_WINDOW);
    const hi = Math.min(numPages, current + PAGE_WINDOW);
    return { lo, hi };
  }, [current, numPages]);

  const file = useMemo(() => (bytes ? { data: bytes } : null), [bytes]);

  return (
    <Modal open={open} title={title ?? 'Source document'} onClose={onClose}>
      <div className="gds-root" data-testid="pdf-viewer">
        <div className="gds-pdf__toolbar">
          <button
            type="button"
            className="gds-btn gds-btn--ghost"
            onClick={() => jumpToPage(current - 1)}
            disabled={current <= 1}
            data-testid="pdf-prev"
          >
            ‹ Previous
          </button>
          <span data-testid="pdf-page-indicator">
            {numPages > 0 ? `Page ${current} of ${numPages}` : 'Loading…'}
          </span>
          <button
            type="button"
            className="gds-btn gds-btn--ghost"
            onClick={() => jumpToPage(current + 1)}
            disabled={numPages === 0 || current >= numPages}
            data-testid="pdf-next"
          >
            Next ›
          </button>
        </div>

        {error && (
          <p className="gds-pdf__error" data-testid="pdf-error">
            {error}
          </p>
        )}

        <div className="gds-pdf__scroll" ref={scrollRef} data-testid="pdf-scroll">
          {file && (
            <Document
              file={file}
              onLoadSuccess={({ numPages: n }: { numPages: number }) => setNumPages(n)}
              onLoadError={(e: Error) => setError(e.message)}
              loading={<p>Opening document…</p>}
            >
              {Array.from({ length: numPages }, (_, i) => i + 1).map((n) => {
                const mounted = n >= windowed.lo && n <= windowed.hi;
                return (
                  <div key={n} data-page-anchor={n} data-testid={`pdf-page-slot-${n}`}>
                    {mounted ? (
                      <Page pageNumber={n} renderTextLayer={false} renderAnnotationLayer={false} />
                    ) : (
                      // A spacer, not a page: the scrollbar keeps describing the
                      // whole document without allocating a canvas per page.
                      <div
                        style={{ height: ESTIMATED_PAGE_HEIGHT }}
                        data-testid={`pdf-page-spacer-${n}`}
                      />
                    )}
                  </div>
                );
              })}
            </Document>
          )}
        </div>
      </div>
    </Modal>
  );
};

export default PdfViewer;
