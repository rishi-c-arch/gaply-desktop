// Gaply — the Variable View (Research OS, Phase 8P, decision 452; ADR-012 Part
// 4): one row per column of the DatasetView's profile, read-only. A name is the
// user's own and is shown as text inside <bdi>, on their own screen only. The
// profile table's levels are not shown here.
import React from 'react';
import { CELL_PADDING } from '../../../design-system/primitives';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { FileFindings } from '../flowParts';
import { columnLabel, FROM_A_RECIPE, FROM_THE_FILE, missingCodeWords, nonFiniteSentence, scoreCountsSentence, VARIABLE_HEADERS, VARIABLE_NOTE } from './windowWording';


const kindName = (c: ColumnProfile): string => (c.kind === 'numeric' ? 'Numeric' : c.kind === 'categorical' ? 'Categorical' : 'Empty');

export const VariableView: React.FC<{ view: DatasetView }> = ({ view }) => {
  const columns = view.profile.columns;
  const label = (alias: string) => {
    const c = columns.find((x) => x.alias === alias);
    return c ? `${alias} ${columnLabel(c)}` : alias;
  };
  return (
    <div data-testid="sw-variable-view">
      <table className="sw-vars" data-testid="sw-vars">
        <thead>
          <tr>
            {VARIABLE_HEADERS.map((h) => (
              <th key={h} scope="col" style={CELL_PADDING}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {columns.map((c) => (
            <tr key={c.alias} data-testid={`sw-var-${c.alias}`}>
              <td className="gds-mono" style={CELL_PADDING}>{c.alias.replace(/^C/, '')}</td>
              <td className="gds-mono" style={CELL_PADDING}>{c.alias}</td>
              <td style={CELL_PADDING}>
                <bdi data-testid={`sw-var-name-${c.alias}`}>{columnLabel(c)}</bdi>
              </td>
              <td style={CELL_PADDING}>{kindName(c)}</td>
              <td className="gds-mono" style={CELL_PADDING}>{c.distinct}</td>
              <td className="gds-mono" style={CELL_PADDING}>{c.missing}</td>
              <td className="gds-mono" style={CELL_PADDING}>{c.suspectMissingCodes.map((s) => missingCodeWords(s.code, s.cells)).join(', ')}</td>
              <td style={CELL_PADDING}>{c.prepared === true ? FROM_A_RECIPE : FROM_THE_FILE}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="sw-notes">
        <p data-testid="sw-vars-note">{VARIABLE_NOTE}</p>
        <FileFindings prefix="sw" view={view} />
        {(view.scoreCounts ?? []).map((s) => (
          <p key={s.column} data-testid={`sw-score-counts-${s.column}`}>
            <bdi>{scoreCountsSentence(label(s.column), s)}</bdi>
          </p>
        ))}
        {columns
          .filter((c) => c.nonFinite > 0)
          .map((c) => (
            <p key={c.alias} data-testid={`sw-non-finite-${c.alias}`}>
              <bdi>{nonFiniteSentence(label(c.alias), c.nonFinite)}</bdi>
            </p>
          ))}
      </div>
    </div>
  );
};
