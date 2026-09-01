// Gaply — the import size gate, as the user meets it (§11 D57).
//
// The thresholds themselves are pinned in Rust, where they are enforced. What
// these cover is the half that only exists on screen: that the estimate is
// SHOWN before the work starts, that the confirm tier reads as a question the
// user may answer either way, that a refusal stops before anything is indexed,
// and that a scanned PDF produces the OCR advice rather than a progress bar.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DocumentRow } from './DocumentRow';
import { ImportPreflight } from './aiBridge';

vi.mock('./PdfViewer', () => ({ default: () => <div data-testid="pdf-viewer-stub" /> }));

afterEach(cleanup);

const PICKED = '/Users/rishi/Desktop/thesis.pdf';

function preflight(over: Partial<ImportPreflight> = {}): ImportPreflight {
  return {
    pages: 38,
    pageEquivalents: 38,
    bytes: 2_000_000,
    estimatedSeconds: 19,
    estimateBasis: { kind: 'seeded' },
    verdict: 'ok',
    summary: '38 pages — about 19 seconds to index on this machine.',
    ...over,
  } as ImportPreflight;
}

function bridge(over: Record<string, any> = {}) {
  return {
    citationDocument: async () => null,
    documentSource: async () => ({
      documentId: 9,
      path: PICKED,
      exists: true,
      extension: 'pdf',
    }),
    importPreflight: async () => preflight(),
    linkSourceDocument: async () => ({
      documentId: 9,
      title: 'Thesis',
      chunksIndexed: 40,
      chunksEmbedded: 40,
      chunksPending: 0,
      checkable: true,
    }),
    fetchOpenAccess: async () => [],
    ...over,
  } as any;
}

function row(over: Record<string, any> = {}) {
  return render(
    <DocumentRow citationId="c1" bridge={bridge(over)} pickSource={async () => PICKED} />,
  );
}

describe('Import size guidance', () => {
  it('states the cost before indexing, even when nothing is in the way', async () => {
    // The estimate is the feature. A user who is only ever told about the
    // expensive case has no idea what "expensive" is being measured against.
    const linkSourceDocument = vi.fn(async () => ({
      documentId: 9,
      title: 'Thesis',
      chunksIndexed: 40,
      chunksEmbedded: 40,
      chunksPending: 0,
      checkable: true,
    }));
    row({ linkSourceDocument });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));

    await waitFor(() => expect(screen.getByTestId('document-linked-ok')).toBeTruthy());
    expect(screen.getByTestId('document-estimate').textContent).toBe(
      '38 pages — about 19 seconds to index on this machine.',
    );
    expect(linkSourceDocument).toHaveBeenCalled();
  });

  it('above the confirm threshold it ASKS, and indexes nothing until answered', async () => {
    const linkSourceDocument = vi.fn();
    row({
      linkSourceDocument,
      importPreflight: async () =>
        preflight({
          pages: 420,
          pageEquivalents: 420,
          verdict: 'confirmationRequired',
          estimatedSeconds: 210,
          summary:
            '420 pages — indexing will take about 4 minutes on this machine, and it runs to completion once started. Start it?',
        }),
    });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));

    await waitFor(() => expect(screen.getByTestId('document-confirm')).toBeTruthy());
    expect(screen.getByTestId('document-confirm').textContent).toMatch(/Start it\?/);
    expect(screen.getByTestId('document-confirm').textContent).toMatch(/about 4 minutes/);
    // Nothing has been indexed yet — that is the whole point of asking.
    expect(linkSourceDocument).not.toHaveBeenCalled();
  });

  it('the user may always say yes, and the answer is passed to the backend', async () => {
    // This is a speed bump, not a gate that can be failed.
    const linkSourceDocument = vi.fn(
      async (
        _id: string,
        _path: string,
        _title?: string,
        _onEvent?: unknown,
        _confirmed?: boolean,
      ) => ({
        documentId: 9,
        title: 'Thesis',
        chunksIndexed: 900,
        chunksEmbedded: 900,
        chunksPending: 0,
        checkable: true,
      }),
    );
    row({
      linkSourceDocument,
      importPreflight: async () =>
        preflight({ verdict: 'confirmationRequired', summary: '420 pages — … Start it?' }),
    });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));
    await waitFor(() => expect(screen.getByTestId('document-confirm-yes')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-confirm-yes'));

    await waitFor(() => expect(linkSourceDocument).toHaveBeenCalled());
    // The 5th argument is `confirmed` — the backend enforces the same gate, so
    // it has to be told the user actually answered.
    expect(linkSourceDocument.mock.calls[0][4]).toBe(true);
  });

  it('saying no indexes nothing and leaves the row untouched', async () => {
    const linkSourceDocument = vi.fn();
    row({
      linkSourceDocument,
      importPreflight: async () =>
        preflight({ verdict: 'confirmationRequired', summary: '420 pages — … Start it?' }),
    });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));
    await waitFor(() => expect(screen.getByTestId('document-confirm-no')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-confirm-no'));

    await waitFor(() => expect(screen.queryByTestId('document-confirm')).toBeNull());
    expect(linkSourceDocument).not.toHaveBeenCalled();
    expect(screen.getByTestId('document-none')).toBeTruthy();
  });

  it('above the ceiling it refuses, and says the limit is time and not accuracy', async () => {
    // A user told only "too big" reasonably concludes their thesis cannot be
    // checked properly. That is not what this limit means and not true.
    const linkSourceDocument = vi.fn();
    row({
      linkSourceDocument,
      importPreflight: async () =>
        preflight({
          pages: 2400,
          pageEquivalents: 2400,
          verdict: 'refused',
          summary:
            '2400 pages — above the 1500-page limit for a single import. This is a limit on TIME, not on accuracy: indexing it would run for about 20 minutes, and the app would look hung for most of it. Split it into chapters and import those.',
        }),
    });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));

    await waitFor(() => expect(screen.getByTestId('document-link-error')).toBeTruthy());
    const text = screen.getByTestId('document-link-error').textContent ?? '';
    expect(text).toMatch(/limit on TIME, not on accuracy/);
    expect(text).toMatch(/Split it into chapters/);
    expect(linkSourceDocument).not.toHaveBeenCalled();
    expect(screen.queryByTestId('document-confirm')).toBeNull();
  });

  it('a scanned PDF gets the OCR advice before any indexing time is spent', async () => {
    const linkSourceDocument = vi.fn();
    row({
      linkSourceDocument,
      importPreflight: async () =>
        preflight({
          pages: null,
          pageEquivalents: 0,
          estimatedSeconds: 0,
          verdict: 'refused',
          summary:
            'This PDF has no extractable text — it looks scanned or image-only. Extraction needs a text-based PDF (export from your editor, or run OCR first).',
        }),
    });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));

    await waitFor(() => expect(screen.getByTestId('document-link-error')).toBeTruthy());
    const text = screen.getByTestId('document-link-error').textContent ?? '';
    expect(text).toMatch(/no extractable text/);
    expect(text).toMatch(/OCR/);
    // Not one chunk, and no progress line — the refusal came first.
    expect(linkSourceDocument).not.toHaveBeenCalled();
    expect(screen.queryByTestId('document-linking')).toBeNull();
  });

  it('re-asks rather than failing when the BACKEND is the one demanding confirmation', async () => {
    // The gate is enforced server-side; a UI that rendered that answer as an
    // error would turn an enforced invariant into a dead end.
    const linkSourceDocument = vi.fn(async () => ({
      outcome: 'confirmationRequired',
      preflight: preflight({ verdict: 'confirmationRequired', summary: '900 pages — … Start it?' }),
    }));
    row({ linkSourceDocument });
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));

    await waitFor(() => expect(screen.getByTestId('document-confirm')).toBeTruthy());
    expect(screen.getByTestId('document-confirm').textContent).toMatch(/900 pages/);
    expect(screen.queryByTestId('document-link-error')).toBeNull();
  });
});
