// Gaply — the "Deep web/database check (Turnitin-class)" seam.
//
// WHY A SEAM: our LOCAL plagiarism agent compares against your own session and
// the shared corpus only. It cannot replicate the large-database coverage of a
// commercial service. Copyleaks IS that production comprehensive-plagiarism
// path. It is a PAID, ONLINE capability — local checking stays free.
//
// TRUST BOUNDARY: the client never holds the Copyleaks API key, and the
// client-side request carries NO raw manuscript text — only a document
// reference + scan options. The request is routed through the Railway proxy,
// which holds the key and submits the manuscript text it already has
// server-side. This keeps manuscript bytes off the client's network path and
// off any client-side credential.
//
// Ships as a MOCK now; ProxyCopyleaksClient documents the real path.

export interface DeepCheckRequest {
  /** Server-side reference to the already-parsed manuscript — NOT its text. */
  manuscriptRef: string;
  /** What to scan against (web, published papers, repositories). */
  scope: Array<'internet' | 'scholar' | 'repositories'>;
  /** Explicit user consent — a deep check sends the manuscript to a third party. */
  consent: true;
}

export interface DeepMatch {
  source: string;
  url: string;
  similarity: number; // [0,1]
  matchedWords: number;
}

export interface DeepCheckResult {
  status: 'completed' | 'pending' | 'error';
  aggregateScore: number; // [0,1] overall similarity
  matches: DeepMatch[];
  provider: string; // "copyleaks"
}

export interface CopyleaksClient {
  /** Submit a deep check. The request MUST NOT contain raw manuscript text. */
  checkDeep(request: DeepCheckRequest): Promise<DeepCheckResult>;
}

/** Test/dev double — records requests so tests can assert the payload carries
 *  no manuscript text, and returns a canned result. No network. */
export class MockCopyleaksClient implements CopyleaksClient {
  public requests: DeepCheckRequest[] = [];
  constructor(private result?: DeepCheckResult) {}

  async checkDeep(request: DeepCheckRequest): Promise<DeepCheckResult> {
    this.requests.push(request);
    return (
      this.result ?? {
        status: 'completed',
        provider: 'copyleaks',
        aggregateScore: 0.12,
        matches: [
          { source: 'ResearchGate preprint', url: 'https://example.org/p/123', similarity: 0.12, matchedWords: 84 },
        ],
      }
    );
  }
}

/** Production path (documented, not wired): POST the reference to the Railway
 *  proxy, which holds the Copyleaks key and submits the manuscript text
 *  server-side. Throws until the proxy endpoint exists — the deep check is a
 *  paid/online capability gated behind explicit consent. */
export class ProxyCopyleaksClient implements CopyleaksClient {
  constructor(private proxyBaseUrl: string) {}
  async checkDeep(_request: DeepCheckRequest): Promise<DeepCheckResult> {
    // Real: await fetch(`${this.proxyBaseUrl}/deep-plagiarism`, {..., body: JSON.stringify(_request)})
    // The proxy attaches the Copyleaks key and the server-held manuscript text.
    throw new Error(
      'ProxyCopyleaksClient: the Railway deep-plagiarism endpoint is not wired yet. ' +
        'Local plagiarism checking is available and free.'
    );
  }
}
