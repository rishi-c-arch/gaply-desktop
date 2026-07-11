// Set 7 — TauriChatClient: the production chat path hands the turn to the
// Rust backend (`run_copilot_chat`), where the code-enforced firewall lives.
// These tests prove the invoke wiring + honest turn mapping (mocked invoke).
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { RustChatTurn, TauriChatClient } from './chatBridge';
import { ChatProxyPayload } from './chatContext';
import { SYSTEM_PROMPT } from './chatGuards';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

const PAYLOAD: ChatProxyPayload = {
  task: 'research_copilot',
  system: SYSTEM_PROMPT,
  language: 'en',
  question: 'which finding is most severe?',
  findings: [
    { agent: 'validation_maths', tier: 'mathematically_certain', severity: 'critical', summary: 't-test for 3 groups', provenance: ['rule:TestGroupMismatch (CRITICAL)'] },
  ],
  rag: [{ source: 'Strong methods', text: 'ANOVA fits 3+ group designs.' }],
  citations: [{ title: 'Sleep and memory', doi: '10.1/z', retracted: false }],
};

function turn(overrides: Partial<RustChatTurn> = {}): RustChatTurn {
  return {
    kind: 'answered',
    answer: 'The critical validation finding is the most severe.',
    provenance: ['rule:TestGroupMismatch (CRITICAL)'],
    ai_assessed: false,
    warnings: [],
    available: true,
    ...overrides,
  };
}

beforeEach(() => invoke.mockReset());

describe('TauriChatClient', () => {
  it('invokes run_copilot_chat with the structured context + question (never the system prompt or manuscript)', async () => {
    invoke.mockResolvedValue(turn());
    await new TauriChatClient().ask(PAYLOAD);
    expect(invoke).toHaveBeenCalledWith('run_copilot_chat', {
      context: { findings: PAYLOAD.findings, rag: PAYLOAD.rag, citations: PAYLOAD.citations },
      question: PAYLOAD.question,
      language: 'en',
    });
    // The frontend system prompt is NOT sent — the Rust backend pins its own
    // instruction; a spoofed frontend string can't weaken the firewall.
    expect(JSON.stringify(invoke.mock.calls[0][1])).not.toContain(SYSTEM_PROMPT.slice(0, 40));
  });

  it('maps an answered turn to ChatResponse', async () => {
    invoke.mockResolvedValue(turn({ ai_assessed: true }));
    const res = await new TauriChatClient().ask(PAYLOAD);
    expect(res.answer).toMatch(/most severe/);
    expect(res.provenance).toEqual(['rule:TestGroupMismatch (CRITICAL)']);
    expect(res.aiAssessed).toBe(true);
  });

  it('surfaces the honest needs-cloud answer when the backend is offline', async () => {
    invoke.mockResolvedValue(
      turn({ kind: 'unavailable', available: false, answer: 'Answering questions needs Gaply’s cloud connection…', provenance: [] })
    );
    const res = await new TauriChatClient().ask(PAYLOAD);
    expect(res.answer).toMatch(/cloud connection/);
    expect(res.provenance).toEqual([]);
  });

  it('surfaces the backend refusal verbatim (firewall backstop below the UI guard)', async () => {
    invoke.mockResolvedValue(
      turn({ kind: 'refused_ghostwriting', answer: "I can't write your abstract for you — Gaply protects research integrity…", provenance: [] })
    );
    const res = await new TauriChatClient().ask(PAYLOAD);
    expect(res.answer).toMatch(/can't write/);
  });
});
