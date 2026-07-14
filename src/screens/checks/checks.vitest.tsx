// F7 — single-agent check screens. Mocked bridges/clients, no real Tauri, no
// network. Each screen renders its agent's output through the F6 viewer.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import PlagiarismCheckPage from './PlagiarismCheckPage';
import AiCheckPage from './AiCheckPage';
import StatsCheckPage from './StatsCheckPage';
import { makeMockCheckBridge } from './checkBridge';
import {
  plagiarismToReport,
  aiToReport,
  validationToReport,
} from './adapters';
import {
  AiDetectionReport,
  PlagiarismReport,
  StatsValidityReport,
} from './agentTypes';
import { AICHECK_FIXTURE } from './aicheckFixture';

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
    expect(handed).toHaveLength(1); // only a path string was handed over
    global.fetch = origFetch;
    xhrOpen.mockRestore();
  });
});
