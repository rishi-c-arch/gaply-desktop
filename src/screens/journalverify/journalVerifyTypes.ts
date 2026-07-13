// Gaply — Journal Verification (paid) wire types. Mirror the Rust serialization
// of journal_verify::JournalVerificationResult (Set 4), combining the grounded
// registry facts (journal_registry) + the labeled self-reported site summary
// (journal_site_summary). Evidence + signals, NEVER a verdict.

export interface YearCount {
  year: number;
  works: number;
}

/** Grounded registry facts — every field from a real registry; missing → named
 *  in `unverified`. NO model touched these. */
export interface JournalRegistryFacts {
  query: string;
  name: string | null;
  issn: string | null;
  doaj_registered: boolean | null;
  openalex_in_doaj: boolean | null;
  /** Present in the NLM Catalog (PubMed E-utilities); null = couldn't verify. */
  pubmed_indexed: boolean | null;
  works_by_year: YearCount[];
  recent_activity: boolean | null;
  scope: string[];
  /** Data-grounded caution ("what the data shows — not a verdict"), or null. */
  warning: string | null;
  reasons: string[];
  verified_sources: string[];
  unverified: string[];
  /** Scopus / Web of Science — not checked (no free API); not a signal. */
  sources_not_checked: string[];
}

export type SiteSummaryStatus = 'summarized' | 'site_unreachable' | 'llm_unavailable';

/** One self-reported detail — an extracted page-grounded value, or null ("not
 *  stated"). `dropped_unverifiable` = the model returned something not on the
 *  page, so it was dropped. */
export interface SiteDetail {
  field: string; // apc | review_timeline | guidelines_link | contact
  value: string | null;
  source: string;
  dropped_unverifiable: boolean;
}

/** The journal's OWN claims from its website — labeled self-reported/unverified,
 *  distinct from the grounded registry facts. */
export interface SiteSummary {
  status: SiteSummaryStatus;
  source_url: string;
  details: SiteDetail[];
  /** Required, never empty — the self-reported label. */
  label: string;
  notice: string | null;
  page_truncated: boolean;
}

export interface JournalMatch {
  name: string | null;
  issn: string | null;
}

/** The combined result. NO verdict — evidence + signals; the researcher decides. */
export interface JournalVerificationResult {
  input: string;
  input_kind: 'name' | 'link' | 'issn';
  /** Multiple name matches → the user disambiguates (never silently picked). */
  disambiguation: JournalMatch[];
  /** Not located in any registry — the honest alert (warning sign, not proof). */
  not_found: boolean;
  registry: JournalRegistryFacts | null;
  site_summary: SiteSummary | null;
  notes: string[];
}
