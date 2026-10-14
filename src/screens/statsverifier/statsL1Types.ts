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
  source: 'local' | 'ai_suggestion' | 'manual';
  reason: string | null;
}

export interface L1OpenView {
  run: number;
  claims: L1ClaimView[];
  notLocated: number;
  schema: L1SchemaView;
  proposals: L1ProposalView[];
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
      topic: 'assumptions' | 'reporting_guidance' | 'effect_size_reporting' | 'choosing_a_variant';
      editedQuery?: string | null;
      paper?: string | null;
    };

export type OptInCategory = 'column_headers' | 'claim_sentence' | 'methods_passage' | 'paper_doi_or_title';

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

export interface ReproductionView {
  counts: { reproduced: number; notReproduced: number; couldNotCheck: number };
  data: string;
  unmatchedPairs: number;
  claims: Array<{
    claimId: number;
    sectionIndex: number;
    paragraph: number;
    sentence: number;
    method: string | null;
    variant: string | null;
    variantSetBy: string | null;
    verdict:
      | { kind: 'reproduced'; notes: string[] }
      | { kind: 'notReproduced'; conclusionChanges: boolean | null }
      | { kind: 'couldNotCheck'; reason: string };
    notes: string[];
    candidates: Array<{
      variant: string;
      numbers: Array<{
        kind: string;
        printed: string;
        relation: string;
        computed: number;
        acceptedLo: number | null;
        acceptedHi: number | null;
        status: string;
      }>;
    }>;
    nUsed: number | null;
    rowsRead: number | null;
    exclusions: Array<{ reason: string; rows: number }>;
  }>;
}

export interface L1UsageView {
  tasks: Array<{ task: string; calls: number; inputTokens: number; outputTokens: number }>;
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
