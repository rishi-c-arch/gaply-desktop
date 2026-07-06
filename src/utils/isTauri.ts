/**
 * True when running inside the Tauri desktop webview (dev or production).
 * Tauri 2 injects `__TAURI_INTERNALS__` into every app window.
 */
export const isTauri: boolean =
  typeof window !== 'undefined' &&
  ('__TAURI_INTERNALS__' in window || window.location.protocol === 'tauri:');

export default isTauri;
