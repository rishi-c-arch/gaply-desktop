// The descriptive summary's route and the hub tile that reaches it (Research
// OS, Phase 2B): both appear with REACT_APP_FEATURE_STATS_DESCRIBE on and are
// hidden with it off. Since Phase 4B-2 the tile is the method selector's
// (decision 77), which opens the summary. The flag hides unfinished UI; it is
// not an authorization boundary.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { DESCRIBE_PATH, describeRoute } from './describeRoute';
import { STATISTICS_HOME_PATH } from './statisticsRoute';
import AppHomeHero from '../home/AppHomeHero';
import { ThemeProvider } from '../../contexts/ThemeContext';

const flags = vi.hoisted(() => ({ describe: false }));
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'statsDescribe' && flags.describe,
}));
vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));
vi.mock('../home/HeroAnimation', () => ({ default: () => <div /> }));

afterEach(cleanup);

function routeTo(path: string) {
  render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        {describeRoute(<div data-testid="describe-here" />)}
        <Route path="*" element={<div data-testid="no-route" />} />
      </Routes>
    </MemoryRouter>
  );
}

function hub() {
  render(
    <MemoryRouter>
      <ThemeProvider>
        <AppHomeHero />
      </ThemeProvider>
    </MemoryRouter>
  );
}

describe('REACT_APP_FEATURE_STATS_DESCRIBE', () => {
  it('on: the route and the hub tile appear', () => {
    flags.describe = true;
    expect(DESCRIBE_PATH).toBe('/app/statistics/describe');
    routeTo(DESCRIBE_PATH);
    expect(screen.getByTestId('describe-here')).toBeTruthy();
    cleanup();
    hub();
    // The selector replaced the summary's own tile (Phase 4B-2).
    expect(screen.queryByText('Descriptive Summary')).toBeNull();
    const tiles = screen.getAllByText('Statistical Methods');
    expect(tiles.length).toBeGreaterThan(0);
    expect(tiles[0].closest('a')?.getAttribute('href')).toBe(STATISTICS_HOME_PATH);
  });

  it('off: both are hidden', () => {
    flags.describe = false;
    routeTo(DESCRIBE_PATH);
    expect(screen.queryByTestId('describe-here')).toBeNull();
    expect(screen.getByTestId('no-route')).toBeTruthy();
    cleanup();
    hub();
    expect(screen.queryByText('Descriptive Summary')).toBeNull();
    expect(screen.queryByText('Statistical Methods')).toBeNull();
    // The rest of the hub is unchanged.
    expect(screen.getAllByText(/Statistical Analysis Verifier/).length).toBeGreaterThan(0);
  });
});
