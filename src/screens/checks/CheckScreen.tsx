// Gaply — shared scaffold for the three single-agent check screens (F7). Each
// is: an upload/select entry → a run action → the F6 viewer scoped to that
// agent. Fully local & free; the manuscript never leaves the device.
import React, { useRef, useState } from 'react';
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
import ReportViewerPage from '../report/ReportViewerPage';
import { PublishReadyReport, ReportTab } from '../report/reportTypes';
import { ACCEPT_HINT, estimatePdfPageCount, validateFile } from '../analysis/validateFile';

export interface CheckScreenProps {
  title: string;
  subtitle: string;
  /** Which viewer tabs to show (scopes the F6 viewer to this agent). */
  tabs: ReportTab[];
  /** Run the agent for a selected file path → a scoped report. */
  run: (path: string, title?: string) => Promise<PublishReadyReport>;
  /** Optional extra panel (e.g. the plagiarism deep-check tease). */
  extra?: React.ReactNode;
  testid: string;
}

const CheckScreen: React.FC<CheckScreenProps> = ({ title, subtitle, tabs, run, extra, testid }) => {
  const navigate = useNavigate();
  const [selected, setSelected] = useState<{ name: string; path: string } | null>(null);
  const [report, setReport] = useState<PublishReadyReport | null>(null);
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
    setReport(null);
  };

  const runNow = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      setReport(await run(selected.path, selected.name));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'check failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid={testid}>
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'plag', label: 'Plagiarism Check', icon: '≡', onSelect: () => navigate('/app/check/plagiarism') },
              { id: 'ai', label: 'AI Check', icon: '◬', onSelect: () => navigate('/app/check/ai') },
              { id: 'stats', label: 'Statistical Analysis Check', icon: 'Σ', onSelect: () => navigate('/app/check/stats') },
            ]}
            activeId=""
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title={title}>
            <Badge status="certain">local · free</Badge>
          </HeaderBar>
        }
      >
        <Panel title={title}>
          <div style={{ display: 'grid', gap: 16 }}>
            {!report ? (
              <>
                <Card title="Select a manuscript">
                  <p style={{ margin: '0 0 12px', color: 'var(--g-text-3)', fontSize: 13 }}>
                    {subtitle} · {ACCEPT_HINT} · parsed on your device, never uploaded.
                  </p>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                    <Button variant="secondary" onClick={() => inputRef.current?.click()} data-testid="pick-file">
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
                  {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="check-error">{error}</p>}
                </Card>
                {extra}
                <Link to="/app">← Back to Home</Link>
              </>
            ) : (
              <div data-testid="check-report">
                <ReportViewerPage report={report} tabs={tabs} bare />
                <div style={{ marginTop: 12 }}>
                  <Button variant="ghost" onClick={() => setReport(null)} data-testid="run-another">
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

export default CheckScreen;
