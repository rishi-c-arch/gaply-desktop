// A saved one-way ANOVA result (Research OS, Phase 8G-2, decision 330), read
// from the two snapshots Rust wrote (src-tauri/tests/fixtures/projects/
// stored/anova_post_hoc.json, a level left out and all three post hoc tests,
// and anova.json, every level and none, by
// stored_anova_results_have_the_layout_of_their_fixtures): the shape check
// accepts each as its own case; the saved result renders through the live
// screen's AnovaResultCard with the same text as the card the live screen
// draws from the same result, and with no button but the tests' own choice;
// and a stored result with a field out of shape, or a code outside the
// ANOVA's list, is refused whole. Here, under projects/, as savedPaired's.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AnovaResultCard } from '../AnovaFlow';
import type { AnovaResultView } from '../anovaTypes';
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
  capabilityId: 'stats.analyze.one_way_anova',
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

describe('a saved one-way ANOVA', () => {
  it.each([
    ['anova_post_hoc.json', 3, [0, 2, 3]],
    ['anova.json', 0, [0, 1, 2, 3]],
  ])('%s passes the shape check as its own case, its tests and levels as saved', (name, tests, levels) => {
    const checked = checkStored(stored(name));
    expect(checked.kind).toBe('anova');
    if (checked.kind !== 'anova') return;
    expect(checked.result.result.postHoc).toHaveLength(tests);
    expect(checked.result.result.groups.map((g) => g.level)).toEqual(levels);
    expect(checked.dataFile.format).toBe('csv');
  });

  it('renders through the live card, the same text as the live screen would show for the same result', async () => {
    const s = stored('anova_post_hoc.json');
    saved(s, RUN(1));
    const card = await screen.findByTestId('an-result-card');
    const checked = checkStored(s);
    if (checked.kind !== 'anova') throw new Error('not an ANOVA');
    const savedText = card.textContent;
    cleanup();
    const live = checked.result as AnovaResultView;
    render(<AnovaResultCard result={live} names={{ column: (a) => `C${a}`, level: (i) => `level ${i + 1}` }} />);
    expect(screen.getByTestId('an-result-card').textContent).toBe(savedText);
    // Its own tables: Tukey first, with its row count; a choice between the
    // three tests, and no other button.
    expect(screen.getByTestId('an-post-hoc-heading-tukey').textContent).toBe("Tukey's HSD: 3 pairs");
    const buttons = within(screen.getByTestId('an-result-card'))
      .getAllByRole('button')
      .map((b) => b.getAttribute('data-testid'));
    expect(buttons).toEqual(['an-show-tukey', 'an-show-games_howell', 'an-show-bonferroni']);
  });

  it('refuses a stored result with a field out of shape or a code from another list', () => {
    const s = stored('anova_post_hoc.json');
    const bundle = JSON.parse(s.bundleJson as string);
    const bad = (f: (b: typeof bundle) => void) => {
      const b = JSON.parse(JSON.stringify(bundle));
      f(b);
      return checkStored({ ...s, bundleJson: JSON.stringify(b) }).kind;
    };
    expect(bad(() => undefined)).toBe('anova');
    expect(bad((b) => (b.primaryOutputs.welch = { computed: { f: 'x', df1: 1, df2: 2, p: 0.5 } }))).toBe('badShape');
    expect(bad((b) => (b.primaryOutputs.etaSquared = { not_computable: 'stats.ttest.overflow' }))).toBe('badShape');
    expect(bad((b) => (b.primaryOutputs.postHoc[0].test = 'scheffe'))).toBe('badShape');
    expect(bad((b) => (b.primaryOutputs.postHoc[0].pairs.computed[0].outcome.computed.p = { below: null }))).toBe('badShape');
    expect(bad((b) => (b.primaryOutputs.groups[0].mean = null))).toBe('badShape');
  });
});
