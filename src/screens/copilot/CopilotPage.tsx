// Gaply — Research Copilot standalone route (/app/copilot). Premium-gated (free
// users get a teaser). Docks the chat to a PublishReady-style context.
import React, { useMemo } from 'react';
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
import { useEntitlement } from '../subscription/entitlement';
import { createSubscriptionService } from '../../services/supabase';
import { useGaplySession } from '../session/SessionProvider';
import { SAMPLE_REPORT } from '../report/sampleReport';
import ResearchCopilotPanel from './ResearchCopilotPanel';
import { ChatClient, TauriChatClient } from './chatBridge';
import { ChatContext } from './chatContext';
import { mayUseCloud } from '../settings/settingsStore';

const SAMPLE_CONTEXT: ChatContext = {
  report: SAMPLE_REPORT,
  ragSnippets: [
    {
      source: 'Reporting standards — strong methods',
      url: 'https://gaply.local/standards/methods',
      text: 'A strong methods section pre-specifies the statistical test, justifies it for the design (e.g. ANOVA for 3+ groups), and reports effect sizes and confidence intervals alongside p-values.',
    },
    {
      source: 'Journal of Rest — Author Guidelines',
      url: 'https://rest.example/authors',
      text: 'Structured abstracts required; declare all conflicts of interest; numbered Vancouver references.',
    },
  ],
  citations: [
    { title: 'Sleep and memory', doi: '10.1000/zzz', retracted: false },
  ],
};

export interface CopilotPageProps {
  client?: ChatClient;
  /** Test seam — production uses the env-configured subscription service. */
  subscriptionService?: ReturnType<typeof createSubscriptionService>;
  forceTier?: 'free' | 'premium';
  context?: ChatContext;
}

const CopilotPage: React.FC<CopilotPageProps> = ({ client, subscriptionService, forceTier, context }) => {
  const navigate = useNavigate();
  const { session } = useGaplySession();
  // User JWT rides each paid chat turn → the proxy's server-side entitlement
  // gate (Set 8). UX gating stays presentation-only.
  const chatClient = useMemo(
    () => client ?? new TauriChatClient(() => session?.access_token),
    [client, session]
  );
  // Entitlement gating mirrors the other four paid pages EXACTLY. (M3: this
  // previously gated on useSubscription().tier === 'free', which reports 'free'
  // whenever the plan can't be verified — wrongly showing an OFFLINE PREMIUM user
  // the upsell instead of the honest "can't verify right now" state.) UX only;
  // the REAL gate is server-side at the proxy, where the JWT rides each turn.
  const entitlement = useEntitlement(
    'research_copilot',
    subscriptionService,
    forceTier ? (forceTier === 'premium' ? 'entitled' : 'not_entitled') : undefined
  );

  const rail = (
    <NavRail
      items={[
        { id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') },
        { id: 'copilot', label: 'Research Copilot ★', icon: '✦' },
      ]}
      activeId="copilot"
      brand={<GaplyGlobe scale="mark" />}
    />
  );

  if (entitlement.status === 'signed_out') {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
        <AppShell rail={rail} header={<HeaderBar title="Research Copilot ★" />}>
          <Panel title="Research Copilot ★">
            <Card title="Research Copilot ★ — sign in required" data-testid="copilot-signin">
              <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>
                Gaply needs you signed in to use its features. Your manuscript still never leaves this
                device — sign-in only identifies your account and plan.
              </p>
              <Button data-testid="copilot-signin-cta" onClick={() => navigate('/auth')}>Sign in to continue →</Button>
            </Card>
          </Panel>
        </AppShell>
      </div>
    );
  }

  if (entitlement.status === 'offline_unverified') {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
        <AppShell rail={rail} header={<HeaderBar title="Research Copilot ★" />}>
          <Panel title="Research Copilot ★">
            <Card title="Research Copilot ★ — can’t verify your plan right now" data-testid="copilot-offline">
              <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>
                You’re signed in, but {entitlement.reason ?? 'the server is unreachable'}. The
                Copilot’s answers need the cloud connection anyway, so try again once you’re back
                online. Your offline tools keep working as usual.
              </p>
            </Card>
          </Panel>
        </AppShell>
      </div>
    );
  }

  if (entitlement.status === 'not_entitled') {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
        <AppShell rail={rail} header={<HeaderBar title="Research Copilot ★" />}>
          <Panel title="Research Copilot ★">
            <Card title="A tutor for your manuscript — in any language" glass data-testid="copilot-teaser">
              <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>
                Ask why a finding matters, how to strengthen your methods, or what to read next.
                It teaches — it will never write your paper for you.
              </p>
              <Button data-testid="copilot-unlock" onClick={() => navigate('/app/billing')}>Unlock Research Copilot →</Button>
            </Card>
          </Panel>
        </AppShell>
      </div>
    );
  }

  if (entitlement.status === 'checking') {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
        <AppShell rail={rail} header={<HeaderBar title="Research Copilot ★" />}>
          <Panel title="Research Copilot ★">
            <p className="gds-jc__disclaimer" data-testid="copilot-loading">Checking your plan…</p>
          </Panel>
        </AppShell>
      </div>
    );
  }

  // entitled — F14 privacy gate: the Copilot is cloud-only, so consent off means
  // the chat is unavailable (no proxy call can happen) until re-enabled.
  if (!mayUseCloud('research_copilot')) {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
        <AppShell rail={rail} header={<HeaderBar title="Research Copilot ★" />}>
          <Panel title="Research Copilot ★">
            <Card title="Cloud access is off for the Copilot" data-testid="copilot-cloud-off">
              <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>
                You turned off the Copilot's cloud access in Settings → Sync &amp; Privacy, so it
                cannot answer (it is the one suite that needs the network). Re-enable it there to
                chat again.
              </p>
              <Button onClick={() => navigate('/app/settings')}>Open Settings</Button>
            </Card>
          </Panel>
        </AppShell>
      </div>
    );
  }

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
      <AppShell
        rail={rail}
        header={<HeaderBar title="Research Copilot ★"><Badge status="certain">premium</Badge></HeaderBar>}
      >
        <Panel title="Ask about your analysis">
          <div style={{ height: 'calc(100vh - 160px)' }}>
            <ResearchCopilotPanel context={context ?? SAMPLE_CONTEXT} client={chatClient} />
          </div>
        </Panel>
      </AppShell>
    </div>
  );
};

export default CopilotPage;
