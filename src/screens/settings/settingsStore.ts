// Gaply — F14 settings store: per-suite cloud-consent (privacy) toggles,
// appearance, and the local-data inventory. Everything persists to
// localStorage under the `gaply.` prefix. Supabase keys (`sb-*`) are NEVER
// touched here — account data lives server-side and "Clear local data" must
// not sign the user out or delete anything from Supabase.

/* ------------------------------ cloud consent ---------------------------- */

/** The suites that are allowed to talk to the cloud AT ALL. The five local
 *  agents (extraction, validation, AI check, local plagiarism, report view)
 *  are deliberately absent: they have no cloud path to toggle. */
export type CloudSuite =
  | 'citation_verification'
  | 'journal_check'
  | 'publishready'
  | 'research_copilot'
  | 'community_sync';

export const CLOUD_SUITES: Array<{ id: CloudSuite; label: string; sends: string }> = [
  { id: 'citation_verification', label: 'Citation verification', sends: 'DOIs & titles → CrossRef/OpenAlex via the Gaply proxy' },
  { id: 'journal_check', label: 'Journal Check online fallback', sends: 'journal names not in the offline directory' },
  { id: 'publishready', label: 'PublishReady ★', sends: 'structured findings only — never manuscript text' },
  { id: 'research_copilot', label: 'Research Copilot ★', sends: 'your questions + structured findings — never manuscript text' },
  { id: 'community_sync', label: 'Community posts', sends: 'your posts + integrity-badge metadata' },
];

const PRIVACY_KEY = 'gaply.settings.privacy';
const APPEARANCE_KEY = 'gaply.settings.appearance';

/** Prefix that scopes what "Clear local data" may wipe. */
export const LOCAL_DATA_PREFIX = 'gaply.';

function defaultStorage(): Storage | null {
  return typeof localStorage === 'undefined' ? null : localStorage;
}

/** Read the consent map. Missing/unreadable → everything allowed (consent is
 *  an opt-OUT: suites default to usable, the toggle is the off switch). */
export function readCloudConsent(storage: Storage | null = defaultStorage()): Record<CloudSuite, boolean> {
  const all = Object.fromEntries(CLOUD_SUITES.map((s) => [s.id, true])) as Record<CloudSuite, boolean>;
  if (!storage) return all;
  try {
    const raw = storage.getItem(PRIVACY_KEY);
    if (!raw) return all;
    const parsed = JSON.parse(raw) as Partial<Record<CloudSuite, boolean>>;
    for (const s of CLOUD_SUITES) if (typeof parsed[s.id] === 'boolean') all[s.id] = parsed[s.id]!;
    return all;
  } catch {
    return all;
  }
}

export function setCloudConsent(
  suite: CloudSuite,
  allowed: boolean,
  storage: Storage | null = defaultStorage()
): Record<CloudSuite, boolean> {
  const next = { ...readCloudConsent(storage), [suite]: allowed };
  storage?.setItem(PRIVACY_KEY, JSON.stringify(next));
  return next;
}

/** THE gate. Every cloud call-site asks this before touching the network;
 *  false means the user turned the suite's cloud access off in Settings. */
export function mayUseCloud(suite: CloudSuite, storage: Storage | null = defaultStorage()): boolean {
  return readCloudConsent(storage)[suite];
}

/* -------------------------------- appearance ----------------------------- */

export interface Appearance {
  /** 'darkest' is the shipped default (pure-black base); 'dark' lifts the
   *  surfaces one step for lower-contrast displays. */
  theme: 'darkest' | 'dark';
  /** UI scale in percent (applied as a font-size multiplier on .gds-root). */
  uiScale: 90 | 100 | 110;
}

export const DEFAULT_APPEARANCE: Appearance = { theme: 'darkest', uiScale: 100 };

export function readAppearance(storage: Storage | null = defaultStorage()): Appearance {
  if (!storage) return DEFAULT_APPEARANCE;
  try {
    const raw = storage.getItem(APPEARANCE_KEY);
    if (!raw) return DEFAULT_APPEARANCE;
    const p = JSON.parse(raw) as Partial<Appearance>;
    return {
      theme: p.theme === 'dark' ? 'dark' : 'darkest',
      uiScale: p.uiScale === 90 || p.uiScale === 110 ? p.uiScale : 100,
    };
  } catch {
    return DEFAULT_APPEARANCE;
  }
}

export function writeAppearance(a: Appearance, storage: Storage | null = defaultStorage()): void {
  storage?.setItem(APPEARANCE_KEY, JSON.stringify(a));
}

/* ------------------------------- local data ------------------------------ */

export interface LocalDataEntry {
  key: string;
  bytes: number;
}

/** Enumerate gaply-owned localStorage entries (key + UTF-16 payload bytes). */
export function listLocalData(storage: Storage | null = defaultStorage()): LocalDataEntry[] {
  if (!storage) return [];
  const entries: LocalDataEntry[] = [];
  for (let i = 0; i < storage.length; i += 1) {
    const key = storage.key(i);
    if (!key || !key.startsWith(LOCAL_DATA_PREFIX)) continue;
    entries.push({ key, bytes: (key.length + (storage.getItem(key)?.length ?? 0)) * 2 });
  }
  return entries.sort((a, b) => a.key.localeCompare(b.key));
}

export function localBytesUsed(storage: Storage | null = defaultStorage()): number {
  return listLocalData(storage).reduce((sum, e) => sum + e.bytes, 0);
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Wipe ONLY gaply-prefixed local data. Purely local: never calls the
 *  network, and never removes Supabase (`sb-*`) auth/account keys — the user
 *  stays signed in and their account rows are untouched. Returns the number
 *  of keys removed. */
export function clearLocalData(storage: Storage | null = defaultStorage()): number {
  if (!storage) return 0;
  const doomed = listLocalData(storage).map((e) => e.key);
  for (const key of doomed) storage.removeItem(key);
  return doomed.length;
}
