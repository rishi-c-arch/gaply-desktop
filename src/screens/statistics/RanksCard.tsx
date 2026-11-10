// Gaply — the rank tests' result card (Research OS, Phase 8Y-2, decisions 619 and 620):
// a file of its own, since the method has no page (ADR-012:233-241). The Output's entry
// and a saved result both draw it.
//
// The groups' table, then the test the count of groups chose: for two the Mann-Whitney
// table and decision 620's sentence naming the first and second groups, for three or
// more the Kruskal-Wallis table. A not-computable value shows its sentence where its
// number would be; an exact p that is absent shows the sentence of the note that says
// why. Every number goes through shownNumber. The tone is neutral (decision 66).
//
// A level's label is the user's own text, shown as text only: never in a key, an
// attribute, a title or a test id, which take the level's index.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import type { KruskalWallis, MannWhitney, RankGroup, RanksCode, RanksResultView, RanksStat } from './ranksTypes';
import {
  KW_ROW_LABELS,
  MW_ROW_LABELS,
  RANKS_DIGITS_NOTE,
  RANKS_GROUP_COLUMNS,
  RANKS_HEADINGS,
  RANKS_ROWS_USED_NOTE,
  ranksFirstSecondSentence,
  ranksSentence,
  ranksStatText,
  type RanksNames,
} from './ranksWording';
import { shownNumber } from './ttestWording';

const n = shownNumber;
const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };

/** A row with one number. */
const NumberRow: React.FC<{ label: string; value: number; testId: string }> = ({ label, value, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    <td className="gds-mono" style={CELL_PADDING}>
      {n(value)}
    </td>
  </tr>
);

/** A row with a number, or its sentence where the number would be. */
const StatRow: React.FC<{ label: string; stat: RanksStat; testId: string }> = ({ label, stat, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    <td className={'value' in stat ? 'gds-mono' : undefined} style={CELL_PADDING}>
      {ranksStatText(stat)}
    </td>
  </tr>
);

const EXACT_NOTES: readonly RanksCode[] = ['stats.ranks.exact_not_given_ties', 'stats.ranks.exact_not_given_size'];

const MannWhitneyTable: React.FC<{ test: MannWhitney; groups: RankGroup[]; notes: RanksCode[] }> = ({ test, groups, notes }) => {
  const label = (level: number) => groups.find((g) => g.level === level)?.label ?? '';
  return (
    <section aria-label={RANKS_HEADINGS.mannWhitney} data-testid="rk-mw">
      <h4 style={{ margin: '8px 0 4px' }}>{RANKS_HEADINGS.mannWhitney}</h4>
      <table style={{ borderCollapse: 'collapse' }}>
        <tbody>
          <NumberRow label={MW_ROW_LABELS.u1} value={test.u1} testId="rk-mw-u1" />
          <NumberRow label={MW_ROW_LABELS.u2} value={test.u2} testId="rk-mw-u2" />
          <StatRow label={MW_ROW_LABELS.z} stat={test.z} testId="rk-mw-z" />
          <StatRow label={MW_ROW_LABELS.p} stat={test.p} testId="rk-mw-p" />
          {test.exactP !== null ? (
            <NumberRow label={MW_ROW_LABELS.exactP} value={test.exactP} testId="rk-mw-exact-p" />
          ) : (
            <tr data-testid="rk-mw-exact-p">
              <th scope="row" style={head}>
                {MW_ROW_LABELS.exactP}
              </th>
              <td style={CELL_PADDING} data-testid="rk-mw-exact-p-not-given">
                {notes
                  .filter((code) => EXACT_NOTES.includes(code))
                  .map((code) => ranksSentence({ code }))
                  .join(' ')}
              </td>
            </tr>
          )}
          <NumberRow label={MW_ROW_LABELS.rankBiserial} value={test.rankBiserial} testId="rk-mw-rank-biserial" />
        </tbody>
      </table>
      <p style={{ margin: '4px 0 0' }} data-testid="rk-mw-first-second">
        {ranksFirstSecondSentence(label(test.first), label(test.second))}
      </p>
    </section>
  );
};

const KruskalWallisTable: React.FC<{ test: KruskalWallis }> = ({ test }) => (
  <section aria-label={RANKS_HEADINGS.kruskalWallis} data-testid="rk-kw">
    <h4 style={{ margin: '8px 0 4px' }}>{RANKS_HEADINGS.kruskalWallis}</h4>
    <table style={{ borderCollapse: 'collapse' }}>
      <tbody>
        <StatRow label={KW_ROW_LABELS.h} stat={test.h} testId="rk-kw-h" />
        <NumberRow label={KW_ROW_LABELS.df} value={test.df} testId="rk-kw-df" />
        <StatRow label={KW_ROW_LABELS.p} stat={test.p} testId="rk-kw-p" />
        <StatRow label={KW_ROW_LABELS.epsilonSquared} stat={test.epsilonSquared} testId="rk-kw-epsilon-squared" />
      </tbody>
    </table>
  </section>
);

/** The neutral result card (decision 66): the groups, the test, what is not computable
 *  in its own words, what is not assessed, and the manifest. */
export const RanksCard: React.FC<{ result: RanksResultView; names: RanksNames }> = ({ result, names }) => {
  const r = result.result;
  return (
    <div data-testid="rk-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => ranksSentence(d, names)} testId="rk-result-notes" />
      <section aria-label={RANKS_HEADINGS.ranks}>
        <h4 style={{ margin: '8px 0 4px' }}>{RANKS_HEADINGS.ranks}</h4>
        <table data-testid="rk-groups" style={{ borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              {RANKS_GROUP_COLUMNS.map((c) => (
                <th key={c} scope="col" style={head}>
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {r.groups.map((g) => (
              <tr key={g.level} data-testid={`rk-group-${g.level}`}>
                <th scope="row" style={head} data-testid={`rk-group-label-${g.level}`}>
                  {g.label}
                </th>
                <td className="gds-mono" style={CELL_PADDING}>{n(g.n)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(g.median)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(g.meanRank)}</td>
                <td className="gds-mono" style={CELL_PADDING}>{n(g.rankSum)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
      {'mann_whitney' in r.test ? <MannWhitneyTable test={r.test.mann_whitney} groups={r.groups} notes={r.notes} /> : <KruskalWallisTable test={r.test.kruskal_wallis} />}
      {r.notes.length > 0 && (
        <ul data-testid="rk-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{ranksSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="rk-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e, i) => (
              <li key={`${e.code}-${i}`}>{ranksSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="rk-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{ranksSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="rk-digits-note">
        {RANKS_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="rk" manifest={result.manifest} rowsUsedNote={RANKS_ROWS_USED_NOTE} />
    </div>
  );
};
