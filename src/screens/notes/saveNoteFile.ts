// Gaply — Note Creator file delivery. Delegates to the shared saveTextFile
// (src/utils/saveTextFile.ts) with the notes MIME type (text/markdown). Kept as
// a thin, named wrapper so the notes lane keeps its own vocabulary + the exact
// cancel-vs-failure contract it relies on (cancel → silent null, real write
// failure → throws).
import { saveTextFile } from '../../utils/saveTextFile';

/** Save Markdown to a file the user chooses. Returns the path (Tauri) or null
 *  (browser download, or the user cancelled the dialog — an honest no-op). */
export const saveNoteFile = (suggestedName: string, markdown: string): Promise<string | null> =>
  saveTextFile(suggestedName, markdown, 'text/markdown');
