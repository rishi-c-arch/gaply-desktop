// Gaply — Level 1 bridge (Phase 2, Step 6). Calls the thin Rust commands in
// src-tauri/src/stats_l1_commands.rs. Rust owns every decision: the webview
// passes paths, ids, the user's ticks and edits, and displays what comes back.
// It never reaches OpenAI (the CSP's connect-src has no OpenAI host), never
// holds the key (saveKey sends it once; nothing returns it), and opens a
// model-supplied link only through openSourceLink, which Rust rechecks.
import { isTauri } from '../../utils/isTauri';
import {
  AiKeyStatusView,
  AiKeyTestView,
  AiSettings,
  BindingEdits,
  HelperTaskInput,
  L1ClaimInput,
  L1ConfirmView,
  L1Handle,
  L1HelperResultView,
  L1OpenView,
  L1PreviewView,
  L1ProposalView,
  L1RevisionView,
  L1UsageView,
  OptInCategory,
  ReproductionView,
} from './statsL1Types';

export interface StatsL1Bridge {
  open(manuscript: string, data: string): Promise<L1OpenView>;
  editClaim(run: number, claimId: number, claim: L1ClaimInput): Promise<L1RevisionView>;
  addClaim(run: number, claim: L1ClaimInput): Promise<L1RevisionView>;
  proposals(run: number): Promise<L1ProposalView[]>;
  /** The exact request, before anything is sent. No network. */
  helperPreview(run: number, input: HelperTaskInput, categories: OptInCategory[]): Promise<L1PreviewView>;
  /** Clicking Send on a preview: the consent for its task and categories. */
  helperSend(run: number, preview: L1Handle): Promise<L1HelperResultView>;
  helperRevoke(run: number, task: string): Promise<boolean>;
  helperCancel(run: number): Promise<void>;
  confirmBinding(run: number, proposal: L1Handle, edits?: BindingEdits): Promise<L1ConfirmView>;
  compute(run: number, tokens: string[]): Promise<ReproductionView>;
  close(run: number): Promise<boolean>;
  usage(run: number): Promise<L1UsageView>;
  settingGet(): Promise<AiSettings>;
  settingSet(setting: AiSettings): Promise<AiSettings>;
  keyStatus(): Promise<AiKeyStatusView>;
  keySave(key: string): Promise<AiKeyStatusView>;
  keyTest(): Promise<AiKeyTestView>;
  keyRemove(): Promise<AiKeyStatusView>;
  openSourceLink(url: string): Promise<void>;
}

export class TauriStatsL1Bridge implements StatsL1Bridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    if (!isTauri) throw new Error('The Statistical Analysis Verifier runs in the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(cmd, args);
  }

  open(manuscript: string, data: string) {
    return this.invoke<L1OpenView>('stats_l1_open', { manuscript, data });
  }
  editClaim(run: number, claimId: number, claim: L1ClaimInput) {
    return this.invoke<L1RevisionView>('stats_l1_edit_claim', { run, claimId, claim });
  }
  addClaim(run: number, claim: L1ClaimInput) {
    return this.invoke<L1RevisionView>('stats_l1_add_claim', { run, claim });
  }
  proposals(run: number) {
    return this.invoke<L1ProposalView[]>('stats_l1_proposals', { run });
  }
  helperPreview(run: number, input: HelperTaskInput, categories: OptInCategory[]) {
    return this.invoke<L1PreviewView>('stats_l1_helper_preview', { run, input, categories });
  }
  helperSend(run: number, preview: L1Handle) {
    return this.invoke<L1HelperResultView>('stats_l1_helper_send', { run, preview });
  }
  helperRevoke(run: number, task: string) {
    return this.invoke<boolean>('stats_l1_helper_revoke', { run, task });
  }
  helperCancel(run: number) {
    return this.invoke<void>('stats_l1_helper_cancel', { run });
  }
  confirmBinding(run: number, proposal: L1Handle, edits?: BindingEdits) {
    return this.invoke<L1ConfirmView>('stats_l1_confirm_binding', { run, proposal, edits: edits ?? null });
  }
  compute(run: number, tokens: string[]) {
    return this.invoke<ReproductionView>('stats_l1_compute', { run, tokens });
  }
  close(run: number) {
    return this.invoke<boolean>('stats_l1_close', { run });
  }
  usage(run: number) {
    return this.invoke<L1UsageView>('stats_ai_usage', { run });
  }
  settingGet() {
    return this.invoke<AiSettings>('stats_ai_setting_get');
  }
  settingSet(setting: AiSettings) {
    return this.invoke<AiSettings>('stats_ai_setting_set', { setting });
  }
  keyStatus() {
    return this.invoke<AiKeyStatusView>('stats_ai_key_status');
  }
  keySave(key: string) {
    return this.invoke<AiKeyStatusView>('stats_ai_key_save', { key });
  }
  keyTest() {
    return this.invoke<AiKeyTestView>('stats_ai_key_test');
  }
  keyRemove() {
    return this.invoke<AiKeyStatusView>('stats_ai_key_remove');
  }
  openSourceLink(url: string) {
    return this.invoke<void>('stats_open_source_link', { url });
  }
}

/** Test/dev double: records every call and its arguments (the key is recorded
 *  as `<redacted>`), and returns what `responses` holds for the command. */
export function makeMockStatsL1Bridge(
  responses: Partial<Record<string, unknown>> = {}
): StatsL1Bridge & { calls: Array<[string, unknown]> } {
  const calls: Array<[string, unknown]> = [];
  const call = <T>(cmd: string, args: unknown): Promise<T> => {
    calls.push([cmd, args]);
    if (!(cmd in responses)) return Promise.reject(new Error(`no mock response for ${cmd}`));
    return Promise.resolve(responses[cmd] as T);
  };
  return {
    calls,
    open: (manuscript, data) => call('stats_l1_open', { manuscript, data }),
    editClaim: (run, claimId, claim) => call('stats_l1_edit_claim', { run, claimId, claim }),
    addClaim: (run, claim) => call('stats_l1_add_claim', { run, claim }),
    proposals: (run) => call('stats_l1_proposals', { run }),
    helperPreview: (run, input, categories) => call('stats_l1_helper_preview', { run, input, categories }),
    helperSend: (run, preview) => call('stats_l1_helper_send', { run, preview }),
    helperRevoke: (run, task) => call('stats_l1_helper_revoke', { run, task }),
    helperCancel: (run) => call('stats_l1_helper_cancel', { run }),
    confirmBinding: (run, proposal, edits) => call('stats_l1_confirm_binding', { run, proposal, edits: edits ?? null }),
    compute: (run, tokens) => call('stats_l1_compute', { run, tokens }),
    close: (run) => call('stats_l1_close', { run }),
    usage: (run) => call('stats_ai_usage', { run }),
    settingGet: () => call('stats_ai_setting_get', {}),
    settingSet: (setting) => call('stats_ai_setting_set', { setting }),
    keyStatus: () => call('stats_ai_key_status', {}),
    keySave: () => call('stats_ai_key_save', { key: '<redacted>' }),
    keyTest: () => call('stats_ai_key_test', {}),
    keyRemove: () => call('stats_ai_key_remove', {}),
    openSourceLink: (url) => call('stats_open_source_link', { url }),
  };
}
