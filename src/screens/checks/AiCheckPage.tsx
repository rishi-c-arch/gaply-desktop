// Gaply — AI Check (Set 5). Local, free: the TWO-WAY tiered analysis —
// human-written vs AI-associated — over the run_aicheck command. 2-color
// in-document highlighting, the honest proportion, per-passage evidence, and
// the un-strippable cautions, all verbatim from the Rust core.
//
// Two-way is the Set-4 live-probe decision: no supported local model makes
// the AI-generated vs AI-paraphrased distinction reliably, so this page
// renders the paraphrase lane as honestly UNAVAILABLE (see AiCheckReport),
// never a fake third category.
//
// Keeps CheckScreen's scaffold shape (file-pick testids, shell) but renders
// its own report — the tiered wire doesn't fit the F6 PublishReadyReport.
import React, { useMemo, useRef, useState } from 'react';
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
import { AiCheckResult } from './agentTypes';
import { mayUseCloud } from '../settings/settingsStore';
import { readVerifyCitations, setVerifyCitations } from '../settings/settingsStore';

export interface AiCheckPageProps {
  bridge?: CheckBridge;
}

const AiCheckPage: React.FC<AiCheckPageProps> = ({ bridge }) => {
  const b = useMemo(() => bridge ?? new TauriCheckBridge(), [bridge]);
  const navigate = useNavigate();
  const [selected, setSelected] = useState<{ name: string; path: string } | null>(null);
  const [result, setResult] = useState<AiCheckResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // C2: the AI-Check-owned opt-in (default OFF, persisted). The network lane runs
  // only when this AND the global cloud gate are both on.
  const [verifyCitations, setVerify] = useState<boolean>(() => readVerifyCitations());
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
    setResult(null);
  };

  // Desktop: an ABSOLUTE path from the Tauri dialog (the real fix — the core can
  // open it). Size/pages are enforced by the core; validate the extension only.
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

  // Tauri → native picker (absolute path); browser/vitest → the hidden <input>.
  const pickFile = async () => {
    if (isTauri) {
      const p = await pickManuscriptPath(['pdf', 'docx'], 'Manuscript');
      if (p) acceptPath(p);
    } else {
      inputRef.current?.click();
    }
  };

  const runNow = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    // AND-gate: the AI-Check opt-in AND the global cloud consent for the suite.
    const doVerify = verifyCitations && mayUseCloud('citation_verification');
    try {
      setResult(await b.aicheck(selected.path, doVerify));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'check failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="ai-check">
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
            {!result ? (
              <>
                <Card title="Select a manuscript">
                  <p style={{ margin: '0 0 12px', color: 'var(--g-text-3)', fontSize: 13 }}>
                    Up to two analysis stages — Every manuscript receives a fast local pre-pass. On
                    devices with approximately 16 GB or more RAM, Gaply also performs an additional
                    local deep verification using the 7B model. Findings remain signals—not
                    verdicts—and your manuscript never leaves your device. Optionally, Gaply can check
                    your reference list's details — author, year, title, DOI (never your text) —
                    against public scholarly databases to flag citations it can't verify. · {ACCEPT_HINT}
                  </p>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
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
                      {busy ? 'Running…' : 'Run check'}
                    </Button>
                  </div>
                  <label style={{ display: 'flex', gap: 8, alignItems: 'flex-start', marginTop: 12, fontSize: 13, color: 'var(--g-text-2)', cursor: 'pointer' }}>
                    <input
                      type="checkbox"
                      checked={verifyCitations}
                      data-testid="verify-citations"
                      onChange={(e) => {
                        setVerify(e.target.checked);
                        setVerifyCitations(e.target.checked);
                      }}
                      style={{ marginTop: 2 }}
                    />
                    <span>
                      Verify references online
                      <span style={{ display: 'block', color: 'var(--g-text-3)', fontSize: 12 }}>
                        Sends only citation details (author, year, title, DOI) to CrossRef/OpenAlex — never your
                        manuscript. Off by default. You can turn this off anytime.
                      </span>
                    </span>
                  </label>
                  {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="check-error">{error}</p>}
                </Card>
                <Link to="/app">← Back to Home</Link>
              </>
            ) : (
              <div data-testid="check-report">
                <AiCheckReport result={result} />
                <div style={{ marginTop: 12 }}>
                  <Button variant="ghost" onClick={() => setResult(null)} data-testid="run-another">
                    ← Run another
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

export default AiCheckPage;
