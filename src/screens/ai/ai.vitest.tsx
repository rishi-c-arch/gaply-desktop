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
  Page: ({ pageNumber, renderTextLayer, renderAnnotationLayer }: any) => (
    <div
      data-testid={`mock-page-${pageNumber}`}
      data-text-layer={String(!!renderTextLayer)}
      data-annotation-layer={String(!!renderAnnotationLayer)}
    />
  ),
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

  /* ---- "checked, and none of them support you" is an ANSWER, not a refusal --- */

  it('renders a checked-but-unsupported verdict against what was examined', () => {
    // Regression: insufficient_evidence with zero supporting_chunks is the one
    // verdict the engine's validator lets cite nothing, and the UI met it with
    // "this assessment cannot be shown". That threw away a true, useful and
    // fully grounded result and looked like a malfunction.
    render(
      <EvidenceCard
        finding={finding({
          verdict: 'insufficient_evidence',
          evidence: [],
          chunksSent: 12,
          examined: [row({ chunkId: 'c3', page: 8, quote: 'Soil pH was measured monthly.' })],
        })}
        loadBytes={bytes}
      />,
    );
    expect(screen.queryByTestId('evidence-ungrounded-notice')).toBeNull();
    // §11 D108. Was "Insufficient evidence" — a grade. It is now the FACT:
    // the model cited no passage, which is why what was examined is shown.
    expect(screen.getByTestId('evidence-verdict').textContent).toContain(
      'No supporting passage cited',
    );
    expect(screen.getByTestId('evidence-searched').textContent).toBe(
      'Checked 12 retrieved passages from this source; none support the claim.',
    );
    // The explanation IS shown — but only underneath that account.
    const card = screen.getByTestId('evidence-card');
    const searched = screen.getByTestId('evidence-searched');
    const explanation = screen.getByTestId('evidence-explanation');
    expect(
      searched.compareDocumentPosition(explanation) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(card.textContent).toContain('reports richness but not the magnitude');

    // The passages examined stay verifiable and page-linked, exactly like cited ones.
    expect(screen.getByTestId('evidence-examined-quote-0').textContent).toContain('Soil pH');
    const open = screen.getByTestId('evidence-examined-open-0') as HTMLButtonElement;
    expect(open.disabled).toBe(false);
    expect(open.textContent).toContain('p.8');
  });

  it('says how many of the retrieved passages it is showing', () => {
    render(
      <EvidenceCard
        finding={finding({ verdict: 'insufficient_evidence', evidence: [], chunksSent: 12, examined: [row(), row({ chunkId: 'c9' })] })}
        loadBytes={bytes}
      />,
    );
    expect(screen.getByTestId('evidence-examined-label').textContent).toBe(
      'What was examined (top 2 of 12):',
    );
  });

  it('distinguishes "nothing was retrieved" from "nothing supported the claim"', () => {
    // D15: the engine never ran the model. Different fact, different fix
    // (index the document), so it must not read as a judgement.
    render(
      <EvidenceCard
        finding={finding({ verdict: 'no_evidence', evidence: [], examined: [], chunksSent: 0 })}
        loadBytes={bytes}
      />,
    );
    expect(screen.getByTestId('evidence-searched').textContent).toContain(
      'No passages were retrieved',
    );
    expect(screen.queryByTestId('evidence-examined-label')).toBeNull();
  });

  it.each(['strong', 'partial', 'weak', 'contradicts'] as const)(
    'still REFUSES a %s verdict that cites nothing, even with passages to show',
    (verdict) => {
      // The actual invariant. These all assert something about a particular
      // passage; showing "what was searched" instead would let a claim of
      // support borrow the credibility of text it never cited.
      render(
        <EvidenceCard
          finding={finding({ verdict, evidence: [], chunksSent: 12, examined: [row()] })}
          loadBytes={bytes}
        />,
      );
      expect(screen.getByTestId('evidence-ungrounded-notice')).toBeTruthy();
      expect(screen.queryByTestId('evidence-explanation')).toBeNull();
      expect(screen.queryByTestId('evidence-searched')).toBeNull();
      expect(screen.queryByText(/reports richness but not the magnitude/)).toBeNull();
    },
  );

  it('shows advisories when the output was accepted with them', () => {
    render(
      <EvidenceCard
        finding={finding({ advisories: ['why is 22 words; the limit is 20'] })}
        loadBytes={bytes}
      />,
    );
    expect(screen.getByTestId('evidence-advisories').textContent).toContain('22 words');
  });

  /** §11 D108. The card no longer grades. It used to badge "✓ Supported" and
   *  "✕ Contradicted"; the verdict behind those measured as a constant `weak`
   *  (14 of 14 valid outputs across two runs), so "Weak support" was the only
   *  badge a reader ever actually saw — and it was not a reading of their
   *  sentence. */
  it('does not grade the sentence, whatever verdict the model returned', () => {
    for (const v of ['strong', 'partial', 'weak', 'contradicts'] as const) {
      const { unmount } = render(<EvidenceCard finding={finding({ verdict: v })} loadBytes={bytes} />);
      const badge = screen.getByTestId('evidence-verdict').textContent ?? '';
      expect(badge).toContain('Source passages found');
      // None of the grading vocabulary, and neither tick nor cross.
      expect(badge).not.toMatch(/Supported|Contradicted|Weak support|Partially/);
      expect(badge).not.toContain('✓');
      expect(badge).not.toContain('✕');
      // And it says so in words, not only by omission.
      expect(screen.getByTestId('evidence-no-grade').textContent).toMatch(/does not grade/i);
      unmount();
    }
  });

  /** The one distinction that survives is DETERMINISTIC: nothing was retrieved
   *  and no model ran. That is a fact about the engine, not a judgement. */
  it('still distinguishes "no evidence retrieved", which no model produced', () => {
    render(<EvidenceCard finding={finding({ verdict: 'no_evidence', evidence: [] })} loadBytes={bytes} />);
    expect(screen.getByTestId('evidence-verdict').textContent).toContain('No evidence retrieved');
  });

  /** §11 D108. The decomposition is what measured correct, so it is shown. */
  it('shows the claim decomposition, marked as the model\u2019s reading', () => {
    render(
      <EvidenceCard
        finding={finding({
          claimElements: [
            { element: '46% macro-F1', status: 'found' },
            { element: '95% accuracy', status: 'absent' },
          ],
        })}
        loadBytes={bytes}
      />,
    );
    const el = screen.getByTestId('evidence-claim-elements').textContent ?? '';
    expect(el).toContain('46% macro-F1 — in the source');
    expect(el).toContain('95% accuracy — NOT in the passages read');
    expect(el).toMatch(/not Gaply\u2019s conclusion/);
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
  it('renders a SELECTABLE text layer, so a sentence can be copied out', async () => {
    // The workflow this viewer serves: find the sentence in the source, copy
    // it, paste it into the claim box. A canvas alone cannot be selected.
    // Annotations stay off — links and widgets are not text.
    render(
      <PdfViewer open documentId={1} initialPage={200} onClose={() => {}} loadBytes={bytes} />,
    );
    await waitFor(() => expect(screen.getByTestId('mock-page-200')).toBeTruthy());
    const page = screen.getByTestId('mock-page-200');
    expect(page.getAttribute('data-text-layer')).toBe('true');
    expect(page.getAttribute('data-annotation-layer')).toBe('false');
  });

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
