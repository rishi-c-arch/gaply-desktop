// Gaply — the ONLY way the frontend obtains journal facts.
//
// Both calls are `invoke` to the local backend. Neither fetches anything: the
// backend already crawled, classified, extracted and stored, and these read
// what it stored. That is the invariant `network_invariant.vitest.ts` enforces
// structurally — this file is its positive half, the path that is allowed.
import { isTauri } from '../../../utils/isTauri';
import { JournalFingerprint, JournalProfileRow } from './fingerprintTypes';

export interface JournalFingerprintBridge {
  /** The ten profiled journals with what ingestion found for each. */
  profiles(): Promise<JournalProfileRow[]>;
  /** One journal's full fingerprint. Never throws for an uncrawled journal —
   *  it returns `provenance: null` and empty arrays. */
  fingerprint(journalKey: string): Promise<JournalFingerprint>;
}

export const tauriJournalFingerprintBridge: JournalFingerprintBridge = {
  async profiles() {
    if (!isTauri) return [];
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<JournalProfileRow[]>('journal_profiles');
  },
  async fingerprint(journalKey: string) {
    if (!isTauri) {
      return {
        journal_key: journalKey,
        provenance: null,
        requirements: [],
        conflicts: [],
        conventions: [],
        expectations: [],
        standards: [],
      };
    }
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<JournalFingerprint>('journal_fingerprint', { journalKey });
  },
};
