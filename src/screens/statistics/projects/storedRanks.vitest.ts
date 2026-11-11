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

/** A snapshot with its bundle changed by `edit`, and its spec's parameters by `params`. */
function changed(file: string, edit: (bundle: O) => void, params?: (p: O) => void): O {
  const view = snapshot(file);
  const bundle = JSON.parse(view.bundleJson) as O;
  edit(bundle);
  const spec = JSON.parse(view.specJson) as O;
  if (params) params(spec.parameters);
  return { ...view, bundleJson: JSON.stringify(bundle), specJson: JSON.stringify(spec) };
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
/** z as stats_ranks::mann_whitney computes it with no tie runs (stats_ranks/mod.rs:829-832), in
 *  Rust's operation order: the tie sum is 0 when no two values are tied (decision 668). */
const mannWhitneyZ = (u1: number, n1: number, n2: number) => {
  const bigN = n1 + n2;
  const variance = ((n1 * n2) / 12) * (bigN + 1 - 0 / (bigN * (bigN - 1)));
  return (u1 - (n1 * n2) / 2) / Math.sqrt(variance);
};
/** What Rust writes for a Mann-Whitney result with ties: the tie runs, the note, no exact p. */
const withTies = (b: O) => {
  b.primaryOutputs.tieRuns = 1;
  mw(b).exactP = null;
  b.primaryOutputs.notes = [code('exact_not_given_ties'), ...b.primaryOutputs.notes];
};
/** Every group's rank sum and mean rank as one run of N equal values gives them. */
const ranksOfOneRun = (b: O) => {
  const r = b.primaryOutputs;
  r.tieRuns = 1;
  for (const g of r.groups) {
    g.rankSum = (g.n * (r.n + 1)) / 2;
    g.meanRank = g.rankSum / g.n;
  }
};
/** What Rust writes when every included value is equal (stats_ranks::mann_whitney): decision
 *  652, rank sums included, so the case passes for its own reason. */
const everyValueEqualMw = (b: O) => {
  withTies(b);
  ranksOfOneRun(b);
  const [g1, g2] = b.primaryOutputs.groups;
  mw(b).u1 = g1.rankSum - (g1.n * (g1.n + 1)) / 2;
  mw(b).u2 = g1.n * g2.n - mw(b).u1;
  mw(b).rankBiserial = (2 * mw(b).u1) / (g1.n * g2.n) - 1;
  mw(b).z = nc('all_values_tied');
  mw(b).p = nc('all_values_tied');
};
/** The same for three or more groups (stats_ranks::kruskal_wallis). */
const everyValueEqualKw = (b: O) => {
  ranksOfOneRun(b);
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

  // Decision 668: z and p overflowed with no tie runs, and epsilon-squared overflowed beside a
  // numeric H, were accepted here; Rust cannot write either (stats_ranks/mod.rs:829-831, :903),
  // so both are now refusals, in "what stats_ranks never writes (decision 668)" below.
  it('takes a Kruskal-Wallis p with its own distribution code', () => {
    expect(kindOf(changed(KW, (b) => (kw(b).p = nc('distribution_out_of_range'))))).toBe(OK);
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
    // Decision 668: a first group of 50 and a second of 5 as Rust would write them with no
    // ties, u1 at n1 n2 / 2 so Rust's z is 0, and only the size note missing.
    expect(
      kindOf(
        changed(MW, (b) => {
          const r = b.primaryOutputs;
          r.n = 55;
          Object.assign(r.groups[0], { n: 50, rankSum: 1400, meanRank: 1400 / 50 });
          Object.assign(r.groups[1], { n: 5, rankSum: 140, meanRank: 140 / 5 });
          mw(b).u1 = 1400 - (50 * 51) / 2;
          mw(b).u2 = 50 * 5 - mw(b).u1;
          mw(b).rankBiserial = (2 * mw(b).u1) / (50 * 5) - 1;
          mw(b).z = { value: mannWhitneyZ(mw(b).u1, 50, 5) };
          r.notes = [];
          b.manifest.rowsUsed = 55;
          b.manifest.rowsRead = 55 + r.exclusions.reduce((s: number, e: O) => s + e.count, 0);
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
          // Decision 652: n, the rank sums and the manifest as Rust would write them for 50 groups
          // whose ranks balance, H 0.
          const N = g.n * 50;
          const rankSum = (g.n * (N + 1)) / 2;
          b.primaryOutputs.groups = Array.from({ length: 50 }, (_, level) => ({ ...g, level, rankSum, meanRank: rankSum / g.n }));
          b.primaryOutputs.n = N;
          b.manifest.rowsUsed = N;
          b.manifest.rowsRead = N;
          kw(b).df = 49;
          kw(b).h = { value: 0 };
          kw(b).p = { value: 1 };
          kw(b).epsilonSquared = { value: 0 };
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
    // Decision 652: the manifest's rows read and the spec's codes as Rust would have them.
    const set = (exclusions: O[]) =>
      changed(
        KW,
        (b) => {
          b.primaryOutputs.exclusions = exclusions;
          b.manifest.exclusions = exclusions;
          b.manifest.rowsRead = b.manifest.rowsUsed + 2 * exclusions.length;
        },
        (p) => {
          p.missingCodes = [-99];
          // other_level needs a level left out of the three included.
          p.levels = [0, 1, 2];
        }
      );
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
          // Decision 652: a first group of 50 and a second of 4, as Rust would write them.
          mw(b).exactP = null;
          const r = b.primaryOutputs;
          r.tieRuns = 1;
          r.n = 54;
          Object.assign(r.groups[0], { n: 50, rankSum: 1400, meanRank: 1400 / 50 });
          Object.assign(r.groups[1], { n: 4, rankSum: 85, meanRank: 85 / 4 });
          mw(b).u1 = 1400 - (50 * 51) / 2;
          mw(b).u2 = 50 * 4 - mw(b).u1;
          mw(b).rankBiserial = (2 * mw(b).u1) / (50 * 4) - 1;
          // Decision 668: u1 = 125 is above n1 n2 / 2 = 100, so Rust's z is positive.
          mw(b).z = { value: 1.71 };
          b.manifest.rowsUsed = 54;
          b.manifest.rowsRead = 54 + b.primaryOutputs.exclusions.reduce((s: number, e: O) => s + e.count, 0);
          r.notes = notes(['exact_not_given_ties', 'exact_not_given_size', 'group_below_five']);
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

/** Decisions 651 and 652: what Rust never writes, and the spec's half of it. Each case its
 *  own rule, each rule's red a mutation (decision 502). */
describe("what stats_ranks never writes (decisions 651, 652)", () => {
  const exc = (c: string, column: number, count = 1) => ({ code: code(c), count, params: { column } });

  it('refuses all_values_tied on one of z and p and not the other, either way round', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          withTies(b);
          mw(b).p = nc('all_values_tied');
        })
      )
    ).toBe(BAD);
    expect(
      kindOf(
        changed(MW, (b) => {
          withTies(b);
          mw(b).z = nc('all_values_tied');
        })
      )
    ).toBe(BAD);
  });

  it("refuses a numeric H beside a p or an epsilon-squared of all_values_tied", () => {
    expect(kindOf(changed(KW, (b) => (kw(b).p = nc('all_values_tied'))))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (kw(b).epsilonSquared = nc('all_values_tied'))))).toBe(BAD);
  });

  it('refuses an H or an epsilon-squared of distribution_out_of_range, which only a p can have', () => {
    expect(
      kindOf(
        changed(KW, (b) => {
          kw(b).h = nc('distribution_out_of_range');
          kw(b).p = nc('distribution_out_of_range');
          kw(b).epsilonSquared = nc('distribution_out_of_range');
        })
      )
    ).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (kw(b).epsilonSquared = nc('distribution_out_of_range'))))).toBe(BAD);
  });

  it("refuses decision 651's example: every value equal with a rank sum one run of N cannot give", () => {
    // u1 = 3 for n1 = 4 and n2 = 5; the fixture's rank sum (13) and u1 agree with each other.
    expect(
      kindOf(
        changed(MW, (b) => {
          withTies(b);
          mw(b).z = nc('all_values_tied');
          mw(b).p = nc('all_values_tied');
        })
      )
    ).toBe(BAD);
    expect(
      kindOf(
        changed(KW, (b) => {
          b.primaryOutputs.tieRuns = 1;
          kw(b).h = nc('all_values_tied');
          kw(b).p = nc('all_values_tied');
          kw(b).epsilonSquared = nc('all_values_tied');
        })
      )
    ).toBe(BAD);
  });

  it('refuses a u1 other than the first rank sum less n1 (n1 + 1) / 2', () => {
    expect(
      kindOf(
        changed(MW, (b) => {
          mw(b).u1 += 1;
          mw(b).u2 -= 1;
          mw(b).rankBiserial = (2 * mw(b).u1) / (b.primaryOutputs.groups[0].n * b.primaryOutputs.groups[1].n) - 1;
          // Decision 668: Rust's z for the edited u1, so only the u1 rule refuses it.
          mw(b).z = { value: mannWhitneyZ(mw(b).u1, 4, 5) };
        })
      )
    ).toBe(BAD);
  });

  it('refuses a u2 other than n1 n2 less u1', () => {
    expect(kindOf(changed(MW, (b) => (mw(b).u2 += 0.5)))).toBe(BAD);
  });

  it('refuses a rank-biserial correlation other than 2 u1 / (n1 n2) - 1', () => {
    expect(kindOf(changed(MW, (b) => (mw(b).rankBiserial = 0.25)))).toBe(BAD);
  });

  it('refuses a mean rank other than the rank sum over n', () => {
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.groups[0].meanRank += 1)))).toBe(BAD);
  });

  it("refuses n other than the groups' sum, a group of 0 and more tie runs than n / 2", () => {
    expect(
      kindOf(
        changed(KW, (b) => {
          b.primaryOutputs.n += 1;
          b.manifest.rowsUsed += 1;
          b.manifest.rowsRead += 1;
        })
      )
    ).toBe(BAD);
    expect(
      kindOf(
        changed(KW, (b) => {
          const g = b.primaryOutputs.groups[0];
          b.primaryOutputs.n -= g.n;
          b.manifest.rowsUsed -= g.n;
          b.manifest.rowsRead -= g.n;
          g.n = 0;
          g.meanRank = g.rankSum / 1;
        })
      )
    ).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.tieRuns = 1000000)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.tieRuns = Math.floor(b.primaryOutputs.n / 2) + 1)))).toBe(BAD);
  });

  it('refuses group_below_five dropped although a group is below 5, and present although none is', () => {
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.notes = [])))).toBe(BAD);
    expect(
      kindOf(
        changed(MW, (b) => {
          const r = b.primaryOutputs;
          r.groups[0].n = 6;
          r.groups[1].n = 6;
          r.n = 12;
          r.groups[0].rankSum = 39;
          r.groups[1].rankSum = 39;
          r.groups[0].meanRank = 6.5;
          r.groups[1].meanRank = 6.5;
          mw(b).u1 = 18;
          mw(b).u2 = 18;
          mw(b).rankBiserial = 0;
          // Decision 668: Rust's z for u1 = n1 n2 / 2, so only the note rule refuses it.
          mw(b).z = { value: mannWhitneyZ(18, 6, 6) };
          b.manifest.rowsUsed = 12;
          b.manifest.rowsRead = 12 + r.exclusions.reduce((s: number, e: O) => s + e.count, 0);
        })
      )
    ).toBe(BAD);
  });

  it("refuses a manifest's rowsUsed other than n, and rowsRead other than rowsUsed and the exclusions", () => {
    expect(kindOf(changed(KW, (b) => (b.manifest.rowsUsed += 1)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (b.manifest.rowsRead += 1)))).toBe(BAD);
  });

  it('refuses an exclusion naming a column other than the one Rust names for its code', () => {
    const on = (list: O[]) =>
      changed(
        MW,
        (b) => {
          b.primaryOutputs.exclusions = list;
          b.manifest.exclusions = list;
          b.manifest.rowsRead = b.manifest.rowsUsed + list.length;
        },
        (p) => (p.missingCodes = [-99])
      );
    expect(kindOf(on([exc('missing_group', 2), exc('missing_outcome', 1)]))).toBe(OK);
    expect(kindOf(on([exc('missing_group', 1)]))).toBe(BAD);
    expect(kindOf(on([exc('other_level', 3)]))).toBe(BAD);
    expect(kindOf(on([exc('missing_outcome', 2)]))).toBe(BAD);
    expect(kindOf(on([exc('missing_code', 2)]))).toBe(BAD);
  });

  it('refuses missing_code when the spec has no missing codes', () => {
    expect(kindOf(changed(MW, () => undefined, (p) => (p.missingCodes = [])))).toBe(BAD);
  });

  it("takes levels the spec gives, in any order Rust accepts, and refuses groups that are not the spec's", () => {
    expect(kindOf(changed(MW, () => undefined, (p) => (p.levels = [1, 0])))).toBe(OK);
    expect(kindOf(changed(MW, () => undefined, (p) => (p.levels = [0, 2])))).toBe(BAD);
    expect(kindOf(changed(MW, () => undefined, (p) => (p.levels = [0])))).toBe(BAD);
    expect(kindOf(changed(KW, () => undefined, (p) => (p.levels = null)))).toBe(OK);
  });

  it('refuses other_level when every level was included, which split cannot leave out', () => {
    expect(kindOf(changed(MW, () => undefined, (p) => (p.levels = null)))).toBe(BAD);
  });

  it('refuses an outcome that is also the group, which Rust refuses as same_column', () => {
    expect(kindOf(changed(KW, () => undefined, (p) => (p.group = p.outcome)))).toBe(BAD);
  });

  it('refuses a spec whose parameters lack a field the checker reads, or name no column', () => {
    // On the fixture with no exclusion, so these rules alone can refuse them (a missing code
    // would otherwise throw, and a named column fail to match).
    for (const k of ['outcome', 'group', 'levels', 'missingCodes']) {
      expect(kindOf(changed(KW, () => undefined, (p) => delete p[k])), k).toBe(BAD);
    }
    expect(kindOf(changed(KW, () => undefined, (p) => (p.outcome = 'score')))).toBe(BAD);
    expect(kindOf(changed(KW, () => undefined, (p) => (p.levels = 'all')))).toBe(BAD);
    expect(kindOf(changed(KW, () => undefined, (p) => (p.missingCodes = 'none')))).toBe(BAD);
  });

  it('each repeated or unordered exclusion is refused for that reason, its rows read being right', () => {
    const set = (list: O[]) =>
      changed(KW, (b) => {
        b.primaryOutputs.exclusions = list;
        b.manifest.exclusions = list;
        b.manifest.rowsRead = b.manifest.rowsUsed + list.length;
      });
    expect(kindOf(set([exc('missing_group', 2), exc('missing_group', 2)]))).toBe(BAD);
    expect(kindOf(set([exc('other_level', 2), exc('missing_group', 2)]))).toBe(BAD);
  });
});

/** Decision 668: what stats_ranks never writes, found by decision 667's review. Each case its
 *  own rule, each rule's red a mutation (decision 502). */
describe('what stats_ranks never writes (decision 668)', () => {
  /** 50 groups whose ranks balance, as "takes 50 groups" builds them, H set by `h`. */
  const balanced50 = (b: O, h: number) => {
    const g = b.primaryOutputs.groups[0];
    const N = g.n * 50;
    const rankSum = (g.n * (N + 1)) / 2;
    b.primaryOutputs.groups = Array.from({ length: 50 }, (_, level) => ({ ...g, level, rankSum, meanRank: rankSum / g.n }));
    b.primaryOutputs.n = N;
    b.manifest.rowsUsed = N;
    b.manifest.rowsRead = N;
    kw(b).df = 49;
    kw(b).h = { value: h };
    kw(b).p = { value: 1 };
    kw(b).epsilonSquared = { value: h / (N - 1) };
  };
  /** The Mann-Whitney snapshot's groups made 6 and 6 with u1 = n1 n2 / 2, tied, so only the
   *  sign rule constrains z. */
  const centredWithTies = (b: O) => {
    withTies(b);
    const r = b.primaryOutputs;
    Object.assign(r.groups[0], { n: 6, rankSum: 39, meanRank: 6.5 });
    Object.assign(r.groups[1], { n: 6, rankSum: 39, meanRank: 6.5 });
    r.n = 12;
    r.notes = [code('exact_not_given_ties')];
    mw(b).u1 = 18;
    mw(b).u2 = 18;
    mw(b).rankBiserial = 0;
    mw(b).z = { value: 0 };
    mw(b).p = { value: 1 };
    b.manifest.rowsUsed = 12;
    b.manifest.rowsRead = 12 + r.exclusions.reduce((s: number, e: O) => s + e.count, 0);
  };

  it('takes a centred, tied Mann-Whitney result with z 0', () => {
    expect(kindOf(changed(MW, centredWithTies))).toBe(OK);
  });

  it('refuses a negative H', () => {
    expect(kindOf(changed(KW, (b) => {
      kw(b).h = { value: -1 };
      kw(b).epsilonSquared = { value: -1 / (b.primaryOutputs.n - 1) };
    }))).toBe(BAD);
  });

  it('refuses an H of 0 while a rank sum is off its centre', () => {
    expect(kindOf(changed(KW, (b) => {
      kw(b).h = { value: 0 };
      kw(b).epsilonSquared = { value: 0 };
      kw(b).p = { value: 1 };
    }))).toBe(BAD);
  });

  it('refuses an H above 0 while every rank sum is at its centre', () => {
    expect(kindOf(changed(KW, (b) => balanced50(b, 0)))).toBe(OK);
    expect(kindOf(changed(KW, (b) => balanced50(b, 2)))).toBe(BAD);
  });

  it('refuses an epsilon-squared other than H / (N - 1)', () => {
    expect(kindOf(changed(KW, (b) => (kw(b).epsilonSquared = { value: 0.5 })))).toBe(BAD);
  });

  it('refuses an epsilon-squared that overflowed beside a numeric H, which Rust cannot write', () => {
    expect(kindOf(changed(KW, (b) => (kw(b).epsilonSquared = nc('overflow'))))).toBe(BAD);
  });

  it('refuses a Kruskal-Wallis p outside 0 to 1', () => {
    expect(kindOf(changed(KW, (b) => (kw(b).p = { value: 1.5 })))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => (kw(b).p = { value: -0.5 })))).toBe(BAD);
  });

  it('refuses a Mann-Whitney p outside 0 to 1', () => {
    expect(kindOf(changed(MW, (b) => {
      withTies(b);
      mw(b).p = { value: 1.5 };
    }))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => {
      withTies(b);
      mw(b).p = { value: -0.5 };
    }))).toBe(BAD);
  });

  it('refuses a z of the wrong sign for u1 - n1 n2 / 2, with ties', () => {
    expect(kindOf(changed(MW, (b) => {
      withTies(b);
      mw(b).z = { value: 1.7 };
    }))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => {
      withTies(b);
      mw(b).z = { value: 0 };
    }))).toBe(BAD);
    expect(kindOf(changed(MW, (b) => {
      centredWithTies(b);
      mw(b).z = { value: 0.25 };
    }))).toBe(BAD);
  });

  it('refuses an exact p of 0', () => {
    expect(kindOf(changed(MW, (b) => (mw(b).exactP = 0)))).toBe(BAD);
  });

  it("refuses, with no tie runs, a z other than Rust's from u1, n1 and n2, of the right sign", () => {
    expect(kindOf(changed(MW, (b) => (mw(b).z = { value: mannWhitneyZ(3, 4, 5) * 2 })))).toBe(BAD);
  });

  it('refuses, with no tie runs, z and p that overflowed, which Rust cannot write there', () => {
    expect(kindOf(changed(MW, (b) => {
      mw(b).z = nc('overflow');
      mw(b).p = nc('overflow');
    }))).toBe(BAD);
  });

  it("refuses, with the spec's levels null, groups at levels other than 0 to k - 1", () => {
    expect(kindOf(changed(KW, (b) => (b.primaryOutputs.groups[2].level = 3)))).toBe(BAD);
    expect(kindOf(changed(KW, (b) => b.primaryOutputs.groups.forEach((g: O) => (g.level += 1))))).toBe(BAD);
  });
});
