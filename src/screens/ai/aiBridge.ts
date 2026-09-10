// Gaply — the AI layer's IPC surface, following checkBridge.ts.
//
// Same shape as the existing bridge: a thin class with one private `invoke`,
// and `Channel` for streamed progress. No logic lives here.

export type ActiveDevice = 'cpu' | 'metal';

/* --------------------------- rejected invokes ---------------------------- */

/** What a Tauri command's `Err` arrives as. `GaplyError` serializes to
 *  `{ code, message }` — a PLAIN OBJECT, never an `Error`. */
export interface WireError {
  code?: string;
  message?: string;
}

/** A rejected invoke as text a person can read.
 *
 *  `String(e)` on the wire shape above yields the literal `"[object Object]"`,
 *  which is what the citation panel printed in red for every real backend
 *  failure. Nothing here may return that: the last resort is a sentence that
 *  admits the engine gave no reason, which is at least true. */
export function errorText(e: unknown): string {
  if (typeof e === 'string' && e.trim()) return e;
  if (e instanceof Error && e.message) return e.message;
  if (e && typeof e === 'object') {
    const o = e as Record<string, unknown>;
    for (const k of ['message', 'error', 'reason'] as const) {
      const v = o[k];
      if (typeof v === 'string' && v.trim()) return v;
    }
    try {
      const j = JSON.stringify(e);
      if (j && j !== '{}' && j !== 'null') return j;
    } catch {
      // cyclic — fall through to the honest fallback
    }
  }
  return 'The check failed and the engine gave no reason.';
}

/** The `code` on a rejected invoke, when there is one. Lets a caller tell a
 *  cancellation from a fault without matching on message text. */
export function errorCode(e: unknown): string | null {
  if (e && typeof e === 'object') {
    const c = (e as Record<string, unknown>).code;
    if (typeof c === 'string') return c;
  }
  return null;
}

/** The backend tags both engine-state enums with `state`, not `kind`
 *  (`#[serde(tag = "state")]` on `EngineState` / `GenState`). Reading `kind`
 *  here silently yielded `undefined` for every status, so the install surface
 *  could never leave its NotInstalled branch. */
export interface EngineStateWire {
  state: string;
  [k: string]: unknown;
}

export interface AiModelStatus {
  embedding: EngineStateWire;
  generative: EngineStateWire;
  generativeModelId: string;
  inFlight: number;
  /** Computed by the backend from GGUF metadata — never invented here. */
  generativeRam: { totalBytes: number; [k: string]: unknown } | null;
  activeDevice: ActiveDevice;
  deviceFallbackReason: string | null;
}

export interface DocumentSource {
  documentId: number;
  path: string;
  exists: boolean;
  extension: string;
}

/** Both installers stream these; the generative one adds resume/progress. */
export type InstallEvent =
  | { kind: 'started'; files: number; totalBytes: number; modelId?: string }
  | { kind: 'alreadyPresent'; file: string }
  | { kind: 'downloading'; file: string; bytes?: number; fromBytes?: number; totalBytes?: number }
  | { kind: 'progress'; file: string; bytes: number; totalBytes: number }
  | { kind: 'verifying'; file: string }
  | { kind: 'verified'; file: string }
  | { kind: 'cancelled' }
  | { kind: 'done'; modelId: string; dir: string };

/** Stages `ai_citation_support` streams over its Channel. Tagged `kind` —
 *  `#[serde(tag = "kind", rename_all = "camelCase")]` on `AiSupportEvent`. */
export type SupportEvent =
  | { kind: 'retrieving' }
  | { kind: 'retrieved'; chunksSent: number; chunksDropped: number }
  | { kind: 'generating'; queuedBehind: number }
  | { kind: 'decoding'; tokens: number; maxTokens: number }
  | { kind: 'validating' };

/** One manuscript sentence citing a given source, found by the pre-pass. */
export interface CitingSentence {
  seq: number;
  page: number | null;
  sentence: string;
  /** The marker that resolved, e.g. "(Smith, 2019)". */
  marker: string;
}

/** What a per-citation check WOULD do, before anything is queued. */
export interface CitationAuditPreview {
  libraryId: string;
  /** null = the cited source is not indexed; the UI offers Link/Index. */
  documentId: number | null;
  unverifiableReason: string | null;
  sentences: CitingSentence[];
  totalSentences: number;
  documentTypesSupported: string[];
}

export interface JobProgressEvent {
  jobId: number;
  completed: number;
  total: number;
  currentCategory: string;
  latestItemSummary: string;
}

/**
 * THE ONE REFUSAL, shared by the Audit and the Citation Manager (§11 D90).
 *
 * Both features already use the identical CONDITION —
 * `checkable_document_for_citation`: a linked document with chunks AND
 * embeddings. What differed was the offer. The Audit grouped the blocked
 * sources and said what to do; the Manager showed a static sentence with no way
 * to act on it.
 *
 * Kept as one constant so the two cannot drift into describing the same state
 * in different words.
 */
export const NOT_CHECKABLE_MESSAGE =
  'Gaply has no readable copy of this source, so there are no passages to check the claim against. Fetch an open-access copy or attach the PDF, then try again.';

/** One distinct cited work, and whether a check could run against it (§11 D88). */
export interface CitedSourceStatus {
  label: string;
  libraryId: string | null;
  documentId: number | null;
  reason: string | null;
  citingSentences: number;
  /** §11 D101. Does the LIBRARY entry record a DOI? The open-access fetch looks
   *  one up, so without it the fetch can do nothing for this source. */
  hasDoi: boolean;
  /** §11 D110. Registry-confirmed retraction of this cited work. */
  retracted: boolean;
}

/** A deterministic consistency finding (§11 D94). No model was involved. */
export interface ConsistencyFinding {
  kind: string;
  severity: 'structural' | 'cosmetic';
  message: string;
  action: string | null;
}

export interface ConsistencyReport {
  findings: ConsistencyFinding[];
  structural: number;
  cosmetic: number;
}

/** What an audit WOULD do — computed with no job and no model (§11 D88). */
export interface ThesisAuditPreview {
  totalSentences: number;
  cited: number;
  uncited: number;
  skipped: number;
  wouldCheck: number;
  /** Significant sentences carrying NO marker, which are NOT examined (D128). */
  notExamined: number;
  wouldBeUnverifiable: number;
  sources: CitedSourceStatus[];
  checkableSources: number;
  blockedSources: number;
  /** The MANUSCRIPT's numbered reference list, and how many entries carry a
   *  DOI — the explanation for why nothing can be fetched (§11 D101). */
  referenceEntries: number;
  referenceEntriesWithDoi: number;
  /** §11 D110. Distinct cited works a registry has CONFIRMED retracted. */
  retractedSources: number;
  /** §11 D94. Deterministic checks, run before any queueing. */
  consistency: ConsistencyReport;
  documentTypesSupported: string[];
}

export interface AuditPlan {
  jobId: number;
  documentTypesSupported: string[];
  totalSentences: number;
  cited: number;
  uncited: number;
  skipped: number;
  markersFound: number;
  queuedCitationNeed: number;
  queuedCitationSupport: number;
  queuedUnverifiable: number;
}

/** Stages of linking a local file to a citation as its source. */
export type LinkSourceEvent =
  | { kind: 'parsing' }
  | { kind: 'indexed'; chunks: number }
  | { kind: 'embedding'; done: number; total: number }
  | { kind: 'linked'; documentId: number };

/** How one open-access fetch ended. Mirrors `oa_fetch::FetchOutcome`. */
export type OaOutcome =
  | 'fetched'
  | 'abstractOnly'
  | 'paywalled'
  | 'noOaCopy'
  | 'rateLimited'
  | 'failed'
  | 'alreadyLinked';

/**
 * WHAT a fetch was for (§11 D132). A discriminated union rather than two
 * nullable ids, because exactly one is true of any fetch: a work in the user's
 * library, or a reference the audit staged from their manuscript. A staged
 * source never acquires a library entry as a side effect of being checked.
 */
export type OaFetchSubject =
  | { kind: 'citation'; citationId: string }
  | { kind: 'staged'; stagedId: number };

/** A stable map key for either kind of subject. */
export function subjectKey(s: OaFetchSubject): string {
  return s.kind === 'citation' ? s.citationId : `staged:${s.stagedId}`;
}

/**
 * One reference entry the audit staged from the manuscript (§11 D132).
 *
 * Never a `citation_library` row: staging keeps the audit's needs separate from
 * the user's collection. `documentId` is set once a PDF has been fetched for it.
 */
export interface StagedSource {
  id: number;
  surname: string;
  year: number | null;
  title: string | null;
  doi: string | null;
  documentId: number | null;
  matchedBy: string | null;
}

/** One source's result. `outcome` is the tag; the rest varies by arm. */
export interface OaFetchReport {
  subject: OaFetchSubject;
  title: string | null;
  outcome: OaOutcome;
  documentId?: number;
  chunksIndexed?: number;
  chunksEmbedded?: number;
  checkable?: boolean;
  source?: string;
  license?: string | null;
  injectionFlagged?: boolean;
  detail?: string;
  retryAfterSecs?: number;
}

/** Where an import estimate's number came from. */
export type EstimateBasis = { kind: 'seeded' } | { kind: 'measured'; samples: number };

/** What an import is about to cost, before anything is spent on it. */
export interface ImportPreflight {
  pages: number | null;
  pageEquivalents: number;
  bytes: number;
  estimatedSeconds: number;
  estimateBasis: EstimateBasis;
  verdict: 'ok' | 'confirmationRequired' | 'refused';
  reason?: string;
  /** The sentence to show. Built in Rust so every surface says the same thing. */
  summary: string;
}

/** What the CURRENT source is doing (§11 D105). Most of a fetch's wall clock is
 *  spent between `fetching` and `done`, so these are what a surface has to show
 *  in that window. `embedding` is the long pole and the only one that moves. */
export type OaFetchPhase =
  | { phase: 'resolving' }
  | { phase: 'downloading' }
  | { phase: 'indexing' }
  | { phase: 'embedding'; done: number; total: number };

/** Stages a batch fetch streams. */
export type OaFetchEvent =
  | { kind: 'started'; total: number }
  | { kind: 'fetching'; index: number; total: number; title: string | null }
  | { kind: 'phase'; index: number; total: number; phase: OaFetchPhase }
  | { kind: 'done'; index: number; total: number; report: OaFetchReport };

export interface LinkSourceResult {
  documentId: number;
  title: string;
  chunksIndexed: number;
  chunksEmbedded: number;
  chunksPending: number;
  /** Whether a support check can actually run against it now. */
  checkable: boolean;
}


class AiBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  async modelStatus(): Promise<AiModelStatus> {
    return this.invoke<AiModelStatus>('ai_model_status', {});
  }

  private async channelInvoke<T, E>(
    cmd: string,
    args: Record<string, unknown>,
    onEvent?: (ev: E) => void,
  ): Promise<T> {
    const { invoke, Channel } = await import('@tauri-apps/api/core');
    const ch = new Channel<E>();
    if (onEvent) ch.onmessage = onEvent;
    return invoke<T>(cmd, { ...args, onEvent: ch });
  }

  /** The embedding model. Resumable and cancellable. */
  async installEmbedding(onEvent?: (ev: InstallEvent) => void) {
    return this.channelInvoke<unknown, InstallEvent>('ai_model_install', {}, onEvent);
  }

  /** A pinned generative candidate. */
  async installGenerative(modelId: string, onEvent?: (ev: InstallEvent) => void) {
    return this.channelInvoke<unknown, InstallEvent>(
      'ai_model_install_generative',
      { modelId },
      onEvent,
    );
  }

  async cancelInstall(): Promise<void> {
    return this.invoke<void>('ai_model_install_cancel', {});
  }

  async generativeCandidates(): Promise<Array<Record<string, unknown>>> {
    return this.invoke<Array<Record<string, unknown>>>('ai_generative_candidates', {});
  }

  /* ------------------------------ tasks --------------------------------- */

  /**
   * `section` is REQUIRED by the Rust command (`section: String`, not
   * `Option<String>`), and an optional TS parameter that is left undefined
   * serializes to a MISSING KEY — which Tauri rejects with "invalid args
   * `section` for command `ai_citation_need`: missing required key `section`".
   * The key being present in this source was never enough; the value has to be.
   *
   * The default is the honest one: this panel has no manuscript context, so it
   * does not know which IMRaD section the sentence came from, and the prompt
   * interpolates the value into `<section>…</section>`. "unknown" says that;
   * an empty tag would read as a section that is genuinely blank.
   */
  async citationNeed(sentence: string, section: string = 'unknown') {
    return this.invoke<Record<string, unknown>>('ai_citation_need', { sentence, section });
  }

  /** `onEvent` is not optional decoration: the command streams retrieval and
   *  decode progress, and dropping it is what left the panel showing one static
   *  line for the whole run. */
  async citationSupport(
    claim: string,
    documentId: number,
    citedSource?: string,
    onEvent?: (ev: SupportEvent) => void,
  ) {
    return this.channelInvoke<Record<string, unknown>, SupportEvent>(
      'ai_citation_support',
      { claim, documentId, citedSource },
      onEvent,
    );
  }

  async cancelGeneration(): Promise<void> {
    return this.invoke<void>('ai_generate_cancel', {});
  }

  /**
   * Stop an embedding run in progress (§11 D115).
   *
   * Applies to `linkSourceDocument`, which checks the shared flag between
   * batches. It does NOT apply to `fetchOpenAccess`, whose embedding loop does
   * not read it — so a surface must not offer cancel during a fetch.
   *
   * Cancelling mid-run is safe by construction: the link is still recorded,
   * and `checkable` is computed from the vectors that actually exist, so the
   * result reports itself honestly as linked-but-not-checkable.
   */
  async cancelEmbedding(): Promise<void> {
    return this.invoke<void>('ai_embed_cancel', {});
  }

  /* ------------------------------- jobs --------------------------------- */

  /**
   * What the audit WOULD do, before anything starts (§11 D88).
   *
   * `startThesisAudit` plans AND spawns the runner, so it cannot back a
   * "start?" card — by the time the card renders the model is already
   * generating. This creates nothing.
   */
  /**
   * The manuscript's own bytes, for the annotated view (§11 D92).
   *
   * PDF only — a .docx records no page geometry, so the backend refuses it with
   * that reason rather than a generic error.
   */
  async manuscriptBytes(path: string): Promise<Uint8Array> {
    const raw = await this.invoke<ArrayBuffer | number[]>('ai_manuscript_bytes', { path });
    return raw instanceof ArrayBuffer ? new Uint8Array(raw) : new Uint8Array(raw);
  }

  async previewThesisAudit(path: string): Promise<ThesisAuditPreview> {
    return this.invoke<ThesisAuditPreview>('ai_thesis_audit_preview', { path });
  }

  async startThesisAudit(path: string, onEvent?: (ev: JobProgressEvent) => void) {
    return this.channelInvoke<AuditPlan, JobProgressEvent>(
      'ai_job_start_thesis_audit',
      { path },
      onEvent,
    );
  }

  async jobStatus(jobId: number): Promise<Record<string, unknown>> {
    return this.invoke<Record<string, unknown>>('ai_job_status', { jobId });
  }

  async pauseJob(jobId: number): Promise<boolean> {
    return this.invoke<boolean>('ai_job_pause', { jobId });
  }

  async resumeJob(jobId: number, onEvent?: (ev: JobProgressEvent) => void) {
    return this.channelInvoke<Record<string, unknown>, JobProgressEvent>(
      'ai_job_resume',
      { jobId },
      onEvent,
    );
  }

  async cancelJob(jobId: number): Promise<boolean> {
    return this.invoke<boolean>('ai_job_cancel', { jobId });
  }

  /**
   * Export a finished audit (§11 D58). Returns the bytes; the caller saves them.
   */
  async exportAuditReport(
    jobId: number,
    manuscriptName: string,
    format: 'pdf' | 'html',
  ): Promise<{ bytes: number[]; extension: string; suggestedName: string }> {
    return this.invoke('ai_job_export_report', { jobId, manuscriptName, format });
  }

  /**
   * Re-queue ONLY the items that became checkable after sources were fetched,
   * and run them. Re-running the whole audit would re-judge sentences whose
   * answers have not changed and overwrite verdicts already read.
   */
  async recheckItems(
    jobId: number,
    subjects: OaFetchSubject[],
    onEvent?: (ev: JobProgressEvent) => void,
  ): Promise<{ requeued: number; items: Array<{ seq: number; documentId: number }> }> {
    // SUBJECTS, not citation ids (§11 D135): a staged manuscript reference has
    // no library id, and the re-check has to narrow by what was actually
    // fetched. Same union the fetch takes, so the two cannot disagree about
    // which sources a press was about.
    return this.channelInvoke('ai_job_recheck_items', { jobId, subjects }, onEvent);
  }

  async jobResults(jobId: number, offset: number, limit: number) {
    return this.invoke<Record<string, unknown>>('ai_job_results', { jobId, offset, limit });
  }

  /* ---- the per-citation slice of the thesis audit (§11 D54) ---- */

  /** Create + index + embed a local file and link it to this citation. */
  async linkSourceDocument(
    citationId: string,
    path: string,
    title?: string,
    onEvent?: (ev: LinkSourceEvent) => void,
    confirmed?: boolean,
  ): Promise<LinkSourceResult & { outcome?: string; preflight?: ImportPreflight }> {
    return this.channelInvoke<LinkSourceResult & { outcome?: string; preflight?: ImportPreflight }, LinkSourceEvent>(
      'ai_link_source_document',
      { citationId, path, title, confirmed },
      onEvent,
    );
  }

  /**
   * What an import will cost, BEFORE it starts (§11 D57).
   *
   * The backend enforces the same thresholds, so this is guidance rather than
   * the gate: a size check that lived only here would be a suggestion.
   */
  async importPreflight(path: string): Promise<ImportPreflight> {
    return this.invoke<ImportPreflight>('ai_import_preflight', { path });
  }

  /**
   * Fetch the open-access full text for one or more citations (§11 D56).
   *
   * ONE method for every surface and BOTH subject kinds: the single-source
   * action passes a list of one, and a staged manuscript reference is just
   * another subject. A by-id convenience was tried and removed — it is a second
   * name every mock must also know, for an argument the caller can map in one
   * line. A second entry point would be a second place for the outcome
   * vocabulary to drift, and the outcomes are the whole point — a batch of
   * twelve answers twelve times, never with a count.
   */
  async fetchOpenAccess(
    subjects: OaFetchSubject[],
    onEvent?: (ev: OaFetchEvent) => void,
  ): Promise<OaFetchReport[]> {
    return this.channelInvoke<OaFetchReport[], OaFetchEvent>(
      'citation_fetch_oa',
      { subjects },
      onEvent,
    );
  }


  /**
   * The reference entries this audit staged from the manuscript (§11 D134).
   *
   * Deterministic, no model, no network. The fetch button needs these to say how
   * many sources it is about to fetch BEFORE it is pressed.
   */
  async jobStagedSources(jobId: number): Promise<StagedSource[]> {
    return this.invoke<StagedSource[]>('ai_job_staged_sources', { jobId });
  }

  /** Deterministic, no model, no job: which manuscript sentences cite this. */
  async citationAuditPreview(citationId: string, path: string): Promise<CitationAuditPreview> {
    return this.invoke<CitationAuditPreview>('ai_citation_audit_preview', { citationId, path });
  }

  /** Queue the slice on the shared job runner; streams the usual progress. */
  async citationAuditStart(
    citationId: string,
    path: string,
    onEvent?: (ev: JobProgressEvent) => void,
  ) {
    return this.channelInvoke<Record<string, unknown>, JobProgressEvent>(
      'ai_citation_audit_start',
      { citationId, path },
      onEvent,
    );
  }

  /** Which indexed document backs a citation (§11 D45). null is a real answer. */
  async citationDocument(citationId: string): Promise<{ documentId: number; matchedBy: string } | null> {
    return this.invoke<{ documentId: number; matchedBy: string } | null>('ai_citation_document', {
      citationId,
    });
  }

  /** Where a document's file is, and whether it is still there. */
  async documentSource(documentId: number): Promise<DocumentSource> {
    return this.invoke<DocumentSource>('ai_document_source', { documentId });
  }

  /**
   * The document's bytes (§11 D42 — read in Rust).
   *
   * Returned as an ArrayBuffer by Tauri's raw response, which react-pdf takes
   * directly as `{ data }`. Deliberately NOT cached here: a thesis is tens of
   * megabytes and the viewer is opened on demand.
   */
  async documentBytes(documentId: number): Promise<Uint8Array> {
    const raw = await this.invoke<ArrayBuffer | number[]>('ai_document_bytes', { documentId });
    return raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
  }
}

export const aiBridge = new AiBridge();
