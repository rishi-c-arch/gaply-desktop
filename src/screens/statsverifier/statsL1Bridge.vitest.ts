// Level 1 bridge (Phase 2, Step 6): each method invokes the Rust command of
// that name with the argument keys its signature takes (Tauri maps camelCase
// to snake_case), and nothing else crosses: no key comes back, and the mock
// records the key as redacted.
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { makeMockStatsL1Bridge, TauriStatsL1Bridge } from './statsL1Bridge';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
});

describe('TauriStatsL1Bridge', () => {
  it('invokes each command with the argument keys the Rust signature takes', async () => {
    const b = new TauriStatsL1Bridge();
    const h = { run: 1, id: 2 };
    await b.open('/m.txt', '/d.csv');
    await b.editClaim(1, 0, { family: 't', statistic: '2.1', df: ['6'], p: '.04' });
    await b.addClaim(1, { family: 'F', statistic: '3.2', df: ['2', '30'], p: null });
    await b.proposals(1);
    await b.helperPreview(1, { task: 'binding_suggestion', claimId: 0, methodId: 'independent_t' }, ['column_headers']);
    await b.helperSend(1, h);
    await b.helperRevoke(1, 'claim_method');
    await b.helperCancel(1);
    await b.confirmBinding(1, h);
    await b.compute(1, ['a'.repeat(64)]);
    await b.close(1);
    await b.usage(1);
    await b.settingGet();
    await b.settingSet({ aiHelp: true, webContext: false, model: 'gpt-5.5' });
    await b.keyStatus();
    await b.keySave('KEY_SECRET_a1b2');
    await b.keyTest();
    await b.keyRemove();
    await b.openSourceLink('https://www.bmj.com/x');

    const seen = invoke.mock.calls.map(([cmd, args]) => [cmd, Object.keys(args ?? {}).sort()]);
    expect(seen).toEqual([
      ['stats_l1_open', ['data', 'manuscript']],
      ['stats_l1_edit_claim', ['claim', 'claimId', 'run']],
      ['stats_l1_add_claim', ['claim', 'run']],
      ['stats_l1_proposals', ['run']],
      ['stats_l1_helper_preview', ['categories', 'input', 'run']],
      ['stats_l1_helper_send', ['preview', 'run']],
      ['stats_l1_helper_revoke', ['run', 'task']],
      ['stats_l1_helper_cancel', ['run']],
      ['stats_l1_confirm_binding', ['edits', 'proposal', 'run']],
      ['stats_l1_compute', ['run', 'tokens']],
      ['stats_l1_close', ['run']],
      ['stats_ai_usage', ['run']],
      ['stats_ai_setting_get', []],
      ['stats_ai_setting_set', ['setting']],
      ['stats_ai_key_status', []],
      ['stats_ai_key_save', ['key']],
      ['stats_ai_key_test', []],
      ['stats_ai_key_remove', []],
      ['stats_open_source_link', ['url']],
    ]);
    // The helper input is the tagged enum Rust deserialises.
    expect(invoke.mock.calls[4][1].input).toEqual({ task: 'binding_suggestion', claimId: 0, methodId: 'independent_t' });
    // Confirming without edits sends an explicit null.
    expect(invoke.mock.calls[8][1].edits).toBeNull();
  });
});

describe('makeMockStatsL1Bridge', () => {
  it('records calls and never records the key', async () => {
    const m = makeMockStatsL1Bridge({ stats_ai_key_save: { saved: true, provider: 'openai' } });
    await expect(m.keySave('KEY_SECRET_a1b2')).resolves.toEqual({ saved: true, provider: 'openai' });
    expect(JSON.stringify(m.calls)).not.toContain('KEY_SECRET_a1b2');
    await expect(m.compute(1, [])).rejects.toThrow('no mock response');
  });
});
