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

function report(over: Partial<OaFetchReport>): OaFetchReport {
  return {
    citationId: 'cite-naidu',
    title: 'Incidence of needlestick injury',
    outcome: 'fetched',
    ...over,
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
    // The list of one: the single action and the batch are the same command.
    expect(fetchOpenAccess.mock.calls[0][0]).toEqual(['cite-naidu']);
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
