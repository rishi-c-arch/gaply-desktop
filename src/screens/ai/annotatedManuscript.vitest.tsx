// §11 D92. The annotated view: status mapping, the two-cue rule, the omit-
// rather-than-approximate rule, and the .docx refusal.
import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, waitFor, cleanup } from '@testing-library/react';

// No `globals` in vitest.config.ts, so RTL's auto-cleanup never registers and
// renders leak between tests.
afterEach(cleanup);

vi.mock('react-pdf', () => ({
  Document: ({ children, onLoadSuccess }: any) => {
    React.useEffect(() => { void onLoadSuccess?.({ numPages: 1 }); }, [onLoadSuccess]);
    return <div data-testid="pdf-doc">{children}</div>;
  },
  Page: ({ pageNumber }: any) => <div data-testid={`pdf-page-${pageNumber}`} />,
}));
vi.mock('./pdfWorker', () => ({ configurePdfWorker: () => {} }));
vi.mock('react-pdf/dist/esm/Page/TextLayer.css', () => ({}));

import { AnnotatedManuscript, AnnotatedUnavailable, annotatable } from './AnnotatedManuscript';
import { statusOf, STATUS_STYLE } from './annotationStatus';
import { PageText } from './anchorSentences';

describe('statusOf — what gets a colour, and what must not', () => {
  it('maps each verdict to its status', () => {
    expect(statusOf('citation_support', 'strong')).toBe('checked');
    for (const v of ['partial', 'weak', 'contradicts', 'insufficient_evidence']) {
      expect(statusOf('citation_support', v)).toBe('weak');
    }
    expect(statusOf('unverifiable', null)).toBe('blocked');
    expect(statusOf('citation_need', 'needs_citation')).toBe('advisory');
  });

  /** §11 D78's rule carried here: 65 declinations must not become 65 marks. */
  it('gives NO highlight to a sentence judged not to need one', () => {
    expect(statusOf('citation_need', 'no_citation_needed')).toBeNull();
    expect(statusOf('citation_support', undefined)).toBeNull();
  });
});

describe('the two-cue rule — colour is never the only signal', () => {
  it('every status carries a word and an edge treatment, and they differ', () => {
    const labels = new Set<string>();
    const fills = new Set<string>();
    for (const s of ['checked', 'weak', 'blocked', 'advisory'] as const) {
      const st = STATUS_STYLE[s];
      expect(st.label.trim()).not.toBe('');
      expect(st.meaning.trim()).not.toBe('');
      expect(st.action.trim()).not.toBe('');
      labels.add(st.label);
      fills.add(st.fill);
    }
    expect(labels.size).toBe(4); // distinguishable without colour
    expect(fills.size).toBe(4);
  });

  /** §11 D89, carried into the annotated view: a 43%-precision suggestion is
   *  visually subordinate to an evidence-backed finding here too. */
  it('the suggestion is the only dashed one, and the only light one', () => {
    expect(STATUS_STYLE.advisory.edge).toBe('dashed');
    expect(STATUS_STYLE.advisory.weight).toBe('light');
    for (const s of ['checked', 'weak', 'blocked'] as const) {
      expect(STATUS_STYLE[s].edge).toBe('solid');
      expect(STATUS_STYLE[s].weight).toBe('full');
    }
  });
});

const PAGE: PageText = {
  page: 1,
  width: 600,
  height: 800,
  items: 'Organic farming increases soil microbial biomass by a third here.'
    .split(' ')
    .map((w, i) => ({ str: w + ' ', x: 50 + i * 30, y: 700, w: 28, h: 10 })),
};

const items = [
  { seq: 1, kind: 'citation_support', page: 1, sentence: 'Organic farming increases soil microbial biomass by a third here.', result: { output: { verdict: 'strong' } } },
  { seq: 2, kind: 'citation_need', page: 1, sentence: 'A sentence that is printed nowhere on this page at all.', result: { output: { verdict: 'needs_citation' } } },
  { seq: 3, kind: 'citation_need', page: 1, sentence: 'Another uncited line entirely absent from the page text.', result: { output: { verdict: 'no_citation_needed' } } },
];

describe('AnnotatedManuscript', () => {
  const load = {
    loadBytes: async () => new Uint8Array([1]),
    loadPages: async () => [PAGE],
  };

  it('draws only what it located, and never guesses at the rest', async () => {
    render(<AnnotatedManuscript path="/t.pdf" items={items as any} {...load} />);
    // seq 1 is on the page -> highlighted.
    await waitFor(() => expect(screen.getAllByTestId('annot-hl-1').length).toBeGreaterThan(0));
    expect(screen.getAllByTestId('annot-hl-1')[0].getAttribute('data-status')).toBe('checked');
    // seq 2 is judged but absent from the page -> NO highlight, listed instead.
    expect(screen.queryByTestId('annot-hl-2')).toBeNull();
    expect(screen.getByTestId('annot-unplaced-2')).toBeTruthy();
    // seq 3 needs no citation -> not a finding, so not drawn and not listed.
    expect(screen.queryByTestId('annot-hl-3')).toBeNull();
    expect(screen.queryByTestId('annot-unplaced-3')).toBeNull();
  });

  it('says how many it could show, so a gap is never silent', async () => {
    render(<AnnotatedManuscript path="/t.pdf" items={items as any} {...load} />);
    // Wait for the VALUE, not the element: the count renders immediately and
    // only becomes true once the page text has been read.
    await waitFor(() =>
      expect(screen.getByTestId('annot-counts').textContent).toContain('1 of 2 judged sentences'),
    );
    expect(screen.getByTestId('annot-counts').textContent).toMatch(/1 could not be located/);
  });

  it('explains every highlight in plain English, with an action', async () => {
    render(<AnnotatedManuscript path="/t.pdf" items={items as any} {...load} />);
    await waitFor(() => expect(screen.getByTestId('annot-entry-1')).toBeTruthy());
    const e = screen.getByTestId('annot-entry-1').textContent ?? '';
    expect(e).toContain('verified with evidence'); // the WORD, not just a colour
    expect(e).toContain('page 1');
    expect(e).toMatch(/What to do:/);
  });

  it('the unplaced entry says it was judged and why it is not shown', async () => {
    render(<AnnotatedManuscript path="/t.pdf" items={items as any} {...load} />);
    await waitFor(() => expect(screen.getByTestId('annot-unplaced-2')).toBeTruthy());
    const t = screen.getByTestId('annot-unplaced-2').textContent ?? '';
    expect(t).toMatch(/could not show you where it sits/);
    expect(t).toMatch(/finding below is unaffected/);
    expect(t).toMatch(/What to do:/);
  });

  it('shows a legend naming all four statuses', async () => {
    render(<AnnotatedManuscript path="/t.pdf" items={items as any} {...load} />);
    await waitFor(() => expect(screen.getByTestId('annot-legend')).toBeTruthy());
    for (const s of ['checked', 'weak', 'blocked', 'advisory']) {
      expect(screen.getByTestId(`annot-legend-${s}`).textContent).toBe(STATUS_STYLE[s as 'checked'].label);
    }
  });

  it('surfaces a load failure instead of rendering an empty page', async () => {
    render(
      <AnnotatedManuscript
        path="/t.docx"
        items={items as any}
        loadBytes={async () => { throw new Error('the annotated view needs a PDF'); }}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('annotated-error').textContent).toMatch(/needs a PDF/));
  });
});

describe('AnnotatedUnavailable — the .docx refusal', () => {
  it('says WHY, and points at the honest alternative', () => {
    render(<AnnotatedUnavailable />);
    const t = screen.getByTestId('annotated-unavailable').textContent ?? '';
    expect(t).toMatch(/don’t record where their pages break/);
    expect(t).toMatch(/paragraph/);          // the alternative we DO have
    expect(t).toMatch(/Export the manuscript as a PDF/);
  });
});

describe('annotatable', () => {
  it('keeps only items that have a status', () => {
    expect(annotatable(items as any).map((x) => x.it.seq)).toEqual([1, 2]);
  });
});
