// Gaply — desktop file-path acquisition.
//
// In a Tauri v2 webview an `<input type="file">` File object has NO usable
// `.path`, so passing `file.name` to the Rust commands sends a BARE filename the
// core can't open (it resolves against the app CWD, not the file's real dir) →
// "check failed". The correct desktop flow is to get the ABSOLUTE path from the
// Tauri dialog plugin. This extracts the proven pattern from AnalysisTheaterPage.
//
// The dialog plugin (`@tauri-apps/plugin-dialog`) and its `dialog:default`
// permission are already wired (package.json / Cargo.toml / lib.rs / capabilities).
// The plugin is DYNAMIC-imported inside each fn so tests can `vi.mock` it and so
// the module stays importable in a plain browser (vitest) where it's never called.

/** Native single-file picker → the chosen file's ABSOLUTE path, or null if the
 *  user cancelled. `extensions` are bare (no dot), e.g. ['pdf','docx']. */
export async function pickManuscriptPath(
  extensions: string[],
  title = 'Manuscript',
): Promise<string | null> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const chosen = await open({
    multiple: false,
    directory: false,
    filters: [{ name: title, extensions }],
  });
  return typeof chosen === 'string' ? chosen : null;
}

/** Native multi-file picker → the chosen files' ABSOLUTE paths (empty if
 *  cancelled). Used by the Set-2 multi-file sites (e.g. Gap Finder). */
export async function pickManuscriptPaths(
  extensions: string[],
  title = 'Manuscripts',
): Promise<string[]> {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const chosen = await open({
    multiple: true,
    directory: false,
    filters: [{ name: title, extensions }],
  });
  return Array.isArray(chosen) ? chosen : typeof chosen === 'string' ? [chosen] : [];
}

/** Display name from an absolute path (handles both / and \ separators). */
export function basenameOf(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}
