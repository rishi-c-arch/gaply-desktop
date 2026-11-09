// Gaply — crosstabs' result card (Research OS, Phase 8X-2, decisions 588 and 589): a
// file of its own, since crosstabs has no page (decision 591; ADR-012:233-241). The
// Output's entry and a saved result both draw it.
//
// The table is rows by columns, with the row totals, the column totals and n. Each cell
// is found by its (row, column), never by its position in the list: a (row, column)
// with no frequency, or a level with no total, draws an empty cell, and nothing
// undefined reaches shownNumber (ttestWording.ts:249-250 calls toPrecision). One cell
// statistic shows at a time, chosen by aria-pressed buttons as the ANOVA's post hoc
// switch is (AnovaFlow.tsx:510-526), since a 50 x 50 table of eight numbers a cell is
// too much to draw and read (decision 593). The continuity correction and Fisher's
// test show only when the result has them, for a 2 x 2 table. A not-computable value
// shows its sentence where its number would be. The tone is neutral (decision 66).
//
// A level's label is the user's own text, shown as text only: never in a key, an
// attribute, a title or a test id, which take the level's index.
import React, { useMemo, useState } from 'react';
import { Button } from '../../design-system';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import { shownNumber } from './ttestWording';
import type { ChiOutcome, Frequency, XtabResultView } from './xtabTypes';
import {
  CELL_STATISTICS,
  XTAB_DIGITS_NOTE,
  XTAB_MEASURE_LABELS,
  XTAB_ROWS_USED_NOTE,
  XTAB_SWITCH_LABEL,
  XTAB_TEST_COLUMNS,
  XTAB_TEST_LABELS,
  XTAB_TESTS_HEADING,
  XTAB_TOTAL,
  xtabCellsBelowFive,
  xtabSentence,
  xtabStatText,
  xtabTableCaption,
  type CellStatistic,
  type XtabNames,
} from './xtabWording';

const n = shownNumber;
/** A number, or an empty cell where there is none. */
const maybe = (v: number | undefined) => (typeof v === 'number' ? n(v) : '');

/** The number a cell shows for the chosen statistic; the percentages are the
 *  proportions times 100. */
const CELL_VALUE: Record<CellStatistic, (f: Frequency) => number> = {
  count: (f) => f.observed,
  expected: (f) => f.expected,
  row_percent: (f) => f.rowProportion * 100,
  column_percent: (f) => f.columnProportion * 100,
  total_percent: (f) => f.totalProportion * 100,
  residual: (f) => f.residual,
  pearson_residual: (f) => f.pearsonResidual,
  adjusted_residual: (f) => f.adjustedResidual,
};

const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };

/** A chi-square test's line: its value, df and p, or its sentence where they
 *  would be. */
const ChiRow: React.FC<{ label: string; test: ChiOutcome; testId: string }> = ({ label, test, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    {'computed' in test ? (
      <>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.statistic)}</td>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.df)}</td>
        <td className="gds-mono" style={CELL_PADDING}>{n(test.computed.p)}</td>
      </>
    ) : (
      <td colSpan={3} style={CELL_PADDING} data-testid={`${testId}-not-computable`}>
        {xtabSentence({ code: test.not_computable })}
      </td>
    )}
  </tr>
);

/** One of Fisher's p values: no statistic and no df, as SPSS's table has none. */
const FisherRow: React.FC<{ label: string; p: number; testId: string }> = ({ label, p, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    <td style={CELL_PADDING} />
    <td style={CELL_PADDING} />
    <td className="gds-mono" style={CELL_PADDING}>{n(p)}</td>
  </tr>
);

/** The neutral result card (decision 66): the table, the tests, the measures, what
 *  is not computable in its own words, what is not assessed, and the manifest. */
export const XtabCard: React.FC<{ result: XtabResultView; names: XtabNames }> = ({ result, names }) => {
  const r = result.result;
  const [shown, setShown] = useState<CellStatistic>('count');
  const at = useMemo(() => new Map(r.frequencies.map((f) => [`${f.row} ${f.column}`, f])), [r.frequencies]);
  const value = CELL_VALUE[shown];
  const shownLabel = CELL_STATISTICS.find((c) => c.stat === shown)?.label ?? '';
  return (
    <div data-testid="xt-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => xtabSentence(d, names)} testId="xt-result-notes" />
      <div role="group" aria-label={XTAB_SWITCH_LABEL} style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        {CELL_STATISTICS.map((c) => (
          <Button
            key={c.stat}
            variant={c.stat === shown ? 'primary' : 'secondary'}
            aria-pressed={c.stat === shown}
            onClick={() => setShown(c.stat)}
            data-testid={`xt-show-${c.stat}`}
          >
            {c.label}
          </Button>
        ))}
      </div>
      <table data-testid="xt-table" style={{ borderCollapse: 'collapse' }}>
        <caption style={{ textAlign: 'left' }}>{xtabTableCaption(shownLabel)}</caption>
        <thead>
          <tr>
            <td />
            {r.columnLevels.map((c) => (
              <th key={c.level} scope="col" style={head} data-testid={`xt-column-label-${c.level}`}>
                {c.label}
              </th>
            ))}
            <th scope="col" style={head}>
              {XTAB_TOTAL}
            </th>
          </tr>
        </thead>
        <tbody>
          {r.rowLevels.map((row) => (
            <tr key={row.level}>
              <th scope="row" style={head} data-testid={`xt-row-label-${row.level}`}>
                {row.label}
              </th>
              {r.columnLevels.map((col) => {
                const f = at.get(`${row.level} ${col.level}`);
                return (
                  <td key={col.level} className="gds-mono" style={CELL_PADDING} data-testid={`xt-cell-${row.level}-${col.level}`}>
                    {f ? n(value(f)) : ''}
                  </td>
                );
              })}
              <td className="gds-mono" style={CELL_PADDING} data-testid={`xt-row-total-${row.level}`}>
                {maybe(r.rowTotals[row.level])}
              </td>
            </tr>
          ))}
          <tr>
            <th scope="row" style={head}>
              {XTAB_TOTAL}
            </th>
            {r.columnLevels.map((col) => (
              <td key={col.level} className="gds-mono" style={CELL_PADDING} data-testid={`xt-column-total-${col.level}`}>
                {maybe(r.columnTotals[col.level])}
              </td>
            ))}
            <td className="gds-mono" style={CELL_PADDING} data-testid="xt-n">
              {n(r.n)}
            </td>
          </tr>
        </tbody>
      </table>
      <p style={{ margin: 0 }} data-testid="xt-cells-below-five">
        {xtabCellsBelowFive(r.expectedBelowFive, r.cellCount, r.smallestExpected)}
      </p>
      <section aria-label={XTAB_TESTS_HEADING} data-testid="xt-tests">
        <h4 style={{ margin: '8px 0 4px' }}>{XTAB_TESTS_HEADING}</h4>
        <table style={{ borderCollapse: 'collapse' }}>
          <thead>
            <tr>
              {XTAB_TEST_COLUMNS.map((c) => (
                <th key={c} scope="col" style={head}>
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            <ChiRow label={XTAB_TEST_LABELS.pearson} test={r.pearson} testId="xt-test-pearson" />
            <ChiRow label={XTAB_TEST_LABELS.likelihood} test={r.likelihoodRatio} testId="xt-test-likelihood" />
            {r.continuityCorrected && <ChiRow label={XTAB_TEST_LABELS.continuity} test={r.continuityCorrected} testId="xt-test-continuity" />}
            {r.fisher && (
              <>
                <FisherRow label={XTAB_TEST_LABELS.fisherTwoSided} p={r.fisher.twoSided} testId="xt-test-fisher-two-sided" />
                <FisherRow label={XTAB_TEST_LABELS.fisherLess} p={r.fisher.less} testId="xt-test-fisher-less" />
                <FisherRow label={XTAB_TEST_LABELS.fisherGreater} p={r.fisher.greater} testId="xt-test-fisher-greater" />
              </>
            )}
          </tbody>
        </table>
      </section>
      <dl data-testid="xt-measures" style={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: '2px 12px', margin: 0 }}>
        <dt>{XTAB_MEASURE_LABELS.phi}</dt>
        <dd data-testid="xt-phi">{xtabStatText(r.phi)}</dd>
        <dt>{XTAB_MEASURE_LABELS.cramersV}</dt>
        <dd data-testid="xt-cramers-v">{xtabStatText(r.cramersV)}</dd>
        <dt>{XTAB_MEASURE_LABELS.contingency}</dt>
        <dd data-testid="xt-contingency">{xtabStatText(r.contingencyCoefficient)}</dd>
      </dl>
      {r.notes.length > 0 && (
        <ul data-testid="xt-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{xtabSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="xt-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {/* missing_code can come twice, once for each column: keyed by code and index. */}
            {r.exclusions.map((e, i) => (
              <li key={`${e.code}-${i}`}>{xtabSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="xt-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{xtabSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="xt-digits-note">
        {XTAB_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="xt" manifest={result.manifest} rowsUsedNote={XTAB_ROWS_USED_NOTE} />
    </div>
  );
};
