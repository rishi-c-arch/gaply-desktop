// Gaply — the JournalFingerprint on the wire.
//
// Mirrors Rust `gaply_core::journal_fingerprint` exactly (snake_case, as
// serialized). §7's three kinds of knowledge are three arrays of three
// different types: there is no shared row type here, because the moment one
// exists a screen can render a convention as though it were a rule.

/**
 * What the journal said about which submissions a requirement governs.
 *
 * **A tagged union, not `string | null`, and that is load-bearing.** A journal
 * frequently states a limit without saying which submission it governs. `null`
 * or `''` in that slot invites `articleType || 'All'`, which prints a claim the
 * journal never made. There is nothing here to default.
 */
export type ArticleTypeLabel =
  | { kind: 'stated'; name: string }
  | { kind: 'not_stated' };

/** The one place NOT STATED becomes words. Never "All", never "Any". */
export function articleTypeText(a: ArticleTypeLabel): string {
  return a.kind === 'stated' ? a.name : 'NOT STATED';
}

export type FactStatus = 'verified' | 'inferred' | 'unavailable' | 'conflicted';

export interface RequirementView {
  kind: string;
  value: string;
  article_type: ArticleTypeLabel;
  status: FactStatus;
  source_url: string;
  source_heading: string;
  /** The journal's own sentence. Never empty — the schema refuses a row
   *  without one. Rendered whole: a truncated span is not a span. */
  source_span: string;
}

/** Two requirements that disagree. **No winner field, by construction.** */
export interface Conflict {
  kind: string;
  article_type: ArticleTypeLabel;
  /** Both sides. A renderer shows both and chooses neither. */
  values: RequirementView[];
}

export interface ConventionView {
  metric: string;
  median: number | null;
  iqr_low: number | null;
  iqr_high: number | null;
  n: number;
  /** Carries THE UNIT. §7 v6: the source supplies pages, not words, and a
   *  median rendered without its unit invites arithmetic nobody sanctioned. */
  detail: string;
  status: 'inferred' | 'unavailable';
}

export interface ExpectationView {
  claim: string;
  frequency_k: number | null;
  frequency_n: number | null;
  status: FactStatus;
  source_url: string;
  source_span: string;
}

export interface StandardBindingView {
  standard: string;
  design: string;
  source_url: string;
  source_span: string;
}

export interface FingerprintProvenance {
  journal_key: string;
  version: number;
  content_hash: string;
  /** Unix seconds. */
  fetched_at: number;
  refetch_after: number;
  source_count: number;
  quarantined_at: number | null;
  quarantine_reason: string | null;
}

export interface JournalFingerprint {
  journal_key: string;
  /** `null` when the journal has never been crawled — a state to render, not
   *  an empty fingerprint that looks fetched. */
  provenance: FingerprintProvenance | null;
  requirements: RequirementView[];
  conflicts: Conflict[];
  conventions: ConventionView[];
  expectations: ExpectationView[];
  standards: StandardBindingView[];
}

/** One row of the picker (item 9), all of it known BEFORE a run starts. */
export interface JournalProfileRow {
  key: string;
  name: string;
  entry: string;
  ingested: boolean;
  requirement_count: number;
  conflict_count: number;
  convention_count: number;
  expectation_count: number;
  standard_count: number;
  by_pattern: number;
  by_model: number;
  version: number | null;
  fetched_at: number | null;
  quarantine_reason: string | null;
}
