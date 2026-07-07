// F10 — PublishReady tests. Mocked bridge/subscription, no network.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import PublishReadyPage from './PublishReadyPage';
import { makePublishReadyMock } from './publishReadyBridge';
import { buildProxyPayload } from './buildPayload';
import { synthesizeReviewerLetter, suggestAlternatives } from './synthesize';
import { PublishReadyReport, Finding } from '../report/reportTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const RAW_SENTINEL = 'RAW_MANUSCRIPT_EXCERPT_DO_NOT_LEAK';

function auth(session: any = { user: { id: 'u1', email: 'a@b.c' } }): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

// A report whose plagiarism finding DETAIL quotes the manuscript (the excerpt
// that must never reach the proxy), plus a refuted citation + a maths flag.
const REPORT: PublishReadyReport = {
  verdict: 'concern',
  combined_confidence: 1,
  findings: [
    { severity: 'critical', tier: 'mathematically_certain', certainty_label: 'mathematically certain', agent: 'validation_maths', title: 'rule failed: missing effect size', detail: 'p-value without effect size.', confidence: 1, provenance: ['rule:MissingEffectSize (MAJOR)', 'agent:validation_maths (deterministic)'], section: 'Results' },
    { severity: 'major', tier: 'reconsidered_after_peer_review', certainty_label: 'reconsidered after peer review', agent: 'verification', title: 'citation c1 REFUTED by evidence', detail: 'DOI resolves to a different work.', confidence: 0.9, provenance: ['evidence:ev-c1-0', 'agent:verification (harness-gated, via proxy)'] },
    { severity: 'minor', tier: 'ai_assessed_moderate', certainty_label: 'AI-assessed, moderate confidence', agent: 'plagiarism', title: 'near-verbatim — 88% similarity', detail: `"${RAW_SENTINEL}" matches a 2019 paper.`, confidence: 0.88, provenance: ['similarity:0.880', 'match_type:near-verbatim', 'source:corpus'] },
  ],
  checklist: [
    { requirement: 'word limit (3000 words)', passed: false, detail: '4100 words', guideline_source: 'https://x' },
    { requirement: 'required section: Methods', passed: true, detail: 'found', guideline_source: null },
  ],
  debate: { rounds_run: 2, converged: true, overridden_by_constraint: true, rejected_agents: [], revised_agents: ['verification'] },
  disclaimer: 'Certainty tiers … never definitive proof.',
};

function renderPR(props: any = {}, session: any = { user: { id: 'u1', email: 'a@b.c' } }) {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <PublishReadyPage {...props} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

const journalStep = () => {
  fireEvent.change(screen.getByTestId('pr-journal-input'), { target: { value: 'lancet' } });
};

/* ------------------------------ premium gate ---------------------------- */

describe('premium gate', () => {
  it('free user sees the blurred teaser + upgrade CTA', async () => {
    renderPR({ forceTier: 'free', bridge: makePublishReadyMock(REPORT) });
    expect(await screen.findByTestId('pr-teaser')).toBeTruthy();
    expect(screen.getByTestId('pr-unlock').textContent).toMatch(/Unlock the verdict/i);
    // no entry form for free users
    expect(screen.queryByTestId('pr-run')).toBeNull();
  });

  it('premium user reaches the full entry → report flow', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-file'), { target: { files: [new File(['x'], `${RAW_SENTINEL}.pdf`, { type: 'application/pdf' })] } });
    journalStep();
    fireEvent.click(await screen.findByTestId('pr-journal-The Lancet'));
    fireEvent.click(screen.getByTestId('pr-run'));

    // reviewer letter tab renders the flagship output
    await screen.findByTestId('publishready');
    await waitFor(() => expect(screen.getByTestId('tab-Reviewer Letter')).toBeTruthy());
    fireEvent.click(screen.getByTestId('tab-Reviewer Letter'));
    expect(await screen.findByTestId('reviewer-letter-panel')).toBeTruthy();
    // major revision (1 critical + 1 refuted)
    expect(screen.getByTestId('pr-verdict').textContent).toMatch(/MAJOR REVISION/);
    expect(screen.getByTestId('pr-probability').textContent).toMatch(/%/);
  });
});

/* --------------------- privacy: no manuscript text ---------------------- */

describe('privacy: only structured findings leave the device', () => {
  it('buildProxyPayload contains ZERO raw manuscript text', () => {
    const payload = buildProxyPayload(REPORT, { name: 'The Lancet', quartile: 'Q1' });
    const serialized = JSON.stringify(payload);
    // the plagiarism EXCERPT (in finding.detail) must NOT be present
    expect(serialized).not.toContain(RAW_SENTINEL);
    // detail free-text must not be included at all
    expect(serialized).not.toContain('p-value without effect size');
    // but structured findings ARE present
    expect(serialized).toContain('rule failed: missing effect size');
    expect(serialized).toContain('citation c1 REFUTED');
    expect(payload.findings[0].evidence).toContain('rule:MissingEffectSize (MAJOR)');
  });

  it('the running bridge sends only the structured payload (no excerpt)', async () => {
    const bridge = makePublishReadyMock(REPORT);
    const res = await bridge.run({ manuscriptPath: `/private/${RAW_SENTINEL}.pdf`, journal: { name: 'The Lancet', quartile: 'Q1' } });
    expect(JSON.stringify(res.proxyPayload)).not.toContain(RAW_SENTINEL);
    expect(bridge.lastPayload).toBeTruthy();
  });
});

/* --------------------- reviewer letter synthesis ------------------------ */

describe('reviewer letter synthesis (from a mocked debate result)', () => {
  it('renders a recommendation badge + probability from the report', () => {
    const letter = synthesizeReviewerLetter(REPORT, { name: 'The Lancet', quartile: 'Q1' });
    expect(letter.recommendation).toBe('major_revision'); // 1 critical + 1 refuted
    expect(letter.publicationProbability).toBeGreaterThanOrEqual(0);
    expect(letter.publicationProbability).toBeLessThanOrEqual(100);
    expect(letter.novelty.score).toBeGreaterThan(0);
  });

  it('a clean report yields ACCEPT with high probability', () => {
    const clean: PublishReadyReport = { ...REPORT, verdict: 'pass', findings: [{ severity: 'info', tier: 'ai_assessed_moderate', certainty_label: 'AI-assessed, moderate confidence', agent: 'extraction', title: 'Extraction: pass', detail: 'ok', confidence: 0.9, provenance: ['swarm:round-table'] } as Finding], checklist: [{ requirement: 'x', passed: true, detail: 'ok', guideline_source: null }] };
    const letter = synthesizeReviewerLetter(clean, { name: 'Some Q3 Journal', quartile: 'Q3' });
    expect(letter.recommendation).toBe('accept');
    expect(letter.publicationProbability).toBeGreaterThan(70);
  });
});

/* ------------------------- journal-fit alternatives --------------------- */

describe('journal fit', () => {
  it('suggests same-or-higher-quartile alternatives only', () => {
    const alts = suggestAlternatives({ name: 'Nonexistent Q2 Journal', quartile: 'Q2' });
    expect(alts.length).toBeGreaterThan(0);
    const rank: Record<string, number> = { Q1: 1, Q2: 2, Q3: 3, Q4: 4 };
    // every alternative must be Q1 or Q2 (rank <= 2) — never lower than target
    for (const a of alts) expect(rank[a.quartile]).toBeLessThanOrEqual(2);
  });

  it('excludes the target journal itself', () => {
    const alts = suggestAlternatives({ name: 'The Lancet', quartile: 'Q1' });
    expect(alts.some((a) => a.name === 'The Lancet')).toBe(false);
    for (const a of alts) expect(a.quartile).toBe('Q1'); // Q1 target → only Q1 alts
  });
});
