// Gaply — PublishReady (F10), the paid flagship. Upload manuscript + target
// journal → full simulated peer review with a publish/no-publish verdict. The
// manuscript stays on device; only the structured payload leaves (via proxy).
// Gated behind premium (F12): free users get a blurred teaser + upgrade CTA.
import React, { useMemo, useState } from 'react';
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
import { ToastProvider, useToast } from '../../design-system/Toast';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import ReportViewerPage from '../report/ReportViewerPage';
import { downloadReportPdf } from '../report/exportPdf';
import { ReportTab } from '../report/reportTypes';
import { estimatePdfPageCount, validateFile } from '../analysis/validateFile';
import { JOURNALS } from '../journal/journalData';
import { PublishReadyBridge, TauriPublishReadyBridge } from './publishReadyBridge';
import { mayUseCloud } from '../settings/settingsStore';
import { PublishReadyResult, TargetJournal } from './publishReadyTypes';
import ReviewerLetterPanel from './ReviewerLetterPanel';
import ResearchCopilotPanel from '../copilot/ResearchCopilotPanel';
import { ChatClient, ProxyChatClient } from '../copilot/chatBridge';
import './publishready.css';
import '../journal/journal.css'; // reuse .gds-jc__input / __disclaimer

const PR_TABS: ReportTab[] = ['Overview', 'Statistics', 'Citations', 'AI Risk', 'Plagiarism', 'Checklist', 'Reviewer Letter' as ReportTab];

export interface PublishReadyPageProps {
  bridge?: PublishReadyBridge;
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
  /** Test seam: force premium/free instead of fetching. */
  forceTier?: 'free' | 'premium';
  /** Research Copilot chat client for the docked panel (proxy in prod). */
  chatClient?: ChatClient;
}

/** Collapsible Research Copilot dock, attached to the PublishReady report. */
const CopilotDock: React.FC<{ report: PublishReadyResult['report']; client: ChatClient }> = ({ report, client }) => {
  const [open, setOpen] = useState(false);
  return (
    <div style={{ borderTop: '1px solid var(--g-border)', background: 'var(--g-bg-layer1)' }} data-testid="pr-copilot-dock">
      <button
        onClick={() => setOpen((o) => !o)}
        data-testid="pr-copilot-toggle"
        style={{ width: '100%', textAlign: 'left', padding: '10px 16px', background: 'none', border: 'none', color: 'var(--g-text-2)', cursor: 'pointer', fontWeight: 600 }}
      >
        {open ? '▾' : '▸'} Research Copilot ★ — ask about this review
      </button>
      {open && (
        <div style={{ height: 360, padding: '0 16px 16px' }}>
          <ResearchCopilotPanel
            context={{ report, ragSnippets: [], citations: [] }}
            client={client}
          />
        </div>
      )}
    </div>
  );
};

const Inner: React.FC<PublishReadyPageProps> = ({ bridge, subscriptionService, forceTier, chatClient }) => {
  const copilot = useMemo(() => chatClient ?? new ProxyChatClient('/proxy'), [chatClient]);
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const { toast } = useToast();
  const b = useMemo(() => bridge ?? new TauriPublishReadyBridge(), [bridge]);
  const subs = useMemo(() => subscriptionService ?? createSubscriptionService(), [subscriptionService]);

  const [tier, setTier] = useState<'free' | 'premium' | 'loading'>(forceTier ?? 'loading');
  const [file, setFile] = useState<{ name: string; path: string } | null>(null);
  const [journalQuery, setJournalQuery] = useState('');
  const [journal, setJournal] = useState<TargetJournal | null>(null);
  const [result, setResult] = useState<PublishReadyResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // resolve tier (premium gate)
  React.useEffect(() => {
    if (forceTier) return;
    if (!session) { setTier('free'); return; }
    subs.getTier(session.user.id).then((r) => setTier(r.tier));
  }, [forceTier, session, subs]);

  const journalMatches = useMemo(() => {
    const q = journalQuery.trim().toLowerCase();
    if (!q) return [];
    return JOURNALS.filter((j) => j.name.toLowerCase().includes(q)).slice(0, 6);
  }, [journalQuery]);

  const acceptFile = async (f: File) => {
    setError(null);
    const pageCount = await estimatePdfPageCount(f);
    const res = validateFile({ name: f.name, sizeBytes: f.size, pageCount });
    if (!res.ok) { setError(res.error); return; }
    setFile({ name: f.name, path: (f as any).path ?? f.name });
  };

  const run = async () => {
    if (!file || !journal) return;
    // F14 privacy gate — the review round-trip goes through the proxy; consent
    // off means the run never starts.
    if (!mayUseCloud('publishready')) {
      setError('PublishReady’s cloud access is turned off in Settings → Sync & Privacy.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setResult(await b.run({ manuscriptPath: file.path, journal }));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'PublishReady failed');
    } finally {
      setBusy(false);
    }
  };

  /* ------------------------------- teaser ------------------------------- */
  if (tier === 'free') {
    return (
      <Shell navigate={navigate}>
        <div className="gds-pr-entry" data-testid="pr-teaser">
          <Card title="PublishReady ★ — full simulated peer review">
            <div className="gds-pr-teaser">
              <div className="gds-pr-teaser__blur" aria-hidden="true">
                <div className="gds-pr__head">
                  <div className="gds-pr__verdict" data-status="assessed">MAJOR REVISION</div>
                  <div className="gds-pr__gauge"><span className="gds-pr__gauge-label">61% publication probability</span></div>
                </div>
                <p className="gds-pr__body">Dear Author, we have completed a full review of your manuscript against the target journal…</p>
              </div>
              <div className="gds-pr-teaser__cta" data-testid="pr-unlock">
                <div>
                  <Badge status="neutral">Premium</Badge>
                  <strong>See the reviewer letter, verdict &amp; publication probability.</strong>
                  <Button onClick={() => navigate('/app/billing')}>Unlock the verdict →</Button>
                </div>
              </div>
            </div>
          </Card>
        </div>
      </Shell>
    );
  }

  if (tier === 'loading') {
    return <Shell navigate={navigate}><p className="gds-jc__disclaimer" data-testid="pr-loading">Checking your plan…</p></Shell>;
  }

  /* ------------------------------- result ------------------------------- */
  if (result) {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="publishready">
        <AppShell
          rail={<PRRail navigate={navigate} />}
          header={
            <HeaderBar title="PublishReady ★">
              <Badge status="certain">premium</Badge>
              <Button
                variant="secondary"
                data-testid="pr-export"
                onClick={() =>
                  void downloadReportPdf(result.report, 'publishready-report.pdf')
                    .then(() => toast('PublishReady PDF exported', 'certain'))
                    .catch(() => toast('Export failed', 'flagged'))
                }
              >
                Export PublishReady PDF
              </Button>
            </HeaderBar>
          }
        >
          <div style={{ display: 'grid', gridTemplateRows: '1fr auto', height: '100%', minHeight: 0 }}>
            <div style={{ minHeight: 0, overflow: 'auto' }}>
              <ReportViewerPage
                report={result.report}
                tabs={PR_TABS}
                bare
                reviewerLetter={<ReviewerLetterPanel letter={result.reviewerLetter} />}
              />
            </div>
            <CopilotDock report={result.report} client={copilot} />
          </div>
        </AppShell>
      </div>
    );
  }

  /* ------------------------------- entry -------------------------------- */
  return (
    <Shell navigate={navigate}>
      <div className="gds-pr-entry" data-testid="pr-entry">
        <Card title="1 · Choose your manuscript">
          <input type="file" accept=".pdf,.docx" data-testid="pr-file" onChange={(e) => e.target.files?.[0] && void acceptFile(e.target.files[0])} />
          {file && <span className="gds-mono" style={{ marginLeft: 8 }}>{file.name}</span>}
          {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} data-testid="pr-error">{error}</p>}
        </Card>

        <Card title="2 · Choose the target journal (with its quartile)">
          <input
            className="gds-jc__input"
            style={{ width: '100%' }}
            placeholder="Search a journal from Journal Check…"
            value={journalQuery}
            data-testid="pr-journal-input"
            onChange={(e) => setJournalQuery(e.target.value)}
          />
          {journal ? (
            <p data-testid="pr-journal-picked" style={{ marginTop: 8 }}>
              Target: <strong>{journal.name}</strong> <Badge status="neutral">{journal.quartile}</Badge>
            </p>
          ) : (
            <div style={{ display: 'grid', gap: 4, marginTop: 8 }}>
              {journalMatches.map((j) => (
                <button
                  key={j.name}
                  className="gds-pr__alt"
                  data-testid={`pr-journal-${j.name}`}
                  onClick={() => setJournal({ name: j.name, quartile: j.quartile ?? 'Q4' })}
                >
                  <span>{j.name}</span><Badge status="neutral">{j.quartile ?? '—'}</Badge>
                </button>
              ))}
            </div>
          )}
        </Card>

        <Button onClick={run} disabled={!file || !journal || busy} data-testid="pr-run">
          {busy ? 'Reviewing…' : 'Run PublishReady review'}
        </Button>
        <p className="gds-jc__disclaimer">
          Your manuscript stays on this device — only structured findings (not text) go to the cloud.
        </p>
      </div>
    </Shell>
  );
};

const PRRail: React.FC<{ navigate: (p: string) => void }> = ({ navigate }) => (
  <NavRail
    items={[
      { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
      { id: 'pr', label: 'PublishReady ★', icon: '✓' },
    ]}
    activeId="pr"
    brand={<GaplyGlobe scale="mark" />}
  />
);

const Shell: React.FC<{ navigate: (p: string) => void; children: React.ReactNode }> = ({ navigate, children }) => (
  <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="publishready">
    <AppShell rail={<PRRail navigate={navigate} />} header={<HeaderBar title="PublishReady ★" />}>
      <Panel title="PublishReady — full simulated peer review">{children}</Panel>
    </AppShell>
  </div>
);

const PublishReadyPage: React.FC<PublishReadyPageProps> = (props) => (
  <ToastProvider>
    <Inner {...props} />
  </ToastProvider>
);

export default PublishReadyPage;
