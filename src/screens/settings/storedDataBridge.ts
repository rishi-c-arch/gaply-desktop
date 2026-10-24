// Gaply — the Stored data bridge (Research OS, Phase 5D-1a): the one wrapper
// that calls stored_data_summary in src-tauri/src/stored_data_commands.rs.
// Rust counts; the section shows what comes back, after readStoredData has
// checked its shape. It sends nothing.
import { isTauri } from '../../utils/isTauri';
import { STORED_CATEGORIES, StoredCategoryId } from './storedDataWording';

export const STORED_DATA_COMMANDS = { summary: 'stored_data_summary' } as const;

export interface StoredDataBridge {
  /** Exempt from the workspace check (decision 206). */
  summary(): Promise<unknown>;
}

export const tauriStoredDataBridge: StoredDataBridge = {
  async summary() {
    if (!isTauri) throw new Error('Stored data is read by the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<unknown>(STORED_DATA_COMMANDS.summary);
  },
};

export interface StoredCount {
  items: number;
  bytes: number;
}

const isCount = (n: unknown): n is number => typeof n === 'number' && Number.isSafeInteger(n) && n >= 0;

/** The nine counts by category, or null when the answer is not exactly
 *  nine categories, each named once, each with two whole counts. Only the
 *  two counts are kept: nothing else in the answer reaches the screen. */
export function readStoredData(v: unknown): Map<StoredCategoryId, StoredCount> | null {
  const categories = (v as { categories?: unknown } | null)?.categories;
  if (!Array.isArray(categories) || categories.length !== STORED_CATEGORIES.length) return null;
  const known = new Set<string>(STORED_CATEGORIES.map((c) => c.id));
  const out = new Map<StoredCategoryId, StoredCount>();
  for (const c of categories) {
    const { category, items, bytes } = (c ?? {}) as { category?: unknown; items?: unknown; bytes?: unknown };
    if (typeof category !== 'string' || !known.has(category) || out.has(category as StoredCategoryId)) return null;
    if (!isCount(items) || !isCount(bytes)) return null;
    out.set(category as StoredCategoryId, { items, bytes });
  }
  return out;
}
