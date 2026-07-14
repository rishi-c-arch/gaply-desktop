// Gaply — Plagiarism Check (F7). Local, free. TWO honest lanes:
//   · PRIMARY "Exact text matches" (Set 2/3): deterministic verbatim overlaps
//     against the user's curated "my papers" library — real matches shown
//     side-by-side in both documents, with a named-scope, un-strippable
//     disclosure (NOT a Turnitin replacement).
//   · SECONDARY "Similar meaning" (existing embedding lane, unchanged): the
//     on-device semantic-similarity scan. Kept as-is; clearly labeled so the
//     two don't confuse — the exact-match lane is the default.
// The deep comprehensive check remains an honest coming-soon (no vendor, no
// fabricated score). Path-only over IPC; text never leaves the device.
import React, { useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
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
import ReportViewerPage from '../report/ReportViewerPage';
import { PublishReadyReport } from '../report/reportTypes';
import { useFeatureFlag } from '../../config/Feature';
import { CheckBridge, TauriCheckBridge } from './checkBridge';
import { plagiarismToReport } from './adapters';
import { ExactPlagiarismReport, PlagiarismReport } from './agentTypes';
import PlagiarismExactReport from './PlagiarismExactReport';
import PlagiarismLibraryManager from './PlagiarismLibraryManager';

export interface PlagiarismCheckPageProps {
  bridge?: CheckBridge;
}

type Mode = 'exact' | 'similar';

const PlagiarismCheckPage: React.FC<PlagiarismCheckPageProps> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  const navigate = useNavigate();
  const deepEnabled = useFeatureFlag('deepPlagiarism');

  const [mode, setMode] = useState<Mode>('exact');
  const [exact, setExact] = useState<ExactPlagiarismReport | null>(null);
  const [sim, setSim] = useState<PublishReadyReport | null>(null);
  // Raw semantic report kept alongside the adapted one, so we can honestly flag
  // an EMPTY corpus-match result (M1: there's no external corpus configured yet —
  // an empty list must not read as "verified clean").
  const [simRaw, setSimRaw] = useState<PlagiarismReport | null>(null);
  const [selected, setSelected] = useState<{ name: string; path: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const acceptFile = async (file: File) => {
    setError(null);
    const pageCount = await estimatePdfPageCount(file);
    const res = validateFile({ name: file.name, sizeBytes: file.size, pageCount });
    if (!res.ok) {
      setError(res.error);
      return;
    }
    setSelected({ name: file.name, path: (file as any).path ?? file.name });
  };

  // Desktop: an ABSOLUTE path from the Tauri dialog (the real fix). Size/pages
  // are enforced by the core; validate the extension only.
  const acceptPath = (path: string) => {
    setError(null);
    const name = basenameOf(path);
    const res = validateFile({ name, sizeBytes: 1 });
    if (!res.ok) {
      setError(res.error);
      return;
    }
    setSelected({ name, path });
  };

  // Tauri → native picker (absolute path); browser/vitest → the hidden <input>.
  const pickFile = async () => {
    if (isTauri) {
      const p = await pickManuscriptPath(['pdf', 'docx', 'txt', 'md'], 'Manuscript');
      if (p) acceptPath(p);
    } else {
      inputRef.current?.click();
    }
  };

  const runNow = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      if (mode === 'exact') {
        setExact(await b.checkExact(selected.path));
      } else {
        const raw = await b.plagiarism(selected.path);
        setSimRaw(raw);
        setSim(plagiarismToReport(raw));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'check failed');
    } finally {
      setBusy(false);
    }
  };

  const report = mode === 'exact' ? exact : sim;
  const reset = () => {
    if (mode === 'exact') setExact(null);
    else {
      setSim(null);
      setSimRaw(null);
    }
  };
  const switchMode = (m: Mode) => {
    setMode(m);
    setError(null);
    setSelected(null);
  };

  const deepPanel = (
    <Card title="Deep plagiarism analysis" glass data-testid="deep-check">
      <p style={{ margin: '0 0 8px', color: 'var(--g-text-2)', fontSize: 13 }}>
        Broader coverage beyond your own documents. <Badge status="neutral">coming soon</Badge>
      </p>
      <p style={{ margin: 0, color: 'var(--g-text-3)', fontSize: 12 }} data-testid="deep-note">
        Deep plagiarism analysis is coming soon. Local, on-device checks are available now.
      </p>
      {deepEnabled && (
        <Button variant="secondary" disabled style={{ marginTop: 8 }} data-testid="run-deep">
          Run deep analysis (coming soon)
        </Button>
      )}
    </Card>
  );

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="plagiarism-check">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'plag', label: 'Plagiarism Check', icon: '≡', onSelect: () => navigate('/app/check/plagiarism') },
              { id: 'ai', label: 'AI Check', icon: '◬', onSelect: () => navigate('/app/check/ai') },
              { id: 'stats', label: 'Statistical Analysis Check', icon: 'Σ', onSelect: () => navigate('/app/check/stats') },
            ]}
            activeId="plag"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Plagiarism Check">
            <Badge status="certain">local · free</Badge>
          </HeaderBar>
        }
      >
        <Panel title="Plagiarism Check">
          <div style={{ display: 'grid', gap: 16 }}>
            {/* lane toggle — exact is the default */}
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }} data-testid="plag-mode">
              <Button
                variant={mode === 'exact' ? 'primary' : 'secondary'}
                onClick={() => switchMode('exact')}
                data-testid="mode-exact"
              >
                Exact text matches
              </Button>
              <Button
                variant={mode === 'similar' ? 'primary' : 'secondary'}
                onClick={() => switchMode('similar')}
                data-testid="mode-similar"
              >
                Similar meaning (semantic)
              </Button>
            </div>

            {/* the "my papers" library — only relevant to the exact lane */}
            {mode === 'exact' && <PlagiarismLibraryManager bridge={b} />}

            {!report ? (
              <>
                <Card title="Check a document">
                  <p style={{ margin: '0 0 12px', color: 'var(--g-text-3)', fontSize: 13 }}>
                    {mode === 'exact'
                      ? 'Find verbatim text your document shares with itself or your library — real overlapping passages.'
                      : 'Find passages with similar MEANING (semantic), even when reworded — a softer, fuzzier signal.'}{' '}
                    · {ACCEPT_HINT} · parsed on your device.
                  </p>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
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
                    <Button onClick={runNow} disabled={!selected || busy} data-testid="run-check">
                      {busy ? 'Running…' : 'Run check'}
                    </Button>
                  </div>
                  {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="check-error">{error}</p>}
                </Card>
                {deepPanel}
              </>
            ) : (
              <div data-testid="check-report">
                {mode === 'exact' ? (
                  <PlagiarismExactReport result={exact!} />
                ) : (
                  <>
                    <ReportViewerPage report={sim!} tabs={['Overview', 'Plagiarism']} bare />
                    {simRaw && simRaw.corpus_matches.length === 0 && (
                      <p
                        style={{ marginTop: 12, color: 'var(--g-text-2)', fontSize: 13 }}
                        data-testid="plag-no-corpus-note"
                      >
                        No external plagiarism corpus is currently configured, so the semantic lane
                        compared your manuscript only against itself. An empty result here does{' '}
                        <strong>not</strong> mean the text is original — for verbatim overlap against
                        sources you choose, use “Exact text matches” against your reference library.
                      </p>
                    )}
                  </>
                )}
                <div style={{ marginTop: 12 }}>
                  <Button variant="ghost" onClick={reset} data-testid="run-another">
                    ← Check another
                  </Button>
                </div>
              </div>
            )}
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

export default PlagiarismCheckPage;
