// Gaply — the Stored data bridge (Research OS, Phases 5D-1a and 5D-1b): the
// one wrapper that calls the commands in src-tauri/src/stored_data_commands.rs.
// Rust counts and deletes; the section shows what comes back, after
// readStoredData, readConfirmation and readDeleted have checked its shape. A
// delete sends a category's id and the confirmation Rust issued for it, and
// nothing else.
import { isTauri } from '../../utils/isTauri';
import { STORED_CATEGORIES, StoredCategoryId } from './storedDataWording';

export const STORED_DATA_COMMANDS = {
  summary: 'stored_data_summary',
  prepareDelete: 'stored_data_prepare_delete',
  delete: 'stored_data_delete',
} as const;

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

/** The deletes (decision 213): a confirmation for one category, then its
 *  delete. Both exempt from the workspace check. */
export interface StoredDataDeleter {
  prepareDelete(category: StoredCategoryId): Promise<unknown>;
  delete(category: StoredCategoryId, confirmation: string): Promise<unknown>;
}

export const tauriStoredDataDeleter: StoredDataDeleter = {
  async prepareDelete(category) {
    if (!isTauri) throw new Error('Stored data is deleted by the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<unknown>(STORED_DATA_COMMANDS.prepareDelete, { category });
  },
  async delete(category, confirmation) {
    if (!isTauri) throw new Error('Stored data is deleted by the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<unknown>(STORED_DATA_COMMANDS.delete, { category, confirmation });
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

/** The confirmation Rust issued, or null when the answer has another shape:
 *  64 lowercase hex characters, nothing else of the answer kept. */
export function readConfirmation(v: unknown): string | null {
  const c = (v as { confirmation?: unknown } | null)?.confirmation;
  return typeof c === 'string' && /^[0-9a-f]{64}$/.test(c) ? c : null;
}

export interface StoredDeleted {
  deleted: number;
  outcome: 'done' | 'not_done';
  code: string | null;
}

/** What a delete of `category` did, or null when the answer is not for that
 *  category or has another shape. Only these three fields are kept. */
export function readDeleted(v: unknown, category: StoredCategoryId): StoredDeleted | null {
  const o = (v ?? {}) as { category?: unknown; deleted?: unknown; outcome?: unknown; code?: unknown };
  if (o.category !== category || !isCount(o.deleted)) return null;
  if (o.outcome === 'done' && o.code === undefined) return { deleted: o.deleted, outcome: 'done', code: null };
  if (o.outcome === 'not_done' && typeof o.code === 'string' && /^stored_data\.[a-z_]+$/.test(o.code)) {
    return { deleted: o.deleted, outcome: 'not_done', code: o.code };
  }
  return null;
}
