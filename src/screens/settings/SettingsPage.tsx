// Gaply — Settings & account (F14). Makes the privacy architecture VISIBLE
// and CONTROLLABLE: per-suite cloud toggles (the mayUseCloud gate every cloud
// call-site consults), the "Where your data lives" panel, offline-data
// inventory with a safe "clear local data", subscription + usage meters
// (usage_counters), appearance, and the future-SLM status line.
import React, { useEffect, useMemo, useState } from 'react';
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
  ThreePanelWorkspace,
  UsageMeter,
} from '../../design-system';
import { ToastProvider, useToast } from '../../design-system/Toast';
import { createProfileService, createUsageService, ProfileRow } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { useSubscription, readUsage } from '../subscription/useSubscription';
import { ONLINE_CAPPED } from '../subscription/tiers';
import { JOURNAL_COUNT } from '../journal/journalData';
import {
  Appearance,
  CLOUD_SUITES,
  clearLocalData,
  formatBytes,
  listLocalData,
  localBytesUsed,
  readAppearance,
  readCloudConsent,
  setCloudConsent,
  writeAppearance,
} from './settingsStore';
import { ConnectivityIndicator } from './connectivity';
import './settings.css';

/** Bundled offline packs (shipped inside the app, usable with zero network). */
const OFFLINE_PACKS = [
  { id: 'journal-directory', name: 'Journal directory (SJR quartiles + indexing)', detail: `${JOURNAL_COUNT} journals · bundled` },
  { id: 'reporting-standards', name: 'Reporting-standards guidelines', detail: 'statistical reporting rules · bundled' },
];

const SECTIONS = [
  { id: 'profile', label: 'Profile & ORCID' },
  { id: 'privacy', label: 'Sync & Privacy' },
  { id: 'offline', label: 'Offline Data' },
  { id: 'subscription', label: 'Subscription' },
  { id: 'appearance', label: 'Appearance' },
  { id: 'about', label: 'About' },
];

export interface SettingsPageProps {
  /** Test seams — production uses the env-configured services. */
  profileService?: ReturnType<typeof createProfileService>;
  usageService?: ReturnType<typeof createUsageService>;
}

const Inner: React.FC<SettingsPageProps> = ({ profileService, usageService }) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  const { toast } = useToast();
  const { tier, isPremium, status } = useSubscription();
  const profiles = useMemo(() => profileService ?? createProfileService(), [profileService]);
  const usageSvc = useMemo(() => usageService ?? createUsageService(), [usageService]);

  /* profile */
  const [profile, setProfile] = useState<ProfileRow | null>(null);
  const [displayName, setDisplayName] = useState('');
  const [orcid, setOrcid] = useState('');

  /* privacy */
  const [consent, setConsent] = useState(() => readCloudConsent());

  /* offline data */
  const [localEntries, setLocalEntries] = useState(() => listLocalData());
  const [bytesUsed, setBytesUsed] = useState(() => localBytesUsed());
  const [clearArmed, setClearArmed] = useState(false);

  /* subscription usage */
  const [usage, setUsage] = useState<Record<string, number>>({});

  /* appearance */
  const [appearance, setAppearance] = useState<Appearance>(() => readAppearance());

  useEffect(() => {
    let alive = true;
    if (!session) return;
    profiles.getOwn(session.user.id).then((r) => {
      if (!alive || !r.data) return;
      setProfile(r.data);
      setDisplayName(r.data.display_name ?? '');
      setOrcid(r.data.orcid ?? '');
    });
    (async () => {
      const next: Record<string, number> = {};
      for (const f of Object.keys(ONLINE_CAPPED)) next[f] = await readUsage(session.user.id, f, usageSvc);
      if (alive) setUsage(next);
    })();
    return () => {
      alive = false;
    };
  }, [session, profiles, usageSvc]);

  const saveProfile = async () => {
    if (!session) return;
    const res = await profiles.upsertOwn({
      id: session.user.id,
      email: session.user.email ?? profile?.email ?? '',
      display_name: displayName || null,
      orcid: orcid || null,
    });
    if (res.error) toast(`Not saved: ${res.error}`, 'flagged');
    else toast('Profile saved', 'certain');
  };

  const toggleSuite = (id: (typeof CLOUD_SUITES)[number]['id']) => {
    setConsent(setCloudConsent(id, !consent[id]));
  };

  const refreshLocal = () => {
    setLocalEntries(listLocalData());
    setBytesUsed(localBytesUsed());
  };

  const doClear = () => {
    if (!clearArmed) {
      setClearArmed(true);
      return;
    }
    const removed = clearLocalData();
    setClearArmed(false);
    refreshLocal();
    setConsent(readCloudConsent()); // consent resets to defaults with the store
    setAppearance(readAppearance());
    toast(`Cleared ${removed} local item(s) — your account & subscription are untouched`, 'certain');
  };

  const setTheme = (theme: Appearance['theme']) => {
    const next = { ...appearance, theme };
    setAppearance(next);
    writeAppearance(next);
  };
  const setScale = (uiScale: Appearance['uiScale']) => {
    const next = { ...appearance, uiScale };
    setAppearance(next);
    writeAppearance(next);
  };

  return (
    <div
      className="gds-root"
      style={{ height: '100vh', fontSize: `${appearance.uiScale}%` }}
      data-gds-theme={appearance.theme}
      data-testid="settings"
    >
      <AppShell
        rail={
          <NavRail
            items={[
              { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
              { id: 'settings', label: 'Settings', icon: '⚙' },
            ]}
            activeId="settings"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={
          <HeaderBar title="Settings">
            <ConnectivityIndicator />
            {session ? (
              <Badge status={isPremium ? 'certain' : 'neutral'}>{tier}</Badge>
            ) : (
              <Badge status="neutral">offline</Badge>
            )}
          </HeaderBar>
        }
      >
        <ThreePanelWorkspace
          outline={
            <Panel title="Sections">
              <nav className="gds-set__nav" data-testid="settings-nav">
                {SECTIONS.map((s) => (
                  <button
                    key={s.id}
                    className="gds-set__navitem"
                    onClick={() => document.getElementById(`set-${s.id}`)?.scrollIntoView({ behavior: 'smooth' })}
                  >
                    {s.label}
                  </button>
                ))}
              </nav>
            </Panel>
          }
        >
          <Panel title="Settings & account">
            <div className="gds-set" data-testid="settings-sections">
              {/* ------------------------- Profile & ORCID ------------------------ */}
              <section id="set-profile">
                <Card title="Profile & ORCID">
                  {session ? (
                    <div className="gds-set__form">
                      <label className="gds-set__field">
                        <span>Display name</span>
                        <input
                          value={displayName}
                          onChange={(e) => setDisplayName(e.target.value)}
                          placeholder="Dr. …"
                          data-testid="profile-name"
                        />
                      </label>
                      <label className="gds-set__field">
                        <span>ORCID iD</span>
                        <input
                          value={orcid}
                          onChange={(e) => setOrcid(e.target.value)}
                          placeholder="0000-0000-0000-0000"
                          className="gds-mono"
                          data-testid="profile-orcid"
                        />
                      </label>
                      <div className="gds-set__meta">
                        <span>{session.user.email}</span>
                        {profile?.role && <span>· {profile.role}</span>}
                        {profile?.field && <span>· {profile.field}</span>}
                      </div>
                      <Button onClick={saveProfile} data-testid="profile-save">Save profile</Button>
                    </div>
                  ) : (
                    <p className="gds-set__muted" data-testid="profile-offline">
                      Working locally — sign in to keep a profile (name, ORCID, field). Nothing
                      else changes: every local suite works without an account.
                    </p>
                  )}
                </Card>
              </section>

              {/* -------------------------- Sync & Privacy ------------------------ */}
              <section id="set-privacy">
                <Card title="Sync & Privacy">
                  {/* Where your data lives — the honest diagram */}
                  <div className="gds-set__datamap" data-testid="privacy-panel">
                    <h4 className="gds-set__datamap-title">Where your data lives</h4>
                    <div className="gds-set__datamap-boxes">
                      <div className="gds-set__databox" data-testid="device-box">
                        <div className="gds-set__databox-head">🖥 Your device</div>
                        <ul>
                          <li>Manuscripts (PDF/DOCX/TXT)</li>
                          <li>Full analysis & findings</li>
                          <li>Plagiarism corpus & RAG index</li>
                        </ul>
                        <Badge status="certain">never uploaded</Badge>
                      </div>
                      <div className="gds-set__databox-arrow" aria-hidden="true">→ structured summaries only →</div>
                      <div className="gds-set__databox" data-testid="cloud-box">
                        <div className="gds-set__databox-head">☁ Gaply cloud</div>
                        <ul>
                          <li>Account info & subscription</li>
                          <li>Structured summaries for paid checks</li>
                          <li>Usage counters & community posts</li>
                        </ul>
                        <Badge status="neutral">metadata only</Badge>
                      </div>
                    </div>
                    <p className="gds-set__datamap-note" data-testid="on-device-statement">
                      Manuscripts are processed <strong>on-device</strong> and never uploaded — the
                      six analysis agents run locally; only the citation-verification agent may
                      reach the network, through the Gaply proxy, with structured summaries only.
                    </p>
                  </div>

                  {/* per-suite cloud toggles */}
                  <div className="gds-set__toggles" data-testid="privacy-toggles">
                    {CLOUD_SUITES.map((s) => (
                      <label key={s.id} className="gds-set__toggle">
                        <input
                          type="checkbox"
                          checked={consent[s.id]}
                          onChange={() => toggleSuite(s.id)}
                          data-testid={`privacy-${s.id}`}
                        />
                        <span className="gds-set__toggle-label">{s.label}</span>
                        <span className="gds-set__toggle-sends">{s.sends}</span>
                      </label>
                    ))}
                  </div>
                  <p className="gds-set__muted">
                    Off means off: the suite will not touch the network until you re-enable it.
                    Local suites have no cloud path and no toggle.
                  </p>
                </Card>
              </section>

              {/* --------------------------- Offline Data ------------------------- */}
              <section id="set-offline">
                <Card title="Offline Data">
                  <div className="gds-set__packs" data-testid="offline-packs">
                    {OFFLINE_PACKS.map((p) => (
                      <div key={p.id} className="gds-set__pack">
                        <span>{p.name}</span>
                        <span className="gds-set__muted">{p.detail}</span>
                      </div>
                    ))}
                  </div>
                  <div className="gds-set__storage" data-testid="storage-used">
                    Local storage used: <strong>{formatBytes(bytesUsed)}</strong>
                    {localEntries.length > 0 && <span className="gds-set__muted"> · {localEntries.length} item(s)</span>}
                  </div>
                  <div className="gds-set__clear-row">
                    <Button variant="secondary" onClick={doClear} data-testid="clear-local">
                      {clearArmed ? 'Confirm — clear local data' : 'Clear local data…'}
                    </Button>
                    {clearArmed && (
                      <span className="gds-set__muted" data-testid="clear-warning">
                        Wipes caches & preferences on this device only — your account, subscription
                        and synced metadata in Supabase are untouched.
                      </span>
                    )}
                  </div>
                </Card>
              </section>

              {/* -------------------------- Subscription -------------------------- */}
              <section id="set-subscription">
                <Card title="Subscription">
                  <div className="gds-set__plan" data-testid="plan-line">
                    Current plan: <Badge status={isPremium ? 'certain' : 'neutral'}>{tier}</Badge>
                    <span className="gds-set__muted">status: {status}</span>
                  </div>
                  {session && !isPremium && (
                    <div className="gds-set__meters" data-testid="settings-usage-meters">
                      <UsageMeter
                        label="Citation verifications"
                        used={usage.citation_verification ?? 0}
                        limit={ONLINE_CAPPED.citation_verification}
                      />
                      <UsageMeter
                        label="Journal checks"
                        used={usage.journal_check ?? 0}
                        limit={ONLINE_CAPPED.journal_check}
                      />
                    </div>
                  )}
                  <p className="gds-set__muted">
                    Upgrade, downgrade, Razorpay billing (INR/UPI) and the student discount live on
                    the billing page. Free offline suites stay unlimited on every plan.
                  </p>
                  <Button onClick={() => navigate('/app/billing')} data-testid="manage-plan">
                    {isPremium ? 'Manage plan & billing' : 'Upgrade — see plans'}
                  </Button>
                </Card>
              </section>

              {/* --------------------------- Appearance --------------------------- */}
              <section id="set-appearance">
                <Card title="Appearance">
                  <div className="gds-set__row">
                    <span className="gds-set__rowlabel">Theme</span>
                    <Button
                      variant={appearance.theme === 'darkest' ? 'primary' : 'secondary'}
                      onClick={() => setTheme('darkest')}
                      data-testid="theme-darkest"
                    >
                      Darkest
                    </Button>
                    <Button
                      variant={appearance.theme === 'dark' ? 'primary' : 'secondary'}
                      onClick={() => setTheme('dark')}
                      data-testid="theme-dark"
                    >
                      Dark
                    </Button>
                  </div>
                  <div className="gds-set__row">
                    <span className="gds-set__rowlabel">UI scale</span>
                    {([90, 100, 110] as const).map((s) => (
                      <Button
                        key={s}
                        variant={appearance.uiScale === s ? 'primary' : 'secondary'}
                        onClick={() => setScale(s)}
                        data-testid={`scale-${s}`}
                      >
                        {s}%
                      </Button>
                    ))}
                  </div>
                </Card>
              </section>

              {/* ------------------------------ About ----------------------------- */}
              <section id="set-about">
                <Card title="About">
                  <p className="gds-set__muted">
                    Gaply Desktop — research-integrity engine. Six agents (extraction, statistical
                    validation, AI detection, plagiarism, RAG, citation verification) run on your
                    device in a round-table swarm.
                  </p>
                  <p className="gds-set__slm" data-testid="slm-status">
                    Local analysis model: <strong>interim heuristic</strong> — fine-tuned Gaply
                    model coming (the model seam is swappable; your workflow won't change).
                  </p>
                  <p className="gds-set__muted">
                    <a href="/privacy" className="gds-set__link">Privacy policy</a> ·{' '}
                    <a href="/terms" className="gds-set__link">Terms</a>
                  </p>
                </Card>
              </section>
            </div>
          </Panel>
        </ThreePanelWorkspace>
      </AppShell>
    </div>
  );
};

const SettingsPage: React.FC<SettingsPageProps> = (props) => (
  <ToastProvider>
    <Inner {...props} />
  </ToastProvider>
);

export default SettingsPage;
