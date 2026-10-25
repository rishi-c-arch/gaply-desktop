// Gaply — the Stored data section of Settings (Research OS, Phases 5D-1a and
// 5D-1b; decisions 206, 207, 209, 213 and 215): what Gaply's older features
// keep on this computer, one line per category, with its name, its sentence,
// how many items and how much space, and a Delete… on each category that
// holds at least one item.
//
// Shown only when storedDataShown() says so; otherwise it draws nothing and
// calls nothing. It shows the two counts readStoredData keeps and nothing
// else of the answer, and one fixed sentence when the summary fails.
//
// * Delete… opens a dialog titled "Delete {name}?", with the category's count
//   and size, what stops, and decision 215's sentence about backups. Cancel
//   has the focus, Escape cancels. Its Delete calls prepare delete, then
//   delete, on that click only.
// * A refusal shows its code's sentence and the dialog stays open; Cancel then
//   reads Close and takes the focus (decision 175). Close and Escape return
//   the focus to the category's Delete….
// * After a delete: "Deleted: {count}." (or, after categories 1 to 5 when the
//   compaction did not finish, its own sentence), the section read again, and
//   the focus on the Stored data heading.
// * Under the section, always, decision 209's note of what it does not list.
// * One generation counter covers the reads and the deletes: a late answer
//   never overwrites a newer one. ProjectsPanel.tsx is the pattern; it is
//   unchanged.
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Button, Card } from '../../design-system';
import ProjectDialog from '../statistics/projects/ProjectDialog';
import { mb } from '../statistics/projects/projectWording';
import { useGeneration, type Generation } from '../statistics/projects/useGeneration';
import {
  DELETE_PARTIAL,
  readConfirmation,
  readDeleted,
  readStoredData,
  StoredCount,
  StoredDataBridge,
  StoredDataDeleter,
  StoredDeleted,
  tauriStoredDataBridge,
  tauriStoredDataDeleter,
} from './storedDataBridge';
import { storedDataShown } from './storedDataGate';
import {
  countWords,
  DELETION_TEXT,
  deleteSentence,
  refusalCode,
  STORED_CATEGORIES,
  STORED_DATA_TEXT,
  StoredCategoryId,
  StoredCategoryWords,
  WHAT_STOPS,
} from './storedDataWording';

export interface StoredDataSectionProps {
  /** Test seams; the app passes nothing. */
  bridge?: StoredDataBridge;
  deleter?: StoredDataDeleter;
  desktop?: boolean;
}

type View = { kind: 'reading' } | { kind: 'failed' } | { kind: 'read'; counts: Map<StoredCategoryId, StoredCount> };

/** The line the section shows after a delete. */
function deletedLine(c: StoredCategoryWords, d: StoredDeleted): string {
  const count = DELETION_TEXT.deleted(countWords(d.deleted, c));
  if (d.outcome === 'done') return count;
  // Decision 224: the not-done line keeps the count, then its sentence,
  // chosen by the code (decision 230): a partial delete says what is left,
  // then the not-done sentence too if its checkpoint did not finish.
  switch (d.code) {
    case DELETE_PARTIAL:
      return d.checkpoint === 'done'
        ? `${count} ${DELETION_TEXT.partlyRemoved}`
        : `${count} ${DELETION_TEXT.partlyRemoved} ${DELETION_TEXT.notCompacted}`;
    case 'stored_data.checkpoint_busy':
    case 'stored_data.checkpoint_failed':
      return `${count} ${DELETION_TEXT.notCompacted}`;
    case 'stored_data.remove_failed':
      return `${count} ${DELETION_TEXT.partlyRemoved}`;
    default:
      return `${count} ${DELETION_TEXT.failed}`;
  }
}

const countLine = (c: StoredCategoryWords, n: StoredCount) => `${countWords(n.items, c)}. Size: ${mb(n.bytes)}.`;

export const StoredDataSection: React.FC<StoredDataSectionProps> = (props) =>
  storedDataShown(props.desktop) ? <StoredDataShown {...props} /> : null;

const StoredDataShown: React.FC<StoredDataSectionProps> = ({ bridge = tauriStoredDataBridge, deleter = tauriStoredDataDeleter }) => {
  const gen = useGeneration();
  const [view, setView] = useState<View>({ kind: 'reading' });
  const [target, setTarget] = useState<StoredCategoryWords | null>(null);
  const [message, setMessage] = useState('');
  const heading = useRef<HTMLHeadingElement>(null);
  // The Delete… that opened the dialog, recorded from its click.
  const opener = useRef<HTMLElement | null>(null);

  const read = useCallback(async () => {
    const g = gen.next();
    try {
      const counts = readStoredData(await bridge.summary());
      if (gen.is(g)) setView(counts ? { kind: 'read', counts } : { kind: 'failed' });
    } catch {
      if (gen.is(g)) setView({ kind: 'failed' });
    }
  }, [bridge, gen]);

  useEffect(() => {
    void read();
  }, [read]);

  const deleted = useCallback(
    (c: StoredCategoryWords, d: StoredDeleted) => {
      setTarget(null);
      setMessage(deletedLine(c, d));
      void read();
    },
    [read]
  );

  return (
    <Card>
      <h3 ref={heading} tabIndex={-1} className="gds-card__title" data-testid="stored-data-heading">
        {STORED_DATA_TEXT.heading}
      </h3>
      <div data-testid="stored-data">
        <p role="status" aria-live="polite" data-testid="stored-data-message" style={{ margin: 0 }}>
          {message}
        </p>
        {view.kind === 'reading' && <p className="gds-set__muted">{STORED_DATA_TEXT.reading}</p>}
        {view.kind === 'failed' && (
          <p className="gds-set__muted" data-testid="stored-data-failed">
            {STORED_DATA_TEXT.failed}
          </p>
        )}
        {view.kind === 'read' && (
          <ul className="gds-set__stored">
            {STORED_CATEGORIES.map((c) => {
              const n = view.counts.get(c.id) ?? { items: 0, bytes: 0 };
              return (
                <li key={c.id} className="gds-set__stored-line" data-testid={`stored-line-${c.id}`}>
                  <span className="gds-set__stored-name">{c.name}</span>
                  <span className="gds-set__muted">{c.sentence}</span>
                  <span className="gds-set__stored-count" data-testid={`stored-count-${c.id}`}>
                    {countLine(c, n)}
                  </span>
                  {n.items > 0 && (
                    <span>
                      <Button
                        variant="secondary"
                        onClick={(e) => {
                          opener.current = e.currentTarget;
                          setMessage('');
                          setTarget(c);
                        }}
                        data-testid={`stored-delete-${c.id}`}
                      >
                        {DELETION_TEXT.open}
                      </Button>
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
        <p className="gds-set__muted" data-testid="stored-data-note">
          {STORED_DATA_TEXT.note}
        </p>
      </div>
      {target && view.kind === 'read' && (
        <DeleteDialog
          category={target}
          count={view.counts.get(target.id) ?? { items: 0, bytes: 0 }}
          deleter={deleter}
          gen={gen}
          headingAfter={() => heading.current}
          openerAfter={() => opener.current}
          onCancel={() => setTarget(null)}
          onDeleted={(d) => deleted(target, d)}
          onFailed={() => void read()}
        />
      )}
    </Card>
  );
};

interface DeleteDialogProps {
  category: StoredCategoryWords;
  count: StoredCount;
  deleter: StoredDataDeleter;
  /** The section's counter: the dialog is part of the section's screen. */
  gen: Generation;
  headingAfter: () => HTMLElement | null;
  openerAfter: () => HTMLElement | null;
  onCancel: () => void;
  onDeleted: (d: StoredDeleted) => void;
  /** A failure, not a refusal: the section reads what is left. */
  onFailed: () => void;
}

const DeleteDialog: React.FC<DeleteDialogProps> = ({
  category,
  count,
  deleter,
  gen,
  headingAfter,
  openerAfter,
  onCancel,
  onDeleted,
  onFailed,
}) => {
  const [pending, setPending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  // F9 (decision 226): what the dialog shows, and the message when the last
  // Delete found the count changed. `shown` starts at the section's count.
  const [shown, setShown] = useState<StoredCount>(count);
  const [changed, setChanged] = useState<string | null>(null);
  const alive = useRef(true);
  const inFlight = useRef(false);
  const cancelButton = useRef<HTMLButtonElement>(null);
  // Focus goes back to the Delete… that opened it, unless the category was
  // deleted: then to the section's heading.
  const done = useRef(false);
  const returnFocus = useRef<(() => HTMLElement | null) | null>(() => (done.current ? headingAfter() : openerAfter()));
  const title = useMemo(() => DELETION_TEXT.title(category.name), [category.name]);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  // After a refusal the dialog stays open so it can be read, and its Cancel
  // reads Close and takes the focus once it is enabled again (decision 175).
  useEffect(() => {
    if (refusal !== null && !pending) cancelButton.current?.focus();
  }, [refusal, pending]);

  const cancel = () => {
    if (!pending) onCancel();
  };

  const remove = async () => {
    if (inFlight.current) return;
    inFlight.current = true;
    const g = gen.next();
    setPending(true);
    setRefusal(null);
    try {
      const prepared = readConfirmation(await deleter.prepareDelete(category.id));
      if (!alive.current || !gen.is(g)) return;
      if (prepared === null) throw new Error('no confirmation');
      // F9 (decision 226): delete only what the dialog shows. If the count
      // changed since it was shown, delete nothing, show the new count and
      // ask again; the next Delete asks prepare_delete afresh.
      if (prepared.items !== shown.items || prepared.bytes !== shown.bytes) {
        setShown({ items: prepared.items, bytes: prepared.bytes });
        setChanged(DELETION_TEXT.changed);
        return;
      }
      setChanged(null);
      const d = readDeleted(await deleter.delete(category.id, prepared.confirmation), category.id);
      if (!alive.current || !gen.is(g)) return;
      if (d === null) throw new Error('an answer of another shape');
      done.current = true;
      onDeleted(d);
    } catch (e) {
      if (!alive.current || !gen.is(g)) return;
      setRefusal(deleteSentence(e));
      if (refusalCode(e) === null) onFailed();
    } finally {
      inFlight.current = false;
      if (alive.current) setPending(false);
    }
  };

  return (
    <ProjectDialog title={title} onClose={cancel} initialFocus={cancelButton} returnFocus={returnFocus} testId="stored-delete-dialog">
      <p data-testid="stored-delete-count">{countLine(category, shown)}</p>
      <p data-testid="stored-delete-stops">{WHAT_STOPS[category.id]}</p>
      <p data-testid="stored-delete-sentence">{DELETION_TEXT.backups}</p>
      {changed && (
        <p role="alert" data-testid="stored-delete-changed">
          {changed}
        </p>
      )}
      {refusal && (
        <p role="alert" data-testid="stored-delete-refusal">
          {refusal}
        </p>
      )}
      <p role="status" aria-live="polite" data-testid="stored-delete-pending" style={{ margin: '4px 0' }}>
        {pending ? DELETION_TEXT.deleting : ''}
      </p>
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
        <button
          ref={cancelButton}
          type="button"
          className="gds-btn gds-btn--secondary"
          onClick={cancel}
          disabled={pending}
          data-testid="stored-delete-cancel"
        >
          {refusal !== null ? DELETION_TEXT.close : DELETION_TEXT.cancel}
        </button>
        <Button onClick={() => void remove()} disabled={pending} data-testid="stored-delete-confirm">
          {DELETION_TEXT.confirm}
        </Button>
      </div>
    </ProjectDialog>
  );
};
