// Gaply — AI Check (Set 5 two-way tiered analysis + Sets 1-3 progress/cancel).
// Local, free. Renders the honest two-way report over run_aicheck, plus:
//  - a re-checkable pre-flight memory panel (truthful about the attainable tier),
//  - a live progress UI consuming the event channel (stage + memory-skip notes),
//  - a Cancel button that stops in seconds,
//  - an honest Cancelled state (nothing renders — partial isn't a valid signal).
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
} from '../../design-system';
import { ACCEPT_HINT, estimatePdfPageCount, validateFile } from '../analysis/validateFile';
import { basenameOf, pickManuscriptPath } from '../common/pickFile';
import { isTauri } from '../../utils/isTauri';
import { isFeatureEnabled } from '../../config/featureFlags';
import AiCheckReport from './AiCheckReport';
import { CheckBridge, TauriCheckBridge } from './checkBridge';
import { AiCheckEvent, AiCheckMemoryStatus, AiCheckResult } from './agentTypes';
import { mayUseCloud } from '../settings/settingsStore';
import { readVerifyCitations, setVerifyCitations } from '../settings/settingsStore';
import './aicheck.css';

export interface AiCheckPageProps {
  bridge?: CheckBridge;
}

interface Cancelled {
  stage: string;
  done: number;
  total: number;
}

/** The fixed analysis stages, in order — the checklist backbone. */
export const STAGES = [
  'Reading the document',
  'Fast pre-pass',
  'Stage-1 language model',
  'Deep verification',
  'Finishing',
] as const;

/** Which stage index an event advances to (memory_skip / cancelled don't move). */
export const EVENT_STAGE: Partial<Record<AiCheckEvent['type'], number>> = {
  extract: 0,
  pre_pass: 1,
  stage1_lm: 2,
  deep_verify: 3,
  report: 4,
};

/** A memory-skip note belongs UNDER the stage it's ABOUT, not the running one
 *  (the deep skip fires while Stage-1 is still the current stage). */
const skipTargetStage = (model: string): number => (model.includes('Stage-1') ? 2 : 3);

/** The honest Cancelled detail — stage-aware (Stage-1 isn't passage-scoring). */
function cancelledDetail(c: Cancelled): string {
  if (c.stage.includes('deep')) {
    return `Cancelled during deep verification; ${c.done} of ${c.total} passages scored. Results not shown, as partial analysis isn't a valid signal.`;
  }
  return `Cancelled during the Stage-1 language-model pass. Results not shown, as partial analysis isn't a valid signal.`;
}

const AiCheckPage: React.FC<AiCheckPageProps> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  const navigate = useNavigate();
  const [selected, setSelected] = useState<{ name: string; path: string } | null>(null);
  const [result, setResult] = useState<AiCheckResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [verifyCitations, setVerify] = useState<boolean>(() => readVerifyCitations());
  // Progress + cancel state.
  const [mem, setMem] = useState<AiCheckMemoryStatus | null>(null);
  const [stageIdx, setStageIdx] = useState(-1);
  const [deep, setDeep] = useState<{ done: number; total: number } | null>(null);
  const [skipByStage, setSkipByStage] = useState<Record<number, string[]>>({});
  const [cancelling, setCancelling] = useState(false);
  const [cancelled, setCancelled] = useState<Cancelled | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Pre-flight memory status on mount (re-checkable). Silent on failure — this
  // is a desktop feature; a non-Tauri context simply shows no panel.
  const refreshMemory = async () => {
    try {
      setMem(await b.aicheckMemoryStatus());
    } catch {
      setMem(null);
    }
  };
  useEffect(() => {
    void refreshMemory();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [b]);

  const acceptFile = async (file: File) => {
    setError(null);
    const pageCount = await estimatePdfPageCount(file);
    const res = validateFile({ name: file.name, sizeBytes: file.size, pageCount });
    if (!res.ok) {
      setError(res.error);
      return;
    }
    setSelected({ name: file.name, path: (file as any).path ?? file.name });
    setResult(null);
  };

  const acceptPath = (path: string) => {
    setError(null);
    const name = basenameOf(path);
    const res = validateFile({ name, sizeBytes: 1 });
    if (!res.ok) {
      setError(res.error);
      return;
    }
    setSelected({ name, path });
    setResult(null);
  };

  const pickFile = async () => {
    if (isTauri) {
      const p = await pickManuscriptPath(['pdf', 'docx', 'txt', 'md'], 'Manuscript');
      if (p) acceptPath(p);
    } else {
      inputRef.current?.click();
    }
  };

  const onEvent = (ev: AiCheckEvent) => {
    if (ev.type === 'deep_verify') {
      setStageIdx(3);
      setDeep({ done: ev.done, total: ev.total });
    } else if (ev.type === 'memory_skip') {
      const at = skipTargetStage(ev.model);
      setSkipByStage((m) => ({ ...m, [at]: [...(m[at] ?? []), `${ev.model} skipped: ${ev.reason}`] }));
    } else if (ev.type === 'cancelled') {
      setCancelled({ stage: ev.stage, done: ev.done, total: ev.total });
    } else if (EVENT_STAGE[ev.type] !== undefined) {
      setStageIdx(EVENT_STAGE[ev.type]!);
    }
  };

  const runNow = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setResult(null);
    setCancelled(null);
    setStageIdx(-1);
    setDeep(null);
    setSkipByStage({});
    setCancelling(false);
    const doVerify = verifyCitations && mayUseCloud('citation_verification');
    try {
      setResult(await b.aicheck(selected.path, doVerify, onEvent));
    } catch (e) {
      // The "cancelled" code is a benign stop — no error toast; the Cancelled
      // state (set by the cancelled event) renders instead.
      if ((e as { code?: string })?.code === 'cancelled') {
        setCancelled((c) => c ?? { stage: 'deep verification', done: 0, total: 0 });
      } else {
        setError(e instanceof Error ? e.message : 'check failed');
      }
    } finally {
      setBusy(false);
      setCancelling(false);
    }
  };

  const doCancel = async () => {
    setCancelling(true);
    try {
      await b.cancelAicheck();
    } catch {
      /* the run's own rejection carries the terminal state */
    }
  };

  const reset = () => {
    setResult(null);
    setCancelled(null);
    setError(null);
    setStageIdx(-1);
    setDeep(null);
    setSkipByStage({});
    void refreshMemory();
  };

  const memChip = (m: AiCheckMemoryStatus) =>
    m.deep_fits ? { text: 'Ready', cls: 'aic-chip--ready' }
    : m.tier_attainable === 'heuristic_only' ? { text: 'Pre-pass only', cls: 'aic-chip--pre' }
    : { text: 'Needs memory', cls: 'aic-chip--needs' };

  return (
    <div className="gds-root gds-aicheck" style={{ height: '100vh' }} data-testid="ai-check">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'plag', label: 'Plagiarism Check', icon: '≡', onSelect: () => navigate('/app/check/plagiarism') },
              { id: 'ai', label: 'AI Check', icon: '◬', onSelect: () => navigate('/app/check/ai') },
              ...(isFeatureEnabled('statsCheck')
                ? [{ id: 'stats', label: 'Statistical Analysis Check', icon: 'Σ', onSelect: () => navigate('/app/check/stats') }]
                : []),
            ]}
            activeId=""
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="AI Check">
            <Badge status="certain">local · free</Badge>
          </HeaderBar>
        }
      >
        <Panel title="AI Check">
          <div style={{ display: 'grid', gap: 16 }}>
            {result ? (
              <div data-testid="check-report">
                <AiCheckReport result={result} />
                <div style={{ marginTop: 12 }}>
                  <Button variant="ghost" onClick={reset} data-testid="run-another">
                    ← Run another
                  </Button>
                </div>
              </div>
            ) : cancelled ? (
              <>
                <Card title="Analysis cancelled" data-testid="cancelled-state">
                  <p style={{ margin: 0, color: 'var(--g-text-2)', fontSize: 13 }}>
                    {cancelledDetail(cancelled)}
                  </p>
                  <div style={{ marginTop: 12 }}>
                    <Button onClick={reset} data-testid="run-another">Run again</Button>
                  </div>
                </Card>
              </>
            ) : busy ? (
              <Card title="Analyzing…" data-testid="progress">
                <ol className="aic-stages">
                  {STAGES.map((label, i) => {
                    const status = i < stageIdx ? 'done' : i === stageIdx ? 'running' : 'pending';
                    const notes = skipByStage[i] ?? [];
                    return (
                      <li
                        key={i}
                        className={`aic-stage aic-stage--${status}`}
                        data-testid={status === 'running' ? 'progress-stage' : undefined}
                      >
                        <span className="aic-stage__dot" aria-hidden="true" />
                        <div>
                          <span className="aic-stage__label">{label}</span>
                          {/* Deep verification expands to the live counter + slim fill. */}
                          {i === 3 && deep && status !== 'pending' && (
                            <div className="aic-stage__deep" data-testid="progress-bar">
                              <span className="aic-stage__counter">passage {deep.done} of {deep.total}</span>
                              <div className="aic-stage__track">
                                <div
                                  className="aic-stage__fill"
                                  style={{ width: `${Math.round((deep.done / Math.max(deep.total, 1)) * 100)}%` }}
                                />
                              </div>
                            </div>
                          )}
                          {notes.map((n, k) => (
                            <p key={k} className="aic-stage__skip" data-testid="memory-skip-note">⚠ {n}</p>
                          ))}
                        </div>
                      </li>
                    );
                  })}
                </ol>
                <div className="aic-progress__actions">
                  <Button variant="secondary" onClick={() => void doCancel()} disabled={cancelling} data-testid="cancel-run">
                    {cancelling ? 'Cancelling…' : 'Cancel'}
                  </Button>
                </div>
              </Card>
            ) : (
              <div className="aic-idle">
                <div className="aic-hero">
                  <p className="aic-hero__eyebrow">AI Check</p>
                  <h2 className="aic-hero__title">Check your manuscript</h2>
                </div>

                {/* The centered input card — the picker as the focal object. */}
                <div className="aic-inputcard">
                  <div className="aic-inputcard__row">
                    <Button variant="secondary" onClick={() => void pickFile()} data-testid="pick-file">
                      Choose file
                    </Button>
                    {selected && <span className="gds-mono" data-testid="selected-name">{selected.name}</span>}
                    <input
                      ref={inputRef}
                      type="file"
                      accept=".pdf,.docx,.txt,.md"
                      style={{ display: 'none' }}
                      data-testid="file-input"
                      onChange={(e) => e.target.files?.[0] && void acceptFile(e.target.files[0])}
                    />
                    <span className="aic-inputcard__spacer" />
                    <Button onClick={runNow} disabled={!selected || busy} data-testid="run-check">
                      Run check
                    </Button>
                  </div>
                  <label className="aic-optin">
                    <input
                      type="checkbox"
                      checked={verifyCitations}
                      data-testid="verify-citations"
                      onChange={(e) => {
                        setVerify(e.target.checked);
                        setVerifyCitations(e.target.checked);
                      }}
                    />
                    <span>
                      Verify references online
                      <span className="aic-optin__help">
                        Sends only citation details (author, year, title, DOI) to CrossRef/OpenAlex, never your
                        manuscript. Off by default. You can turn this off anytime.
                      </span>
                    </span>
                  </label>
                  {error && <p className="aic-error" role="alert" data-testid="check-error">{error}</p>}
                </div>

                <div className="aic-duo">
                {mem && (() => {
                  const chip = memChip(mem);
                  const freeGb = mem.free_mb / 1024;
                  const needGb = mem.deep_need_mb ? mem.deep_need_mb / 1024 : 0;
                  const scale = Math.max(needGb * 1.3, freeGb, 1);
                  const freePct = Math.min(100, (freeGb / scale) * 100);
                  const needPct = Math.min(100, (needGb / scale) * 100);
                  return (
                    <Card title="On-device capacity" data-testid="memory-status">
                      <div className="aic-capacity__head">
                        <span className={`aic-chip ${chip.cls}`}>{chip.text}</span>
                        <Button variant="ghost" onClick={() => void refreshMemory()} data-testid="memory-recheck">
                          Re-check
                        </Button>
                      </div>
                      <p className="aic-tier" data-testid="memory-tier">This device runs {mem.tier_label}.</p>
                      {/* Meter ILLUSTRATES the gap; the numbers below STATE it. */}
                      {needGb > 0 && (
                        <div className="aic-meter" aria-hidden="true">
                          <div
                            className={`aic-meter__fill ${mem.deep_fits ? 'aic-meter__fill--ready' : 'aic-meter__fill--needs'}`}
                            style={{ width: `${freePct}%` }}
                          />
                          <div className="aic-meter__marker" style={{ left: `${needPct}%` }} />
                        </div>
                      )}
                      <p className="aic-capacity__nums">
                        <strong>{freeGb.toFixed(1)} GB</strong> free of {mem.total_gb.toFixed(1)} GB.
                      </p>
                      {/* HINT GUARD: never contradict a "Ready" chip with a "free memory" nudge. */}
                      {!mem.deep_fits && (
                        <p className="aic-hint-line" data-testid="memory-hint">{mem.hint}</p>
                      )}
                    </Card>
                  );
                })()}

                  {/* How it works — the locked intro copy, verbatim. */}
                  <div className="aic-infocard">
                    <h3 className="aic-infocard__title">How it works</h3>
                    <p className="aic-lead">
                      Every manuscript gets a fast local pre-pass and a reading by a small on-device
                      language model. Where memory allows, flagged passages also get deep verification
                      by a larger on-device model: the compact 1.5B on most machines, the full 7B on
                      devices with about 16 GB of RAM or more. Findings remain signals, not verdicts,
                      and your manuscript never leaves your device. Optionally, Gaply can check your
                      reference list's details (author, year, title, DOI; never your text) against
                      public scholarly databases to flag citations it can't verify.{' '}
                      <span className="aic-hint">· {ACCEPT_HINT}</span>
                    </p>
                  </div>
                </div>

                <Link className="aic-back" to="/app">← Back to Home</Link>
              </div>
            )}
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

export default AiCheckPage;
