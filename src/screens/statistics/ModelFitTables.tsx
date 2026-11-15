// Gaply — AMOS's ten model-fit tables, shared by the confirmatory factor analysis's card and the
// structural equation model's (Research OS, Phase 9C-5, decision 814). Moved here unchanged from
// CfaCard.tsx (its FitTable) and cfaWording.ts (the tables, their rows' names, the dash and its
// line, the whole numbers and the cells' text), which re-export them under their old names, so
// the CFA's card draws the same markup and its test ids and sentences do not change. A card
// passes its test-id prefix ("cf", "sm") and its own sentence for a not-computable code.
//
// Each table has AMOS's rows for the default, saturated and independence models, the saturated
// row left out of RMSEA's and HOELTER's as AMOS leaves it out. A cell with no number shows "–",
// and under the table one line per code its cells hold gives the code's sentence, in the order
// the codes first appear. NPAR, DF and HOELTER are whole numbers shown with every digit; a -0
// shows as 0. The tone is neutral (decision 66): no fit measure is given a cutoff or a band's
// name.
import React from 'react';
import { CELL_PADDING } from '../../design-system/primitives';
import type { CfaModelFit } from './cfaTypes';
import type { SemModelFit } from './semTypes';
import { shownNumber } from './ttestWording';

/** A measure of one model-fit row: the CFA's and the SEM's rows have the same keys. */
export type FitKey = keyof CfaModelFit & keyof SemModelFit;

/** A model-fit cell's value: a whole number (NPAR, DF), a number, or its code. */
export type FitCell = number | { value: number } | { not_computable: string };

/** One model's row, as either card's result holds it. */
export type FitRowView = Record<FitKey, FitCell>;

/** One model-fit table's column: AMOS's name, the measure, and whether it is a whole number
 *  shown with every digit (NPAR, DF, HOELTER). */
export interface FitColumn {
  label: string;
  key: FitKey;
  whole?: boolean;
}

export const FIT_ROWS = { model: 'Default model', saturated: 'Saturated model', independence: 'Independence model' } as const;
export type FitRow = keyof typeof FIT_ROWS;

/** The model-fit tables' first column, as AMOS heads it. */
export const FIT_MODEL_LABEL = 'Model';

/** AMOS's ten model-fit tables, in AMOS's order, each with its columns as AMOS names them.
 *  The saturated row is left out of RMSEA's and HOELTER's, as AMOS leaves it out. */
export const FIT_TABLES: ReadonlyArray<{ id: string; heading: string; columns: FitColumn[]; saturated: boolean }> = [
  {
    id: 'cmin',
    heading: 'CMIN',
    saturated: true,
    columns: [
      { label: 'NPAR', key: 'npar', whole: true },
      { label: 'CMIN', key: 'cmin' },
      { label: 'DF', key: 'df', whole: true },
      { label: 'P', key: 'p' },
      { label: 'CMIN/DF', key: 'cminDf' },
    ],
  },
  {
    id: 'rmr',
    heading: 'RMR, GFI',
    saturated: true,
    columns: [
      { label: 'RMR', key: 'rmr' },
      { label: 'GFI', key: 'gfi' },
      { label: 'AGFI', key: 'agfi' },
      { label: 'PGFI', key: 'pgfi' },
    ],
  },
  {
    id: 'baseline',
    heading: 'Baseline comparisons',
    saturated: true,
    columns: [
      { label: 'NFI Delta1', key: 'nfi' },
      { label: 'RFI rho1', key: 'rfi' },
      { label: 'IFI Delta2', key: 'ifi' },
      { label: 'TLI rho2', key: 'tli' },
      { label: 'CFI', key: 'cfi' },
    ],
  },
  {
    id: 'parsimony',
    heading: 'Parsimony-adjusted measures',
    saturated: true,
    columns: [
      { label: 'PRATIO', key: 'pratio' },
      { label: 'PNFI', key: 'pnfi' },
      { label: 'PCFI', key: 'pcfi' },
    ],
  },
  {
    id: 'ncp',
    heading: 'NCP',
    saturated: true,
    columns: [
      { label: 'NCP', key: 'ncp' },
      { label: 'LO 90', key: 'ncpLo90' },
      { label: 'HI 90', key: 'ncpHi90' },
    ],
  },
  {
    id: 'fmin',
    heading: 'FMIN',
    saturated: true,
    columns: [
      { label: 'FMIN', key: 'fmin' },
      { label: 'F0', key: 'f0' },
      { label: 'LO 90', key: 'f0Lo90' },
      { label: 'HI 90', key: 'f0Hi90' },
    ],
  },
  {
    id: 'rmsea',
    heading: 'RMSEA',
    saturated: false,
    columns: [
      { label: 'RMSEA', key: 'rmsea' },
      { label: 'LO 90', key: 'rmseaLo90' },
      { label: 'HI 90', key: 'rmseaHi90' },
      { label: 'PCLOSE', key: 'pclose' },
    ],
  },
  {
    id: 'aic',
    heading: 'AIC',
    saturated: true,
    columns: [
      { label: 'AIC', key: 'aic' },
      { label: 'BCC', key: 'bcc' },
      { label: 'BIC', key: 'bic' },
      { label: 'CAIC', key: 'caic' },
    ],
  },
  {
    id: 'ecvi',
    heading: 'ECVI',
    saturated: true,
    columns: [
      { label: 'ECVI', key: 'ecvi' },
      { label: 'LO 90', key: 'ecviLo90' },
      { label: 'HI 90', key: 'ecviHi90' },
      { label: 'MECVI', key: 'mecvi' },
    ],
  },
  {
    id: 'hoelter',
    heading: 'HOELTER',
    saturated: false,
    columns: [
      { label: 'HOELTER .05', key: 'hoelter05', whole: true },
      { label: 'HOELTER .01', key: 'hoelter01', whole: true },
    ],
  },
];

/** Under the CMIN table (decision 783). */
export const FIT_THREE_MODELS =
  'The default model is the one you specified. The saturated model has a parameter for every variance and covariance, so it fits them exactly. The independence model has only the columns\' variances, so it says the columns are uncorrelated.';

/** What a model-fit cell shows where it has no number. */
export const FIT_DASH = '–';
/** Under a model-fit table, once per code its cells hold: "–: " and the code's sentence. */
export const fitDashLine = (code: string, cellSentence: (code: string) => string): string => `${FIT_DASH}: ${cellSentence(code)}`;

/** A whole number with every digit (NPAR, DF, HOELTER, the iterations): shownNumber rounds
 *  to six figures, and HOELTER grows without bound as the chi-square nears 0. -0 is 0. */
export function fitWholeText(v: number): string {
  // String writes 1e21 and above with an exponent; BigInt writes the double's every digit (the
  // review of 9C-3, finding 7).
  if (Number.isInteger(v) && Math.abs(v) >= 1e21) return BigInt(v).toString();
  return Object.is(v, -0) ? '0' : String(v);
}

/** A model-fit cell: a number as the card shows it (a whole one with every digit), or "–". */
export function fitCellText(s: FitCell, whole: boolean): string {
  if (typeof s === 'number') return fitWholeText(s);
  if (!('value' in s)) return FIT_DASH;
  return whole ? fitWholeText(s.value) : shownNumber(s.value);
}

const head: React.CSSProperties = { textAlign: 'left', ...CELL_PADDING };
const table: React.CSSProperties = { borderCollapse: 'collapse' };

const ROW_ORDER: FitRow[] = ['model', 'saturated', 'independence'];

/** One of AMOS's ten model-fit tables, its rows' not-computable cells shown as "–", and under
 *  it one line per code those cells hold, in the order they first appear. */
export const FitTable: React.FC<{
  spec: (typeof FIT_TABLES)[number];
  fit: Record<FitRow, FitRowView>;
  prefix: string;
  cellSentence: (code: string) => string;
}> = ({ spec, fit, prefix, cellSentence }) => {
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
      <h4 style={{ margin: '8px 0 4px' }}>{spec.heading}</h4>
      <table data-testid={`${prefix}-fit-${spec.id}`} style={table}>
        <thead>
          <tr>
            <th scope="col" style={head}>
              {FIT_MODEL_LABEL}
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
            <tr key={r} data-testid={`${prefix}-fit-${spec.id}-${r}`}>
              <th scope="row" style={head}>
                {FIT_ROWS[r]}
              </th>
              {spec.columns.map((c) => {
                const s = fit[r][c.key];
                const value = typeof s === 'number' || 'value' in s;
                return (
                  <td key={c.key} className={value ? 'gds-mono' : undefined} style={CELL_PADDING}>
                    {fitCellText(s, c.whole === true)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
      </table>
      {codes.length > 0 && (
        <ul style={{ margin: '4px 0 0' }} data-testid={`${prefix}-fit-${spec.id}-codes`}>
          {codes.map((code) => (
            <li key={code}>{fitDashLine(code, cellSentence)}</li>
          ))}
        </ul>
      )}
      {spec.id === 'cmin' && (
        <p style={{ margin: '4px 0 0' }} data-testid={`${prefix}-three-models`}>
          {FIT_THREE_MODELS}
        </p>
      )}
    </section>
  );
};

/** The ten tables, in AMOS's order. */
export const ModelFitTables: React.FC<{ fit: Record<FitRow, FitRowView>; prefix: string; cellSentence: (code: string) => string }> = ({ fit, prefix, cellSentence }) => (
  <>
    {FIT_TABLES.map((t) => (
      <FitTable key={t.id} spec={t} fit={fit} prefix={prefix} cellSentence={cellSentence} />
    ))}
  </>
);
