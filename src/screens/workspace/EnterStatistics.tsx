// Gaply — opening the Stats Check or the Stats Verifier opens the Statistics
// workspace (Research OS, Phase 4A, decision 49). The chooser is behind a
// flag that is off, so entering one of these screens is the user's choice of
// workspace: in the desktop app, this reads workspace_state and calls
// workspace_activate only when Statistics is not already active, then shows
// the screen. Rust's checks stand whatever this shows: the verifier's
// commands refuse with workspace.inactive until Statistics is active.
//
// * One activation in flight at a time, for every screen: a single shared
//   promise, so React StrictMode's double effect makes one call.
// * The screen's body renders only once Statistics is Active, so no child
//   calls a command while the workspace is Activating.
// * A refusal shows its code's sentence from the wording table.
// * In the browser build (no Tauri) the screen renders as before, with no
//   call at all.
import React, { useEffect, useMemo, useState } from 'react';
import { isTauri } from '../../utils/isTauri';
import { workspaceBridge, type WorkspaceBridge } from './workspaceBridge';
import { openProjectStore } from '../statistics/projects/projectSession';
import { isActive, workspaceRefusal, type WorkspaceStateView } from './workspaceTypes';
import { ENTER_TEXT, workspaceSentence } from './workspaceWording';

/** How often, and how many times, to re-read the state while another
 *  activation of Statistics is under way. */
const ACTIVATING_POLL_MS = 100;
const ACTIVATING_POLLS = 50;

let inFlight: Promise<WorkspaceStateView> | null = null;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Statistics active, through at most one workspace_activate at a time. It
 *  reads the state first and activates only from no workspace; while
 *  Statistics is already opening it waits for that activation instead. */
export function enterStatistics(bridge: WorkspaceBridge): Promise<WorkspaceStateView> {
  if (!inFlight) {
    inFlight = (async () => {
      let now = await bridge.state();
      for (let i = 0; i < ACTIVATING_POLLS && now.state.phase === 'activating' && now.state.workspace === 'statistics'; i++) {
        await sleep(ACTIVATING_POLL_MS);
        now = await bridge.state();
      }
      if (isActive(now.state, 'statistics')) return now;
      const activated = await bridge.activate('statistics');
      // Activating closes any open project in Rust (decision 122).
      openProjectStore.clear();
      return activated;
    })().finally(() => {
      inFlight = null;
    });
  }
  return inFlight;
}

export interface EnterStatisticsProps {
  /** Test seam: the bridge to use. */
  bridge?: WorkspaceBridge;
  /** Test seam: whether this is the desktop app. */
  desktop?: boolean;
  children: React.ReactNode;
}

type Phase = 'opening' | 'active' | 'refused';

export const EnterStatistics: React.FC<EnterStatisticsProps> = ({ bridge: given, desktop = isTauri, children }) => {
  const bridge = useMemo(() => (desktop ? given ?? workspaceBridge() : null), [desktop, given]);
  const [phase, setPhase] = useState<Phase>(desktop ? 'opening' : 'active');
  const [message, setMessage] = useState<string>(ENTER_TEXT.opening);

  useEffect(() => {
    if (!bridge) return;
    let alive = true;
    enterStatistics(bridge).then(
      (s) => {
        if (!alive) return;
        if (isActive(s.state, 'statistics')) {
          setPhase('active');
        } else {
          setPhase('refused');
          setMessage(ENTER_TEXT.notOpened);
        }
      },
      (e) => {
        if (!alive) return;
        const r = workspaceRefusal(e);
        setPhase('refused');
        setMessage(r ? workspaceSentence(r) : ENTER_TEXT.notOpened);
      }
    );
    return () => {
      alive = false;
    };
  }, [bridge]);

  if (phase === 'active') return <>{children}</>;
  return (
    <div className="gds-root" style={{ minHeight: '100vh', padding: 24 }} data-testid="ws-enter">
      <p role="status" aria-live="polite" data-testid="ws-enter-message">
        {message}
      </p>
    </div>
  );
};

export default EnterStatistics;
