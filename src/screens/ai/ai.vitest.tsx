// Gaply — Phase 9a: the PDF viewer and the D18 evidence unit.
//
// The D18 test is the important one: it pins that AI prose cannot reach the
// screen without its evidence, as a COMPONENT INVARIANT rather than a habit.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { EvidenceCard, EvidenceRow, GroundedFinding } from './EvidenceCard';
import { PAGE_WINDOW, PdfViewer } from './PdfViewer';
import { currentWorkerSrc } from './pdfWorker';

/* react-pdf needs a real worker and a canvas; neither exists in jsdom. The mock
   keeps the CONTRACT we care about — how many <Page>s mount, and with which
   pageNumber — and drops the rendering we cannot do here. */
const mockPages = vi.hoisted(() => ({ count: 400 }));
vi.mock('react-pdf', () => ({
  pdfjs: { GlobalWorkerOptions: { workerSrc: '' }, version: '4.8.69' },
  Document: ({ children, onLoadSuccess }: any) => {
    React.useEffect(() => {
      onLoadSuccess?.({ numPages: mockPages.count });
    }, [onLoadSuccess]);
    return <div data-testid="mock-document">{children}</div>;
  },
  Page: ({ pageNumber }: any) => <div data-testid={`mock-page-${pageNumber}`} />,
}));

afterEach(cleanup);

const bytes = async () => new Uint8Array([0x25, 0x50, 0x44, 0x46]);

function row(over: Partial<EvidenceRow> = {}): EvidenceRow {
  return {
    chunkId: 'c7',
    documentId: 42,
    page: 5,
    sourceLabel: 'Smith 2019',
    quote: 'Species richness rose 31 percent under organic management.',
    fileAvailable: true,
    ...over,
  };
}

function finding(over: Partial<GroundedFinding> = {}): GroundedFinding {
  return {
    verdict: 'weak',
    confidence: 0.4,
    explanation: 'The source reports richness but not the magnitude claimed.',
    evidence: [row()],
    ...over,
  };
}

describe('D18 — AI prose never renders without its evidence', () => {
  it('renders the explanation together with its evidence rows', () => {
    render(<EvidenceCard finding={finding()} loadBytes={bytes} />);
    expect(screen.getByTestId('evidence-explanation')).toBeTruthy();
    expect(screen.getByTestId('evidence-quote-0').textContent).toContain('31 percent');
    // and it is marked as model output, not deterministic output
    expect(screen.getByTestId('evidence-ai-marker').textContent).toContain('Local AI');
  });

  /** THE invariant. A finding whose evidence is empty must not show its prose. */
  it('refuses to render an explanation that arrives with no evidence', () => {
    render(<EvidenceCard finding={finding({ evidence: [] })} loadBytes={bytes} />);
    expect(screen.queryByTestId('evidence-explanation')).toBeNull();
    expect(screen.getByTestId('evidence-ungrounded-notice')).toBeTruthy();
    // the prose itself must be nowhere on screen
    expect(screen.queryByText(/reports richness but not the magnitude/)).toBeNull();
  });

  it('shows advisories when the output was accepted with them', () => {
    render(
      <EvidenceCard
        finding={finding({ advisories: ['why is 22 words; the limit is 20'] })}
        loadBytes={bytes}
      />,
    );
    expect(screen.getByTestId('evidence-advisories').textContent).toContain('22 words');
  });

  it('distinguishes verdicts by glyph as well as colour', () => {
    const { unmount } = render(<EvidenceCard finding={finding({ verdict: 'contradicts' })} loadBytes={bytes} />);
    expect(screen.getByTestId('evidence-verdict').textContent).toContain('Contradicted');
    expect(screen.getByTestId('evidence-verdict').textContent).toContain('✕');
    unmount();
    render(<EvidenceCard finding={finding({ verdict: 'strong' })} loadBytes={bytes} />);
    expect(screen.getByTestId('evidence-verdict').textContent).toContain('✓');
  });
});

describe('evidence rows', () => {
  it('renders "page unknown" and is not clickable when the source has no pagination', () => {
    render(<EvidenceCard finding={finding({ evidence: [row({ page: null })] })} loadBytes={bytes} />);
    const btn = screen.getByTestId('evidence-open-0') as HTMLButtonElement;
    expect(btn.textContent).toContain('page unknown');
    expect(btn.textContent).not.toContain('p.');
    expect(btn.disabled).toBe(true);
    // the quote survives — evidence beside prose does not depend on the file
    expect(screen.getByTestId('evidence-quote-0')).toBeTruthy();
  });

  it('renders a file-not-found state instead of a dead click', () => {
    render(
      <EvidenceCard finding={finding({ evidence: [row({ fileAvailable: false })] })} loadBytes={bytes} />,
    );
    expect((screen.getByTestId('evidence-open-0') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('evidence-missing-0').textContent).toContain('moved or renamed');
    expect(screen.getByTestId('evidence-quote-0')).toBeTruthy();
  });

  it('opens the viewer at the row’s page when clicked', async () => {
    render(<EvidenceCard finding={finding({ evidence: [row({ page: 137 })] })} loadBytes={bytes} />);
    fireEvent.click(screen.getByTestId('evidence-open-0'));
    await waitFor(() => expect(screen.getByTestId('pdf-viewer')).toBeTruthy());
    await waitFor(() =>
      expect(screen.getByTestId('pdf-page-indicator').textContent).toBe('Page 137 of 400'),
    );
    // the requested page is mounted, not merely selected
    expect(screen.getByTestId('mock-page-137')).toBeTruthy();
  });
});

describe('PdfViewer', () => {
  it('mounts only a window of pages for a 400-page document', async () => {
    render(
      <PdfViewer open documentId={1} initialPage={200} onClose={() => {}} loadBytes={bytes} />,
    );
    await waitFor(() => expect(screen.getByTestId('pdf-page-indicator').textContent).toContain('200'));

    // every page has a SLOT, so the scrollbar describes the whole document...
    expect(screen.getAllByTestId(/^pdf-page-slot-/).length).toBe(400);
    // ...but only a handful are real pages
    const rendered = screen.getAllByTestId(/^mock-page-/);
    expect(rendered.length).toBe(PAGE_WINDOW * 2 + 1);
    expect(rendered.length).toBeLessThan(20);
    expect(screen.getByTestId('mock-page-200')).toBeTruthy();
    expect(screen.queryByTestId('mock-page-1')).toBeNull();
  });

  it('steps pages with prev/next and clamps at the ends', async () => {
    render(<PdfViewer open documentId={1} initialPage={1} onClose={() => {}} loadBytes={bytes} />);
    await waitFor(() => expect(screen.getByTestId('pdf-page-indicator').textContent).toContain('Page 1'));
    expect((screen.getByTestId('pdf-prev') as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(screen.getByTestId('pdf-next'));
    await waitFor(() =>
      expect(screen.getByTestId('pdf-page-indicator').textContent).toBe('Page 2 of 400'),
    );
  });

  it('surfaces a read failure instead of an empty frame', async () => {
    render(
      <PdfViewer
        open
        documentId={1}
        initialPage={null}
        onClose={() => {}}
        loadBytes={async () => {
          throw new Error('the source file for this document is no longer at /x/y.pdf');
        }}
      />,
    );
    await waitFor(() =>
      expect(screen.getByTestId('pdf-error').textContent).toContain('no longer at'),
    );
  });
});

describe('D43 — the worker is local, never a CDN', () => {
  it('never points workerSrc at a remote host', async () => {
    const { configurePdfWorker } = await import('./pdfWorker');
    configurePdfWorker();
    const src = currentWorkerSrc();
    expect(src).not.toContain('unpkg.com');
    expect(src).not.toContain('cdnjs');
    expect(src.startsWith('http://') && !src.startsWith('http://localhost')).toBe(false);
  });
});
