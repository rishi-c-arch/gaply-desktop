// Gaply — home dashboard (F4): the app's hub. FREE-OFFLINE route: never
// requires a session; signed-out/offline users get a graceful local/empty
// state everywhere. Reuses F1 primitives throughout (rail, workspace, Card,
// ScoreRing, Badge) and the F2 metadata services (analysis_history is
// METADATA ONLY — title/date/tier counts; never manuscript content).
import React, { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import {
  AppShell,
  Badge,
  BadgeStatus,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  NavRail,
  Panel,
  ScoreRing,
  ThreePanelWorkspace,
} from '../../design-system';
import {
  createAnalysisHistoryService,
  createSubscriptionService,
  AnalysisHistoryRow,
  CertaintySummary,
} from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import '../auth/auth.css';
import './dashboard.css';

/* ------------------------------ rail items ------------------------------- */
/* ★ = paid flagship (teased for free/signed-out). Unbuilt destinations are
   every item now routes to a real screen; the one unbuilt destination ("My
   Manuscripts") goes to an honest /app/coming-soon rather than a dead link. */
const RAIL = [
  { id: 'home', label: 'Home', icon: '◫', to: '/app' },
  { id: 'manuscripts', label: 'My Manuscripts', icon: '≣', to: '/app/coming-soon?feature=My%20Manuscripts' },
  { id: 'publishready', label: 'PublishReady ★', icon: '✓', to: '/app/publishready' },
  { id: 'plagiarism', label: 'Plagiarism Check', icon: '≡', to: '/app/check/plagiarism' },
  { id: 'ai-check', label: 'AI Check', icon: '◬', to: '/app/check/ai' },
  { id: 'stats-check', label: 'Statistical Analysis Check', icon: 'Σ', to: '/app/check/stats' },
  { id: 'citations', label: 'Citation Manager', icon: '❞', to: '/app/citations' },
  { id: 'journal', label: 'Journal Check', icon: '◈', to: '/app/journal' },
  { id: 'copilot', label: 'Research Copilot ★', icon: '✦', to: '/app/copilot' },
  { id: 'coauthor', label: 'Research Co-Author', icon: '☍', to: '/app/community' },
  { id: 'settings', label: 'Settings', icon: '⚙', to: '/app/settings' },
];

/* --------------------- certainty summary → ring score --------------------- */

export function summaryToRing(s: CertaintySummary): { score: number; status: BadgeStatus } {
  const certain = s.certain ?? 0;
  const assessed = s.assessed ?? 0;
  const flagged = s.flagged ?? 0;
  const total = certain + assessed + flagged;
  const score = total > 0 ? Math.round((certain / total) * 100) : 0;
  const status: BadgeStatus = flagged > 0 ? 'flagged' : assessed > 0 ? 'assessed' : 'certain';
  return { score, status };
}

/* --------------------------- community placeholder ------------------------ */
/* Placeholder data until F13 builds the real Research Co-Author community. */
const PULSE: Array<{ id: string; title: string; author: string; badge: BadgeStatus }> = [
  { id: 'p1', title: 'Is n = 42 enough for a 2×2 mixed ANOVA?', author: 'Dr. Meera S.', badge: 'certain' },
  { id: 'p2', title: 'Reviewer asked for effect sizes — which one for chi-square?', author: 'A. Okafor', badge: 'assessed' },
  { id: 'p3', title: 'Co-author needed: scoping review on sleep & memory', author: 'J. Tanaka', badge: 'flagged' },
];

export interface HomeDashboardPageProps {
  /** Test seams — production uses the env-configured services. */
  historyService?: ReturnType<typeof createAnalysisHistoryService>;
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
}

const HomeDashboardPage: React.FC<HomeDashboardPageProps> = ({
  historyService,
  subscriptionService,
}) => {
  const { session } = useGaplySession();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const history = useMemo(() => historyService ?? createAnalysisHistoryService(), [historyService]);
  const subs = useMemo(() => subscriptionService ?? createSubscriptionService(), [subscriptionService]);

  const [recent, setRecent] = useState<AnalysisHistoryRow[] | null>(null);
  const [tier, setTier] = useState<'free' | 'premium'>('free');
  const [dragOver, setDragOver] = useState(false);

  // Honor onboarding's "Scan a sample manuscript" (?sample=1) → upload flow.
  useEffect(() => {
    if (searchParams.get('sample') === '1') {
      navigate('/app/upload?sample=1', { replace: true });
    }
  }, [searchParams, navigate]);

  // Recent manuscripts (metadata only) + tier — only with a session.
  useEffect(() => {
    let alive = true;
    if (!session) {
      setRecent(null);
      setTier('free');
      return;
    }
    history.list(session.user.id, 8).then((r) => alive && setRecent(r.data ?? []));
    subs.getTier(session.user.id).then((r) => alive && setTier(r.tier));
    return () => {
      alive = false;
    };
  }, [session, history, subs]);

  const onDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files?.[0];
    if (file) navigate('/app/upload', { state: { fileName: file.name } });
  };

  const showNudge = !session || tier === 'free';

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="home-dashboard">
      <AppShell
        rail={
          <NavRail
            items={RAIL.map((r) => ({
              id: r.id,
              label: r.label,
              icon: r.icon,
              onSelect: r.to ? () => navigate(r.to!) : undefined,
            }))}
            activeId="home"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Home">
            {session ? (
              tier === 'premium' ? <Badge status="certain">premium</Badge> : <Badge status="neutral">free</Badge>
            ) : (
              <Badge status="neutral">offline</Badge>
            )}
          </HeaderBar>
        }
      >
        <ThreePanelWorkspace
          inspector={
            <Panel title="Community pulse">
              <div className="gds-dash__pulse" data-testid="community-pulse">
                {PULSE.map((p) => (
                  <div key={p.id} className="gds-dash__pulse-item">
                    <span className="gds-dash__pulse-title">{p.title}</span>
                    <span className="gds-dash__pulse-meta">{p.author}</span>
                    <Badge status={p.badge}>{p.badge}</Badge>
                  </div>
                ))}
                <span className="gds-dash__recent-date">From Research Co-Author (preview)</span>
              </div>
            </Panel>
          }
        >
          <Panel title="Home">
            <div className="gds-dash">
              {!session && (
                <div className="gds-signin-nudge" data-testid="signin-nudge">
                  <span>Sign in to sync &amp; unlock online features — free tools run right here.</span>
                  <Link to="/auth">Sign in</Link>
                </div>
              )}

              {/* hero: globe as live integrity-status object */}
              <section className="gds-dash__hero" data-testid="dash-hero">
                <div className="gds-dash__hero-globe">
                  <GaplyGlobe scale="hero" />
                </div>
                <span className="gds-dash__hero-status">
                  {session ? 'integrity engine ready' : 'integrity engine ready — offline'}
                </span>
                <div className="gds-dash__hero-actions">
                  <Button onClick={() => navigate('/app/upload')} data-testid="start-analysis">
                    Start a new analysis
                  </Button>
                </div>
                <div
                  className="gds-dash__drop"
                  data-over={dragOver}
                  data-testid="dropzone"
                  onDragOver={(e) => {
                    e.preventDefault();
                    setDragOver(true);
                  }}
                  onDragLeave={() => setDragOver(false)}
                  onDrop={onDrop}
                >
                  …or drop a manuscript here (PDF, DOCX, TXT) — parsed on your device
                </div>
              </section>

              {/* quick actions */}
              <div className="gds-dash__quick" data-testid="quick-actions">
                <Button variant="secondary" onClick={() => navigate('/app/upload')}>New scan</Button>
                <Button variant="secondary" onClick={() => navigate('/app/journal')}>Check a journal</Button>
                <Button variant="secondary" onClick={() => navigate('/app/citations')}>Verify citations</Button>
                <Button variant="secondary" onClick={() => navigate('/app/copilot')}>Ask Copilot ★</Button>
              </div>

              {/* recent manuscripts (metadata only) */}
              <Card title="Recent manuscripts">
                {!session ? (
                  <p className="gds-dash__recent-date" data-testid="recent-empty">
                    Working locally — analyses stay on this device. Sign in to keep a synced
                    history (titles &amp; scores only, never your manuscript).
                  </p>
                ) : recent === null ? (
                  <p className="gds-dash__recent-date">Loading…</p>
                ) : recent.length === 0 ? (
                  <p className="gds-dash__recent-date" data-testid="recent-empty">
                    No analyses yet — upload your first manuscript to see it here.
                  </p>
                ) : (
                  <div className="gds-dash__recent" data-testid="recent-list">
                    {recent.map((row) => {
                      const ring = summaryToRing(row.certainty_summary_json);
                      return (
                        <Card key={row.id} className="gds-dash__recent-card" data-testid={`recent-${row.id}`}>
                          <ScoreRing size={48} strokeWidth={5} score={ring.score} status={ring.status} />
                          <div className="gds-dash__recent-meta">
                            <p className="gds-dash__recent-title">{row.title}</p>
                            <span className="gds-dash__recent-date">
                              {new Date(row.created_at).toLocaleDateString()}
                            </span>
                          </div>
                        </Card>
                      );
                    })}
                  </div>
                )}
              </Card>

              {/* ONE upgrade nudge — hidden for premium */}
              {showNudge && (
                <Card title="PublishReady ★" glass data-testid="upgrade-nudge">
                  <div className="gds-dash__nudge-letter" aria-hidden="true">
                    <p>Dear Dr. Sharma, we have completed the review of your manuscript…</p>
                    <p>The statistical reporting is thorough and the citations verify cleanly…</p>
                    <p>We are pleased to recommend acceptance with minor revisions…</p>
                    <div className="gds-dash__nudge-cta">
                      <Button onClick={() => navigate('/auth')}>
                        See if your paper will get published →
                      </Button>
                    </div>
                  </div>
                </Card>
              )}
            </div>
          </Panel>
        </ThreePanelWorkspace>
      </AppShell>
    </div>
  );
};

export default HomeDashboardPage;
