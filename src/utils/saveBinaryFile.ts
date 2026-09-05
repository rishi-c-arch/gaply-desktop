// Gaply — the single BINARY file save path (§11 D91), sibling to saveTextFile.
//
// It exists for the same reason that one does, and its header already said so:
// "The native dialog sidesteps the webview's download-scope limits." Text got
// that treatment; binary did not, so the PDF exports kept clicking an
// `<a download>` inside the desktop webview — where the download is handled by
// the webview rather than the OS, and the name and extension it is handed are
// not what it saves. The bytes were always a valid PDF; the delivery was not.
//
// CONTRACT (identical to saveTextFile, so the two cannot drift):
//  • the user CANCELLING the dialog → returns null (a silent no-op),
//  • a genuine write FAILURE → throws,
//  • success → the chosen path (Tauri) or null (browser download).
import { isTauri } from './isTauri';

/** The extension `name` ends with, lower-cased and without the dot. */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot > 0 && dot < name.length - 1 ? name.slice(dot + 1).toLowerCase() : '';
}

/**
 * Force `path` to carry `ext`.
 *
 * A dialog filter is a REQUEST to the OS; the returned path is the fact. macOS
 * can hide extensions, and a panel given no type at all may hand back a name
 * without one. Checking the result costs nothing and is the half that can be
 * tested without driving a real save panel.
 */
export function withExtension(path: string, ext: string): string {
  if (!ext) return path;
  return path.toLowerCase().endsWith(`.${ext}`) ? path : `${path}.${ext}`;
}

export async function saveBinaryFile(
  suggestedName: string,
  bytes: Uint8Array,
  mimeType = 'application/octet-stream',
): Promise<string | null> {
  const ext = extensionOf(suggestedName);
  if (isTauri) {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const chosen = await save({
      defaultPath: suggestedName,
      // NAME THE TYPE. Without filters the panel decides what the extension
      // means, which is how a .pdf stops being one.
      ...(ext ? { filters: [{ name: ext.toUpperCase(), extensions: [ext] }] } : {}),
    });
    if (!chosen) return null; // cancelled — honest no-op
    const path = withExtension(chosen, ext);
    const { writeFile } = await import('@tauri-apps/plugin-fs');
    await writeFile(path, bytes); // a real failure throws → propagated
    return path;
  }
  // Browser only. In a real browser the anchor download works and the MIME type
  // is what the receiving OS reads; it is the WEBVIEW that mishandles it.
  const blob = new Blob([bytes as unknown as BlobPart], { type: mimeType });
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
