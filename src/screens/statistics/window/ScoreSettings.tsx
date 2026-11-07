// Gaply — the scale score's form (Research OS, Phase 8T-2a, decision 504;
// ADR-012 Part 8's items 1 to 8, ADR-011 Part 10). Nothing is filled in: the
// name is empty, no item is chosen, no box is ticked, the scale's ends and the
// number of items a person needs are empty, and neither the sum nor the mean nor
// a rule is chosen. Its items are the file's own numeric columns only (ADR-011
// question 13), so a score is never offered.
//
// What is held here is the user's choices, in this component's state only, and
// never browser storage. Every rule here only decides whether Make can be
// pressed; Rust validates the recipe, and its refusal is the boundary.
import React, { useMemo, useState } from 'react';
import { Button } from '../../../design-system';
import { SCALE_FIELD_STYLE } from '../AlphaSettings';
import { alphaMissingCodeQuestion, HIGHEST_LABEL, LOWEST_LABEL, REVERSE_LABEL, REVERSE_SENTENCE } from '../alphaWording';
import type { ColumnProfile, DatasetView } from '../describeTypes';
import { SCORE_NAME_MAX_CHARS, typedNumber, type ScaleScoreStep, type Statistic } from '../prepTypes';
import {
  ALPHA_ADD_ALL,
  ALPHA_COLUMNS_HEADING,
  ALPHA_ITEMS_HEADING,
  ALPHA_ITEMS_NONE,
  ALPHA_REMOVE_ALL,
  ALPHA_REMOVE_ITEM,
  columnLabel,
  SCORE_AT_LEAST_LABEL,
  SCORE_CODES_LEGEND,
  SCORE_ITEMS_LEGEND,
  SCORE_MISSING,
  scoreMissingSentence,
  SCORE_NAME_LABEL,
  SCORE_RULE_AT_LEAST,
  SCORE_RULE_EVERY,
  SCORE_RULE_LEGEND,
  SCORE_SCALE_LEGEND,
  SCORE_STATISTIC_CHOICES,
  SCORE_STATISTIC_LEGEND,
} from './windowWording';

/** The columns a score may take: the file's own numeric columns, never a score. */
export function scoreItemColumns(view: DatasetView): ColumnProfile[] {
  return view.profile.columns.filter((c) => c.kind === 'numeric' && c.prepared !== true);
}

/** The name as it is sent: trimmed (decision 408). */
const trimmed = (name: string) => name.trim();

/** A typed number of items a person needs: a whole number from 1 to the
 *  number of items less one, else null. */
function atLeastOf(text: string, items: number): number | null {
  const v = typedNumber(text);
  return v !== null && Number.isInteger(v) && v >= 1 && v <= items - 1 ? v : null;
}

type Rule = 'every_item' | 'at_least' | null;

export interface ScoreSettingsState {
  name: string;
  items: string[];
  reversedItems: string[];
  lowest: string;
  highest: string;
  statistic: Statistic | null;
  rule: Rule;
  atLeast: string;
  ticked: number[];
  offered: Array<{ code: number; where: Array<{ header: string; cells: number }> }>;
  /** What is still missing, in one sentence; null once Make can be pressed. */
  stillMissing: string | null;
  /** The sum's sentence is due: a sum, and "at least n". */
  sumOverFewer: boolean;
  /** The step these choices give, as `alias`; null until every piece is ready. */
  step: (alias: string) => ScaleScoreStep | null;
  /** The step as typed so far, for the sum's sentence before Make. */
  draft: (alias: string) => ScaleScoreStep;
  setName: (text: string) => void;
  setLowest: (text: string) => void;
  setHighest: (text: string) => void;
  setStatistic: (s: Statistic) => void;
  setRule: (r: Exclude<Rule, null>) => void;
  setAtLeast: (text: string) => void;
  toggleItem: (alias: string, on: boolean) => void;
  toggleReversed: (alias: string, on: boolean) => void;
  toggleCode: (code: number, on: boolean) => void;
  addAll: () => void;
  removeAll: () => void;
}

/** The user's choices, over the open file's view. */
export function useScoreSettings(view: DatasetView): ScoreSettingsState {
  const [name, setName] = useState('');
  const [items, setItems] = useState<string[]>([]);
  const [reversed, setReversed] = useState<string[]>([]);
  const [lowest, setLowest] = useState('');
  const [highest, setHighest] = useState('');
  const [statistic, setStatistic] = useState<Statistic | null>(null);
  const [rule, setRule] = useState<Rule>(null);
  const [atLeast, setAtLeast] = useState('');
  const [ticked, setTicked] = useState<number[]>([]);

  const columns = useMemo(() => scoreItemColumns(view), [view]);
  /** The items and the reversed items in the file's column order. */
  const inOrder = useMemo(() => columns.map((c) => c.alias).filter((a) => items.includes(a)), [columns, items]);
  const reversedItems = useMemo(() => inOrder.filter((a) => reversed.includes(a)), [inOrder, reversed]);

  /** The chosen items' candidate codes, ascending, with where each appears. */
  const offered = useMemo(() => {
    const where = new Map<number, Array<{ header: string; cells: number }>>();
    for (const c of columns) {
      if (!items.includes(c.alias)) continue;
      for (const s of c.suspectMissingCodes) where.set(s.code, [...(where.get(s.code) ?? []), { header: columnLabel(c), cells: s.cells }]);
    }
    return Array.from(where.keys())
      .sort((a, b) => a - b)
      .map((code) => ({ code, where: where.get(code) ?? [] }));
  }, [columns, items]);
  const missingCodes = useMemo(() => ticked.filter((t) => offered.some((s) => s.code === t)).sort((a, b) => a - b), [ticked, offered]);

  const lo = typedNumber(lowest);
  const hi = typedNumber(highest);
  const n = atLeastOf(atLeast, inOrder.length);

  const parts: string[] = [];
  const sent = trimmed(name);
  if (sent === '') parts.push(SCORE_MISSING.name);
  else if (Array.from(sent).length > SCORE_NAME_MAX_CHARS) parts.push(SCORE_MISSING.nameLong);
  if (inOrder.length < 2) parts.push(inOrder.length === 1 ? SCORE_MISSING.oneMoreItem : SCORE_MISSING.twoItems);
  if (reversedItems.length > 0) {
    if (lo === null) parts.push(SCORE_MISSING.lowest);
    if (hi === null) parts.push(SCORE_MISSING.highest);
  }
  if (statistic === null) parts.push(SCORE_MISSING.statistic);
  if (rule === null) parts.push(SCORE_MISSING.rule);
  // With fewer than two items, the items' piece already says what is needed.
  else if (rule === 'at_least' && inOrder.length >= 2 && n === null) parts.push(SCORE_MISSING.atLeast(inOrder.length - 1));
  const stillMissing = scoreMissingSentence(parts);

  const draft = (alias: string): ScaleScoreStep => ({
    kind: 'scale_score',
    alias,
    name: sent,
    items: inOrder,
    reversed: reversedItems,
    scale: reversedItems.length > 0 && lo !== null && hi !== null ? { min: lo, max: hi } : null,
    statistic,
    rule: rule === 'every_item' ? 'every_item' : rule === 'at_least' && n !== null ? { at_least: n } : null,
    missingCodes,
  });

  /** The reversed items after a change: when none is left, the scale's ends go
   *  with them, so they are empty when the fields next appear. */
  const setReversedTo = (next: string[]) => {
    setReversed(next);
    if (next.length === 0) {
      setLowest('');
      setHighest('');
    }
  };

  return {
    name,
    items: inOrder,
    reversedItems,
    lowest,
    highest,
    statistic,
    rule,
    atLeast,
    ticked: missingCodes,
    offered,
    stillMissing,
    sumOverFewer: statistic === 'sum' && rule === 'at_least',
    step: (alias) => (stillMissing === null ? draft(alias) : null),
    draft,
    setName,
    setLowest,
    setHighest,
    setStatistic,
    setRule,
    setAtLeast,
    toggleItem: (alias, on) => {
      setItems((t) => (on ? [...t.filter((y) => y !== alias), alias] : t.filter((y) => y !== alias)));
      if (on) return;
      setReversedTo(reversedItems.filter((a) => a !== alias));
      // A ticked code stays only while an item still offers it.
      const still = new Set(columns.filter((c) => c.alias !== alias && inOrder.includes(c.alias)).flatMap((c) => c.suspectMissingCodes.map((x) => x.code)));
      setTicked((t) => t.filter((code) => still.has(code)));
    },
    toggleReversed: (alias, on) => setReversedTo(on ? [...reversedItems.filter((y) => y !== alias), alias] : reversedItems.filter((y) => y !== alias)),
    toggleCode: (code, on) => setTicked((t) => (on ? [...t.filter((y) => y !== code), code] : t.filter((y) => y !== code))),
    addAll: () => setItems(columns.map((c) => c.alias)),
    removeAll: () => {
      setItems([]);
      setReversedTo([]);
      setTicked([]);
    },
  };
}

/** The form's controls. `nameField` takes the focus when the dialog opens. */
export const ScoreSettings: React.FC<{ view: DatasetView; settings: ScoreSettingsState; nameField: React.RefObject<HTMLInputElement | null> }> = ({ view, settings: s, nameField }) => {
  const columns = scoreItemColumns(view);
  const chosen = columns.filter((c) => s.items.includes(c.alias));
  return (
    <div style={{ display: 'grid', gap: 8 }} data-testid="sc-settings">
      <label style={{ display: 'block' }}>
        <span>{SCORE_NAME_LABEL}</span>{' '}
        <input
          ref={nameField as React.RefObject<HTMLInputElement>}
          type="text"
          value={s.name}
          data-testid="sc-name"
          style={{ ...SCALE_FIELD_STYLE, width: '24em', maxWidth: '100%' }}
          onChange={(e) => s.setName(e.target.value)}
        />
      </label>
      <fieldset data-testid="sc-item-choices">
        <legend>{SCORE_ITEMS_LEGEND}</legend>
        <div style={{ display: 'flex', gap: 8, marginBottom: 4 }}>
          <Button variant="secondary" onClick={s.addAll} data-testid="sc-tick-all">
            {ALPHA_ADD_ALL}
          </Button>
          <Button variant="secondary" onClick={s.removeAll} data-testid="sc-untick-all">
            {ALPHA_REMOVE_ALL}
          </Button>
        </div>
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)', gap: 16 }}>
          <div data-testid="sc-columns">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_COLUMNS_HEADING}</h4>
            {columns.map((c) => (
              <label key={c.alias} style={{ display: 'block' }}>
                <input type="checkbox" checked={s.items.includes(c.alias)} data-testid={`sc-item-${c.alias}`} onChange={(e) => s.toggleItem(c.alias, e.target.checked)} />{' '}
                {c.alias} <bdi>{columnLabel(c)}</bdi>
              </label>
            ))}
          </div>
          <div data-testid="sc-items-list">
            <h4 style={{ margin: '4px 0' }}>{ALPHA_ITEMS_HEADING}</h4>
            {chosen.length === 0 && (
              <p style={{ margin: 0 }} data-testid="sc-items-none">
                {ALPHA_ITEMS_NONE}
              </p>
            )}
            {chosen.map((c) => (
              <div key={c.alias} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }} data-testid={`sc-chosen-${c.alias}`}>
                <span>
                  {c.alias} <bdi>{columnLabel(c)}</bdi>
                </span>
                <label>
                  <input type="checkbox" checked={s.reversedItems.includes(c.alias)} data-testid={`sc-reverse-${c.alias}`} onChange={(e) => s.toggleReversed(c.alias, e.target.checked)} />{' '}
                  {REVERSE_LABEL}
                </label>
                <Button variant="ghost" aria-label={`${ALPHA_REMOVE_ITEM} ${c.alias}`} onClick={() => s.toggleItem(c.alias, false)} data-testid={`sc-remove-${c.alias}`}>
                  {ALPHA_REMOVE_ITEM}
                </Button>
              </div>
            ))}
          </div>
        </div>
      </fieldset>
      {s.reversedItems.length > 0 && (
        <fieldset data-testid="sc-scale">
          <legend>{SCORE_SCALE_LEGEND}</legend>
          <p id="sc-reverse-sentence" className="gds-jc__disclaimer" data-testid="sc-reverse-sentence">
            {REVERSE_SENTENCE}
          </p>
          <label style={{ display: 'block' }}>
            <span>{LOWEST_LABEL}</span>{' '}
            <input type="text" inputMode="decimal" value={s.lowest} aria-describedby="sc-reverse-sentence" data-testid="sc-scale-min" style={SCALE_FIELD_STYLE} onChange={(e) => s.setLowest(e.target.value)} />
          </label>
          <label style={{ display: 'block' }}>
            <span>{HIGHEST_LABEL}</span>{' '}
            <input type="text" inputMode="decimal" value={s.highest} aria-describedby="sc-reverse-sentence" data-testid="sc-scale-max" style={SCALE_FIELD_STYLE} onChange={(e) => s.setHighest(e.target.value)} />
          </label>
        </fieldset>
      )}
      <fieldset data-testid="sc-statistic">
        <legend>{SCORE_STATISTIC_LEGEND}</legend>
        {SCORE_STATISTIC_CHOICES.map((c) => (
          <label key={c.statistic} style={{ display: 'block' }}>
            <input type="radio" name="sc-statistic" checked={s.statistic === c.statistic} data-testid={`sc-statistic-${c.statistic}`} onChange={() => s.setStatistic(c.statistic)} /> {c.label}
          </label>
        ))}
      </fieldset>
      <fieldset data-testid="sc-rule">
        <legend>{SCORE_RULE_LEGEND}</legend>
        <label style={{ display: 'block' }}>
          <input type="radio" name="sc-rule" checked={s.rule === 'every_item'} data-testid="sc-rule-every" onChange={() => s.setRule('every_item')} /> {SCORE_RULE_EVERY}
        </label>
        <label style={{ display: 'block' }}>
          <input type="radio" name="sc-rule" checked={s.rule === 'at_least'} data-testid="sc-rule-at-least" onChange={() => s.setRule('at_least')} /> {SCORE_RULE_AT_LEAST}
        </label>
        {s.rule === 'at_least' && (
          <input
            type="text"
            inputMode="numeric"
            aria-label={SCORE_AT_LEAST_LABEL}
            value={s.atLeast}
            data-testid="sc-at-least"
            style={{ ...SCALE_FIELD_STYLE, marginLeft: 24 }}
            onChange={(e) => s.setAtLeast(e.target.value)}
          />
        )}
      </fieldset>
      {s.offered.length > 0 && (
        <fieldset data-testid="sc-missing-codes">
          <legend>{SCORE_CODES_LEGEND}</legend>
          {s.offered.map((o) => (
            <label key={o.code} style={{ display: 'block' }}>
              <input type="checkbox" checked={s.ticked.includes(o.code)} data-testid={`sc-code-${o.code}`} onChange={(e) => s.toggleCode(o.code, e.target.checked)} /> {alphaMissingCodeQuestion(o.code, o.where)}
            </label>
          ))}
        </fieldset>
      )}
    </div>
  );
};
