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
import { useSubscription } from '../subscription/useSubscription';
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
  forceTier?: 'free' | 'premium';
  context?: ChatContext;
}

const CopilotPage: React.FC<CopilotPageProps> = ({ client, forceTier, context }) => {
  const navigate = useNavigate();
  const sub = useSubscription();
  const chatClient = useMemo(() => client ?? new TauriChatClient(), [client]);
  const tier = forceTier ?? (sub.loading ? 'loading' : sub.tier);

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

  if (tier === 'free') {
    return (
      <div className="gds-root" style={{ height: '100vh' }} data-testid="copilot-page">
        <AppShell rail={rail} header={<HeaderBar title="Research Copilot ★" />}>
          <Panel title="Research Copilot ★">
            <Card title="A tutor for your manuscript — in any language" glass data-testid="copilot-teaser">
              <p style={{ color: 'var(--g-text-2)', fontSize: 14 }}>
                Ask why a finding matters, how to strengthen your methods, or what to read next.
                It teaches — it will never write your paper for you.
              </p>
              <Button onClick={() => navigate('/app/billing')}>Unlock Research Copilot →</Button>
            </Card>
          </Panel>
        </AppShell>
      </div>
    );
  }

  // F14 privacy gate — the Copilot is cloud-only, so consent off means the
  // chat is unavailable (no proxy call can happen) until re-enabled.
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
