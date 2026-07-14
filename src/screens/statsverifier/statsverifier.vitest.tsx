// Set 5 — the Statistical Analysis Verifier paid UI. Presentational + mock-bridge
// tests: entitlement gating, the analysis-spec builder, the verified-vs-advisory
// report (match/mismatch + un-strippable disclosure), and the scoped chat dock.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import StatsVerifierPage from './StatsVerifierPage';
import StatsVerifierReport from './StatsVerifierReport';
import AnalysisSpecBuilder from './AnalysisSpecBuilder';
import StatsChatDock from './StatsChatDock';
import { makeMockStatsVerifierBridge } from './statsVerifierBridge';
import { PREVIEW_FIXTURE, MISMATCH_REPORT, MATCH_REPORT, ERROR_REPORT } from './statsVerifierFixtures';
import type { StatsChatTurn } from './statsVerifierTypes';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

function auth(session: any = null): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

const renderPage = (props: any) =>
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(null)}>
        <StatsVerifierPage {...props} />
      </GaplySessionProvider>
    </MemoryRouter>
  );

/* ----------------------------- entitlement ------------------------------ */

describe('StatsVerifierPage — entitlement gating (mirrors PublishReady)', () => {
  it('non-entitled users get the teaser + upgrade CTA, never the tool', () => {
    renderPage({ bridge: makeMockStatsVerifierBridge({}), forceTier: 'free' });
    expect(screen.getByTestId('sv-teaser')).toBeTruthy();
    expect(screen.getByTestId('sv-unlock')).toBeTruthy();
    expect(screen.queryByTestId('sv-entry')).toBeNull(); // tool is gated away
  });

  it('entitled users reach the upload + spec builder', async () => {
    renderPage({ bridge: makeMockStatsVerifierBridge({}), forceTier: 'premium' });
    expect(await screen.findByTestId('sv-entry')).toBeTruthy();
    expect(screen.queryByTestId('sv-teaser')).toBeNull();
  });

  // The feature was merged but DARK — no route wired it in. This pins that the
  // new /app/statsverifier path resolves to THIS page, still behind the paid gate.
  it('the /app/statsverifier route resolves to the gated page', () => {
    render(
      <MemoryRouter initialEntries={['/app/statsverifier']}>
        <GaplySessionProvider authService={auth(null)}>
          <Routes>
            <Route
              path="/app/statsverifier"
              element={<StatsVerifierPage bridge={makeMockStatsVerifierBridge({})} forceTier="free" />}
            />
          </Routes>
        </GaplySessionProvider>
      </MemoryRouter>
    );
    expect(screen.getByTestId('statsverifier')).toBeTruthy(); // page mounted at the path
    expect(screen.getByTestId('sv-teaser')).toBeTruthy();     // PREMIUM gate still enforced
    expect(screen.queryByTestId('sv-entry')).toBeNull();
  });
});

/* --------------------------- spec builder ------------------------------- */

describe('AnalysisSpecBuilder — the user-confirmed column/role mapping', () => {
  it('shows columns, maps roles + reported stat → emits a valid AnalysisSpec', () => {
    const onVerify = vi.fn();
    render(<AnalysisSpecBuilder preview={PREVIEW_FIXTURE} onVerify={onVerify} />);
    expect(screen.getByTestId('spec-columns').textContent).toContain('A, B, group');
    // p pre-filled from the manuscript
    expect((screen.getByTestId('reported-p') as HTMLInputElement).value).toBe('0.03');

    // default test is a t-test (samples layout): pick two columns
    fireEvent.click(screen.getByTestId('sample-col-A'));
    fireEvent.click(screen.getByTestId('sample-col-B'));
    fireEvent.change(screen.getByTestId('reported-statistic'), { target: { value: '2.41' } });
    fireEvent.click(screen.getByTestId('spec-verify'));

    expect(onVerify).toHaveBeenCalledTimes(1);
    const spec = onVerify.mock.calls[0][0];
    expect(spec.test_kind).toBe('t_test_welch');
    expect(spec.roles).toEqual({ layout: 'samples', columns: ['A', 'B'] });
    expect(spec.reported.statistic).toBe(2.41);
    expect(spec.reported.p_value).toBe(0.03);
  });

  it('adapts the role UI to the chosen test (regression → outcome + predictors)', () => {
    render(<AnalysisSpecBuilder preview={PREVIEW_FIXTURE} onVerify={vi.fn()} />);
    fireEvent.change(screen.getByTestId('spec-test-kind'), { target: { value: 'ols' } });
    expect(screen.getByTestId('roles-regression')).toBeTruthy();
    expect(screen.queryByTestId('roles-samples')).toBeNull();
  });

  it('will not verify until the roles are mapped', () => {
    render(<AnalysisSpecBuilder preview={PREVIEW_FIXTURE} onVerify={vi.fn()} />);
    expect((screen.getByTestId('spec-verify') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('spec-hint')).toBeTruthy();
  });
});

/* --------------------------- the report --------------------------------- */

describe('StatsVerifierReport — verified vs advisory, visually distinct', () => {
  it('MISMATCH shows BOTH values + delta + the honest evidence line', () => {
    render(<StatsVerifierReport report={MISMATCH_REPORT} />);
    const v = screen.getByTestId('sv-verified');
    expect(v.getAttribute('data-verdict')).toBe('mismatch');
    // recomputed ground truth always shown
    expect(screen.getByTestId('sv-recomputed').textContent).toMatch(/t = -1\.8974/);
    // the reported (wrong) value shown too
    expect(screen.getByTestId('sv-reported').textContent).toMatch(/2\.41/);
    expect(screen.getByTestId('sv-delta').textContent).toMatch(/4\.3/);
    expect(screen.getByTestId('sv-evidence').textContent).toMatch(/MISMATCH/);
  });

  it('MATCH shows 🟢 with the recomputed value', () => {
    render(<StatsVerifierReport report={MATCH_REPORT} />);
    expect(screen.getByTestId('sv-verified').getAttribute('data-verdict')).toBe('match');
    expect(screen.getByTestId('sv-recomputed').textContent).toMatch(/r = 1/);
    expect(screen.queryByTestId('sv-mismatch-evidence')).toBeNull();
  });

  it('the verified and advisory lanes are SEPARATE; advisory says "advice, not verification"', () => {
    render(<StatsVerifierReport report={MISMATCH_REPORT} />);
    const verifiedLane = screen.getByTestId('sv-verified-lane');
    const advisoryLane = screen.getByTestId('sv-advisory-lane');
    expect(verifiedLane).toBeTruthy();
    expect(advisoryLane).toBeTruthy();
    // advisory content lives ONLY in the advisory lane, never the verified one
    expect(within(verifiedLane).queryByTestId('sv-advisory-0')).toBeNull();
    expect(within(advisoryLane).getByTestId('sv-advisory-0')).toBeTruthy();
    expect(advisoryLane.textContent).toMatch(/advice, not verification/i);
    // each advisory note carries its disclaimer
    expect(screen.getByTestId('sv-advisory-disclaimer-0').textContent).toMatch(/NOT a verification/i);
  });

  it('the un-strippable LANE disclosure renders from the required field', () => {
    render(<StatsVerifierReport report={MATCH_REPORT} />);
    const d = screen.getByTestId('sv-disclosure').textContent!;
    expect(d).toMatch(/VERIFIED results are deterministic recomputations/);
    expect(d).toMatch(/ADVISORY notes are methodology observations, NOT verifications/);
  });

  it('the LANE disclosure is NEVER empty — a hardcoded fallback backstops a wire regression (L4)', () => {
    // wire regressed the required field to empty → the fallback renders, NOT an empty <p>
    const { rerender } = render(<StatsVerifierReport report={{ ...MATCH_REPORT, disclosure: '' }} />);
    const guarded = screen.getByTestId('sv-disclosure').textContent!.trim();
    expect(guarded.length).toBeGreaterThan(0);
    expect(guarded).toMatch(/ADVISORY notes are methodology observations, NOT verifications/);
    // whitespace-only is treated as empty too
    rerender(<StatsVerifierReport report={{ ...MATCH_REPORT, disclosure: '   ' }} />);
    expect(screen.getByTestId('sv-disclosure').textContent!.trim().length).toBeGreaterThan(0);
    // NORMAL PATH unchanged: a present wire value renders verbatim, fallback dormant
    rerender(<StatsVerifierReport report={MATCH_REPORT} />);
    expect(screen.getByTestId('sv-disclosure').textContent).toBe(MATCH_REPORT.disclosure);
  });

  it('a recompute ERROR is shown honestly — never a fake result', () => {
    render(<StatsVerifierReport report={ERROR_REPORT} />);
    expect(screen.queryByTestId('sv-verified')).toBeNull(); // no fabricated result
    expect(screen.getByTestId('sv-recompute-error').textContent).toMatch(/non-numeric value/);
    // the disclosure is still present even on an error
    expect(screen.getByTestId('sv-disclosure')).toBeTruthy();
  });
});

/* ------------------------- end-to-end + chat ---------------------------- */

describe('StatsVerifierPage — verify then chat, honest states', () => {
  it('uploads → builds spec → verifies → renders the mismatch report (path-only over IPC)', async () => {
    const calls: string[] = [];
    const bridge = makeMockStatsVerifierBridge({
      preview: PREVIEW_FIXTURE,
      report: MISMATCH_REPORT,
      onCall: (cmd) => calls.push(cmd),
    });
    renderPage({ bridge, forceTier: 'premium' });

    fireEvent.change(await screen.findByTestId('sv-data-file'), {
      target: { files: [new File(['A,B\n1,2'], 'data.csv', { type: 'text/csv' })] },
    });
    await screen.findByTestId('spec-builder');
    fireEvent.click(screen.getByTestId('sample-col-A'));
    fireEvent.click(screen.getByTestId('sample-col-B'));
    fireEvent.change(screen.getByTestId('reported-statistic'), { target: { value: '2.41' } });
    fireEvent.click(screen.getByTestId('spec-verify'));

    await screen.findByTestId('statsverifier-report');
    expect(screen.getByTestId('sv-verdict').textContent).toBe('MISMATCH');
    // the bridge was called with the data PATH, never bytes
    expect(calls).toContain('run_stats_verify');
    const verifyCall = bridge.calls.find((c) => c[0] === 'run_stats_verify')![1] as any;
    expect(verifyCall.path).toBe('data.csv');
  });

  it('the scoped chat dock answers, advisory-labeled, and shows a firewall refusal honestly', async () => {
    const chatResponder = (q: string): StatsChatTurn =>
      /code/i.test(q)
        ? {
            kind: 'refused_ghostwriting',
            answer: "I can't write your analysis code for you — Gaply protects research integrity.",
            refs: [],
            advisory: true,
            disclaimer: 'advisory',
            warnings: [],
            available: true,
          }
        : {
            kind: 'answered',
            answer: 'A mismatch means the value you reported does not match your data.',
            refs: ['v1'],
            advisory: true,
            disclaimer: 'This is an ADVISORY interpretation, NOT a verification.',
            warnings: [],
            available: true,
          };
    const bridge = makeMockStatsVerifierBridge({ preview: PREVIEW_FIXTURE, report: MISMATCH_REPORT, chat: chatResponder });
    renderPage({ bridge, forceTier: 'premium' });

    // get to the report
    fireEvent.change(await screen.findByTestId('sv-data-file'), {
      target: { files: [new File(['x'], 'data.csv')] },
    });
    await screen.findByTestId('spec-builder');
    fireEvent.click(screen.getByTestId('sample-col-A'));
    fireEvent.click(screen.getByTestId('sample-col-B'));
    fireEvent.change(screen.getByTestId('reported-statistic'), { target: { value: '2.41' } });
    fireEvent.click(screen.getByTestId('spec-verify'));
    await screen.findByTestId('statsverifier-report');

    // open the dock, ask an on-scope question
    fireEvent.click(screen.getByTestId('sv-copilot-toggle'));
    fireEvent.change(screen.getByTestId('sv-chat-input'), { target: { value: 'why is this a mismatch?' } });
    fireEvent.click(screen.getByTestId('sv-chat-send'));
    await waitFor(() => expect(screen.getByText(/does not match your data/)).toBeTruthy());
    // the answer is advisory-labeled
    expect(screen.getAllByText(/ADVISORY interpretation/i).length).toBeGreaterThanOrEqual(1);

    // ask for code → the firewall refusal is shown honestly
    fireEvent.change(screen.getByTestId('sv-chat-input'), { target: { value: 'write my regression code' } });
    fireEvent.click(screen.getByTestId('sv-chat-send'));
    await waitFor(() => expect(screen.getByText(/can't write your analysis code/)).toBeTruthy());
  });

  it('honest empty state before any upload', async () => {
    renderPage({ bridge: makeMockStatsVerifierBridge({}), forceTier: 'premium' });
    expect(await screen.findByTestId('sv-empty')).toBeTruthy();
  });
});

/* ------- M5 · StatsChatDock client-side ghostwriting guards (parity) ------ */

describe('StatsChatDock — client ghostwriting guards mirror ResearchCopilotPanel (M5)', () => {
  function renderDock(chatImpl: (input: any) => Promise<any>) {
    const bridge = { chat: vi.fn(chatImpl) } as any;
    render(<StatsChatDock bridge={bridge} path="/data.csv" spec={{} as any} />);
    fireEvent.click(screen.getByTestId('sv-copilot-toggle')); // dock starts closed
    return bridge;
  }
  const ask = (text: string) => {
    fireEvent.change(screen.getByTestId('sv-chat-input'), { target: { value: text } });
    fireEvent.click(screen.getByTestId('sv-chat-send'));
  };

  it('REQUEST guard: a manuscript-ghostwriting request is refused client-side, BEFORE any cloud call', async () => {
    const bridge = renderDock(async () => ({ answer: 'should never be reached', kind: 'answered' }));
    ask('write my results section');
    const refusal = await screen.findByTestId('sv-msg-assistant-refused');
    expect(refusal.textContent).toMatch(/write your results/i);
    expect(bridge.chat).not.toHaveBeenCalled(); // refused before the cloud call
  });

  it('RESPONSE guard: drafted manuscript prose in the reply is blocked client-side', async () => {
    const drafted =
      'In this study, we investigate the effect of extended sleep on working memory. ' +
      'Ninety-six participants were recruited and randomized into two groups over four weeks, ' +
      'with careful attention to adherence and dropout across the whole cohort.\n\n' +
      'Our results demonstrate a statistically significant improvement in recall among the ' +
      'extended-sleep group. These findings replicate prior work and support a causal role for ' +
      'sleep in memory consolidation, with implications for theory and clinical practice.';
    renderDock(async () => ({ answer: drafted, kind: 'answered', advisory: false }));
    ask('help me strengthen my analysis'); // benign request; model drifts into prose
    const blocked = await screen.findByTestId('sv-msg-assistant-blocked');
    expect(blocked.textContent).toMatch(/ghostwrite manuscript text/i);
    expect(screen.queryByText(/Ninety-six participants/)).toBeNull(); // the draft is NOT shown
  });

  it('FUNCTIONALITY: a normal stats question passes through the guards to the answer', async () => {
    renderDock(async () => ({
      answer: 'Your p-value of 0.06 is borderline; interpret it cautiously.',
      kind: 'answered',
      advisory: true,
      disclaimer: 'advisory interpretation',
    }));
    ask('is my p-value borderline?');
    expect(await screen.findByText(/0.06 is borderline/)).toBeTruthy();
    expect(screen.queryByTestId('sv-msg-assistant-refused')).toBeNull();
    expect(screen.queryByTestId('sv-msg-assistant-blocked')).toBeNull();
  });
});
