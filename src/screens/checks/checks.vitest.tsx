// F7 — single-agent check screens. Mocked bridges/clients, no real Tauri, no
// network. Each screen renders its agent's output through the F6 viewer.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import PlagiarismCheckPage from './PlagiarismCheckPage';
import AiCheckPage, { STAGES, EVENT_STAGE } from './AiCheckPage';
import StatsCheckPage from './StatsCheckPage';
import { makeMockCheckBridge } from './checkBridge';
import {
  plagiarismToReport,
  aiToReport,
  validationToReport,
} from './adapters';
import {
  AiCheckMemoryStatus,
  AiCheckResult,
  AiDetectionReport,
  PlagiarismReport,
  StatsValidityReport,
} from './agentTypes';
import { AICHECK_FIXTURE } from './aicheckFixture';
import { readVerifyCitations, setVerifyCitations, setCloudConsent } from '../settings/settingsStore';

// Exercise the RUNNABLE plagiarism path: mock the free-check flags ON (Set 1
// ships them OFF — see featureGates.vitest.tsx for the coming-soon/OFF behavior).
vi.mock('../../config/Feature', () => ({
  useFeatureFlag: (n: string) => n === 'plagiarismCheck',
  Feature: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const RAW_MANUSCRIPT_SENTINEL = 'SECRET_MANUSCRIPT_BODY_DO_NOT_LEAK';

function auth(session: any = null): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

/* -------------------------------- fixtures ------------------------------- */

const PLAG: PlagiarismReport = {
  chunk_count: 12,
  threshold: 0.8,
  corpus_matches: [
    {
      manuscript_chunk_seq: 3,
      manuscript_excerpt: 'sleep supports memory consolidation',
      similarity: 0.91,
      source: { kind: 'corpus', document_id: 1, chunk_id: 9, title: 'Prior Review 2019', source_url: 'https://x', source_type: 'journal', excerpt: '...' },
    },
  ],
  self_matches: [
    { manuscript_chunk_seq: 7, manuscript_excerpt: 'as noted above', similarity: 0.99, source: { kind: 'self_manuscript', other_chunk_seq: 2, excerpt: '...' } },
  ],
  note: 'Per-session isolated store; shared corpus read-only.',
};

const AI: AiDetectionReport = {
  model: 'HeuristicModel',
  overall_mean_perplexity: 42.1,
  overall_burstiness: 3.2,
  signal: 'leans_ai_like',
  confidence: 'low',
  disclaimer: 'This is a statistical signal, not proof of misconduct.',
  sections: [
    { section: 'discussion', sentence_count: 12, mean_perplexity: 21.0, burstiness: 1.1, signal: 'leans_ai_like', uncertainty: 'Low confidence; short section.' },
    { section: 'methods', sentence_count: 8, mean_perplexity: 55.0, burstiness: 6.0, signal: 'leans_human_like', uncertainty: 'Low confidence.' },
  ],
};

const STATS_FAIL: StatsValidityReport = {
  passed: false,
  checks: [],
  flags: [
    { rule: 'missing_effect_size', severity: 'MAJOR', location: { section: 'results', paragraph: 2 }, explanation: 'p-value without an effect size.' },
    { rule: 'test_group_mismatch', severity: 'CRITICAL', location: { section: 'methods', paragraph: 1 }, explanation: 't-test used for 3+ groups.' },
  ],
};

function renderScreen(node: React.ReactElement, session: any = null) {
  return render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>{node}</GaplySessionProvider>
    </MemoryRouter>
  );
}

async function runFile(name = 'paper.pdf') {
  fireEvent.change(await screen.findByTestId('file-input'), {
    target: { files: [new File(['x'], name, { type: 'application/pdf' })] },
  });
  await screen.findByTestId('selected-name');
  fireEvent.click(screen.getByTestId('run-check'));
  await screen.findByTestId('check-report');
}

/* ------------------------------ each renders ----------------------------- */
// The Plagiarism page (exact + similar lanes, library manager, side-by-side
// report) is covered in depth by plagiarism.vitest.tsx.

describe('AI Check', () => {
  it('renders the two-way tiered report with the mandatory caution (Set 5)', async () => {
    const bridge = makeMockCheckBridge({ aicheck: AICHECK_FIXTURE });
    renderScreen(<AiCheckPage bridge={bridge} />);
    await runFile();
    // the un-strippable caution, verbatim from the core
    expect(screen.getByTestId('ai-disclaimer').textContent).toMatch(/NOT proof of AI authorship/);
    // the honest % with two-tier highlights
    expect(screen.getByTestId('ai-proportion').textContent).toBe('42.0%');
    expect(screen.getByTestId('aicheck-highlight-0').getAttribute('data-tier')).toBe('flagged');
    expect(screen.getByTestId('aicheck-highlight-1').getAttribute('data-tier')).toBe('assessed');
    // the paraphrase lane is honestly unavailable — never a fake category
    expect(screen.getByTestId('paraphrase-unavailable').textContent).toContain('UNAVAILABLE');
  });

  it('renders the Evidence Summary grouped by bias tier, every row with its detail, no score (honesty gate)', async () => {
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE })} />);
    await runFile();
    const summary = screen.getByTestId('evidence-summary');
    // three bias-tier groups
    expect(screen.getByTestId('evidence-group-factual')).toBeTruthy();
    expect(screen.getByTestId('evidence-group-structural')).toBeTruthy();
    expect(screen.getByTestId('evidence-group-stylometric')).toBeTruthy();
    // the style group carries the non-native caveat (verbal down-weighting)
    expect(screen.getByTestId('evidence-group-stylometric').textContent).toMatch(/over-flag non-native/i);
    // EVERY row shows its detail string (no bare levels)
    expect(summary.textContent).toMatch(/2 of 14 references could not be verified/); // citation row detail
    expect(summary.textContent).toMatch(/CV 0.26/); // stylometry row detail
    // Set E: the deep-verifier perplexity row groups under STYLE-BASED (a
    // bigger-model perplexity signal is style-based / down-weighted, NOT factual)
    // and carries the softness caveat in its detail.
    const styleGroup = screen.getByTestId('evidence-group-stylometric');
    expect(styleGroup.textContent).toMatch(/Deep-verifier perplexity/);
    expect(styleGroup.textContent).toMatch(/human range overlaps AI/);
    expect(screen.getByTestId('evidence-group-factual').textContent).not.toMatch(/Deep-verifier/);
    // Set 2: the citation-density PARSE-FAILURE row renders honestly as
    // "Unavailable" with its reason — NOT a fabricated level.
    expect(screen.getByTestId('evidence-group-structural').textContent).toMatch(/Unavailable/);
    expect(summary.textContent).toMatch(/could not be attributed — density unreliable/);
    screen.getAllByTestId('evidence-row').forEach((row) => {
      expect(row.textContent).toMatch(/: /); // "Level  Signal: detail"
    });
    // the perplexity row's in-row provisional footnote
    expect(screen.getByTestId('provisional-footnote').textContent).toMatch(/preliminary/);
    // HONESTY GATE: document_score.value is null → no score is presented — the
    // gate line says so, and no "AI Signal Score" label renders.
    expect(summary.textContent).toMatch(/isn't shown/); // the gate line
    expect(summary.textContent).not.toMatch(/AI Signal Score/);
    // the C2b interim ack line is GONE (replaced by the real citation rows)
    expect(screen.queryByTestId('citation-verification-note')).toBeNull();
  });

  it('renders a NotApplicable signal honestly (render path exists; no Phase-1 producer)', async () => {
    // NotApplicable has no backend producer yet (needs document classification);
    // the render must still handle it honestly. Hand-authored row proves that.
    const withNA: AiCheckResult = {
      ...AICHECK_FIXTURE,
      analysis: {
        ...AICHECK_FIXTURE.analysis,
        document_score: {
          value: null,
          band: null,
          evidence: [
            { signal: 'citation density', status: 'not_applicable', level: null, bias_tier: 'structural', detail: 'not a scholarly document' },
          ],
        },
      },
    };
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: withNA })} />);
    await runFile();
    const structural = screen.getByTestId('evidence-group-structural');
    expect(structural.textContent).toMatch(/Not applicable/);
    expect(structural.textContent).toMatch(/not a scholarly document/);
  });
});

describe('AI Check — progress, cancel, pre-flight (Set 3)', () => {
  const pickAndRun = async (name = 'paper.pdf') => {
    fireEvent.change(await screen.findByTestId('file-input'), {
      target: { files: [new File(['x'], name, { type: 'application/pdf' })] },
    });
    await screen.findByTestId('selected-name');
    fireEvent.click(screen.getByTestId('run-check'));
  };

  it('pre-flight names the attainable tier + the gap, and guards the hint so Ready shows no nudge', async () => {
    const needsMem: AiCheckMemoryStatus = {
      free_mb: 1024, total_gb: 8, stage1_fits: true, stage1_available: true, deep_fits: false, deep_need_mb: 2400,
      tier_attainable: 'compact_1_5b', tier_label: 'the compact 1.5B deep verifier',
      hint: 'Needs about 2.3 GB free to run; closing browsers usually frees the most.',
    };
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, memoryStatus: needsMem })} />);
    const panel = await screen.findByTestId('memory-status');
    expect(panel.textContent).toMatch(/Needs memory/);
    expect(screen.getByTestId('memory-tier').textContent).toMatch(/compact 1\.5B deep verifier/);
    expect(screen.getByTestId('memory-hint').textContent).toMatch(/2\.3 GB free to run/); // NAMES THE GAP
    expect(panel.textContent).toMatch(/1\.0 GB free of 8\.0 GB/);

    // Ready → NO hint line (no contradiction with the Ready chip).
    cleanup();
    const ready: AiCheckMemoryStatus = { ...needsMem, deep_fits: true, hint: 'Ready: the compact 1.5B deep verifier will run.' };
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, memoryStatus: ready })} />);
    expect((await screen.findByTestId('memory-status')).textContent).toMatch(/Ready/);
    expect(screen.queryByTestId('memory-hint')).toBeNull();
  });

  it('no deep model but the bundled Stage-1 LM present → NOT mislabeled "Pre-pass only"', async () => {
    // The post-Set-1 stranger state: no deep model, but the 0.5B runs.
    const stage1: AiCheckMemoryStatus = {
      free_mb: 1024, total_gb: 8, stage1_fits: true, stage1_available: true, deep_fits: false, deep_need_mb: null,
      tier_attainable: 'heuristic_only', tier_label: 'the fast pre-pass only',
      hint: 'This device runs the fast pre-pass and the on-device Stage-1 language model. Deep verification by a larger model isn’t available on this machine.',
    };
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, memoryStatus: stage1 })} />);
    const panel = await screen.findByTestId('memory-status');
    expect(panel.textContent).toMatch(/Stage-1 · no deep model/); // honest chip
    expect(panel.textContent).not.toMatch(/Pre-pass only/); // NOT the weak-detector label
    expect(screen.getByTestId('memory-hint').textContent).toMatch(/on-device Stage-1 language model/);

    // Truly model-less (not even Stage-1) → the honest "Pre-pass only".
    cleanup();
    const none: AiCheckMemoryStatus = { ...stage1, stage1_available: false, hint: 'This device runs the fast pre-pass only; no on-device model is available.' };
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, memoryStatus: none })} />);
    expect((await screen.findByTestId('memory-status')).textContent).toMatch(/Pre-pass only/);
  });

  it('Re-check re-queries the memory status (the empowerment loop)', async () => {
    const calls: string[] = [];
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, onCall: (c) => calls.push(c) })} />);
    await screen.findByTestId('memory-status');
    expect(calls.filter((c) => c === 'aicheck_memory_status')).toHaveLength(1); // mount
    fireEvent.click(screen.getByTestId('memory-recheck'));
    await waitFor(() => expect(calls.filter((c) => c === 'aicheck_memory_status')).toHaveLength(2));
  });

  it('live events drive the stage UI (deep 7 of 16 + bar) and surface memory-skip notes', async () => {
    const bridge = makeMockCheckBridge({
      aicheck: AICHECK_FIXTURE,
      aicheckPending: true, // stay at the last event so the mid-run UI is observable
      aicheckEvents: [
        { type: 'pre_pass' },
        { type: 'stage1_lm' },
        { type: 'memory_skip', model: '7B deep verifier', reason: 'this machine has under ~16 GB of RAM' },
        { type: 'deep_verify', done: 7, total: 16 },
      ],
    });
    renderScreen(<AiCheckPage bridge={bridge} />);
    await pickAndRun();
    // The running stage is "Deep verification"; the counter lives in its bar.
    expect((await screen.findByTestId('progress-stage')).textContent).toMatch(/Deep verification/);
    expect(screen.getByTestId('progress-bar').textContent).toMatch(/passage 7 of 16/);
    // The memory-skip note is rendered inline under the deep stage it's about.
    expect(screen.getByTestId('memory-skip-note').textContent).toMatch(/7B deep verifier skipped/);
    expect(screen.getByTestId('cancel-run')).toBeTruthy();
    expect(screen.queryByTestId('check-report')).toBeNull();
  });

  it('Cancel calls the command and renders the honest Cancelled state — no toast, no numbers', async () => {
    const calls: string[] = [];
    const bridge = makeMockCheckBridge({
      aicheck: AICHECK_FIXTURE,
      aicheckEvents: [
        { type: 'pre_pass' },
        { type: 'stage1_lm' },
        { type: 'deep_verify', done: 4, total: 16 },
        { type: 'cancelled', stage: 'deep verification', done: 4, total: 16 },
      ],
      onCall: (c) => calls.push(c),
    });
    renderScreen(<AiCheckPage bridge={bridge} />);
    await pickAndRun();
    // The run is in-flight at deep_verify{4,16}; clicking Cancel releases the
    // mock's gate → the cancelled event + reject with code 'cancelled'.
    fireEvent.click(await screen.findByTestId('cancel-run'));
    expect(calls).toContain('cancel_aicheck');
    const cancelled = await screen.findByTestId('cancelled-state');
    expect(cancelled.textContent).toMatch(/Cancelled during deep verification; 4 of 16 passages scored/);
    expect(cancelled.textContent).toMatch(/partial analysis isn't a valid signal/);
    // PIN #5 — a cancelled run renders NO report, NO evidence, NO numbers, and
    // NO error toast (the "cancelled" code is a benign stop).
    expect(screen.queryByTestId('check-report')).toBeNull();
    expect(screen.queryByTestId('evidence-summary')).toBeNull();
    expect(screen.queryByTestId('check-error')).toBeNull();
  });

  it('STAGES + EVENT_STAGE map the event channel to the checklist (pure)', () => {
    expect(STAGES[EVENT_STAGE.extract!]).toBe('Reading the document');
    expect(STAGES[EVENT_STAGE.pre_pass!]).toBe('Fast pre-pass');
    expect(STAGES[EVENT_STAGE.stage1_lm!]).toBe('Stage-1 language model');
    expect(STAGES[EVENT_STAGE.deep_verify!]).toBe('Deep verification');
    expect(STAGES[EVENT_STAGE.report!]).toBe('Finishing');
    // memory_skip / cancelled don't advance the stage.
    expect(EVENT_STAGE.memory_skip).toBeUndefined();
    expect(EVENT_STAGE.cancelled).toBeUndefined();
  });

});

describe('AI Check — citation verification opt-in (C2b)', () => {
  const box = () => screen.getByTestId('verify-citations') as HTMLInputElement;
  const lastVerify = (calls: Array<[string, string, boolean | undefined]>) =>
    calls.filter(([c]) => c === 'run_aicheck').at(-1)?.[2];

  it('defaults OFF on fresh state — the run passes verifyCitations=false', async () => {
    localStorage.clear();
    const calls: Array<[string, string, boolean | undefined]> = [];
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, onCall: (c, p, v) => calls.push([c, p, v]) })} />);
    expect(box().checked).toBe(false);
    await runFile();
    expect(lastVerify(calls)).toBe(false);
  });

  it('toggle ON + suite allowed → verifyCitations=true and persists', async () => {
    localStorage.clear();
    const calls: Array<[string, string, boolean | undefined]> = [];
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, onCall: (c, p, v) => calls.push([c, p, v]) })} />);
    fireEvent.click(box());
    expect(readVerifyCitations()).toBe(true); // persisted to gaply.settings.aicheck
    await runFile();
    expect(lastVerify(calls)).toBe(true);
  });

  it('AND-gate: opt-in ON but the global suite cloud is OFF → verifyCitations=false', async () => {
    localStorage.clear();
    setVerifyCitations(true); // AI-Check opt-in ON (persisted before mount)
    setCloudConsent('citation_verification', false); // global kill-switch OFF
    const calls: Array<[string, string, boolean | undefined]> = [];
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, onCall: (c, p, v) => calls.push([c, p, v]) })} />);
    expect(box().checked).toBe(true); // control reflects the opt-in
    await runFile();
    expect(lastVerify(calls)).toBe(false); // AND-gate collapses to false
  });
});

describe('Statistical Analysis Check', () => {
  it('labels findings as mathematically certain (green)', async () => {
    const bridge = makeMockCheckBridge({ validation: STATS_FAIL });
    renderScreen(<StatsCheckPage bridge={bridge} />);
    await runFile();
    // certainty label present on the findings
    expect(screen.getAllByText('mathematically certain').length).toBeGreaterThanOrEqual(1);
    // critical rule sorts first
    expect(screen.getByTestId('finding-0').getAttribute('data-tier')).toBe('mathematically_certain');
  });
});

/* --------------------------- adapters (pure) ----------------------------- */

describe('adapters keep certainty tiers honest', () => {
  it('validation → mathematically_certain, ai/plagiarism → ai_assessed_moderate', () => {
    expect(validationToReport(STATS_FAIL).findings.every((f) => f.tier === 'mathematically_certain')).toBe(true);
    expect(aiToReport(AI).findings.every((f) => f.tier === 'ai_assessed_moderate')).toBe(true);
    expect(plagiarismToReport(PLAG).findings.every((f) => f.tier === 'ai_assessed_moderate')).toBe(true);
    // AI disclaimer is carried verbatim
    expect(aiToReport(AI).disclaimer).toBe(AI.disclaimer);
    // every finding has provenance
    for (const rep of [validationToReport(STATS_FAIL), aiToReport(AI), plagiarismToReport(PLAG)]) {
      expect(rep.findings.every((f) => f.provenance.length > 0)).toBe(true);
    }
  });
});

/* ------------------- deep plagiarism: honest coming-soon ------------------ */

describe('deep plagiarism analysis is an HONEST coming-soon (no fabrication)', () => {
  it('shows a coming-soon card with NO fabricated score, source, or vendor name', async () => {
    renderScreen(<PlagiarismCheckPage bridge={makeMockCheckBridge({ plagiarism: PLAG })} />, {
      user: { id: 'u1', email: 'a@b.c' },
    });
    const panel = await screen.findByTestId('deep-check');
    expect(within(panel).getByTestId('deep-note').textContent).toMatch(/coming soon/i);
    // no fabricated percentage, no placeholder source, no third-party vendor
    expect(panel.textContent).not.toMatch(/%/);
    expect(panel.textContent).not.toMatch(/copyleaks|turnitin|researchgate|example\.org/i);
    // with the flag off (default), there is no run button and no result
    expect(screen.queryByTestId('run-deep')).toBeNull();
    expect(screen.queryByTestId('deep-result')).toBeNull();
  });
});

/* --------------------- local checks make zero network -------------------- */

describe('locality: local checks carry no manuscript over the network', () => {
  it('a local run makes no fetch/XHR and hands the bridge only a path', async () => {
    const fetchSpy = vi.fn();
    const origFetch = global.fetch;
    (global as any).fetch = fetchSpy;
    const xhrOpen = vi.spyOn(XMLHttpRequest.prototype, 'open');
    const handed: string[] = [];
    const bridge = makeMockCheckBridge({
      aicheck: AICHECK_FIXTURE,
      onCall: (_cmd, path) => handed.push(path),
    });

    renderScreen(<AiCheckPage bridge={bridge} />);
    fireEvent.change(await screen.findByTestId('file-input'), {
      target: { files: [new File(['x'], `${RAW_MANUSCRIPT_SENTINEL}.pdf`, { type: 'application/pdf' })] },
    });
    await screen.findByTestId('selected-name');
    fireEvent.click(screen.getByTestId('run-check'));
    await screen.findByTestId('check-report');

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(xhrOpen).not.toHaveBeenCalled();
    // Only a PATH crossed (the pre-flight aicheck_memory_status hands '', no
    // path and no bytes) — filter it out and the manuscript path is the only one.
    expect(handed.filter((p) => p.length > 0)).toHaveLength(1);
    global.fetch = origFetch;
    xhrOpen.mockRestore();
  });
});
