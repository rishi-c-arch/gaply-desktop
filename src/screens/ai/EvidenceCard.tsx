// Gaply — THE D18 unit. AI prose never renders without its evidence.
//
// D18 (docs/AI_ENGINE_PLAN.md) fixed what the engine actually guarantees: a
// stored card cannot cite a passage that was not retrieved, and cannot claim a
// page the store disagrees with. It guarantees NOTHING about the prose. The
// smoke case that made this concrete: evidence saying "No significant effect
// was observed", explanation asserting "a significant increase ... which
// supports the claim". Every chunk_id real, every page correct, the finding
// fabricated.
//
// So the rule this component exists to enforce: **the explanation is never
// shown alone.** It is not a convention here, it is the type — see
// `EvidenceCardProps`, where `explanation` and `evidence` arrive together in one
// required object and there is no way to pass one without the other.
import './ai.css';
import React, { useState } from 'react';
import { Badge, BadgeStatus } from '../../design-system/primitives';

/**
 * LAZY on purpose. react-pdf pulls in pdfjs and a worker, which is heavy and
 * has side effects at import time — enough that merely having it in a module
 * graph disturbed an unrelated screen's tests. Nothing should pay for the
 * viewer until someone opens one.
 */
const PdfViewer = React.lazy(() => import('./PdfViewer'));

/** The five spec verdicts, plus the engine's no-generation outcome. */
export type Verdict =
  | 'strong'
  | 'partial'
  | 'weak'
  | 'contradicts'
  | 'insufficient_evidence'
  | 'no_evidence';

/**
 * Verdict → the EXISTING BadgeStatus scheme. Deliberately not a parallel one:
 * `certain` is reserved for the deterministic path, and everything a model
 * produced is `assessed` or `flagged` — a reader who has learned the report
 * colours elsewhere does not have to learn a second vocabulary here.
 */
const VERDICT_STATUS: Record<Verdict, BadgeStatus> = {
  strong: 'assessed',
  partial: 'assessed',
  weak: 'assessed',
  contradicts: 'flagged',
  insufficient_evidence: 'neutral',
  no_evidence: 'neutral',
};

/** Never colour alone: each verdict carries a distinct glyph and wording. */
const VERDICT_GLYPH: Record<Verdict, string> = {
  strong: '✓',
  partial: '≈',
  weak: '!',
  contradicts: '✕',
  insufficient_evidence: '?',
  no_evidence: '—',
};

const VERDICT_LABEL: Record<Verdict, string> = {
  strong: 'Supported',
  partial: 'Partially supported',
  weak: 'Weak support',
  contradicts: 'Contradicted',
  insufficient_evidence: 'Insufficient evidence',
  no_evidence: 'No evidence retrieved',
};

/** One retrieved passage, as the engine actually recorded it. */
export interface EvidenceRow {
  chunkId: string;
  documentId: number;
  /** Null when the source records no reliable pagination. NEVER invented. */
  page: number | null;
  /** How the source is cited, e.g. "Smith 2019". */
  sourceLabel: string;
  /** The chunk text the verdict rests on. Shown inline, always. */
  quote: string;
  /** False when the file has moved since indexing. */
  fileAvailable: boolean;
}

/**
 * The D18 invariant, expressed as a type.
 *
 * `explanation` cannot be passed without `evidence`, because they are one
 * object. A caller that has prose but no evidence has nothing this component
 * will render — which is the point.
 */
export interface GroundedFinding {
  verdict: Verdict;
  /** 0–1, as the model reported it. */
  confidence: number | null;
  explanation: string;
  /** Style/length deviations the output was ACCEPTED with. Shown, never dropped. */
  advisories?: string[];
  evidence: EvidenceRow[];
  /**
   * The passages the check EXAMINED, best match first — not passages the model
   * cited. Distinct from `evidence` on purpose, and never interchangeable with
   * it: `evidence` is what a verdict rests ON, this is what it was looked FOR
   * in. Only meaningful for a verdict that cites nothing.
   */
  examined?: EvidenceRow[];
  /** How many passages went to the model, of which `examined` shows the top. */
  chunksSent?: number;
}

/**
 * Verdicts that assert something about a PARTICULAR passage, and therefore
 * cannot be shown without one.
 *
 * `insufficient_evidence` is the one verdict the engine's own validator lets
 * cite nothing ("only insufficient_evidence may cite nothing"), because it is
 * a statement about the ABSENCE of support rather than about a passage;
 * `no_evidence` is the engine reporting it never ran the model at all. Every
 * other verdict — including `contradicts`, which claims a source says the
 * opposite — points at text, and text it will not show is exactly the D18
 * failure this unit exists to stop.
 */
const MUST_CITE: ReadonlySet<Verdict> = new Set<Verdict>([
  'strong',
  'partial',
  'weak',
  'contradicts',
]);

export interface EvidenceCardProps {
  finding: GroundedFinding;
  /** Test seam, forwarded to the viewer. */
  loadBytes?: (documentId: number) => Promise<Uint8Array>;
}

/** The passage list — identical in both renderings, because a passage a reader
 *  can open and check is the same affordance whether it supports the claim or
 *  merely happens to be what was searched. */
const PassageList: React.FC<{
  rows: EvidenceRow[];
  onOpen: (row: EvidenceRow) => void;
  testidPrefix: string;
}> = ({ rows, onOpen, testidPrefix }) => (
  <ol className="gds-evidence__rows">
    {rows.map((row, i) => {
      const canOpen = row.page !== null && row.fileAvailable;
      return (
        <li key={`${row.chunkId}-${i}`} className="gds-evidence__row" data-testid={`${testidPrefix}-row-${i}`}>
          <button
            type="button"
            className="gds-btn gds-btn--ghost gds-evidence__ref"
            onClick={() => canOpen && onOpen(row)}
            disabled={!canOpen}
            data-testid={`${testidPrefix}-open-${i}`}
            title={
              row.page === null
                ? 'This source records no page numbers'
                : !row.fileAvailable
                  ? 'The source file has moved since it was indexed'
                  : 'Verify against the source'
            }
          >
            {row.sourceLabel} — {row.page === null ? 'page unknown' : `p.${row.page}`}
            {canOpen && ' · Verify against the source'}
          </button>
          {!row.fileAvailable && (
            <span className="gds-evidence__missing" data-testid={`${testidPrefix}-missing-${i}`}>
              file not found — it was moved or renamed since indexing
            </span>
          )}
          {/* The quote is shown WHATEVER the file's state: evidence beside
              prose is the rule, and a missing file must not remove it. */}
          <blockquote data-testid={`${testidPrefix}-quote-${i}`}>{row.quote}</blockquote>
        </li>
      );
    })}
  </ol>
);

export const EvidenceCard: React.FC<EvidenceCardProps> = ({ finding, loadBytes }) => {
  const [viewing, setViewing] = useState<EvidenceRow | null>(null);
  const status = VERDICT_STATUS[finding.verdict] ?? 'neutral';

  const cited = finding.evidence ?? [];
  const examined = finding.examined ?? [];

  // THE INVARIANT, enforced at runtime as well as in the type: a verdict that
  // asserts something about a passage renders its refusal, not its prose. A
  // `finding` assembled from JSON at runtime can still arrive empty, and a type
  // cannot catch that.
  if (cited.length === 0 && MUST_CITE.has(finding.verdict)) {
    return (
      <section className="gds-evidence gds-evidence--ungrounded" data-testid="evidence-card">
        <Badge status="neutral">Local AI</Badge>
        <p data-testid="evidence-ungrounded-notice">
          This assessment cannot be shown: it arrived without the evidence it was based on.
          Gaply does not display AI conclusions on their own.
        </p>
      </section>
    );
  }

  // NOT SUPPORTED IS AN ANSWER. "I read these passages and none of them support
  // your claim" is a true, useful and fully grounded result, and refusing to
  // show it taught the reader nothing while looking like a malfunction. The
  // grounding rule is kept exactly: the explanation still never appears alone —
  // it appears UNDER the account of what was searched, next to the passages
  // themselves, which stay openable and page-linked like any other evidence.
  if (cited.length === 0) {
    const n = finding.chunksSent ?? examined.length;
    return (
      <section className="gds-evidence gds-evidence--unsupported" data-testid="evidence-card">
        <header className="gds-evidence__head">
          <Badge status="neutral" data-testid="evidence-ai-marker">
            Local AI
          </Badge>
          <Badge status={status} data-testid="evidence-verdict">
            <span aria-hidden="true">{VERDICT_GLYPH[finding.verdict]} </span>
            {VERDICT_LABEL[finding.verdict]}
          </Badge>
          {finding.confidence !== null && (
            <span className="gds-evidence__confidence" data-testid="evidence-confidence">
              confidence {Math.round(finding.confidence * 100)}%
            </span>
          )}
        </header>

        <p className="gds-evidence__searched" data-testid="evidence-searched">
          {n > 0
            ? `Checked ${n} retrieved passage${n === 1 ? '' : 's'} from this source; none support the claim.`
            : 'No passages were retrieved from this source, so the claim could not be checked against it.'}
        </p>

        <p className="gds-evidence__explanation" data-testid="evidence-explanation">
          {finding.explanation}
        </p>

        {examined.length > 0 && (
          <>
            <p className="gds-evidence__searched-label" data-testid="evidence-examined-label">
              {examined.length === n
                ? 'What was examined:'
                : `What was examined (top ${examined.length} of ${n}):`}
            </p>
            <PassageList rows={examined} onOpen={setViewing} testidPrefix="evidence-examined" />
          </>
        )}

        {viewing && (
          <React.Suspense fallback={null}>
            <PdfViewer
              open
              documentId={viewing.documentId}
              initialPage={viewing.page}
              title={viewing.sourceLabel}
              onClose={() => setViewing(null)}
              loadBytes={loadBytes}
            />
          </React.Suspense>
        )}
      </section>
    );
  }

  return (
    <section className="gds-evidence" data-testid="evidence-card">
      <header className="gds-evidence__head">
        {/* The consistent marker: everything in this unit came from the local
            model, not from the deterministic pipeline. */}
        <Badge status="neutral" data-testid="evidence-ai-marker">
          Local AI
        </Badge>
        <Badge status={status} data-testid="evidence-verdict">
          <span aria-hidden="true">{VERDICT_GLYPH[finding.verdict]} </span>
          {VERDICT_LABEL[finding.verdict]}
        </Badge>
        {finding.confidence !== null && (
          <span className="gds-evidence__confidence" data-testid="evidence-confidence">
            confidence {Math.round(finding.confidence * 100)}%
          </span>
        )}
      </header>

      <p className="gds-evidence__explanation" data-testid="evidence-explanation">
        {finding.explanation}
      </p>

      {finding.advisories && finding.advisories.length > 0 && (
        <ul className="gds-evidence__advisories" data-testid="evidence-advisories">
          {finding.advisories.map((a, i) => (
            <li key={i}>{a}</li>
          ))}
        </ul>
      )}

      <PassageList rows={cited} onOpen={setViewing} testidPrefix="evidence" />

      {viewing && (
        <React.Suspense fallback={null}>
        <PdfViewer
          open
          documentId={viewing.documentId}
          initialPage={viewing.page}
          title={viewing.sourceLabel}
          onClose={() => setViewing(null)}
          loadBytes={loadBytes}
        />
        </React.Suspense>
      )}
    </section>
  );
};

export default EvidenceCard;
