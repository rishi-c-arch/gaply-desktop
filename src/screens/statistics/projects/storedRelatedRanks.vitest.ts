// Gaply — the related rank tests' saved results pass the shape check exactly as Rust
// writes them, and nothing else (Research OS, Phase 8Z-2, decision 657), on
// storedRanks.vitest.ts's pattern. Each case breaks one rule, the rest of the result kept as
// Rust would write it, so each is refused for its own reason; each case's red is a mutation
// (decision 502). The stored results are made here from the two committed snapshots,
// changed in the test; the snapshot files stay as they are.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { checkStored } from './storedShape';

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');

type O = Record<string, any>;

const snapshot = (file: string): O => JSON.parse(fs.readFileSync(path.join(STORED, file), 'utf8')) as O;

function changed(file: string, edit: (bundle: O) => void, params?: (p: O) => void): O {
  const view = snapshot(file);
  const bundle = JSON.parse(view.bundleJson) as O;
  edit(bundle);
  const spec = JSON.parse(view.specJson) as O;
  if (params) params(spec.parameters);
  return { ...view, bundleJson: JSON.stringify(bundle), specJson: JSON.stringify(spec) };
}

const kindOf = (v: O) => checkStored(v).kind;
const W = 'related_ranks_w.json';
const F = 'related_ranks_f.json';
const OK = 'related_ranks';
const BAD = 'badShape';
const code = (c: string) => `stats.related_ranks.${c}`;
const nc = (c: string) => ({ not_computable: code(c) });
const wil = (b: O) => b.primaryOutputs.test.wilcoxon;
const fri = (b: O) => b.primaryOutputs.test.friedman;

/** z as stats_related_ranks::z_and_p computes it with no tie runs (stats_related_ranks/mod.rs:700-701),
 *  in Rust's operation order: the tie sum is 0 when no two sizes are tied (decision 668). */
const wilcoxonZ = (v: number, n: number) => {
  const variance = (n * (n + 1) * (2 * n + 1)) / 24 - 0 / 48;
  return (v - (n * (n + 1)) / 4) / Math.sqrt(variance);
};

/** A Wilcoxon result as Rust would write it for these two sides and zero differences. */
function setW(b: O, neg: [number, number], pos: [number, number], zero = 0, tieRuns = 0) {
  const r = b.primaryOutputs;
  const t = wil(b);
  const side = ([count, rankSum]: [number, number]) => ({ count, rankSum, meanRank: count > 0 ? rankSum / count : null });
  t.negative = side(neg);
  t.positive = side(pos);
  const n = neg[0] + pos[0];
  t.n = n;
  t.zeroDifferences = zero;
  t.tieRuns = tieRuns;
  t.v = pos[1];
  const top = (n * (n + 1)) / 2;
  t.tMinus = top - t.v;
  t.negative.rankSum = t.tMinus;
  if (neg[0] > 0) t.negative.meanRank = t.tMinus / neg[0];
  if (n === 0) {
    t.z = nc('all_differences_zero');
    t.p = nc('all_differences_zero');
    t.rankBiserial = nc('all_differences_zero');
    // Decision 668: every difference zero, so Rust's median difference is 0.
    t.medianDifference = { value: 0 };
  } else {
    // Decision 668: Rust's z when no two sizes are tied; a placeholder of the right sign otherwise.
    t.z = { value: tieRuns === 0 ? wilcoxonZ(t.v, n) : Math.sign(t.v - t.tMinus) * 0.5 };
    t.p = { value: 0.5 };
    t.rankBiserial = { value: (t.v - t.tMinus) / top };
  }
  r.rowsUsed = n + zero;
  const notes: string[] = [];
  if (tieRuns > 0) notes.push(code('exact_not_given_ties'));
  if (zero > 0) notes.push(code('exact_not_given_zeros'));
  if (n >= 50) notes.push(code('exact_not_given_size'));
  t.exactP = notes.length === 0 ? 0.5 : null;
  if (zero > 0) notes.push(code('zero_differences_dropped'));
  if (n < 5) notes.push(code('rows_below_five'));
  r.notes = notes;
  b.manifest.rowsUsed = r.rowsUsed;
  b.manifest.rowsRead = r.rowsUsed + r.exclusions.reduce((s: number, e: O) => s + e.count, 0);
}

/** A Friedman result as Rust would write it for these rank sums and rows. */
function setF(b: O, rowsUsed: number, sums: number[], q: unknown = { value: 1 }) {
  const r = b.primaryOutputs;
  const t = fri(b);
  r.rowsUsed = rowsUsed;
  t.columns = sums.map((rankSum, i) => ({ column: t.columns[i].column, rankSum, meanRank: rankSum / rowsUsed }));
  t.q = q;
  const k = sums.length;
  if (typeof q === 'object' && q !== null && 'value' in q) {
    t.p = { value: 0.25 };
    t.kendallsW = { value: (q as { value: number }).value / (rowsUsed * (k - 1)) };
  } else {
    t.p = q;
    t.kendallsW = q;
  }
  r.notes = rowsUsed < 5 ? [code('rows_below_five')] : [];
  b.manifest.rowsUsed = rowsUsed;
  b.manifest.rowsRead = rowsUsed + r.exclusions.reduce((s: number, e: O) => s + e.count, 0);
}

const balanced = (rowsUsed: number, k: number) => Array.from({ length: k }, () => (rowsUsed * (k + 1)) / 2);

describe('what Rust writes is opened (decision 657)', () => {
  it('opens both committed snapshots', () => {
    expect(kindOf(snapshot(W))).toBe(OK);
    expect(kindOf(snapshot(F))).toBe(OK);
  });

  it.each<[string, (b: O) => void]>([
    ['zero differences, ties', (b) => setW(b, [3, 12], [2, 9], 2, 1)],
    ['every difference zero', (b) => setW(b, [0, 0], [0, 0], 6, 0)],
    ['one sign only', (b) => setW(b, [0, 0], [6, 21])],
    ['50 pairs', (b) => setW(b, [40, 1000], [10, 275])],
    ['fewer than 5 pairs', (b) => setW(b, [2, 7], [2, 3])],
    ['a numeric z beside a p that overflowed', (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).p = nc('overflow');
    }],
    ['an overflowed median difference and correlation', (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).medianDifference = nc('overflow');
      wil(b).rankBiserial = nc('overflow');
    }],
  ])('a Wilcoxon result: %s', (_name, edit) => {
    expect(kindOf(changed(W, edit))).toBe(OK);
  });

  it.each<[string, (b: O) => void]>([
    ['every row tied', (b) => setF(b, 6, balanced(6, 3), nc('all_values_tied'))],
    ['a q of 0', (b) => setF(b, 6, balanced(6, 3), { value: 0 })],
    ['q overflowed', (b) => setF(b, 6, [10, 12, 14], nc('overflow'))],
    ['a p out of the distribution range', (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).p = nc('distribution_out_of_range');
    }],
    ['a p that overflowed', (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).p = nc('overflow');
    }],
    ['a W that overflowed', (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).kendallsW = nc('overflow');
    }],
    ['fewer than 5 rows', (b) => setF(b, 4, [6, 8, 10])],
  ])('a Friedman result: %s', (_name, edit) => {
    expect(kindOf(changed(F, edit))).toBe(OK);
  });

  it('takes the columns of a spec in any order', () => {
    expect(kindOf(changed(W, (b) => {
      wil(b).first = 'C2';
      wil(b).second = 'C1';
      b.primaryOutputs.columns.reverse();
      b.primaryOutputs.exclusions.reverse();
      b.manifest.exclusions.reverse();
    }, (p) => p.columns.reverse()))).toBe(OK);
  });
});

describe('the result (decision 657)', () => {
  it('refuses an extra key, and a missing one', () => {
    expect(kindOf(changed(W, (b) => (b.primaryOutputs.extra = 1)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => delete b.primaryOutputs.limitations))).toBe(BAD);
  });

  it('refuses a rowsUsed that is not whole', () => {
    expect(
      kindOf(
        changed(F, (b) => {
          b.primaryOutputs.rowsUsed = 6.5;
          setF(b, 6.5, [10.5, 13, 15.5]);
        })
      )
    ).toBe(BAD);
  });

  it('refuses a rowsUsed of 0', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [0, 0], [0, 0], 0);
      wil(b).exactP = 0.5;
      b.primaryOutputs.notes = [code('rows_below_five')];
    }))).toBe(BAD);
  });

  it('refuses the columns not one per spec column', () => {
    expect(kindOf(changed(W, (b) => b.primaryOutputs.columns.pop()))).toBe(BAD);
    expect(kindOf(changed(W, (b) => b.primaryOutputs.columns.push({ column: 'C9', median: 1 })))).toBe(BAD);
  });

  it('refuses a column with an extra key, another alias, or a median not a number', () => {
    expect(kindOf(changed(W, (b) => (b.primaryOutputs.columns[0].extra = 1)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (b.primaryOutputs.columns[0].column = 'C9')))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (b.primaryOutputs.columns[0].median = 'x')))).toBe(BAD);
  });

  it('refuses a Wilcoxon result of a three-column spec, and a Friedman result of a two-column one', () => {
    expect(kindOf(changed(W, (b) => b.primaryOutputs.columns.push({ column: 'C3', median: 1 }), (p) => p.columns.push('C3')))).toBe(BAD);
    expect(kindOf(changed(F, (b) => b.primaryOutputs.columns.pop(), (p) => p.columns.pop()))).toBe(BAD);
  });

  it('refuses limitations reordered or short', () => {
    expect(kindOf(changed(W, (b) => b.primaryOutputs.limitations.reverse()))).toBe(BAD);
    expect(kindOf(changed(F, (b) => b.primaryOutputs.limitations.pop()))).toBe(BAD);
  });

  it('refuses a spec whose parameters lack a field the checker reads, or name no column', () => {
    for (const k of ['columns', 'missingCodes']) {
      expect(kindOf(changed(W, () => undefined, (p) => delete p[k])), k).toBe(BAD);
    }
    expect(kindOf(changed(W, () => undefined, (p) => (p.missingCodes = 'none')))).toBe(BAD);
  });

  it('refuses a column of the spec that is no alias, the result naming it as the spec does', () => {
    expect(
      kindOf(
        changed(
          W,
          (b) => {
            b.primaryOutputs.columns[1].column = 'score';
            wil(b).second = 'score';
            b.primaryOutputs.exclusions = b.primaryOutputs.exclusions.filter((e: O) => e.params.column === 1);
            b.manifest.exclusions = b.primaryOutputs.exclusions;
            b.manifest.rowsRead = b.manifest.rowsUsed + b.primaryOutputs.exclusions.reduce((s: number, e: O) => s + e.count, 0);
          },
          (p) => (p.columns = ['C1', 'score'])
        )
      )
    ).toBe(BAD);
  });

  it('refuses a column chosen twice, which Rust refuses as column_repeated', () => {
    expect(
      kindOf(
        changed(
          W,
          (b) => {
            b.primaryOutputs.columns[1].column = 'C1';
            wil(b).second = 'C1';
            b.primaryOutputs.exclusions = [];
            b.manifest.exclusions = [];
            b.manifest.rowsRead = b.manifest.rowsUsed;
          },
          (p) => (p.columns = ['C1', 'C1'])
        )
      )
    ).toBe(BAD);
  });

  it('refuses one column and 51, the result consistent with each', () => {
    expect(kindOf(changed(F, (b) => {
      b.primaryOutputs.columns = b.primaryOutputs.columns.slice(0, 1);
      b.primaryOutputs.test = { wilcoxon: wil({ primaryOutputs: { test: { wilcoxon: {} } } }) };
    }, (p) => p.columns.splice(1)))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      const aliases = Array.from({ length: 51 }, (_, i) => `C${i + 1}`);
      b.primaryOutputs.columns = aliases.map((column) => ({ column, median: 1 }));
      const rows = b.primaryOutputs.rowsUsed;
      fri(b).columns = aliases.map((column) => ({ column, rankSum: (rows * 52) / 2, meanRank: 26 }));
      fri(b).df = 50;
      fri(b).q = nc('overflow');
      fri(b).p = nc('overflow');
      fri(b).kendallsW = nc('overflow');
    }, (p) => (p.columns = Array.from({ length: 51 }, (_, i) => `C${i + 1}`))))).toBe(BAD);
  });
});

describe('the exclusions and the manifest (decision 657)', () => {
  const ex = (c: string, column: number, count = 2) => ({ code: code(c), count, params: { column } });
  const on = (list: O[], file = W) =>
    changed(file, (b) => {
      b.primaryOutputs.exclusions = list;
      b.manifest.exclusions = list;
      b.manifest.rowsRead = b.manifest.rowsUsed + list.reduce((s: number, e: O) => s + e.count, 0);
    });

  it('accepts each exclusion as Rust writes it, in the spec column order, value before code', () => {
    expect(kindOf(on([ex('missing_value', 1), ex('missing_code', 1), ex('missing_value', 2), ex('missing_code', 2)]))).toBe(OK);
  });

  it('refuses an extra key, in an exclusion or in its params', () => {
    expect(kindOf(on([{ ...ex('missing_value', 1), extra: 1 }]))).toBe(BAD);
    expect(kindOf(on([{ code: code('missing_value'), count: 2, params: { column: 1, value: 3 } }]))).toBe(BAD);
  });

  it('refuses another code', () => {
    expect(kindOf(on([ex('no_weights', 1)]))).toBe(BAD);
  });

  it('refuses a count of 0, or one not whole', () => {
    expect(kindOf(on([ex('missing_value', 1, 0)]))).toBe(BAD);
    expect(kindOf(on([ex('missing_value', 1, 1.5)]))).toBe(BAD);
  });

  it('refuses a column the spec does not name', () => {
    expect(kindOf(on([ex('missing_value', 9)]))).toBe(BAD);
  });

  it('refuses another order, and a pair twice', () => {
    expect(kindOf(on([ex('missing_value', 2), ex('missing_value', 1)]))).toBe(BAD);
    expect(kindOf(on([ex('missing_code', 1), ex('missing_value', 1)]))).toBe(BAD);
    expect(kindOf(on([ex('missing_value', 1), ex('missing_value', 1)]))).toBe(BAD);
  });

  it('refuses missing_code when the spec has no missing codes', () => {
    const v = on([ex('missing_code', 1)]);
    const s = JSON.parse(v.specJson);
    s.parameters.missingCodes = [];
    expect(kindOf({ ...v, specJson: JSON.stringify(s) })).toBe(BAD);
  });

  it("refuses a manifest's rowsUsed, rowsRead or exclusions unlike the result's", () => {
    expect(kindOf(changed(W, (b) => (b.manifest.rowsUsed += 1)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (b.manifest.rowsRead += 1)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      b.manifest.exclusions = [];
      b.manifest.rowsRead = b.manifest.rowsUsed + b.primaryOutputs.exclusions.reduce((s: number, e: O) => s + e.count, 0);
    }))).toBe(BAD);
  });

  it('refuses a suspect-code warning without its column, value or count', () => {
    const full = { column: 1, value: -99, count: 3 };
    const warning = (params: O) => ({ code: code('suspect_missing_code'), params, severity: 'warning' });
    expect(kindOf(changed(W, (b) => (b.diagnostics = [warning(full)])))).toBe(OK);
    for (const k of ['column', 'value', 'count']) {
      const less: O = { ...full };
      delete less[k];
      expect(kindOf(changed(W, (b) => (b.diagnostics = [warning(less)]))), k).toBe(BAD);
    }
  });
});

describe('the Wilcoxon form (decision 657)', () => {
  it('refuses an extra key, a missing key, and an extra side key', () => {
    expect(kindOf(changed(W, (b) => (wil(b).extra = 1)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => delete wil(b).exactP))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (wil(b).positive.extra = 1)))).toBe(BAD);
  });

  it("refuses first and second that are not the spec's two, in order", () => {
    expect(kindOf(changed(W, (b) => ([wil(b).first, wil(b).second] = [wil(b).second, wil(b).first])))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (wil(b).first = 'C9')))).toBe(BAD);
  });

  it('refuses a count that is not whole', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).positive.count = 2.5;
      wil(b).negative.count = 3.5;
      wil(b).positive.meanRank = 9 / 2.5;
      wil(b).negative.meanRank = 12 / 3.5;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).tieRuns = 0.5;
    }))).toBe(BAD);
  });

  it('refuses an n other than the sides together', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [2, 3], 1);
      wil(b).n = 6;
      b.primaryOutputs.rowsUsed = 7;
      b.manifest.rowsUsed = 7;
      b.manifest.rowsRead = 7 + b.primaryOutputs.exclusions.reduce((s: number, e: O) => s + e.count, 0);
      wil(b).positive = { count: 2, rankSum: 11, meanRank: 5.5 };
      wil(b).v = 11;
      wil(b).tMinus = 10;
      wil(b).negative = { count: 3, rankSum: 10, meanRank: 10 / 3 };
      // Decision 668: Rust's z for the edited V and n, so only the n rule refuses it.
      wil(b).z = { value: wilcoxonZ(11, 6) };
      wil(b).rankBiserial = { value: (11 - 10) / 21 };
    }))).toBe(BAD);
  });

  it('refuses a rowsUsed other than n and the zero differences', () => {
    expect(kindOf(changed(W, (b) => {
      b.primaryOutputs.rowsUsed += 1;
      b.manifest.rowsUsed += 1;
      b.manifest.rowsRead += 1;
    }))).toBe(BAD);
  });

  it('refuses more tie runs than n / 2', () => {
    expect(kindOf(changed(W, (b) => setW(b, [4, 15], [2, 6], 0, 4)))).toBe(BAD);
  });

  it('refuses a rank sum that is not a multiple of 0.5', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).v = 9.25;
      wil(b).positive = { count: 3, rankSum: 9.25, meanRank: 9.25 / 3 };
      wil(b).tMinus = 21 - 9.25;
      wil(b).negative = { count: 3, rankSum: 11.75, meanRank: 11.75 / 3 };
      wil(b).rankBiserial = { value: (9.25 - 11.75) / 21 };
    }))).toBe(BAD);
  });

  it('refuses a rank sum outside what its count can reach', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).v = 3;
      wil(b).positive = { count: 3, rankSum: 3, meanRank: 1 };
      wil(b).tMinus = 18;
      wil(b).negative = { count: 3, rankSum: 18, meanRank: 6 };
      wil(b).rankBiserial = { value: (3 - 18) / 21 };
      wil(b).z = { value: wilcoxonZ(3, 6) };
    }))).toBe(BAD);
  });

  it('refuses a half rank sum where no two sizes are tied', () => {
    expect(kindOf(changed(W, (b) => setW(b, [3, 11.5], [3, 9.5])))).toBe(BAD);
  });

  it('refuses a mean rank null when a side has pairs, a number when it has none, and one other than the sum over the count', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).positive.meanRank = null;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [0, 0], [6, 21]);
      wil(b).negative.meanRank = 0;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).positive.meanRank = 3.5;
    }))).toBe(BAD);
  });

  it('refuses a V other than the positive rank sum', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).v = 10;
      wil(b).tMinus = 11;
      wil(b).negative = { count: 3, rankSum: 11, meanRank: 11 / 3 };
      wil(b).rankBiserial = { value: (10 - 11) / 21 };
      wil(b).z = { value: wilcoxonZ(10, 6) };
    }))).toBe(BAD);
  });

  it('refuses a T- other than n (n + 1) / 2 less V', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).tMinus = 13;
      wil(b).negative = { count: 3, rankSum: 13, meanRank: 13 / 3 };
      wil(b).rankBiserial = { value: (9 - 13) / 21 };
    }))).toBe(BAD);
  });

  it('refuses a negative rank sum other than T-', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).negative = { count: 3, rankSum: 13, meanRank: 13 / 3 };
    }))).toBe(BAD);
  });

  it('refuses all_differences_zero with pairs, and not all_differences_zero with none', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).z = nc('all_differences_zero');
      wil(b).p = nc('all_differences_zero');
      wil(b).rankBiserial = nc('all_differences_zero');
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [0, 0], [0, 0], 6);
      wil(b).z = { value: 0 };
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [0, 0], [0, 0], 6);
      wil(b).rankBiserial = { value: 0 };
    }))).toBe(BAD);
  });

  it('refuses a rank-biserial correlation other than its formula', () => {
    expect(kindOf(changed(W, (b) => (wil(b).rankBiserial = { value: 0.125 })))).toBe(BAD);
  });

  it('refuses a code Rust cannot write for z, p or the correlation', () => {
    const on = (f: (t: O) => void) => changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      f(wil(b));
    });
    expect(kindOf(on((t) => (t.z = nc('all_values_tied'))))).toBe(BAD);
    expect(kindOf(on((t) => (t.p = nc('distribution_out_of_range'))))).toBe(BAD);
    expect(kindOf(on((t) => (t.rankBiserial = nc('all_values_tied'))))).toBe(BAD);
  });

  it('refuses a normal p outside 0 to 1', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).p = { value: 5 };
    }))).toBe(BAD);
  });

  // Decision 668: with a tie run, so the sign rule is the only one that can refuse these
  // (with none, z must also be Rust's value, a rule of its own below).
  it('refuses a z whose sign is not that of V less T-', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).z = { value: 0.5 };
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).z = { value: 0 };
    }))).toBe(BAD);
  });

  it('refuses a z not computable beside a p of another kind', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).z = nc('overflow');
    }))).toBe(BAD);
  });

  it('refuses a median difference of a code Rust cannot write', () => {
    expect(kindOf(changed(W, (b) => (wil(b).medianDifference = nc('all_values_tied'))))).toBe(BAD);
  });

  it('refuses an exact p absent with no note, present with a note, or outside (0, 1]', () => {
    expect(kindOf(changed(W, (b) => (wil(b).exactP = null)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).exactP = 0.5;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (wil(b).exactP = 0)))).toBe(BAD);
    expect(kindOf(changed(W, (b) => (wil(b).exactP = 1.5)))).toBe(BAD);
  });

  const noteOff = (c: string) => (b: O) => (b.primaryOutputs.notes = b.primaryOutputs.notes.filter((x: string) => x !== code(c)));

  it('refuses tie runs with no ties note, and a ties note with none', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      noteOff('exact_not_given_ties')(b);
      wil(b).exactP = 0.5;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9], 0, 1);
      wil(b).tieRuns = 0;
    }))).toBe(BAD);
  });

  it('refuses zero differences with no zeros note, or with no dropped note', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [2, 3], 1);
      noteOff('exact_not_given_zeros')(b);
      wil(b).exactP = 0.5;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [2, 3], 1);
      noteOff('zero_differences_dropped')(b);
    }))).toBe(BAD);
  });

  it('refuses 50 pairs with no size note, and a size note below 50', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [40, 1000], [10, 275]);
      noteOff('exact_not_given_size')(b);
      wil(b).exactP = 0.5;
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      b.primaryOutputs.notes = [code('exact_not_given_size')];
      wil(b).exactP = null;
    }))).toBe(BAD);
  });

  it('refuses fewer than 5 pairs with no note, and the note with 5 or more', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [2, 7], [2, 3]);
      noteOff('rows_below_five')(b);
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      b.primaryOutputs.notes = [code('rows_below_five')];
    }))).toBe(BAD);
  });

  it('refuses the notes out of their order, and another note', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [2, 3], 1);
      b.primaryOutputs.notes.reverse();
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      b.primaryOutputs.notes = [code('no_weights')];
    }))).toBe(BAD);
  });
});

describe('the Friedman form (decision 657)', () => {
  it('refuses an extra key and a missing one', () => {
    expect(kindOf(changed(F, (b) => (fri(b).extra = 1)))).toBe(BAD);
    expect(kindOf(changed(F, (b) => delete fri(b).p))).toBe(BAD);
  });

  it("refuses its columns not the spec's, in order, each of its three keys", () => {
    expect(kindOf(changed(F, (b) => fri(b).columns.reverse()))).toBe(BAD);
    expect(kindOf(changed(F, (b) => (fri(b).columns[0].column = 'C9')))).toBe(BAD);
    expect(kindOf(changed(F, (b) => (fri(b).columns[0].extra = 1)))).toBe(BAD);
    expect(kindOf(changed(F, (b) => fri(b).columns.pop()))).toBe(BAD);
  });

  it('refuses a mean rank other than the rank sum over the rows', () => {
    expect(kindOf(changed(F, (b) => (fri(b).columns[0].meanRank += 1)))).toBe(BAD);
  });

  it('refuses rank sums that do not add up to rows k (k + 1) / 2', () => {
    expect(kindOf(changed(F, (b) => setF(b, 6, [12, 12, 13])))).toBe(BAD);
  });

  it('refuses a rank sum outside rows to rows k', () => {
    expect(kindOf(changed(F, (b) => setF(b, 6, [5, 12, 19])))).toBe(BAD);
    expect(kindOf(changed(F, (b) => setF(b, 6, [19, 12, 5])))).toBe(BAD);
  });

  it('refuses a df other than k less one', () => {
    expect(kindOf(changed(F, (b) => (fri(b).df = 5)))).toBe(BAD);
  });

  it('refuses a negative q and a code Rust cannot write for it', () => {
    expect(kindOf(changed(F, (b) => setF(b, 6, [10, 12, 14], { value: -1 })))).toBe(BAD);
    expect(kindOf(changed(F, (b) => setF(b, 6, [10, 12, 14], nc('distribution_out_of_range'))))).toBe(BAD);
  });

  it('refuses all_values_tied on some of q, p and W and not the others', () => {
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).p = nc('all_values_tied');
    }))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).kendallsW = nc('all_values_tied');
    }))).toBe(BAD);
  });

  it('refuses all_values_tied with rank sums that are not all equal', () => {
    expect(kindOf(changed(F, (b) => setF(b, 6, [10, 12, 14], nc('all_values_tied'))))).toBe(BAD);
  });

  it('refuses a q not computable with a p or a W of another code', () => {
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14], nc('overflow'));
      fri(b).p = { value: 0.5 };
    }))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14], nc('overflow'));
      fri(b).kendallsW = { value: 0.5 };
    }))).toBe(BAD);
  });

  it('refuses a q of 0 with rank sums that differ, and a q above 0 with rank sums all equal', () => {
    expect(kindOf(changed(F, (b) => setF(b, 6, [10, 12, 14], { value: 0 })))).toBe(BAD);
    expect(kindOf(changed(F, (b) => setF(b, 6, balanced(6, 3), { value: 2 })))).toBe(BAD);
  });

  it('refuses a p outside 0 to 1, and a code Rust cannot write for it', () => {
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).p = { value: 1.5 };
    }))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).p = nc('no_weights');
    }))).toBe(BAD);
  });

  it("refuses a W other than q / (rows (k - 1)), and a code Rust cannot write for it", () => {
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).kendallsW = { value: 0.75 };
    }))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      fri(b).kendallsW = nc('distribution_out_of_range');
    }))).toBe(BAD);
  });

  it('refuses a below-five note with five rows, none with four, and any other note', () => {
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      b.primaryOutputs.notes = [code('rows_below_five')];
    }))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      setF(b, 4, [6, 8, 10]);
      b.primaryOutputs.notes = [];
    }))).toBe(BAD);
    expect(kindOf(changed(F, (b) => {
      setF(b, 6, [10, 12, 14]);
      b.primaryOutputs.notes = [code('exact_not_given_ties')];
    }))).toBe(BAD);
  });
});

/** Decision 668: what stats_related_ranks never writes, found by decision 667's review. Each
 *  case its own rule, each rule's red a mutation (decision 502). */
describe('what stats_related_ranks never writes (decision 668)', () => {
  const zero = (b: O) => setW(b, [0, 0], [0, 0], 6, 0);

  it('takes every difference zero as Rust writes it', () => {
    expect(kindOf(changed(W, zero))).toBe(OK);
  });

  it('refuses an extra value beside all_differences_zero on z, p or the correlation', () => {
    for (const k of ['z', 'p', 'rankBiserial']) {
      expect(kindOf(changed(W, (b) => {
        zero(b);
        wil(b)[k] = { ...nc('all_differences_zero'), value: 1 };
      })), k).toBe(BAD);
    }
  });

  it('refuses an extra value that is not a number beside all_differences_zero', () => {
    expect(kindOf(changed(W, (b) => {
      zero(b);
      wil(b).z = { ...nc('all_differences_zero'), value: 'x' };
    }))).toBe(BAD);
  });

  it('refuses a median difference other than 0 when every difference is zero', () => {
    expect(kindOf(changed(W, (b) => {
      zero(b);
      wil(b).medianDifference = { value: 7 };
    }))).toBe(BAD);
    expect(kindOf(changed(W, (b) => {
      zero(b);
      wil(b).medianDifference = nc('overflow');
    }))).toBe(BAD);
  });

  it('takes a median difference of -0 when every difference is zero', () => {
    expect(kindOf(changed(W, (b) => {
      zero(b);
      wil(b).medianDifference = { value: -0 };
    }))).toBe(OK);
  });

  it("refuses, with no tie runs, a z other than Rust's from V and n, of the right sign", () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).z = { value: wilcoxonZ(9, 6) * 2 };
    }))).toBe(BAD);
  });

  it('refuses, with no tie runs, z and p that overflowed, which Rust cannot write there', () => {
    expect(kindOf(changed(W, (b) => {
      setW(b, [3, 12], [3, 9]);
      wil(b).z = nc('overflow');
      wil(b).p = nc('overflow');
    }))).toBe(BAD);
  });
});
