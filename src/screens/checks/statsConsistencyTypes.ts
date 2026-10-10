// Gaply — the Level 0 statistics-consistency view, as `run_stats_consistency`
// returns it. Mirrors `StatsConsistencyView` in src-tauri/src/stats_consistency.rs;
// the wire shape is pinned key by key in wire_contract_tests.rs
// (`the_level_0_view_is_camel_case_key_by_key`). Counts only: no ratio, score
// or percentage crosses.

/** `Location` declares no casing in Rust and is snake_case on the wire. */
export interface L0Location {
  section: string;
  paragraph: number;
  section_index?: number;
}

export interface L0Words {
  code: string;
  text: string;
}

export interface L0ReasonView {
  methodName: string;
  code: string;
  text: string;
}

export type L0VerdictView =
  /** `conclusionChanges: null` means it was not assessed. */
  | { kind: 'inconsistent'; conclusionChanges: boolean | null }
  | { kind: 'concern'; reason: L0Words }
  | { kind: 'couldNotCheck'; reasons: L0ReasonView[] }
  | { kind: 'consistent'; note: L0Words | null };

export type L0VerdictKind = L0VerdictView['kind'];

export interface L0MethodCheckView {
  methodId: string;
  methodName: string;
  engineVersion: string;
  provenance: string;
}

export interface L0ResultView {
  id: number;
  location: L0Location;
  /** The printed unit, exactly as printed. */
  raw: string;
  /** The reported p's digits exactly as printed; null for `ns`. */
  reportedP: string | null;
  reportedPRelation: string | null;
  recomputedP: { lo: number; hi: number } | null;
  verdict: L0VerdictView;
  /** Required: a card cannot render without the check its verdict rests on. */
  check: L0MethodCheckView;
}

export interface StatsConsistencyView {
  claimsRead: number;
  verdictCounts: { inconsistent: number; concern: number; couldNotCheck: number; consistent: number };
  notReadCount: number;
  notRead: { code: string; text: string; count: number }[];
  alpha: { values: string[]; assumed: boolean; note: string | null };
  results: L0ResultView[];
}
