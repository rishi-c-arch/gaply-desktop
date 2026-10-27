// Gaply — the behaviour more than one method's screen shares (Research OS,
// Phase 8D-2b, decision 268), moved here from the independent t-test's
// screen (TTestFlow.tsx) one piece at a time, not copied. Each takes what is
// its method's own (its bridge, its sentences) as arguments.
//
// None of it keeps, logs or sends anything: a file's slot lives in the
// screen's ref, a refusal becomes the method's own sentence, and an export
// goes only to the file the user names in the save dialog.
import { useEffect, useRef, type MutableRefObject, type RefObject } from 'react';
import { shortHash } from './describeWording';
import type { ExportView } from './describeTypes';

/** Focus moves to each stage's heading when the stage changes; the heading
 *  takes the returned ref. */
export function useStageFocus<S>(stage: S): RefObject<HTMLHeadingElement | null> {
  const heading = useRef<HTMLHeadingElement>(null);
  const shownStage = useRef<S>(stage);
  useEffect(() => {
    if (shownStage.current === stage) return;
    shownStage.current = stage;
    heading.current?.focus();
  }, [stage]);
  return heading;
}

/** The file is closed, and its rows dropped in Rust, when the screen goes. */
export function useCloseOnUnmount(
  bridge: { close(slot: number): Promise<unknown> },
  slot: MutableRefObject<number | null>
): void {
  useEffect(
    () => () => {
      if (slot.current !== null) void bridge.close(slot.current).catch(() => undefined);
    },
    [bridge, slot]
  );
}

/** While a run is unfinished, its state is read every `pollMs` through the
 *  method's own status command; a refusal is shown in the method's words. */
export function useRunPolling<V extends { runId: number; terminal: boolean }>(
  runView: V | null,
  setRunView: (v: V) => void,
  bridge: { status(runId: number): Promise<V> },
  setError: (sentence: string) => void,
  refusalSentence: (e: unknown) => string,
  pollMs: number
): void {
  useEffect(() => {
    if (!runView || runView.terminal) return;
    let live = true;
    const t = setTimeout(() => {
      bridge
        .status(runView.runId)
        .then((s) => live && setRunView(s))
        .catch((e) => live && setError(refusalSentence(e)));
    }, pollMs);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [runView, setRunView, bridge, setError, refusalSentence, pollMs]);
}

/** An action of the screen: busy while it runs, its refusal shown in the
 *  method's own words. */
export function actWith(
  setBusy: (busy: boolean) => void,
  setError: (sentence: string | null) => void,
  refusalSentence: (e: unknown) => string
): (f: () => Promise<void>) => Promise<void> {
  return async (f) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(refusalSentence(e));
    } finally {
      setBusy(false);
    }
  };
}

/** Choosing a data file and closing it: the path goes to `open` once and is
 *  never kept; the file open before is closed, so Rust drops its rows. What
 *  the screen does after each is its own. */
export function fileActions<V extends { slot: number }>(o: {
  bridge: { open(path: string): Promise<V>; close(slot: number): Promise<unknown> };
  slot: MutableRefObject<number | null>;
  pickPath: () => Promise<string | null>;
  act: (f: () => Promise<void>) => Promise<void>;
  onOpened: (opened: V) => void;
  onClosed: () => void;
}): { choose: () => Promise<void>; closeFile: () => Promise<void> } {
  const { bridge, slot, pickPath, act, onOpened, onClosed } = o;
  return {
    choose: () =>
      act(async () => {
        const path = await pickPath();
        if (!path) return;
        const opened = await bridge.open(path);
        if (slot.current !== null) void bridge.close(slot.current).catch(() => undefined);
        slot.current = opened.slot;
        onOpened(opened);
      }),
    closeFile: () =>
      act(async () => {
        if (slot.current !== null) await bridge.close(slot.current);
        slot.current = null;
        onClosed();
      }),
  };
}

/** An export saved exactly as Rust made it: the bytes are decoded, checked
 *  to encode back to the same bytes, and written through the save dialog to
 *  the one file the user names. What to say after: a note, or why it was not
 *  saved. */
export async function saveExactly(
  out: ExportView,
  choice: { file: string; mime: string },
  save: (name: string, text: string, mime: string) => Promise<string | null>
): Promise<{ note: string } | { error: string }> {
  const bytes = Uint8Array.from(out.bytes);
  const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
  // What is written must be the bytes Rust returned, and their hash.
  const again = new TextEncoder().encode(text);
  if (again.length !== bytes.length || again.some((b, i) => b !== bytes[i])) {
    return { error: 'The export could not be saved exactly as Gaply made it, so it was not saved.' };
  }
  const where = await save(choice.file, text, choice.mime);
  return { note: where ? `Saved. SHA-256 of the file: ${shortHash(out.sha256)}` : 'Not saved: the save dialog was closed.' };
}
