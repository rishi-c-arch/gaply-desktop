// Gaply — the Document row in the citation detail pane.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DocumentRow, fileName } from './DocumentRow';

/** The viewer is lazy; stub it so a test asserts what it was ASKED to open
 *  rather than exercising pdf.js. */
const viewerProps = vi.hoisted(() => ({ last: null as any }));
vi.mock('./PdfViewer', () => ({
  default: (props: any) => {
    viewerProps.last = props;
    return <div data-testid="pdf-viewer-stub" />;
  },
}));

afterEach(() => {
  cleanup();
  viewerProps.last = null;
});

const LINKED = '/Users/rishi/Desktop/chapter 1 .pdf';

function bridge(over: Record<string, any> = {}) {
  return {
    citationDocument: async () => ({ documentId: 7, matchedBy: 'title' }),
    documentSource: async () => ({
      documentId: 7,
      path: LINKED,
      exists: true,
      extension: 'pdf',
    }),
    ...over,
  } as any;
}

describe('Document row', () => {
  it('shows the file name and an enabled Open for a linked document', async () => {
    render(<DocumentRow citationId="cite-1" bridge={bridge()} />);
    await waitFor(() => expect(screen.getByTestId('document-name')).toBeTruthy());
    expect(screen.getByTestId('document-name').textContent).toBe('chapter 1 .pdf');
    expect((screen.getByTestId('document-open') as HTMLButtonElement).disabled).toBe(false);
    expect((screen.getByTestId('document-reveal') as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByTestId('document-missing')).toBeNull();
  });

  it('opens the viewer on the linked document id, at page 1', async () => {
    // Page 1 and not "wherever the last evidence row pointed": this entry point
    // is the citation, which names no passage.
    render(<DocumentRow citationId="cite-1" citedSource="Chapter 1" bridge={bridge()} />);
    await waitFor(() => expect(screen.getByTestId('document-open')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-open'));
    await waitFor(() => expect(screen.getByTestId('pdf-viewer-stub')).toBeTruthy());
    expect(viewerProps.last.documentId).toBe(7);
    expect(viewerProps.last.initialPage).toBe(1);
    expect(viewerProps.last.open).toBe(true);
    expect(viewerProps.last.title).toBe('Chapter 1');
  });

  it('reveals the file at its real path', async () => {
    const reveal = vi.fn(async () => {});
    render(<DocumentRow citationId="cite-1" bridge={bridge()} reveal={reveal} />);
    await waitFor(() => expect(screen.getByTestId('document-reveal')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-reveal'));
    await waitFor(() => expect(reveal).toHaveBeenCalledWith(LINKED));
  });

  it('renders the evidence rows\u2019 not-found state when the file has moved', async () => {
    // Same sentence as the evidence rows use. One fact, one phrasing.
    render(
      <DocumentRow
        citationId="cite-1"
        bridge={bridge({
          documentSource: async () => ({ documentId: 7, path: LINKED, exists: false, extension: 'pdf' }),
        })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-missing')).toBeTruthy());
    expect(screen.getByTestId('document-missing').textContent).toBe(
      'file not found — it was moved or renamed since indexing',
    );
    // Neither action can work against a file that is not there.
    expect((screen.getByTestId('document-reveal') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId('document-open') as HTMLButtonElement).disabled).toBe(true);
    // The name is still shown — it is what the user has to go looking for.
    expect(screen.getByTestId('document-name').textContent).toBe('chapter 1 .pdf');
  });

  it('offers a live Link action when nothing is linked', async () => {
    render(<DocumentRow citationId="cite-1" bridge={bridge({ citationDocument: async () => null })} />);
    await waitFor(() => expect(screen.getByTestId('document-none')).toBeTruthy());
    expect((screen.getByTestId('document-link') as HTMLButtonElement).disabled).toBe(false);
    expect(screen.queryByTestId('document-open')).toBeNull();
    // Says WHY it matters, rather than leaving a dead button unexplained.
    expect(screen.getByTestId('document-none').textContent).toMatch(/citation support/i);
  });

  it('links a picked file: index, embed with progress, then a checkable source', async () => {
    let emit: (ev: any) => void = () => {};
    const linkSourceDocument = vi.fn(
      async (_c: string, _p: string, _t: string | undefined, onEvent: any) => {
        emit = onEvent;
        emit({ kind: 'parsing' });
        emit({ kind: 'indexed', chunks: 42 });
        emit({ kind: 'embedding', done: 20, total: 42 });
        return {
          documentId: 9,
          title: 'Naidu 2023',
          chunksIndexed: 42,
          chunksEmbedded: 42,
          chunksPending: 0,
          checkable: true,
        };
      },
    );
    const onLinked = vi.fn();
    render(
      <DocumentRow
        citationId="cite-naidu"
        citedSource="Naidu 2023"
        bridge={bridge({ citationDocument: async () => null, linkSourceDocument })}
        pickSource={async () => '/Users/rishi/Desktop/naidu.pdf'}
        onLinked={onLinked}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));

    await waitFor(() => expect(screen.getByTestId('document-linked-ok')).toBeTruthy());
    expect(linkSourceDocument.mock.calls[0][0]).toBe('cite-naidu');
    expect(linkSourceDocument.mock.calls[0][1]).toBe('/Users/rishi/Desktop/naidu.pdf');
    // The row flips to the linked state, and the panel is told to re-ask.
    expect(screen.getByTestId('document-name').textContent).toBe('naidu.pdf');
    expect(onLinked).toHaveBeenCalledWith(9);
  });

  it('does not call a source checkable when its passages are not embedded', async () => {
    // Retrieval needs the vectors. "Linked" and "checkable" are different
    // facts, and reporting the first as the second sends the user back to a
    // check that still cannot run.
    const onLinked = vi.fn();
    render(
      <DocumentRow
        citationId="cite-naidu"
        bridge={bridge({
          citationDocument: async () => null,
          linkSourceDocument: async () => ({
            documentId: 9,
            title: 't',
            chunksIndexed: 42,
            chunksEmbedded: 10,
            chunksPending: 32,
            checkable: false,
          }),
        })}
        pickSource={async () => '/x.pdf'}
        onLinked={onLinked}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));
    await waitFor(() => expect(screen.getByTestId('document-link-error')).toBeTruthy());
    expect(screen.getByTestId('document-link-error').textContent).toMatch(/32 passages are not embedded/);
    expect(screen.queryByTestId('document-linked-ok')).toBeNull();
    expect(onLinked).not.toHaveBeenCalled();
  });

  it('picking no file leaves everything alone', async () => {
    const linkSourceDocument = vi.fn();
    render(
      <DocumentRow
        citationId="cite-1"
        bridge={bridge({ citationDocument: async () => null, linkSourceDocument })}
        pickSource={async () => null}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));
    await waitFor(() => expect(linkSourceDocument).not.toHaveBeenCalled());
    expect(screen.getByTestId('document-none')).toBeTruthy();
  });

  it('reports a failed lookup instead of claiming nothing is linked', async () => {
    // "No document linked" is a claim about the store. A failed read is not
    // evidence for it, and must not be reported as one.
    render(
      <DocumentRow
        citationId="cite-1"
        bridge={bridge({ citationDocument: async () => Promise.reject({ code: 'database', message: 'db locked' }) })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-error')).toBeTruthy());
    expect(screen.getByTestId('document-error').textContent).toBe('db locked');
    expect(screen.queryByTestId('document-none')).toBeNull();
  });

  it('fileName handles both separators and a bare name', () => {
    expect(fileName('/a/b/c.pdf')).toBe('c.pdf');
    expect(fileName('C:\\\\docs\\\\c.pdf')).toBe('c.pdf');
    expect(fileName('c.pdf')).toBe('c.pdf');
  });
});
