// Gaply — the linear regression's result card (Research OS, Phase 9A-2, decisions 684 and
// 685): a file of its own, since the method has no page (ADR-012:233-241). The Output's
// entry and a saved result both draw it.
//
// Six tables, in SPSS's order: the model summary, the analysis of variance, the coefficients
// (the intercept, then each predictor in the list's order), the correlations and
// collinearity, the descriptive statistics, and the residuals statistics. Then the note, the
// rows left out, what is not assessed, the digits note and the manifest. A not-computable
// value shows its sentence where its number would be; in the intercept's row, when every cell
// after B holds the same code, one cell spanning them holds its sentence once, as XtabCard.tsx
// does. Every number goes through shownNumber. The tone is neutral (decision 66).
//
// A column's name is the user's own text, shown as text only: never in a key, an attribute,
// a title or a test id, which take the column's alias.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import type { Extremes, InterceptEstimate, LinearRegressionResultView, LinearRegressionStat, PredictorEstimate, Spread } from './linearRegressionTypes';
import {
  LINEAR_REGRESSION_DIGITS_NOTE,
  LINEAR_REGRESSION_ROWS_USED_NOTE,
  LR_ANOVA_HEADS,
  LR_ANOVA_ROWS,
  LR_COEFFICIENT_HEADS,
  LR_COLUMN_HEAD,
  LR_CORRELATION_HEADS,
  LR_DESCRIPTIVE_HEADS,
  LR_HEADINGS,
  LR_INTERCEPT,
  LR_MODEL_LABELS,
  LR_RESIDUAL_HEADS,
  LR_RESIDUAL_ROWS,
  linearRegressionCoefficientsSentence,
  linearRegressionSentence,
  linearRegressionStatText,
  type LinearRegressionNames,
} from './linearRegressionWording';
import { shownNumber } from './ttestWording';

const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };
const table: React.CSSProperties = { borderCollapse: 'collapse' };

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));
const codeOf = (s: LinearRegressionStat) => ('not_computable' in s ? s.not_computable : null);

/** A cell holding a number, or its sentence where the number would be. */
const StatCell: React.FC<{ stat: LinearRegressionStat; testId?: string }> = ({ stat, testId }) => (
  <td className={'value' in stat ? 'gds-mono' : undefined} style={CELL_PADDING} data-testid={testId}>
    {linearRegressionStatText(stat)}
  </td>
);

/** The 95% interval in one cell, "a to b" as AnovaFlow.tsx writes it; a shared code once. */
const IntervalCell: React.FC<{ lower: LinearRegressionStat; upper: LinearRegressionStat; testId: string }> = ({ lower, upper, testId }) => {
  if ('value' in lower && 'value' in upper) {
    return (
      <td className="gds-mono" style={CELL_PADDING} data-testid={testId}>
        {shownNumber(lower.value)} to {shownNumber(upper.value)}
      </td>
    );
  }
  const same = codeOf(lower) !== null && codeOf(lower) === codeOf(upper);
  return (
    <td style={CELL_PADDING} data-testid={testId}>
      {same ? linearRegressionStatText(lower) : `${linearRegressionStatText(lower)} to ${linearRegressionStatText(upper)}`}
    </td>
  );
};

const Heading: React.FC<{ text: string }> = ({ text }) => <h4 style={{ margin: '8px 0 4px' }}>{text}</h4>;

const HeadRow: React.FC<{ heads: readonly string[]; first?: string }> = ({ heads, first }) => (
  <thead>
    <tr>
      <th scope="col" style={head}>
        {first}
      </th>
      {heads.map((c) => (
        <th key={c} scope="col" style={head}>
          {c}
        </th>
      ))}
    </tr>
  </thead>
);

/** The intercept's row. When SE, t, p and both interval ends hold one code, one cell spans
 *  them, the empty beta cell included, and holds its sentence (XtabCard.tsx:73-76). */
const InterceptRow: React.FC<{ c: InterceptEstimate }> = ({ c }) => {
  const code = codeOf(c.standardError);
  const spanned = code !== null && [c.t, c.p, c.lower, c.upper].every((s) => codeOf(s) === code);
  return (
    <tr data-testid="lr-coef-intercept">
      <th scope="row" style={head}>
        {LR_INTERCEPT}
      </th>
      <StatCell stat={c.b} />
      {spanned ? (
        <td colSpan={5} style={CELL_PADDING} data-testid="lr-coef-intercept-not-computable">
          {linearRegressionStatText(c.standardError)}
        </td>
      ) : (
        <>
          <StatCell stat={c.standardError} />
          <td style={CELL_PADDING} />
          <StatCell stat={c.t} />
          <StatCell stat={c.p} />
          <IntervalCell lower={c.lower} upper={c.upper} testId="lr-coef-intercept-interval" />
        </>
      )}
    </tr>
  );
};

/** A predictor's row: each coded cell holds its sentence, and beta its number. */
const PredictorRow: React.FC<{ p: PredictorEstimate; name: string }> = ({ p, name }) => (
  <tr data-testid={`lr-coef-${p.column}`}>
    <th scope="row" style={head}>
      {name}
    </th>
    <StatCell stat={p.b} />
    <StatCell stat={p.standardError} />
    <StatCell stat={p.beta} testId={`lr-beta-${p.column}`} />
    <StatCell stat={p.t} />
    <StatCell stat={p.p} />
    <IntervalCell lower={p.lower} upper={p.upper} testId={`lr-interval-${p.column}`} />
  </tr>
);

/** A residuals row: four statistics, or two with the mean and SD cells empty. */
const ResidualRow: React.FC<{ label: string; s: Spread | Extremes; testId: string }> = ({ label, s, testId }) => (
  <tr data-testid={testId}>
    <th scope="row" style={head}>
      {label}
    </th>
    <StatCell stat={s.minimum} />
    <StatCell stat={s.maximum} />
    {'mean' in s ? <StatCell stat={s.mean} /> : <td style={CELL_PADDING} />}
    {'standardDeviation' in s ? <StatCell stat={s.standardDeviation} /> : <td style={CELL_PADDING} />}
  </tr>
);

/** The neutral result card (decision 66): the six tables, what is not computable in its own
 *  words, what is not assessed, and the manifest. */
export const LinearRegressionCard: React.FC<{ result: LinearRegressionResultView; names: LinearRegressionNames }> = ({ result, names }) => {
  const r = result.result;
  const name = (alias: string) => names.column(aliasNumber(alias));
  const m = r.model;
  const a = r.anova;
  return (
    <div data-testid="lr-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => linearRegressionSentence(d, names)} testId="lr-result-notes" />

      <section aria-label={LR_HEADINGS.model}>
        <Heading text={LR_HEADINGS.model} />
        <table data-testid="lr-model" style={table}>
          <tbody>
            {(
              [
                ['r', m.r],
                ['rSquared', m.rSquared],
                ['adjustedRSquared', m.adjustedRSquared],
                ['standardErrorOfEstimate', m.standardErrorOfEstimate],
                ['durbinWatson', m.durbinWatson],
              ] as const
            ).map(([key, stat]) => (
              <tr key={key} data-testid={`lr-model-${key}`}>
                <th scope="row" style={head}>
                  {LR_MODEL_LABELS[key]}
                </th>
                <StatCell stat={stat} />
              </tr>
            ))}
            <tr data-testid="lr-model-rowsUsed">
              <th scope="row" style={head}>
                {LR_MODEL_LABELS.rowsUsed}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {shownNumber(r.rowsUsed)}
              </td>
            </tr>
          </tbody>
        </table>
      </section>

      <section aria-label={LR_HEADINGS.anova}>
        <Heading text={LR_HEADINGS.anova} />
        <table data-testid="lr-anova" style={table}>
          <HeadRow heads={LR_ANOVA_HEADS} />
          <tbody>
            <tr data-testid="lr-anova-regression">
              <th scope="row" style={head}>
                {LR_ANOVA_ROWS.regression}
              </th>
              <StatCell stat={a.regression.sumOfSquares} />
              <td className="gds-mono" style={CELL_PADDING}>{shownNumber(a.regression.df)}</td>
              <StatCell stat={a.regression.meanSquare} />
              <StatCell stat={a.f} testId="lr-anova-f" />
              <StatCell stat={a.p} testId="lr-anova-p" />
            </tr>
            <tr data-testid="lr-anova-residual">
              <th scope="row" style={head}>
                {LR_ANOVA_ROWS.residual}
              </th>
              <StatCell stat={a.residual.sumOfSquares} />
              <td className="gds-mono" style={CELL_PADDING}>{shownNumber(a.residual.df)}</td>
              <StatCell stat={a.residual.meanSquare} />
              <td style={CELL_PADDING} />
              <td style={CELL_PADDING} />
            </tr>
            <tr data-testid="lr-anova-total">
              <th scope="row" style={head}>
                {LR_ANOVA_ROWS.total}
              </th>
              <StatCell stat={a.total.sumOfSquares} />
              <td className="gds-mono" style={CELL_PADDING}>{shownNumber(a.total.df)}</td>
              <td style={CELL_PADDING} />
              <td style={CELL_PADDING} />
              <td style={CELL_PADDING} />
            </tr>
          </tbody>
        </table>
      </section>

      <section aria-label={LR_HEADINGS.coefficients}>
        <Heading text={LR_HEADINGS.coefficients} />
        <table data-testid="lr-coefficients" style={table}>
          <HeadRow heads={LR_COEFFICIENT_HEADS} />
          <tbody>
            <InterceptRow c={r.intercept} />
            {r.predictors.map((p) => (
              <PredictorRow key={p.column} p={p} name={name(p.column)} />
            ))}
          </tbody>
        </table>
        <p style={{ margin: '4px 0 0' }} data-testid="lr-coefficients-sentence">
          {linearRegressionCoefficientsSentence(name(r.outcome))}
        </p>
      </section>

      <section aria-label={LR_HEADINGS.correlations}>
        <Heading text={LR_HEADINGS.correlations} />
        <table data-testid="lr-correlations" style={table}>
          <HeadRow heads={LR_CORRELATION_HEADS} />
          <tbody>
            {r.predictors.map((p) => (
              <tr key={p.column} data-testid={`lr-corr-${p.column}`}>
                <th scope="row" style={head}>
                  {name(p.column)}
                </th>
                <StatCell stat={p.zeroOrder} />
                <StatCell stat={p.partial} />
                <StatCell stat={p.part} />
                <StatCell stat={p.tolerance} />
                <StatCell stat={p.vif} />
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section aria-label={LR_HEADINGS.descriptives}>
        <Heading text={LR_HEADINGS.descriptives} />
        <table data-testid="lr-descriptives" style={table}>
          <HeadRow heads={LR_DESCRIPTIVE_HEADS} first={LR_COLUMN_HEAD} />
          <tbody>
            {r.columns.map((c) => (
              <tr key={c.column} data-testid={`lr-desc-${c.column}`}>
                <th scope="row" style={head}>
                  {name(c.column)}
                </th>
                <StatCell stat={c.mean} />
                <StatCell stat={c.sd} />
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section aria-label={LR_HEADINGS.residuals}>
        <Heading text={LR_HEADINGS.residuals} />
        <table data-testid="lr-residuals" style={table}>
          <HeadRow heads={LR_RESIDUAL_HEADS} />
          <tbody>
            <ResidualRow label={LR_RESIDUAL_ROWS.predicted} s={r.residuals.predicted} testId="lr-res-predicted" />
            <ResidualRow label={LR_RESIDUAL_ROWS.residual} s={r.residuals.residual} testId="lr-res-residual" />
            <ResidualRow label={LR_RESIDUAL_ROWS.standardizedPredicted} s={r.residuals.standardizedPredicted} testId="lr-res-standardizedPredicted" />
            <ResidualRow label={LR_RESIDUAL_ROWS.standardizedResidual} s={r.residuals.standardizedResidual} testId="lr-res-standardizedResidual" />
          </tbody>
        </table>
      </section>

      {r.notes.length > 0 && (
        <ul data-testid="lr-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{linearRegressionSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="lr-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((e) => (
              <li key={`${e.code}-${e.params.column}`}>{linearRegressionSentence(e, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="lr-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{linearRegressionSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="lr-digits-note">
        {LINEAR_REGRESSION_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="lr" manifest={result.manifest} rowsUsedNote={LINEAR_REGRESSION_ROWS_USED_NOTE} />
    </div>
  );
};
