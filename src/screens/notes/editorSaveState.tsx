// Gaply — Note Creator: the shared save-state + unsaved-work guard.
//
// THE BUG THIS EXISTS TO FIX. Every editor used to print a constant
// "Saved locally · on device" next to a coloured dot — including on a note that
// had never been written to sqlite — while `onClose` discarded the whole draft
// with no prompt. Two honest-looking signals pointed the wrong way at once: the
// label said the work was safe, and the back arrow threw it away.
//
// So the two halves ship together and share one input, `dirty`:
//   • <SaveState> reports the REAL state — unsaved / saving / saved at a time —
//     and never claims "saved" for something that isn't in the database.
//   • useDirtyBaseline + <DiscardWarning> gate the close.
//
// Dirtiness is a STRING COMPARISON against a baseline snapshot captured at
// mount. Each editor builds its own snapshot from exactly the fields it can
// write, so "dirty" means "a save would change the stored row" — not "React
// re-rendered". Trimming inside the snapshot matches what save() persists, so
// typing a trailing space and deleting it again lands back at clean.
import React, { useState } from 'react';
import './notes.css';

/** Compare the live snapshot against a baseline captured at mount. Returns
 *  `dirty` plus an explicit `resetBaseline(next)` — it takes the next snapshot
 *  as an ARGUMENT rather than reading the render's `snapshot`, because callers
 *  reset it in the same event handler that sets the state producing it (the
 *  closed-over value is still the old one at that point). */
export function useDirtyBaseline(snapshot: string): {
  dirty: boolean;
  resetBaseline: (next: string) => void;
} {
  const [baseline, setBaseline] = useState(snapshot); // first render only
  return { dirty: snapshot !== baseline, resetBaseline: setBaseline };
}

export interface SaveStateProps {
  /** True when the live snapshot differs from the last saved/opened state. */
  dirty: boolean;
  /** True while a save is in flight. */
  busy?: boolean;
  /** The stored note's `updated_at` (epoch SECONDS), or null/undefined for a
   *  note that has never been saved. */
  savedAt?: number | null;
  testid?: string;
}

/** Honest local-storage status. Deliberately four states, not one string:
 *  a note with no row says so, and "saved" carries WHEN whenever it can.
 *
 *  TWO separate questions, deliberately not conflated. "Is there a stored row?"
 *  is `savedAt != null` — that alone decides saved vs. never-saved. "Can we name
 *  the moment?" is the plausible-epoch check the card date chip also applies; an
 *  implausible timestamp drops the date and keeps the truthful "Saved locally"
 *  rather than either fabricating 1 Jan 1970 or calling a stored note unsaved. */
export const SaveState: React.FC<SaveStateProps> = ({ dirty, busy, savedAt, testid = 'save-state' }) => {
  const stored = savedAt != null;
  const when = typeof savedAt === 'number' && savedAt > 1e9
    ? ` · ${new Date(savedAt * 1000).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' })}`
    : '';

  const [kind, text] = busy
    ? ['saving', 'Saving…']
    : dirty
      ? ['unsaved', 'Unsaved changes']
      : stored
        ? ['saved', `Saved locally${when}`]
        : ['new', 'Not saved yet'];

  return (
    <div
      className={`an-synced an-synced--${kind}`}
      data-testid={testid}
      data-state={kind}
      role="status"
    >
      {text}
    </div>
  );
};

export interface DiscardWarningProps {
  /** What the user would lose, e.g. "this note" / "this manuscript". */
  what?: string;
  onDiscard: () => void;
  onKeep: () => void;
  testid?: string;
}

/** The close gate. Same alertdialog idiom as the manuscript's structure-switch
 *  and section-delete warnings: keeping the work is the quiet default action,
 *  discarding is the explicitly-armed one. */
export const DiscardWarning: React.FC<DiscardWarningProps> = ({ what = 'this note', onDiscard, onKeep, testid = 'discard-warning' }) => (
  <div className="an-ms-dropwarn" role="alertdialog" aria-label="Unsaved changes" data-testid={testid}>
    <p><b>You have unsaved changes.</b> Closing {what} now discards everything you’ve written since it was last saved — this can’t be undone.</p>
    <div className="an-ms-dropwarn-actions">
      <button className="an-ghostbtn" data-testid={`${testid}-keep`} onClick={onKeep}>Keep editing</button>
      <button className="an-deletebtn an-deletebtn--armed" data-testid={`${testid}-discard`} onClick={onDiscard}>Discard changes</button>
    </div>
  </div>
);

/** Wire a guarded close: returns the handler the Back button calls, plus the
 *  warning's open state. `onClose` runs immediately when nothing is dirty. */
export function useCloseGuard(dirty: boolean, onClose: () => void): {
  askedToClose: boolean;
  requestClose: () => void;
  keepEditing: () => void;
  discard: () => void;
} {
  const [askedToClose, setAsked] = useState(false);
  return {
    askedToClose,
    requestClose: () => (dirty ? setAsked(true) : onClose()),
    keepEditing: () => setAsked(false),
    discard: () => { setAsked(false); onClose(); },
  };
}
