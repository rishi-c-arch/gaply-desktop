// Gaply — Journal Verification bridge (Set 4). One thin command
// (verify_journal_full) → the combined evidence result. The user's JWT rides to
// the proxy for the server-side entitlement gate on the LLM site-summary (the
// cloud work); the grounded registry facts run locally regardless.
import { isTauri } from '../../utils/isTauri';
import { JournalVerificationResult } from './journalVerifyTypes';

export interface JournalVerifyBridge {
  /** Verify by a journal NAME or LINK. */
  verify(query: string): Promise<JournalVerificationResult>;
  /** Verify a specific ISSN (e.g. after picking a disambiguation match). */
  verifyByIssn(issn: string): Promise<JournalVerificationResult>;
}

export class TauriJournalVerifyBridge implements JournalVerifyBridge {
  constructor(private getUserToken?: () => string | undefined) {}

  private async invoke(args: Record<string, unknown>): Promise<JournalVerificationResult> {
    if (!isTauri) throw new Error('Journal Verification runs in the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<JournalVerificationResult>('verify_journal_full', {
      userToken: this.getUserToken?.() ?? null,
      localPredatorySignals: null,
      ...args,
    });
  }

  verify(query: string) {
    return this.invoke({ query, issn: null });
  }
  verifyByIssn(issn: string) {
    return this.invoke({ query: null, issn });
  }
}

/** Test/dev double — a query→result map (+ an issn map), so the Set 5 UI tests
 *  cover the combined result, not-found, and multiple-match variants. */
export function makeMockJournalVerifyBridge(opts: {
  byQuery?: Record<string, JournalVerificationResult>;
  byIssn?: Record<string, JournalVerificationResult>;
  onCall?: (kind: 'query' | 'issn', value: string) => void;
  fallback?: JournalVerificationResult;
}): JournalVerifyBridge & { calls: Array<['query' | 'issn', string]> } {
  const calls: Array<['query' | 'issn', string]> = [];
  const pick = (map: Record<string, JournalVerificationResult> | undefined, key: string) =>
    map?.[key] ?? opts.fallback ?? notFoundResult(key);
  return {
    calls,
    async verify(query) {
      calls.push(['query', query]);
      opts.onCall?.('query', query);
      return pick(opts.byQuery, query);
    },
    async verifyByIssn(issn) {
      calls.push(['issn', issn]);
      opts.onCall?.('issn', issn);
      return pick(opts.byIssn, issn);
    },
  };
}

/** A default honest not-found result (for unseeded queries). */
export function notFoundResult(input: string): JournalVerificationResult {
  return {
    input,
    input_kind: input.startsWith('http') ? 'link' : 'name',
    disambiguation: [],
    not_found: true,
    registry: null,
    site_summary: null,
    notes: ["couldn't resolve this name in OpenAlex (no matching journal)"],
  };
}
