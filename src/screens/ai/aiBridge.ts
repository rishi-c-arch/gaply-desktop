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

class AiBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  async modelStatus(): Promise<AiModelStatus> {
    return this.invoke<AiModelStatus>('ai_model_status', {});
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
