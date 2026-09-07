// Gaply — one mapping from a stored job/command result to the D18 unit.
//
// This existed twice: inline in ThesisAuditScreen's drill-down and again as
// `toFinding` in CitationAiPanel. The per-citation check is a SLICE of the
// thesis audit (§11 D54) and renders the same rows from the same job items, so
// a second copy would be a second place for the evidence mapping to drift —
// and evidence mapping is the thing D18 exists to keep honest.
import { EvidenceRow, GroundedFinding, Verdict } from './EvidenceCard';

export interface FindingContext {
  /** Which document the cited chunks belong to, for the page links. */
  documentId: number;
  /** How the source is named in the evidence rows. */
  sourceLabel: string;
  /** False when the source file has moved since indexing. */
  fileAvailable?: boolean;
}

/** A job item's `result_json`, or a command's result payload, as the D18 unit. */
export function resultToFinding(raw: any, ctx: FindingContext): GroundedFinding {
  const out = raw?.output ?? {};
  const rows = (chunks: any[]): EvidenceRow[] =>
    chunks.map((c) => ({
      chunkId: String(c.chunkId ?? c.chunk_id ?? ''),
      documentId: ctx.documentId,
      page: typeof c.page === 'number' ? c.page : null,
      sourceLabel: ctx.sourceLabel,
      // v2 asks for a verbatim `quote`; v1 only has `why`. Either is the text
      // shown beside the prose — never nothing.
      quote: String(c.quote ?? c.why ?? c.text ?? ''),
      fileAvailable: ctx.fileAvailable ?? true,
    }));

  return {
    verdict: (out.verdict as Verdict) ?? 'no_evidence',
    confidence: typeof out.confidence === 'number' ? out.confidence : null,
    explanation: String(out.explanation ?? raw?.reason ?? ''),
    advisories: raw?.advisories ?? [],
    evidence: rows(out.supporting_chunks ?? []),
    // What the check SEARCHED, so an insufficient_evidence verdict is shown
    // against something real rather than refused (§11 D52).
    examined: rows(raw?.examinedPassages ?? []),
    chunksSent: typeof raw?.chunksSent === 'number' ? raw.chunksSent : undefined,
    // §11 D108. Carried through with the evidence, for the same reason: it is
    // what the model got right while the verdict did not.
    claimElements: Array.isArray(out.claim_elements)
      ? out.claim_elements
          .filter((e: any) => typeof e?.element === 'string')
          .map((e: any) => ({ element: String(e.element), status: String(e.status ?? 'unmarked') }))
      : undefined,
  };
}
