// Gaply — Note Creator file delivery. An ISOLATED copy of the citation
// exporters' proven saveExportToFile (citations/exporters.ts): the native Tauri
// save dialog + writeTextFile in the desktop app, a Blob download in the browser.
// Kept local ON PURPOSE (isolation over DRY) so the shipped citation lane is not
// touched. FUTURE WORK: extract a shared src/utils/saveTextFile.ts and have both
// features use it. This is the file-save path (NOT the PDF opener path) — the
// native dialog sidesteps the webview's download-scope limits, so it just works.
import { isTauri } from '../../utils/isTauri';

/** Save Markdown to a file the user chooses. Returns the path (Tauri) or null
 *  (browser download, or the user cancelled the dialog — an honest no-op). */
export async function saveNoteFile(suggestedName: string, markdown: string): Promise<string | null> {
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({ defaultPath: suggestedName });
    if (!path) return null; // user cancelled
    const { writeTextFile } = await import('@tauri-apps/plugin-fs');
    await writeTextFile(path, markdown);
    return path;
  }
  // Browser fallback (web build): the same blob pattern the exporters use.
  const blob = new Blob([markdown], { type: 'text/markdown' });
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
