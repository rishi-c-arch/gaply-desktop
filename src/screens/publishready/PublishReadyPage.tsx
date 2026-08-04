// Gaply — PublishReady (F10), the paid flagship. Upload manuscript + target
// journal → full simulated peer review with a publish/no-publish verdict. The
// manuscript stays on device; only the structured payload leaves (via proxy).
// Gated behind premium (F12): free users get a blurred teaser + upgrade CTA.
import React, { useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { pickManuscriptPath, basenameOf } from '../common/pickFile';
import { isTauri } from '../../utils/isTauri';
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
import { useEntitlement } from '../subscription/entitlement';
import { mayUseCloud } from '../settings/settingsStore';
import { PublishReadyResult, TargetJournal } from './publishReadyTypes';
import ReviewerLetterPanel from './ReviewerLetterPanel';
import ResearchCopilotPanel from '../copilot/ResearchCopilotPanel';
import { ChatClient, TauriChatClient } from '../copilot/chatBridge';
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
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const copilot = useMemo(
    () => chatClient ?? new TauriChatClient(() => session?.access_token),
    [chatClient, session]
  );
  const { toast } = useToast();
  const b = useMemo(() => bridge ?? new TauriPublishReadyBridge(), [bridge]);

  // Entitlement gate (Set 8) — UX-ONLY presentation. THE REAL gate is
  // server-side at the proxy (the user's JWT rides every paid request and the
  // server verifies + consumes uses). `forceTier` is the existing test seam,
  // mapped onto entitlement states. No prices anywhere: the answer is only
  // ever "entitled or not"; the upgrade CTA links out to billing/payment.
  const entitlement = useEntitlement(
    'publishready',
    subscriptionService,
    forceTier ? (forceTier === 'premium' ? 'entitled' : 'not_entitled') : undefined
  );

  const [file, setFile] = useState<{ name: string; path: string } | null>(null);
  const prInputRef = useRef<HTMLInputElement>(null);
  const [journalQuery, setJournalQuery] = useState('');
  const [journal, setJournal] = useState<TargetJournal | null>(null);
  // H4: the target journal's author-guidelines URL, supplied by the user — NOT
  // prefilled (§18.6.2: the directory carries websites, not guidelines pages).
  // `guidelinesNote` surfaces the honest ingest outcome.
  const [guidelinesUrl, setGuidelinesUrl] = useState('');
  const [guidelinesNote, setGuidelinesNote] = useState<string | null>(null);
  // The URL the COMPLETED run actually used. Held separately from the input
  // above, which the user may edit after a run: the checklist's empty state
  // describes the run that produced the report, not the current form state.
  const [ranGuidelinesUrl, setRanGuidelinesUrl] = useState<string | null>(null);
  const [result, setResult] = useState<PublishReadyResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  // Tauri desktop: an ABSOLUTE path from the dialog. No File object, so skip the
  // page-count validation (validate sizeBytes:1) — exactly as AiCheckPage.acceptPath;
  // behavior matches a working upload.
  const acceptPath = (path: string) => {
    setError(null);
    const res = validateFile({ name: basenameOf(path), sizeBytes: 1 });
    if (!res.ok) { setError(res.error); return; }
    setFile({ name: basenameOf(path), path });
  };
  const pickManuscript = async () => {
    const p = await pickManuscriptPath(['pdf', 'docx'], 'Manuscript');
    if (p) acceptPath(p);
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
    setGuidelinesNote(null);
    try {
      // H4: populate the journal-guideline corpus FIRST (local fetch, behind the
      // same cloud consent), so the pipeline's checklist cross-references the real
      // guidelines the user pointed at. Best-effort + honest: an unreachable or
      // quarantined page just leaves the checklist structural-only (Set 1's honest
      // degradation) — it NEVER blocks the review.
      const url = guidelinesUrl.trim();
      setRanGuidelinesUrl(url || null);
      if (url) {
        try {
          const ing = await b.ingestGuidelines({ guidelinesUrl: url });
          setGuidelinesNote(ing.note);
        } catch {
          setGuidelinesNote(
            'Could not fetch that guidelines page — the checklist will show structural checks only.'
          );
        }
      }
      // The session's access token rides to the backend → proxy, where the
      // REAL entitlement check + server-side use consumption happen.
      setResult(
        await b.run({
          manuscriptPath: file.path,
          journal,
          userToken: session?.access_token,
          // Only the guidelines the user actually supplied scope the checklist.
          guidelinesUrl: url || undefined,
        })
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : 'PublishReady failed');
    } finally {
      setBusy(false);
    }
  };

  /* --------------------------- entitlement UX --------------------------- */
  // All of these are PRESENTATION: honest, distinct states — never a silent
  // failure, never a fake result, and never a price.
  if (entitlement.status === 'signed_out') {
    return (
      <Shell navigate={navigate}>
        <div className="gds-pr-entry" data-testid="pr-signin">
          <Card title="PublishReady ★ — sign in required">
            <p className="gds-jc__disclaimer">
              Gaply needs you signed in to use its features. Your manuscript still never leaves
              this device — sign-in only identifies your account and plan.
            </p>
            <Button data-testid="pr-signin-cta" onClick={() => navigate('/auth')}>Sign in to continue →</Button>
          </Card>
        </div>
      </Shell>
    );
  }

  if (entitlement.status === 'offline_unverified') {
    return (
      <Shell navigate={navigate}>
        <div className="gds-pr-entry" data-testid="pr-offline">
          <Card title="PublishReady ★ — can’t verify your plan right now">
            <p className="gds-jc__disclaimer">
              You’re signed in, but {entitlement.reason ?? 'the server is unreachable'}. PublishReady’s
              review needs the cloud connection anyway, so try again once you’re back online. Your
              offline tools keep working as usual.
            </p>
          </Card>
        </div>
      </Shell>
    );
  }

  if (entitlement.status === 'not_entitled') {
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

  if (entitlement.status === 'checking') {
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
                guidelinesUrl={ranGuidelinesUrl ?? undefined}
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
          <Button variant="secondary" data-testid="pr-pick" onClick={() => (isTauri ? void pickManuscript() : prInputRef.current?.click())}>
            Choose file
          </Button>
          <input ref={prInputRef} type="file" accept=".pdf,.docx" style={{ display: 'none' }} data-testid="pr-file" onChange={(e) => e.target.files?.[0] && void acceptFile(e.target.files[0])} />
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
                  onClick={() => {
                    // NO guidelines-URL prefill. The bundled directory carries a
                    // journal WEBSITE, not an author-guidelines page — 258 of 258
                    // entries, 0 guideline URLs — so prefilling put a homepage in
                    // the field. A homepage ingests successfully, reports a
                    // plausible chunk count, and yields a checklist identical to
                    // the blank case: a silent failure wearing the appearance of
                    // a working feature. An empty field fails visibly instead.
                    // ARCHITECTURE_TRACE §18.6.2.
                    setJournal({ name: j.name, quartile: j.quartile ?? 'Q4' });
                  }}
                >
                  <span>{j.name}</span><Badge status="neutral">{j.quartile ?? '—'}</Badge>
                </button>
              ))}
            </div>
          )}
        </Card>

        <Card title="3 · Target journal guidelines (optional)">
          <input
            className="gds-jc__input"
            style={{ width: '100%' }}
            placeholder="Paste your target journal's author-guidelines URL…"
            value={guidelinesUrl}
            data-testid="pr-guidelines-input"
            onChange={(e) => setGuidelinesUrl(e.target.value)}
          />
          <p className="gds-jc__disclaimer">
            Optional. Gaply fetches this page and cross-references your manuscript against the
            real guidelines — word limit, structured abstract, conflict-of-interest, reference
            style (deterministic checks over the actual page, no guessing). Leave blank to run
            the structural checks only.
          </p>
          {guidelinesNote && (
            <p className="gds-jc__disclaimer" data-testid="pr-guidelines-note">{guidelinesNote}</p>
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
