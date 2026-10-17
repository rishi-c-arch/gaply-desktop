// Gaply — "Choose your workspace" (Research OS, Phase 3; blueprint §3.1).
//
// Three cards, one per workspace. Each shows three separate facts: whether
// the workspace is available in this build (Rust's descriptor), whether the
// account may use it (useEntitlement, decision 9: shown, not enforced), and
// whether it is the one open now (Rust's workspace_state). The chooser reads
// the state and never decides it: opening a workspace is one call to
// workspace_activate, and Rust says whether it worked.
//
// Behind REACT_APP_FEATURE_WORKSPACES, which hides unfinished UI and is not
// an authorization boundary.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Card } from '../../design-system';
import { useEntitlement, type EntitlementStatus } from '../subscription/entitlement';
import { workspaceBridge, type WorkspaceBridge } from './workspaceBridge';
import { isActive, workspaceRefusal, type WorkspaceDescriptorView, type WorkspaceId, type WorkspaceState } from './workspaceTypes';
import {
  AVAILABILITY_TEXT,
  CHOOSER_HEADING,
  CHOOSER_INTRO,
  continueLabel,
  ENTITLEMENT_TEXT,
  openLabel,
  PHASE_TEXT,
  WORKSPACE_DESCRIPTIONS,
  WORKSPACE_NAMES,
  workspaceSentence,
} from './workspaceWording';
import { WORKSPACE_HOME } from './workspaceRoute';

/** The entitlement key each card asks useEntitlement about. None of them is
 *  in PREMIUM_ONLY, so a session is enough (decision 9). */
const ENTITLEMENT_KEY: Record<WorkspaceId, string> = {
  statistics: 'workspace_statistics',
  compute: 'workspace_compute',
  design: 'workspace_design',
};

export interface WorkspaceChooserProps {
  bridge?: WorkspaceBridge;
  /** Test seam: an entitlement status instead of asking the server. */
  forceEntitlement?: EntitlementStatus;
}

const WorkspaceCardView: React.FC<{
  view: WorkspaceDescriptorView;
  state: WorkspaceState;
  busy: boolean;
  forceEntitlement?: EntitlementStatus;
  onOpen: (id: WorkspaceId) => void;
  onContinue: (id: WorkspaceId) => void;
}> = ({ view, state, busy, forceEntitlement, onOpen, onContinue }) => {
  const entitlement = useEntitlement(ENTITLEMENT_KEY[view.id], undefined, forceEntitlement);
  const available = view.availability === 'available';
  const active = isActive(state, view.id);
  const name = WORKSPACE_NAMES[view.displayNameCode];
  const descId = `ws-desc-${view.id}`;
  const factsId = `ws-facts-${view.id}`;
  return (
    <Card title={name}>
      <div data-testid={`ws-card-${view.id}`} data-available={available ? 'yes' : 'no'}>
        <p id={descId}>{WORKSPACE_DESCRIPTIONS[view.id]}</p>
        <dl id={factsId} style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '2px 12px', margin: '8px 0' }}>
          <dt>Availability</dt>
          <dd data-testid={`ws-availability-${view.id}`}>{AVAILABILITY_TEXT[view.availability]}</dd>
          <dt>Access</dt>
          <dd data-testid={`ws-entitlement-${view.id}`}>{ENTITLEMENT_TEXT[entitlement.status]}</dd>
          <dt>In this window</dt>
          <dd data-testid={`ws-active-${view.id}`}>
            {active ? PHASE_TEXT.active : state.phase !== 'no_workspace' && state.workspace === view.id ? PHASE_TEXT[state.phase] : 'Not open'}
          </dd>
        </dl>
        {active ? (
          <Button data-testid={`ws-continue-${view.id}`} aria-describedby={`${descId} ${factsId}`} onClick={() => onContinue(view.id)}>
            {continueLabel(view.id)}
          </Button>
        ) : (
          <Button
            data-testid={`ws-open-${view.id}`}
            aria-describedby={`${descId} ${factsId}`}
            disabled={!available || busy}
            aria-disabled={!available || busy}
            onClick={() => available && !busy && onOpen(view.id)}
          >
            {available ? openLabel(view.id) : AVAILABILITY_TEXT[view.availability]}
          </Button>
        )}
      </div>
    </Card>
  );
};

export const WorkspaceChooser: React.FC<WorkspaceChooserProps> = ({ bridge: given, forceEntitlement }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => given ?? workspaceBridge(), [given]);
  const heading = useRef<HTMLHeadingElement>(null);
  const [views, setViews] = useState<WorkspaceDescriptorView[] | null>(null);
  const [state, setState] = useState<WorkspaceState>({ phase: 'no_workspace' });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    heading.current?.focus();
  }, []);

  useEffect(() => {
    let alive = true;
    Promise.all([bridge.list(), bridge.state()])
      .then(([list, s]) => {
        if (!alive) return;
        setViews(list);
        setState(s.state);
      })
      .catch(() => alive && setMessage('The workspaces could not be read.'));
    return () => {
      alive = false;
    };
  }, [bridge]);

  const go = useCallback(
    (id: WorkspaceId) => {
      const home = WORKSPACE_HOME[id];
      if (home) navigate(home);
    },
    [navigate]
  );

  const open = useCallback(
    async (id: WorkspaceId) => {
      if (busy) return;
      setBusy(true);
      setMessage(null);
      try {
        const s = await bridge.activate(id);
        setState(s.state);
        if (isActive(s.state, id)) go(id);
      } catch (e) {
        const r = workspaceRefusal(e);
        setMessage(r ? workspaceSentence(r) : 'The workspace could not be opened.');
        bridge.state().then((s) => setState(s.state), () => undefined);
      } finally {
        setBusy(false);
      }
    },
    [bridge, busy, go]
  );

  return (
    <div className="gds-root" style={{ minHeight: '100vh', padding: 24 }} data-testid="workspace-chooser">
      <h1 ref={heading} tabIndex={-1} data-testid="ws-chooser-heading">
        {CHOOSER_HEADING}
      </h1>
      <p>{CHOOSER_INTRO}</p>
      <p role="status" aria-live="polite" data-testid="ws-chooser-message">
        {message ?? ''}
      </p>
      {views && (
        <section aria-label={CHOOSER_HEADING} style={{ display: 'grid', gap: 16, gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))' }}>
          {views.map((v) => (
            <WorkspaceCardView
              key={v.id}
              view={v}
              state={state}
              busy={busy}
              forceEntitlement={forceEntitlement}
              onOpen={open}
              onContinue={go}
            />
          ))}
        </section>
      )}
    </div>
  );
};

export default WorkspaceChooser;
