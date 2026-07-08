// Gaply — LIVE report route (/app/report). Loads the REAL compiled report by
// id (the report_id from AnalysisEvent::Finished, cached as report:{id}), via
// the get_report Tauri command. The SAMPLE fixture is NEVER used here — with no
// id, or before a report loads, it shows an honest empty/loading state.
import React, { useEffect, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { AppShell, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { isTauri } from '../../utils/isTauri';
import ReportViewerPage from './ReportViewerPage';
import type { PublishReadyReport } from './reportTypes';

type LoadState =
  | { phase: 'idle' }
  | { phase: 'loading' }
  | { phase: 'loaded'; report: PublishReadyReport }
  | { phase: 'error'; message: string };

const Shell: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const navigate = useNavigate();
  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="report-page">
      <AppShell
        rail={
          <NavRail
            items={[{ id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') }]}
            activeId="home"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title="Integrity report" />}
      >
        {children}
      </AppShell>
    </div>
  );
};

const LiveReportPage: React.FC = () => {
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const id = params.get('id');
  const [state, setState] = useState<LoadState>({ phase: 'idle' });

  useEffect(() => {
    if (!id) {
      setState({ phase: 'idle' });
      return;
    }
    if (!isTauri) {
      // The real report lives in the desktop core; there's nothing to load in a
      // plain browser. Be honest rather than showing a fixture.
      setState({ phase: 'error', message: 'Reports are available in the Gaply desktop app.' });
      return;
    }
    let alive = true;
    setState({ phase: 'loading' });
    (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const report = (await invoke('get_report', { reportId: id })) as PublishReadyReport;
        if (alive) setState({ phase: 'loaded', report });
      } catch (e) {
        if (alive) setState({ phase: 'error', message: e instanceof Error ? e.message : 'Could not load the report.' });
      }
    })();
    return () => {
      alive = false;
    };
  }, [id]);

  if (state.phase === 'loaded') {
    // Hand the REAL compiled report to the viewer (no sample fixture).
    return <ReportViewerPage report={state.report} />;
  }

  return (
    <Shell>
      <Panel title="Integrity report">
        {state.phase === 'idle' ? (
          <Card title="No report yet" data-testid="report-empty">
            <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>
              Run an analysis to generate a report. It will open here automatically when it finishes.
            </p>
            <Button onClick={() => navigate('/app/upload')} data-testid="report-empty-scan">
              Start a new analysis
            </Button>
          </Card>
        ) : state.phase === 'loading' ? (
          <p style={{ color: 'var(--g-text-3)', fontSize: 14 }} data-testid="report-loading">Loading report…</p>
        ) : (
          <Card title="Report unavailable" data-testid="report-error">
            <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>{state.message}</p>
            <Button onClick={() => navigate('/app/upload')}>Start a new analysis</Button>
          </Card>
        )}
      </Panel>
    </Shell>
  );
};

export default LiveReportPage;
