// Gaply — TypeScript mirror of gaply_core::report::PublishReadyReport (the
// output of compile_report()). Field names match the serde-serialized shape
// exactly, so a future `compile_report` Tauri command drops straight in.
//
// This viewer is REUSED by the free checks and by paid PublishReady — it just
// renders whatever findings the report carries.

export type CertaintyTier =
  | 'mathematically_certain'
  | 'ai_assessed_moderate'
  | 'reconsidered_after_peer_review';

export type FindingSeverity = 'critical' | 'major' | 'minor' | 'info';

export type AgentKind =
  | 'extraction'
  | 'validation_maths'
  | 'ai_detection'
  | 'plagiarism'
  | 'rag'
  | 'verification';

export interface Finding {
  severity: FindingSeverity;
  tier: CertaintyTier;
  /** Human tier label, verbatim from Rust (e.g. "mathematically certain"). */
  certainty_label: string;
  agent: AgentKind;
  title: string;
  detail: string;
  confidence: number;
  /** Never empty: rule ids, evidence refs, weights, source URLs. */
  provenance: string[];
  /** Optional UI enrichment (present when the RevisingVerificationAgent changed
   *  a verdict): the before/after verdict for the reconsidered tier. Populated
   *  from the debate when available; the base compile_report Finding omits it. */
  reconsidered?: { from: string; to: string };
  /** Optional: which manuscript section this finding anchors to (for inline
   *  highlighting + the outline tree). */
  section?: string;
}

export interface ChecklistItem {
  requirement: string;
  passed: boolean;
  detail: string;
  guideline_source: string | null;
}

export interface DebateSummary {
  rounds_run: number;
  converged: boolean;
  overridden_by_constraint: boolean;
  rejected_agents: AgentKind[];
  revised_agents: AgentKind[];
}

/** Why a harness note exists. Mirrors gaply_core::report::HarnessNoteReason. */
export type HarnessNoteReason = 'output_rejected_by_internal_gate';

/**
 * A statement about GAPLY'S OWN RUN — never about the manuscript.
 *
 * "Verification output rejected by its internal gate" used to be a `minor`
 * FINDING and appeared in 20 of 22 stored reports, sitting in the same list,
 * with the same severity vocabulary, as real defects in the author's paper.
 * It is recorded here instead: still shown if asked, no longer counted as
 * something wrong with the manuscript.
 *
 * Optional because reports cached before the field existed do not carry it.
 */
export interface HarnessNote {
  agent: AgentKind;
  detail: string;
  reason: HarnessNoteReason;
}

export interface PublishReadyReport {
  verdict: string; // "pass" | "concern"
  combined_confidence: number;
  findings: Finding[];
  checklist: ChecklistItem[];
  debate: DebateSummary;
  /** Absent on reports cached before the field existed. */
  harness_notes?: HarnessNote[];
  disclaimer: string;
}

/* ------------------------------- helpers -------------------------------- */

/** The three certainty tiers → the design-system status colors. */
export function tierStatus(tier: CertaintyTier): 'certain' | 'assessed' | 'flagged' {
  switch (tier) {
    case 'mathematically_certain':
      return 'certain'; // 🟢
    case 'ai_assessed_moderate':
      return 'assessed'; // 🟡
    case 'reconsidered_after_peer_review':
      return 'flagged'; // 🔴
  }
}

const SEVERITY_RANK: Record<FindingSeverity, number> = {
  critical: 0,
  major: 1,
  minor: 2,
  info: 3,
};
const TIER_RANK: Record<CertaintyTier, number> = {
  mathematically_certain: 0,
  reconsidered_after_peer_review: 1,
  ai_assessed_moderate: 2,
};

/** Priority ordering, mirroring compile_report's sort so the viewer is robust
 *  even if handed an unsorted report: CRITICAL hard constraints first —
 *  severity outranks EVERYTHING including a high-confidence AI-assessed finding;
 *  then certainty tier, then confidence (desc). Stable. */
export function sortFindings(findings: Finding[]): Finding[] {
  return findings
    .map((f, i) => [f, i] as const)
    .sort(([a, ai], [b, bi]) => {
      const s = SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity];
      if (s !== 0) return s;
      const t = TIER_RANK[a.tier] - TIER_RANK[b.tier];
      if (t !== 0) return t;
      const c = b.confidence - a.confidence;
      if (c !== 0) return c;
      return ai - bi; // stable
    })
    .map(([f]) => f);
}

export const AGENT_LABEL: Record<AgentKind, string> = {
  extraction: 'Extraction',
  validation_maths: 'Validation / Maths',
  ai_detection: 'AI Detection',
  plagiarism: 'Plagiarism',
  rag: 'RAG · Journal',
  verification: 'Verification',
};

export const REPORT_TABS = [
  'Overview',
  'Statistics',
  'Citations',
  'AI Risk',
  'Plagiarism',
  'Checklist',
] as const;
export type ReportTab = (typeof REPORT_TABS)[number];

/** Which agents' findings belong under each tab. */
export function findingsForTab(findings: Finding[], tab: ReportTab): Finding[] {
  switch (tab) {
    case 'Statistics':
      return findings.filter((f) => f.agent === 'validation_maths');
    case 'Citations':
      return findings.filter((f) => f.agent === 'verification');
    case 'AI Risk':
      return findings.filter((f) => f.agent === 'ai_detection');
    case 'Plagiarism':
      return findings.filter((f) => f.agent === 'plagiarism');
    case 'Overview':
    default:
      return findings;
  }
}
