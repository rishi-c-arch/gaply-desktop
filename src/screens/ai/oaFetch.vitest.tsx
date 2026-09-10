// Gaply — the open-access fetch action, and the vocabulary it reports with.
//
// The outcomes are the feature. "There is no free copy", "the lookup was
// rate-limited" and "the abstract was stored instead" call for three different
// responses from the user, so the tests below are organised by outcome rather
// than by component: each asserts that one answer survives the trip from the
// backend's tag to the sentence on screen.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { DocumentRow } from './DocumentRow';
import { OaFetchReport } from './aiBridge';
import { describeOaOutcome, isFetchSuccess, summariseOaBatch } from './oaOutcome';

vi.mock('./PdfViewer', () => ({ default: () => <div data-testid="pdf-viewer-stub" /> }));

// The row reads the SAME consent gate the batch action does. Most tests here
// are about the outcome vocabulary, so consent is granted by default and the
// refusal gets its own test.
import { setCloudConsent } from '../settings/settingsStore';

beforeEach(() => {
  setCloudConsent('citation_verification', true);
});

afterEach(cleanup);

const FETCHED_PATH = '/Users/rishi/Library/Application Support/ai.gaply.app/oa_papers/10-1-a.pdf';

/** §11 D132: `citationId` is now nested in a `subject` union. The helper takes a
 *  bare id so the call sites stay readable. */
function report(over: Partial<OaFetchReport> & { citationId?: string }): OaFetchReport {
  const { citationId, ...rest } = over;
  return {
    subject: { kind: 'citation', citationId: citationId ?? 'cite-naidu' },
    title: 'Incidence of needlestick injury',
    outcome: 'fetched',
    ...rest,
  } as OaFetchReport;
}

function bridge(over: Record<string, any> = {}) {
  return {
    citationDocument: async () => null,
    documentSource: async () => ({
      documentId: 12,
      path: FETCHED_PATH,
      exists: true,
      extension: 'pdf',
    }),
    linkSourceDocument: async () => {
      throw new Error('not used here');
    },
    fetchOpenAccess: async () => [report({})],
    cancelEmbedding: async () => {},
    importPreflight: async () => ({
      pages: 38,
      pageEquivalents: 38,
      bytes: 2_000_000,
      estimatedSeconds: 19,
      estimateBasis: { kind: 'seeded' },
      verdict: 'ok',
      summary: '38 pages — about 19 seconds to index on this machine.',
    }),
    ...over,
  } as any;
}

describe('Fetch open-access PDF — the action', () => {
  it('is offered only when the citation has a DOI', async () => {
    // The lookup IS the DOI. A button that can only ever answer "there was
    // nothing to look up" is worse than a button that is not there.
    const { unmount } = render(<DocumentRow citationId="c1" bridge={bridge()} />);
    await waitFor(() => expect(screen.getByTestId('document-none')).toBeTruthy());
    expect(screen.queryByTestId('document-fetch-oa')).toBeNull();
    unmount();

    render(<DocumentRow citationId="c1" bridge={bridge()} doi="10.1/a" />);
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
  });

  it('says what leaves the machine, before anything is pressed', async () => {
    // The privacy invariant is the reason this operation is permitted at all,
    // so the user is told it at the point of decision — not in a settings page
    // they would have to go looking for.
    render(<DocumentRow citationId="c1" bridge={bridge()} doi="10.1/a" />);
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa-note')).toBeTruthy());
    const note = screen.getByTestId('document-fetch-oa-note').textContent ?? '';
    expect(note).toMatch(/DOI/);
    expect(note).toMatch(/nothing else leaves your machine/i);
  });

  /** §11 D105. Fetching a 37-page paper spends ~90 seconds between the press
   *  and the outcome. Nothing was subscribed to the phase channel, so the row
   *  said "Looking for a free copy…" for all of it — reported, reasonably, as
   *  the feature doing nothing. */
  it('reports what the fetch is DOING while it runs, not just at the end', async () => {
    let emit: ((ev: any) => void) | undefined;
    let finish: ((r: any[]) => void) | undefined;
    const fetchOpenAccess = vi.fn(
      (_ids: string[], onEvent?: (ev: any) => void) =>
        new Promise<any[]>((resolve) => {
          emit = onEvent;
          finish = resolve;
        }),
    );
    render(
      <DocumentRow citationId="c1" doi="10.1/a" bridge={bridge({ fetchOpenAccess })} />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));
    await waitFor(() => expect(emit).toBeTruthy());

    // The phase channel is SUBSCRIBED — the bug was that it was not.
    expect(fetchOpenAccess.mock.calls[0][1]).toBeTypeOf('function');

    emit!({ kind: 'phase', index: 0, total: 1, phase: { phase: 'downloading' } });
    await waitFor(() =>
      expect(screen.getByTestId('document-fetching').textContent).toMatch(/Downloading/i),
    );

    // Embedding is the long pole and the only phase that MOVES. A line that
    // sits still for ninety seconds is a spinner with extra words.
    emit!({ kind: 'phase', index: 0, total: 1, phase: { phase: 'embedding', done: 32, total: 113 } });
    await waitFor(() =>
      expect(screen.getByTestId('document-fetching').textContent).toMatch(/32 of 113 passages/),
    );

    finish!([report({ outcome: 'fetched', documentId: 12, chunksIndexed: 113, chunksEmbedded: 113, checkable: true })]);
    // And it goes away when there is an outcome to read instead.
    await waitFor(() => expect(screen.queryByTestId('document-fetching')).toBeNull());
  });

  it('fetched: flips the row to linked and offers the re-check', async () => {
    const onLinked = vi.fn();
    const fetchOpenAccess = vi.fn(async (_ids: string[]) => [
      report({ outcome: 'fetched', documentId: 12, chunksIndexed: 23, chunksEmbedded: 23, checkable: true }),
    ]);
    render(
      <DocumentRow
        citationId="cite-naidu"
        doi="10.4103/ijmr.ijmr_892_23"
        bridge={bridge({ fetchOpenAccess })}
        onLinked={onLinked}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));

    await waitFor(() => expect(screen.getByTestId('document-fetch-result')).toBeTruthy());
    expect(screen.getByTestId('document-fetch-result').textContent).toMatch(/fetched and indexed/i);
    // The list of one: the single action and the batch are the same command,
    // and since §11 D132 it carries SUBJECTS so a staged manuscript reference
    // is addressable by the same call.
    expect(fetchOpenAccess.mock.calls[0][0]).toEqual([
      { kind: 'citation', citationId: 'cite-naidu' },
    ]);
    expect(screen.getByTestId('document-name').textContent).toBe('10-1-a.pdf');
    expect(onLinked).toHaveBeenCalledWith(12);
  });

  it('abstract only: says so, and says the verdict will be capped', async () => {
    // The whole risk of the abstract path is a user believing the paper was
    // read. The sentence has to carry the limit, not just the success.
    render(
      <DocumentRow
        citationId="cite-naidu"
        doi="10.1/a"
        bridge={bridge({
          fetchOpenAccess: async () => [
            report({ outcome: 'abstractOnly', documentId: 13, chunksIndexed: 1, checkable: true }),
          ],
        })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));

    await waitFor(() => expect(screen.getByTestId('document-fetch-result')).toBeTruthy());
    const text = screen.getByTestId('document-fetch-result').textContent ?? '';
    expect(text).toMatch(/abstract/i);
    expect(text).toMatch(/partial/i);
  });

  it('paywalled: reports it without claiming a failure', async () => {
    render(
      <DocumentRow
        citationId="c1"
        doi="10.1/a"
        bridge={bridge({
          fetchOpenAccess: async () => [
            report({ outcome: 'paywalled', detail: 'unpaywall named no PDF' }),
          ],
        })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));
    await waitFor(() => expect(screen.getByTestId('document-fetch-result')).toBeTruthy());
    const text = screen.getByTestId('document-fetch-result').textContent ?? '';
    expect(text).toMatch(/paywalled/i);
    // Still not linked — a paywalled source produced no document.
    expect(screen.queryByTestId('document-name')).toBeNull();
  });

  it('no OA copy: reports the reason it was given', async () => {
    render(
      <DocumentRow
        citationId="c1"
        doi="10.1/a"
        bridge={bridge({
          fetchOpenAccess: async () => [
            report({ outcome: 'noOaCopy', detail: 'not in unpaywall; not in openalex' }),
          ],
        })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));
    await waitFor(() => expect(screen.getByTestId('document-fetch-result')).toBeTruthy());
    expect(screen.getByTestId('document-fetch-result').textContent).toMatch(
      /no open-access copy found.*not in unpaywall/i,
    );
  });

  it('respects the Settings consent gate, and makes no request when it is off', async () => {
    setCloudConsent('citation_verification', false);
    const fetchOpenAccess = vi.fn(async (_ids: string[]) => [report({})]);
    render(
      <DocumentRow citationId="c1" doi="10.1/a" bridge={bridge({ fetchOpenAccess })} />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));
    await waitFor(() => expect(screen.getByTestId('document-fetch-result')).toBeTruthy());
    expect(screen.getByTestId('document-fetch-result').textContent).toMatch(
      /turned off in Settings/i,
    );
    expect(fetchOpenAccess).not.toHaveBeenCalled();
  });

  it('a rejected invoke is rendered as its message, never [object Object]', async () => {
    // The wire shape is a PLAIN OBJECT `{code, message}`; String(e) on it is
    // the literal "[object Object]", which is what this row used to print.
    render(
      <DocumentRow
        citationId="c1"
        doi="10.1/a"
        bridge={bridge({
          fetchOpenAccess: async () => {
            throw { code: 'Internal', message: 'http client build failed' };
          },
        })}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));
    await waitFor(() => expect(screen.getByTestId('document-fetch-result')).toBeTruthy());
    const text = screen.getByTestId('document-fetch-result').textContent ?? '';
    expect(text).toBe('http client build failed');
    expect(text).not.toContain('[object Object]');
  });
});

/* ------------------------------------------------------------------ *
 *  §11 D115 — a multi-minute run must be stoppable.
 * ------------------------------------------------------------------ */

describe('stopping a link in progress', () => {
  /** Indexing and embedding a thesis streams "Embedding 32 of 113 passages…"
   *  for minutes. `ai_link_source_document` reads the shared cancel flag, but
   *  the command that SETS it was not on the IPC surface and no surface offered
   *  the action — a live progress indicator with no way to stop it. */
  it('offers Stop while linking, and calls the cancel command', async () => {
    const cancelEmbedding = vi.fn(async () => {});
    let emit: ((e: any) => void) | undefined;
    let finish: ((r: any) => void) | undefined;
    const linkSourceDocument = vi.fn(
      (_id: string, _p: string, _t: string, onEvent?: (e: any) => void) =>
        new Promise((resolve) => {
          emit = onEvent;
          finish = resolve;
        }),
    );
    render(
      <DocumentRow
        citationId="c1"
        bridge={bridge({ linkSourceDocument, cancelEmbedding }) as any}
        pickSource={async () => '/thesis.pdf'}
      />,
    );
    await waitFor(() => expect(screen.getByTestId('document-link')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-link'));
    await waitFor(() => expect(emit).toBeTruthy());

    emit!({ kind: 'embedding', done: 32, total: 113 });
    await waitFor(() =>
      expect(screen.getByTestId('document-linking').textContent).toMatch(/32 of 113/),
    );

    // THE AFFORDANCE, beside the progress line it belongs to.
    const stop = screen.getByTestId('document-link-cancel');
    fireEvent.click(stop);
    await waitFor(() => expect(cancelEmbedding).toHaveBeenCalledTimes(1));

    // The flag is read BETWEEN batches, so the current one finishes. Saying
    // "Stopping…" is the honest report of that, and the note says what is kept.
    expect(screen.getByTestId('document-link-cancel').textContent).toMatch(/Stopping/);
    expect(screen.getByTestId('document-stopping-note').textContent).toMatch(/stays linked/i);

    finish!({ documentId: 12, chunksIndexed: 113, chunksEmbedded: 32, chunksPending: 81, checkable: false });
    await waitFor(() => expect(screen.queryByTestId('document-linking')).toBeNull());
  });

  it('does NOT offer Stop during a fetch, whose loop cannot honour it', async () => {
    // §11 D101. `oa_fetch`'s embedding loop never reads the cancel flag, so a
    // Stop button there would be one that cannot succeed.
    let emit: ((ev: any) => void) | undefined;
    const fetchOpenAccess = vi.fn(
      (_ids: string[], onEvent?: (ev: any) => void) =>
        new Promise<any[]>(() => {
          emit = onEvent;
        }),
    );
    render(<DocumentRow citationId="c1" doi="10.1/a" bridge={bridge({ fetchOpenAccess }) as any} />);
    await waitFor(() => expect(screen.getByTestId('document-fetch-oa')).toBeTruthy());
    fireEvent.click(screen.getByTestId('document-fetch-oa'));
    await waitFor(() => expect(emit).toBeTruthy());

    emit!({ kind: 'phase', index: 0, total: 1, phase: { phase: 'embedding', done: 5, total: 60 } });
    await waitFor(() =>
      expect(screen.getByTestId('document-fetching').textContent).toMatch(/5 of 60/),
    );
    expect(screen.queryByTestId('document-link-cancel')).toBeNull();
  });
});

describe('Fetch open-access PDF — the outcome vocabulary', () => {
  it('rate-limited is retryable and is NEVER phrased as absence', () => {
    // The lookup did not run. Saying "no free copy" here would report a claim
    // the fetch never made.
    const text = describeOaOutcome(report({ outcome: 'rateLimited', retryAfterSecs: 30 }));
    expect(text).toMatch(/rate-limited/i);
    expect(text).toMatch(/try again/i);
    expect(text).not.toMatch(/no (open-access |free )?copy/i);
    expect(isFetchSuccess(report({ outcome: 'rateLimited' }))).toBe(false);
  });

  it('fetched-but-unembedded is not reported as success', () => {
    const text = describeOaOutcome(
      report({ outcome: 'fetched', chunksIndexed: 40, chunksEmbedded: 10, checkable: false }),
    );
    expect(text).toMatch(/30 passages are not embedded/);
    expect(text).toMatch(/cannot run/i);
  });

  it('an injected abstract says that it was redacted', () => {
    const text = describeOaOutcome(
      report({ outcome: 'abstractOnly', injectionFlagged: true, checkable: true }),
    );
    expect(text).toMatch(/aimed at the model/i);
    expect(text).toMatch(/redacted/i);
  });

  it('already linked is reported, not silently skipped', () => {
    const text = describeOaOutcome(report({ outcome: 'alreadyLinked', documentId: 7 }));
    expect(text).toMatch(/already linked/i);
    expect(text).toMatch(/nothing was fetched/i);
    // It IS a usable state, even though no request was made.
    expect(isFetchSuccess(report({ outcome: 'alreadyLinked' }))).toBe(true);
  });

  it('a batch summary names every category that occurred, never just a count', () => {
    // "8 of 12 succeeded" is the sentence that hides the four the user has to
    // do something about.
    const summary = summariseOaBatch([
      report({ citationId: 'a', outcome: 'fetched' }),
      report({ citationId: 'b', outcome: 'fetched' }),
      report({ citationId: 'c', outcome: 'paywalled' }),
      report({ citationId: 'd', outcome: 'abstractOnly' }),
      report({ citationId: 'e', outcome: 'rateLimited' }),
    ]);
    expect(summary).toMatch(/5 sources/);
    expect(summary).toMatch(/2 fetched/);
    expect(summary).toMatch(/1 paywalled/);
    expect(summary).toMatch(/1 abstract only/);
    expect(summary).toMatch(/1 rate-limited/);
  });
});
