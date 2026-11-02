// A saved Cronbach's alpha result (Research OS, Phase 8I-2, decisions 382
// and 384), read from the two snapshots Rust wrote (src-tauri/tests/fixtures/
// projects/stored/alpha_reversed.json, two items reverse-scored with the
// scale's ends and a ticked code, and alpha.json, nothing reversed, by
// stored_alpha_results_have_the_layout_of_their_fixtures): the shape check
// accepts each as its own case; the saved result renders through the live
// screen's AlphaResultCard, its items by position (C1, C2, …), with the same
// text as the card the live screen draws from the same result with the same
// names, and with no button at all. Here, under projects/, as savedCorr's.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AlphaResultCard } from '../AlphaFlow';
import type { AlphaResultView } from '../alphaTypes';
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
  capabilityId: 'stats.analyze.cronbach_alpha',
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

describe("a saved Cronbach's alpha result", () => {
  it.each([
    ['alpha_reversed.json', ['C1', 'C2', 'C3', 'C4'], [false, true, false, true]],
    ['alpha.json', ['C1', 'C3'], [false, false]],
  ])('%s passes the shape check as its own case, its items and flags as saved', (name, items, flags) => {
    const checked = checkStored(stored(name));
    expect(checked.kind).toBe('cronbach_alpha');
    if (checked.kind !== 'cronbach_alpha') return;
    expect(checked.result.result.items.map((i) => i.item)).toEqual(items);
    expect(checked.result.result.items.map((i) => i.reversed)).toEqual(flags);
    expect(checked.result.result.k).toBe(items.length);
    expect(checked.dataFile.format).toBe('csv');
  });

  it('renders through the live card, its items by position, the same text as the live card for the same result', async () => {
    const s = stored('alpha_reversed.json');
    saved(s, RUN(1));
    const card = await screen.findByTestId('al-result-card');
    const checked = checkStored(s);
    if (checked.kind !== 'cronbach_alpha') throw new Error('not alpha');
    const savedText = card.textContent;
    expect(within(card).getByTestId('al-item-row-C2').textContent).toContain('Reversed');
    expect(within(card).getByTestId('al-item-row-C1').textContent).not.toContain('Reversed');
    cleanup();
    const live = checked.result as AlphaResultView;
    render(<AlphaResultCard result={live} names={{ column: (a) => `C${a}` }} />);
    expect(screen.getByTestId('al-result-card').textContent).toBe(savedText);
    expect(within(screen.getByTestId('al-result-card')).queryAllByRole('button')).toHaveLength(0);
  });
});
