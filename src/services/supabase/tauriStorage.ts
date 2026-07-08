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
    } catch {
      /* swallow — a failed persist must not break sign-in */
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
