// Gaply — Research Gap Finder (Set 7). PAID staged flow consuming Sets 2–6:
// upload papers/links → grounded gaps (+ the SEPARATE suggestions lane) →
// achievability Q&A → structured draft scaffolds → verified journal card.
//
// UI HONESTY RULES (mirroring the code-enforced backend gates):
// - grounded gaps vs broader suggestions are UNMISTAKABLY distinct sections
//   with honest labels — a user must never confuse "grounded in my papers"
//   with "the AI's broader idea";
// - journal facts show verified-vs-couldn't-verify clearly; the predatory
//   caution keeps its "not a verdict — verify independently" framing;
// - every backend honest-degradation (needs-cloud, refusals) surfaces as
//   honest UI, never a fake result.
//
// Entitlement is Set 8 wholesale: useEntitlement('research_gap_finder') for
// the four UX states; the REAL gate is server-side (the JWT rides every
// cloud command). Constraints accumulate in FRONTEND state only
// (chat-history-stays-local); prices appear nowhere.
import React, { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Badge, Button, Card, HeaderBar, NavRail, Panel } from '../../design-system';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { useEntitlement, EntitlementStatus } from '../subscription/entitlement';
import { riskVerdict, searchLocal } from '../journal/journalData';
import {
  CorpusReport,
  DraftResult,
  EMPTY_CONSTRAINTS,
  FitResult,
  GapFinderBridge,
  GapFindings,
  JournalVerification,
  ResearcherConstraints,
  TauriGapFinderBridge,
} from './gapfinderBridge';
import '../journal/journal.css'; // reuse .gds-risk light + input styles

export interface GapFinderPageProps {
  bridge?: GapFinderBridge;
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
  /** Test seam: skip the server entitlement ask. */
  forceEntitlement?: EntitlementStatus;
  /** Test seam: stable session id. */
  session?: string;
}

interface QaMsg {
  role: 'user' | 'assistant';
  text: string;
}

const GapFinderPage: React.FC<GapFinderPageProps> = ({ bridge, subscriptionService, forceEntitlement, session }) => {
  const navigate = useNavigate();
  const { session: authSession } = useGaplySession();
  const b = useMemo(() => bridge ?? new TauriGapFinderBridge(), [bridge]);
  const sessionId = useMemo(
    () => session ?? `gf-${Math.random().toString(36).slice(2, 10)}`,
    [session]
  );
  const entitlement = useEntitlement('research_gap_finder', subscriptionService, forceEntitlement);
  const token = () => authSession?.access_token;

  // --- staged state (all local; constraints = the Set 4 accumulator) ------
  const [files, setFiles] = useState<{ name: string; path: string }[]>([]);
  const [linkInput, setLinkInput] = useState('');
  const [links, setLinks] = useState<string[]>([]);
  const [corpus, setCorpus] = useState<CorpusReport | null>(null);
  const [findings, setFindings] = useState<GapFindings | null>(null);
  const [constraints, setConstraints] = useState<ResearcherConstraints>(EMPTY_CONSTRAINTS);
  const [qaLog, setQaLog] = useState<QaMsg[]>([]);
  const [qaInput, setQaInput] = useState('');
  const [achievable, setAchievable] = useState<GapFindings['grounded_gaps']>([]);
  const [draft, setDraft] = useState<DraftResult | null>(null);
  const [draftNote, setDraftNote] = useState('');
  const [journalIssn, setJournalIssn] = useState('');
  const [journalName, setJournalName] = useState('');
  const [journal, setJournal] = useState<JournalVerification | null>(null);
  const [fit, setFit] = useState<FitResult | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async (stage: string, f: () => Promise<void>) => {
    setBusy(stage);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(e instanceof Error ? e.message : `${stage} failed`);
    } finally {
      setBusy(null);
    }
  };

  /* --------------------------- entitlement UX --------------------------- */
  if (entitlement.status === 'checking') {
    return <Shell navigate={navigate}><p className="gds-jc__disclaimer" data-testid="gf-loading">Checking your plan…</p></Shell>;
  }
  if (entitlement.status === 'signed_out') {
    return (
      <Shell navigate={navigate}>
        <Card title="Research Gap Finder ★ — sign in required">
          <div data-testid="gf-signin">
            <p className="gds-jc__disclaimer">
              Gaply needs you signed in to use its features. Your papers still never leave this
              device un-summarized — sign-in only identifies your account and plan.
            </p>
            <Button data-testid="gf-signin-cta" onClick={() => navigate('/auth')}>Sign in to continue →</Button>
          </div>
        </Card>
      </Shell>
    );
  }
  if (entitlement.status === 'offline_unverified') {
    return (
      <Shell navigate={navigate}>
        <Card title="Research Gap Finder ★ — can’t verify your plan right now">
          <p className="gds-jc__disclaimer" data-testid="gf-offline">
            You’re signed in, but your plan can’t be verified right now (offline or server
            unreachable). The Gap Finder’s reasoning needs the cloud connection anyway — try again
            once you’re back online.
          </p>
        </Card>
      </Shell>
    );
  }
  if (entitlement.status === 'not_entitled') {
    return (
      <Shell navigate={navigate}>
        <Card title="Research Gap Finder ★ — find what only you can research">
          <div data-testid="gf-teaser">
            <p>
              Upload your base papers and Gaply extracts research gaps grounded in them, narrows
              them to what your funding, lab and time can achieve, drafts refinable objectives, and
              verifies your target journal from real registry data.
            </p>
            <div data-testid="gf-unlock">
              <Badge status="neutral">Premium</Badge>
              <Button onClick={() => navigate('/app/billing')}>Unlock the Gap Finder →</Button>
            </div>
          </div>
        </Card>
      </Shell>
    );
  }

  /* ------------------------------ the flow ------------------------------ */
  return (
    <Shell navigate={navigate}>
      <div data-testid="gapfinder" style={{ display: 'grid', gap: 16, padding: 16 }}>
        {error && <p style={{ color: 'var(--g-flagged)' }} data-testid="gf-error">{error}</p>}

        {/* Stage 1 — papers */}
        <Panel title="1 · Base papers">
          <input
            type="file"
            accept=".pdf,.docx,.txt"
            multiple
            data-testid="gf-files"
            onChange={(e) => {
              const list = Array.from(e.target.files ?? []).slice(0, 8);
              setFiles(list.map((f) => ({ name: f.name, path: (f as any).path ?? f.name })));
            }}
          />
          <div className="gds-jc__input" style={{ display: 'flex', gap: 8, marginTop: 8 }}>
            <input
              placeholder="…or paste a paper link (PDF or page URL)"
              value={linkInput}
              data-testid="gf-link-input"
              onChange={(e) => setLinkInput(e.target.value)}
            />
            <Button
              variant="secondary"
              data-testid="gf-link-add"
              onClick={() => {
                if (linkInput.trim()) {
                  setLinks((l) => [...l, linkInput.trim()].slice(0, 8));
                  setLinkInput('');
                }
              }}
            >
              Add link
            </Button>
          </div>
          {(files.length > 0 || links.length > 0) && (
            <p data-testid="gf-inputs">{files.map((f) => f.name).concat(links).join(' · ')}</p>
          )}
          <Button
            data-testid="gf-build-corpus"
            disabled={busy !== null || (files.length === 0 && links.length === 0)}
            onClick={() =>
              void run('corpus', async () => {
                setCorpus(await b.buildCorpus({ session: sessionId, paths: files.map((f) => f.path), links }));
              })
            }
          >
            Read the papers
          </Button>
          {corpus && (
            <div data-testid="gf-corpus">
              <p>{corpus.note}</p>
              {corpus.digests.map((d) => (
                <span key={d.id} className="gds-chat__chip" data-testid={`gf-paper-${d.id}`}>
                  {d.id}: {d.title}{d.truncated ? ' (truncated to fit)' : ''}
                </span>
              ))}
              {corpus.results.filter((r) => r.status === 'unavailable').map((r, i) => (
                <p key={i} style={{ color: 'var(--g-flagged)', fontSize: 13 }} data-testid="gf-paper-failed">
                  {'origin' in r ? r.origin : ''}: {(r as any).reason}
                </p>
              ))}
            </div>
          )}
        </Panel>

        {/* Stage 2 — grounded gaps + the SEPARATE suggestions lane */}
        {corpus && corpus.digests.length > 0 && (
          <Panel title="2 · Research gaps ★">
            <Button
              data-testid="gf-find-gaps"
              disabled={busy !== null}
              onClick={() =>
                void run('gaps', async () => {
                  const out = await b.findGaps({ session: sessionId, corpus, userToken: token() });
                  setFindings(out.findings);
                })
              }
            >
              Find gaps in these papers
            </Button>
            {findings && !findings.available && (
              <p className="gds-jc__disclaimer" data-testid="gf-gaps-offline">
                Gap finding needs Gaply’s cloud connection, which isn’t reachable right now. Your
                papers stay read and ready — try again once connected.
              </p>
            )}
            {findings && findings.available && (
              <div style={{ display: 'grid', gap: 12 }}>
                <section data-testid="gf-grounded" style={{ border: '1px solid var(--g-certain, #2c7)', borderRadius: 8, padding: 12 }}>
                  <h4><Badge status="certain">Grounded in your papers</Badge></h4>
                  {findings.grounded_gaps.length === 0 && <p>No grounded gaps were found in these papers.</p>}
                  {findings.grounded_gaps.map((g, i) => (
                    <div key={i} data-testid="gf-gap">
                      <strong>{g.description}</strong>
                      <p style={{ fontSize: 13 }}>{g.rationale}</p>
                      <p style={{ fontSize: 12 }}>
                        grounded in: {g.paper_refs.map((r) => <span key={r} className="gds-chat__chip">{r}</span>)}
                      </p>
                    </div>
                  ))}
                </section>
                <section data-testid="gf-suggestions" style={{ border: '1px dashed var(--g-flagged, #e93)', borderRadius: 8, padding: 12 }}>
                  <h4><Badge status="flagged">Broader ideas — NOT grounded in your papers</Badge></h4>
                  <p className="gds-jc__disclaimer">
                    These are the AI’s wider suggestions with no grounding in your uploaded papers —
                    verify them independently before building on them.
                  </p>
                  {findings.suggestions.length === 0 && <p>None this time.</p>}
                  {findings.suggestions.map((s, i) => (
                    <div key={i} data-testid="gf-suggestion">
                      <strong>{s.description}</strong>
                      {s.note && <p style={{ fontSize: 13 }}>{s.note}</p>}
                    </div>
                  ))}
                </section>
              </div>
            )}
          </Panel>
        )}

        {/* Stage 3 — achievability Q&A */}
        {findings && findings.available && findings.grounded_gaps.length > 0 && (
          <Panel title="3 · What can YOU achieve? ★">
            <div data-testid="gf-constraints">
              {Object.entries({
                funding: constraints.funding_level,
                lab: constraints.lab_access,
                time: constraints.time_horizon,
                team: constraints.team_size,
              })
                .filter(([, v]) => v)
                .map(([k, v]) => (
                  <span key={k} className="gds-chat__chip">{k}: {v}</span>
                ))}
              {constraints.available_facilities.map((f) => (
                <span key={f} className="gds-chat__chip">facility: {f}</span>
              ))}
            </div>
            <div data-testid="gf-qa-log">
              {qaLog.map((m, i) => (
                <p key={i} data-testid={`gf-qa-${m.role}`}><strong>{m.role === 'user' ? 'You' : 'Gaply'}:</strong> {m.text}</p>
              ))}
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <input
                className="gds-chat__input"
                placeholder="Tell Gaply about your funding, lab, facilities, time…"
                value={qaInput}
                data-testid="gf-qa-input"
                onChange={(e) => setQaInput(e.target.value)}
              />
              <Button
                data-testid="gf-qa-send"
                disabled={busy !== null || !qaInput.trim()}
                onClick={() =>
                  void run('qa', async () => {
                    const answer = qaInput.trim();
                    setQaLog((l) => [...l, { role: 'user', text: answer }]);
                    setQaInput('');
                    const turn = await b.qaTurn({
                      session: sessionId,
                      corpus: corpus!,
                      groundedGaps: findings.grounded_gaps,
                      constraints,
                      latestAnswer: answer,
                      userToken: token(),
                    });
                    if (!turn.available) {
                      setQaLog((l) => [...l, { role: 'assistant', text: turn.warnings[0] ?? 'The Q&A needs the cloud connection right now.' }]);
                      return;
                    }
                    setConstraints(turn.constraints);
                    if (turn.kind === 'refused_ghostwriting') {
                      setQaLog((l) => [...l, { role: 'assistant', text: turn.follow_up_question || turn.warnings[0] || 'I can’t write your paper — but I can help you narrow and plan.' }]);
                      return;
                    }
                    if (turn.achievable_gaps.length > 0) setAchievable(turn.achievable_gaps);
                    setQaLog((l) => [
                      ...l,
                      { role: 'assistant', text: turn.follow_up_question || 'I have what I need — see your achievable gaps below.' },
                    ]);
                  })
                }
              >
                Send
              </Button>
            </div>
            {achievable.length > 0 && (
              <div data-testid="gf-achievable">
                <h4><Badge status="certain">Achievable for you (still grounded)</Badge></h4>
                {achievable.map((g, i) => (
                  <p key={i}>
                    <strong>{g.description}</strong>{' '}
                    {g.paper_refs.map((r) => <span key={r} className="gds-chat__chip">{r}</span>)}
                  </p>
                ))}
              </div>
            )}
          </Panel>
        )}

        {/* Stage 4 — structured draft scaffolds */}
        {achievable.length > 0 && (
          <Panel title="4 · Draft scaffold ★ (a design to refine — never your paper)">
            <div style={{ display: 'flex', gap: 8 }}>
              <input
                className="gds-chat__input"
                placeholder="Optional note, e.g. “prefer low-cost designs”"
                value={draftNote}
                data-testid="gf-draft-note"
                onChange={(e) => setDraftNote(e.target.value)}
              />
              <Button
                data-testid="gf-draft-run"
                disabled={busy !== null}
                onClick={() =>
                  void run('draft', async () => {
                    setDraft(
                      await b.draft({
                        session: sessionId,
                        corpus: corpus!,
                        achievableGaps: achievable,
                        constraints,
                        userNote: draftNote,
                        userToken: token(),
                      })
                    );
                  })
                }
              >
                Draft objectives & methodology
              </Button>
            </div>
            {draft && !draft.available && (
              <p className="gds-jc__disclaimer" data-testid="gf-draft-offline">
                Drafting needs Gaply’s cloud connection right now — your narrowed gaps and
                constraints are kept.
              </p>
            )}
            {draft && draft.kind === 'refused_ghostwriting' && (
              <p className="gds-jc__disclaimer" data-testid="gf-draft-refused">{draft.warnings[0]}</p>
            )}
            {draft && draft.available && draft.kind === 'answered' && (
              <div data-testid="gf-draft">
                {draft.drafts.map((d) => (
                  <div key={d.gap_ref} data-testid={`gf-draft-${d.gap_ref}`}>
                    <h4>{d.gap_ref} <span style={{ fontSize: 12 }}>(grounded in {d.paper_refs.join(', ')})</span></h4>
                    <strong>Objectives</strong>
                    <ul>{d.objectives.map((o, i) => <li key={i} data-testid="gf-objective">{o}</li>)}</ul>
                    <strong>Methodology steps</strong>
                    <ol>{d.methodology_steps.map((s, i) => <li key={i} data-testid="gf-step">{s}</li>)}</ol>
                    {d.expected_outcomes.length > 0 && (
                      <>
                        <strong>Expected outcomes</strong>
                        <ul>{d.expected_outcomes.map((o, i) => <li key={i}>{o}</li>)}</ul>
                      </>
                    )}
                    {d.feasibility_notes.length > 0 && (
                      <>
                        <strong>Feasibility</strong>
                        <ul>{d.feasibility_notes.map((o, i) => <li key={i}>{o}</li>)}</ul>
                      </>
                    )}
                  </div>
                ))}
                {draft.warnings.length > 0 && (
                  <p className="gds-jc__disclaimer" data-testid="gf-draft-warnings">{draft.warnings.join(' · ')}</p>
                )}
              </div>
            )}
          </Panel>
        )}

        {/* Stage 5 — verified journal card + fit */}
        {achievable.length > 0 && (
          <Panel title="5 · Target journal — verified from registries">
            <div className="gds-jc__input" style={{ display: 'flex', gap: 8 }}>
              <input placeholder="ISSN (e.g. 1365-2869)" value={journalIssn} data-testid="gf-journal-issn" onChange={(e) => setJournalIssn(e.target.value)} />
              <input placeholder="Journal name (optional)" value={journalName} data-testid="gf-journal-name" onChange={(e) => setJournalName(e.target.value)} />
              <Button
                data-testid="gf-journal-verify"
                disabled={busy !== null || !journalIssn.trim()}
                onClick={() =>
                  void run('journal', async () => {
                    // F9 stays the single risk authority: fold its local
                    // directory signals into the registry verification.
                    const local = searchLocal(journalName || journalIssn)[0];
                    const signals = local ? riskVerdict(local).reasons : [];
                    const card = await b.verifyJournal({
                      issn: journalIssn.trim(),
                      name: journalName.trim() || undefined,
                      localPredatorySignals: signals,
                    });
                    setJournal(card);
                    setFit(null);
                  })
                }
              >
                Verify journal
              </Button>
            </div>
            {journal && (
              <div data-testid="gf-journal-card">
                <h4>{journal.name ?? journal.query}</h4>
                <p style={{ fontSize: 12 }}>Every fact below comes from the registries — never from the AI.</p>
                <ul>
                  <li data-testid="gf-journal-doaj">
                    DOAJ registered:{' '}
                    {journal.doaj_registered === true ? 'yes (verified)' : journal.doaj_registered === false ? 'no (verified)' : 'couldn’t verify'}
                  </li>
                  <li data-testid="gf-journal-activity">
                    Recent publishing activity:{' '}
                    {journal.recent_activity === true
                      ? `yes — ${journal.works_by_year.map((y) => `${y.year}: ${y.works}`).join(', ')}`
                      : journal.recent_activity === false
                        ? 'none found (verified)'
                        : 'couldn’t verify'}
                  </li>
                  <li data-testid="gf-journal-scope">
                    Scope: {journal.scope.length > 0 ? journal.scope.join(', ') : 'couldn’t verify'}
                  </li>
                </ul>
                {journal.unverified.length > 0 && (
                  <p data-testid="gf-journal-unverified" style={{ color: 'var(--g-text-3)' }}>
                    Couldn’t verify: {journal.unverified.join('; ')}
                  </p>
                )}
                {journal.warning ? (
                  <div className="gds-risk" data-level="red" data-testid="gf-journal-warning">
                    {journal.warning}
                  </div>
                ) : (
                  <div className="gds-risk" data-level="green" data-testid="gf-journal-ok">
                    Registry data shows a registered, actively publishing journal.
                  </div>
                )}
                <Button
                  data-testid="gf-fit-run"
                  disabled={busy !== null}
                  onClick={() =>
                    void run('fit', async () => {
                      setFit(
                        await b.fit({
                          session: sessionId,
                          corpus: corpus!,
                          achievableGaps: achievable,
                          journalCard: journal,
                          userToken: token(),
                        })
                      );
                    })
                  }
                >
                  Is my gap a fit for this journal? ★
                </Button>
                {fit && !fit.available && (
                  <p className="gds-jc__disclaimer" data-testid="gf-fit-offline">
                    Fit reasoning needs the cloud connection — the verified journal facts above stay valid.
                  </p>
                )}
                {fit && fit.available && (
                  <div data-testid="gf-fit">
                    {fit.fits.map((f, i) => (
                      <p key={i}>
                        <Badge status={f.verdict === 'good_fit' ? 'certain' : f.verdict === 'poor_fit' ? 'flagged' : 'neutral'}>
                          {f.verdict.replace('_', ' ')}
                        </Badge>{' '}
                        {f.reasoning} <span style={{ fontSize: 12 }}>(AI’s reasoning over the verified card — the facts above are the registry’s)</span>
                      </p>
                    ))}
                  </div>
                )}
              </div>
            )}
          </Panel>
        )}
      </div>
    </Shell>
  );
};

const Shell: React.FC<{ navigate: (p: string) => void; children: React.ReactNode }> = ({ navigate, children }) => (
  <div className="gds-root" style={{ minHeight: '100vh' }}>
    <AppShell
      rail={
        <NavRail
          items={[
            { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
            { id: 'gapfinder', label: 'Gap Finder ★', icon: '◎' },
          ]}
        />
      }
      header={
        <HeaderBar title="Research Gap Finder ★">
          <Badge status="certain">premium</Badge>
        </HeaderBar>
      }
    >
      {children}
    </AppShell>
  </div>
);

export default GapFinderPage;
