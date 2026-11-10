// Gaply — the rank tests' saved results pass the shape check exactly as Rust writes
// them, and nothing else (Research OS, Phase 8Y-2, decision 622), on
// storedXtab.vitest.ts's pattern.
//
// SavedResult.vitest.tsx removes every key of the two snapshots one at a time, and makes
// every number of the Kruskal-Wallis one null or a string; this file holds what those
// cannot reach:
// * the result's keys, a group's, and each test form's are exact: an EXTRA key is
//   refused, a Mann-Whitney key in a Kruskal-Wallis result among them;
// * exactP is a number or null, and nothing else;
// * a suspect-code warning names its column, value and count;
// * an exclusion is one of the four, naming its column and nothing else;
// * the not-computable codes are the three stats_ranks writes, the notes in their
//   order, the limitations exactly its seven in order.
//
// The stored results are made here from the two committed snapshots, changed in the
// test; the snapshot files stay as they are.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkStored } from './storedShape';

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');

type O = Record<string, any>;

function snapshot(file: string): O {
  return JSON.parse(fs.readFileSync(path.join(STORED, file), 'utf8')) as O;
}

/** A snapshot with its bundle changed by `edit`. */
function changed(file: string, edit: (bundle: O) => void): O {
  const view = snapshot(file);
  const bundle = JSON.parse(view.bundleJson) as O;
  edit(bundle);
  return { ...view, bundleJson: JSON.stringify(bundle) };
}

const kindOf = (v: O) => checkStored(v).kind;
const MW = 'ranks_mw.json';
const KW = 'ranks_kw.json';
const OK = 'independent_ranks';
const BAD = 'badShape';
const warning = (params: O) => ({ code: 'stats.ranks.suspect_missing_code', params, severity: 'warning' });
const mw = (b: O) => b.primaryOutputs.test.mann_whitney;
const kw = (b: O) => b.primaryOutputs.test.kruskal_wallis;
const code = (c: string) => `stats.ranks.${c}`;
const nc = (c: string) => ({ not_computable: code(c) });
/** What Rust writes for a Mann-Whitney result with ties: the tie runs, the note, no exact p. */
const withTies = (b: O) => {
  b.primaryOutputs.tieRuns = 1;
  mw(b).exactP = null;
  b.primaryOutputs.notes = [code('exact_not_given_ties'), ...b.primaryOutputs.notes];
};
/** What Rust writes when every included value is equal (stats_ranks::mann_whitney). */
const everyValueEqualMw = (b: O) => {
  withTies(b);
  mw(b).z = nc('all_values_tied');
  mw(b).p = nc('all_values_tied');
};
/** The same for three or more groups (stats_ranks::kruskal_wallis). */
const everyValueEqualKw = (b: O) => {
  b.primaryOutputs.tieRuns = 1;
  kw(b).h = nc('all_values_tied');
  kw(b).p = nc('all_values_tied');
  kw(b).epsilonSquared = nc('all_values_tied');
};

describe("the rank tests' saved results (decision 622)", () => {
  it('opens both committed snapshots as the rank tests', () => {
    expect(kindOf(snapshot(MW))).toBe(OK);
    expect(kindOf(snapshot(KW))).toBe(OK);
  });

  it('refuses an extra key in the result, in a group and in each test form', () => {
    expect(kindOf(changed(MW, (b) => (b.primaryOutputs.extra = 1)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (b.primaryOutputs.groups[0].extra = 1)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (mw(b).extra = 1)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (kw(b).extra = 1)))).toBe(BAD);
  });

  it('refuses a Mann-Whitney key in a Kruskal-Wallis result, and a Kruskal-Wallis key in a Mann-Whitney result', () => {
    expect(kindOf(changed(KW, (b) => (kw(b).exactP = 0.5)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (kw(b).u1 = 3)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.test.mann_whitney = mw({ primaryOutputs: { test: { mann_whitney: {} } } }))))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (mw(b).h = { value: 1 })))).toBe(BAD);
  });

  it('refuses a test form that does not fit the count of groups', () => {
    expect(kindOf(changed(KW, (b) => b.primaryOutputs.groups.splice(2)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => b.primaryOutputs.groups.push({ ...b.primaryOutputs.groups[0], level: 9 })))).toBe(BAD);
  });

  it('takes exactP as a number or null, and nothing else, null exactly when a note says why', () => {
    const note = `stats.ranks.exact_not_given_ties`;
    expect(kindOf(changed(MW, withTies))).toBe(OK);
    expect(kindOf(changed(MW, (b) => (mw(b).exactP = 0.25)))).toBe(OK);
    expect(kindOf(changed(MW, (b) => (mw(b).exactP = null)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (b.primaryOutputs.notes = [note, ...b.primaryOutputs.notes])))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (mw(b).exactP = '0.25')))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => delete mw(b).exactP))).toBe(BAD);
  });

  it('takes every value equal as Rust writes it, for two groups and for three', () => {
    expect(kindOf(changed(MW, everyValueEqualMw))).toBe(OK);
    expect(kindOf(changed(KW, everyValueEqualKw))).toBe(OK);
  });

  it('takes overflow where Rust writes it, a Kruskal-Wallis p with its own distribution code', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          mw(b).z = nc('overflow');
          mw(b).p = nc('overflow');
        })
      )
    ).toBe(OK);
    expect(kindOf(changed(KW, (b) => (kw(b).p = nc('distribution_out_of_range'))))).toBe(OK);
    expect(kindOf(changed(KW, (b) => (kw(b).epsilonSquared = nc('overflow'))))).toBe(OK);
  });

  it('refuses a code stats_ranks never writes there', () => {
    expect(kindOf(changed(MW, (b) => (mw(b).z = nc('same_column'))))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (kw(b).epsilonSquared = nc('no_weights'))))).toBe(BAD);
  });

  it('refuses z or its p with distribution_out_of_range, which no normal p can be', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          mw(b).z = nc('distribution_out_of_range');
          mw(b).p = nc('distribution_out_of_range');
        })
      )
    ).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (mw(b).p = nc('distribution_out_of_range'))))).toBe(BAD);
  });

  it('refuses z not computable beside a numeric p', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          withTies(b);
          mw(b).z = nc('all_values_tied');
        })
      )
    ).toBe(BAD);
  });

  it('refuses H not computable beside a numeric p or epsilon-squared', () => {
    expect(
      kindOf(
        changed(KW, (b) => {
          everyValueEqualKw(b);
          kw(b).p = { value: 0.5 };
        })
      )
    ).toBe(BAD);
    expect(
      kindOf(
        changed(KW, (b) => {
          everyValueEqualKw(b);
          kw(b).epsilonSquared = { value: 0.5 };
        })
      )
    ).toBe(BAD);
  });

  it('refuses all_values_tied with tieRuns other than 1', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          everyValueEqualMw(b);
          b.primaryOutputs.tieRuns = 2;
        })
      )
    ).toBe(BAD);
    expect(
      kindOf(
        changed(KW, (b) => {
          everyValueEqualKw(b);
          b.primaryOutputs.tieRuns = 2;
        })
      )
    ).toBe(BAD);
  });

  it('refuses duplicate or unordered levels', () => {
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.groups[1].level = b.primaryOutputs.groups[0].level)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => b.primaryOutputs.groups.reverse()))).toBe(BAD);
  });

  it('refuses a Mann-Whitney result whose first equals its second', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          b.primaryOutputs.groups[1].level = b.primaryOutputs.groups[0].level;
          mw(b).second = mw(b).first;
        })
      )
    ).toBe(BAD);
  });

  it('refuses a df other than the groups less one', () => {
    expect(kindOf(changed(KW, (b) => (kw(b).df = kw(b).df + 1)))).toBe(BAD);
  });

  it('refuses U1 and U2 that do not add up to n1 n2', () => {
    expect(kindOf(changed(MW, (b) => (mw(b).u2 = mw(b).u2 + 1)))).toBe(BAD);
  });

  it('refuses an exact p outside 0 to 1', () => {
    expect(kindOf(changed(MW, (b) => (mw(b).exactP = 7)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (mw(b).exactP = -0.5)))).toBe(BAD);
  });

  it('refuses tie runs with no ties note and an exact p, and a ties note with none', () => {
    expect(kindOf(changed(MW, (b) => (b.primaryOutputs.tieRuns = 1)))).toBe(BAD);
    expect(
      kindOf(
        changed(MW, (b) => {
          withTies(b);
          b.primaryOutputs.tieRuns = 0;
        })
      )
    ).toBe(BAD);
  });

  it('refuses a size note with no group of 50 values or more, and a group of 50 with no size note', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          mw(b).exactP = null;
          b.primaryOutputs.notes = [code('exact_not_given_size'), ...b.primaryOutputs.notes];
        })
      )
    ).toBe(BAD);
    expect(
      kindOf(
        changed(MW, (b) => {
          b.primaryOutputs.groups[0].n = 50;
          mw(b).u2 = 50 * b.primaryOutputs.groups[1].n - mw(b).u1;
        })
      )
    ).toBe(BAD);
  });

  it('refuses 51 groups', () => {
    expect(
      kindOf(
        changed(KW, (b) => {
          const g = b.primaryOutputs.groups[0];
          b.primaryOutputs.groups = Array.from({ length: 51 }, (_, level) => ({ ...g, level }));
          kw(b).df = 50;
        })
      )
    ).toBe(BAD);
  });

  it('takes 50 groups', () => {
    expect(
      kindOf(
        changed(KW, (b) => {
          const g = b.primaryOutputs.groups[0];
          b.primaryOutputs.groups = Array.from({ length: 50 }, (_, level) => ({ ...g, level }));
          kw(b).df = 49;
        })
      )
    ).toBe(OK);
  });

  it('refuses an exclusion with an extra key, a repeated or unordered reason, and manifest exclusions unlike the result\'s', () => {
    const ex = (c: string, params: O) => ({ code: code(c), count: 2, params });
    const set = (exclusions: O[]) =>
      changed(KW, (b) => {
        b.primaryOutputs.exclusions = exclusions;
        b.manifest.exclusions = exclusions;
      });
    expect(kindOf(set([{ ...ex('missing_group', { column: 2 }), extra: 1 }]))).toBe(BAD);
    expect(kindOf(set([ex('missing_group', { column: 2 }), ex('missing_group', { column: 2 })]))).toBe(BAD);
    expect(kindOf(set([ex('other_level', { column: 2 }), ex('missing_group', { column: 2 })]))).toBe(BAD);
    expect(
      kindOf(
        changed(KW, (b) => {
          b.primaryOutputs.exclusions = [ex('missing_group', { column: 2 })];
          b.manifest.exclusions = [];
        })
      )
    ).toBe(BAD);
  });

  it('opens a suspect-code warning with its column, value and count, and refuses one without any of them', () => {
    const full = { column: 1, value: -99, count: 3 };
    expect(kindOf(changed(MW, (b) => (b.diagnostics = [warning(full)])))).toBe(OK);
    for (const k of ['column', 'value', 'count']) {
      const less: O = { ...full };
      delete less[k];
      expect(kindOf(changed(MW, (b) => (b.diagnostics = [warning(less)]))), k).toBe(BAD);
    }
  });

  it('opens each exclusion naming its column, and refuses one naming none, more, or another code', () => {
    const ex = (code: string, params: O) => ({ code: `stats.ranks.${code}`, count: 2, params });
    const set = (exclusions: O[]) =>
      changed(KW, (b) => {
        b.primaryOutputs.exclusions = exclusions;
        b.manifest.exclusions = exclusions;
      });
    expect(kindOf(set([ex('missing_group', { column: 2 }), ex('other_level', { column: 2 }), ex('missing_outcome', { column: 1 }), ex('missing_code', { column: 1 })]))).toBe(OK);
    expect(kindOf(set([ex('missing_group', {})]))).toBe(BAD);
    expect(kindOf(set([ex('other_level', { column: 2, count: 1 })]))).toBe(BAD);
    expect(kindOf(set([ex('same_column', { column: 1 })]))).toBe(BAD);
  });

  it("refuses first and second groups that are not the result's two, in order", () => {
    expect(kindOf(changed(MW, (b) => (mw(b).first = 9)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (mw(b).second = 9)))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => ([mw(b).first, mw(b).second] = [mw(b).second, mw(b).first])))).toBe(BAD);
  });

  it('refuses an exact note on a Kruskal-Wallis result', () => {
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.notes = ['stats.ranks.exact_not_given_size'])))).toBe(BAD);
  });

  it('refuses notes out of their order or of another code, and the limitations reordered', () => {
    const notes = (list: string[]) => list.map((c) => `stats.ranks.${c}`);
    expect(
      kindOf(
        changed(MW, (b) => {
          mw(b).exactP = null;
          b.primaryOutputs.tieRuns = 1;
          b.primaryOutputs.groups[0].n = 50;
          mw(b).u2 = 50 * b.primaryOutputs.groups[1].n - mw(b).u1;
          b.primaryOutputs.notes = notes(['exact_not_given_ties', 'exact_not_given_size', 'group_below_five']);
        })
      )
    ).toBe(OK);
    expect(
      kindOf(
        changed(MW, (b) => {
          mw(b).exactP = null;
          b.primaryOutputs.tieRuns = 1;
          b.primaryOutputs.notes = notes(['group_below_five', 'exact_not_given_ties']);
        })
      )
    ).toBe(BAD);
    expect(kindOf(changed(MW, (b) => (b.primaryOutputs.notes = notes(['no_weights']))))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => b.primaryOutputs.limitations.reverse()))).toBe(BAD);
  });
});
