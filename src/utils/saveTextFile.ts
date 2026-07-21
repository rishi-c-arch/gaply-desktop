// Gaply — the single text-file save path, shared by the Note Creator and the
// Citation Manager (extracted from their two identical copies). The native Tauri
// save dialog + writeTextFile in the desktop app; a Blob download in the browser.
// The native dialog sidesteps the webview's download-scope limits.
//
// CONTRACT (preserved exactly from both callers):
//  • the user CANCELLING the dialog → returns null (a silent no-op),
//  • a genuine write FAILURE → throws (writeTextFile rejects, propagated),
//  • success → returns the chosen path (Tauri) or null (browser download).
// The ONLY thing that differed between the two callers was the browser Blob MIME
// type (notes: text/markdown, citations: text/plain) — kept as a parameter so
// each lane's behaviour is byte-for-byte unchanged.
import { isTauri } from './isTauri';

export async function saveTextFile(
  suggestedName: string,
  text: string,
  mimeType = 'text/plain',
): Promise<string | null> {
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: suggestedName });
    if (!path) return null; // user cancelled — honest no-op
    const { writeTextFile } = await import('@tauri-apps/plugin-fs');
    await writeTextFile(path, text); // a real failure throws → propagated
    return path;
  }
  // Browser fallback: the blob download pattern both callers used.
  const blob = new Blob([text], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = suggestedName;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
  return null;
}
