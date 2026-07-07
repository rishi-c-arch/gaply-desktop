// Gaply DS — /design gallery (dev-only route). Renders every primitive and the
// shell in the dark theme so the whole system is visually reviewable at once.
import React, { useState } from 'react';
import {
  AppShell,
  Badge,
  Button,
  Card,
  GaplyGlobe,
  HeaderBar,
  Modal,
  NavRail,
  Panel,
  ScoreRing,
  Tabs,
  ThreePanelWorkspace,
  ToastProvider,
  UsageMeter,
  useToast,
} from './index';

const RAIL_ITEMS = [
  { id: 'overview', label: 'Overview', icon: '◫' },
  { id: 'manuscripts', label: 'Manuscripts', icon: '≣' },
  { id: 'reports', label: 'Reports', icon: '✓' },
  { id: 'settings', label: 'Settings', icon: '⚙' },
];

const ToastDemo: React.FC = () => {
  const { toast } = useToast();
  return (
    <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
      <Button variant="secondary" onClick={() => toast('Mathematically certain finding', 'certain')}>
        Toast · certain
      </Button>
      <Button variant="secondary" onClick={() => toast('AI-assessed, moderate confidence', 'assessed')}>
        Toast · assessed
      </Button>
      <Button variant="secondary" onClick={() => toast('Citation flagged as refuted', 'flagged')}>
        Toast · flagged
      </Button>
    </div>
  );
};

const GallerySections: React.FC = () => {
  const [modalOpen, setModalOpen] = useState(false);
  return (
    <div style={{ display: 'grid', gap: 16 }}>
      <Card title="Buttons">
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <Button>Primary</Button>
          <Button variant="secondary">Secondary</Button>
          <Button variant="ghost">Ghost</Button>
          <Button variant="danger">Danger</Button>
          <Button disabled>Disabled</Button>
        </div>
      </Card>

      <Card title="Badges — the three certainty tiers + neutral">
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <Badge status="certain">mathematically certain</Badge>
          <Badge status="assessed">AI-assessed</Badge>
          <Badge status="flagged">flagged</Badge>
          <Badge status="neutral">reconsidered</Badge>
        </div>
      </Card>

      <Card title="ScoreRing">
        <div style={{ display: 'flex', gap: 24, alignItems: 'center' }}>
          <ScoreRing score={92} status="certain" />
          <ScoreRing score={61} status="assessed" />
          <ScoreRing score={23} status="flagged" />
          <ScoreRing score={78} status="neutral" size={48} strokeWidth={5} />
        </div>
      </Card>

      <Card title="Card — glassmorphism variant" glass>
        <p style={{ margin: 0, color: 'var(--g-text-2)' }}>
          Glass card over the base surface. DOI in mono:{' '}
          <span className="gds-mono">10.1000/j.rest.2026.01.001</span>
        </p>
      </Card>

      <Card title="Tabs">
        <Tabs
          tabs={[
            { id: 'findings', label: 'Findings', content: <p>Priority-ordered findings render here.</p> },
            { id: 'checklist', label: 'PublishReady', content: <p>Journal checklist renders here.</p> },
            { id: 'debate', label: 'Debate', content: <p>Round-table transcript renders here.</p> },
          ]}
        />
      </Card>

      <Card title="UsageMeter">
        <div style={{ display: 'grid', gap: 12, maxWidth: 420 }}>
          <UsageMeter label="Verifications this month" used={14} limit={50} />
          <UsageMeter label="Cloud tokens" used={41} limit={50} unit="k" />
          <UsageMeter label="Storage" used={52} limit={50} unit=" MB" />
        </div>
      </Card>

      <Card title="Modal + Toast">
        <div style={{ display: 'grid', gap: 12 }}>
          <div>
            <Button onClick={() => setModalOpen(true)}>Open modal</Button>
            <Modal open={modalOpen} title="Confirm analysis" onClose={() => setModalOpen(false)}>
              <p>Run the 6-agent swarm on this manuscript?</p>
              <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
                <Button variant="secondary" onClick={() => setModalOpen(false)}>
                  Cancel
                </Button>
                <Button onClick={() => setModalOpen(false)}>Run</Button>
              </div>
            </Modal>
          </div>
          <ToastDemo />
        </div>
      </Card>

      <Card title="Globe — hero scale (offline HDR, bundled)">
        <GaplyGlobe scale="hero" />
      </Card>
    </div>
  );
};

const DesignGalleryPage: React.FC = () => (
  <ToastProvider>
    <div className="gds-root" style={{ height: '100vh' }}>
      <AppShell
        rail={
          <NavRail
            items={RAIL_ITEMS}
            activeId="overview"
            brand={<GaplyGlobe scale="mark" />}
            brandName="Gaply"
          />
        }
        header={
          <HeaderBar title="Design System — /design (dev only)">
            <GaplyGlobe scale="micro" />
            <Badge status="neutral">v0.1</Badge>
          </HeaderBar>
        }
      >
        <ThreePanelWorkspace
          outline={
            <Panel title="Outline">
              <p style={{ color: 'var(--g-text-3)', fontSize: 13 }}>
                Left panel — document outline / navigation.
              </p>
            </Panel>
          }
          inspector={
            <Panel title="Inspector">
              <div style={{ display: 'grid', gap: 8 }}>
                <Badge status="certain">2 certain</Badge>
                <Badge status="assessed">3 assessed</Badge>
                <Badge status="flagged">1 flagged</Badge>
              </div>
            </Panel>
          }
        >
          <Panel title="All primitives">
            <GallerySections />
          </Panel>
        </ThreePanelWorkspace>
      </AppShell>
    </div>
  </ToastProvider>
);

export default DesignGalleryPage;
