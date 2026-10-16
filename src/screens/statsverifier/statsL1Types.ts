// Gaply — Level 1 (does the data reproduce each printed result?) wire types.
// Mirrors the camelCase views in src-tauri/src/stats_l1_commands.rs and
// stats_reproduction.rs. Model text arrives in fields named modelReason,
// modelText or (context) sentence, and is rendered as plain text only.

/** A closed reason code with numeric or nested-code parameters. */
export interface WireCode {
  code: string;
  [param: string]: string | number | WireCode;
}

/** Errors reject with `{ code, message }`; for Level 1, `message` is a
 *  WireCode in JSON — never a path, a header or manuscript text. */
export interface L1Handle {
  run: number;
  id: number;
}

export interface L1ClaimView {
  claimId: number;
  revision: string;
  sectionIndex: number;
  section: string;
  paragraph: number;
  sentenceIndex: number;
  sentence: string;
  family: 't' | 'F' | 'chi_square' | 'r' | 'z';
  statistic: string;
  df: string[];
  n: string | null;
  p: string;
  level0: WireCode | null;
}

export interface L1LevelView {
  code: string;
  label: string | null;
  count: number;
}

export interface L1ColumnView {
  alias: string;
  header: string;
  columnType: string;
  hasMissing: boolean;
  levelBucket: 'two' | 'three_to_ten' | 'more_than_ten' | null;
  levels: L1LevelView[];
  issues: WireCode[];
}

export interface L1SchemaView {
  rows: number;
  columns: L1ColumnView[];
  issues: WireCode[];
}

export interface L1ProposalView {
  proposal: L1Handle;
  claimId: number;
  method: string;
  roles: Array<{ role: string; aliases: string[] }>;
  levels: Array<{ role: string; levels: string[] }>;
  variant: string;
  /** `stated_in_paper` only under Step 2's word-list rule. */
  variantSetBy: WireCode;
  source: 'local' | 'ai_suggestion' | 'manual';
  reason: string | null;
}

export interface L1SlotView {
  role: string;
  kind: 'numeric' | 'group' | 'numeric_list';
  /** A group slot's level rule: `exactly` or `at_least`, with `levels`. */
  levels: WireCode | null;
  min: number | null;
}

export interface L1MethodView {
  method: string;
  slots: L1SlotView[];
  /** The stated variants a user may tick; empty when the method has none. */
  variants: string[];
  needsTarget: boolean;
}

export interface L1OpenView {
  run: number;
  claims: L1ClaimView[];
  notLocated: number;
  schema: L1SchemaView;
  proposals: L1ProposalView[];
  catalogue: L1MethodView[];
}

export interface L1RevisionView {
  claimId: number;
  revision: string;
}

export interface L1ClaimInput {
  family: 't' | 'F' | 'chi_square' | 'r' | 'z';
  statistic: string;
  df: string[];
  n?: string | null;
  /** null for a p printed only as `ns`. */
  p: string | null;
  pRelation?: '=' | '<' | '<=' | '>' | '>=' | null;
  sentence?: string | null;
}

export type HelperTaskInput =
  | { task: 'claim_method'; claimId: number; methodsPassage?: string | null }
  | { task: 'binding_suggestion'; claimId: number; methodId: string }
  | { task: 'result_explanation'; claimId: number }
  | {
      task: 'web_context';
      claimId: number;
      methodId: string;
      /** A variant word from the method's catalogue entry, or null for none. */
      variant?: string | null;
      topic: 'assumptions' | 'reporting_guidance' | 'effect_size_reporting' | 'choosing_a_variant';
      /** Only a DOI (evaluation fixes, part B): the query is never typed. */
      doi?: string | null;
    };

export type OptInCategory = 'column_headers' | 'claim_sentence' | 'methods_passage' | 'paper_doi';

export interface L1PreviewView {
  preview: L1Handle;
  task: string;
  /** The exact request body. */
  body: string;
  sha256: string;
  categories: string[];
}

export interface L1ContextItemView {
  sentence: string;
  sources: Array<{ url: string; title: string; domain: string }>;
  retrievedAt: number;
  querySent: string;
}

export type L1HelperResultView =
  | {
      kind: 'claimMethod';
      proposal: L1Handle | null;
      method: string | null;
      variantHint: string | null;
      modelReason: string;
      reliedOn: string[];
    }
  | {
      kind: 'bindingSuggestion';
      proposal: L1Handle;
      roles: Array<{ role: string; alias: string | null; levelsToSelect: number | null }>;
      modelReason: string;
      reliedOn: string[];
    }
  | { kind: 'explanation'; modelText: string }
  | {
      kind: 'context';
      items: L1ContextItemView[];
      uncitedSentences: number;
      droppedCitations: string[];
      emptyMessage: string | null;
    };

export interface BindingEdits {
  methodId: string;
  roles: Array<{ role: string; aliases: number[] }>;
  levels: Array<{ role: string; levels: string[] }>;
  filters: Array<{ alias: number; levels: string[] }>;
  variantTicked?: string | null;
  olsOverallF?: boolean | null;
  olsPredictor?: number | null;
}

export interface L1ConfirmView {
  /** The token handle; compute takes it back. */
  token: string;
  claimId: number;
  rowsRead: number;
  rowsFilteredOut: number;
  rowsExcludedMissing: number;
  nUsed: number;
  suspectMissingCodes: Array<{ alias: string; code: number; cells: number }>;
}

export interface L1NumberView {
  kind: WireCode;
  /** Exactly as printed; `ns` for a p printed only as not significant. */
  printed: string;
  relation: string;
  computed: number;
  acceptedLo: number | null;
  acceptedHi: number | null;
  status: WireCode;
  /** What the stored cells' rounding could make the computed value; null
   *  when that range is zero (Step 8b). */
  precisionLo: number | null;
  precisionHi: number | null;
}

export interface L1ProvenanceView {
  methodId: string;
  reference: string | null;
  methodVersion: string;
  engineVersion: string;
  inputsDigest: string;
  bindingSource: WireCode;
  variantSetBy: WireCode;
}

export type L1VerdictView =
  | { kind: 'reproduced'; notes: WireCode[] }
  | { kind: 'notReproduced'; conclusionChanges: boolean | null }
  | { kind: 'couldNotCheck'; reason: WireCode };

export interface L1ClaimReproductionView {
  claimId: number;
  sectionIndex: number;
  paragraph: number;
  sentence: number;
  method: string | null;
  variant: WireCode | null;
  variantSetBy: WireCode | null;
  verdict: L1VerdictView;
  notes: WireCode[];
  candidates: Array<{ variant: WireCode; numbers: L1NumberView[]; storedDecimals: number | null }>;
  nUsed: number | null;
  rowsRead: number | null;
  exclusions: Array<{ reason: WireCode; rows: number }>;
  provenance: L1ProvenanceView | null;
}

/** Every reason, note, status and variant is a closed code from
 *  gaply_core::stats_l1::codes, worded by l1/l1Wording.ts. */
export interface ReproductionView {
  counts: { reproduced: number; notReproduced: number; couldNotCheck: number };
  data: WireCode;
  unmatchedPairs: number;
  claims: L1ClaimReproductionView[];
  /** The significance threshold p was compared with, as the paper printed
   *  it, and whether the paper stated it (otherwise .05, the default). */
  alpha: { printed: string[]; stated: boolean };
}

export interface L1UsageView {
  /** `requests`: every request that left the device for the task, each
   *  attempt, retries and failed calls included; the tokens are what the
   *  replies reported. */
  tasks: Array<{ task: string; requests: number; inputTokens: number; outputTokens: number }>;
}

export interface AiSettings {
  aiHelp: boolean;
  webContext: boolean;
  model: string;
}

export interface AiKeyStatusView {
  saved: boolean;
  provider: string;
}

export interface AiKeyTestView {
  result: string;
}
