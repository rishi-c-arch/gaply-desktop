// Gaply — what an export must pass before today's export path saves it
// (Research OS, Phase 5B-3b; decision 151). project_report and
// project_provenance return bytes, a fixed file name and the bytes' SHA-256;
// Rust writes nothing. Here:
//
// * the file name must be the one fixed name for that export
//   (gaply_project_report.md, gaply_project_provenance.json), never another;
// * the SHA-256 of the bytes, taken here, must equal the one Rust returned;
// * the bytes must decode as UTF-8 and encode back to exactly the same bytes,
//   because today's path (utils/saveTextFile.ts) takes text and writes it
//   with TextEncoder: so what reaches the disk is the bytes Rust returned,
//   unchanged. The summary's export checks the same way (DescribeFlow.tsx).
//
// Any of these failing refuses with its sentence, before the save dialog, and
// nothing is saved. No error's message is read or kept.
import type { ProjectExportView } from './projectTypes';
import { EXPORT_WORDS, PROJECT_TEXT, PROVENANCE_EXPORT_SENTENCE, REPORT_EXPORT_SENTENCE } from './projectWording';

export type ExportKind = 'report' | 'provenance';

/** Today's export path: utils/saveTextFile's signature. */
export type SaveFile = (suggestedName: string, text: string, mimeType?: string) => Promise<string | null>;

export const EXPORTS: Record<
  ExportKind,
  { fileName: ProjectExportView['fileName']; mime: string; sentence: string; title: string; button: string }
> = {
  report: {
    fileName: 'gaply_project_report.md',
    mime: 'text/markdown',
    sentence: REPORT_EXPORT_SENTENCE,
    title: PROJECT_TEXT.exportReportTitle,
    button: PROJECT_TEXT.exportReport,
  },
  provenance: {
    fileName: 'gaply_project_provenance.json',
    mime: 'application/json',
    sentence: PROVENANCE_EXPORT_SENTENCE,
    title: PROJECT_TEXT.exportProvenanceTitle,
    button: PROJECT_TEXT.exportProvenance,
  },
};

export type Prepared = { ok: true; fileName: string; text: string; sha256: string } | { ok: false; sentence: string };

const isByte = (b: unknown) => typeof b === 'number' && Number.isInteger(b) && b >= 0 && b <= 255;

/** Lowercase hex SHA-256 of `bytes`, or null when it cannot be taken. */
async function sha256Hex(bytes: Uint8Array): Promise<string | null> {
  try {
    const d = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
    return Array.from(d, (b) => (b < 16 ? '0' : '') + b.toString(16)).join('');
  } catch {
    return null;
  }
}

/** The checks, in order: the name, the fingerprint, the exact text. */
export async function prepareExport(view: unknown, kind: ExportKind): Promise<Prepared> {
  const want = EXPORTS[kind].fileName;
  if (!view || typeof view !== 'object') return { ok: false, sentence: EXPORT_WORDS.notExact };
  const v = view as { fileName?: unknown; bytes?: unknown; sha256?: unknown };
  if (v.fileName !== want) return { ok: false, sentence: EXPORT_WORDS.wrongName };
  if (!Array.isArray(v.bytes) || !v.bytes.every(isByte) || typeof v.sha256 !== 'string') {
    return { ok: false, sentence: EXPORT_WORDS.notExact };
  }
  const bytes = Uint8Array.from(v.bytes as number[]);
  const taken = await sha256Hex(bytes);
  if (taken === null) return { ok: false, sentence: EXPORT_WORDS.notExact };
  if (taken !== v.sha256) return { ok: false, sentence: EXPORT_WORDS.wrongHash };
  let text: string;
  try {
    text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
  } catch {
    return { ok: false, sentence: EXPORT_WORDS.notExact };
  }
  const again = new TextEncoder().encode(text);
  if (again.length !== bytes.length || again.some((b, i) => b !== bytes[i])) {
    return { ok: false, sentence: EXPORT_WORDS.notExact };
  }
  return { ok: true, fileName: want, text, sha256: taken };
}
