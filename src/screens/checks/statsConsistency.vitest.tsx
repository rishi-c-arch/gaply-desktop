// Level 0 on the Stats Check screen: the header counts, one card per verdict,
// the props-type rule, the not-read list, and a floor note shown on its card.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import StatsConsistencySection, { INCONSISTENT_TEXT, L0ResultCard, VERDICT_ORDER, formatP } from './StatsConsistencySection';
import StatsCheckPage from './StatsCheckPage';
import { makeMockCheckBridge } from './checkBridge';
import { L0ResultView, StatsConsistencyView } from './statsConsistencyTypes';
import { StatsValidityReport } from './agentTypes';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';

// The globe needs ResizeObserver, which jsdom lacks; checks.vitest.tsx stubs it the same way.
vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const CHECK = {
  methodId: 'p_consistency',
  methodName: 'p-value recomputed from the test statistic and its degrees of freedom',
  engineVersion: 'test',
  provenance: 'Nuijten et al. 2016',
};
const FLOOR_TEXT = 'Printed as p = .0001; the exact p is smaller still. APA style is p < .001.';

function result(id: number, raw: string, verdict: L0ResultView['verdict'], p: string | null = '.045'): L0ResultView {
  return {
    id,
    location: { section: 'results', paragraph: id, section_index: 2 },
    raw,
    reportedP: p,
    reportedPRelation: p === null ? null : '=',
    recomputedP: { lo: 0.0441, hi: 0.0452 },
    verdict,
    check: CHECK,
  };
}

// Document order deliberately NOT display order: the screen must reorder.
const VIEW: StatsConsistencyView = {
  claimsRead: 6,
  verdictCounts: { inconsistent: 1, concern: 1, couldNotCheck: 1, consistent: 3 },
  notReadCount: 3,
  notRead: [
    { code: 'missing_degrees_of_freedom', text: 'A t, F, r or chi-square with no degrees of freedom printed.', count: 2 },
    { code: 'p_without_statistic', text: 'A p-value with no test statistic it could be read with.', count: 1 },
  ],
  alpha: { values: ['.05'], assumed: true, note: 'The paper states no significance threshold, so .05 was assumed.' },
  results: [
    result(0, 't(28) = 2.10, p = .045', { kind: 'consistent', note: null }),
    result(1, 't(28) = 8.50, p = .0001', { kind: 'consistent', note: { code: 'floor_printed_as_equality', text: FLOOR_TEXT } }, '.0001'),
    result(2, 'r(28) = 1.50, p = .020', {
      kind: 'couldNotCheck',
      reasons: [{ methodName: CHECK.methodName, code: 'invalid_input', text: 'The printed numbers do not define this computation, so it was not made.' }],
    }),
    result(3, 't(28) = 2.10, p = .022', {
      kind: 'concern',
      reason: { code: 'one_tailed_unstated', text: 'These numbers agree only if the test was one-tailed, and the paper does not say it was.' },
    }),
    result(4, 't(28) = 2.10, p = .001', { kind: 'inconsistent', conclusionChanges: null }, '.001'),
    result(5, 't(28) = 2.20, p = .036', { kind: 'consistent', note: null }),
  ],
};

describe('Level 0 section — header', () => {
  it('shows the count per verdict, the not-read count and the alpha with its note', () => {
    render(<StatsConsistencySection view={VIEW} />);
    expect(screen.getByTestId('l0-count-inconsistent').textContent).toBe('1 Inconsistent');
    expect(screen.getByTestId('l0-count-concern').textContent).toBe('1 Concern');
    expect(screen.getByTestId('l0-count-could-not-check').textContent).toBe('1 Could not check');
    expect(screen.getByTestId('l0-count-consistent').textContent).toBe('3 Consistent');
    expect(screen.getByTestId('l0-count-not-read').textContent).toBe('3 not read');
    expect(screen.getByTestId('l0-alpha').textContent).toBe(
      'Significance threshold used: α = .05. The paper states no significance threshold, so .05 was assumed.'
    );
  });

  it('has no percentage, score or premium badge', () => {
    render(<StatsConsistencySection view={VIEW} />);
    const text = screen.getByTestId('l0-section').textContent ?? '';
    expect(text).not.toMatch(/%|score|grade|premium/i);
  });
});

describe('Level 0 section — cards', () => {
  it('renders one card per verdict, ordered Inconsistent, Concern, Could not check, then Consistent', () => {
    render(<StatsConsistencySection view={VIEW} />);
    const cards = Array.from(document.querySelectorAll('[data-testid^="l0-card-"]'));
    // The two plain Consistent results are folded; the noted one is shown.
    expect(cards.map((c) => c.getAttribute('data-testid'))).toEqual(
      VERDICT_ORDER.map((k) => `l0-card-${k}`)
    );
    for (const card of cards) {
      expect(within(card as HTMLElement).getByTestId('l0-method').textContent).toBe(CHECK.methodName);
      expect(card.textContent).toContain('Recomputed p: .0441 to .0452');
    }

    const bad = screen.getByTestId('l0-card-inconsistent');
    expect(within(bad).getByTestId('l0-verdict-label').textContent).toBe('Inconsistent');
    expect(bad.textContent).toContain(INCONSISTENT_TEXT);
    expect(bad.textContent).not.toMatch(/p-value is wrong|wrong p/i);
    expect(within(bad).getByTestId('l0-conclusion').textContent).toBe('Whether the conclusion changes was not assessed.');

    expect(within(screen.getByTestId('l0-card-concern')).getByTestId('l0-reason').textContent).toBe(
      VIEW.results[3].verdict.kind === 'concern' ? VIEW.results[3].verdict.reason.text : ''
    );
    const cnc = screen.getByTestId('l0-card-couldNotCheck');
    expect(within(cnc).getByTestId('l0-verdict-label').className).toContain('gds-badge--neutral');
    expect(within(cnc).getByTestId('l0-reasons').textContent).toContain('do not define this computation');
  });

  it('says when the conclusion changes, and when it does not', () => {
    const changes = result(9, 't(28) = 1.20, p = .001', { kind: 'inconsistent', conclusionChanges: true }, '.001');
    const { unmount } = render(<L0ResultCard result={changes} alpha=".05" />);
    expect(screen.getByTestId('l0-conclusion').textContent).toBe(
      'The conclusion at α = .05 changes: the printed p and the recomputed p fall on opposite sides of it.'
    );
    unmount();
    render(<L0ResultCard result={{ ...changes, verdict: { kind: 'inconsistent', conclusionChanges: false } }} alpha=".05" />);
    expect(screen.getByTestId('l0-conclusion').textContent).toBe('The conclusion at α = .05 does not change.');
  });

  it('shows a floor note on its card, as text, never only in a tooltip', () => {
    render(<StatsConsistencySection view={VIEW} />);
    const card = screen.getByTestId('l0-card-consistent');
    expect(within(card).getByTestId('l0-note').textContent).toBe(FLOOR_TEXT);
    expect(card.querySelector('[title]')).toBeNull();
  });

  it('folds plain Consistent results into a count the user can expand', () => {
    render(<StatsConsistencySection view={VIEW} />);
    expect(screen.getAllByTestId('l0-card-consistent')).toHaveLength(1);
    fireEvent.click(screen.getByTestId('l0-plain-toggle'));
    expect(screen.getAllByTestId('l0-card-consistent')).toHaveLength(3);
  });

  it('requires the method check in its props type', () => {
    const { check, ...withoutCheck } = VIEW.results[0];
    expect(check.methodId).toBe('p_consistency');
    // The type is the guard; `npm run build` type-checks this file, so an
    // unused directive here fails the build if `check` ever becomes optional.
    // @ts-expect-error a card cannot render without its MethodCheck
    const el = <L0ResultCard result={withoutCheck} alpha=".05" />;
    expect(el).toBeTruthy();
  });

  it('formats p-ranges without a leading zero and small ones in exponent form', () => {
    expect(formatP(0.04521)).toBe('.0452');
    expect(formatP(3.1e-9)).toBe('3.1e-9');
    expect(formatP(0)).toBe('0');
  });
});

describe('Level 0 section — not read', () => {
  it('shows the not-read count with each reason, never hidden', () => {
    render(<StatsConsistencySection view={VIEW} />);
    const nr = screen.getByTestId('l0-not-read');
    expect(nr.textContent).toContain('3 attempts were not read as a complete result:');
    expect(screen.getByTestId('l0-not-read-missing_degrees_of_freedom').textContent).toBe(
      '2 × A t, F, r or chi-square with no degrees of freedom printed.'
    );
    expect(screen.getByTestId('l0-not-read-p_without_statistic').textContent).toContain('1 ×');
  });

  it('still shows a zero count when everything was read', () => {
    render(<StatsConsistencySection view={{ ...VIEW, notReadCount: 0, notRead: [] }} />);
    expect(screen.getByTestId('l0-not-read').textContent).toBe('0 attempts were not read as a complete result.');
  });
});

/* ---------------------------- through the screen --------------------------- */

const STATS_OK: StatsValidityReport = { passed: true, checks: [], flags: [] };
// The same signed-out double checks.vitest.tsx uses.
const auth = (): AuthService =>
  ({
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  }) as any;

async function runThroughScreen(bridge: ReturnType<typeof makeMockCheckBridge>) {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>
        <StatsCheckPage bridge={bridge} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  fireEvent.change(await screen.findByTestId('file-input'), {
    target: { files: [new File(['x'], 'paper.pdf', { type: 'application/pdf' })] },
  });
  await screen.findByTestId('selected-name');
  fireEvent.click(screen.getByTestId('run-check'));
  await screen.findByTestId('check-report');
}

describe('Stats Check screen — Level 0 beside the validation findings', () => {
  it('runs both commands on the same path and renders Level 0 as its own section', async () => {
    const calls: [string, string][] = [];
    await runThroughScreen(
      makeMockCheckBridge({ validation: STATS_OK, consistency: VIEW, onCall: (c, p) => calls.push([c, p]) })
    );
    expect(await screen.findByTestId('l0-section')).toBeTruthy();
    expect(calls.map((c) => c[0]).sort()).toEqual(['run_stats_consistency', 'validate_manuscript']);
    expect(calls[0][1]).toBe(calls[1][1]);
  });

  it("shows an import refusal's own message in the section", async () => {
    const message = 'This file looks like a Gaply audit report, not a manuscript.';
    await runThroughScreen(makeMockCheckBridge({ validation: STATS_OK, consistencyError: message }));
    expect((await screen.findByTestId('l0-refused')).textContent).toBe(message);
  });
});
