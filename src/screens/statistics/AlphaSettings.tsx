// Gaply — Cronbach's alpha's settings (Research OS, Phase 8R, decision 470;
// ADR-012 Part 6): the one copy, rendered by the old page (AlphaFlow.tsx) and by
// the Statistics window's dialog (window/AlphaDialog.tsx). None is made for the
// user: the numeric columns, scores included, as items (none ticked, at least
// two; "Add every numeric column" and "Remove all" are actions the user takes),
// a Reverse-score box beside each item (none ticked; removing an item unticks
// it), the scale's two ends when an item is reverse-scored (two empty fields,
// never filled in and never taken from the data), and each candidate missing
// code in the items, asked as a question.
//
// The spec lists the items and reversed items in the file's order and the codes
// ascending, and has no scale when nothing is reverse-scored, so one choice gives
// one spec (decision 338). What is held here is the user's choices, in this
// component's state only, and never browser storage.
import React, { useCallback, useMemo, useState } from 'react';
import { Button } from '../../design-system';
import type { DatasetView } from './describeTypes';
import type { AlphaSpec, ScaleEnds } from './alphaTypes';
import {
  alphaMissingCodeQuestion,
  HIGHEST_LABEL,
  ITEMS_QUESTION,
  LOWEST_LABEL,
  REVERSE_LABEL,
  REVERSE_SENTENCE,
  scaleEnd,
  settingsMissingSentence,
  type AlphaNames,
} from './alphaWording';
import { ALPHA_ADD_ALL, ALPHA_COLUMNS_HEADING, ALPHA_ITEMS_HEADING, ALPHA_ITEMS_NONE, ALPHA_REMOVE_ALL, ALPHA_REMOVE_ITEM, columnLabel } from './window/windowWording';

const aliasNumber = (alias: string) => Number(alias.replace(/^C/, ''));

const numeric = (view: DatasetView) => view.profile.columns.filter((c) => c.kind === 'numeric');

/** A scale field's own visible box. The app's global Tailwind preflight sets
 *  `border: 0` and `padding: 0` on every input, and nothing in the design
 *  system restores a text input's, so without this the field is an invisible
 *  box on the dark panel (real-app check, Step 10). Inline, where no
 *  stylesheet can remove it. */
const SCALE_FIELD_STYLE: React.CSSProperties = {
  borderWidth: 1,
  borderStyle: 'solid',
  borderColor: 'var(--g-border-strong)',
  borderRadius: 'var(--g-radius-button)',
  background: 'var(--g-bg-layer2)',
  color: 'var(--g-text)',
  padding: '4px 8px',
  width: '8em',
};

/** Decision 338: the items and the reversed items in the file's column order,
 *  the reversed items always among the items, the codes ascending and once
 *  each, and the scale only when an item is reverse-scored. */
export function buildAlphaSpec(
  view: DatasetView,
  items: string[],
  reversed: string[],
  scale: ScaleEnds | null,
  missingCodes: number[],
  operationId: number
): AlphaSpec {
  const codes = missingCodes.filter((c, i) => missingCodes.indexOf(c) === i).sort((a, b) => a - b);
  const inOrder = view.profile.columns.map((c) => c.alias).filter((a) => items.includes(a));
  const reverse = inOrder.filter((a) => reversed.includes(a));
  return {
    schemaVersion: 1,
    operationId,
    workspaceId: 'statistics',
    capabilityId: 'stats.analyze.cronbach_alpha',
    capabilityVersion: 1,
    inputs: [view.inputRef],
    parameters: {
      items: inOrder,
      reversed: reverse,
      scale: reverse.length > 0 ? scale : null,
      missingCodes: codes,
    },
    randomSeed: null,
    confirmationRefs: [],
  };
}

/** A column in a sentence, by the name the user sees. */
export function alphaNames(view: DatasetView | null): AlphaNames {
  return {
    column: (alias: number) => {
      const c = view?.profile.columns.find((col) => aliasNumber(col.alias) === alias);
      return c ? `"${columnLabel(c)}"` : `C${alias}`;
    },
  };
}

export interface AlphaSettingsState {
  items: string[];
  reversedItems: string[];
  lowest: string;
  highest: string;
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  /** What is still missing, in the method's words; null once it can be checked. */
  stillMissing: string | null;
  /** The spec these choices give. */
  spec: (operationId: number) => AlphaSpec;
  reset: () => void;
  setLowest: (text: string) => void;
  setHighest: (text: string) => void;
  toggleItem: (alias: string, on: boolean) => void;
  toggleReversed: (alias: string, on: boolean) => void;
  toggleCode: (code: number, on: boolean) => void;
  addAll: () => void;
  removeAll: () => void;
}

/** The user's choices, over the open file's view. */
export function useAlphaSettings(view: DatasetView | null): AlphaSettingsState {
  const [items, setItems] = useState<string[]>([]);
  const [reversed, setReversed] = useState<string[]>([]);
  const [lowest, setLowest] = useState('');
  const [highest, setHighest] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);

  /** The reversed items that are still items: unticking an item unticks
   *  its Reverse-score box. */
  const reversedItems = useMemo(() => reversed.filter((a) => items.includes(a)), [reversed, items]);

  /** The candidate codes of the ticked items, ascending, with where each
   *  appears (the union over the ticked items). */
  const offered = useMemo(() => {
    const where = new Map<number, Array<{ header: string; cells: number }>>();
    for (const c of view?.profile.columns ?? []) {
      if (!items.includes(c.alias)) continue;
      for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
    }
    return Array.from(where.keys())
      .sort((a, b) => a - b)
      .map((code) => ({ code, where: where.get(code) ?? [] }));
  }, [view, items]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)), [ticked, offered]);

  const spec = useCallback(
    (operationId: number) => {
      const lo = scaleEnd(lowest);
      const hi = scaleEnd(highest);
      const scale = lo !== null && hi !== null ? { min: lo, max: hi } : null;
      return buildAlphaSpec(view as DatasetView, items, reversedItems, scale, missingCodes, operationId);
    },
    [view, items, reversedItems, lowest, highest, missingCodes]
  );

  const reset = useCallback(() => {
    setItems([]);
    setReversed([]);
    setLowest('');
    setHighest('');
    setTicked([]);
  }, []);

  /** The reversed items after a change: when none is left, the scale's ends
   *  go with them, so they are empty when the fields next appear. */
  const setReversedTo = (next: string[]) => {
    setReversed(next);
    if (next.length === 0) {
      setLowest('');
      setHighest('');
    }
  };

  const toggleItem = (alias: string, on: boolean) => {
    setItems((t) => (on ? [...t.filter((y) => y !== alias), alias] : t.filter((y) => y !== alias)));
    if (!on) setReversedTo(reversedItems.filter((a) => a !== alias));
  };

  const toggleReversed = (alias: string, on: boolean) =>
    setReversedTo(on ? [...reversedItems.filter((y) => y !== alias), alias] : reversedItems.filter((y) => y !== alias));

  const toggleCode = (code: number, on: boolean) =>
    setTicked((t) => (on ? [...t.filter((y) => y !== code), code] : t.filter((y) => y !== code)));

  return {
    items,
    reversedItems,
    lowest,
    highest,
    ticked,
    offered,
    stillMissing: settingsMissingSentence(items.length, reversedItems.length, lowest, highest),
    spec,
    reset,
    setLowest,
    setHighest,
    toggleItem,
    toggleReversed,
    toggleCode,
    addAll: () => setItems(numeric(view as DatasetView).map((c) => c.alias)),
    removeAll: () => {
      setItems([]);
      setReversedTo([]);
    },
  };
}

/** The controls, the page's and the dialog's alike. */
export const AlphaSettings: React.FC<{ view: DatasetView; settings: AlphaSettingsState }> = ({ view, settings: s }) => {
  const columns = numeric(view);
  const chosen = columns.filter((c) => s.items.includes(c.alias));
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="al-settings">
      <fieldset data-testid="al-item-choices">
        <legend>{ITEMS_QUESTION}</legend>
        <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
          <Button variant="secondary" onClick={s.addAll} data-testid="al-tick-all">
            {ALPHA_ADD_ALL}
          </Button>
          <Button variant="secondary" onClick={s.removeAll} data-testid="al-untick-all">
            {ALPHA_REMOVE_ALL}
          </Button>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
          <div data-testid="al-columns">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_COLUMNS_HEADING}</h4>
            {columns.map((c) => (
              <label key={c.alias} style={{ display: 'block' }}>
                <input
                  type="checkbox"
                  checked={s.items.includes(c.alias)}
                  data-testid={`al-item-${c.alias}`}
                  onChange={(e) => s.toggleItem(c.alias, e.target.checked)}
                />{' '}
                {c.alias} <bdi>{columnLabel(c)}</bdi>
              </label>
            ))}
          </div>
          <div data-testid="al-items-list">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_ITEMS_HEADING}</h4>
            {chosen.length === 0 && (
              <p style={{ margin: 0 }} data-testid="al-items-none">
                {ALPHA_ITEMS_NONE}
              </p>
            )}
            {chosen.map((c) => (
              <div key={c.alias} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }} data-testid={`al-chosen-${c.alias}`}>
                <span>
                  {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>
                <label>
                  <input
                    type="checkbox"
                    checked={s.reversedItems.includes(c.alias)}
                    data-testid={`al-reverse-${c.alias}`}
                    onChange={(e) => s.toggleReversed(c.alias, e.target.checked)}
                  />{' '}
                  {REVERSE_LABEL}
                </label>
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} ${c.alias}`} onClick={() => s.toggleItem(c.alias, false)} data-testid={`al-remove-${c.alias}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
              </div>
            ))}
          </div>
        </div>
      </fieldset>
      {s.reversedItems.length > 0 && (
        <fieldset data-testid="al-scale">
          <legend>The scale&apos;s ends</legend>
          <p id="al-reverse-sentence" className="gds-jc__disclaimer" data-testid="al-reverse-sentence">
            {REVERSE_SENTENCE}
          </p>
          <label style={{ display: 'block' }}>
            <span>{LOWEST_LABEL}</span>{' '}
            <input
              type="text"
              inputMode="decimal"
              value={s.lowest}
              aria-describedby="al-reverse-sentence"
              data-testid="al-scale-min"
              style={SCALE_FIELD_STYLE}
              onChange={(e) => s.setLowest(e.target.value)}
            />
          </label>
          <label style={{ display: 'block' }}>
            <span>{HIGHEST_LABEL}</span>{' '}
            <input
              type="text"
              inputMode="decimal"
              value={s.highest}
              aria-describedby="al-reverse-sentence"
              data-testid="al-scale-max"
              style={SCALE_FIELD_STYLE}
              onChange={(e) => s.setHighest(e.target.value)}
            />
          </label>
        </fieldset>
      )}
      {s.offered.length > 0 && (
        <fieldset data-testid="al-missing-codes">
          <legend>Candidate missing codes</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input
                type="checkbox"
                checked={s.ticked.includes(o.code)}
                data-testid={`al-code-${o.code}`}
                onChange={(e) => s.toggleCode(o.code, e.target.checked)}
              />{' '}
              {alphaMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
      {s.stillMissing && (
        <p role="status" data-testid="al-settings-missing">
          {s.stillMissing}
        </p>
      )}
    </div>
  );
};
