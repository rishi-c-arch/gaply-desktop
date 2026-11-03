// Gaply — the Statistics window's Open and Close (Research OS, Phase 8P,
// decision 450): today's fileActions and useOpenProgress over the session's
// slot, never copied. A path goes to the open once and is not kept. The old
// slot closes only after the new one has opened, so a cancelled picker or a
// refused open changes nothing. A file that finishes opening after the session
// was cleared is not this session's: it is closed, not kept.
import { useRef, useState } from 'react';
import { refusalSentence } from '../describeWording';
import type { DatasetView } from '../describeTypes';
import { actWith, fileActions, useOpenProgress } from '../flowHooks';
import { sessionSlot, statisticsSession } from './statisticsSession';

export interface WindowFileOptions {
  bridge: {
    open(path: string, onRowsRead?: (rowsRead: number) => void): Promise<DatasetView>;
    close(slot: number): Promise<unknown>;
  };
  /** The native file picker: an absolute path, or null when cancelled. */
  pickPath: () => Promise<string | null>;
}

export interface WindowFile {
  choose: () => Promise<void>;
  closeFile: () => Promise<void>;
  /** The open's rows read, for the progress line. */
  rowsRead: number | null;
  /** The last refusal's sentence. */
  error: string | null;
}

export function useWindowFile({ bridge, pickPath }: WindowFileOptions): WindowFile {
  const [error, setError] = useState<string | null>(null);
  const progress = useOpenProgress();
  const startedIn = useRef(0);
  const act = actWith(statisticsSession.setBusy, setError, refusalSentence);
  const { choose, closeFile } = fileActions<DatasetView>({
    bridge,
    slot: sessionSlot,
    track: progress.track,
    act,
    pickPath: () => {
      startedIn.current = statisticsSession.generation();
      return pickPath();
    },
    onOpened: (opened) => {
      if (statisticsSession.generation() !== startedIn.current) {
        void bridge.close(opened.slot).catch(() => undefined);
        return;
      }
      statisticsSession.setFile(opened);
    },
    onClosed: () => statisticsSession.clearFile(),
  });
  return { choose, closeFile, rowsRead: progress.rowsRead, error };
}
