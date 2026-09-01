// Gaply — the AI assistance section of the citation panel (Phase 9 item 2).
//
// Sits BELOW the deterministic citation UI, which is untouched and instant.
// That order is the point: the app's certain half must never wait on, or look
// broken because of, the uncertain half.
import './ai.css';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Card } from '../../design-system/primitives';
import {
  aiBridge,
  CitationAuditPreview,
  errorCode,
  errorText,
  JobProgressEvent,
  SupportEvent,
} from './aiBridge';
import { resultToFinding } from './findingFromResult';
import { projectDuration, SECONDS_PER_ITEM } from './ThesisAuditScreen';
import { pickManuscriptPath } from '../common/pickFile';
import { EvidenceCard, EvidenceRow, GroundedFinding, Verdict } from './EvidenceCard';
import { AiUnavailable } from './AiStatusPanel';

type Phase = 'idle' | 'running' | 'done' | 'failed' | 'rejected' | 'cancelled';

/** What the run is doing right now, from the command's own Channel.
 *
 *  A single unchanging "Checking on this machine" is indistinguishable from a
 *  wedged process, and on an unoptimised build a real check is minutes long, so
 *  this panel says the STAGE and the ELAPSED time instead of predicting a
 *  duration it cannot know. */
type Progress =
  | { stage: 'starting' }
  | { stage: 'retrieving' }
  | { stage: 'retrieved'; sent: number; dropped: number }
  | { stage: 'waiting'; ahead: number }
  | { stage: 'generating' }
  | { stage: 'decoding'; tokens: number; maxTokens: number; tokensPerSec: number | null }
  | { stage: 'validating' };

function describe(p: Progress): string {
  switch (p.stage) {
    case 'starting':
      return 'Starting…';
    case 'retrieving':
      return 'Finding passages in the cited document…';
    case 'retrieved':
      return `Judging ${p.sent} passage${p.sent === 1 ? '' : 's'}${p.dropped ? ` (${p.dropped} dropped for length)` : ''}…`;
    case 'waiting':
      // The engine runs ONE generation at a time; this is a queue, not slowness.
      return `Waiting for the model — ${p.ahead} other check${p.ahead === 1 ? '' : 's'} ahead of this one.`;
    case 'generating':
      return 'Reading the passages…';
    case 'decoding': {
      // `maxTokens` was `undefined` here for a while: the backend enum renamed
      // its VARIANTS to camelCase but not its struct-variant FIELDS, so the
      // panel read a key that was never sent and printed "up to undefined
      // tokens". Pinned now by the Rust event_wire tests; this guard stays,
      // because a missing field does not throw in JavaScript — it renders.
      const of = Number.isFinite(p.maxTokens) ? ` of up to ${p.maxTokens}` : '';
      if (!p.tokensPerSec) return `Writing the answer — ${p.tokens}${of} tokens.`;
      const rate = p.tokensPerSec.toFixed(1);
      // MEASURED, not assumed: the remaining estimate comes from THIS run's
      // observed rate. On a squeezed machine that is several times off any
      // constant anyone could have baked in.
      const left = Number.isFinite(p.maxTokens) ? p.maxTokens - p.tokens : 0;
      const tail =
        left > 0
          ? ` · ~${elapsedLabel(Math.ceil(left / p.tokensPerSec))} left at ${rate} tok/s`
          : ` · ${rate} tok/s`;
      return `Writing the answer — ${p.tokens}${of} tokens${tail}.`;
    }
    case 'validating':
      return 'Checking the answer against the passages it was given…';
  }
}

const GB = 1024 ** 3;

/**
 * The manuscript, for this app session only.
 *
 * There is no persistent "current manuscript" anywhere in Gaply — the thesis
 * audit picks a path each time, and the Citation Manager's `extractedCitations`
 * prop was never passed by any caller. Rather than invent a store, this keeps
 * the SAME path the audit screen would ask for, so the two screens agree within
 * a session and neither claims a durability it does not have.
 */
const MANUSCRIPT_KEY = 'gaply.ai.manuscript';

export function rememberedManuscript(): string | null {
  try {
    return sessionStorage.getItem(MANUSCRIPT_KEY);
  } catch {
    return null;
  }
}

export function rememberManuscript(path: string): void {
  try {
    sessionStorage.setItem(MANUSCRIPT_KEY, path);
  } catch {
    // A private window with storage disabled still gets the feature; it just
    // asks for the manuscript again next time.
  }
}

/** Last path segment, for naming the manuscript on screen. */
export function manuscriptName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/**
 * What the run cost, in the terms that explain a slow one.
 *
 * A wall-clock number alone cannot separate "this model is slow" from "this
 * machine had nothing left" — and on 8 GB with a 2.4 GB model those look
 * identical from the outside, which is how a decode at a fifth of the measured
 * rate turned into a hunt for a mis-selected model. Rate and headroom together
 * say which it was. Every figure is measured and reported by the backend;
 * nothing here is estimated.
 */
export function describeRun(raw: Record<string, any>): string | null {
  const parts: string[] = [];
  if (typeof raw.decodeTokensPerSec === 'number' && raw.decodeTokensPerSec > 0) {
    parts.push(`${raw.decodeTokensPerSec.toFixed(1)} tok/s decode`);
  }
  const free = raw.freeMemoryBytes;
  const total = raw.totalMemoryBytes;
  if (typeof free === 'number' && typeof total === 'number' && total > 0) {
    parts.push(`${(free / GB).toFixed(1)} GB free of ${(total / GB).toFixed(1)} GB at the end`);
  }
  return parts.length > 0 ? `${parts.join(', ')}.` : null;
}

/** mm:ss since a run started. */
function elapsedLabel(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export interface CitationAiPanelProps {
  /** The library entry this panel is about; the per-citation slice needs it. */
  citationId?: string;
  /** The claim to judge. May be empty — see the panel's claim field. */
  sentence: string;
  /** The cited source's indexed document, when there is one. */
  documentId?: number | null;
  citedSource?: string;
  aiInstalled: boolean;
  onOpenSettings?: () => void;
  bridge?: Pick<
    typeof aiBridge,
    | 'citationNeed'
    | 'citationSupport'
    | 'cancelGeneration'
    | 'documentSource'
    | 'citationAuditPreview'
    | 'citationAuditStart'
    | 'jobResults'
    | 'cancelJob'
  >;
  /** Test seam for the native file dialog. */
  pickManuscript?: () => Promise<string | null>;
  /** Open the manuscript-level audit, which owns the citation_need question. */
  onOpenAudit?: () => void;
}

/** Map a raw support result onto the D18 unit. */
export async function toFinding(
  raw: Record<string, any>,
  documentId: number,
  citedSource: string,
  documentExists: boolean,
): Promise<GroundedFinding> {
  const out = raw.output ?? {};
  const chunks: any[] = out.supporting_chunks ?? [];
  const evidence: EvidenceRow[] = chunks.map((c) => ({
    chunkId: String(c.chunk_id ?? ''),
    documentId,
    page: typeof c.page === 'number' ? c.page : null,
    sourceLabel: citedSource,
    quote: String(c.quote ?? c.why ?? ''),
    fileAvailable: documentExists,
  }));
  // What the check SEARCHED, as opposed to what the model cited. Carried so a
  // verdict that cites nothing can still be shown against something real.
  const examinedRaw: any[] = raw.examinedPassages ?? [];
  const examined: EvidenceRow[] = examinedRaw.map((c) => ({
    chunkId: String(c.chunkId ?? ''),
    documentId,
    page: typeof c.page === 'number' ? c.page : null,
    sourceLabel: citedSource,
    quote: String(c.text ?? ''),
    fileAvailable: documentExists,
  }));

  return {
    verdict: (out.verdict as Verdict) ?? 'no_evidence',
    confidence: typeof out.confidence === 'number' ? out.confidence : null,
    explanation: String(out.explanation ?? raw.reason ?? ''),
    advisories: raw.advisories ?? [],
    evidence,
    examined,
    chunksSent: typeof raw.chunksSent === 'number' ? raw.chunksSent : undefined,
  };
}

export const CitationAiPanel: React.FC<CitationAiPanelProps> = ({
  citationId,
  sentence,
  documentId,
  citedSource = 'Cited source',
  aiInstalled,
  onOpenSettings,
  bridge = aiBridge,
  onOpenAudit,
  // The formats plan_thesis_audit's parser accepts; the same list the audit
  // screen offers, because it is the same parse.
  pickManuscript = () => pickManuscriptPath(['pdf', 'docx', 'txt', 'md'], 'Manuscript'),
}) => {
  /* ---------------- the per-citation slice (§11 D54) ---------------- */
  const [manuscript, setManuscript] = useState<string | null>(rememberedManuscript);
  const [preview, setPreview] = useState<CitationAuditPreview | null>(null);
  const [slice, setSlice] = useState<'idle' | 'previewing' | 'confirm' | 'running' | 'done'>('idle');
  const [sliceItems, setSliceItems] = useState<any[]>([]);
  const [sliceProgress, setSliceProgress] = useState<JobProgressEvent | null>(null);
  const [sliceJobId, setSliceJobId] = useState<number | null>(null);
  const [sliceError, setSliceError] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>('idle');
  const [message, setMessage] = useState<string | null>(null);
  const [finding, setFinding] = useState<GroundedFinding | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  /** Which weights actually answered. Stamped by the backend on results AND
   *  failures, so "which model was this?" is never inferred again. */
  const [judgedBy, setJudgedBy] = useState<string | null>(null);
  /** How the run went, so a slow one explains itself rather than inviting a
   *  second investigation into whether the app picked the wrong model. */
  const [runStats, setRunStats] = useState<string | null>(null);
  // The Citation Manager has no manuscript sentence to offer, so the claim is
  // typed here. It used to be filled with the citation's own TITLE, which made
  // the task self-referential — "does this document support 'Chapter 1 —
  // Literature Review'?" — and a title has none of the elements the prompt
  // asks the model to decompose. See the note above the field.
  const [claim, setClaim] = useState(sentence);
  const [seconds, setSeconds] = useState(0);
  // Set the moment Cancel is pressed, so the run that then rejects is reported
  // as the user's decision rather than as a failure. The panel asked for it; it
  // does not need to parse an error string to recognise its own request.
  // Decode rate, measured from the heartbeat. The FIRST tick is the baseline,
  // not the start of the run: everything before it is prefill, and folding a
  // one-off prefill into a per-token rate makes the rate wrong in exactly the
  // direction that flatters a slow machine.
  const decodeStart = useRef<{ at: number; tokens: number } | null>(null);
  const cancelling = useRef(false);
  const [cancelRequested, setCancelRequested] = useState(false);

  // A running check is minutes of near-silence, so the panel counts. Ticking
  // only while running keeps this out of every other render.
  useEffect(() => {
    if (phase !== 'running') return;
    const id = setInterval(() => setSeconds((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [phase]);

  // The engine runs ONE generation at a time, so a check nobody can see the
  // result of still holds the model. If this panel goes away mid-run — the user
  // selected another citation, or left — stop the run it started. Per-run
  // cancellation makes that precise: it cannot touch anyone else's check.
  const runningRef = useRef(false);
  runningRef.current = phase === 'running';
  useEffect(
    () => () => {
      if (runningRef.current) void bridge.cancelGeneration();
    },
    [bridge],
  );

  const reset = () => {
    setMessage(null);
    setFinding(null);
    setProgress(null);
    setSeconds(0);
    setJudgedBy(null);
    setRunStats(null);
    decodeStart.current = null;
    cancelling.current = false;
    setCancelRequested(false);
  };

  const requestCancel = useCallback(() => {
    cancelling.current = true;
    setCancelRequested(true);
    void bridge.cancelGeneration();
  }, [bridge]);

  const onSupportEvent = useCallback((ev: SupportEvent) => {
    switch (ev.kind) {
      case 'retrieving':
        setProgress({ stage: 'retrieving' });
        break;
      case 'retrieved':
        setProgress({ stage: 'retrieved', sent: ev.chunksSent, dropped: ev.chunksDropped });
        break;
      case 'generating':
        setProgress(
          ev.queuedBehind > 0
            ? { stage: 'waiting', ahead: ev.queuedBehind }
            : { stage: 'generating' },
        );
        break;
      case 'decoding': {
        const now = Date.now();
        if (!decodeStart.current) decodeStart.current = { at: now, tokens: ev.tokens };
        const base = decodeStart.current;
        const secs = (now - base.at) / 1000;
        const produced = ev.tokens - base.tokens;
        setProgress({
          stage: 'decoding',
          tokens: ev.tokens,
          maxTokens: ev.maxTokens,
          // Needs a second tick and a real interval before it means anything.
          tokensPerSec: secs >= 1 && produced > 0 ? produced / secs : null,
        });
        break;
      }
      case 'validating':
        setProgress({ stage: 'validating' });
        break;
      default:
        break;
    }
  }, []);

  /** Ask which sentences cite this source. No model, no job — just a parse. */
  const runPreview = useCallback(
    async (path: string) => {
      if (!citationId) return;
      setSliceError(null);
      setSlice('previewing');
      try {
        const p = await bridge.citationAuditPreview(citationId, path);
        setPreview(p);
        setSlice('confirm');
      } catch (e) {
        setSliceError(errorText(e));
        setSlice('idle');
      }
    },
    [bridge, citationId],
  );

  const chooseManuscript = useCallback(async () => {
    const p = await pickManuscript();
    if (!p) return;
    rememberManuscript(p);
    setManuscript(p);
    void runPreview(p);
  }, [pickManuscript, runPreview]);

  const onJobProgress = useCallback(
    (ev: JobProgressEvent) => {
      setSliceProgress(ev);
      setSliceJobId(ev.jobId);
      // The event is a notification; the database is the record (same rule the
      // thesis audit follows).
      bridge
        .jobResults(ev.jobId, 0, 200)
        .then((r: any) => setSliceItems(r.items ?? []))
        .catch(() => {});
      if (ev.completed >= ev.total) setSlice('done');
    },
    [bridge],
  );

  /** THE point of no return for model time — after the user has seen the list. */
  const runSlice = useCallback(async () => {
    if (!citationId || !manuscript) return;
    setSliceError(null);
    setSliceItems([]);
    setSlice('running');
    try {
      await bridge.citationAuditStart(citationId, manuscript, onJobProgress);
    } catch (e) {
      setSliceError(errorText(e));
      setSlice('confirm');
    }
  }, [bridge, citationId, manuscript, onJobProgress]);

  const runSupport = useCallback(async () => {
    if (documentId == null || !claim.trim()) return;
    reset();
    setPhase('running');
    setProgress({ stage: 'starting' });
    try {
      const src = await bridge.documentSource(documentId);
      const raw = (await bridge.citationSupport(
        claim.trim(),
        documentId,
        citedSource,
        onSupportEvent,
      )) as Record<string, any>;
      if (typeof raw.loadedModelFile === 'string') setJudgedBy(raw.loadedModelFile);
      setRunStats(describeRun(raw));

      // The engine's three honest non-success outcomes, each said plainly
      // rather than collapsed into "something went wrong".
      if (raw.outcome === 'noEvidence') {
        setPhase('done');
        setFinding({
          verdict: 'no_evidence',
          confidence: null,
          explanation: String(raw.reason ?? 'No indexed evidence was retrieved for this claim.'),
          evidence: [],
        });
        return;
      }
      if (raw.outcome === 'validationFailed' || raw.outcome === 'error') {
        setPhase('rejected');
        // The reason is the useful half. "not valid JSON: EOF while parsing a
        // list" says the model ran out of room, which is a different problem
        // from a wrong answer and points at a different fix.
        const why = typeof raw.reason === 'string' && raw.reason.trim() ? ` (${raw.reason})` : '';
        setMessage(
          `AI output failed verification — the model’s answer did not meet Gaply’s grounding checks, so it was discarded rather than shown${why}.`,
        );
        return;
      }
      setFinding(await toFinding(raw, documentId, citedSource, src.exists));
      setPhase('done');
    } catch (e) {
      // A run the user stopped is not a failure, and must not be dressed as one.
      // Two independent signals: this panel knows it asked, and the engine
      // labels it `cancelled` — either is enough.
      if (cancelling.current || errorCode(e) === 'cancelled') {
        setPhase('cancelled');
        return;
      }
      setPhase('failed');
      setMessage(errorText(e));
    }
  }, [bridge, claim, documentId, citedSource, onSupportEvent]);

  const runNeed = useCallback(async () => {
    if (!claim.trim()) return;
    reset();
    setPhase('running');
    try {
      const raw = (await bridge.citationNeed(claim.trim())) as Record<string, any>;
      if (typeof raw.loadedModelFile === 'string') setJudgedBy(raw.loadedModelFile);
      setRunStats(describeRun(raw));
      if (raw.outcome === 'validationFailed') {
        setPhase('rejected');
        setMessage('AI output failed verification — retry, or check this sentence yourself.');
        return;
      }
      const out = raw.output ?? raw;
      // citation_need has NO evidence block (§11 D8) — it judges a sentence,
      // not a source. It therefore renders as plain text, NOT through the
      // evidence unit, which would otherwise show an empty-evidence refusal.
      setMessage(
        out.needs_citation
          ? `Needs a citation (${out.severity ?? 'unrated'}): ${out.reason ?? ''}`
          : `No citation required: ${out.reason ?? ''}`,
      );
      setPhase('done');
    } catch (e) {
      if (cancelling.current || errorCode(e) === 'cancelled') {
        setPhase('cancelled');
        return;
      }
      setPhase('failed');
      setMessage(errorText(e));
    }
  }, [bridge, claim]);

  if (!aiInstalled) {
    return <AiUnavailable feature="Citation checking" onOpenSettings={onOpenSettings} />;
  }

  return (
    <Card title="AI assistance" data-testid="citation-ai-panel">
      {/* ---------- PRIMARY: the sentences that cite this source ---------- */}
      {citationId && (
        <div data-testid="citation-slice">
          {!manuscript && (
            <>
              <p className="gds-ai__hint" data-testid="slice-no-manuscript">
                Import your manuscript to check the sentences that cite this source.
                Gaply finds them itself — you do not have to paste them in.
              </p>
              <Button variant="primary" onClick={chooseManuscript} data-testid="slice-import">
                Import your manuscript
              </Button>
            </>
          )}

          {manuscript && slice === 'idle' && (
            <>
              <p className="gds-ai__hint" data-testid="slice-manuscript">
                Manuscript: <code>{manuscriptName(manuscript)}</code>
              </p>
              <div className="gds-audit__actions">
                <Button
                  variant="primary"
                  onClick={() => void runPreview(manuscript)}
                  data-testid="slice-check"
                >
                  Check citation support
                </Button>
                <Button variant="ghost" onClick={chooseManuscript} data-testid="slice-rechoose">
                  Use a different manuscript
                </Button>
              </div>
            </>
          )}

          {slice === 'previewing' && (
            <p className="gds-ai__hint" data-testid="slice-previewing">
              Reading your manuscript for sentences that cite this source…
            </p>
          )}

          {sliceError && (
            <p className="gds-ai__hint" data-testid="slice-error" style={{ color: 'var(--g-flagged)' }}>
              {sliceError}
            </p>
          )}

          {preview && slice !== 'previewing' && (
            <>
              <p className="gds-ai__value" data-testid="slice-count">
                {preview.sentences.length === 0
                  ? `No sentences in your manuscript cite this source (of ${preview.totalSentences} checked).`
                  : `${preview.sentences.length} sentence${preview.sentences.length === 1 ? '' : 's'} in your manuscript cite this source.`}
              </p>

              {preview.sentences.length > 0 && (
                <ol className="gds-evidence__rows" data-testid="slice-sentences">
                  {preview.sentences.map((sn, i) => {
                    const result = sliceItems.find((it: any) => it.seq === sn.seq)?.result;
                    return (
                      <li className="gds-evidence__row" key={sn.seq} data-testid={`slice-sentence-${i}`}>
                        <p className="gds-evidence__searched-label">
                          {sn.page === null ? 'page unknown' : `p.${sn.page}`} · {sn.marker}
                        </p>
                        <blockquote data-testid={`slice-quote-${i}`}>{sn.sentence}</blockquote>
                        {/* Each sentence's verdict renders through the SAME D18
                            unit as every other finding — evidence rows, page
                            links, refusal rules and all. */}
                        {result && preview.documentId !== null && (
                          <EvidenceCard
                            finding={resultToFinding(result, {
                              documentId: preview.documentId,
                              sourceLabel: citedSource,
                            })}
                          />
                        )}
                      </li>
                    );
                  })}
                </ol>
              )}

              {/* The source itself is not checkable: say so and point at the
                  fix, rather than offering a check that cannot produce
                  evidence (§11 D40 — unverifiable is a RESULT). */}
              {preview.sentences.length > 0 && preview.documentId === null && (
                <p className="gds-ai__hint" data-testid="slice-unverifiable">
                  These sentences cite this source, but it cannot be checked:{' '}
                  {preview.unverifiableReason ?? 'its document is not indexed.'} Use{' '}
                  <b>Link document</b> above to point Gaply at the source file — it is indexed
                  and embedded there, and this check becomes available.
                </p>
              )}

              {slice === 'confirm' && preview.sentences.length > 0 && preview.documentId !== null && (
                <div className="gds-audit__actions">
                  <Button variant="primary" onClick={runSlice} data-testid="slice-confirm">
                    Check all {preview.sentences.length} — about{' '}
                    {projectDuration(preview.sentences.length)}
                  </Button>
                </div>
              )}

              {(slice === 'running' || slice === 'done') && sliceProgress && (
                <p className="gds-ai__hint" data-testid="slice-progress">
                  {sliceProgress.completed} of {sliceProgress.total} checked
                  {slice === 'running' ? ' — this keeps running if you look elsewhere.' : '.'}
                </p>
              )}

              {slice === 'running' && sliceJobId !== null && bridge.cancelJob && (
                <Button
                  variant="ghost"
                  onClick={() => void bridge.cancelJob!(sliceJobId)}
                  data-testid="slice-cancel"
                >
                  Stop checking
                </Button>
              )}
            </>
          )}
        </div>
      )}

      {/* "Does this sentence need a citation?" is a question about the
          MANUSCRIPT, not about this source — it judges uncited sentences, which
          by definition cite nothing at all. It belongs to the audit, which
          already queues exactly those items through the same planner. The panel
          keeps a link, not a copy. */}
      {citationId && onOpenAudit && (
        <p className="gds-ai__hint" data-testid="slice-need-link">
          Looking for sentences that may need a citation?{' '}
          <button type="button" className="gds-link" onClick={onOpenAudit} data-testid="slice-need-open">
            Check the whole manuscript
          </button>
          .
        </p>
      )}

      {/* ---------- FALLBACK: one sentence, typed by hand ---------- */}
      {citationId && (
        <p className="gds-ai__searched-label" data-testid="slice-manual-label">
          or check a single sentence
        </p>
      )}

      {/* THE CLAIM IS AN INPUT, not the citation's title.
          It used to be prefilled with `selected.csl.title`, which made the
          request self-referential — "does this document support the string
          'Chapter 1 — Literature Review'?" — with the same text as the claim
          AND the cited source. The prompt asks the model to decompose a claim
          into subject, direction, magnitude, population and condition; a title
          has none of them, and the model answered with prose. The eval harness
          runs this identical code path against real sentences ("Organic
          management increased soil invertebrate species richness by about 31
          percent") and passes; the difference was never the model. */}
      <label className="gds-ai__label" htmlFor="ai-claim">
        Claim to check against this source
      </label>
      <textarea
        id="ai-claim"
        className="gds-ai__claim"
        rows={2}
        value={claim}
        placeholder="Paste the sentence from your writing that cites this source…"
        onChange={(e) => setClaim(e.target.value)}
        data-testid="ai-claim"
      />
      <p className="gds-ai__hint">
        A sentence that asserts something checkable. A reference title is not a
        claim — there is nothing in it to verify.
      </p>

      <div className="gds-audit__actions">
        {/* A support check reads the CITED DOCUMENT. With nothing indexed behind
            this citation there is no evidence to read, so the check cannot
            produce a result — and a button that is merely greyed out, with the
            reason hidden in a tooltip, reads as a bug. Say it instead. */}
        {documentId == null ? (
          <p className="gds-ai__hint" data-testid="ai-support-unavailable">
            Citation support needs the cited source indexed in Gaply. This
            citation isn’t linked to an indexed document, so there are no
            passages to check it against.
          </p>
        ) : (
          <Button
            variant="secondary"
            onClick={runSupport}
            disabled={phase === 'running' || !claim.trim()}
            data-testid="ai-check-support"
          >
            Check citation support
          </Button>
        )}
        <Button
          variant="secondary"
          onClick={runNeed}
          disabled={phase === 'running' || !claim.trim()}
          data-testid="ai-check-need"
        >
          Check if citation is needed
        </Button>
        {phase === 'running' && (
          <Button
            variant="ghost"
            onClick={requestCancel}
            disabled={cancelRequested}
            data-testid="ai-cancel"
          >
            {cancelRequested ? 'Stopping…' : 'Cancel'}
          </Button>
        )}
      </div>

      {phase === 'running' && (
        <p className="gds-ai__hint" data-testid="ai-running">
          {/* No predicted duration. This runs on your CPU and the honest number
              varies by machine and build by more than an order of magnitude;
              quoting "about a minute" and then going silent for ten is what
              makes a working check look wedged. */}
          Checking on this machine — {elapsedLabel(seconds)} elapsed.
          {' '}
          <span data-testid="ai-progress">
            {/* Cancellation is checked between model steps, and the FIRST step
                reads the whole prompt in one pass. On CPU that step alone can
                run for minutes, so a Cancel press cannot land instantly and
                saying "cancelling…" without saying why would look like a second
                thing that does not work. (The engine fix is a chunked prefill;
                until then this is the truthful account.) */}
            {cancelRequested
              ? 'Stopping — the model step already in progress has to finish first, which on CPU can take a few minutes.'
              : progress
                ? describe(progress)
                : ''}
          </span>
        </p>
      )}

      {phase === 'cancelled' && (
        <p className="gds-ai__hint" data-testid="ai-cancelled">
          Stopped. Nothing was saved — run the check again when you want it.
        </p>
      )}

      {phase === 'rejected' && (
        <p className="gds-ai__hint" data-testid="ai-validation-failed" style={{ color: 'var(--g-flagged)' }}>
          {message}
        </p>
      )}

      {phase === 'failed' && (
        <p className="gds-ai__hint" data-testid="ai-error" style={{ color: 'var(--g-flagged)' }}>
          {message}
        </p>
      )}

      {phase === 'done' && message && (
        <p className="gds-ai__hint" data-testid="ai-need-result">
          {message}
        </p>
      )}

      {finding && <EvidenceCard finding={finding} />}

      {judgedBy && phase !== 'running' && (
        <p className="gds-ai__hint" data-testid="ai-judged-by">
          Judged on this machine by <code>{judgedBy}</code>
          {runStats ? ` — ${runStats}` : '.'}
        </p>
      )}
    </Card>
  );
};

export default CitationAiPanel;
