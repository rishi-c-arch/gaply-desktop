// Gaply — one sentence per open-access fetch outcome.
//
// Shared by the single-source action and the batch report on purpose. These are
// the sentences a user reads to decide what to do next — "there is no free copy"
// and "the lookup was rate-limited, try again" call for opposite responses — and
// two copies of that vocabulary is two things to drift.
import { OaFetchReport } from './aiBridge';

/** Whether this outcome left the citation checkable. */
export function isFetchSuccess(r: OaFetchReport): boolean {
  return r.outcome === 'fetched' || r.outcome === 'abstractOnly' || r.outcome === 'alreadyLinked';
}

/** A person-readable sentence for one source's result. */
export function describeOaOutcome(r: OaFetchReport): string {
  switch (r.outcome) {
    case 'fetched':
      return r.checkable
        ? `Open-access copy fetched and indexed — ${r.chunksIndexed ?? 0} passages. It can be checked now.`
        : // Fetched but not embedded is not success: retrieval needs the
          // vectors, and saying "done" sends the user back to a check that
          // still cannot run.
          `Fetched and indexed, but ${(r.chunksIndexed ?? 0) - (r.chunksEmbedded ?? 0)} passages are not embedded yet — a support check cannot run until they are.`;
    case 'abstractOnly':
      return (
        'No free full text exists, so the abstract was stored instead. A check against it is ' +
        'capped at “partial” and labelled — an abstract can support a claim, not establish one.' +
        (r.injectionFlagged
          ? ' This abstract contained text aimed at the model; it was redacted before storing.'
          : '')
      );
    case 'paywalled':
      return 'No free copy — this source is paywalled. Link the file yourself if you have access to it.';
    case 'noOaCopy':
      return `No open-access copy found. ${r.detail ?? ''}`.trim();
    case 'rateLimited':
      // NOT absence. The lookup never actually answered the question.
      return `The lookup was rate-limited and did not run — try again in about ${r.retryAfterSecs ?? 60}s.`;
    case 'alreadyLinked':
      return 'Already linked to an indexed document — nothing was fetched.';
    case 'failed':
    default:
      return `The fetch failed: ${r.detail ?? 'the engine gave no reason.'}`;
  }
}

/** A one-line tally for a batch, naming every category that occurred. */
export function summariseOaBatch(reports: OaFetchReport[]): string {
  if (reports.length === 0) return 'Nothing to fetch.';
  const counts = new Map<string, number>();
  for (const r of reports) counts.set(r.outcome, (counts.get(r.outcome) ?? 0) + 1);
  const label: Record<string, string> = {
    fetched: 'fetched',
    abstractOnly: 'abstract only',
    paywalled: 'paywalled',
    noOaCopy: 'no OA copy',
    rateLimited: 'rate-limited',
    alreadyLinked: 'already linked',
    failed: 'failed',
  };
  // Every category that occurred is named. A batch reported as "8 of 12
  // succeeded" hides the four sentences the user actually needs to read.
  const parts: string[] = [];
  counts.forEach((n, k) => parts.push(`${n} ${label[k] ?? k}`));
  return `${reports.length} source${reports.length === 1 ? '' : 's'}: ${parts.join(', ')}.`;
}
