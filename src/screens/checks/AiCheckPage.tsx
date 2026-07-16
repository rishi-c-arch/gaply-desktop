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

/** Current live stage (from the event channel). `total` present → show the bar. */
interface Stage {
  label: string;
  done?: number;
  total?: number;
}
interface Cancelled {
  stage: string;
  done: number;
  total: number;
}

/** The stage label for an event — pure, so it's unit-testable + reused. */
export function stageLabel(ev: AiCheckEvent): string | null {
  switch (ev.type) {
    case 'extract':
      return 'Reading the document…';
    case 'pre_pass':
      return 'Fast pre-pass…';
    case 'stage1_lm':
      return 'Stage-1 language model…';
    case 'deep_verify':
      return `Deep verification — passage ${ev.done} of ${ev.total}`;
    case 'report':
      return 'Finishing…';
    default:
      return null; // memory_skip / cancelled don't move the stage
  }
}

/** The honest Cancelled detail — stage-aware (Stage-1 isn't passage-scoring). */
function cancelledDetail(c: Cancelled): string {
  if (c.stage.includes('deep')) {
    return `Cancelled during deep verification; ${c.done} of ${c.total} passages scored — results not shown, as partial analysis isn't a valid signal.`;
  }
  return `Cancelled during the Stage-1 language-model pass — results not shown, as partial analysis isn't a valid signal.`;
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
  const [stage, setStage] = useState<Stage | null>(null);
  const [skipNotes, setSkipNotes] = useState<string[]>([]);
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
      const p = await pickManuscriptPath(['pdf', 'docx'], 'Manuscript');
      if (p) acceptPath(p);
    } else {
      inputRef.current?.click();
    }
  };

  const onEvent = (ev: AiCheckEvent) => {
    const label = stageLabel(ev);
    if (label) {
      setStage(ev.type === 'deep_verify' ? { label, done: ev.done, total: ev.total } : { label });
    } else if (ev.type === 'memory_skip') {
      setSkipNotes((n) => [...n, `${ev.model} skipped — ${ev.reason}`]);
    } else if (ev.type === 'cancelled') {
      setCancelled({ stage: ev.stage, done: ev.done, total: ev.total });
    }
  };

  const runNow = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setResult(null);
    setCancelled(null);
    setStage(null);
    setSkipNotes([]);
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
    setStage(null);
    setSkipNotes([]);
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
                <p style={{ margin: '0 0 10px', fontSize: 14, color: 'var(--g-text)' }} data-testid="progress-stage">
                  {stage?.label ?? 'Starting…'}
                </p>
                {stage?.total ? (
                  <div style={{ marginBottom: 10 }} data-testid="progress-bar">
                    <div style={{ height: 8, background: 'var(--g-border)', borderRadius: 4, overflow: 'hidden' }}>
                      <div
                        style={{
                          height: '100%',
                          width: `${Math.round(((stage.done ?? 0) / Math.max(stage.total, 1)) * 100)}%`,
                          background: 'var(--g-accent)',
                        }}
                      />
                    </div>
                    <span style={{ fontSize: 12, color: 'var(--g-text-3)' }}>{stage.done} / {stage.total}</span>
                  </div>
                ) : null}
                {skipNotes.map((n, i) => (
                  <p key={i} data-testid="memory-skip-note" style={{ margin: '2px 0', fontSize: 12, color: 'var(--g-flagged, #b26a00)' }}>
                    ⚠ {n}
                  </p>
                ))}
                <div style={{ marginTop: 12 }}>
                  <Button variant="secondary" onClick={() => void doCancel()} disabled={cancelling} data-testid="cancel-run">
                    {cancelling ? 'Cancelling…' : 'Cancel'}
                  </Button>
                </div>
              </Card>
            ) : (
              <>
                {/* Calm lead-in — locked copy, restyled quiet. */}
                <p className="aic-lead">
                  Up to two analysis stages — Every manuscript receives a fast local pre-pass. On
                  devices with approximately 16 GB or more RAM, Gaply also performs an additional
                  local deep verification using the 7B model. Findings remain signals—not
                  verdicts—and your manuscript never leaves your device. Optionally, Gaply can check
                  your reference list's details — author, year, title, DOI (never your text) —
                  against public scholarly databases to flag citations it can't verify.{' '}
                  <span className="aic-hint">· {ACCEPT_HINT}</span>
                </p>

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

                <Card title="Select a manuscript">
                  <div className="aic-picker">
                    <Button variant="secondary" onClick={() => void pickFile()} data-testid="pick-file">
                      Choose file
                    </Button>
                    {selected && <span className="gds-mono" data-testid="selected-name">{selected.name}</span>}
                    <input
                      ref={inputRef}
                      type="file"
                      accept=".pdf,.docx"
                      style={{ display: 'none' }}
                      data-testid="file-input"
                      onChange={(e) => e.target.files?.[0] && void acceptFile(e.target.files[0])}
                    />
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
                        Sends only citation details (author, year, title, DOI) to CrossRef/OpenAlex — never your
                        manuscript. Off by default. You can turn this off anytime.
                      </span>
                    </span>
                  </label>
                  {error && <p className="aic-error" role="alert" data-testid="check-error">{error}</p>}
                </Card>
                <Link className="aic-back" to="/app">← Back to Home</Link>
              </>
            )}
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

export default AiCheckPage;
