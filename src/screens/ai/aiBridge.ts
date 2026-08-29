// Gaply — the AI layer's IPC surface, following checkBridge.ts.
//
// Same shape as the existing bridge: a thin class with one private `invoke`,
// and `Channel` for streamed progress. No logic lives here.

export type ActiveDevice = 'cpu' | 'metal';

export interface AiModelStatus {
  embedding: { kind: string; [k: string]: unknown };
  generative: { kind: string; [k: string]: unknown };
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

export interface JobProgressEvent {
  jobId: number;
  completed: number;
  total: number;
  currentCategory: string;
  latestItemSummary: string;
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

  async citationNeed(sentence: string, section?: string) {
    return this.invoke<Record<string, unknown>>('ai_citation_need', { sentence, section });
  }

  async citationSupport(claim: string, documentId: number, citedSource?: string) {
    return this.channelInvoke<Record<string, unknown>, unknown>('ai_citation_support', {
      claim,
      documentId,
      citedSource,
    });
  }

  async cancelGeneration(): Promise<void> {
    return this.invoke<void>('ai_generate_cancel', {});
  }

  /* ------------------------------- jobs --------------------------------- */

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

  async jobResults(jobId: number, offset: number, limit: number) {
    return this.invoke<Record<string, unknown>>('ai_job_results', { jobId, offset, limit });
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
