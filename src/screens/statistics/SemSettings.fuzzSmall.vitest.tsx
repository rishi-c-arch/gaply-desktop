// The structural equation model's editor, fuzzed at small limits (Research OS, Phase 9C-5b,
// decision 823's N9): fixtures/semFixtures is mocked so the screen's caps are 5 columns, 2
// factors, 3 paths, 4 covariances and 3 members held equal, and a view of 7 numeric columns
// reaches every one of them. The driver and its checks are semSpecRules.ts's, as the real-limits
// file's; the caps it checks are these. Its red was the review's planted fault, a path end allowed
// at a factor's column (the report's mutation record). Split into its of 10 sequences, as the
// real-limits file is, for the same reason.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup } from '@testing-library/react';

import { semLimits } from './fixtures/semFixtures';
import { MAX_COLUMNS, MAX_COVARIANCES, MAX_EQUAL_MEMBERS, MAX_FACTORS, MAX_PATHS } from './semWording';
import { checkSemSpec, fuzzSemSettings, type FuzzAtCap } from './semSpecRules';
import type { SemParams } from './semTypes';

vi.mock('./fixtures/semFixtures', async (importOriginal) => {
  const real = await importOriginal<typeof import('./fixtures/semFixtures')>();
  return { ...real, semLimits: { ...real.semLimits, maxColumns: 5, maxFactors: 2, maxPaths: 3, maxCovariances: 4, maxEqualMembers: 3 } };
});

afterEach(cleanup);

const CHUNK = 10;

describe('the SEM settings at small limits, fuzzed', () => {
  it('reads the small limits through the mock', () => {
    expect([semLimits.maxColumns, semLimits.maxFactors, semLimits.maxPaths, semLimits.maxCovariances, semLimits.maxEqualMembers]).toEqual([5, 2, 3, 4, 3]);
    // What the screen reads (the review of this step, finding 2): semWording's limits, which SemSettings imports.
    expect([MAX_COLUMNS, MAX_FACTORS, MAX_PATHS, MAX_COVARIANCES, MAX_EQUAL_MEMBERS]).toEqual([5, 2, 3, 4, 3]);
  });

  it('checks a spec against the small caps it is given, not semLimits.json\'s (decision 848\'s E4)', () => {
    const col = (c: number) => ({ column: `C${c}` });
    // Paths into C2 from C3 on; covariances of C1 with C3 on; factors on C10 and C11, C12 and C13,
    // and so on; variances held equal of C1, C3, C4 and on, each a column of the model.
    const spec = (paths: number, covariances: number, factors: number, members: number): SemParams => ({
      indicators: Array.from({ length: factors }, (_, f) => [`C${10 + 2 * f}`, `C${11 + 2 * f}`]),
      paths: Array.from({ length: paths }, (_, i) => ({ to: col(2), from: col(3 + i) })),
      covariances: Array.from({ length: covariances }, (_, i) => ({ a: col(1), b: col(3 + i) })),
      equal: members > 0 ? [Array.from({ length: members }, (_, i) => ({ variance: { variable: col(i === 0 ? 1 : 2 + i) } }))] : [],
      missingCodes: [],
    });
    // One past each small cap: 4 paths (5 columns, C2 to C6); 5 covariances (6 columns, C1 and C3
    // to C7, past 5 too); 3 factors; 4 members (with 4 covariances and a path, C1 to C6).
    expect(checkSemSpec(spec(4, 0, 0, 0), semLimits)).toEqual(['too_many_paths']);
    expect(checkSemSpec(spec(0, 5, 0, 0), semLimits)).toEqual(['too_many_covariances', 'too_many_columns']);
    expect(checkSemSpec(spec(1, 0, 3, 0), semLimits)).toContain('too_many_factors');
    expect(checkSemSpec(spec(1, 4, 0, 4), semLimits)).toContain('too_many_equal_members');
    // Each cap exactly: 3 paths (4 columns); 4 covariances (5 columns) and 3 members.
    expect(checkSemSpec(spec(3, 0, 0, 0), semLimits)).toEqual([]);
    expect(checkSemSpec(spec(0, 4, 0, 3), semLimits)).toEqual([]);
  });

  // With the planted fault applied by hand, 50 sequences of 50 actions caught it at 6 of seeds
  // 20261118 to 20261127, this one at its sequence 13 (9c5b/logs/p0_fuzz_seeds_*.log).
  const total = { actions: 0, ready: 0, atCap: { columns: 0, factors: 0, paths: 0, covariances: 0, members: 0 } as FuzzAtCap };
  for (let first = 0; first < 50; first += CHUNK) {
    it(`holds every rule over 7 numeric columns (seed 20261122, sequences ${first} to ${first + CHUNK - 1} of 50 actions)`, () => {
      const ran = fuzzSemSettings(20261122, CHUNK, 50, 7, semLimits, first);
      total.actions += ran.actions;
      total.ready += ran.ready;
      for (const k of Object.keys(total.atCap) as Array<keyof FuzzAtCap>) total.atCap[k] += ran.atCap[k];
      console.log(`fuzz small 7 from ${first}: ${ran.actions} actions, ${ran.ready} ready states, at caps ${JSON.stringify(ran.atCap)}`);
    });
  }
  it('ran 50 sequences of 50 actions, reached ready states, and reached every cap', () => {
    expect(total.actions).toBe(2500);
    expect(total.ready).toBeGreaterThan(0);
    // Every cap was reached, so each cap check ran on a state at it (the review of this step, finding 1).
    for (const [cap, n] of Object.entries(total.atCap)) expect(n, cap).toBeGreaterThan(0);
  });
});
