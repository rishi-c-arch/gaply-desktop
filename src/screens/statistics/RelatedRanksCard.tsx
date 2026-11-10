// Gaply — the related rank tests' result card (Research OS, Phase 8Z-2, decisions 653 and
// 654): a file of its own, since the method has no page (ADR-012:233-241). The Output's
// entry and a saved result both draw it.
//
// The columns' table, then the test the count of columns chose: for two the ranks of the
// differences, the Wilcoxon table and the sentence naming the first and second columns;
// for three or more the Friedman table and its sentence. A not-computable value shows its
// sentence where its number would be; an exact p that is absent says so in its cell, and
// the notes say why. Every number goes through shownNumber. The tone is neutral (decision 66).
//
// A column's name is the user's own text, shown as text only: never in a key, an
// attribute, a title or a test id, which take the column's alias.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import type { Friedman, RelatedRanksResultView, RelatedRanksStat, SignRanks, Wilcoxon } from './relatedRanksTypes';
import {
  FRIEDMAN_ROW_LABELS,
  RELATED_COLUMN_HEADS,
  RELATED_EXACT_NOT_GIVEN_CELL,
  RELATED_FRIEDMAN_HEADS,
  RELATED_NO_PAIRS,
  RELATED_RANK_HEADS,
  RELATED_RANK_ROWS,
  RELATED_RANKS_DIGITS_NOTE,
  RELATED_RANKS_HEADINGS,
  RELATED_RANKS_ROWS_USED_NOTE,
  WILCOXON_ROW_LABELS,
  relatedRanksFriedmanSentence,
  relatedRanksSentence,
  relatedRanksStatText,
  relatedRanksWilcoxonSentence,
  type RelatedRanksNames,
} from './relatedRanksWording';
import { shownNumber } from './ttestWording';

const n = shownNumber;
const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

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
const StatRow: React.FC<{ label: string; stat: RelatedRanksStat; testId: string }> = ({ label, stat, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    <td className={'value' in stat ? 'gds-mono' : undefined} style={CELL_PADDING}>
      {relatedRanksStatText(stat)}
    </td>
  </tr>
);

/** One sign's row: its N, its mean rank or that it has no pair, its sum of ranks. */
const SignRow: React.FC<{ label: string; side: SignRanks; testId: string }> = ({ label, side, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    <td className="gds-mono" style={CELL_PADDING}>{n(side.count)}</td>
    {side.meanRank === null ? <td style={CELL_PADDING}>{RELATED_NO_PAIRS}</td> : <td className="gds-mono" style={CELL_PADDING}>{n(side.meanRank)}</td>}
    <td className="gds-mono" style={CELL_PADDING}>{n(side.rankSum)}</td>
  </tr>
);

const RanksOfDifferences: React.FC<{ test: Wilcoxon }> = ({ test }) => (
  <section aria-label={RELATED_RANKS_HEADINGS.ranks}>
    <h4 style={{ margin: '8px 0 4px' }}>{RELATED_RANKS_HEADINGS.ranks}</h4>
    <table data-testid="rr-ranks" style={{ borderCollapse: 'collapse' }}>
      <thead>
        <tr>
          <th scope="col" style={head} />
          {RELATED_RANK_HEADS.map((c) => (
            <th key={c} scope="col" style={head}>
              {c}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        <SignRow label={RELATED_RANK_ROWS.negative} side={test.negative} testId="rr-negative" />
        <SignRow label={RELATED_RANK_ROWS.positive} side={test.positive} testId="rr-positive" />
        <tr data-testid="rr-zero">
          <th scope="row" style={head}>
            {RELATED_RANK_ROWS.zero}
          </th>
          <td className="gds-mono" style={CELL_PADDING}>{n(test.zeroDifferences)}</td>
          <td style={CELL_PADDING} />
          <td style={CELL_PADDING} />
        </tr>
      </tbody>
    </table>
  </section>
);

const WilcoxonTable: React.FC<{ test: Wilcoxon; names: RelatedRanksNames }> = ({ test, names }) => (
  <section aria-label={RELATED_RANKS_HEADINGS.wilcoxon} data-testid="rr-wilcoxon">
    <h4 style={{ margin: '8px 0 4px' }}>{RELATED_RANKS_HEADINGS.wilcoxon}</h4>
    <table style={{ borderCollapse: 'collapse' }}>
      <tbody>
        <NumberRow label={WILCOXON_ROW_LABELS.v} value={test.v} testId="rr-v" />
        <StatRow label={WILCOXON_ROW_LABELS.z} stat={test.z} testId="rr-z" />
        <StatRow label={WILCOXON_ROW_LABELS.p} stat={test.p} testId="rr-p" />
        {test.exactP !== null ? (
          <NumberRow label={WILCOXON_ROW_LABELS.exactP} value={test.exactP} testId="rr-exact-p" />
        ) : (
          <tr data-testid="rr-exact-p">
            <th scope="row" style={head}>
              {WILCOXON_ROW_LABELS.exactP}
            </th>
            <td style={CELL_PADDING} data-testid="rr-exact-p-not-given">
              {RELATED_EXACT_NOT_GIVEN_CELL}
            </td>
          </tr>
        )}
        <StatRow label={WILCOXON_ROW_LABELS.rankBiserial} stat={test.rankBiserial} testId="rr-rank-biserial" />
        <StatRow label={WILCOXON_ROW_LABELS.medianDifference} stat={test.medianDifference} testId="rr-median-difference" />
      </tbody>
    </table>
    <p style={{ margin: '4px 0 0' }} data-testid="rr-first-second">
      {relatedRanksWilcoxonSentence(names.column(aliasNumber(test.first)), names.column(aliasNumber(test.second)))}
    </p>
  </section>
);

const FriedmanTable: React.FC<{ test: Friedman }> = ({ test }) => (
  <section aria-label={RELATED_RANKS_HEADINGS.friedman} data-testid="rr-friedman">
    <h4 style={{ margin: '8px 0 4px' }}>{RELATED_RANKS_HEADINGS.friedman}</h4>
    <table style={{ borderCollapse: 'collapse' }}>
      <tbody>
        <StatRow label={FRIEDMAN_ROW_LABELS.q} stat={test.q} testId="rr-q" />
        <NumberRow label={FRIEDMAN_ROW_LABELS.df} value={test.df} testId="rr-df" />
        <StatRow label={FRIEDMAN_ROW_LABELS.p} stat={test.p} testId="rr-friedman-p" />
        <StatRow label={FRIEDMAN_ROW_LABELS.kendallsW} stat={test.kendallsW} testId="rr-kendalls-w" />
      </tbody>
    </table>
    <p style={{ margin: '4px 0 0' }} data-testid="rr-friedman-sentence">
      {relatedRanksFriedmanSentence(test.columns.length)}
    </p>
  </section>
);

/** The neutral result card (decision 66): the columns, the test, what is not computable in
 *  its own words, what is not assessed, and the manifest. */
export const RelatedRanksCard: React.FC<{ result: RelatedRanksResultView; names: RelatedRanksNames }> = ({ result, names }) => {
  const r = result.result;
  const friedman = 'friedman' in r.test ? r.test.friedman : null;
  const rankOf = (alias: string) => friedman?.columns.find((c) => c.column === alias);
  return (
    <div data-testid="rr-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => relatedRanksSentence(d, names)} testId="rr-result-notes" />
      <section aria-label={RELATED_RANKS_HEADINGS.columns}>
        <h4 style={{ margin: '8px 0 4px' }}>{RELATED_RANKS_HEADINGS.columns}</h4>
        <table data-testid="rr-columns-table" style={{ borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              {[...RELATED_COLUMN_HEADS, ...(friedman ? RELATED_FRIEDMAN_HEADS : [])].map((c) => (
                <th key={c} scope="col" style={head}>
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {r.columns.map((c) => (
              <tr key={c.column} data-testid={`rr-col-${c.column}`}>
                <th scope="row" style={head}>
                  {names.column(aliasNumber(c.column))}
                </th>
                <td className="gds-mono" style={CELL_PADDING}>{n(c.median)}</td>
                {friedman && <td className="gds-mono" style={CELL_PADDING}>{n(rankOf(c.column)?.meanRank ?? Number.NaN)}</td>}
                {friedman && <td className="gds-mono" style={CELL_PADDING}>{n(rankOf(c.column)?.rankSum ?? Number.NaN)}</td>}
              </tr>
            ))}
          </tbody>
        </table>
      </section>
      {'wilcoxon' in r.test && <RanksOfDifferences test={r.test.wilcoxon} />}
      {'wilcoxon' in r.test ? <WilcoxonTable test={r.test.wilcoxon} names={names} /> : <FriedmanTable test={r.test.friedman} />}
      {r.notes.length > 0 && (
        <ul data-testid="rr-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{relatedRanksSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="rr-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e) => (
              <li key={`${e.code}-${e.params.column}`}>{relatedRanksSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="rr-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{relatedRanksSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="rr-digits-note">
        {RELATED_RANKS_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="rr" manifest={result.manifest} rowsUsedNote={RELATED_RANKS_ROWS_USED_NOTE} />
    </div>
  );
};
