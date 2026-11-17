// The structural equation model's editor, fuzzed at the real limits (Research OS, Phase 9C-5b,
// decision 823's N9, on the design of the review of 365b638): seeded random user actions through
// the screen, each state checked by semSpecRules.ts's driver (no cap passed, no parameter in two
// groups, no label outside text, and every ready spec passing the TypeScript copy of check_spec,
// naming only the view's numeric columns and holding every row and member the screen shows). A
// failure prints its seed and its shortest failing sequence. Its red was the review's planted
// fault, a path end allowed at a factor's column (the report's mutation record).
//
// Each run is split into its of 10 sequences, so each finishes inside the config's 20 s under the
// whole suite's load at default workers (one it of 50 took 33 s there); the sequences are the same
// as one run of 50, and the totals are asserted after the last chunk.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup } from '@testing-library/react';

import SEM_LIMITS from './fixtures/semLimits.json';
import { fuzzSemSettings } from './semSpecRules';

afterEach(cleanup);

const CHUNK = 10;

describe('the SEM settings at the real limits, fuzzed', () => {
  // Five columns: a narrower view reaches ready states far more often. With the planted fault
  // applied by hand, 50 sequences of 50 actions over 5 columns caught it at 7 of seeds 20261116 to
  // 20261125, this one at its sequence 9 (9c5b/logs/p0_fuzz_seeds_*.log).
  const five = { actions: 0, ready: 0 };
  for (let first = 0; first < 50; first += CHUNK) {
    it(`holds every rule over 5 numeric columns (seed 20261118, sequences ${first} to ${first + CHUNK - 1} of 50 actions)`, () => {
      const ran = fuzzSemSettings(20261118, CHUNK, 50, 5, SEM_LIMITS, first);
      five.actions += ran.actions;
      five.ready += ran.ready;
      console.log(`fuzz real 5 from ${first}: ${ran.actions} actions, ${ran.ready} ready states; not ready: ${JSON.stringify(ran.missing)}`);
    });
  }
  it('ran 50 sequences of 50 actions over 5 columns, and reached ready states', () => {
    expect(five.actions).toBe(2500);
    expect(five.ready).toBeGreaterThan(0);
  });

  // Decision 848's E2: each sequence starts with 24 columns on their own, by one bulk action through
  // the settings' handle, so states where adding every pair would pass the most covariances (276
  // pairs of 24, past 256) are reached; their tally must be above 0. One sequence per it: with 24
  // columns drawn at every step, an it of all five took more than 20 s under load. Each it has its own
  // timeout of 90 s: at default workers in the whole suite one sequence took 39.4 s (17 Nov 2026,
  // 9c5f/logs/final_vitest_default_1794890571.log), past the config's 20 s.
  const wide = { actions: 0, ready: 0, pairsPast: 0 };
  for (let first = 0; first < 5; first += 1) {
    it(`holds every rule over 26 numeric columns, past the covariances the pairs would make (seed 20261117, sequence ${first} of 5, 60 actions)`, () => {
      const ran = fuzzSemSettings(20261117, 1, 60, 26, SEM_LIMITS, first, { startOwn: 24 });
      wide.actions += ran.actions;
      wide.ready += ran.ready;
      wide.pairsPast += ran.pairsPast;
      console.log(`fuzz real 26 from ${first}: ${ran.actions} actions, ${ran.applied} applied, ${ran.ready} ready states, ${ran.pairsPast} past the pairs' cap; not ready: ${JSON.stringify(ran.missing)}`);
    }, 90000);
  }
  it('ran 5 sequences of 60 actions over 26 columns, reached ready states, and reached states past the pairs\' cap', () => {
    expect(wide.actions).toBe(300);
    expect(wide.ready).toBeGreaterThan(0);
    expect(wide.pairsPast).toBeGreaterThan(0);
  });
});
