// Gaply — the Output's outline (Research OS, Phase 8R, decision 471; ADR-012 Part
// 7): a numbered list, newest last, each line the method's name. A click moves the
// result pane to it. Each line has Remove, disabled while its run is going.
import React from 'react';
import { Button } from '../../../design-system';
import { METHODS } from '../methods';
import { runIsGoing, type OutputEntry } from './statisticsSession';
import { OUTPUT_LABEL, OUTPUT_REMOVE, RUN_GOING } from './windowWording';

export function methodName(kind: OutputEntry['kind']): string {
  return METHODS.find((m) => m.kind === kind)?.name ?? kind;
}

export const OutputOutline: React.FC<{
  entries: readonly OutputEntry[];
  selected: number | null;
  onGo: (number: number) => void;
  onRemove: (number: number) => void;
}> = ({ entries, selected, onGo, onRemove }) => (
  <nav className="sw-outline" aria-label={OUTPUT_LABEL} data-testid="sw-outline">
    <ol className="sw-outline__list">
      {entries.map((e) => (
        <li key={e.number} className="sw-outline__line">
          <button
            type="button"
            className="sw-outline__go"
            aria-current={e.number === selected ? 'true' : undefined}
            onClick={() => onGo(e.number)}
            data-testid={`sw-outline-go-${e.number}`}
          >
            {e.number} {methodName(e.kind)}
          </button>
          <Button
            variant="ghost"
            aria-label={`${OUTPUT_REMOVE} ${e.number}`}
            title={runIsGoing(e) ? RUN_GOING : undefined}
            disabled={runIsGoing(e)}
            onClick={() => onRemove(e.number)}
            data-testid={`sw-outline-remove-${e.number}`}
          >
            {OUTPUT_REMOVE}
          </Button>
        </li>
      ))}
    </ol>
  </nav>
);
