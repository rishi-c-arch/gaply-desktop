// Gaply — what the workspace screens say (Research OS, Phase 3). The card
// texts are the blueprint's §3.1. Every closed code the workspace commands
// return has one sentence here, keyed by the code list Rust pins
// (workspaceCodes.json). No sentence is built from data.
import type { EntitlementStatus } from '../subscription/entitlement';
import type { Availability, DisplayNameCode, WorkspaceCode, WorkspaceId, WorkspacePhase, WorkspaceRefusal } from './workspaceTypes';

export const CHOOSER_HEADING = 'Choose your workspace';
export const CHOOSER_INTRO = 'Open one workspace at a time. You can switch later.';

export const WORKSPACE_NAMES: Record<DisplayNameCode, string> = {
  'workspace.name.statistics': 'Statistics',
  'workspace.name.compute': 'Compute',
  'workspace.name.design': 'Design',
};

/** Blueprint §3.1, word for word. */
export const WORKSPACE_DESCRIPTIONS: Record<WorkspaceId, string> = {
  statistics: 'Analyze datasets and verify research results.',
  compute: 'Solve equations, build numerical models, and explore signals.',
  design: 'Draft, model, and validate engineering geometry.',
};

export const NAME_OF: Record<WorkspaceId, string> = {
  statistics: 'Statistics',
  compute: 'Compute',
  design: 'Design',
};

export const AVAILABILITY_TEXT: Record<Availability, string> = {
  available: 'Available',
  missing: 'Coming later',
  incompatible: 'Not available on this computer',
};

export const ENTITLEMENT_TEXT: Record<EntitlementStatus, string> = {
  checking: 'Checking access…',
  signed_out: 'Sign in to check access',
  entitled: 'Included with your account',
  not_entitled: 'Not included in your plan',
  offline_unverified: 'Access cannot be checked right now',
  unverifiable_no_account: 'This build has no account service',
};

export const PHASE_TEXT: Record<WorkspacePhase, string> = {
  no_workspace: 'No workspace is open',
  activating: 'Opening',
  active: 'Open',
  deactivating: 'Closing',
};

export const openLabel = (id: WorkspaceId) => `Open ${NAME_OF[id]}`;
export const continueLabel = (id: WorkspaceId) => `Continue in ${NAME_OF[id]}`;

export const SHELL_TEXT = {
  switch: 'Switch workspace',
  recent: 'Recent items',
  recentEmpty: 'Nothing here yet. Items you open will be listed here.',
  jobs: 'Jobs',
  jobsNone: 'No run yet in this workspace.',
  settings: 'Settings',
  tools: 'Tools',
  notActive: 'Statistics is not open in this window.',
  choose: 'Choose a workspace',
};

export const SWITCH_DIALOG = {
  title: 'A run is still going',
  body: 'Statistics cannot be closed while this run is queued or running. Wait for it to end, or cancel it.',
  wait: 'Wait, then switch',
  cancel: 'Cancel the run, then switch',
  stay: 'Stay here',
  waiting: 'Waiting for the run to end…',
};

export const GATE_TEXT = {
  heading: 'This is part of the Statistics workspace',
  body: 'Open the Statistics workspace to use the descriptive summary.',
  open: 'Open the Statistics workspace',
};

const WORDS: Record<WorkspaceCode, (r: WorkspaceRefusal) => string> = {
  'workspace.unavailable': (r) => `${name(r.params.workspace)} is not available in this version of Gaply.`,
  'workspace.not_ready': (r) => `${name(r.params.workspace)} could not be made ready, so it was not opened.`,
  'workspace.inactive': (r) => `This needs the ${name(r.params.expected)} workspace, which is not open.`,
  'workspace.wrong_workspace': (r) =>
    `This belongs to the ${name(r.params.found)} workspace, but ${name(r.params.expected)} is the one open.`,
  'workspace.run_active': (r) =>
    `Run ${r.params.run ?? '?'} is still queued or running, so the workspace cannot be closed yet.`,
  // While a workspace is opening nothing can be closed, so it says to wait
  // (evaluation of Phases 2 and 3, F15).
  'workspace.illegal_transition': (r) =>
    !r.params.workspace
      ? 'That cannot be done from here.'
      : r.params.from === 'activating'
        ? `${name(r.params.workspace)} is still opening. Wait for it to open, then try again.`
        : `${name(r.params.workspace)} is ${PHASE_TEXT[r.params.from ?? 'active'].toLowerCase()}. Close it first.`,
};

function name(id: WorkspaceId | undefined): string {
  return id ? NAME_OF[id] : 'That workspace';
}

/** One sentence for a workspace refusal. */
export function workspaceSentence(r: WorkspaceRefusal): string {
  const say = WORDS[r.code];
  return say ? say(r) : 'That could not be done.';
}

/** The codes this file has a sentence for. */
export const WORDED_CODES = Object.keys(WORDS).sort();
