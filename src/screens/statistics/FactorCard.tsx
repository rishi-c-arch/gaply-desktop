// Gaply — the factor analysis's result card (Research OS, Phase 9B-3, decisions 729 and 730):
// a file of its own, since the method has no page (ADR-012:233-241). The Output's entry and a
// saved result both draw it.
//
// SPSS's tables in SPSS's order: the descriptive statistics, KMO and Bartlett's test (with the
// determinant of the correlation matrix and its log, since the matrix itself is not shown),
// each item's sampling adequacy, the communalities, the total variance explained, the
// component or factor matrix, and then the rotation's: for varimax the rotated matrix and the
// transformation matrix, for promax the pattern matrix, the structure matrix and the
// correlation matrix of the components or factors. No rotation table is drawn when there is no
// rotation; the notes say why. "Component" for principal components, "Factor" for principal
// axis, as SPSS names them; their columns are numbered 1 to m. Then the notes, the rows left
// out, what is not assessed, the digits note and the manifest. A not-computable value shows its
// sentence where its number would be. Every number goes through shownNumber. The tone is
// neutral (decision 66): no KMO value is given a band's name.
//
// An item's name is the user's own text, shown as text only: never in a key, an attribute, a
// title or a test id, which take the item's alias.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import { DiagnosticList, ManifestPanel } from './flowParts';
import type { Extraction, FactorResultView, FactorStat, FactorSums } from './factorTypes';
import {
  FACTOR_DIGITS_NOTE,
  FACTOR_ROWS_USED_NOTE,
  FA_HEADINGS,
  FA_LABELS,
  FA_PROMAX_SUMS_SENTENCE,
  factorExtractedFootnote,
  factorRotationFootnote,
  factorSentence,
  factorStatText,
  factorWord,
  type FactorNames,
} from './factorWording';
import { shownNumber } from './ttestWording';

const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };
const table: React.CSSProperties = { borderCollapse: 'collapse' };

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

/** A cell holding a number, or its sentence where the number would be. */
const StatCell: React.FC<{ stat: FactorStat; testId?: string }> = ({ stat, testId }) => (
  <td className={'value' in stat ? 'gds-mono' : undefined} style={CELL_PADDING} data-testid={testId}>
    {factorStatText(stat)}
  </td>
);

const Empty: React.FC = () => <td style={CELL_PADDING} />;

const Heading: React.FC<{ text: string }> = ({ text }) => <h4 style={{ margin: '8px 0 4px' }}>{text}</h4>;

const HeadRow: React.FC<{ heads: readonly string[]; first?: string }> = ({ heads, first }) => (
  <thead>
    <tr>
      <th scope="col" style={head}>
        {first}
      </th>
      {heads.map((c, i) => (
        <th key={i} scope="col" style={head}>
          {c}
        </th>
      ))}
    </tr>
  </thead>
);

const numbered = (m: number) => Array.from({ length: m }, (_, k) => String(k + 1));

/** A matrix of items by components or factors, each row named by its item. */
const ItemMatrix: React.FC<{ id: string; heading: string; extraction: Extraction; items: string[]; rows: FactorStat[][]; name: (alias: string) => string; footnote?: string }> = ({
  id,
  heading,
  extraction,
  items,
  rows,
  name,
  footnote,
}) => (
  <section aria-label={heading}>
    <Heading text={heading} />
    <table data-testid={`fa-${id}`} style={table}>
      <thead>
        <tr>
          <th scope="col" style={head}>
            {FA_LABELS.item}
          </th>
          <th scope="colgroup" colSpan={rows[0]?.length ?? 0} style={head}>
            {factorWord(extraction)}
          </th>
        </tr>
        <tr>
          <th scope="col" style={head} />
          {numbered(rows[0]?.length ?? 0).map((k) => (
            <th key={k} scope="col" style={head}>
              {k}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, i) => (
          <tr key={items[i]} data-testid={`fa-${id}-${items[i]}`}>
            <th scope="row" style={head}>
              {name(items[i])}
            </th>
            {row.map((s, k) => (
              <StatCell key={k} stat={s} />
            ))}
          </tr>
        ))}
      </tbody>
    </table>
    {footnote && (
      <p style={{ margin: '4px 0 0' }} data-testid={`fa-${id}-footnote`}>
        {footnote}
      </p>
    )}
  </section>
);

/** An m × m matrix, its rows and columns numbered. */
const SquareMatrix: React.FC<{ id: string; heading: string; extraction: Extraction; rows: FactorStat[][] }> = ({ id, heading, extraction, rows }) => (
  <section aria-label={heading}>
    <Heading text={heading} />
    <table data-testid={`fa-${id}`} style={table}>
      <HeadRow heads={numbered(rows.length)} first={factorWord(extraction)} />
      <tbody>
        {rows.map((row, j) => (
          <tr key={j} data-testid={`fa-${id}-${j + 1}`}>
            <th scope="row" style={head}>
              {j + 1}
            </th>
            {row.map((s, k) => (
              <StatCell key={k} stat={s} />
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  </section>
);

const SumsCells: React.FC<{ s: FactorSums | undefined }> = ({ s }) =>
  s ? (
    <>
      <StatCell stat={s.total} />
      <StatCell stat={s.percent} />
      <StatCell stat={s.cumulative} />
    </>
  ) : (
    <>
      <Empty />
      <Empty />
      <Empty />
    </>
  );

/** The neutral result card (decision 66). */
export const FactorCard: React.FC<{ result: FactorResultView; names: FactorNames }> = ({ result, names }) => {
  const r = result.result;
  const name = (alias: string) => names.column(aliasNumber(alias));
  const e = r.extraction.method;
  const rot = r.rotation;
  const varimaxSums = rot?.method === 'varimax' ? rot.sums : null;
  const sumHeads = [FA_LABELS.total, FA_LABELS.percent, FA_LABELS.cumulative];
  return (
    <div data-testid="fa-result-card" data-tone="neutral" style={{ display: 'grid', gap: 8 }}>
      <DiagnosticList items={result.diagnostics} sentence={(d) => factorSentence(d, names)} testId="fa-result-notes" />

      <section aria-label={FA_HEADINGS.descriptives}>
        <Heading text={FA_HEADINGS.descriptives} />
        <table data-testid="fa-descriptives" style={table}>
          <HeadRow heads={[FA_LABELS.mean, FA_LABELS.sd]} first={FA_LABELS.item} />
          <tbody>
            {r.descriptives.map((d) => (
              <tr key={d.item} data-testid={`fa-desc-${d.item}`}>
                <th scope="row" style={head}>
                  {name(d.item)}
                </th>
                <StatCell stat={d.mean} />
                <StatCell stat={d.sd} />
              </tr>
            ))}
          </tbody>
        </table>
        <p style={{ margin: '4px 0 0' }} data-testid="fa-rows-used">
          {FA_LABELS.rowsUsed}: <span className="gds-mono">{shownNumber(r.rowsUsed)}</span>
        </p>
      </section>

      <section aria-label={FA_HEADINGS.kmo}>
        <Heading text={FA_HEADINGS.kmo} />
        <table data-testid="fa-kmo" style={table}>
          <tbody>
            <tr data-testid="fa-kmo-overall">
              <th scope="row" colSpan={2} style={head}>
                {FA_LABELS.kmo}
              </th>
              <StatCell stat={r.kmo.overall} />
            </tr>
            <tr data-testid="fa-bartlett-chi">
              <th scope="row" rowSpan={3} style={head}>
                {FA_LABELS.bartlett}
              </th>
              <th scope="row" style={head}>
                {FA_LABELS.chiSquare}
              </th>
              <StatCell stat={r.bartlett.chiSquare} />
            </tr>
            <tr data-testid="fa-bartlett-df">
              <th scope="row" style={head}>
                {FA_LABELS.df}
              </th>
              <td className="gds-mono" style={CELL_PADDING}>
                {shownNumber(r.bartlett.df)}
              </td>
            </tr>
            <tr data-testid="fa-bartlett-p">
              <th scope="row" style={head}>
                {FA_LABELS.p}
              </th>
              <StatCell stat={r.bartlett.p} />
            </tr>
            <tr data-testid="fa-determinant">
              <th scope="row" colSpan={2} style={head}>
                {FA_LABELS.determinant}
              </th>
              <StatCell stat={r.determinant.value} />
            </tr>
            <tr data-testid="fa-log-determinant">
              <th scope="row" colSpan={2} style={head}>
                {FA_LABELS.logDeterminant}
              </th>
              <StatCell stat={r.determinant.log} />
            </tr>
          </tbody>
        </table>
      </section>

      <section aria-label={FA_HEADINGS.msa}>
        <Heading text={FA_HEADINGS.msa} />
        <table data-testid="fa-msa" style={table}>
          <HeadRow heads={[FA_LABELS.msa]} first={FA_LABELS.item} />
          <tbody>
            {r.kmo.items.map((a) => (
              <tr key={a.item} data-testid={`fa-msa-${a.item}`}>
                <th scope="row" style={head}>
                  {name(a.item)}
                </th>
                <StatCell stat={a.msa} />
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section aria-label={FA_HEADINGS.communalities}>
        <Heading text={FA_HEADINGS.communalities} />
        <table data-testid="fa-communalities" style={table}>
          <HeadRow heads={[FA_LABELS.initial, FA_LABELS.extraction]} first={FA_LABELS.item} />
          <tbody>
            {r.extraction.communalities.map((c) => (
              <tr key={c.item} data-testid={`fa-comm-${c.item}`}>
                <th scope="row" style={head}>
                  {name(c.item)}
                </th>
                <StatCell stat={c.initial} />
                <StatCell stat={c.extraction} />
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      <section aria-label={FA_HEADINGS.variance}>
        <Heading text={FA_HEADINGS.variance} />
        <table data-testid="fa-variance" style={table}>
          <thead>
            <tr>
              <th scope="col" style={head}>
                {factorWord(e)}
              </th>
              <th scope="colgroup" colSpan={3} style={head}>
                {FA_LABELS.initialEigenvalues}
              </th>
              <th scope="colgroup" colSpan={3} style={head}>
                {FA_LABELS.extractionSums}
              </th>
              {varimaxSums && (
                <th scope="colgroup" colSpan={3} style={head} data-testid="fa-variance-rotation-head">
                  {FA_LABELS.rotationSums}
                </th>
              )}
            </tr>
            <tr>
              <th scope="col" style={head} />
              {[...sumHeads, ...sumHeads, ...(varimaxSums ? sumHeads : [])].map((h, i) => (
                <th key={i} scope="col" style={head}>
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {r.eigenvalues.map((ev, k) => (
              <tr key={k} data-testid={`fa-variance-${k + 1}`}>
                <th scope="row" style={head}>
                  {k + 1}
                </th>
                <SumsCells s={ev} />
                <SumsCells s={r.extraction.sums[k]} />
                {varimaxSums && <SumsCells s={varimaxSums[k]} />}
              </tr>
            ))}
          </tbody>
        </table>
        {rot?.method === 'promax' && (
          <p style={{ margin: '4px 0 0' }} data-testid="fa-promax-sums-sentence">
            {FA_PROMAX_SUMS_SENTENCE}
          </p>
        )}
      </section>

      <ItemMatrix
        id="matrix"
        heading={FA_HEADINGS.matrix(e)}
        extraction={e}
        items={r.items}
        rows={r.extraction.loadings}
        name={name}
        footnote={factorExtractedFootnote(e, r.extraction.factors, r.extraction.iterations)}
      />

      {rot?.method === 'varimax' && (
        <>
          <ItemMatrix id="rotated" heading={FA_HEADINGS.rotated(e)} extraction={e} items={r.items} rows={rot.loadings} name={name} footnote={factorRotationFootnote('varimax', rot.iterations)} />
          {rot.transformation && <SquareMatrix id="transformation" heading={FA_HEADINGS.transformation(e)} extraction={e} rows={rot.transformation} />}
        </>
      )}
      {rot?.method === 'promax' && (
        <>
          <ItemMatrix id="pattern" heading={FA_HEADINGS.pattern} extraction={e} items={r.items} rows={rot.loadings} name={name} footnote={factorRotationFootnote('promax', rot.iterations)} />
          {rot.structure && <ItemMatrix id="structure" heading={FA_HEADINGS.structure} extraction={e} items={r.items} rows={rot.structure} name={name} />}
          {rot.factorCorrelations && <SquareMatrix id="correlations" heading={FA_HEADINGS.correlations(e)} extraction={e} rows={rot.factorCorrelations} />}
        </>
      )}

      {r.notes.length > 0 && (
        <ul data-testid="fa-notes" style={{ margin: 0 }}>
          {r.notes.map((code) => (
            <li key={code}>{factorSentence({ code })}</li>
          ))}
        </ul>
      )}
      <section aria-label="Rows left out" data-testid="fa-exclusions">
        <h4 style={{ margin: '8px 0 4px' }}>Rows left out</h4>
        {r.exclusions.length === 0 ? (
          <p style={{ margin: 0 }}>No row was left out.</p>
        ) : (
          <ul style={{ margin: 0 }}>
            {r.exclusions.map((x) => (
              <li key={`${x.code}-${x.params.column}`}>{factorSentence(x, names)}</li>
            ))}
          </ul>
        )}
      </section>
      <section aria-label="What this does not assess" data-testid="fa-limitations">
        <h4 style={{ margin: '8px 0 4px' }}>What this does not assess</h4>
        <ul style={{ margin: 0 }}>
          {r.limitations.map((code) => (
            <li key={code}>{factorSentence({ code })}</li>
          ))}
        </ul>
      </section>
      <p className="gds-jc__disclaimer" data-testid="fa-digits-note">
        {FACTOR_DIGITS_NOTE}
      </p>
      <ManifestPanel prefix="fa" manifest={result.manifest} rowsUsedNote={FACTOR_ROWS_USED_NOTE} />
    </div>
  );
};
