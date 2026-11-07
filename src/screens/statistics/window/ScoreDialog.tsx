// Gaply — Transform, Scale score… (Research OS, Phase 8T-2a, decisions 505 and
// 507; ADR-012 Part 8 and question 8). The score's form, the make, and deleting
// a score or every score, in one dialog on the design system's ProjectDialog. It
// is not a MethodDialog (decision 510): a make validates and builds in one call,
// so there is no second press, and it adds no Output entry.
//
// The recipe sent is the scores the slot already holds, read when the dialog
// opens on a prepared view, then this one as the next column. An answer is taken
// only while the session's generation and slot are what they were when the act
// began; otherwise it is dropped. A refused make keeps the dialog and the form.
//
// It owns no file and closes nothing: it gets the slot's number, and the only
// bridge it is given is the prep bridge, which has no close.
import React, { useEffect, useRef, useState } from 'react';
import { Button } from '../../../design-system';
import { ProjectDialog } from '../../../design-system/ProjectDialog';
import { refusalOf } from '../../statsverifier/l1/l1Wording';
import type { DatasetView } from '../describeTypes';
import { actWith } from '../flowHooks';
import { DiagnosticList } from '../flowParts';
import type { PrepBridge } from '../prepBridge';
import type { PrepMadeView, PrepRecipeView, Recipe, ScaleScoreStep } from '../prepTypes';
import { prepRefusalSentence, prepSentence } from '../prepWording';
import { AskFirst } from './MethodDialog';
import './MethodDialog.css';
import { ScoreSettings, useScoreSettings } from './ScoreSettings';
import { statisticsSession } from './statisticsSession';
import { SCALE_SCORE_CAPABILITY } from './windowMenus';
import {
  ASK_DELETE_ALL,
  ASK_DELETE_SCORE,
  DIALOG_CANCEL,
  SCORE_DELETE,
  SCORE_DELETE_ALL,
  scoreDeleteLabel,
  scoreDeletedSentence,
  SCORE_DIALOG_TITLE,
  SCORE_HELD_READING,
  SCORE_MAKE,
  scoreMadeSentence,
  SCORES_DELETED,
  SCORES_HEADING,
} from './windowWording';

export interface ScoreDialogProps {
  view: DatasetView;
  slot: number;
  prep: PrepBridge;
  /** An act ended and the dialog closes: the sentence for sw-message, or null
   *  when the slot was found gone. */
  onDone: (message: string | null) => void;
  onCancel: () => void;
  returnFocus?: React.RefObject<(() => HTMLElement | null) | null>;
}

/** A recipe of `steps`, its versions the literal types prepTypes.ts's Recipe gives. */
function recipeOf(sourceColumns: number, steps: ScaleScoreStep[]): Recipe {
  return { schemaVersion: 1, capabilityId: SCALE_SCORE_CAPABILITY, capabilityVersion: 1, engineVersion: 1, sourceColumns, steps };
}

export const ScoreDialog: React.FC<ScoreDialogProps> = ({ view, slot, prep, onDone, onCancel, returnFocus }) => {
  const settings = useScoreSettings(view);
  const nameField = useRef<HTMLInputElement>(null);
  const prepared = view.preparation !== undefined;
  // undefined while the held recipe is being read; null when the slot holds none.
  const [held, setHeld] = useState<PrepRecipeView | null | undefined>(prepared ? undefined : null);
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [report, setReport] = useState<{ key: string; made: PrepMadeView; recipe: Recipe } | null>(null);
  const [asking, setAsking] = useState<number | 'all' | null>(null);
  const inFlight = useRef(false);
  const done = useRef(onDone);
  done.current = onDone;

  const heldSteps = held?.recipe.steps ?? [];
  const sourceColumns = held?.recipe.sourceColumns ?? view.profile.columns.length;
  const alias = `C${view.profile.columns.length + 1}`;
  const settingsKey = JSON.stringify(settings.draft(alias));

  /** What the session is now: an answer is this dialog's only while it holds. */
  const mark = () => {
    const generation = statisticsSession.generation();
    return () => statisticsSession.generation() === generation && statisticsSession.get().slot === slot;
  };

  /** run.unknown_input: the slot is gone, as the grid says it (gridCells.ts),
   *  under the same check; the dialog closes. */
  const slotWent = (e: unknown, still: () => boolean): boolean => {
    if (refusalOf(e)?.code !== 'run.unknown_input') return false;
    if (still()) {
      statisticsSession.slotGone();
      done.current(null);
    }
    return true;
  };

  const take = (dataset: DatasetView, still: () => boolean, message: string | null) => {
    if (!still()) return;
    statisticsSession.setFile(dataset);
    done.current(message);
  };

  // The scores the slot holds, read once when the dialog opens on a prepared view.
  useEffect(() => {
    if (!prepared) return undefined;
    let open = true;
    const still = mark();
    prep.recipe(slot).then(
      (r) => {
        if (open) setHeld(r);
      },
      (e) => {
        if (!open || slotWent(e, still)) return;
        setRefusal(prepRefusalSentence(e, view));
      }
    );
    return () => {
      open = false;
    };
    // Read when it opens: the dialog is mounted for one view and one slot.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const act = actWith(statisticsSession.setBusy, setRefusal, (e) => prepRefusalSentence(e, view));
  const go = async (f: () => Promise<void>) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setPending(true);
    try {
      await act(f);
    } finally {
      inFlight.current = false;
      setPending(false);
    }
  };

  const make = () =>
    go(async () => {
      const step = settings.step(alias);
      if (step === null || held === undefined) return;
      const recipe = recipeOf(sourceColumns, [...heldSteps, step]);
      const still = mark();
      let made: PrepMadeView;
      try {
        made = await prep.make(slot, recipe);
      } catch (e) {
        if (slotWent(e, still)) return;
        throw e;
      }
      if (made.dataset) {
        const counts = made.counts.find((c) => c.column === step.alias);
        take(made.dataset, still, counts ? scoreMadeSentence(step.name, counts) : null);
      } else if (still()) setReport({ key: settingsKey, made, recipe });
    });

  /** Deleting one: the held recipe less that step, the later aliases
   *  renumbered; the only one, or every one, is a clear. */
  const deleteScores = (which: number | 'all') =>
    go(async () => {
      const still = mark();
      try {
        if (which === 'all' || heldSteps.length <= 1) {
          const cleared = await prep.clear(slot);
          take(cleared, still, which === 'all' ? SCORES_DELETED : scoreDeletedSentence(heldSteps[0]?.name ?? ''));
          return;
        }
        const rest = heldSteps.filter((_, j) => j !== which).map((s, j) => ({ ...s, alias: `C${sourceColumns + j + 1}` }));
        const recipe = recipeOf(sourceColumns, rest);
        const made = await prep.make(slot, recipe);
        if (made.dataset) take(made.dataset, still, scoreDeletedSentence(heldSteps[which].name));
        else if (still()) setReport({ key: settingsKey, made, recipe });
      } catch (e) {
        if (slotWent(e, still)) return;
        throw e;
      }
    });

  const cancel = () => {
    if (!pending) onCancel();
  };

  const shown = report !== null && report.key === settingsKey ? report : null;
  const said = (d: { code: string; params?: Record<string, unknown> }) => prepSentence(d, view, shown?.recipe, shown?.made.counts);
  const sumNote = settings.sumOverFewer
    ? prepSentence({ code: 'stats.prep.sum_over_fewer_items', params: { index: heldSteps.length } }, view, recipeOf(sourceColumns, [...heldSteps, settings.draft(alias)]))
    : null;
  const asked = asking === null ? null : asking === 'all' ? ASK_DELETE_ALL : { title: ASK_DELETE_SCORE.title, sentence: ASK_DELETE_SCORE.sentence(heldSteps[asking]?.name ?? '') };

  return (
    <>
      <ProjectDialog title={SCORE_DIALOG_TITLE} onClose={cancel} initialFocus={nameField} returnFocus={returnFocus} testId="sw-score-dialog">
        <div className="sw-dialog__scroll" data-testid="sc-dialog-scroll" tabIndex={0}>
          {held === undefined && refusal === null && (
            <p role="status" data-testid="sc-held-reading">
              {SCORE_HELD_READING}
            </p>
          )}
          {heldSteps.length > 0 && (
            <section data-testid="sc-scores" style={{ marginBottom: 8 }}>
              <h3 style={{ font: 'inherit', fontWeight: 600, margin: '4px 0' }}>{SCORES_HEADING}</h3>
              <ul style={{ margin: 0 }}>
                {heldSteps.map((s, i) => (
                  <li key={s.alias} style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}>
                    <span>
                      {s.alias} <bdi>{s.name}</bdi>
                    </span>
                    <Button variant="ghost" aria-label={scoreDeleteLabel(s.name)} disabled={pending} onClick={() => setAsking(i)} data-testid={`sc-delete-${s.alias}`}>
                      {SCORE_DELETE}
                    </Button>
                  </li>
                ))}
              </ul>
              <Button variant="secondary" disabled={pending} onClick={() => setAsking('all')} data-testid="sc-delete-all">
                {SCORE_DELETE_ALL}
              </Button>
            </section>
          )}
          <ScoreSettings view={view} settings={settings} nameField={nameField} />
          {sumNote && (
            <p className="gds-jc__disclaimer" data-testid="sc-sum-note">
              {sumNote}
            </p>
          )}
          {settings.stillMissing && (
            <p role="status" data-testid="sc-settings-missing">
              {settings.stillMissing}
            </p>
          )}
          {shown && (
            <div className="sw-dialog__report">
              <DiagnosticList items={shown.made.errors} sentence={said} testId="sc-errors" />
              <DiagnosticList items={shown.made.missingInformation} sentence={said} testId="sc-questions" />
            </div>
          )}
          {refusal && (
            <p role="alert" data-testid="sc-dialog-refusal">
              {refusal}
            </p>
          )}
        </div>
        <div className="sw-dialog__actions">
          <Button variant="secondary" onClick={cancel} disabled={pending} data-testid="sc-dialog-cancel">
            {DIALOG_CANCEL}
          </Button>
          <Button onClick={() => void make()} disabled={pending || held === undefined || settings.stillMissing !== null} data-testid="sc-make">
            {SCORE_MAKE}
          </Button>
        </div>
      </ProjectDialog>
      {asking !== null && asked && (
        <AskFirst
          title={asked.title}
          sentence={asked.sentence}
          testId="sc-ask"
          onCancel={() => setAsking(null)}
          onContinue={() => {
            const which = asking;
            setAsking(null);
            void deleteScores(which);
          }}
        />
      )}
    </>
  );
};
