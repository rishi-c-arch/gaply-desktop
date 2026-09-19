// Gaply — Journal Verification (paid). Enter a journal NAME, ISSN, or LINK →
// grounded registry facts + the journal's self-reported claims + the honest
// not-found alert — EVIDENCE, not a verdict. Entitlement gating mirrors
// PublishReady / Stats Verifier (UX; the real gate is the server-side JWT on the
// LLM site-summary). Consumes the Set 4 bridge; NO backend changes.
import React, { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Badge, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { useEntitlement } from '../subscription/entitlement';
import { JournalVerifyBridge, TauriJournalVerifyBridge } from './journalVerifyBridge';
import { JournalVerificationResult } from './journalVerifyTypes';
import JournalVerifyReport from './JournalVerifyReport';
import './journalverify.css';

export interface JournalVerifyPageProps {
  bridge?: JournalVerifyBridge;
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
  forceTier?: 'free' | 'premium';
}

const Rail: React.FC<{ navigate: (p: string) => void }> = ({ navigate }) => (
  <NavRail
    items={[
      { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
      { id: 'jv', label: 'Journal Verification ★', icon: '◈' },
    ]}
    activeId="jv"
    brand={<GaplyGlobe scale="mark" />}
  />
);

const Shell: React.FC<{ navigate: (p: string) => void; children: React.ReactNode }> = ({ navigate, children }) => (
  <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="journal-verify">
    <AppShell rail={<Rail navigate={navigate} />} header={<HeaderBar title="Journal Verification ★" />}>
      <Panel title="Journal Verification — evidence from registries + the journal’s own site">{children}</Panel>
    </AppShell>
  </div>
);

const JournalVerifyPage: React.FC<JournalVerifyPageProps> = ({ bridge, subscriptionService, forceTier }) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const b = useMemo(() => bridge ?? new TauriJournalVerifyBridge(() => session?.access_token), [bridge, session]);

  const entitlement = useEntitlement(
    'journal_verification',
    subscriptionService,
    forceTier ? (forceTier === 'premium' ? 'entitled' : 'not_entitled') : undefined
  );

  const [query, setQuery] = useState('');
  const [result, setResult] = useState<JournalVerificationResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (fn: () => Promise<JournalVerificationResult>) => {
    setBusy(true);
    setError(null);
    try {
      setResult(await fn());
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Verification failed');
    } finally {
      setBusy(false);
    }
  };
  const check = () => query.trim() && run(() => b.verify(query.trim()));
  const pick = (issn: string) => run(() => b.verifyByIssn(issn));

  /* --------------------------- entitlement UX --------------------------- */
  // **No auth server on this build. §11 D187.**
  //
  // Blocked, unlike PublishReady, and the asymmetry is deliberate: PublishReady
  // has five local lanes that produce a real report with the cloud step absent,
  // so letting it run delivers measured work. Whether THIS lane degrades
  // honestly with no cloud has not been measured, and opening it on the
  // assumption that it does would be a claim resting on nothing — the shape
  // this repo keeps correcting. What changes is the SENTENCE: "sign in" sent a
  // researcher to a login screen that cannot work on this build.
  if (entitlement.status === 'unverifiable_no_account') {
    return (
      <Shell navigate={navigate}>
        <Card title="Journal Verification ★ — needs an account" data-testid="jv-no-account">
          <p className="gds-jc__disclaimer">
            This build has no account service configured, so there is nothing to sign in to.
            Journal Check (the offline directory) is on the Home screen and still works.
          </p>
        </Card>
      </Shell>
    );
  }
  if (entitlement.status === 'signed_out') {
    return (
      <Shell navigate={navigate}>
        <Card title="Journal Verification ★ — sign in required" data-testid="jv-signin">
          <p className="gds-jc__disclaimer">Sign in to use Gaply’s premium tools.</p>
          <Button data-testid="jv-signin-cta" onClick={() => navigate('/auth')}>Sign in to continue →</Button>
        </Card>
      </Shell>
    );
  }
  if (entitlement.status === 'offline_unverified') {
    return (
      <Shell navigate={navigate}>
        <Card title="Journal Verification ★ — can’t verify your plan right now" data-testid="jv-offline">
          <p className="gds-jc__disclaimer">You’re signed in, but {entitlement.reason ?? 'the server is unreachable'}. Journal Verification needs the cloud for the site summary — try again once you’re back online.</p>
        </Card>
      </Shell>
    );
  }
  if (entitlement.status === 'not_entitled') {
    return (
      <Shell navigate={navigate}>
        <Card title="Journal Verification ★ — check a journal against the registries" data-testid="jv-teaser">
          <div className="gds-pr-teaser">
            <div className="gds-pr-teaser__blur" aria-hidden="true">
              <p className="gds-pr__body">DOAJ: registered · PubMed: indexed · The journal claims: APC $1200…</p>
            </div>
            <div className="gds-pr-teaser__cta" data-testid="jv-unlock">
              <div>
                <Badge status="neutral">Premium</Badge>
                <strong>Check a journal against DOAJ, OpenAlex &amp; PubMed, and read its self-reported details — evidence, not a verdict.</strong>
                <Button onClick={() => navigate('/app/billing')}>Unlock Journal Verification →</Button>
              </div>
            </div>
          </div>
        </Card>
      </Shell>
    );
  }
  if (entitlement.status === 'checking') {
    return <Shell navigate={navigate}><p className="gds-jc__disclaimer" data-testid="jv-loading">Checking your plan…</p></Shell>;
  }

  /* ------------------------------- tool -------------------------------- */
  return (
    <Shell navigate={navigate}>
      <div style={{ display: 'grid', gap: 16 }} data-testid="jv-entry">
        <Card title="Check a journal">
          <div className="gds-jc__search">
            <input
              className="gds-jc__input"
              placeholder="Enter a journal name, ISSN, or website URL"
              value={query}
              data-testid="jv-input"
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && check()}
            />
            <Button onClick={check} disabled={!query.trim() || busy} data-testid="jv-check">{busy ? 'Checking…' : 'Check'}</Button>
          </div>
          <p className="gds-jc__disclaimer">
            We check public registries (DOAJ, OpenAlex, PubMed) and — for a URL — read the journal’s
            own site for its self-reported details. Evidence + signals; you decide.
          </p>
          {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} role="alert" data-testid="jv-error">{error}</p>}
        </Card>

        {busy && <p className="gds-jc__disclaimer" data-testid="jv-searching">Resolving, checking registries, reading the site…</p>}

        {/* multiple-match disambiguation — never auto-picked */}
        {result && result.disambiguation.length > 0 && (
          <Card title="Which journal did you mean?" data-testid="jv-disambiguation">
            <p className="gds-jc__disclaimer">Several journals match that name — pick the one you mean (names can be mimicked).</p>
            <div className="gds-jv-picker">
              {result.disambiguation.map((m) => (
                <button
                  key={m.issn ?? m.name ?? Math.random()}
                  className="gds-jv-picker__item"
                  data-testid={`jv-pick-${m.issn ?? 'x'}`}
                  disabled={!m.issn || busy}
                  onClick={() => m.issn && pick(m.issn)}
                >
                  <span className="gds-jv-picker__name">{m.name ?? '(unnamed)'}</span>
                  <span className="gds-jv-picker__issn">{m.issn ?? 'no ISSN'}</span>
                </button>
              ))}
            </div>
          </Card>
        )}

        {/* the evidence report */}
        {result && result.disambiguation.length === 0 && <JournalVerifyReport result={result} />}

        {!result && !busy && (
          <p className="gds-jc__disclaimer" data-testid="jv-empty">Enter a journal above to gather evidence from the registries and its own site.</p>
        )}
      </div>
    </Shell>
  );
};

export default JournalVerifyPage;
