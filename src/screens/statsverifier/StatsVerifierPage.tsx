// Gaply — Statistical Analysis Verifier (premium). Upload data + build the
// analysis-spec → deterministic recompute + reported-vs-recomputed verdict →
// verified-vs-advisory report + scoped chat. The recompute is 100% local
// (Set 2/3 engine); the chat is the only cloud hop. Entitlement gating mirrors
// PublishReady EXACTLY — this is UX; the real gate is server-side at the proxy
// (the JWT rides the chat request; the server verifies + consumes uses).
import React, { useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell, Badge, Button, Card, GaplyGlobe, HeaderBar, NavRail, Panel } from '../../design-system';
import { pickManuscriptPath, basenameOf } from '../common/pickFile';
import { isTauri } from '../../utils/isTauri';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { useEntitlement } from '../subscription/entitlement';
import { StatsVerifierBridge, TauriStatsVerifierBridge } from './statsVerifierBridge';
import { AnalysisSpec, StatsPreview, VerificationReport } from './statsVerifierTypes';
import AnalysisSpecBuilder from './AnalysisSpecBuilder';
import StatsVerifierReport from './StatsVerifierReport';
import StatsChatDock from './StatsChatDock';

export interface StatsVerifierPageProps {
  bridge?: StatsVerifierBridge;
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
  /** Test seam: force premium/free instead of fetching. */
  forceTier?: 'free' | 'premium';
}

const SVRail: React.FC<{ navigate: (p: string) => void }> = ({ navigate }) => (
  <NavRail
    items={[
      { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
      { id: 'sv', label: 'Stats Verifier ★', icon: 'Σ' },
    ]}
    activeId="sv"
    brand={<GaplyGlobe scale="mark" />}
  />
);

const Shell: React.FC<{ navigate: (p: string) => void; children: React.ReactNode }> = ({ navigate, children }) => (
  <div className="gds-root" style={{ height: '100vh', overflow: 'auto' }} data-testid="statsverifier">
    <AppShell rail={<SVRail navigate={navigate} />} header={<HeaderBar title="Statistical Analysis Verifier ★" />}>
      <Panel title="Statistical Analysis Verifier — recompute your reported statistics from the data">{children}</Panel>
    </AppShell>
  </div>
);

const StatsVerifierPage: React.FC<StatsVerifierPageProps> = ({ bridge, subscriptionService, forceTier }) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const b = useMemo(
    () => bridge ?? new TauriStatsVerifierBridge(() => session?.access_token),
    [bridge, session]
  );

  const entitlement = useEntitlement(
    'stats_verifier',
    subscriptionService,
    forceTier ? (forceTier === 'premium' ? 'entitled' : 'not_entitled') : undefined
  );

  const [dataFile, setDataFile] = useState<{ name: string; path: string } | null>(null);
  const [manuscriptPath, setManuscriptPath] = useState<string | undefined>();
  const dataInputRef = useRef<HTMLInputElement>(null);
  const manuscriptInputRef = useRef<HTMLInputElement>(null);
  const [preview, setPreview] = useState<StatsPreview | null>(null);
  const [spec, setSpec] = useState<AnalysisSpec | null>(null);
  const [report, setReport] = useState<VerificationReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const acceptDataPath = async (name: string, path: string) => {
    setError(null);
    setReport(null);
    setSpec(null);
    setDataFile({ name, path });
    try {
      setPreview(await b.preview(path, manuscriptPath));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Could not read the data file');
      setPreview(null);
    }
  };
  // Browser fallback: the <input>'s File has no usable .path in the app webview.
  const acceptData = (f: File) => void acceptDataPath(f.name, (f as any).path ?? f.name);
  // Tauri desktop: an ABSOLUTE path from the native dialog — DATA extensions,
  // NOT pdf/docx (this is the CSV/spreadsheet picker).
  const pickData = async () => {
    const p = await pickManuscriptPath(['csv', 'tsv', 'xlsx', 'xls', 'ods'], 'Data');
    if (p) await acceptDataPath(basenameOf(p), p);
  };
  // Tauri desktop: the OPTIONAL manuscript picker — pdf/docx.
  const pickManuscript = async () => {
    const p = await pickManuscriptPath(['pdf', 'docx'], 'Manuscript');
    if (p) setManuscriptPath(p);
  };

  const runVerify = async (s: AnalysisSpec) => {
    if (!dataFile) return;
    setBusy(true);
    setError(null);
    setSpec(s);
    try {
      setReport(await b.verify(dataFile.path, s, manuscriptPath));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Verification failed');
    } finally {
      setBusy(false);
    }
  };

  /* --------------------------- entitlement UX --------------------------- */
  if (entitlement.status === 'signed_out') {
    return (
      <Shell navigate={navigate}>
        <div data-testid="sv-signin">
          <Card title="Statistical Analysis Verifier ★ — sign in required">
            <p className="gds-jc__disclaimer">
              Sign in to use Gaply’s premium tools. Your data still never leaves this device for the
              recompute — sign-in only identifies your account and plan.
            </p>
            <Button data-testid="sv-signin-cta" onClick={() => navigate('/auth')}>Sign in to continue →</Button>
          </Card>
        </div>
      </Shell>
    );
  }

  if (entitlement.status === 'offline_unverified') {
    return (
      <Shell navigate={navigate}>
        <div data-testid="sv-offline">
          <Card title="Statistical Analysis Verifier ★ — can’t verify your plan right now">
            <p className="gds-jc__disclaimer">
              You’re signed in, but {entitlement.reason ?? 'the server is unreachable'}. Try again once
              you’re back online; your offline tools keep working.
            </p>
          </Card>
        </div>
      </Shell>
    );
  }

  if (entitlement.status === 'not_entitled') {
    return (
      <Shell navigate={navigate}>
        <div data-testid="sv-teaser">
          <Card title="Statistical Analysis Verifier ★ — recompute your statistics from the data">
            <div className="gds-pr-teaser">
              <div className="gds-pr-teaser__blur" aria-hidden="true">
                <p className="gds-pr__body">
                  You reported t = 2.41; recomputing from your data gives t = −1.90 → MISMATCH…
                </p>
              </div>
              <div className="gds-pr-teaser__cta" data-testid="sv-unlock">
                <div>
                  <Badge status="neutral">Premium</Badge>
                  <strong>Recompute every reported statistic from your raw data and catch mismatches.</strong>
                  <Button onClick={() => navigate('/app/billing')}>Unlock the verifier →</Button>
                </div>
              </div>
            </div>
          </Card>
        </div>
      </Shell>
    );
  }

  if (entitlement.status === 'checking') {
    return (
      <Shell navigate={navigate}>
        <p className="gds-jc__disclaimer" data-testid="sv-loading">Checking your plan…</p>
      </Shell>
    );
  }

  /* ------------------------------- result ------------------------------- */
  if (report && spec && dataFile) {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="statsverifier">
        <AppShell
          rail={<SVRail navigate={navigate} />}
          header={
            <HeaderBar title="Statistical Analysis Verifier ★">
              <Badge status="certain">premium</Badge>
              <Button variant="secondary" data-testid="sv-new" onClick={() => { setReport(null); setSpec(null); }}>
                New verification
              </Button>
            </HeaderBar>
          }
        >
          <div style={{ display: 'grid', gridTemplateRows: '1fr auto', height: '100%', minHeight: 0 }}>
            <div style={{ minHeight: 0, overflow: 'auto', padding: 16 }}>
              <StatsVerifierReport report={report} />
            </div>
            <StatsChatDock
              bridge={b}
              path={dataFile.path}
              spec={spec}
              manuscriptPath={manuscriptPath}
              userToken={session?.access_token}
            />
          </div>
        </AppShell>
      </div>
    );
  }

  /* ------------------------------- entry -------------------------------- */
  return (
    <Shell navigate={navigate}>
      <div style={{ display: 'grid', gap: 12 }} data-testid="sv-entry">
        <Card title="Upload your data (CSV or spreadsheet)">
          <Button variant="secondary" data-testid="sv-data-pick" onClick={() => (isTauri ? void pickData() : dataInputRef.current?.click())}>
            Choose data file
          </Button>
          <input
            ref={dataInputRef}
            type="file"
            accept=".csv,.tsv,.xlsx,.xls,.ods"
            style={{ display: 'none' }}
            data-testid="sv-data-file"
            onChange={(e) => e.target.files?.[0] && void acceptData(e.target.files[0])}
          />
          {dataFile && <span className="gds-mono" style={{ marginLeft: 8 }}>{dataFile.name}</span>}
          <p className="gds-jc__disclaimer" style={{ marginTop: 8 }}>
            Optionally attach the manuscript (.pdf/.docx) to pre-fill your reported p-value and add
            methodology advisory. Your files stay on this device — the recompute is fully local.
          </p>
          <Button variant="secondary" data-testid="sv-manuscript-pick" onClick={() => (isTauri ? void pickManuscript() : manuscriptInputRef.current?.click())}>
            Attach manuscript
          </Button>
          <input
            ref={manuscriptInputRef}
            type="file"
            accept=".pdf,.docx"
            style={{ display: 'none' }}
            data-testid="sv-manuscript-file"
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) setManuscriptPath((f as any).path ?? f.name);
            }}
          />
          {error && <p style={{ color: 'var(--g-flagged)', fontSize: 13 }} data-testid="sv-error">{error}</p>}
        </Card>

        {preview ? (
          <AnalysisSpecBuilder preview={preview} onVerify={runVerify} busy={busy} />
        ) : (
          <p className="gds-jc__disclaimer" data-testid="sv-empty">
            Upload a data file to build your analysis and verify your reported statistics.
          </p>
        )}
      </div>
    </Shell>
  );
};

export default StatsVerifierPage;
