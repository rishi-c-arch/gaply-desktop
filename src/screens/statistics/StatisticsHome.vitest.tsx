// The Statistics method selector (Research OS, Phase 4B-2, decision 77): a
// card if and only if Rust says the capability is runnable; Planned ones on
// the roadmap panel only, Deprecated ones in the retired list only; one Stats
// Verifier card naming its runnable methods, with its entitlement; the Stats
// Check linked only while its flag is on. The list is Rust's: a test table
// with every lifecycle, runnable and not.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { place, StatisticsHome } from './StatisticsHome';
import { makeMockCapabilityBridge } from './ttestBridge';
import type { CapabilityView, Lifecycle } from './ttestTypes';
import { capabilityList } from './fixtures/ttestFixtures';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import type { EntitlementStatus } from '../subscription/entitlement';

const flags = vi.hoisted(() => ({ statsCheck: false }));
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'statsCheck' && flags.statsCheck,
}));

afterEach(cleanup);

const cap = (id: string, lifecycle: Lifecycle, runnable: boolean, title = `${id}.title`): CapabilityView => ({
  id,
  version: 1,
  lifecycle,
  titleCode: title,
  descriptionCode: 'stats.capability.verify.description',
  licenseNotes: [],
  knownLimitations: ['stats.capability.limit.table.cell_text_cap'],
  runnable,
});

/** Every lifecycle, runnable and not, as Rust might list them. */
const TABLE: CapabilityView[] = [
  cap('stats.check', 'stable', true, 'stats.capability.check.title'),
  cap('stats.verify.independent_t', 'beta', true, 'stats.capability.verify.independent_t.title'),
  cap('stats.verify.pearson', 'beta', true, 'stats.capability.verify.pearson.title'),
  cap('stats.verify.ols', 'planned', false, 'stats.capability.verify.ols.title'),
  cap('stats.describe', 'beta', true, 'stats.capability.describe.title'),
  capabilityList[0],
  cap('stats.analyze.experimental_off', 'experimental', false),
  cap('stats.analyze.experimental_on', 'experimental', true),
  cap('stats.analyze.stable_elsewhere', 'stable', true),
  cap('stats.analyze.retired', 'deprecated', false),
  // Beta, but Rust says this build may not run it: no card anywhere.
  cap('stats.analyze.beta_not_runnable', 'beta', false),
];

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  } as any;
}

function renderHome(list: CapabilityView[] = TABLE, forceEntitlement: EntitlementStatus = 'entitled') {
  const bridge = makeMockCapabilityBridge({ capability_list: list });
  render(
    <MemoryRouter initialEntries={['/app/statistics']}>
      <GaplySessionProvider authService={auth()}>
        <Routes>
          <Route path="/app/statistics" element={<StatisticsHome bridge={bridge} forceEntitlement={forceEntitlement} />} />
          <Route path="/app/statistics/ttest" element={<div data-testid="ttest-here" />} />
          <Route path="/app/statistics/describe" element={<div data-testid="describe-here" />} />
          <Route path="/app/statsverifier" element={<div data-testid="verifier-here" />} />
          <Route path="/app/check/stats" element={<div data-testid="check-here" />} />
        </Routes>
      </GaplySessionProvider>
    </MemoryRouter>
  );
  return bridge;
}

describe('place', () => {
  it('gives a card if and only if Rust says runnable, whatever the lifecycle', () => {
    for (const on of [false, true]) {
      const p = place(TABLE, on);
      const carded = [...p.cards, ...p.verifier, ...(p.check ? [p.check] : [])].map((c) => c.id).sort();
      const runnable = TABLE.filter((c) => c.runnable && (on || c.id !== 'stats.check')).map((c) => c.id).sort();
      expect(carded).toEqual(runnable);
      expect(p.roadmap.map((c) => c.id)).toEqual(['stats.verify.ols']);
      expect(p.retired.map((c) => c.id)).toEqual(['stats.analyze.retired']);
      // Neither the roadmap nor the retired list has anything runnable.
      expect([...p.roadmap, ...p.retired].some((c) => c.runnable)).toBe(false);
    }
  });
});

describe('StatisticsHome', () => {
  it('reads capability_list once, and draws a card only for what is runnable', async () => {
    flags.statsCheck = false;
    const bridge = renderHome();
    await screen.findByTestId('stats-cards');
    expect(bridge.calls).toEqual([['capability_list', {}]]);
    for (const id of ['stats.describe', 'stats.analyze.independent_t', 'stats.analyze.experimental_on', 'stats.analyze.stable_elsewhere']) {
      expect(screen.getByTestId(`stats-card-${id}`), id).toBeTruthy();
    }
    for (const id of [
      'stats.verify.ols',
      'stats.analyze.experimental_off',
      'stats.analyze.retired',
      'stats.analyze.beta_not_runnable',
      'stats.verify.independent_t',
      'stats.verify.pearson',
    ]) {
      expect(screen.queryByTestId(`stats-card-${id}`), id).toBeNull();
    }
    // Neither the not-runnable Beta nor the off Experimental appears at all.
    expect(screen.queryByText('stats.analyze.beta_not_runnable.title')).toBeNull();
    expect(screen.queryByText('stats.analyze.experimental_off.title')).toBeNull();
  });

  it('shows each Beta card with its label and the sentences of its known limitations', async () => {
    renderHome();
    const card = await screen.findByTestId('stats-card-stats.analyze.independent_t');
    expect(within(card).getByText('Independent-samples t-test on your data')).toBeTruthy();
    expect(screen.getByTestId('stats-lifecycle-stats.analyze.independent_t').textContent).toBe('Beta');
    const limits = screen.getByTestId('stats-limits-stats.analyze.independent_t');
    expect(within(limits).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      "The group's levels follow Level 1's reading of cells and are listed in the order Gaply reads them; you choose the two to compare.",
      'Normality is not assessed: Gaply does not check whether each group is roughly normal.',
      "Cohen's d is shown without a confidence interval.",
    ]);
    // A card with no screen of its own says so and offers no way to run.
    expect(screen.getByTestId('stats-no-route-stats.analyze.stable_elsewhere')).toBeTruthy();
    // The t-test and the summary open their own routes.
    fireEvent.click(screen.getByTestId('stats-open-stats.analyze.independent_t'));
    expect(await screen.findByTestId('ttest-here')).toBeTruthy();
  });

  it('lists Planned methods only on the roadmap and Deprecated ones only as retired', async () => {
    renderHome();
    const roadmap = await screen.findByTestId('stats-roadmap');
    expect(within(roadmap).getByTestId('stats-roadmap-stats.verify.ols').textContent).toBe('Linear regression (ordinary least squares)');
    expect(within(roadmap).queryAllByRole('link')).toHaveLength(0);
    const retired = screen.getByTestId('stats-retired');
    expect(within(retired).getByTestId('stats-retired-stats.analyze.retired')).toBeTruthy();
    expect(within(retired).queryAllByRole('link')).toHaveLength(0);
  });

  it('shows the runnable verifier methods as one Stats Verifier card with its entitlement', async () => {
    renderHome(TABLE, 'not_entitled');
    const card = await screen.findByTestId('stats-card-verifier');
    expect(within(screen.getByTestId('stats-verifier-methods')).getAllByRole('listitem').map((l) => l.textContent)).toEqual([
      'Independent-samples t-test',
      'Pearson correlation',
    ]);
    // The Planned OLS is not one of its methods.
    expect(card.textContent).not.toContain('ordinary least squares');
    expect(screen.getByTestId('stats-verifier-access').textContent).toBe('Not included in your plan');
    fireEvent.click(screen.getByTestId('stats-open-verifier'));
    expect(await screen.findByTestId('verifier-here')).toBeTruthy();
  });

  it('shows no entitlement on the describe and t-test cards', async () => {
    renderHome(TABLE, 'not_entitled');
    for (const id of ['stats.describe', 'stats.analyze.independent_t']) {
      const card = await screen.findByTestId(`stats-card-${id}`);
      expect(card.textContent, id).not.toMatch(/plan|access|account/i);
    }
  });

  it('links the Stats Check only while its flag is on, with no entitlement', async () => {
    flags.statsCheck = true;
    renderHome();
    const card = await screen.findByTestId('stats-card-check');
    expect(card.textContent).not.toMatch(/plan|access|account/i);
    fireEvent.click(screen.getByTestId('stats-open-check'));
    expect(await screen.findByTestId('check-here')).toBeTruthy();
    cleanup();
    flags.statsCheck = false;
    renderHome();
    await screen.findByTestId('stats-cards');
    expect(screen.queryByTestId('stats-card-check')).toBeNull();
  });

  it('says so when the list cannot be read, and draws no card', async () => {
    const bridge = makeMockCapabilityBridge({});
    render(
      <MemoryRouter>
        <GaplySessionProvider authService={auth()}>
          <StatisticsHome bridge={bridge} />
        </GaplySessionProvider>
      </MemoryRouter>
    );
    expect((await screen.findByTestId('stats-home-error')).textContent).toBe('The list of methods could not be read.');
    expect(screen.queryByTestId('stats-cards')).toBeNull();
  });
});
