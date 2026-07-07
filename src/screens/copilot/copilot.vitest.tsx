// F11 — Research Copilot chat tests. Mocked LLM, no network.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import ResearchCopilotPanel from './ResearchCopilotPanel';
import { makeMockChat } from './chatBridge';
import { buildChatPayload, ChatContext } from './chatContext';
import { detectGhostwriting, isGhostwritingRequest, SYSTEM_PROMPT } from './chatGuards';
import { detectLanguage } from './detectLanguage';
import { PublishReadyReport } from '../report/reportTypes';

afterEach(cleanup);

const RAW = 'RAW_MANUSCRIPT_TEXT_SENTINEL';

const REPORT: PublishReadyReport = {
  verdict: 'concern',
  combined_confidence: 1,
  findings: [
    { severity: 'critical', tier: 'mathematically_certain', certainty_label: 'mathematically certain', agent: 'validation_maths', title: 'rule failed: test-group mismatch (t-test for 3 groups)', detail: `In "${RAW}" the authors used a t-test.`, confidence: 1, provenance: ['rule:TestGroupMismatch (CRITICAL)', 'agent:validation_maths (deterministic)'], section: 'Methods' },
    { severity: 'minor', tier: 'ai_assessed_moderate', certainty_label: 'AI-assessed, moderate confidence', agent: 'ai_detection', title: 'discussion leans AI-like', detail: RAW, confidence: 0.6, provenance: ['signal:leans_ai_like'] },
  ],
  checklist: [],
  debate: { rounds_run: 1, converged: true, overridden_by_constraint: true, rejected_agents: [], revised_agents: [] },
  disclaimer: 'never definitive proof',
};

const CTX: ChatContext = {
  report: REPORT,
  ragSnippets: [{ source: 'Strong methods', url: 'https://x', text: 'ANOVA fits 3+ group designs.' }],
  citations: [{ title: 'Sleep and memory', doi: '10.1/z', retracted: false }],
};

function renderChat(client = makeMockChat()) {
  render(<ResearchCopilotPanel context={CTX} client={client} />);
  return client;
}
function ask(text: string) {
  fireEvent.change(screen.getByTestId('chat-input'), { target: { value: text } });
  fireEvent.click(screen.getByTestId('chat-send'));
}

/* --------------------------- ghostwriting refusal ----------------------- */

describe('structurally incapable of writing the paper', () => {
  it('"write my abstract" is refused with an educational redirect (no cloud call)', async () => {
    const client = renderChat();
    ask('Please write my abstract for me');
    const refusal = await screen.findByTestId('msg-assistant-refused');
    expect(refusal.textContent).toMatch(/can’t write your abstract|guides, it doesn’t ghostwrite/i);
    expect(refusal.textContent).toMatch(/what a strong abstract needs/i);
    expect(client.payloads.length).toBe(0); // never hit the cloud
  });

  it('isGhostwritingRequest catches write/draft/rewrite intents', () => {
    for (const q of ['write my introduction', 'draft the methods section', 'rewrite my abstract', 'paraphrase this paragraph for me']) {
      expect(isGhostwritingRequest(q)).toBe(true);
    }
    for (const q of ['why was my methods section flagged?', 'how do I improve my abstract?', 'what should I read?']) {
      expect(isGhostwritingRequest(q)).toBe(false);
    }
  });
});

/* --------------------------- grounded guidance -------------------------- */

describe('grounded guidance', () => {
  it('"why was X flagged / how to fix" returns grounded guidance citing the finding', async () => {
    const client = makeMockChat((p) => ({
      answer: 'It was flagged because a t-test does not fit a 3-group design; use a one-way ANOVA and report effect sizes.',
      provenance: p.findings[0].provenance,
      aiAssessed: false,
    }));
    renderChat(client);
    ask('Why was my methods section flagged and how do I fix it?');
    await screen.findByTestId('msg-assistant');
    expect(screen.getByText(/one-way ANOVA/)).toBeTruthy();
    // provenance chips render, grounded in the finding
    expect(screen.getByTestId('msg-provenance').textContent).toMatch(/rule:TestGroupMismatch/);
  });

  it('AI-assessed answers carry the uncertainty disclaimer', async () => {
    const client = makeMockChat((p) => ({ answer: 'The discussion leans AI-like — review it.', provenance: ['signal:leans_ai_like'], aiAssessed: true }));
    renderChat(client);
    ask('Is my discussion AI-written?');
    await screen.findByTestId('msg-disclaimer');
    expect(screen.getByTestId('msg-disclaimer').textContent).toMatch(/not definitive proof/i);
  });
});

/* --------------------- privacy: no manuscript text ---------------------- */

describe('privacy', () => {
  it('the proxy payload contains structured summaries only, never raw manuscript text', () => {
    const payload = buildChatPayload(CTX, 'why flagged?', 'en');
    const s = JSON.stringify(payload);
    expect(s).not.toContain(RAW); // finding.detail (which quotes the manuscript) is excluded
    expect(s).toContain('rule failed: test-group mismatch'); // structured summary present
    expect(payload.system).toBe(SYSTEM_PROMPT);
    expect(payload.findings[0].summary).not.toContain(RAW);
  });

  it('the running chat never sends raw manuscript text', async () => {
    const client = renderChat();
    ask('why was this flagged?');
    await screen.findByTestId('msg-assistant');
    expect(JSON.stringify(client.payloads)).not.toContain(RAW);
  });
});

/* ------------------------------ multilingual ---------------------------- */

describe('multilingual', () => {
  it('a non-English question gets a same-language answer (mocked LLM)', async () => {
    const client = makeMockChat((p) => ({
      answer: p.language === 'hi' ? 'आपका मेथड्स सेक्शन इसलिए फ्लैग हुआ...' : 'English answer',
      provenance: [],
      aiAssessed: false,
    }));
    renderChat(client);
    ask('मेरा मेथड्स सेक्शन क्यों फ्लैग हुआ?'); // Hindi
    await screen.findByTestId('msg-assistant');
    // payload language detected as Hindi; answer is in Hindi
    expect(client.payloads[0].language).toBe('hi');
    expect(screen.getByText(/आपका मेथड्स/)).toBeTruthy();
  });

  it('detectLanguage recognizes scripts', () => {
    expect(detectLanguage('why was this flagged?')).toBe('en');
    expect(detectLanguage('मेरा पेपर')).toBe('hi');
    expect(detectLanguage('¿por qué mi artículo?')).toBe('es');
  });
});

/* ----------------------- response-side ghostwrite guard ----------------- */

describe('response-side ghostwriting guard', () => {
  it('blocks a mocked drafted-prose output', async () => {
    const drafted =
      'In this study, we investigate the effect of extended sleep on working memory. ' +
      'Ninety-six participants were recruited and randomized into two groups, and the intervention ' +
      'was delivered over a four-week period with careful attention to adherence and dropout.\n\n' +
      'Our results demonstrate a statistically significant improvement in recall among the ' +
      'extended-sleep group. These findings replicate prior work and support a causal role for ' +
      'sleep in memory consolidation, with implications for both theory and clinical practice.';
    const client = makeMockChat(() => ({ answer: drafted, provenance: [], aiAssessed: false }));
    renderChat(client);
    ask('help me strengthen my paper'); // benign request; model drifts into ghostwriting
    const blocked = await screen.findByTestId('msg-assistant-blocked');
    expect(blocked.textContent).toMatch(/doesn’t ghostwrite/i);
  });

  it('detectGhostwriting flags paper-voice drafts, passes short guidance', () => {
    expect(detectGhostwriting('Focus on stating your gap clearly; ANOVA fits 3 groups.').blocked).toBe(false);
    const draft = 'In this paper we present a novel method. '.repeat(20) + '\n\n' + 'Our results demonstrate that the approach works well across settings. '.repeat(20);
    expect(detectGhostwriting(draft).blocked).toBe(true);
  });
});
