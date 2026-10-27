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
