// Gaply — manuscript upload + LIVE analysis theater (F5). Everything is LOCAL:
// the Rust core reads the manuscript from a path and returns structured
// reports; no manuscript bytes are ever sent to Supabase/Railway from here.
import React, { useMemo, useRef, useState } from 'react';
import { Link, useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { Badge, Button, Card, Panel } from '../../design-system';
import { isTauri } from '../../utils/isTauri';
import { useGaplySession } from '../session/SessionProvider';
import {
  AnalysisBridge,
  TauriAnalysisBridge,
} from './bridge';
import {
  ACCEPT_HINT,
  estimatePdfPageCount,
  FileMeta,
  validateFile,
} from './validateFile';
import {
  LaneState,
  STAGE_LABEL,
  STAGE_ORDER,
  useAnalysis,
} from './useAnalysis';
import '../auth/auth.css';
import './analysis.css';

export interface AnalysisTheaterPageProps {
  /** Test seams. */
  bridge?: AnalysisBridge;
  /** Skip the picker: begin with a preselected file (drop / sample). */
  initialFile?: { name: string; path: string };
  autoStart?: boolean;
}

function riskColor(risk: number): string {
  if (risk >= 0.6) return 'var(--g-flagged)';
  if (risk >= 0.4) return 'var(--g-assessed)';
  return 'var(--g-certain)';
}

const AnalysisTheaterPage: React.FC<AnalysisTheaterPageProps> = ({
  bridge,
  initialFile,
  autoStart,
}) => {
  const { session } = useGaplySession();
  const navigate = useNavigate();
  const location = useLocation() as { state?: { fileName?: string; path?: string } };
  const [searchParams] = useSearchParams();
  const isSample = searchParams.get('sample') === '1';

  const realBridge = useMemo(() => bridge ?? new TauriAnalysisBridge(), [bridge]);
  const { state, start, abort } = useAnalysis(realBridge);

  // premium unlocks the verification (cloud) lane — free/offline is teased.
  const tier: 'free' | 'premium' = 'free'; // wired to F2 subscription tier in F6+
  const droppedName = location.state?.fileName;

  // NOTE: the sample no longer seeds a fake path — startSample() resolves a
  // REAL bundled file via the core before running (the old '__bundled_sample__'
  // sentinel pointed at no file and failed extraction).
  const seedFile =
    initialFile ?? (droppedName ? { name: droppedName, path: droppedName } : null);

  const [selected, setSelected] = useState<{ name: string; path: string } | null>(seedFile);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [phase, setPhase] = useState<'upload' | 'theater'>(
    autoStart && seedFile ? 'theater' : 'upload'
  );
  const inputRef = useRef<HTMLInputElement>(null);
  const startedRef = useRef(false);

  // auto-start once when seeded (sample flow)
  React.useEffect(() => {
    if (phase === 'theater' && selected && !startedRef.current) {
      startedRef.current = true;
      void start({ path: selected.path, title: selected.name, tier });
    }
  }, [phase, selected, start, tier]);

  // Accept a file by its ABSOLUTE path (the desktop path). This is the correct
  // desktop flow: HTML File objects have NO usable path inside a Tauri webview,
  // so paths must come from the Tauri dialog / drag-drop APIs below.
  const acceptPath = React.useCallback((path: string) => {
    const name = path.split(/[\\/]/).pop() || path;
    const res = validateFile({ name, sizeBytes: 1 }); // size/pages are enforced by the core
    if (!res.ok) {
      setError(res.error);
      return;
    }
    setError(null);
    setSelected({ name, path });
    setPhase('theater');
  }, []);

  // Native file picker → returns an absolute path (never a bare filename).
  const pickViaTauri = React.useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const chosen = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Manuscript', extensions: ['pdf', 'docx', 'txt', 'md'] }],
      });
      if (typeof chosen === 'string') acceptPath(chosen);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not open the file picker.');
    }
  }, [acceptPath]);

  // One-click sample: the core writes the bundled manuscript to a real temp
  // file and returns its absolute path, so the sample runs the REAL pipeline.
  const startSample = React.useCallback(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const path = await invoke<string>('sample_manuscript_path');
      acceptPath(path);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not load the sample manuscript.');
    }
  }, [acceptPath]);

  // Tauri drag-drop delivers ABSOLUTE paths; HTML drop events do not in a
  // webview. This is the real fix for the "Extraction error" bug (the old code
  // passed file.name, so the core got a bare filename it couldn't open).
  React.useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        unlisten = await getCurrentWebview().onDragDropEvent((event: { payload: { type: string; paths?: string[] } }) => {
          const p = event.payload;
          if (p.type === 'over') setDragOver(true);
          else if (p.type === 'leave' || p.type === 'cancelled') setDragOver(false);
          else if (p.type === 'drop') {
            setDragOver(false);
            const first = p.paths?.[0];
            if (first) acceptPath(first);
          }
        });
      } catch {
        /* API unavailable (e.g. plain browser) — HTML fallback below handles it */
      }
    })();
    return () => {
      if (unlisten) unlisten();
    };
  }, [acceptPath]);

  const acceptFile = async (file: File) => {
    setError(null);
    const pageCount = await estimatePdfPageCount(file);
    const meta: FileMeta = { name: file.name, sizeBytes: file.size, pageCount };
    const res = validateFile(meta);
    if (!res.ok) {
      setError(res.error);
      return;
    }
    // NOTE: real filesystem path comes from the Tauri dialog/drag events in the
    // desktop app; in the browser we carry the name (the Rust core is the only
    // reader of bytes and only runs under Tauri).
    setSelected({ name: file.name, path: (file as any).path ?? file.name });
    setPhase('theater');
  };

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files?.[0];
    if (file) void acceptFile(file);
  };

  const runNow = () => {
    if (!selected) return;
    startedRef.current = true;
    void start({ path: selected.path, title: selected.name, tier });
    setPhase('theater');
  };

  /* ------------------------------ UPLOAD ------------------------------ */
  if (phase === 'upload') {
    return (
      <div className="gds-root gds-onboarding" data-testid="upload-page">
        <Panel title="New analysis" className="gds-onboarding__card">
          <div className="gds-upload">
            {isSample ? (
              <Card glass data-testid="upload-sample">
                <Badge status="neutral">sample</Badge> Loaded a bundled sample manuscript.
                <div style={{ marginTop: 12 }}>
                  <Button onClick={() => void startSample()} data-testid="run-sample">Run the sample scan</Button>
                </div>
              </Card>
            ) : droppedName ? (
              <Card glass data-testid="upload-file">
                <Badge status="neutral">queued</Badge>{' '}
                <span className="gds-mono">{droppedName}</span> received — parsed on your device.
                <div style={{ marginTop: 12 }}>
                  <Button onClick={runNow}>Start analysis</Button>
                </div>
              </Card>
            ) : (
              <div
                className="gds-upload__drop"
                data-over={dragOver}
                data-testid="upload-fresh"
                role="button"
                tabIndex={0}
                onClick={() => (isTauri ? void pickViaTauri() : inputRef.current?.click())}
                onKeyDown={(e) =>
                  (e.key === 'Enter' || e.key === ' ') &&
                  (isTauri ? void pickViaTauri() : inputRef.current?.click())
                }
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragOver(true);
                }}
                onDragLeave={() => setDragOver(false)}
                onDrop={onDrop}
              >
                <p style={{ margin: 0, fontWeight: 600 }}>Drop a manuscript, or click to choose</p>
                <p className="gds-upload__hint">{ACCEPT_HINT} · read on your device, never uploaded</p>
                <input
                  ref={inputRef}
                  type="file"
                  accept=".pdf,.docx,.txt,.md"
                  style={{ display: 'none' }}
                  data-testid="file-input"
                  onChange={(e) => e.target.files?.[0] && void acceptFile(e.target.files[0])}
                />
              </div>
            )}
            {error && <p className="gds-upload__error" role="alert" data-testid="upload-error">{error}</p>}
            <Link to="/app">← Back to Home</Link>
          </div>
        </Panel>
      </div>
    );
  }

  /* ------------------------------ THEATER ----------------------------- */
  const activeSectionLane =
    state.lanes.ai.status === 'running' ? state.lanes.ai : state.lanes.extraction;

  return (
    <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="theater-page">
      <Panel
        title={`Analyzing · ${selected?.name ?? 'manuscript'}`}
        actions={
          state.running ? (
            <Button variant="danger" onClick={abort} data-testid="abort">Abort</Button>
          ) : state.aborted ? (
            <Badge status="flagged">aborted</Badge>
          ) : state.complete ? (
            <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
              <Badge status="certain">complete</Badge>
              {state.reportId && (
                <Button
                  onClick={() => navigate(`/app/report?id=${encodeURIComponent(state.reportId!)}`)}
                  data-testid="view-report"
                >
                  View report →
                </Button>
              )}
            </div>
          ) : null
        }
      >
        <div className="gds-analysis" data-testid="theater">
          {activeSectionLane.status === 'running' && activeSectionLane.sectionTotal > 0 && (
            <span className="gds-lane__count" data-testid="section-progress">
              Analyzing section {activeSectionLane.sectionIndex}/{activeSectionLane.sectionTotal}
            </span>
          )}

          <div className="gds-theater">
            {/* left: the six agent lanes */}
            <div style={{ display: 'grid', gap: 12 }} data-testid="lanes">
              {STAGE_ORDER.map((stage) => (
                <AgentLane
                  key={stage}
                  lane={state.lanes[stage]}
                  debate={stage === 'verification' ? state.debate : undefined}
                  session={Boolean(session)}
                />
              ))}
            </div>

            {/* right: browse completed sections while later stages run */}
            <SectionBrowser sections={state.sections} />
          </div>
        </div>
      </Panel>
    </div>
  );
};

/* -------------------------------- lanes -------------------------------- */

const AgentLane: React.FC<{
  lane: LaneState;
  debate?: { speaker: string; text: string; verdict?: string }[];
  session: boolean;
}> = ({ lane, debate }) => {
  const pct =
    lane.status === 'done' || lane.status === 'locked'
      ? 100
      : lane.sectionTotal > 0
      ? Math.round((lane.sectionIndex / lane.sectionTotal) * 100)
      : lane.status === 'running'
      ? 40
      : 0;
  const aiData = lane.stage === 'ai' ? (lane.data as any) : null;

  return (
    <div className="gds-lane" data-status={lane.status} data-testid={`lane-${lane.stage}`}>
      <div className="gds-lane__head">
        <span className="gds-lane__name">{STAGE_LABEL[lane.stage]}</span>
        {lane.status === 'done' && <Badge status="certain">done</Badge>}
        {lane.status === 'running' && <Badge status="neutral">running</Badge>}
        {lane.status === 'locked' && <Badge status="assessed">locked ★</Badge>}
        {lane.status === 'error' && <Badge status="flagged">error</Badge>}
      </div>

      {lane.status !== 'locked' && (
        <div className="gds-lane__bar">
          <div className="gds-lane__fill" style={{ width: `${pct}%` }} />
        </div>
      )}

      {lane.summary && <span className="gds-lane__summary" data-testid={`summary-${lane.stage}`}>{lane.summary}</span>}
      {lane.sectionTotal > 0 && lane.status === 'running' && (
        <span className="gds-lane__count">section {lane.sectionIndex}/{lane.sectionTotal}</span>
      )}

      {/* AI lane: mandatory uncertainty disclaimer */}
      {lane.stage === 'ai' && aiData?.disclaimer && (
        <p className="gds-disclaimer" data-testid="ai-disclaimer">{aiData.disclaimer}</p>
      )}

      {/* Verification lane */}
      {lane.stage === 'verification' && lane.status === 'locked' && (
        <div className="gds-lane__locked" data-testid="verification-locked">
          <span className="gds-lane__summary">{lane.reason}</span>
          <Link to="/auth"><Button>Unlock PublishReady →</Button></Link>
        </div>
      )}
      {lane.stage === 'verification' && debate && debate.length > 0 && (
        <div className="gds-debate" data-testid="debate">
          {debate.map((t, i) => (
            <div key={i} className="gds-debate__turn">
              <span className="gds-debate__avatar" aria-hidden="true">
                {t.speaker.slice(0, 1)}
              </span>
              <div>
                <div className="gds-debate__speaker">
                  {t.speaker}
                  {t.verdict && <> · <Badge status={t.verdict === 'SUPPORTED' ? 'certain' : t.verdict === 'REFUTED' ? 'flagged' : 'assessed'}>{t.verdict}</Badge></>}
                </div>
                <div>{t.text}</div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

/* --------------------------- section browser --------------------------- */

const SectionBrowser: React.FC<{ sections: { title: string; risk?: number }[] }> = ({ sections }) => {
  const [current, setCurrent] = useState(0);
  const present = sections.filter(Boolean);
  return (
    <Card title={`Sections (${present.length})`} data-testid="section-browser">
      {present.length === 0 ? (
        <p className="gds-upload__hint">Sections appear here as they're parsed — browse them while later stages run.</p>
      ) : (
        <>
          <div className="gds-sections">
            {present.map((s, i) => (
              <button
                key={i}
                className="gds-section-item"
                aria-current={i === current}
                data-testid={`section-${i}`}
                onClick={() => setCurrent(i)}
              >
                <span>{s.title}</span>
                {s.risk !== undefined && (
                  <span style={{ color: riskColor(s.risk), fontSize: 11 }}>
                    {Math.round(s.risk * 100)}% AI-risk
                  </span>
                )}
              </button>
            ))}
          </div>
          <p className="gds-lane__summary" style={{ marginTop: 8 }} data-testid="current-section">
            Viewing: {present[current]?.title}
          </p>
        </>
      )}
    </Card>
  );
};

export default AnalysisTheaterPage;
