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

  it('holds every rule over 26 numeric columns, past the covariances the pairs would make (seed 20261117, 5 sequences of 60 actions)', () => {
    const ran = fuzzSemSettings(20261117, 5, 60, 26, SEM_LIMITS);
    console.log(`fuzz real 26: ${ran.actions} actions, ${ran.applied} applied, ${ran.ready} ready states; not ready: ${JSON.stringify(ran.missing)}`);
    expect(ran.actions).toBe(300);
    expect(ran.ready).toBeGreaterThan(0);
  });
});
