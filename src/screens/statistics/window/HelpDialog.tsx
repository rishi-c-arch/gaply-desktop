// Gaply — the Statistics window's Help (Research OS, Phase 8P, decision 453).
// Methods and limits is the Statistics home's content, reused and never copied:
// place decides from Rust's list, and its Limitations, RoadmapList and
// RetiredList draw it. The other two are fixed text. All three use ProjectDialog
// itself, which keeps the focus inside and returns it; it moves out of
// projects/ in 8R.
import React from 'react';
import { Button } from '../../../design-system';
import { isFeatureEnabled } from '../../../config/featureFlags';
import { capabilitySentence, HOME_TEXT, LIFECYCLE_LABELS } from '../capabilityWording';
import type { MethodLimitsView } from '../describeTypes';
import { limitsSentence } from '../flowParts';
import { ProjectDialog } from '../../../design-system/ProjectDialog';
import { Limitations, place, RetiredList, RoadmapList } from '../StatisticsHome';
import type { CapabilityView } from '../ttestTypes';
import type { HelpTopic } from './windowMenus';
import { HELP_CLOSE, HELP_TITLES, HOW_GAPLY_READS, KEYBOARD_LINES } from './windowWording';

export interface HelpDialogProps {
  topic: HelpTopic;
  /** Rust's list, or null while unread. */
  capabilities: CapabilityView[] | null;
  /** The list could not be read. */
  unread: boolean;
  /** This computer's limits, or null without an answer. */
  limits: MethodLimitsView | null;
  onClose: () => void;
}

const MethodEntry: React.FC<{ c: CapabilityView }> = ({ c }) => (
  <section data-testid={`sw-help-method-${c.id}`}>
    <h3 className="gds-card__title">{capabilitySentence(c.titleCode)}</h3>
    <p style={{ margin: 0 }}>{LIFECYCLE_LABELS[c.lifecycle]}</p>
    <p style={{ margin: 0 }}>{capabilitySentence(c.descriptionCode)}</p>
    <Limitations c={c} />
  </section>
);

const Methods: React.FC<Pick<HelpDialogProps, 'capabilities' | 'unread' | 'limits'>> = ({ capabilities, unread, limits }) => {
  const placed = capabilities ? place(capabilities, isFeatureEnabled('statsCheck')) : null;
  return (
    <div className="sw-help" data-testid="sw-help-methods" tabIndex={0}>
      {limits && <p data-testid="sw-limits">{limitsSentence(limits)}</p>}
      {unread && <p role="alert">{HOME_TEXT.unread}</p>}
      {placed && (
        <>
          {placed.cards.map((c) => (
            <MethodEntry key={c.id} c={c} />
          ))}
          {placed.check && <MethodEntry c={placed.check} />}
          {placed.verifier.length > 0 && (
            <section data-testid="sw-help-verifier">
              <h3 className="gds-card__title">Statistical Analysis Verifier</h3>
              <p style={{ margin: 0 }}>{capabilitySentence(placed.verifier[0].descriptionCode)}</p>
              <p style={{ margin: '8px 0 2px' }}>{HOME_TEXT.verifierMethods}</p>
              <ul style={{ margin: 0 }}>
                {placed.verifier.map((m) => (
                  <li key={m.id}>{capabilitySentence(m.titleCode)}</li>
                ))}
              </ul>
            </section>
          )}
          <RoadmapList items={placed.roadmap} />
          <RetiredList items={placed.retired} />
        </>
      )}
    </div>
  );
};

export const HelpDialog: React.FC<HelpDialogProps> = ({ topic, capabilities, unread, limits, onClose }) => (
  <ProjectDialog title={HELP_TITLES[topic]} onClose={onClose} testId="sw-help">
    {topic === 'methods' && <Methods capabilities={capabilities} unread={unread} limits={limits} />}
    {topic === 'file' && (
      <div className="sw-help" data-testid="sw-help-file" tabIndex={0}>
        {HOW_GAPLY_READS}
      </div>
    )}
    {topic === 'keyboard' && (
      <div className="sw-help" data-testid="sw-help-keyboard" tabIndex={0}>
        <ul style={{ margin: 0 }}>
          {KEYBOARD_LINES.map((l) => (
            <li key={l}>{l}</li>
          ))}
        </ul>
      </div>
    )}
    <div className="sw-help-actions">
      <Button data-testid="sw-help-close" onClick={onClose}>
        {HELP_CLOSE}
      </Button>
    </div>
  </ProjectDialog>
);
