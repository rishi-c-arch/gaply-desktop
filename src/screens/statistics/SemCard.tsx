// Gaply — the structural equation model's result card (Research OS, Phase 9C-5, decisions 814 and
// 815), on CfaCard.tsx's pattern and built from what it is built from. The Output's entry and a
// saved result both draw it.
//
// AMOS's text output in AMOS's order: the notes for the model (the sample moments, the
// parameters, the degrees of freedom; the iterations, the attempt that reached the solution where
// it was not the first, and the chi-square); the estimates (the regression weights, the
// standardized regression weights, the covariances and correlations where there are any, the
// variances, the squared multiple correlations); the effects, AMOS's six matrices; and the ten
// model-fit tables, drawn by ModelFitTables.tsx as the CFA's card draws them. Then the notes, the
// rows left out, what is not assessed, the digits note and the manifest.
//
// An endogenous variable's variance, and a covariance's endogenous end, is its error term's,
// named "Error of" it. The effects' matrices have a row per `to` variable the effects name and a
// column per `from` variable, each in the model's order; a cell is blank where the result lists
// no such pair, always on the diagonal. A matrix wider than the card scrolls with the Output's
// pane, as the house's widest tables (the factor analysis's matrices) do.
//
// Every number goes through shownNumber, but NPAR, DF, HOELTER and the iterations, which are
// whole numbers shown with every digit; a -0 shows as 0. In the estimates and the effects a
// not-computable value shows its sentence where its number would be; in the model-fit tables it
// shows "–", and under each table one line per code its cells hold gives the code's sentence. The
// tone is neutral (decision 66): no fit measure is given a cutoff or a band's name.
//
// A column's name is the user's own text, shown as text only: never in a key, an attribute, a
// title or a test id, which take the column's alias. Factors are numbered.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import { ModelFitTables, fitWholeText } from './ModelFitTables';
import type { SemEffect, SemResultView, SemStat, SemVariable } from './semTypes';
import {
  SEM_DIGITS_NOTE,
  SEM_ROWS_USED_NOTE,
  SM_BLANK_CELL,
  SM_HEADINGS,
  SM_LABELS,
  SM_NO_COVARIANCES,
  SM_NO_EFFECTS,
  semAttemptSentence,
  semCellSentence,
  semChiSquareSentence,
  semCovarianceRow,
  semErrorName,
  semIterationsSentence,
  semSentence,
  semStatText,
  semVariableName,
  semWeightRow,
  type SemNames,
} from './semWording';

const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };
const table: React.CSSProperties = { borderCollapse: 'collapse' };

/** A variable's key: its factor's number or its column's alias; never a label. */
const keyOf = (v: SemVariable): string => ('factor' in v ? `f${v.factor}` : v.column);
const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));
/** The model's order: factors by number, then columns by alias number. */
const order = (x: SemVariable, y: SemVariable): number => {
  if ('factor' in x) return 'factor' in y ? x.factor - y.factor : -1;
  return 'factor' in y ? 1 : aliasNumber(x.column) - aliasNumber(y.column);
};

/** A cell holding a number, or its sentence where the number would be. */
const StatCell: React.FC<{ stat: SemStat; testId?: string }> = ({ stat, testId }) => (
  <td className={'value' in stat ? 'gds-mono' : undefined} style={CELL_PADDING} data-testid={testId}>
    {semStatText(stat)}
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

const TESTED = [SM_LABELS.estimate, SM_LABELS.se, SM_LABELS.cr, SM_LABELS.p] as const;

/** An estimate with its S.E., C.R. and P. */
const TestedCells: React.FC<{ s: { estimate: SemStat; se: SemStat; cr: SemStat; p: SemStat } }> = ({ s }) => (
  <>
    <StatCell stat={s.estimate} />
    <StatCell stat={s.se} />
    <StatCell stat={s.cr} />
    <StatCell stat={s.p} />
  </>
);

/** AMOS's six effects matrices, in AMOS's order. */
const MATRICES: ReadonlyArray<{ id: string; heading: string; field: keyof Omit<SemEffect, 'to' | 'from'> }> = [
  { id: 'total', heading: SM_HEADINGS.total, field: 'total' },
  { id: 'total-standardized', heading: SM_HEADINGS.totalStandardized, field: 'totalStandardized' },
  { id: 'direct', heading: SM_HEADINGS.direct, field: 'direct' },
  { id: 'direct-standardized', heading: SM_HEADINGS.directStandardized, field: 'directStandardized' },
  { id: 'indirect', heading: SM_HEADINGS.indirect, field: 'indirect' },
  { id: 'indirect-standardized', heading: SM_HEADINGS.indirectStandardized, field: 'indirectStandardized' },
];

/** One effects matrix: a row per `to`, a column per `from`, blank where no pair is listed. */
const EffectsMatrix: React.FC<{
  spec: (typeof MATRICES)[number];
  rows: SemVariable[];
  columns: SemVariable[];
  cell: Map<string, SemEffect>;
  name: (v: SemVariable) => string;
}> = ({ spec, rows, columns, cell, name }) => (
  <section aria-label={spec.heading}>
    <Heading text={spec.heading} />
    <table data-testid={`sm-effects-${spec.id}`} style={table}>
      <thead>
        <tr>
          <th scope="col" style={head} />
          {columns.map((c) => (
            <th key={keyOf(c)} scope="col" style={head}>
              {name(c)}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={keyOf(r)} data-testid={`sm-effects-${spec.id}-${keyOf(r)}`}>
            <th scope="row" style={head}>
              {name(r)}
            </th>
            {columns.map((c) => {
              const e = cell.get(`${keyOf(r)}<${keyOf(c)}`);
              return e === undefined ? (
                <td key={keyOf(c)} style={CELL_PADDING} data-testid={`sm-effect-${spec.id}-${keyOf(r)}-${keyOf(c)}`} />
              ) : (
                <StatCell key={keyOf(c)} stat={e[spec.field]} testId={`sm-effect-${spec.id}-${keyOf(r)}-${keyOf(c)}`} />
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  </section>
);

/** The neutral result card (decision 66). */
export const SemCard: React.FC<{ result: SemResultView; names: SemNames }> = ({ result, names }) => {
  const r = result.result;
  const name = (v: SemVariable) => semVariableName(v, names);
  // An endogenous variable is one whose variance is its error term's.
  const error = new Set(r.variances.filter((v) => v.error).map((v) => keyOf(v.variable)));
  const term = (v: SemVariable) => (error.has(keyOf(v)) ? semErrorName(name(v)) : name(v));
  const m = r.fit.model;
  const attempt = semAttemptSentence(r.attempt);
  const unique = (vs: SemVariable[]) => vs.filter((v, i) => vs.findIndex((w) => keyOf(w) === keyOf(v)) === i).sort(order);
  const rows = unique(r.effects.map((e) => e.to));
  const columns = unique(r.effects.map((e) => e.from));
  const cell = new Map(r.effects.map((e) => [`${keyOf(e.to)}<${keyOf(e.from)}`, e]));
  return (
    <div data-testid="sm-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => semSentence(d, names)} testId="sm-result-notes" />

      <section aria-label={SM_HEADINGS.notes}>
        <Heading text={SM_HEADINGS.notes} />
        <table data-testid="sm-model-notes" style={table}>
          <tbody>
            <tr data-testid="sm-moments">
              <th scope="row" style={head}>
                {SM_LABELS.moments}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {fitWholeText(m.npar + m.df)}
              </td>
            </tr>
            <tr data-testid="sm-parameters">
              <th scope="row" style={head}>
                {SM_LABELS.parameters}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {fitWholeText(m.npar)}
              </td>
            </tr>
            <tr data-testid="sm-df">
              <th scope="row" style={head}>
                {SM_LABELS.df}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {fitWholeText(m.df)}
              </td>
            </tr>
          </tbody>
        </table>
        <p style={{ margin: '4px 0 0' }} data-testid="sm-iterations">
          {semIterationsSentence(r.iterations)}
        </p>
        {attempt !== null && (
          <p style={{ margin: '4px 0 0' }} data-testid="sm-attempt">
            {attempt}
          </p>
        )}
        <p style={{ margin: '4px 0 0' }} data-testid="sm-chi-square">
          {semChiSquareSentence(m.cmin, m.df, m.p)}
        </p>
      </section>

      <section aria-label={SM_HEADINGS.estimates}>
        <h3 style={{ margin: '8px 0 4px' }}>{SM_HEADINGS.estimates}</h3>

        <section aria-label={SM_HEADINGS.weights}>
          <Heading text={SM_HEADINGS.weights} />
          <table data-testid="sm-weights" style={table}>
            <HeadRow heads={TESTED} />
            <tbody>
              {r.weights.map((w) => (
                <tr key={`${keyOf(w.to)}-${keyOf(w.from)}`} data-testid={`sm-weight-${keyOf(w.to)}-${keyOf(w.from)}`}>
                  <th scope="row" style={head}>
                    {semWeightRow(name(w.to), name(w.from))}
                  </th>
                  <TestedCells s={w} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section aria-label={SM_HEADINGS.standardized}>
          <Heading text={SM_HEADINGS.standardized} />
          <table data-testid="sm-standardized" style={table}>
            <HeadRow heads={[SM_LABELS.estimate]} />
            <tbody>
              {r.weights.map((w) => (
                <tr key={`${keyOf(w.to)}-${keyOf(w.from)}`} data-testid={`sm-standardized-${keyOf(w.to)}-${keyOf(w.from)}`}>
                  <th scope="row" style={head}>
                    {semWeightRow(name(w.to), name(w.from))}
                  </th>
                  <StatCell stat={w.standardized} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        {r.covariances.length > 0 ? (
          <>
            <section aria-label={SM_HEADINGS.covariances}>
              <Heading text={SM_HEADINGS.covariances} />
              <table data-testid="sm-covariances" style={table}>
                <HeadRow heads={TESTED} />
                <tbody>
                  {r.covariances.map((c) => (
                    <tr key={`${keyOf(c.a)}-${keyOf(c.b)}`} data-testid={`sm-covariance-${keyOf(c.a)}-${keyOf(c.b)}`}>
                      <th scope="row" style={head}>
                        {semCovarianceRow(term(c.a), term(c.b))}
                      </th>
                      <TestedCells s={c} />
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
            <section aria-label={SM_HEADINGS.correlations}>
              <Heading text={SM_HEADINGS.correlations} />
              <table data-testid="sm-correlations" style={table}>
                <HeadRow heads={[SM_LABELS.estimate]} />
                <tbody>
                  {r.covariances.map((c) => (
                    <tr key={`${keyOf(c.a)}-${keyOf(c.b)}`} data-testid={`sm-correlation-${keyOf(c.a)}-${keyOf(c.b)}`}>
                      <th scope="row" style={head}>
                        {semCovarianceRow(term(c.a), term(c.b))}
                      </th>
                      <StatCell stat={c.correlation} />
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          </>
        ) : (
          <p style={{ margin: '4px 0 0' }} data-testid="sm-no-covariances">
            {SM_NO_COVARIANCES}
          </p>
        )}

        <section aria-label={SM_HEADINGS.variances}>
          <Heading text={SM_HEADINGS.variances} />
          <table data-testid="sm-variances" style={table}>
            <HeadRow heads={TESTED} />
            <tbody>
              {r.variances.map((v) => (
                <tr key={keyOf(v.variable)} data-testid={`sm-variance-${keyOf(v.variable)}`}>
                  <th scope="row" style={head}>
                    {v.error ? semErrorName(name(v.variable)) : name(v.variable)}
                  </th>
                  <TestedCells s={v} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        <section aria-label={SM_HEADINGS.smc}>
          <Heading text={SM_HEADINGS.smc} />
          <table data-testid="sm-smc" style={table}>
            <HeadRow heads={[SM_LABELS.estimate]} />
            <tbody>
              {r.smc.map((s) => (
                <tr key={keyOf(s.variable)} data-testid={`sm-smc-${keyOf(s.variable)}`}>
                  <th scope="row" style={head}>
                    {name(s.variable)}
                  </th>
                  <StatCell stat={s.value} />
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </section>

      <section aria-label={SM_HEADINGS.effects} data-testid="sm-effects">
        <h3 style={{ margin: '8px 0 4px' }}>{SM_HEADINGS.effects}</h3>
        {r.effects.length === 0 ? (
          <p style={{ margin: 0 }} data-testid="sm-no-effects">
            {SM_NO_EFFECTS}
          </p>
        ) : (
          MATRICES.map((spec, i) => (
            <React.Fragment key={spec.id}>
              <EffectsMatrix spec={spec} rows={rows} columns={columns} cell={cell} name={name} />
              {i === 0 && (
                <p style={{ margin: '4px 0 0' }} data-testid="sm-blank-cell">
                  {SM_BLANK_CELL}
                </p>
              )}
            </React.Fragment>
          ))
        )}
      </section>

      <section aria-label={SM_HEADINGS.fit}>
        <h3 style={{ margin: '8px 0 4px' }}>{SM_HEADINGS.fit}</h3>
        <ModelFitTables fit={r.fit} prefix="sm" cellSentence={semCellSentence} />
      </section>

      {r.notes.length > 0 && (
        <ul data-testid="sm-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{semSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="sm-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((x) => (
              <li key={`${x.code}-${x.params.column}`}>{semSentence(x, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="sm-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{semSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="sm-digits-note">
        {SEM_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="sm" manifest={result.manifest} rowsUsedNote={SEM_ROWS_USED_NOTE} />
    </div>
  );
};
