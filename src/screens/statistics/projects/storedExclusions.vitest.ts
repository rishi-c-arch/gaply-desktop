// Gaply — a saved result's exclusions name their column wherever Rust always
// wrote one (Research OS, Phase 8H, decision 337b). The ANOVA's checker
// required it since 8G-2; the summary's, the t-test's, the one-sample's and
// the paired's did not, so a stored exclusion with its column removed was
// accepted, though no version of Gaply ever wrote one.
//
// What each method's Rust sets, at every commit that wrote its stored results
// (git log on each module, checked 1 Nov 2026):
//   summary     missing_cell, missing_code                     always, since 9c54030
//   t-test      missing_group, other_level, missing_outcome,
//               missing_code                                   always, since ee244bc
//   one-sample  missing_value, missing_code                    always, since 04d8705
//   paired      missing_first, missing_second                  always, since 04d8705
//   paired      missing_code                                   never: a code in either
//                                                              column names neither
//
// The stored results are made here from the committed snapshots (which carry
// no exclusions), with exclusions added; the snapshot files stay as they are.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkStored } from './storedShape';

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');

type O = Record<string, any>;

interface Exclusion {
  code: string;
  count: number;
  params: Record<string, number>;
}

/** A committed snapshot with `exclusions` in its result (where the method's
 *  result has them) and in its manifest. */
function withExclusions(file: string, exclusions: Exclusion[]): O {
  const view = JSON.parse(fs.readFileSync(path.join(STORED, file), 'utf8')) as O;
  const bundle = JSON.parse(view.bundleJson) as O;
  if (Array.isArray(bundle.primaryOutputs.exclusions)) bundle.primaryOutputs.exclusions = exclusions;
  bundle.manifest.exclusions = exclusions;
  return { ...view, bundleJson: JSON.stringify(bundle) };
}

const named = (code: string): Exclusion => ({ code, count: 2, params: { column: 1 } });
const unnamed = (code: string): Exclusion => ({ code, count: 2, params: {} });

const ALWAYS_NAMED: [string, string, string[]][] = [
  ['summary.json', 'summary', ['stats.desc.missing_cell', 'stats.desc.missing_code']],
  [
    'ttest.json',
    'ttest',
    ['stats.ttest.missing_group', 'stats.ttest.other_level', 'stats.ttest.missing_outcome', 'stats.ttest.missing_code'],
  ],
  ['one_sample.json', 'one_sample', ['stats.onesample.missing_value', 'stats.onesample.missing_code']],
  ['paired.json', 'paired', ['stats.onesample.missing_first', 'stats.onesample.missing_second']],
];

describe('a saved exclusion names its column where Rust always set one (decision 337b)', () => {
  for (const [file, kind, codes] of ALWAYS_NAMED) {
    for (const code of codes) {
      it(`${kind}: ${code} with its column opens`, () => {
        expect(checkStored(withExclusions(file, [named(code)])).kind).toBe(kind);
      });
      it(`${kind}: ${code} with its column removed is refused as a bad shape`, () => {
        expect(checkStored(withExclusions(file, [unnamed(code)])).kind).toBe('badShape');
      });
    }
  }

  it('paired: missing_code names no column, as Rust writes it, and opens', () => {
    expect(checkStored(withExclusions('paired.json', [unnamed('stats.onesample.missing_code')])).kind).toBe('paired');
  });

  it('one exclusion without its column refuses the result, whichever place it is in', () => {
    // The t-test carries exclusions twice: in its result and in its manifest.
    const view = withExclusions('ttest.json', [named('stats.ttest.missing_outcome')]);
    const bundle = JSON.parse(view.bundleJson) as O;
    bundle.manifest.exclusions = [unnamed('stats.ttest.missing_outcome')];
    expect(checkStored({ ...view, bundleJson: JSON.stringify(bundle) }).kind).toBe('badShape');
  });
});
