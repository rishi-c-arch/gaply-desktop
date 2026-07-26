// Gaply — Tauri Store-backed session storage for Supabase auth.
//
// WHY: in the desktop app the webview origin differs between `tauri dev`
// (http://localhost:3000) and the bundled build (tauri://localhost /
// http://tauri.localhost). localStorage is keyed per-origin, so a session
// saved under one origin is invisible under the other — sign-ins wouldn't
// survive a dev→prod transition or, on some platforms, an app restart. A Tauri
// Store file (`auth.json` in the app data dir) is origin-independent and
// persists reliably.
//
// This adapter is ONLY used inside Tauri (guarded by isTauri at the call site).
// In a plain browser build the Supabase client keeps its default localStorage,
// so nothing here is imported/executed on the web/Vercel path.

import type { SupportedStorage } from '@supabase/supabase-js';

// Lazily created Store handle. We import the plugin dynamically so the module
// graph never pulls Tauri IPC into the web bundle's synchronous path.
let storePromise: Promise<any> | null = null;

async function store(): Promise<any> {
  if (!storePromise) {
    storePromise = import('@tauri-apps/plugin-store').then(({ load }) =>
      // autoSave debounces writes to disk; we also save() explicitly on
      // set/remove. `defaults` is required by this plugin version's StoreOptions.
      load('auth.json', { defaults: {}, autoSave: true })
    );
  }
  return storePromise;
}

/** Async Store-backed storage; Supabase supports async storage adapters. */
export const tauriSessionStorage: SupportedStorage = {
  async getItem(key: string): Promise<string | null> {
    try {
      const s = await store();
      return ((await s.get(key)) as string | null) ?? null;
    } catch {
      return null; // never throw into the auth flow
    }
  },
  async setItem(key: string, value: string): Promise<void> {
    try {
      const s = await store();
      await s.set(key, value);
      await s.save();
    } catch (e) {
      // A failed persist must not BREAK sign-in — but it must not be INVISIBLE
      // either: a swallowed verifier write is exactly what produces a later
      // "PKCE code verifier not found". Surface it (devtools + terminal/Console),
      // then continue.
      await reportStorageError('setItem', key, e);
    }
  },
  async removeItem(key: string): Promise<void> {
    try {
      const s = await store();
      await s.delete(key);
      await s.save();
    } catch {
      /* swallow */
    }
  },
};

/** Diagnostic-only: is a Supabase PKCE code-verifier currently persisted in the
 *  auth store? Returns a boolean — NEVER the value. Used to pin OAuth
 *  "verifier not found" failures to the write side vs the read side. */
export async function hasCodeVerifier(): Promise<boolean> {
  try {
    const s = await store();
    const keys = (await s.keys()) as string[];
    return keys.some((k) => k.endsWith('-code-verifier'));
  } catch {
    return false;
  }
}

/** Surface an auth-storage write failure without breaking the flow: to the
 *  webview console AND (redacted) to the Rust log so it shows in
 *  terminal/Console. Logs the KIND of key + the error message — never the key's
 *  value. */
async function reportStorageError(operation: string, key: string, e: unknown): Promise<void> {
  const message = e instanceof Error ? e.message : String(e);
  const keyKind = key.endsWith('-code-verifier')
    ? 'code_verifier'
    : key.includes('auth-token')
      ? 'session'
      : 'other';
  // eslint-disable-next-line no-console
  console.error(`[auth-storage] ${operation} failed (${keyKind}): ${message}`);
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('log_storage_error', { operation, keyKind, message });
  } catch {
    /* best-effort — logging must never throw here */
  }
}
