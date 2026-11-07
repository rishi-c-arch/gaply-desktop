// Gaply — scale scores' bridge (Research OS, Phase 8T-1b, decision 499).
// Calls the thin Rust commands in src-tauri/src/stats_prep_commands.rs: make,
// clear, recipe and csv, on a slot the window already holds. Rust owns every
// decision: the webview passes a slot and the recipe the user built, and shows
// what comes back. The CSV comes back as bytes; the save dialog writes the one
// file the user names, and Rust writes nothing. No screen uses this yet: the
// scale score's dialog is 8T-2's.
import { isTauri } from '../../utils/isTauri';
import { invoke, recorder, type MockResponse } from './statsInvoke';
import type { DatasetView, ExportView } from './describeTypes';
import type { PrepMadeView, PrepRecipeView, Recipe } from './prepTypes';

export interface PrepBridge {
  /** Makes or replaces the slot's recipe: every score held, then the new one
   *  (stats_prep_make). A refused recipe is a view with no dataset, not an
   *  error. */
  make(slot: number, recipe: Recipe): Promise<PrepMadeView>;
  /** Takes every score off, the slot back on the file's table
   *  (stats_prep_clear). */
  clear(slot: number): Promise<DatasetView>;
  /** The recipe the slot holds and its counts, or null for none
   *  (stats_prep_recipe). */
  recipe(slot: number): Promise<PrepRecipeView | null>;
  /** The prepared data's CSV bytes and their SHA-256 (stats_prep_csv). */
  csv(slot: number): Promise<ExportView>;
}

export class TauriPrepBridge implements PrepBridge {
  make(slot: number, recipe: Recipe) {
    return invoke<PrepMadeView>('stats_prep_make', { slot, recipe });
  }
  clear(slot: number) {
    return invoke<DatasetView>('stats_prep_clear', { slot });
  }
  recipe(slot: number) {
    return invoke<PrepRecipeView | null>('stats_prep_recipe', { slot });
  }
  csv(slot: number) {
    return invoke<ExportView>('stats_prep_csv', { slot });
  }
}

/** Test/dev double: records every call and its arguments, and returns what
 *  `responses` holds for the command, or refuses a command it was not given. */
export function makeMockPrepBridge(
  responses: Partial<Record<string, MockResponse>> = {}
): PrepBridge & { calls: Array<[string, Record<string, unknown>]> } {
  const { calls, call } = recorder(responses);
  return {
    calls,
    make: (slot, recipe) => call('stats_prep_make', { slot, recipe }),
    clear: (slot) => call('stats_prep_clear', { slot }),
    recipe: (slot) => call('stats_prep_recipe', { slot }),
    csv: (slot) => call('stats_prep_csv', { slot }),
  };
}

/** The bridge a screen uses: Tauri's in the desktop app; otherwise a mock that
 *  answers nothing, so a browser build shows a refusal, never a score. */
export function prepBridge(): PrepBridge {
  return isTauri ? new TauriPrepBridge() : makeMockPrepBridge();
}
