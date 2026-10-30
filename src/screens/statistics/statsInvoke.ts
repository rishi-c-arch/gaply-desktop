// Gaply — the one way a Statistics method's bridge calls Rust, and the one
// mock its tests record calls with (Research OS, Phase 8D-2b, decision 268),
// moved here from the independent t-test's bridge (ttestBridge.ts), not
// copied. The webview passes a path once (to open), slots, run ids and the
// user's choices; a mock records a path as `<path>` and never keeps it.
import { isTauri } from '../../utils/isTauri';

/** One Tauri command, in the desktop app; a refusal everywhere else. */
export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isTauri) throw new Error('The Statistics methods run in the Gaply desktop app.');
  const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
  return tauriInvoke<T>(cmd, args);
}

/** This computer's limits for the methods, as stats_desc_limits answers
 *  them, unchecked: the screen shows a line only for an answer with
 *  MethodLimitsView's shape (Research OS, Phase 8F-2, decision 289). */
export function readMethodLimits(): Promise<unknown> {
  return invoke<unknown>('stats_desc_limits');
}

/** How long an open runs before its rows read are first asked for, and how
 *  often after (decision 290). */
export const PROGRESS_POLL_MS = 250;

/** One counter for every screen's opens, so no two opens share an id. */
let lastProgressId = 0;

/** Opens `path` through stats_desc_open with a progress id of its own and,
 *  while the open runs, from PROGRESS_POLL_MS on, asks stats_desc_open_progress
 *  every PROGRESS_POLL_MS for its rows read, giving each count, never a null,
 *  to `onRowsRead` (decision 290). Nothing is asked, and nothing passed on,
 *  once the open has returned or been refused. A count only: the path goes to
 *  the open once and is not kept. */
export async function openWithProgress<T>(path: string, onRowsRead?: (rowsRead: number) => void): Promise<T> {
  lastProgressId += 1;
  const progressId = lastProgressId;
  let settled = false;
  const timer = onRowsRead
    ? setInterval(() => {
        invoke<{ rowsRead?: unknown } | null>('stats_desc_open_progress', { progressId }).then(
          (p) => {
            if (!settled && p && typeof p.rowsRead === 'number') onRowsRead(p.rowsRead);
          },
          () => undefined
        );
      }, PROGRESS_POLL_MS)
    : undefined;
  try {
    return await invoke<T>('stats_desc_open', { path, progressId });
  } finally {
    settled = true;
    if (timer !== undefined) clearInterval(timer);
  }
}

/** A canned response, or a function of the call's arguments (a sequence of
 *  status polls, a refusal). */
export type MockResponse = unknown | ((args: Record<string, unknown>) => unknown);

/** Records every call and its arguments, and answers with what `responses`
 *  holds for the command, or refuses a command it was not given. */
export function recorder(responses: Partial<Record<string, MockResponse>>) {
  const calls: Array<[string, Record<string, unknown>]> = [];
  const call = <T>(cmd: string, args: Record<string, unknown> = {}, recorded = args): Promise<T> => {
    calls.push([cmd, recorded]);
    if (!(cmd in responses)) return Promise.reject(new Error(`no mock response for ${cmd}`));
    const r = responses[cmd];
    try {
      return Promise.resolve((typeof r === 'function' ? (r as (a: Record<string, unknown>) => unknown)(args) : r) as T);
    } catch (e) {
      return Promise.reject(e);
    }
  };
  return { calls, call };
}
