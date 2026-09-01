// Gaply — the per-citation slice of the thesis audit, in the panel.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { CitationAiPanel } from './CitationAiPanel';

vi.mock('react-pdf', () => ({
  pdfjs: { GlobalWorkerOptions: { workerSrc: '' }, version: '4.8.69' },
  Document: ({ children }: any) => <div>{children}</div>,
  Page: ({ pageNumber }: any) => <div data-testid={`mock-page-${pageNumber}`} />,
}));

afterEach(cleanup);
beforeEach(() => sessionStorage.clear());

const SENTENCES = [
  { seq: 3, page: 4, marker: '(Smith, 2019)', sentence: 'Organic management increased richness by 31 percent (Smith, 2019).' },
  { seq: 9, page: 5, marker: '(Smith, 2019)', sentence: 'The magnitude depended on baseline carbon (Smith, 2019).' },
  { seq: 14, page: 6, marker: '(Smith, 2019)', sentence: 'Effects on microbial diversity were smaller (Smith, 2019).' },
];

function preview(over: Record<string, any> = {}) {
  return {
    libraryId: 'lib-smith',
    documentId: 7,
    unverifiableReason: null,
    sentences: SENTENCES,
    totalSentences: 412,
    documentTypesSupported: ['pdf', 'docx', 'txt'],
    ...over,
  };
}

function bridge(over: Record<string, any> = {}) {
  return {
    citationNeed: async () => ({}),
    citationSupport: async () => ({}),
    cancelGeneration: async () => {},
    documentSource: async () => ({ documentId: 7, path: '/x.pdf', exists: true, extension: 'pdf' }),
    citationAuditPreview: async () => preview(),
    citationAuditStart: async () => ({ jobId: 1 }),
    jobResults: async () => ({ items: [] }),
    cancelJob: async () => true,
    ...over,
  } as any;
}

const panel = (props: Record<string, any> = {}) => (
  <CitationAiPanel
    citationId="lib-smith"
    sentence=""
    documentId={7}
    citedSource="Smith 2019"
    aiInstalled
    pickManuscript={async () => '/Users/rishi/Desktop/chapter 1 .pdf'}
    {...props}
    bridge={props.bridge ?? bridge()}
  />
);

describe('per-citation slice — the panel finds the claims itself', () => {
  it('leads with import when no manuscript is loaded, manual box beneath', () => {
    render(panel());
    expect(screen.getByTestId('slice-no-manuscript').textContent).toMatch(
      /Import your manuscript to check the sentences that cite this source/,
    );
    expect(screen.getByTestId('slice-import')).toBeTruthy();
    // The manual path is still there — demoted, not removed.
    expect(screen.getByTestId('slice-manual-label').textContent).toBe('or check a single sentence');
    expect(screen.getByTestId('ai-claim')).toBeTruthy();
  });

  it('finds three citing sentences and does NOT run a model until confirmed', async () => {
    const citationAuditPreview = vi.fn(async () => preview());
    const citationAuditStart = vi.fn(async () => ({ jobId: 1 }));
    render(panel({ bridge: bridge({ citationAuditPreview, citationAuditStart }) }));

    fireEvent.click(screen.getByTestId('slice-import'));
    await waitFor(() => expect(screen.getByTestId('slice-count')).toBeTruthy());

    expect(screen.getByTestId('slice-count').textContent).toBe(
      '3 sentences in your manuscript cite this source.',
    );
    expect(screen.getAllByTestId(/^slice-sentence-/).length).toBe(3);
    expect(screen.getByTestId('slice-quote-0').textContent).toMatch(/31 percent/);

    // §11 D40: the pre-pass is deterministic and runs NO model. Nothing has
    // been queued at this point — the list is free, the checking is not.
    expect(citationAuditPreview).toHaveBeenCalledWith('lib-smith', '/Users/rishi/Desktop/chapter 1 .pdf');
    expect(citationAuditStart).not.toHaveBeenCalled();

    // The cost is stated before it is spent.
    expect(screen.getByTestId('slice-confirm').textContent).toMatch(/Check all 3 — about/);
  });

  it('queues the slice on confirm and renders each result through the D18 unit', async () => {
    let emit: (ev: any) => void = () => {};
    const citationAuditStart = vi.fn(async (_c: string, _p: string, onEvent: any) => {
      emit = onEvent;
      return { jobId: 42 };
    });
    const jobResults = vi.fn(async () => ({
      items: [
        {
          seq: 3,
          result: {
            output: {
              verdict: 'weak',
              confidence: 0.4,
              explanation: 'Reports richness but not the magnitude.',
              supporting_chunks: [{ chunk_id: 'c7', page: 4, quote: 'Richness rose 31 percent.' }],
            },
          },
        },
      ],
    }));
    render(panel({ bridge: bridge({ citationAuditStart, jobResults }) }));

    fireEvent.click(screen.getByTestId('slice-import'));
    await waitFor(() => expect(screen.getByTestId('slice-confirm')).toBeTruthy());
    fireEvent.click(screen.getByTestId('slice-confirm'));
    await waitFor(() => expect(citationAuditStart).toHaveBeenCalled());
    expect(citationAuditStart.mock.calls[0][0]).toBe('lib-smith');

    // Progress streams through the SHARED job runner's events.
    await waitFor(() => expect(emit).toBeTruthy());
    emit({ jobId: 42, completed: 1, total: 3, currentCategory: 'citation_support', latestItemSummary: 'weak' });
    await waitFor(() => expect(screen.getByTestId('slice-progress')).toBeTruthy());
    expect(screen.getByTestId('slice-progress').textContent).toMatch(/1 of 3 checked/);

    // The result renders as evidence, not prose alone.
    await waitFor(() => expect(screen.getByTestId('evidence-card')).toBeTruthy());
    expect(screen.getByTestId('evidence-quote-0').textContent).toContain('31 percent');
    expect(screen.getByTestId('evidence-open-0').textContent).toContain('p.4');
  });

  it('an unlinked source shows the unverifiable state, not a check that cannot run', async () => {
    render(
      panel({
        bridge: bridge({
          citationAuditPreview: async () =>
            preview({
              documentId: null,
              unverifiableReason: 'no indexed document is linked to this work: Organic Management',
            }),
        }),
      }),
    );
    fireEvent.click(screen.getByTestId('slice-import'));
    await waitFor(() => expect(screen.getByTestId('slice-unverifiable')).toBeTruthy());

    // The sentences ARE still listed — they genuinely cite this source.
    expect(screen.getAllByTestId(/^slice-sentence-/).length).toBe(3);
    expect(screen.getByTestId('slice-unverifiable').textContent).toMatch(/no indexed document is linked/);
    // …and no confirm button, because there is nothing to check against.
    expect(screen.queryByTestId('slice-confirm')).toBeNull();
  });

  it('says plainly when nothing in the manuscript cites this source', async () => {
    render(panel({ bridge: bridge({ citationAuditPreview: async () => preview({ sentences: [] }) }) }));
    fireEvent.click(screen.getByTestId('slice-import'));
    await waitFor(() => expect(screen.getByTestId('slice-count')).toBeTruthy());
    expect(screen.getByTestId('slice-count').textContent).toBe(
      'No sentences in your manuscript cite this source (of 412 checked).',
    );
    expect(screen.queryByTestId('slice-confirm')).toBeNull();
  });

  it('links the citation_need question to the manuscript, keeping only the manual fallback here', () => {
    // §11 D54: "does this sentence need a citation?" judges UNCITED sentences,
    // which by definition cite nothing — it is a question about the manuscript,
    // not about this source. The audit already queues exactly those items
    // through the same planner, so the panel links to it rather than copying it.
    const onOpenAudit = vi.fn();
    render(panel({ onOpenAudit }));
    fireEvent.click(screen.getByTestId('slice-need-open'));
    expect(onOpenAudit).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('slice-need-link').textContent).toMatch(/whole manuscript/);
    // The single-sentence mode survives as the fallback it now is.
    expect(screen.getByTestId('ai-check-need')).toBeTruthy();
  });

  it('remembers the manuscript for the session, so the next citation does not re-ask', async () => {
    const { unmount } = render(panel());
    fireEvent.click(screen.getByTestId('slice-import'));
    await waitFor(() => expect(screen.getByTestId('slice-count')).toBeTruthy());
    unmount();

    render(panel({ citationId: 'lib-other' }));
    expect(screen.queryByTestId('slice-no-manuscript')).toBeNull();
    expect(screen.getByTestId('slice-manuscript').textContent).toMatch(/chapter 1 \.pdf/);
    expect(screen.getByTestId('slice-check')).toBeTruthy();
  });

  it('names the parse and the check differently, so the two cannot be confused', async () => {
    // These are two different operations on one panel: `slice-check` runs a
    // deterministic parse that spends no model time, and the claim box below
    // runs an actual check. They used to carry the SAME label, and that
    // collision is how a block of the cited paper ended up pasted into the
    // claim box — with no manuscript loaded, the only button with that name on
    // screen was the wrong one.
    // Import, then remount: `slice-check` renders in the idle state, and the
    // remembered manuscript is what brings it back (same shape as the test
    // above).
    const { unmount } = render(panel());
    fireEvent.click(screen.getByTestId('slice-import'));
    await waitFor(() => expect(screen.getByTestId('slice-count')).toBeTruthy());
    unmount();
    render(panel({ citationId: 'lib-other' }));

    const parse = screen.getByTestId('slice-check').textContent ?? '';
    expect(parse).toMatch(/find sentences citing this source/i);
    // It must not promise a check it does not perform.
    expect(parse).not.toMatch(/check citation support/i);

    // And no two buttons in the panel may share a label.
    const labels = Array.from(document.querySelectorAll('button'))
      .map((b) => (b.textContent ?? '').trim().toLowerCase())
      .filter(Boolean);
    expect(new Set(labels).size).toBe(labels.length);
  });
});
