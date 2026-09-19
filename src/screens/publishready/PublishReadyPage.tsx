// Gaply — PublishReady (F10), the paid flagship. Upload manuscript + target
// journal → full simulated peer review with a publish/no-publish verdict. The
// manuscript stays on device; only the structured payload leaves (via proxy).
// Gated behind premium (F12): free users get a blurred teaser + upgrade CTA.
import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { pickManuscriptPath, pickManuscriptPaths, basenameOf } from '../common/pickFile';
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
import { tauriJournalFingerprintBridge } from './journal/journalFingerprintBridge';
import { JournalProfileRow } from './journal/fingerprintTypes';
import { AnalysisFileReport, ANALYSIS_EXTENSIONS, PublishReadyBridge, TauriPublishReadyBridge } from './publishReadyBridge';
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
  // **The analysis upload is an INSTRUMENT before it is a feature. §11 D191.**
  // §1 calls supporting analysis "the differentiator" and `[v7]` records that no
  // picker has ever accepted one. This is that input — and it deliberately
  // accepts formats Gaply cannot parse, because the count of what arrives is
  // what decides whether those parsers are worth building. The report is
  // rendered verbatim: a file uploaded and silently ignored is worse than one
  // refused.
  const [analysisNote, setAnalysisNote] = useState<string | null>(null);
  const [analysisRows, setAnalysisRows] = useState<AnalysisFileReport[]>([]);
  // The URL the COMPLETED run actually used. Held separately from the input
  // above, which the user may edit after a run: the checklist's empty state
  // describes the run that produced the report, not the current form state.
  const [ranGuidelinesUrl, setRanGuidelinesUrl] = useState<string | null>(null);
  const [result, setResult] = useState<PublishReadyResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // BLOCKER 6 — what we already know about the reviewer letter, before the run.
  // The panel's honest "unavailable offline" state only ever appeared AFTER a
  // full analysis; a user with no proxy configured paid minutes of local model
  // work to learn something knowable in milliseconds.
  const [letterUnavailable, setLetterUnavailable] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    b.reviewerLetterAvailability()
      .then((a) => {
        if (!cancelled) setLetterUnavailable(a.available ? null : a.reason);
      })
      // Best-effort: if the check itself fails we say NOTHING rather than
      // guessing. A wrong "unavailable" would talk a user out of a run that
      // would have worked, which is worse than the late answer we had before.
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [b]);

  // **The ten journals Gaply has actually crawled. §11 D183.**
  //
  // `journal_profiles` reads what a previous crawl stored — no fetch — and
  // carries the crawler KEY, which the bundled Scopus directory does not have.
  // Picking one passes that key, and the backend builds the checklist from that
  // journal's own extracted requirements instead of four structural rows.
  const [profiles, setProfiles] = useState<JournalProfileRow[]>([]);
  useEffect(() => {
    let cancelled = false;
    tauriJournalFingerprintBridge
      .profiles()
      .then((rows) => {
        if (!cancelled) setProfiles(rows.filter((r) => r.ingested && r.requirement_count > 0));
      })
      .catch(() => {
        // A picker that cannot list the profiled set still works: every journal
        // falls back to the Scopus directory and the structural checklist.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const journalMatches = useMemo(() => {
    const q = journalQuery.trim().toLowerCase();
    if (!q) return [];
    // **MERGED, not appended.** Measured: 4 of the 10 profiled journals also
    // appear in the Scopus directory under the same name (PLOS Medicine, Nature
    // Medicine, The Lancet, BMC Public Health). Showing both would put two
    // entries for one journal in front of a researcher, one useful and one not,
    // which is worse than either alone. The profiled row WINS — it is the same
    // journal with strictly more behind it.
    const norm = (t: string) =>
      t
        .toLowerCase()
        .replace(/&/g, 'and')
        .replace(/^(the|journal of|j\.?)\s+/, '')
        .replace(/[^a-z0-9]+/g, ' ')
        .trim();
    const profiled = profiles
      .filter((p) => p.name.toLowerCase().includes(q))
      .map((p) => ({
        name: p.name,
        quartile: (JOURNALS.find((j) => norm(j.name) === norm(p.name))?.quartile ?? null) as
          | string
          | null,
        key: p.key,
        requirementCount: p.requirement_count,
        origin: p.origin,
        fetchedAt: p.fetched_at,
      }));
    const taken = new Set(profiled.map((p) => norm(p.name)));
    const scopus = JOURNALS.filter(
      (j) => j.name.toLowerCase().includes(q) && !taken.has(norm(j.name))
    ).map((j) => ({
      name: j.name,
      quartile: j.quartile ?? null,
      key: undefined as string | undefined,
      requirementCount: 0,
      origin: null as string | null,
      fetchedAt: null as number | null,
    }));
    return [...profiled, ...scopus].slice(0, 6);
  }, [journalQuery, profiles]);

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

  const pickAnalysis = async () => {
    const paths = await pickManuscriptPaths(ANALYSIS_EXTENSIONS, 'Supporting analysis');
    if (paths.length === 0) return;
    try {
      const rep = await b.ingestAnalysis({ paths });
      setAnalysisNote(rep.summary);
      setAnalysisRows(rep.files);
    } catch (e) {
      // Honest, and NOT silent: the files were chosen, so something must be
      // said about them even when the call itself failed.
      setAnalysisNote(
        e instanceof Error ? `Could not read those files: ${e.message}` : 'Could not read those files.'
      );
      setAnalysisRows([]);
    }
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
      // **§11 D192: the key the ingest minted is what the run must read.**
      // `journal.key` is set only for the ten crawled journals; for every other
      // pick it is absent, and before this the pasted page went into the RAG
      // corpus while its stated requirements were stepped over. Naming the
      // journal lets the extractor store them, and the key it returns is
      // PASSED to the run rather than derived a second time (§11 D129).
      let runJournal = journal;
      if (url) {
        try {
          const ing = await b.ingestGuidelines({
            guidelinesUrl: url,
            journalName: journal.name,
            journalKey: journal.key,
          });
          setGuidelinesNote(ing.note);
          if (ing.journal_key) runJournal = { ...journal, key: ing.journal_key };
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
          journal: runJournal,
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
              {/* THE BLURRED MOCK IS THE REAL SHAPE OF A REPORT, and it used
                  to advertise a number the product refuses to compute.
                  `61% publication probability` was here beside MAJOR REVISION.
                  The engine has four fixed DISPLAY bands, each a 1:1 function
                  of the decided recommendation — 0.05 / 0.30 / 0.70 / 0.92
                  (reviewer_agent.rs) — so 61% is not among them, and the band
                  for MAJOR REVISION is 30%: the mock showed a figure twice as
                  favourable as the verdict beside it.

                  ReviewerLetterPanel deliberately renders NO gauge, because a
                  four-value lookup drawn as a percentage ring is a visual
                  implication beyond the evidence. A free user was therefore
                  sold the one thing an entitled user would never see. What is
                  shown now is what the report actually contains: the
                  recommendation, the letter, and the counted findings. */}
              <div className="gds-pr-teaser__blur" aria-hidden="true">
                <div className="gds-pr__head">
                  <div className="gds-pr__verdict" data-status="assessed">MAJOR REVISION</div>
                </div>
                <p className="gds-pr__body">Dear Author, we have completed a full review of your manuscript against the target journal…</p>
                <p className="gds-pr__body">3 blocking · 8 major · 14 minor findings, each quoting the sentence it rests on.</p>
              </div>
              <div className="gds-pr-teaser__cta" data-testid="pr-unlock">
                <div>
                  <Badge status="neutral">Premium</Badge>
                  <strong>See the reviewer letter, the recommendation &amp; every finding with its evidence.</strong>
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
              {/* Was `<Badge status="certain">premium</Badge>`. Removed, not
                  renamed: the badge asserted that THIS RUN used the premium
                  analysis, and nothing in the backend checks a tier —
                  `run_premium_gate` has no production caller and the proxy has
                  no premium mode (§12.1). Entitlement gates ACCESS to the
                  screen, which is real and is where the "Premium" label
                  legitimately appears; it does not change what runs. Enforcing
                  the tier is a separate week's work blocked on a consent record
                  that does not exist, and claiming it sooner would be the same
                  defect pointing the other way. */}
              <Button
                variant="secondary"
                data-testid="pr-export"
                onClick={() =>
                  void downloadReportPdf(result.report, 'publishready-report.pdf')
                    .then(() => toast('PublishReady PDF exported', 'certain'))
                    .catch(() => toast('Export failed', 'flagged'))
                }
              >
                Export summary PDF
              </Button>
              {/* The FULL report — statistics, significance criteria, text
                  similarity, what was not examined, and the manuscript
                  quotations — rendered by the Rust composer during the run.
                  The summary export above reads only `findings`. */}
              {result.runId && (
                <Button
                  data-testid="pr-export-full"
                  onClick={() =>
                    void import('@tauri-apps/api/core')
                      .then(({ invoke }) =>
                        invoke('export_publishready_pdf', { reportId: result.runId }),
                      )
                      .then(() => toast('Full report opened', 'certain'))
                      .catch((e) =>
                        toast(
                          typeof e === 'string' ? e : 'Could not open the full report',
                          'flagged',
                        ),
                      )
                  }
                >
                  Open full report
                </Button>
              )}
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
                // §11 D192: the ingest's sentence must survive into the report.
                // It was rendered on the ENTRY screen only, which the completed
                // run replaces — so the one case it exists for, a page that was
                // reached and had no guidance on it, reached nobody.
                guidelinesNote={guidelinesNote ?? undefined}
                reviewerLetter={<ReviewerLetterPanel letter={result.reviewerLetter} declined={result.declined} />}
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
        {/* **No auth server on this build — the run proceeds. §11 D187.**

            `unverifiable_no_account` deliberately does NOT return early. Every
            other entitlement state above blocks, because on a build that HAS an
            account service each of them is a thing the user can act on: sign
            in, upgrade, retry when the server is back. This one is not — there
            is nothing to sign in to and no plan to read — and blocking on it
            sent a researcher round a loop of honest screens (sign-in card →
            /auth → "No connection configured" → back), which `RequireAuth` has
            avoided since the commit that created both gates.

            What they get is the five local lanes, the checklist and the
            findings. What they do not get is the reviewer letter, and that is
            stated here BEFORE the run rather than discovered after it — the
            same thing `reviewer_letter_availability` already says, which on
            this build also fires because the proxy URL is the loopback
            default. Two independent reasons, one sentence each, and neither
            invents a state the backend does not have. */}
        {entitlement.status === 'unverifiable_no_account' && (
          <p className="gds-jc__disclaimer" data-testid="pr-no-account">
            <strong>Running locally.</strong> This build has no account service
            configured, so the reviewer letter — the one cloud step — needs an
            account and will not run. Everything else in the report is produced
            on this device and is unaffected.
          </p>
        )}
        <Card title="1 · Choose your manuscript">
          <Button variant="secondary" data-testid="pr-pick" onClick={() => (isTauri ? void pickManuscript() : prInputRef.current?.click())}>
            Choose file
          </Button>
          <input ref={prInputRef} type="file" accept=".pdf,.docx" style={{ display: 'none' }} data-testid="pr-file" onChange={(e) => e.target.files?.[0] && void acceptFile(e.target.files[0])} />
          {file && <span className="gds-mono" style={{ marginLeft: 8 }}>{file.name}</span>}
          {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} data-testid="pr-error">{error}</p>}
          {/* Stated BEFORE the run, not after it. Deliberately not an error and
              not a blocker: the local half of the report is unaffected and
              still worth running, so this says what will be missing and leaves
              the decision with the researcher. */}
          {letterUnavailable && (
            <p
              style={{ color: 'var(--g-muted)', fontSize: 13, marginTop: 8 }}
              data-testid="pr-letter-unavailable"
            >
              <strong>Before you run:</strong> {letterUnavailable}
            </p>
          )}
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
                    setJournal({ name: j.name, quartile: j.quartile ?? 'Q4', key: j.key });
                  }}
                >
                  <span>{j.name}</span><Badge status="neutral">{j.quartile ?? '—'}</Badge>
                  {/* **The difference must be VISIBLE. §11 D183.** A profiled
                      journal yields a checklist built from its own pages; every
                      other journal yields the four structural rows. If the two
                      look identical here, the product hides the thing that makes
                      the choice matter. */}
                  {j.key ? (
                    <span className="gds-pr__profiled" data-testid={`pr-journal-profiled-${j.name}`}>
                      Gaply has read this journal&apos;s guidelines — {j.requirementCount}{' '}
                      requirements
                      {/* **Where it came from, and when. §11 D186.** A snapshot
                          that shipped in the app and one this machine fetched
                          from the journal answer different questions, and the
                          date is what a researcher weighs when a limit is
                          months old. Rendering them alike would hide the
                          difference, which is the defect this whole row exists
                          to avoid. */}
                      {j.fetchedAt ? (
                        <span className="gds-pr__provenance">
                          {' · '}
                          {j.origin === 'bundled' ? 'bundled with this release' : 'fetched on this device'}
                          {', '}
                          {new Date(j.fetchedAt * 1000).toLocaleDateString()}
                        </span>
                      ) : null}
                    </span>
                  ) : (
                    <span
                      className="gds-pr__unprofiled"
                      data-testid={`pr-journal-unprofiled-${j.name}`}
                    >
                      Not crawled — structural checks only
                    </span>
                  )}
                </button>
              ))}
            </div>
          )}
        </Card>

        <Card title="3 · Supporting analysis (optional)">
          <Button variant="secondary" data-testid="pr-pick-analysis" onClick={() => void pickAnalysis()}>
            Choose analysis files
          </Button>
          <p className="gds-jc__disclaimer">
            SPSS syntax and journals (.sps, .jnl) are read. Python, R, notebooks and CSV are
            accepted and kept, and Gaply says so rather than parsing them — there is no parser for
            those yet, and what arrives is what decides whether to build one.
          </p>
          {analysisNote && (
            <p className="gds-jc__disclaimer" data-testid="pr-analysis-note">
              <strong>{analysisNote}</strong>
            </p>
          )}
          {/* THE ROWS, not only the total. The total is what a user skims; the
              row is what tells them their file was kept and nothing came of it. */}
          {analysisRows.length > 0 && (
            <ul data-testid="pr-analysis-rows" style={{ margin: '4px 0 0', paddingLeft: 18 }}>
              {analysisRows.map((f, i) => (
                <li key={i} className="gds-finding__detail" style={{ fontSize: 12 }}>
                  <span className="gds-mono">{f.file_name}</span>{' '}
                  {f.outcome === 'parsed'
                    ? `— ${f.commands} command(s), ${f.statistical} statistical`
                    : `— ${f.reason}`}
                </li>
              ))}
            </ul>
          )}
        </Card>

        <Card title="4 · Target journal guidelines (optional)">
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
