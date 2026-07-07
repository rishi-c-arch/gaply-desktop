// Gaply — upload validation. Pure + specific: never a bare spinner, always a
// precise reason. The page count is supplied by the caller (computed from the
// PDF via estimatePdfPageCount, or omitted for DOCX where pre-parse page count
// isn't meaningful). Everything here is local; no bytes leave the machine.

export const MAX_PAGES = 500;
export const MAX_BYTES = 100 * 1024 * 1024; // 100 MB hard sanity cap
export const ACCEPTED_EXT = ['pdf', 'docx'] as const;
export const ACCEPT_HINT = 'PDF or DOCX, up to ~500 pages';

export interface FileMeta {
  name: string;
  sizeBytes: number;
  /** Known page count (PDF); omit for DOCX. */
  pageCount?: number;
}

export type ValidationResult = { ok: true } | { ok: false; error: string };

export function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
}

export function validateFile(meta: FileMeta): ValidationResult {
  const ext = extensionOf(meta.name);
  if (!ext) {
    return { ok: false, error: `"${meta.name}" has no file extension — Gaply reads ${ACCEPT_HINT}.` };
  }
  if (!(ACCEPTED_EXT as readonly string[]).includes(ext)) {
    return {
      ok: false,
      error: `Gaply reads PDF or DOCX — "${meta.name}" is a .${ext} file.`,
    };
  }
  if (meta.sizeBytes <= 0) {
    return { ok: false, error: `"${meta.name}" is empty (0 bytes).` };
  }
  if (meta.sizeBytes > MAX_BYTES) {
    const mb = (meta.sizeBytes / (1024 * 1024)).toFixed(0);
    return { ok: false, error: `"${meta.name}" is ${mb} MB — max 100 MB.` };
  }
  if (meta.pageCount !== undefined && meta.pageCount > MAX_PAGES) {
    return {
      ok: false,
      error: `File is ${meta.pageCount} pages — max ${MAX_PAGES}.`,
    };
  }
  return { ok: true };
}

/** Read a PDF's page count in the browser from a File, using the already-
 *  bundled pdfjs-dist. Lazy-loaded; returns undefined on any failure so
 *  validation degrades to extension/size only (the Rust core is the ultimate
 *  arbiter of parseability). Not used for DOCX. */
export async function estimatePdfPageCount(file: File): Promise<number | undefined> {
  if (extensionOf(file.name) !== 'pdf') return undefined;
  try {
    const pdfjs: any = await import('pdfjs-dist');
    const buf = await file.arrayBuffer();
    const doc = await pdfjs.getDocument({ data: buf }).promise;
    const n = doc.numPages;
    await doc.destroy?.();
    return n;
  } catch {
    return undefined;
  }
}
