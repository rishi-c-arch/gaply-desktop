// Phase 4b — the shared text-file save path. Pins the CONTRACT both lanes rely
// on (the cancel-vs-failure distinction fixed earlier in notes, and the
// citations exporter's proven path): cancel → null, real write failure → throws,
// success → the chosen path. Tauri path exercised via mocked plugins.
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('./isTauri', () => ({ isTauri: true })); // force the desktop path
const save = vi.fn();
const writeTextFile = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({ save: (...a: unknown[]) => save(...a) }));
vi.mock('@tauri-apps/plugin-fs', () => ({ writeTextFile: (...a: unknown[]) => writeTextFile(...a) }));

import { saveTextFile } from './saveTextFile';

beforeEach(() => { save.mockReset(); writeTextFile.mockReset(); });

describe('saveTextFile contract (shared by notes + citations)', () => {
  it('CANCEL (dialog returns null) → returns null, never writes', async () => {
    save.mockResolvedValue(null);
    expect(await saveTextFile('notes.md', 'body', 'text/markdown')).toBeNull();
    expect(writeTextFile).not.toHaveBeenCalled(); // silent no-op, no write attempted
  });

  it('SUCCESS → writes the exact text to the chosen path and returns it', async () => {
    save.mockResolvedValue('/Users/x/notes.md');
    writeTextFile.mockResolvedValue(undefined);
    const path = await saveTextFile('notes.md', 'the body', 'text/markdown');
    expect(path).toBe('/Users/x/notes.md');
    expect(writeTextFile).toHaveBeenCalledWith('/Users/x/notes.md', 'the body');
  });

  it('a genuine write FAILURE propagates (throws) — distinct from cancel', async () => {
    save.mockResolvedValue('/Users/x/out.bib');
    writeTextFile.mockRejectedValue(new Error('disk full'));
    await expect(saveTextFile('library.bib', 'refs', 'text/plain')).rejects.toThrow(/disk full/);
  });
});
