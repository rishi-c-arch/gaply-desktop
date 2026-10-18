// Gaply — the gate in front of a screen that needs a workspace (Research OS,
// Phase 3). It shows the screen only while Rust says that workspace is
// active; otherwise it says so and offers to open it. The gate never decides
// the state: opening is one explicit call to workspace_activate after the
// user's click, and Rust's checks stand whatever this shows.
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button, Card } from '../../design-system';
import { isFeatureEnabled } from '../../config/featureFlags';
import { workspaceBridge, type WorkspaceBridge } from './workspaceBridge';
import { CHOOSER_PATH } from './workspaceRoute';
import { isActive, workspaceRefusal, type WorkspaceId } from './workspaceTypes';
import { GATE_TEXT, SHELL_TEXT, workspaceSentence } from './workspaceWording';

export interface RequireWorkspaceProps {
  id: WorkspaceId;
  bridge?: WorkspaceBridge;
  /** The gate's sentence for the screen behind it (Phase 4B-2: the t-test
   *  has its own); the descriptive summary's by default. */
  body?: string;
  children: React.ReactNode;
}

export const RequireWorkspace: React.FC<RequireWorkspaceProps> = ({ id, bridge: given, body = GATE_TEXT.body, children }) => {
  const navigate = useNavigate();
  const bridge = useMemo(() => given ?? workspaceBridge(), [given]);
  const [active, setActive] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    bridge.state().then(
      (s) => alive && setActive(isActive(s.state, id)),
      () => alive && setActive(false)
    );
    return () => {
      alive = false;
    };
  }, [bridge, id]);

  const open = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    setMessage(null);
    try {
      const s = await bridge.activate(id);
      setActive(isActive(s.state, id));
    } catch (e) {
      const r = workspaceRefusal(e);
      setMessage(r ? workspaceSentence(r) : 'The workspace could not be opened.');
    } finally {
      setBusy(false);
    }
  }, [bridge, busy, id]);

  if (active === null) return null;
  if (active) return <>{children}</>;
  return (
    <div className="gds-root" style={{ minHeight: '100vh', padding: 24 }} data-testid="ws-gate">
      <Card title={GATE_TEXT.heading}>
        <p>{body}</p>
        <p role="status" aria-live="polite" data-testid="ws-gate-message">
          {message ?? ''}
        </p>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <Button data-testid="ws-gate-open" onClick={open} disabled={busy}>
            {GATE_TEXT.open}
          </Button>
          {isFeatureEnabled('workspaces') && (
            <Button variant="secondary" data-testid="ws-gate-choose" onClick={() => navigate(CHOOSER_PATH)}>
              {SHELL_TEXT.choose}
            </Button>
          )}
        </div>
      </Card>
    </div>
  );
};
