// Gaply — the Thesis Audit screen (Phase 9 item 4).
//
// The shape of this screen is set by one measured fact: an audit is 50–250
// sequential model calls at ~65 s each on CPU (§11 D34). That is HOURS.
//
//   * the deterministic pre-pass is shown BEFORE any model runs, so the user
//     sees what they are about to spend the evening on;
//   * starting requires an explicit confirmation — an 8-hour job must never
//     begin because a screen mounted;
//   * progress streams, results fill in as they land, and a job survives a
//     restart because the runner's state is in the database, not in this
//     component.
import './ai.css';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Badge, Button, Card } from '../../design-system/primitives';
import { aiBridge, AuditPlan, JobProgressEvent, ThesisAuditPreview, errorText, subjectKey } from './aiBridge';
import { describeOaOutcome } from './oaOutcome';
import { EvidenceCard, GroundedFinding, Verdict } from './EvidenceCard';
import { AiUnavailable } from './AiStatusPanel';
import { saveBinaryFile } from '../../utils/saveBinaryFile';
import { AnnotatedManuscript, AnnotatedUnavailable } from './AnnotatedManuscript';

/**
 * Per-item seconds for the projection BEFORE a run has measured anything,
 * PER DEVICE (§11 D62).
 *
 * There used to be one figure, 65, taken from the §11 D34 CPU baseline. After
 * Phase 7 STEP 2 it was wrong in both directions at once — it under-promised CPU
 * by ~2.8x and over-promised Metal by ~2.3x, so a 65-sentence audit was quoted
 * at "70 minutes" whether the truth was ~3.3 hours or ~34 minutes.
 *
 * MEASURED on the same six seeds, 3B, citation_support-v1.5, isolated:
 * CPU ~185 s/item (mean latency 198,751 ms over 6) and Metal ~31 s/item
 * (42,530 ms). Both are seeds only: `observedSecondsPerItem` replaces them with
 * the run's own rate after two items, and the screen says which is in use.
 *
 * These are numbers about a MACHINE and they expire. When the engine gets
 * faster, this is one of the places that has to be re-measured — not adjusted
 * by feel.
 */
export const SECONDS_PER_ITEM_BY_DEVICE = { cpu: 185, metal: 31 } as const;

/** The old single-figure export, kept as the CPU floor for callers without a
 *  device to hand. CPU is the floor everywhere (§11 D36), so defaulting to the
 *  SLOW number is the honest direction to be wrong in: a projection that
 *  under-promises turns a three-hour job into an unpleasant surprise. */
export const SECONDS_PER_ITEM: number = SECONDS_PER_ITEM_BY_DEVICE.cpu;

/** Seed seconds/item for a device, defaulting to the slower CPU figure when the
 *  device is not yet known (status still loading, or an older backend). */
export function seedSecondsPerItem(device?: string | null): number {
    return device === 'metal'
        ? SECONDS_PER_ITEM_BY_DEVICE.metal
        : SECONDS_PER_ITEM_BY_DEVICE.cpu;
}

/**
 * What the wait actually MEANS, in words, at the moment the user consents.
 *
 * A duration alone reads as a progress bar that has not started. "About 3.3
 * hours" and "about 3.3 hours, and this machine will be busy for that time"
 * are different pieces of information, and the second is the one someone needs
 * before starting a job they cannot pause into the evening.
 */
export function waitAdvice(items: number, device?: string | null): string {
    const seconds = items * seedSecondsPerItem(device);
    if (device === 'metal') {
        return seconds > 900
            ? 'Your GPU does the work, so you can keep using the machine, but leave the app open.'
            : 'Your GPU does the work; this should not get in your way.';
    }
    if (seconds > 5400) {
        return 'This runs on the CPU and will keep the machine busy for that whole time. '
            + 'Start it when you do not need the laptop — it can be cancelled, and finished items are kept.';
    }
    return seconds > 900
        ? 'This runs on the CPU and the machine will be noticeably busy. It can be cancelled, and finished items are kept.'
        : 'This runs on the CPU.';
}

/**
 * Seconds per item as THIS run is actually going, or null before there is
 * enough to say. One completed item is a sample, not a rate — the first is
 * also the one that pays for loading the weights — so this waits for two.
 */
export function observedSecondsPerItem(
  completed: number,
  elapsedMs: number,
): number | null {
  if (completed < 2 || elapsedMs <= 0) return null;
  return elapsedMs / 1000 / completed;
}

export function projectDuration(items: number, perItem = SECONDS_PER_ITEM): string {
  const total = items * perItem;
  if (total < 90) return `${Math.round(total)} seconds`;
  if (total < 5400) return `${Math.round(total / 60)} minutes`;
  const h = total / 3600;
  return `${h < 10 ? h.toFixed(1) : Math.round(h)} hours`;
}

export interface AuditItem {
  seq: number;
  kind: string;
  page: number | null;
  sentence: string;
  status: string;
  resultJson?: string | null;
}

type Stage = 'idle' | 'planned' | 'running' | 'paused' | 'done';

export interface ThesisAuditScreenProps {
  /** Open a citation in the Citation Manager, where its Document card offers
   *  "Link document". Attaching a PDF by hand belongs there — this screen would
   *  otherwise grow a second, competing file-picker for the same job. */
  onOpenCitation?: (citationId: string) => void;
  aiInstalled: boolean;
  onOpenSettings?: () => void;
  /** A job left running/paused by a previous launch, offered for resume. */
  resumableJob?: { jobId: number; completed: number; total: number } | null;
  pickManuscript?: () => Promise<string | null>;
  bridge?: Pick<
    typeof aiBridge,
    | 'startThesisAudit'
    | 'previewThesisAudit'
    | 'fetchOpenAccess'
    | 'jobStatus'
    | 'pauseJob'
    | 'resumeJob'
    | 'cancelJob'
    | 'jobResults'
    | 'fetchOpenAccess'
    | 'modelStatus'
    | 'recheckItems'
    | 'exportAuditReport'
  >;
}

export const ThesisAuditScreen: React.FC<ThesisAuditScreenProps> = ({
  aiInstalled,
  onOpenSettings,
  resumableJob = null,
  pickManuscript,
  bridge = aiBridge,
  onOpenCitation,
}) => {
  const [stage, setStage] = useState<Stage>('idle');
  const [plan, setPlan] = useState<AuditPlan | null>(null);
  /** What the audit WOULD do. Drives the confirmation card (§11 D88). */
  const [preview, setPreview] = useState<ThesisAuditPreview | null>(null);
  const [fixing, setFixing] = useState(false);
  const [fixNote, setFixNote] = useState<string | null>(null);
  /** §11 D94. Structural findings must be acknowledged before starting. */
  const [ackStructural, setAckStructural] = useState(false);
  const [path, setPath] = useState<string | null>(null);
  const [progress, setProgress] = useState<JobProgressEvent | null>(null);
  /** When this run's items started landing, for the measured rate. */
  const runStartedAt = useRef<number | null>(null);
  const [items, setItems] = useState<AuditItem[]>([]);
  const [health, setHealth] = useState<Record<string, any> | null>(null);
  /** The full sentence dump, CLOSED by default. The audit reads a hundred
   *  sentences and flags a handful; leading with all hundred buries the report
   *  under the material it was computed from. */
  const [dumpOpen, setDumpOpen] = useState(false);
  /** §11 D92. The annotated manuscript, closed by default — it renders pages. */
  const [annotOpen, setAnnotOpen] = useState(false);
  /** Per-source fetch results from the report's own actions, by citation id. */
  const [fetchNotes, setFetchNotes] = useState<Record<string, string>>({});
  /** Citations whose source became checkable, so a re-check is worth offering. */
  const [nowCheckable, setNowCheckable] = useState<string[]>([]);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [exportNote, setExportNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [drill, setDrill] = useState<GroundedFinding | null>(null);
  /** Which device the engine actually got, for the projection seed (§11 D62).
   *  Null until the status call lands; `seedSecondsPerItem` then falls back to
   *  the slower CPU figure, which is the honest direction to be wrong in. */
  const [activeDevice, setActiveDevice] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    // Optional-called: the projection is a convenience, and a bridge that does
    // not carry `modelStatus` (a narrower test double, an older build) must
    // still render an audit screen rather than throwing out of an effect.
    Promise.resolve(bridge.modelStatus?.())
      .then((s) => {
        if (alive && s) setActiveDevice(s.activeDevice);
      })
      // A projection is a convenience; failing to read the device must never
      // stop the screen. The CPU fallback still gives an honest estimate.
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [bridge]);

  const jobId = plan?.jobId ?? resumableJob?.jobId ?? null;
  /** The manuscript's file name, for the report's cover. */
  const manuscriptLabel = (path ?? 'manuscript').split(/[\\/]/).filter(Boolean).pop() ?? 'manuscript';

  const onProgress = useCallback(
    (ev: JobProgressEvent) => {
      if (runStartedAt.current === null) runStartedAt.current = Date.now();
      setProgress(ev);
      // Pull the newly-retired items rather than trusting the event to carry
      // them: the event is a notification, the database is the record.
      bridge
        .jobResults(ev.jobId, 0, 200)
        .then((r: any) => setItems(r.items ?? []))
        .catch(() => {});
      if (ev.completed >= ev.total) {
        setStage('done');
        bridge.jobStatus(ev.jobId).then((s: any) => setHealth(s.health)).catch(() => {});
      }
    },
    [bridge],
  );

  const choose = useCallback(async () => {
    setError(null);
    const p = pickManuscript ? await pickManuscript() : null;
    if (!p) return;
    setPath(p);
    // PREVIEW ONLY — no job, no model (§11 D88). This used to call
    // startThesisAudit, which plans the job AND spawns the runner, so the
    // "Before you start" card below rendered while the model was already
    // generating and "Not now" abandoned a run in progress.
    try {
      setPreview(await bridge.previewThesisAudit(p));
      setAckStructural(false);
      setStage('planned');
    } catch (e) {
      setError(errorText(e));
    }
  }, [bridge, pickManuscript]);

  const start = useCallback(async () => {
    if (!path) return;
    setStage('running');
    try {
      // NOW the job is created and the runner spawned — after the card, which
      // is what "Start the audit" has always claimed to do.
      const planned = await bridge.startThesisAudit(path, onProgress);
      setPlan(planned);
    } catch (e) {
      setError(errorText(e));
      setStage('planned');
    }
  }, [bridge, path, onProgress]);

  const resume = useCallback(async () => {
    if (!resumableJob) return;
    setStage('running');
    try {
      await bridge.resumeJob(resumableJob.jobId, onProgress);
    } catch (e) {
      setError(errorText(e));
    }
  }, [bridge, resumableJob, onProgress]);

  useEffect(() => {
    if (stage === 'done' && jobId != null && !health) {
      bridge.jobStatus(jobId).then((s: any) => setHealth(s.health)).catch(() => {});
    }
  }, [stage, jobId, health, bridge]);

  const grouped = useMemo(() => {
    const g: Record<string, AuditItem[]> = {};
    for (const it of items) {
      const key = it.kind ?? 'unknown';
      (g[key] ??= []).push(it);
    }
    return g;
  }, [items]);

  /** The unverifiable items, with the citation each one is blocked on. */
  const blocked = React.useMemo(() => {
    const out: Array<{ seq: number; sentence: string; reason: string; citationId?: string }> = [];
    for (const it of items) {
      if (it.kind !== 'unverifiable') continue;
      out.push({
        seq: it.seq,
        sentence: it.sentence,
        reason: String((it as any).reason ?? (it as any).payload?.reason ?? ''),
        citationId: (it as any).libraryId ?? (it as any).payload?.libraryId,
      });
    }
    return out;
  }, [items]);

  /** Fetch open-access copies for the sources this report is blocked on. */
  const fetchForSources = useCallback(
    async (citationIds: string[], label: string) => {
      if (citationIds.length === 0) return;
      setBusyAction(label);
      try {
        const reports = await bridge.fetchOpenAccess(citationIds);
        const notes: Record<string, string> = {};
        const gained: string[] = [];
        for (const r of reports) {
          notes[subjectKey(r.subject)] = describeOaOutcome(r);
          // Only LIBRARY citations here: `recheckItems` re-runs items keyed by
          // citation id, and a staged manuscript reference has none (§11 D132).
          // Staged sources that became checkable still need their own re-check
          // path — dropped visibly here rather than silently keyed to undefined,
          // which is what this line did before the subject was a union.
          if (r.checkable && r.subject.kind === 'citation') gained.push(r.subject.citationId);
        }
        setFetchNotes((prev) => ({ ...prev, ...notes }));
        setNowCheckable((prev) => Array.from(new Set([...prev, ...gained])));
      } catch (e) {
        setExportNote(errorText(e));
      } finally {
        setBusyAction(null);
      }
    },
    [bridge],
  );

  /** Re-run ONLY the items that became checkable. */
  const recheck = useCallback(async () => {
    if (jobId == null || nowCheckable.length === 0) return;
    setBusyAction('recheck');
    try {
      const r = await bridge.recheckItems(jobId, nowCheckable, onProgress);
      setExportNote(
        r.requeued === 0
          ? 'Nothing became checkable, so nothing was re-run.'
          : `Re-checking ${r.requeued} sentence${r.requeued === 1 ? '' : 's'}.`,
      );
      if (r.requeued > 0) {
        setStage('running');
        setNowCheckable([]);
      }
    } catch (e) {
      setExportNote(errorText(e));
    } finally {
      setBusyAction(null);
    }
  }, [bridge, jobId, nowCheckable, onProgress]);

  const exportReport = useCallback(
    async (format: 'pdf' | 'html') => {
      if (jobId == null) return;
      setBusyAction(`export-${format}`);
      setExportNote(null);
      try {
        const r = await bridge.exportAuditReport(jobId, manuscriptLabel, format);
        // §11 D91. One binary save path, shared with the PublishReady export —
        // this one already used the native dialog, but with no filter, so the
        // panel was left to decide what the extension meant.
        const path = await saveBinaryFile(
          r.suggestedName,
          new Uint8Array(r.bytes),
          format === 'pdf' ? 'application/pdf' : 'text/html',
        );
        if (!path) return;
        setExportNote(`Saved to ${path}`);
      } catch (e) {
        setExportNote(errorText(e));
      } finally {
        setBusyAction(null);
      }
    },
    [bridge, jobId, manuscriptLabel],
  );

  if (!aiInstalled) {
    return <AiUnavailable feature="The thesis citation audit" onOpenSettings={onOpenSettings} />;
  }

  const queued =
    plan != null
      ? plan.queuedCitationNeed + plan.queuedCitationSupport + plan.queuedUnverifiable
      : 0;
  // Unverifiable items cost no model time (§11 D40), so they are excluded from
  // the projection. Including them would overstate the wait by hours.
  const modelItems = plan != null ? plan.queuedCitationNeed + plan.queuedCitationSupport : 0;

  return (
    <div className="gds-root" data-testid="thesis-audit">
      {resumableJob && stage === 'idle' && (
        <Card title="Unfinished audit" data-testid="audit-resume-offer">
          <p className="gds-ai__hint">
            An audit was interrupted at {resumableJob.completed} of {resumableJob.total} items.
            Its finished results were kept.
          </p>
          <Button variant="primary" onClick={resume} data-testid="audit-resume">
            Resume audit
          </Button>
        </Card>
      )}

      {stage === 'idle' && (
        <Card title="Thesis citation audit">
          <p className="gds-ai__hint">
            Checks every claim in a manuscript against the sources you cited. Runs entirely on
            this machine.
          </p>
          <Button variant="primary" onClick={choose} data-testid="audit-pick">
            Choose a manuscript…
          </Button>
        </Card>
      )}

      {error && (
        <p className="gds-ai__hint" data-testid="audit-error" style={{ color: 'var(--g-flagged)' }}>
          {error}
        </p>
      )}

      {preview && stage === 'planned' && (
        <Card title="Before you start" data-testid="audit-plan">
          <p className="gds-ai__hint">{path}</p>

          {/* §11 D110. FIRST on the card, above the counts.
              A retracted source is registry-backed, deterministic and more
              serious than anything this audit can conclude — and it costs
              nothing to establish. Learning it after three hours of model time
              inverts the cost of finding out.
              NOT a gate: citing a retracted work is legitimate when the
              retraction is the point. It reports; it does not block. */}
          {preview.retractedSources > 0 && (
            <div className="gds-audit__alert" data-testid="audit-retracted">
              <p className="gds-ai__value" style={{ color: 'var(--g-flagged)' }}>
                {preview.retractedSources} of your cited source
                {preview.retractedSources === 1 ? ' has' : 's have'} been RETRACTED
              </p>
              <ul className="gds-audit__counts">
                {preview.sources
                  .filter((src) => src.retracted)
                  .map((src, i) => (
                    <li key={i} data-testid={`audit-retracted-${i}`}>
                      <b>{src.label}</b> — cited by {src.citingSentences} sentence
                      {src.citingSentences === 1 ? '' : 's'}
                    </li>
                  ))}
              </ul>
              <p className="gds-ai__hint">
                A retraction registry was asked and answered — no language model was involved,
                and this does not depend on the audit running. Citing a retracted work is fine
                when the retraction is your point; otherwise the citation needs replacing. Only
                works that were actually checked appear here; an unchecked entry is not a clean
                one, and the Citation Manager says which is which.
              </p>
            </div>
          )}
          <div className="gds-audit__stats">
            <div className="gds-audit__stat"><b>{preview.totalSentences}</b><span>sentences</span></div>
            <div className="gds-audit__stat"><b>{preview.cited}</b><span>cited</span></div>
            <div className="gds-audit__stat"><b>{preview.uncited}</b><span>uncited</span></div>
            <div className="gds-audit__stat"><b>{preview.skipped}</b><span>filtered out</span></div>
          </div>

          {/* THE HEADLINE, PER SOURCE (§11 D88). `wouldBeUnverifiable` counts
              SENTENCES; a researcher does not have 40 unverifiable sentences,
              they have 4 sources without a PDF cited 40 times. One is
              unactionable, the other is a short list of fixes. */}
          <p className="gds-ai__value" data-testid="audit-source-coverage">
            {preview.sources.length === 0
              ? 'No cited sources were found in this manuscript.'
              : `${preview.checkableSources} of ${preview.sources.length} cited source${
                  preview.sources.length === 1 ? '' : 's'
                } ${preview.checkableSources === 1 ? 'has' : 'have'} a PDF Gaply can read.`}
          </p>
          {preview.blockedSources > 0 && (
            <p className="gds-ai__hint" data-testid="audit-source-gap">
              The other {preview.blockedSources} can be flagged, but{' '}
              <b>not verified against their source</b> — {preview.wouldBeUnverifiable} sentence
              {preview.wouldBeUnverifiable === 1 ? '' : 's'} cite them. Fetching or attaching those
              PDFs now is the difference between a checked claim and a note saying it could not be
              checked.
            </p>
          )}

          {/* The fix, offered HERE — not discovered three hours later in a
              section titled "Cited, but not checkable". */}
          {preview.blockedSources > 0 && (
            <div data-testid="audit-blocked-sources">
              <ul className="gds-audit__counts">
                {preview.sources
                  .filter((src) => src.documentId === null)
                  .slice(0, 8)
                  .map((src, i) => (
                    <li key={i} data-testid={`audit-blocked-${i}`}>
                      <b>{src.label}</b> — {src.reason ?? 'no source available'} (
                      {src.citingSentences} sentence{src.citingSentences === 1 ? '' : 's'})
                    </li>
                  ))}
              </ul>
              {/* §11 D101. `oa_fetch` looks up a DOI and refuses without one.
                  An IEEE-style reference list carries none — R PAPER has 25
                  entries and zero — so offering the fetch there is an
                  affordance guaranteed to return nothing for every source. A
                  feature that cannot apply says so instead.
                  ATTACHING stays available: it is the thing that DOES work. */}
              {!preview.sources.some((src) => src.documentId === null && src.hasDoi) && (
                <p className="gds-ai__hint" data-testid="audit-fetch-unavailable">
                  None of the blocked sources records a DOI, and the open-access fetch looks up a
                  DOI — so it cannot resolve any of them.
                  {preview.referenceEntries > 0 && preview.referenceEntriesWithDoi === 0 && (
                    <> This manuscript’s reference list has {preview.referenceEntries} entries and
                    none records a DOI, which is normal for IEEE-style lists.</>
                  )}{' '}
                  Add the DOI in the Citation Manager, or attach the PDF there.
                </p>
              )}
              <div className="gds-audit__actions">
                {preview.sources.some((src) => src.documentId === null && src.hasDoi) && (
                <Button
                  variant="secondary"
                  disabled={fixing}
                  data-testid="audit-fetch-sources"
                  onClick={async () => {
                    const ids = preview.sources
                      .filter((src) => src.documentId === null && src.libraryId && src.hasDoi)
                      .map((src) => src.libraryId as string);
                    if (ids.length === 0) {
                      setFixNote(
                        'None of these are in your library yet, so there is no DOI to look up. Add them in the Citation Manager first, then re-check.',
                      );
                      return;
                    }
                    setFixing(true);
                    setFixNote(null);
                    try {
                      await bridge.fetchOpenAccess(ids);
                      if (path) setPreview(await bridge.previewThesisAudit(path));
                    } catch (e) {
                      setFixNote(errorText(e));
                    } finally {
                      setFixing(false);
                    }
                  }}
                >
                  {fixing ? 'Looking…' : 'Fetch open-access copies'}
                </Button>
                )}
                {/* Attaching by hand belongs in the Manager's Document card —
                    this screen must not grow a second file-picker for the same
                    job (the prop's own contract). */}
                <Button
                  variant="ghost"
                  data-testid="audit-attach-sources"
                  onClick={() => {
                    const first = preview.sources.find(
                      (src) => src.documentId === null && src.libraryId,
                    );
                    if (first?.libraryId) onOpenCitation?.(first.libraryId);
                    else
                      setFixNote(
                        'These works are not in your library yet, so there is nothing to attach a PDF to. Add them in the Citation Manager first.',
                      );
                  }}
                >
                  Attach a PDF in the Citation Manager
                </Button>
              </div>
              {fixNote && (
                <p className="gds-ai__hint" data-testid="audit-fix-note">{fixNote}</p>
              )}
            </div>
          )}

          {/* §11 D94. Deterministic checks, shown where the decision is made.
              A structural finding means the audit's resolutions may be wrong,
              so it is acknowledged rather than merely displayed. */}
          {preview.consistency && preview.consistency.findings.length > 0 && (
            <div data-testid="audit-consistency">
              {preview.consistency.structural > 0 && (
                <div data-testid="audit-consistency-structural">
                  <p className="gds-ai__value" style={{ color: 'var(--g-flagged)' }}>
                    The manuscript's structure has {preview.consistency.structural} problem
                    {preview.consistency.structural === 1 ? '' : 's'} that affect what the audit can
                    resolve.
                  </p>
                  <ul className="gds-audit__counts">
                    {preview.consistency.findings
                      .filter((f) => f.severity === 'structural')
                      .map((f, i) => (
                        <li key={i} data-testid={`audit-structural-${i}`}>
                          {f.message}
                          {f.action && <><br /><b>What to do:</b> {f.action}</>}
                        </li>
                      ))}
                  </ul>
                  <label className="gds-ai__hint" data-testid="audit-ack-label">
                    <input
                      type="checkbox"
                      checked={ackStructural}
                      onChange={(e) => setAckStructural(e.target.checked)}
                      data-testid="audit-ack"
                    />{' '}
                    I understand the audit may resolve citations to the wrong sources, and want to
                    run it anyway.
                  </label>
                </div>
              )}
              {preview.consistency.cosmetic > 0 && (
                <details data-testid="audit-consistency-cosmetic">
                  <summary>
                    {preview.consistency.cosmetic} other consistency issue
                    {preview.consistency.cosmetic === 1 ? '' : 's'} — worth fixing, but they change
                    no verdict
                  </summary>
                  <ul className="gds-audit__counts">
                    {preview.consistency.findings
                      .filter((f) => f.severity === 'cosmetic')
                      .map((f, i) => (
                        <li key={i} data-testid={`audit-cosmetic-${i}`}>{f.message}</li>
                      ))}
                  </ul>
                </details>
              )}
            </div>
          )}

          {/* §11 D128. `wouldSuggest` used to be added to BOTH the item count
              and the model-work estimate, so the card promised minutes of work
              on a lane that is now retired — and, worse, implied those sentences
              would be examined. They are not, and the card says so plainly
              rather than leaving a reader to assume coverage. */}
          <p className="gds-ai__hint" data-testid="audit-projection">
            {preview.wouldCheck + preview.wouldBeUnverifiable} items;{' '}
            {preview.wouldCheck} need the model. Estimated{' '}
            {projectDuration(preview.wouldCheck, seedSecondsPerItem(activeDevice))}{' '}
            — based on {seedSecondsPerItem(activeDevice)}s per item measured on this machine’s{' '}
            {activeDevice === 'metal' ? 'GPU (Metal)' : 'CPU'}. Unverifiable items are instant and
            are not counted.
          </p>
          {preview.notExamined > 0 && (
            <p className="gds-ai__hint" data-testid="audit-not-examined">
              {preview.notExamined} sentence{preview.notExamined === 1 ? '' : 's'} carry no
              citation and are <b>not examined</b>. Checking whether a sentence needs a citation
              was measured at no better than flagging every sentence, so it was withdrawn — this
              audit checks citations that ARE present against their sources.
            </p>
          )}
          <p className="gds-ai__hint" data-testid="audit-wait-advice">
            {waitAdvice(preview.wouldCheck, activeDevice)}
          </p>
          <div className="gds-audit__actions">
            <Button
              variant="primary"
              onClick={start}
              disabled={(preview.consistency?.structural ?? 0) > 0 && !ackStructural}
              data-testid="audit-confirm-start"
            >
              Start the audit
            </Button>
            <Button
              variant="ghost"
              onClick={() => { setStage('idle'); setPreview(null); setFixNote(null); }}
              data-testid="audit-abandon"
            >
              Not now
            </Button>
          </div>
        </Card>
      )}

      {(stage === 'running' || stage === 'paused') && jobId != null && (
        <Card title="Audit running" data-testid="audit-running">
          <p className="gds-ai__value" data-testid="audit-progress">
            {progress ? `${progress.completed} of ${progress.total}` : 'starting…'}
          </p>
          <div className="gds-ai__progress">
            <div
              style={{
                width: progress && progress.total > 0
                  ? `${Math.round((progress.completed / progress.total) * 100)}%`
                  : '0%',
              }}
            />
          </div>
          {(() => {
            // The estimate switches from the seed figure to THIS run's own rate
            // as soon as there is one, and says which it is using. A number
            // labelled "measured on this machine" that was measured on a
            // different day, with different memory pressure, is worse than no
            // number: it reads as a promise.
            if (!progress || progress.total <= progress.completed) return null;
            const observed = observedSecondsPerItem(
              progress.completed,
              runStartedAt.current ? Date.now() - runStartedAt.current : 0,
            );
            const left = progress.total - progress.completed;
            const seed = seedSecondsPerItem(activeDevice);
            return (
              <p className="gds-ai__hint" data-testid="audit-remaining">
                About {projectDuration(left, observed ?? seed)} left —{' '}
                {observed
                  ? `${Math.round(observed)}s per item, measured on this run`
                  : `${seed}s per item until this run has measured its own rate`}
                .
              </p>
            );
          })()}

          {progress && (
            <p className="gds-ai__hint" data-testid="audit-latest">
              {progress.currentCategory}: {progress.latestItemSummary}
            </p>
          )}
          <div className="gds-audit__actions">
            {stage === 'running' ? (
              <Button variant="secondary" onClick={() => { bridge.pauseJob(jobId); setStage('paused'); }} data-testid="audit-pause">
                Pause
              </Button>
            ) : (
              <Button variant="primary" onClick={() => { bridge.resumeJob(jobId, onProgress); setStage('running'); }} data-testid="audit-resume-running">
                Resume
              </Button>
            )}
            <Button variant="danger" onClick={() => { bridge.cancelJob(jobId); setStage('idle'); }} data-testid="audit-cancel">
              Cancel
            </Button>
          </div>
        </Card>
      )}

      {health && (
        <Card title="Thesis health" data-testid="audit-health">
          <div className="gds-audit__stats">
            <div className="gds-audit__stat"><b>{health.completedItems}</b><span>checked</span></div>
            {/* §11 D116 split `flagged.length` into its three different things,
                because the actions differ. The "to skim" stat counted the
                advisory lane, which §11 D128 retired: it scored 18.3%
                population-weighted precision against an 18.0% no-skill
                baseline, so the number it showed was a count of noise. */}
            <div className="gds-audit__stat" data-testid="audit-stat-blocked">
              <b>{(health.flagged ?? []).filter((f: any) => f.kind === 'unverifiable').length}</b>
              <span>not checkable</span>
            </div>
            {/* "with passages" must COUNT PASSAGES. This counted
                `citation_support` items, so a judged claim that yielded no
                passage was still tallied here — the screen said 4 while the
                report's cover said "passages quoted for 3" (§11 D129's sibling:
                one thing, two numbers, because two places defined it). The row
                tally is a different and legitimate fact; it belongs under
                "Judged, by kind", not under this label. */}
            <div className="gds-audit__stat" data-testid="audit-stat-checked-claims">
              <b>
                {(health.flagged ?? []).filter(
                  (f: any) =>
                    f.kind === 'citation_support' &&
                    ((f.result?.output?.supporting_chunks ?? []).length > 0),
                ).length}
              </b>
              <span>with passages</span>
            </div>
          </div>

          {/* COUNTS BY CATEGORY. The backend has computed these all along —
              `countsPerCategory`, `verdictBreakdown`, `unverifiableReasons` —
              and the screen showed none of them, leading instead with all
              hundred sentences it had read. A reader needs the shape of the
              result before its raw material. */}
          <ul className="gds-audit__counts" data-testid="audit-counts">
            {Object.entries(health.countsPerCategory ?? {}).map(([kind, byStatus]) => {
              const total = Object.values(byStatus as Record<string, number>).reduce(
                (a, b) => a + b,
                0,
              );
              const failed = (byStatus as Record<string, number>).failed ?? 0;
              return (
                <li key={kind} data-testid={`audit-count-${kind}`}>
                  <b>{total}</b> {kind.replace(/_/g, ' ')}
                  {failed > 0 && <span className="gds-ai__hint"> · {failed} could not be judged</span>}
                </li>
              );
            })}
          </ul>

          {/* §11 D108, completed here. The report withdrew the support verdict
              and this screen kept rendering it as a count — "8 support — weak"
              — which is the same withdrawn grade wearing a summary. It is a
              constant (14 of 14 valid outputs across two runs), so the line
              said the same thing about every manuscript.
              The `need:` breakdown stays: citation_need's figures are measured
              and it genuinely answers both ways. */}
          {(() => {
            const breakdown = Object.entries(health.verdictBreakdown ?? {});
            const needs = breakdown.filter(([v]) => !v.startsWith('support:'));
            const located = breakdown
              .filter(([v]) => v.startsWith('support:'))
              .reduce((n, [, c]) => n + Number(c), 0);
            return (
              <>
                {needs.length > 0 && (
                  <ul className="gds-audit__counts" data-testid="audit-verdicts">
                    {needs.map(([v, n]) => (
                      <li key={v} data-testid={`audit-verdict-${v}`}>
                        <b>{String(n)}</b> {v.replace(/^need:/, '').replace(/_/g, ' ')}
                      </li>
                    ))}
                  </ul>
                )}
                {located > 0 && (
                  <p className="gds-ai__hint" data-testid="audit-support-located">
                    <b>{located}</b> cited sentence{located === 1 ? '' : 's'} had their source
                    passages located. Gaply does not grade how well a passage supports a
                    sentence — its grader returned the same answer for every case on a labelled
                    set, so that answer is not shown. The passages are, in the report.
                  </p>
                )}
              </>
            );
          })()}

          {Object.keys(health.unverifiableReasons ?? {}).length > 0 && (
            <ul className="gds-audit__counts" data-testid="audit-unverifiable-reasons">
              {Object.entries(health.unverifiableReasons ?? {}).map(([r, n]) => (
                <li key={r}>
                  <b>{String(n)}</b> {r}
                </li>
              ))}
            </ul>
          )}

          {Object.keys(health.skippedReasons ?? {}).length > 0 && (
            <p className="gds-ai__hint" data-testid="audit-skipped">
              Not judged:{' '}
              {Object.entries(health.skippedReasons ?? {})
                .map(([r, n]) => `${n} ${r}`)
                .join(', ')}
              .
            </p>
          )}
          {/* §11 D116. THREE KINDS land in `flagged`, and only `citation_support`
              carries a verdict or any evidence. Every one of them used to get a
              "Show evidence" button into the support EvidenceCard, so on a real
              job — 65 flagged items: 46 citation_need, 16 unverifiable, 3
              support, of which ONE had a verdict — 64 of 65 drill-downs read
              "No evidence retrieved" over an empty explanation.
              And the explanation was empty for a second reason: a
              `citation_need` result puts its prose at `result.output.reason`,
              while this read `result.reason`, which is where only an
              UNVERIFIABLE item keeps it. The field existed and was never read.

              Each kind now renders through the surface that fits it. */}
          {(health.flagged ?? []).map((f: any, i: number) => {
            const out = f.result?.output ?? {};
            return (
              <div
                className="gds-audit__item"
                key={i}
                data-testid={`audit-flagged-${i}`}
                data-kind={f.kind}
              >
                <p className="gds-audit__sentence">{f.sentence}</p>

                {/* §11 D128. The `citation_need` branch that stood here is gone
                    with the lane. No job plans those items, so nothing reaches
                    this list; a branch kept "just in case" would be an
                    un-exercised rendering of a retired verdict. */}
                {f.kind === 'unverifiable' ? (
                  /* A blocked source is a gap in the library, not a judgement.
                     Its reason IS at `result.reason` — deterministic text, not
                     a model's assertion — and the fix loop is below. */
                  <p className="gds-ai__hint" data-testid={`audit-flagged-blocked-${i}`}>
                    Cited, but not checkable — {String(f.result?.reason ?? 'no source available')}.
                    Nothing was checked either way; the actions are below.
                  </p>
                ) : (
                  <Button
                    variant="ghost"
                    data-testid={`audit-drill-${i}`}
                    onClick={() =>
                      setDrill({
                        verdict: (out.verdict as Verdict) ?? 'no_evidence',
                        confidence: out.confidence ?? null,
                        explanation: String(out.explanation ?? f.result?.reason ?? ''),
                        evidence: (out.supporting_chunks ?? []).map((c: any) => ({
                          chunkId: String(c.chunk_id ?? ''),
                          documentId: f.documentId ?? 0,
                          page: typeof c.page === 'number' ? c.page : null,
                          sourceLabel: f.sourceLabel ?? 'Cited source',
                          quote: String(c.quote ?? c.why ?? ''),
                          fileAvailable: f.fileAvailable ?? true,
                        })),
                      })
                    }
                  >
                    Show the passages
                  </Button>
                )}
              </div>
            );
          })}
          {drill && <EvidenceCard finding={drill} />}

          {/* ---- The loop that turns a report with no evidence into one with
                  evidence. Every unverifiable item here is blocked on ONE
                  thing: the cited work is not in the library. Saying so and
                  stopping is a dead end; the actions are the report. ---- */}
          {blocked.length > 0 && (
            <div data-testid="audit-blocked">
              <h4>Cited, but not checkable — {blocked.length}</h4>
              <p className="gds-ai__hint">
                These sentences DO cite something. Gaply could not check them because the cited
                work is not available to it — a gap in your library, not a defect in the writing.
              </p>

              <div className="gds-audit__actions">
                <Button
                  variant="primary"
                  disabled={busyAction !== null}
                  data-testid="audit-fetch-all"
                  onClick={() =>
                    void fetchForSources(
                      Array.from(
                        new Set(blocked.map((b) => b.citationId).filter(Boolean) as string[]),
                      ),
                      'fetch-all',
                    )
                  }
                >
                  {busyAction === 'fetch-all'
                    ? 'Looking for free copies…'
                    : `Fetch available PDFs for these ${
                        new Set(blocked.map((b) => b.citationId).filter(Boolean)).size
                      } sources`}
                </Button>
              </div>

              {/* Offered only once something ACTUALLY became checkable — asked
                  of the store, not inferred from a fetch reporting success. */}
              {nowCheckable.length > 0 && (
                <div className="gds-audit__actions">
                  <Button
                    variant="primary"
                    disabled={busyAction !== null}
                    onClick={() => void recheck()}
                    data-testid="audit-recheck"
                  >
                    {busyAction === 'recheck'
                      ? 'Re-checking…'
                      : `Re-check the ${nowCheckable.length} item${
                          nowCheckable.length === 1 ? '' : 's'
                        } that became checkable`}
                  </Button>
                </div>
              )}

              <ul className="gds-audit__blocked-list">
                {blocked.map((b) => (
                  <li key={b.seq} data-testid={`audit-blocked-${b.seq}`}>
                    <p className="gds-audit__sentence">{b.sentence}</p>
                    {b.reason && <p className="gds-ai__hint">{b.reason}</p>}
                    {b.citationId && (
                      <div className="gds-audit__actions">
                        <Button
                          variant="secondary"
                          disabled={busyAction !== null}
                          data-testid={`audit-fetch-${b.seq}`}
                          onClick={() => void fetchForSources([b.citationId!], `fetch-${b.seq}`)}
                        >
                          {busyAction === `fetch-${b.seq}`
                            ? 'Looking…'
                            : 'Fetch open-access PDF'}
                        </Button>
                        <Button
                          variant="ghost"
                          disabled={busyAction !== null}
                          data-testid={`audit-attach-${b.seq}`}
                          onClick={() => onOpenCitation?.(b.citationId!)}
                        >
                          Attach PDF…
                        </Button>
                      </div>
                    )}
                    {b.citationId && fetchNotes[b.citationId] && (
                      <p className="gds-ai__hint" data-testid={`audit-fetch-note-${b.seq}`}>
                        {fetchNotes[b.citationId]}
                      </p>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {/* ---- Export ---- */}
          <div className="gds-audit__actions" data-testid="audit-export">
            <Button
              variant="secondary"
              disabled={busyAction !== null}
              onClick={() => void exportReport('pdf')}
              data-testid="audit-export-pdf"
            >
              {busyAction === 'export-pdf' ? 'Saving…' : 'Export report (PDF)'}
            </Button>
            <Button
              variant="ghost"
              disabled={busyAction !== null}
              onClick={() => void exportReport('html')}
              data-testid="audit-export-html"
            >
              Export as HTML
            </Button>
          </div>
          {exportNote && (
            <p className="gds-ai__hint" data-testid="audit-export-note">
              {exportNote}
            </p>
          )}
        </Card>
      )}

      {items.length > 0 && (
        <button
          type="button"
          className="gds-link"
          aria-expanded={dumpOpen}
          onClick={() => setDumpOpen((v) => !v)}
          data-testid="audit-dump-toggle"
        >
          {dumpOpen
            ? 'Hide every sentence the audit read'
            : `Show every sentence the audit read (${items.length})`}
        </button>
      )}

      {/* §11 D92. The manuscript itself, with each judgement drawn where it
          happened. PDF only — a .docx has no page geometry to annotate. */}
      {stage === 'done' && items.length > 0 && path && (
        <div data-testid="annot-section">
          <button
            type="button"
            className="gds-link"
            onClick={() => setAnnotOpen((v) => !v)}
            data-testid="annot-toggle"
          >
            {annotOpen ? 'Hide the annotated manuscript' : 'Show the annotated manuscript'}
          </button>
          {annotOpen &&
            (path.toLowerCase().endsWith('.pdf') ? (
              // §11 D109. The `as any` here is what let the shape mismatch
              // ship: AuditItem carries `resultJson`, the component read
              // `result.output`. No cast now, so the compiler checks it.
              <AnnotatedManuscript path={path} items={items} />
            ) : (
              <AnnotatedUnavailable onOpenReport={() => setAnnotOpen(false)} />
            ))}
        </div>
      )}

      {items.length > 0 && dumpOpen && (
        <Card title="Every sentence read" data-testid="audit-results">
          {Object.entries(grouped).map(([kind, list]) => (
            <div className="gds-audit__group" key={kind} data-testid={`audit-group-${kind}`}>
              <h4>{kind.replace('_', ' ')} — {list.length}</h4>
              {list.map((it) => (
                <div className="gds-audit__item" key={it.seq} data-testid={`audit-item-${it.seq}`}>
                  {/* §11 D89 kept a suggestion out of the `assessed` badge, so
                      an unchecked opinion could not look like an adjudicated
                      result. §11 D128 retired the lane outright, so there is no
                      second case left to distinguish. */}
                  <Badge status={it.kind === 'unverifiable' ? 'neutral' : 'assessed'}>
                    {it.page === null ? 'page unknown' : `p.${it.page}`}
                  </Badge>
                  <p className="gds-audit__sentence">{it.sentence}</p>
                  {it.kind === 'unverifiable' && (
                    <div className="gds-audit__actions">
                      {/* DEFERRED, and said so rather than looking broken. */}
                      <Button variant="ghost" disabled title="Not yet available" data-testid={`audit-link-${it.seq}`}>
                        Link source (coming soon)
                      </Button>
                      <Button variant="ghost" disabled title="Not yet available" data-testid={`audit-index-${it.seq}`}>
                        Index document (coming soon)
                      </Button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          ))}
        </Card>
      )}

    </div>
  );
};

export default ThesisAuditScreen;
