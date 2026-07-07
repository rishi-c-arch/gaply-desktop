// Gaply — home dashboard (minimal F3 landing; F4 builds it out). This route is
// FREE-OFFLINE: it never requires a session. With no session it shows only the
// free-offline features plus a subtle sign-in nudge.
import React from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  AppShell,
  Badge,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
  ThreePanelWorkspace,
} from '../../design-system';
import { useGaplySession } from '../session/SessionProvider';
import '../auth/auth.css';

const FREE_OFFLINE_FEATURES = [
  { id: 'extract', title: 'Manuscript extraction', hint: 'Sections, claims, citations — on device' },
  { id: 'validate', title: 'Statistical validation', hint: 'Deterministic 5-rule maths check' },
  { id: 'ai-detect', title: 'AI-signal scan', hint: 'Perplexity/burstiness heuristics' },
  { id: 'plagiarism', title: 'Self-plagiarism check', hint: 'Per-session local vector store' },
];

const ONLINE_FEATURES = [
  { id: 'verify', title: 'Cloud citation verification', hint: 'Claude via the private proxy' },
  { id: 'sync', title: 'History & library sync', hint: 'Metadata only, never manuscripts' },
];

const HomeDashboardPage: React.FC = () => {
  const { session } = useGaplySession();
  const navigate = useNavigate();

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="home-dashboard">
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'design', label: 'Design', icon: '✦', onSelect: () => navigate('/design') },
            ]}
            activeId="home"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Home">
            {session ? (
              <Badge status="certain">signed in</Badge>
            ) : (
              <Badge status="neutral">offline</Badge>
            )}
          </HeaderBar>
        }
      >
        <ThreePanelWorkspace>
          <Panel title="Your workspace">
            <div style={{ display: 'grid', gap: 16 }}>
              {!session && (
                <div className="gds-signin-nudge" data-testid="signin-nudge">
                  <span>
                    Sign in to sync &amp; unlock online features — free-offline tools work right
                    here, no account needed.
                  </span>
                  <Link to="/auth">Sign in</Link>
                </div>
              )}

              <Card title="Free — runs fully on your device">
                <div style={{ display: 'grid', gap: 8 }}>
                  {FREE_OFFLINE_FEATURES.map((f) => (
                    <div key={f.id} data-testid={`feature-${f.id}`}>
                      <strong>{f.title}</strong>
                      <div style={{ color: 'var(--g-text-3)', fontSize: 13 }}>{f.hint}</div>
                    </div>
                  ))}
                </div>
              </Card>

              <Card title="Online features">
                <div style={{ display: 'grid', gap: 8 }}>
                  {ONLINE_FEATURES.map((f) => (
                    <div key={f.id} style={{ opacity: session ? 1 : 0.5 }} data-testid={`feature-${f.id}`}>
                      <strong>{f.title}</strong>{' '}
                      {!session && <Badge status="neutral">sign in</Badge>}
                      <div style={{ color: 'var(--g-text-3)', fontSize: 13 }}>{f.hint}</div>
                    </div>
                  ))}
                </div>
              </Card>
            </div>
          </Panel>
        </ThreePanelWorkspace>
      </AppShell>
    </div>
  );
};

export default HomeDashboardPage;
