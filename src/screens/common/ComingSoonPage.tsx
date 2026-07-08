// Gaply — honest "coming soon" state for nav targets that are not built yet.
// Used instead of a dead '#' link or a fabricated screen. Reads ?feature= for
// the label. This is the interim Phase-2 pattern (a shared placeholder screen).
import React from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
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

const ComingSoonPage: React.FC = () => {
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const feature = params.get('feature') || 'This feature';

  return (
    <div className="gds-root" style={{ height: '100vh' }} data-testid="coming-soon">
      <AppShell
        rail={
          <NavRail
            items={[{ id: 'home', label: 'Home', icon: '◫', onSelect: () => navigate('/app') }]}
            activeId="home"
            brand={<GaplyGlobe scale="mark" />}
          />
        }
        header={<HeaderBar title={feature}><Badge status="neutral">coming soon</Badge></HeaderBar>}
      >
        <Panel title={feature}>
          <Card title={`${feature} — coming soon`} data-testid="coming-soon-card">
            <p style={{ color: 'var(--g-text-2)', fontSize: 14, maxWidth: 520 }}>
              This part of Gaply isn’t built yet. We’re shipping it in a later
              release — it’s intentionally shown as “coming soon” rather than a
              broken link. Everything else on the rail works today.
            </p>
            <Button onClick={() => navigate('/app')} data-testid="coming-soon-home">
              ← Back to Home
            </Button>
          </Card>
        </Panel>
      </AppShell>
    </div>
  );
};

export default ComingSoonPage;
