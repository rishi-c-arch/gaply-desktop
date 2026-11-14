// Gaply — the confirmatory factor analysis's result card (Research OS, Phase 9C-3, decisions 782
// and 783): a file of its own, since the method has no page (ADR-012:233-241). The Output's
// entry and a saved result both draw it.
//
// AMOS's text output in AMOS's order: the notes for the model (the sample moments, the
// parameters, the degrees of freedom; the iterations and the chi-square); the estimates (the
// regression weights, the standardized regression weights, the covariances and correlations of
// the factors where there are any, the variances, the squared multiple correlations); and the
// ten model-fit tables, each with AMOS's rows for the default, saturated and independence
// models, the saturated row left out of RMSEA's and HOELTER's as AMOS leaves it out. Then the
// notes, the rows left out, what is not assessed, the digits note and the manifest.
//
// Every number goes through shownNumber, but NPAR, DF, HOELTER and the iterations, which are
// whole numbers shown with every digit; a -0 shows as 0. In the estimates a not-computable
// value shows its sentence where its number would be; in the model-fit tables it shows "–",
// and under each table one line per code its cells hold gives the code's sentence. The tone is
// neutral (decision 66): no fit measure is given a cutoff or a band's name.
//
// A column's name is the user's own text, shown as text only: never in a key, an attribute, a
// title or a test id, which take the column's alias. Factors are numbered.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import type { CfaModelFit, CfaResultView, CfaStat } from './cfaTypes';
import {
  CFA_DIGITS_NOTE,
  CFA_FIT_TABLES,
  CFA_ROWS_USED_NOTE,
  CF_HEADINGS,
  CF_LABELS,
  CF_NO_COVARIANCES,
  CF_ROWS,
  CF_THREE_MODELS,
  cfaChiSquareSentence,
  cfaCovarianceRow,
  cfaDashLine,
  cfaErrorRow,
  cfaFactorRow,
  cfaFitText,
  cfaIterationsSentence,
  cfaSentence,
  cfaStatText,
  cfaWeightRow,
  cfaWholeText,
  type CfaNames,
  type CfaRow,
} from './cfaWording';

const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };
const table: React.CSSProperties = { borderCollapse: 'collapse' };

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

/** A cell holding a number, or its sentence where the number would be. */
const StatCell: React.FC<{ stat: CfaStat; testId?: string }> = ({ stat, testId }) => (
  <td className={'value' in stat ? 'gds-mono' : undefined} style={CELL_PADDING} data-testid={testId}>
    {cfaStatText(stat)}
  </td>
);

const Heading: React.FC<{ text: string }> = ({ text }) => <h4 style={{ margin: '8px 0 4px' }}>{text}</h4>;

const HeadRow: React.FC<{ heads: readonly string[] }> = ({ heads }) => (
  <thead>
    <tr>
      <th scope="col" style={head} />
      {heads.map((c, i) => (
        <th key={i} scope="col" style={head}>
          {c}
        </th>
      ))}
    </tr>
  </thead>
);

const TESTED = [CF_LABELS.estimate, CF_LABELS.se, CF_LABELS.cr, CF_LABELS.p] as const;

/** An estimate with its S.E., C.R. and P. */
const TestedCells: React.FC<{ s: { estimate: CfaStat; se: CfaStat; cr: CfaStat; p: CfaStat } }> = ({ s }) => (
  <>
    <StatCell stat={s.estimate} />
    <StatCell stat={s.se} />
    <StatCell stat={s.cr} />
    <StatCell stat={s.p} />
  </>
);

const ROW_ORDER: CfaRow[] = ['model', 'saturated', 'independence'];

/** One of AMOS's ten model-fit tables, its rows' not-computable cells shown as "–", and under
 *  it one line per code those cells hold, in the order they first appear. */
const FitTable: React.FC<{ spec: (typeof CFA_FIT_TABLES)[number]; fit: Record<CfaRow, CfaModelFit> }> = ({ spec, fit }) => {
  const rows = ROW_ORDER.filter((r) => r !== 'saturated' || spec.saturated);
  const codes: string[] = [];
  for (const r of rows) {
    for (const c of spec.columns) {
      const s = fit[r][c.key];
      if (typeof s !== 'number' && !('value' in s) && !codes.includes(s.not_computable)) codes.push(s.not_computable);
    }
  }
  return (
    <section aria-label={spec.heading}>
      <Heading text={spec.heading} />
      <table data-testid={`cf-fit-${spec.id}`} style={table}>
        <thead>
          <tr>
            <th scope="col" style={head}>
              {CF_LABELS.model}
            </th>
            {spec.columns.map((c) => (
              <th key={c.key} scope="col" style={head}>
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r} data-testid={`cf-fit-${spec.id}-${r}`}>
              <th scope="row" style={head}>
                {CF_ROWS[r]}
              </th>
              {spec.columns.map((c) => {
                const s = fit[r][c.key];
                const value = typeof s === 'number' || 'value' in s;
                return (
                  <td key={c.key} className={value ? 'gds-mono' : undefined} style={CELL_PADDING}>
                    {cfaFitText(s, c.whole === true)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      {codes.length > 0 && (
        <ul style={{ margin: '4px 0 0' }} data-testid={`cf-fit-${spec.id}-codes`}>
          {codes.map((code) => (
            <li key={code}>{cfaDashLine(code)}</li>
          ))}
        </ul>
      )}
      {spec.id === 'cmin' && (
        <p style={{ margin: '4px 0 0' }} data-testid="cf-three-models">
          {CF_THREE_MODELS}
        </p>
      )}
    </section>
  );
};

/** The neutral result card (decision 66). */
export const CfaCard: React.FC<{ result: CfaResultView; names: CfaNames }> = ({ result, names }) => {
  const r = result.result;
  const name = (alias: string) => names.column(aliasNumber(alias));
  const m = r.fit.model;
  const covariances = r.factorCovariances;
  const uncorrelated = covariances.length === 0 && r.indicators.length >= 2;
  return (
    <div data-testid="cf-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => cfaSentence(d, names)} testId="cf-result-notes" />

      <section aria-label={CF_HEADINGS.notes}>
        <Heading text={CF_HEADINGS.notes} />
        <table data-testid="cf-model-notes" style={table}>
          <tbody>
            <tr data-testid="cf-moments">
              <th scope="row" style={head}>
                {CF_LABELS.moments}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {cfaWholeText(m.npar + m.df)}
              </td>
            </tr>
            <tr data-testid="cf-parameters">
              <th scope="row" style={head}>
                {CF_LABELS.parameters}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {cfaWholeText(m.npar)}
              </td>
            </tr>
            <tr data-testid="cf-df">
              <th scope="row" style={head}>
                {CF_LABELS.df}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {cfaWholeText(m.df)}
              </td>
            </tr>
          </tbody>
        </table>
        <p style={{ margin: '4px 0 0' }} data-testid="cf-iterations">
          {cfaIterationsSentence(r.iterations)}
        </p>
        <p style={{ margin: '4px 0 0' }} data-testid="cf-chi-square">
          {cfaChiSquareSentence(m.cmin, m.df, m.p)}
        </p>
      </section>

      <section aria-label={CF_HEADINGS.estimates}>
        <h3 style={{ margin: '8px 0 4px' }}>{CF_HEADINGS.estimates}</h3>

        <section aria-label={CF_HEADINGS.weights}>
          <Heading text={CF_HEADINGS.weights} />
          <table data-testid="cf-weights" style={table}>
            <HeadRow heads={TESTED} />
            <tbody>
              {r.loadings.map((l) => (
                <tr key={l.item} data-testid={`cf-weight-${l.item}`}>
                  <th scope="row" style={head}>
                    {cfaWeightRow(name(l.item), l.factor)}
                  </th>
                  <TestedCells s={l} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section aria-label={CF_HEADINGS.standardized}>
          <Heading text={CF_HEADINGS.standardized} />
          <table data-testid="cf-standardized" style={table}>
            <HeadRow heads={[CF_LABELS.estimate]} />
            <tbody>
              {r.loadings.map((l) => (
                <tr key={l.item} data-testid={`cf-standardized-${l.item}`}>
                  <th scope="row" style={head}>
                    {cfaWeightRow(name(l.item), l.factor)}
                  </th>
                  <StatCell stat={l.standardized} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        {covariances.length > 0 && (
          <>
            <section aria-label={CF_HEADINGS.covariances}>
              <Heading text={CF_HEADINGS.covariances} />
              <table data-testid="cf-covariances" style={table}>
                <HeadRow heads={TESTED} />
                <tbody>
                  {covariances.map((c) => (
                    <tr key={`${c.factors[0]}-${c.factors[1]}`} data-testid={`cf-covariance-${c.factors[0]}-${c.factors[1]}`}>
                      <th scope="row" style={head}>
                        {cfaCovarianceRow(c.factors[0], c.factors[1])}
                      </th>
                      <TestedCells s={c} />
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
            <section aria-label={CF_HEADINGS.correlations}>
              <Heading text={CF_HEADINGS.correlations} />
              <table data-testid="cf-correlations" style={table}>
                <HeadRow heads={[CF_LABELS.estimate]} />
                <tbody>
                  {covariances.map((c) => (
                    <tr key={`${c.factors[0]}-${c.factors[1]}`} data-testid={`cf-correlation-${c.factors[0]}-${c.factors[1]}`}>
                      <th scope="row" style={head}>
                        {cfaCovarianceRow(c.factors[0], c.factors[1])}
                      </th>
                      <StatCell stat={c.correlation} />
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          </>
        )}
        {uncorrelated && (
          <p style={{ margin: '4px 0 0' }} data-testid="cf-no-covariances">
            {CF_NO_COVARIANCES}
          </p>
        )}

        <section aria-label={CF_HEADINGS.variances}>
          <Heading text={CF_HEADINGS.variances} />
          <table data-testid="cf-variances" style={table}>
            <HeadRow heads={TESTED} />
            <tbody>
              {r.factorVariances.map((v) => (
                <tr key={`factor-${v.factor}`} data-testid={`cf-variance-factor-${v.factor}`}>
                  <th scope="row" style={head}>
                    {cfaFactorRow(v.factor)}
                  </th>
                  <TestedCells s={v} />
                </tr>
              ))}
              {r.errorVariances.map((v) => (
                <tr key={`error-${v.item}`} data-testid={`cf-variance-error-${v.item}`}>
                  <th scope="row" style={head}>
                    {cfaErrorRow(name(v.item))}
                  </th>
                  <TestedCells s={v} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section aria-label={CF_HEADINGS.smc}>
          <Heading text={CF_HEADINGS.smc} />
          <table data-testid="cf-smc" style={table}>
            <HeadRow heads={[CF_LABELS.estimate]} />
            <tbody>
              {r.smc.map((s) => (
                <tr key={s.item} data-testid={`cf-smc-${s.item}`}>
                  <th scope="row" style={head}>
                    {name(s.item)}
                  </th>
                  <StatCell stat={s.value} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </section>

      <section aria-label={CF_HEADINGS.fit}>
        <h3 style={{ margin: '8px 0 4px' }}>{CF_HEADINGS.fit}</h3>
        {CFA_FIT_TABLES.map((t) => (
          <FitTable key={t.id} spec={t} fit={r.fit} />
        ))}
      </section>

      {r.notes.length > 0 && (
        <ul data-testid="cf-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{cfaSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="cf-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((x) => (
              <li key={`${x.code}-${x.params.column}`}>{cfaSentence(x, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="cf-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{cfaSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="cf-digits-note">
        {CFA_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="cf" manifest={result.manifest} rowsUsedNote={CFA_ROWS_USED_NOTE} />
    </div>
  );
};
