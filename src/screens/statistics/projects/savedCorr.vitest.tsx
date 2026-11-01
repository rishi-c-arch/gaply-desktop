// A saved correlations result (Research OS, Phase 8H-2, decisions 357 and
// 359), read from the two snapshots Rust wrote (src-tauri/tests/fixtures/
// projects/stored/corr_pairwise.json, pairwise with all three coefficients
// and a ticked code, and corr_listwise.json, listwise with Pearson only, by
// stored_corr_results_have_the_layout_of_their_fixtures): the shape check
// accepts each as its own case; the saved result renders through the live
// screen's CorrResultCard, its columns by position (C1, C2, …), with the same
// text as the card the live screen draws from the same result with the same
// names, and with no button but the coefficients' own choice. Here, under
// projects/, as savedAnova's.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { CorrResultCard } from '../CorrFlow';
import type { CorrResultView } from '../corrTypes';
import { makeMockProjectBridge } from './projectBridge';
import type { StoredRunView } from './projectTypes';
import { SavedResult } from './SavedResult';
import { checkStored } from './storedShape';
import { useGeneration } from './useGeneration';

vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || name === 'statsDescribe',
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));
vi.mock('../../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
type Stored = Record<string, unknown> & { bundleJson: string | null; failureJson: string | null };
const stored = (name: string): Stored => JSON.parse(fs.readFileSync(path.join(STORED, name), 'utf8')) as Stored;
const ID = '0123456789abcdef0123456789abcdef';

const RUN = (storedRunId: number): StoredRunView => ({
  storedRunId,
  capabilityId: 'stats.analyze.correlations',
  capabilityVersion: 1,
  state: 'succeeded',
  endCode: null,
  startedAt: 1793500000,
  endedAt: 1793500001,
  savedAt: 1793500100,
  agentVersion: '0.1.0',
  rerunOf: null,
  comparison: null,
  engineVersionDiffers: null,
});

function saved(result: unknown, run: StoredRunView) {
  const bridge = makeMockProjectBridge({ project_run_result: result });
  const Host: React.FC = () => {
    const gen = useGeneration();
    return <SavedResult projectId={ID} run={run} bridge={bridge} gen={gen} />;
  };
  render(<Host />);
}

describe('a saved correlations result', () => {
  it.each([
    ['corr_pairwise.json', ['pearson', 'spearman', 'kendall'], 'stats.corr.fewer_than_two_values'],
    ['corr_listwise.json', ['pearson'], 'stats.corr.missing_value'],
  ])('%s passes the shape check as its own case, its coefficients as saved', (name, coefficients, exclusion) => {
    const checked = checkStored(stored(name));
    expect(checked.kind).toBe('correlations');
    if (checked.kind !== 'correlations') return;
    expect(checked.result.result.coefficients.map((t) => t.coefficient)).toEqual(coefficients);
    expect(checked.result.result.descriptives.map((d) => d.variable)).toEqual(['C1', 'C2', 'C3']);
    expect(checked.result.result.exclusions.map((e) => e.code)).toContain(exclusion);
    expect(checked.dataFile.format).toBe('csv');
  });

  it('renders through the live card, its columns by position, the same text as the live card for the same result', async () => {
    const s = stored('corr_pairwise.json');
    saved(s, RUN(1));
    const card = await screen.findByTestId('cr-result-card');
    const checked = checkStored(s);
    if (checked.kind !== 'correlations') throw new Error('not correlations');
    const savedText = card.textContent;
    expect(within(card).getByTestId('cr-descriptives').textContent).toContain('C1');
    cleanup();
    const live = checked.result as CorrResultView;
    render(<CorrResultCard result={live} names={{ column: (a) => `C${a}` }} />);
    expect(screen.getByTestId('cr-result-card').textContent).toBe(savedText);
    expect(screen.getByTestId('cr-show-pearson').textContent).toBe("Pearson's r: 3 pairs");
    expect(screen.getByTestId('cr-matrix')).toBeTruthy();
    const buttons = within(screen.getByTestId('cr-result-card'))
      .getAllByRole('button')
      .map((b) => b.getAttribute('data-testid'));
    expect(buttons).toEqual(['cr-show-pearson', 'cr-show-spearman', 'cr-show-kendall']);
  });
});
