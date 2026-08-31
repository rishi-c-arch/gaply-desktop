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
import { aiBridge, AuditPlan, JobProgressEvent, errorText } from './aiBridge';
import { EvidenceCard, GroundedFinding, Verdict } from './EvidenceCard';
import { AiUnavailable } from './AiStatusPanel';

/**
 * Per-item seconds for the projection BEFORE a run has measured anything.
 *
 * It is a starting figure, not a property of the machine. It was measured once,
 * on a machine with room to spare; the same model on the same laptop under
 * memory pressure has been observed at a fifth of that speed. So it seeds the
 * estimate and is replaced by the run's own rate as soon as one exists — see
 * `observedSecondsPerItem`. Stated on screen either way, and labelled with
 * which of the two it is.
 */
export const SECONDS_PER_ITEM = 65;

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
  aiInstalled: boolean;
  onOpenSettings?: () => void;
  /** A job left running/paused by a previous launch, offered for resume. */
  resumableJob?: { jobId: number; completed: number; total: number } | null;
  pickManuscript?: () => Promise<string | null>;
  bridge?: Pick<
    typeof aiBridge,
    'startThesisAudit' | 'jobStatus' | 'pauseJob' | 'resumeJob' | 'cancelJob' | 'jobResults'
  >;
}

export const ThesisAuditScreen: React.FC<ThesisAuditScreenProps> = ({
  aiInstalled,
  onOpenSettings,
  resumableJob = null,
  pickManuscript,
  bridge = aiBridge,
}) => {
  const [stage, setStage] = useState<Stage>('idle');
  const [plan, setPlan] = useState<AuditPlan | null>(null);
  const [path, setPath] = useState<string | null>(null);
  const [progress, setProgress] = useState<JobProgressEvent | null>(null);
  /** When this run's items started landing, for the measured rate. */
  const runStartedAt = useRef<number | null>(null);
  const [items, setItems] = useState<AuditItem[]>([]);
  const [health, setHealth] = useState<Record<string, any> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [drill, setDrill] = useState<GroundedFinding | null>(null);

  const jobId = plan?.jobId ?? resumableJob?.jobId ?? null;

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
    // NOTE: the backend plans AND persists the job in one call, so this is the
    // point of no return for item creation — but NOT for running the model.
    // Nothing is generated until `start` below.
    try {
      const planned = await bridge.startThesisAudit(p, onProgress);
      setPlan(planned);
      setStage('planned');
    } catch (e) {
      setError(errorText(e));
    }
  }, [bridge, pickManuscript, onProgress]);

  const start = useCallback(async () => {
    if (!plan) return;
    setStage('running');
    try {
      await bridge.resumeJob(plan.jobId, onProgress);
    } catch (e) {
      setError(errorText(e));
    }
  }, [bridge, plan, onProgress]);

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

      {plan && stage === 'planned' && (
        <Card title="Before you start" data-testid="audit-plan">
          <p className="gds-ai__hint">{path}</p>
          <div className="gds-audit__stats">
            <div className="gds-audit__stat"><b>{plan.totalSentences}</b><span>sentences</span></div>
            <div className="gds-audit__stat"><b>{plan.cited}</b><span>cited</span></div>
            <div className="gds-audit__stat"><b>{plan.uncited}</b><span>uncited</span></div>
            <div className="gds-audit__stat"><b>{plan.skipped}</b><span>filtered out</span></div>
            <div className="gds-audit__stat" data-testid="audit-plan-unverifiable">
              <b>{plan.queuedUnverifiable}</b><span>can’t be checked</span>
            </div>
          </div>
          <p className="gds-ai__hint" data-testid="audit-projection">
            {queued} items queued; {modelItems} need the model. Estimated{' '}
            {projectDuration(modelItems)} — based on {SECONDS_PER_ITEM}s per item measured on this
            machine’s CPU. Unverifiable items are instant and are not counted.
          </p>
          <div className="gds-audit__actions">
            <Button variant="primary" onClick={start} data-testid="audit-confirm-start">
              Start the audit
            </Button>
            <Button variant="ghost" onClick={() => { setStage('idle'); setPlan(null); }} data-testid="audit-abandon">
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
            return (
              <p className="gds-ai__hint" data-testid="audit-remaining">
                About {projectDuration(left, observed ?? SECONDS_PER_ITEM)} left —{' '}
                {observed
                  ? `${Math.round(observed)}s per item, measured on this run`
                  : `${SECONDS_PER_ITEM}s per item until this run has measured its own rate`}
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

      {items.length > 0 && (
        <Card title="Results" data-testid="audit-results">
          {Object.entries(grouped).map(([kind, list]) => (
            <div className="gds-audit__group" key={kind} data-testid={`audit-group-${kind}`}>
              <h4>{kind.replace('_', ' ')} — {list.length}</h4>
              {list.map((it) => (
                <div className="gds-audit__item" key={it.seq} data-testid={`audit-item-${it.seq}`}>
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

      {health && (
        <Card title="Thesis health" data-testid="audit-health">
          <div className="gds-audit__stats">
            <div className="gds-audit__stat"><b>{health.completedItems}</b><span>checked</span></div>
            <div className="gds-audit__stat"><b>{(health.flagged ?? []).length}</b><span>need review</span></div>
          </div>
          {(health.flagged ?? []).map((f: any, i: number) => (
            <div className="gds-audit__item" key={i} data-testid={`audit-flagged-${i}`}>
              <p className="gds-audit__sentence">{f.sentence}</p>
              <Button
                variant="ghost"
                data-testid={`audit-drill-${i}`}
                onClick={() =>
                  setDrill({
                    verdict: (f.result?.output?.verdict as Verdict) ?? 'no_evidence',
                    confidence: f.result?.output?.confidence ?? null,
                    explanation: String(f.result?.output?.explanation ?? f.result?.reason ?? ''),
                    evidence: (f.result?.output?.supporting_chunks ?? []).map((c: any) => ({
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
                Show evidence
              </Button>
            </div>
          ))}
          {drill && <EvidenceCard finding={drill} />}
        </Card>
      )}
    </div>
  );
};

export default ThesisAuditScreen;
