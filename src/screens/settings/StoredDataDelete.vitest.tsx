// The Stored data section's deletes (Research OS, Phase 5D-1b; decisions
// 213, 215 and 216): a Delete… only on a category with items; the dialog's
// words for each category; one prepare and one delete per confirmed click,
// and nothing on Cancel or Escape; a refusal's sentence, with Close taking
// the focus; a failure's sentence and the section read again; after a delete
// "Deleted: {count}.", the section read again and the focus on its heading;
// the not-done sentence after categories 1 to 5 only; the note, always; no
// title, name, path or URL anywhere; nothing in the browser build; and the
// Settings outline's two items, each behind its gate.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { StoredDataSection } from './StoredDataSection';
import { StoredDataBridge, StoredDataDeleter, tauriStoredDataDeleter } from './storedDataBridge';
import {
  DATABASE_CATEGORIES,
  DELETION_TEXT,
  REFUSAL_WORDS,
  STORED_CATEGORIES,
  STORED_DATA_TEXT,
  StoredCategoryId,
  WHAT_STOPS,
} from './storedDataWording';
import SettingsPage from './SettingsPage';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';

const env = vi.hoisted(() => ({ storedData: true, projects: false, workspaces: false, desktop: true }));
vi.mock('../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) =>
    (name === 'storedData' && env.storedData) ||
    (name === 'projects' && env.projects) ||
    (name === 'workspaces' && env.workspaces),
}));
vi.mock('../../utils/isTauri', () => ({
  get isTauri() {
    return env.desktop;
  },
  get default() {
    return env.desktop;
  },
}));
vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

/** Tauri's invoke: every call recorded; the summary answered with nothing in
 *  it, everything else left pending. */
const tauri = vi.hoisted(() => ({ calls: [] as Array<[string, unknown]> }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => {
    tauri.calls.push([cmd, args]);
    return cmd === 'stored_data_summary' ? Promise.resolve({ categories: [] }) : new Promise(() => {});
  },
}));

const SENTINELS = ['SENTINEL_TITLE_5d1b', '/Users/SENTINEL_PATH_5d1b', 'https://SENTINEL_URL_5d1b', 'SENTINEL_NAME_5d1b'];
const CONFIRMATION = 'c'.repeat(64);

type Counts = Partial<Record<StoredCategoryId, [number, number]>>;

/** A summary with every category, each carrying fields a view must never show. */
function summary(over: Counts = {}) {
  return {
    categories: STORED_CATEGORIES.map((c) => {
      const [items, bytes] = over[c.id] ?? [3, 1_000];
      return { category: c.id, items, bytes, title: SENTINELS[0], path: SENTINELS[1], url: SENTINELS[2] };
    }),
    name: SENTINELS[3],
  };
}

/** A summary bridge answering each read with the next of `answers` (the
 *  last one again once they run out). */
function bridgeOf(...answers: unknown[]): StoredDataBridge & { calls: number } {
  const b = {
    calls: 0,
    summary() {
      const a = answers[Math.min(b.calls, answers.length - 1)];
      b.calls += 1;
      return Promise.resolve(a);
    },
  };
  return b;
}

type Deleted = { deleted: number; outcome: 'done' | 'not_done'; code?: string; checkpoint?: string };

/** A deleter recording each call, answering the delete as told. */
function deleterOf(answer: (category: StoredCategoryId) => Promise<unknown>) {
  const calls: Array<[string, ...unknown[]]> = [];
  const d: StoredDataDeleter = {
    prepareDelete(category) {
      calls.push(['prepare', category]);
      return Promise.resolve({
        confirmation: CONFIRMATION,
        category,
        items: 3,
        bytes: 1_000,
        expiresInMs: 300_000,
        title: SENTINELS[0],
        path: SENTINELS[1],
      });
    },
    delete(category, confirmation) {
      calls.push(['delete', category, confirmation]);
      return answer(category);
    },
  };
  return { d, calls };
}

const done = (deleted: Deleted) => (category: StoredCategoryId) =>
  Promise.resolve({
    category,
    before: { category, items: 3, bytes: 1_000 },
    after: { category, items: 0, bytes: 0 },
    ...deleted,
    url: SENTINELS[2],
  });

/** Each dialog's title, written out (decisions 215 and 219). */
const TITLES: Record<StoredCategoryId, string> = {
  manuscript_analyses: 'Delete Manuscript analyses?',
  gap_finder_papers: 'Delete Gap Finder papers?',
  thesis_audits: 'Delete Thesis audits?',
  cached_reports: 'Delete Cached reports?',
  expired_cache: 'Delete Expired cache rows?',
  comparison_log: 'Delete the comparison log?',
  open_access_papers: 'Delete Downloaded open-access papers?',
  temporary_exports: 'Delete Temporary exports?',
  orphan_note_images: 'Delete Unused note images?',
};

const refused = (code: string) => () => Promise.reject(new Error(JSON.stringify({ code })));

beforeEach(() => {
  env.storedData = true;
  env.projects = false;
  env.workspaces = false;
  env.desktop = true;
  tauri.calls = [];
});
afterEach(cleanup);

async function openDialog(id: StoredCategoryId) {
  fireEvent.click(await screen.findByTestId(`stored-delete-${id}`));
  return screen.findByTestId('stored-delete-dialog');
}

describe('the Delete… buttons', () => {
  it('only on a category with at least one item', async () => {
    const counts: Counts = { thesis_audits: [0, 0], comparison_log: [0, 0], orphan_note_images: [1, 60] };
    render(<StoredDataSection bridge={bridgeOf(summary(counts))} deleter={deleterOf(done({ deleted: 1, outcome: 'done' })).d} desktop />);
    await screen.findAllByTestId(/^stored-line-/);
    for (const c of STORED_CATEGORIES) {
      const button = screen.queryByTestId(`stored-delete-${c.id}`);
      const empty = c.id === 'thesis_audits' || c.id === 'comparison_log';
      expect(button === null, c.id).toBe(empty);
      if (button) expect(button.className).toContain('gds-btn--secondary');
    }
  });
});

describe('the dialog', () => {
  it.each(STORED_CATEGORIES.map((c) => [c.id, c] as const))(
    "%s: its title, count and size, what stops and the sentence about backups; Cancel has the focus",
    async (id, c) => {
      const { d, calls } = deleterOf(done({ deleted: 3, outcome: 'done' }));
      render(<StoredDataSection bridge={bridgeOf(summary({ [id]: [2, 2_500_000] }))} deleter={d} desktop />);
      const dialog = await openDialog(id);
      expect(within(dialog).getByRole('heading').textContent).toBe(TITLES[id]);
      expect(dialog.getAttribute('aria-labelledby')).toBeTruthy();
      expect(within(dialog).getByTestId('stored-delete-count').textContent).toBe(
        `2 ${c.many}. Size: 2.50 MB.`
      );
      expect(within(dialog).getByTestId('stored-delete-stops').textContent).toBe(WHAT_STOPS[id]);
      expect(within(dialog).getByTestId('stored-delete-sentence').textContent).toBe(
        "Deleting removes these from this app's folder. It cannot remove copies in Time Machine or other backups, in file-system snapshots, or anything you exported. The disk may keep traces until the space is reused."
      );
      expect(within(dialog).getByTestId('stored-delete-cancel').textContent).toBe('Cancel');
      expect(within(dialog).getByTestId('stored-delete-confirm').textContent).toBe('Delete');
      expect(document.activeElement).toBe(within(dialog).getByTestId('stored-delete-cancel'));
      expect(calls).toEqual([]);
    }
  );

  it('a leading "The" is lowercased in the title, and nothing else changes (decision 219)', () => {
    expect(DELETION_TEXT.title('The comparison log')).toBe('Delete the comparison log?');
    expect(DELETION_TEXT.title('Thesis audits')).toBe('Delete Thesis audits?');
    expect(DELETION_TEXT.title('Theme')).toBe('Delete Theme?');
    expect(STORED_CATEGORIES.map((c) => DELETION_TEXT.title(c.name))).toEqual(Object.values(TITLES));
  });

  it('the nine sentences of what stops are decision 215\'s', () => {
    expect(WHAT_STOPS).toEqual({
      manuscript_analyses:
        'Nothing on screen reads these now. Cached reports of these manuscripts are listed and deleted on their own.',
      gap_finder_papers:
        'The Research Gap Finder, the full analysis and search will no longer find these papers. You can add them again.',
      thesis_audits:
        'Past audits will no longer be listed, resumed or exported. Their evidence cards and fetched sources stay, as the note below says.',
      cached_reports: 'To see one of these reports again, run it again.',
      expired_cache: 'Nothing reads these any more.',
      comparison_log: 'Nothing in the app reads it. A new log starts with the next PublishReady run.',
      open_access_papers:
        "Gaply's viewer will no longer open these copies. Their text, already read for citation checks, stays, as the note below says.",
      temporary_exports: 'Nothing reads these any more.',
      orphan_note_images:
        'No note uses these images. An image changed in the last 10 minutes is kept, in case its note is not saved yet.',
    });
  });

  it('Cancel and Escape call nothing and return the focus to the Delete… that opened it', async () => {
    const { d, calls } = deleterOf(done({ deleted: 3, outcome: 'done' }));
    render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
    let dialog = await openDialog('cached_reports');
    fireEvent.click(within(dialog).getByTestId('stored-delete-cancel'));
    expect(screen.queryByTestId('stored-delete-dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('stored-delete-cached_reports'));
    dialog = await openDialog('cached_reports');
    fireEvent.keyDown(dialog, { key: 'Escape' });
    expect(screen.queryByTestId('stored-delete-dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('stored-delete-cached_reports'));
    await Promise.resolve();
    expect(calls).toEqual([]);
  });

  it('one prepare and one delete per confirmed click, with that category and its confirmation', async () => {
    const { d, calls } = deleterOf(done({ deleted: 3, outcome: 'done' }));
    render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
    const dialog = await openDialog('expired_cache');
    const confirm = within(dialog).getByTestId('stored-delete-confirm');
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => expect(screen.queryByTestId('stored-delete-dialog')).toBeNull());
    expect(calls).toEqual([
      ['prepare', 'expired_cache'],
      ['delete', 'expired_cache', CONFIRMATION],
    ]);
  });

  it('deletes only what the dialog shows: a changed count asks again (decision 226, F9)', async () => {
    const calls: string[] = [];
    const prepareItems = 5; // differs from the section's shown 3
    const deleter: StoredDataDeleter = {
      prepareDelete(category) {
        calls.push('prepare');
        return Promise.resolve({ confirmation: CONFIRMATION, category, items: prepareItems, bytes: 1_000, expiresInMs: 300_000 });
      },
      delete(category) {
        calls.push('delete');
        return done({ deleted: prepareItems, outcome: 'done' })(category);
      },
    };
    render(
      <StoredDataSection
        bridge={bridgeOf(summary({ cached_reports: [3, 1_000] }), summary({ cached_reports: [0, 0] }))}
        deleter={deleter}
        desktop
      />
    );
    const dialog = await openDialog('cached_reports');
    expect(within(dialog).getByTestId('stored-delete-count').textContent).toBe('3 reports. Size: under 0.01 MB.');
    // First Delete: prepare says 5, not the shown 3 → nothing deleted; the
    // dialog shows the new count and asks again.
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    expect((await within(dialog).findByTestId('stored-delete-changed')).textContent).toBe(DELETION_TEXT.changed);
    expect(within(dialog).getByTestId('stored-delete-count').textContent).toBe('5 reports. Size: under 0.01 MB.');
    expect(calls).toEqual(['prepare']);
    expect(screen.getByTestId('stored-delete-dialog')).toBeTruthy();
    // The dialog now shows 5, which prepare still returns: the next Delete deletes.
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    await waitFor(() => expect(screen.queryByTestId('stored-delete-dialog')).toBeNull());
    expect(calls).toEqual(['prepare', 'prepare', 'delete']);
  });
});

describe('a refusal', () => {
  it.each([
    'stored_data.analysis_running',
    'stored_data.gap_finder_running',
    'stored_data.papers_in_use',
    'stored_data.job_running',
    'stored_data.fetch_running',
    'stored_data.confirmation_expired',
  ])('%s: its sentence; the dialog stays, Cancel reads Close and takes the focus; Close returns to Delete…', async (code) => {
    const b = bridgeOf(summary());
    const { d, calls } = deleterOf(refused(code));
    render(<StoredDataSection bridge={b} deleter={d} desktop />);
    const dialog = await openDialog('thesis_audits');
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    const refusal = await within(dialog).findByTestId('stored-delete-refusal');
    expect(refusal.textContent).toBe(REFUSAL_WORDS[code]);
    const cancel = within(dialog).getByTestId('stored-delete-cancel');
    await waitFor(() => expect(document.activeElement).toBe(cancel));
    expect(cancel.textContent).toBe(DELETION_TEXT.close);
    expect(within(dialog).getByTestId('stored-delete-confirm')).toBeTruthy();
    expect(calls.length).toBe(2);
    expect(b.calls).toBe(1);
    fireEvent.click(cancel);
    expect(screen.queryByTestId('stored-delete-dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByTestId('stored-delete-thesis_audits'));
    expect(screen.getByTestId('stored-data-message').textContent).toBe('');
  });

  it('every refusal of decision 214 has its own sentence', () => {
    const sentences = [
      'stored_data.analysis_running',
      'stored_data.gap_finder_running',
      'stored_data.papers_in_use',
      'stored_data.job_running',
      'stored_data.fetch_running',
    ].map((c) => REFUSAL_WORDS[c]);
    expect(new Set(sentences).size).toBe(5);
    for (const s of sentences) expect(s).toMatch(/\.$/);
  });

  it('a failure: its sentence, and the section read again', async () => {
    const b = bridgeOf(summary(), summary({ open_access_papers: [1, 500] }));
    const { d } = deleterOf(() => Promise.reject(new Error(`internal error: stored_data.unavailable at ${SENTINELS[1]}`)));
    render(<StoredDataSection bridge={b} deleter={d} desktop />);
    const dialog = await openDialog('open_access_papers');
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    expect((await within(dialog).findByTestId('stored-delete-refusal')).textContent).toBe(DELETION_TEXT.failed);
    await waitFor(() => expect(b.calls).toBe(2));
    expect(document.body.innerHTML).not.toContain('SENTINEL');
  });
});

describe('after a delete', () => {
  it('"Deleted: {count}.", the section read again, and the focus on the Stored data heading', async () => {
    const b = bridgeOf(summary(), summary({ manuscript_analyses: [0, 0] }));
    const { d } = deleterOf(done({ deleted: 3, outcome: 'done' }));
    render(<StoredDataSection bridge={b} deleter={d} desktop />);
    const dialog = await openDialog('manuscript_analyses');
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    await waitFor(() => expect(screen.getByTestId('stored-data-message').textContent).toBe('Deleted: 3 analyses.'));
    expect(screen.queryByTestId('stored-delete-dialog')).toBeNull();
    // The focus moves in the dialog's unmount cleanup, an effect that can run
    // after the render that shows the message: wait for it (Research OS,
    // Phase 8F-3, decision 297).
    await waitFor(() => expect(document.activeElement).toBe(screen.getByTestId('stored-data-heading')));
    expect(screen.getByTestId('stored-data-heading').textContent).toBe(STORED_DATA_TEXT.heading);
    await waitFor(() => expect(screen.getByTestId('stored-count-manuscript_analyses').textContent).toBe('0 analyses. Size: 0 MB.'));
    expect(b.calls).toBe(2);
    expect(screen.queryByTestId('stored-delete-manuscript_analyses')).toBeNull();
  });

  it.each(DATABASE_CATEGORIES.map((id) => [id]))(
    '%s: a checkpoint that did not finish keeps the count and adds its sentence (decision 224)',
    async (id) => {
      const words = STORED_CATEGORIES.find((c) => c.id === id)!;
      const { d } = deleterOf(done({ deleted: 3, outcome: 'not_done', code: 'stored_data.checkpoint_busy' }));
      render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
      const dialog = await openDialog(id);
      fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
      await waitFor(() =>
        expect(screen.getByTestId('stored-data-message').textContent).toBe(`Deleted: 3 ${words.many}. ${DELETION_TEXT.notCompacted}`)
      );
    }
  );

  it.each(['comparison_log', 'open_access_papers', 'temporary_exports', 'orphan_note_images'] as const)(
    '%s: never the compaction sentence; a removal that failed says how many were deleted',
    async (id) => {
      const words = STORED_CATEGORIES.find((c) => c.id === id)!;
      let { d } = deleterOf(done({ deleted: 1, outcome: 'not_done', code: 'stored_data.remove_failed' }));
      render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
      let dialog = await openDialog(id);
      fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
      await waitFor(() =>
        expect(screen.getByTestId('stored-data-message').textContent).toBe(`Deleted: 1 ${words.one}. ${DELETION_TEXT.partlyRemoved}`)
      );
      expect(screen.getByTestId('stored-data').textContent).not.toContain(DELETION_TEXT.notCompacted);
      cleanup();
      ({ d } = deleterOf(done({ deleted: 3, outcome: 'done' })));
      render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
      dialog = await openDialog(id);
      fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
      await waitFor(() => expect(screen.getByTestId('stored-data-message').textContent).toBe(`Deleted: 3 ${words.many}.`));
    }
  );

  it.each([
    ['done', `Deleted: 2 analyses. ${DELETION_TEXT.partlyRemoved}`],
    ['stored_data.checkpoint_busy', `Deleted: 2 analyses. ${DELETION_TEXT.partlyRemoved} ${DELETION_TEXT.notCompacted}`],
    ['stored_data.checkpoint_failed', `Deleted: 2 analyses. ${DELETION_TEXT.partlyRemoved} ${DELETION_TEXT.notCompacted}`],
  ])('a partly deleted manuscripts delete, its checkpoint %s: the count, the partial sentence, and the not-done one only when the checkpoint did not finish (decision 230)', async (checkpoint, line) => {
    const { d } = deleterOf(done({ deleted: 2, outcome: 'not_done', code: 'stored_data.delete_partial', checkpoint }));
    render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
    const dialog = await openDialog('manuscript_analyses');
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    await waitFor(() => expect(screen.getByTestId('stored-data-message').textContent).toBe(line));
  });

  it('a partial answer without its checkpoint, or a checkpoint on another code, reads as a failure', async () => {
    for (const answer of [
      { deleted: 2, outcome: 'not_done', code: 'stored_data.delete_partial' },
      { deleted: 2, outcome: 'not_done', code: 'stored_data.delete_partial', checkpoint: 'maybe' },
      { deleted: 2, outcome: 'not_done', code: 'stored_data.checkpoint_busy', checkpoint: 'done' },
    ] as Deleted[]) {
      const { d } = deleterOf(done(answer));
      render(<StoredDataSection bridge={bridgeOf(summary())} deleter={d} desktop />);
      const dialog = await openDialog('manuscript_analyses');
      fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
      expect((await within(dialog).findByTestId('stored-delete-refusal')).textContent).toBe(DELETION_TEXT.failed);
      cleanup();
    }
  });

  it('an answer of another shape reads as a failure', async () => {
    const b = bridgeOf(summary());
    const { d } = deleterOf((category) => Promise.resolve({ category, deleted: 3, outcome: 'maybe' }));
    render(<StoredDataSection bridge={b} deleter={d} desktop />);
    const dialog = await openDialog('cached_reports');
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    expect((await within(dialog).findByTestId('stored-delete-refusal')).textContent).toBe(DELETION_TEXT.failed);
  });
});

describe('the note and the privacy of the screen', () => {
  it('the note is under the section while it reads, when it fails and once it has read', async () => {
    let release: (v: unknown) => void = () => {};
    const pending: StoredDataBridge = { summary: () => new Promise((r) => (release = r)) };
    render(<StoredDataSection bridge={pending} desktop />);
    expect(screen.getByTestId('stored-data-note').textContent).toBe(
      'Some of what Gaply keeps is not listed here yet: the files you linked to citations and the papers Gaply fetched for citation checks, with their text, and the evidence cards of citation checks.'
    );
    await act(async () => release(summary()));
    await screen.findAllByTestId(/^stored-line-/);
    expect(screen.getByTestId('stored-data-note')).toBeTruthy();
    cleanup();
    render(<StoredDataSection bridge={{ summary: () => Promise.reject(new Error('x')) }} desktop />);
    await screen.findByTestId('stored-data-failed');
    expect(screen.getByTestId('stored-data-note').textContent).toBe(STORED_DATA_TEXT.note);
  });

  it('no title, name, path or URL reaches the screen, through the summary, the confirmation or the delete', async () => {
    const b = bridgeOf(summary(), summary({ orphan_note_images: [0, 0] }));
    const { d } = deleterOf(done({ deleted: 3, outcome: 'done' }));
    render(<StoredDataSection bridge={b} deleter={d} desktop />);
    const dialog = await openDialog('orphan_note_images');
    for (const s of SENTINELS) expect(document.body.innerHTML).not.toContain(s);
    fireEvent.click(within(dialog).getByTestId('stored-delete-confirm'));
    await waitFor(() => expect(b.calls).toBe(2));
    for (const s of SENTINELS) expect(document.body.innerHTML).not.toContain(s);
  });

  it('in the browser: nothing drawn, and no command called, the tauri deleter included', async () => {
    env.desktop = false;
    const { d, calls } = deleterOf(done({ deleted: 1, outcome: 'done' }));
    const b = bridgeOf(summary());
    const { container } = render(<StoredDataSection bridge={b} deleter={d} />);
    await Promise.resolve();
    expect(container.innerHTML).toBe('');
    expect([b.calls, calls.length]).toEqual([0, 0]);
    await expect(tauriStoredDataDeleter.prepareDelete('cached_reports')).rejects.toThrow();
    await expect(tauriStoredDataDeleter.delete('cached_reports', CONFIRMATION)).rejects.toThrow();
    expect(tauri.calls).toEqual([]);
  });

  it('the tauri deleter sends the category and the confirmation, and nothing else', async () => {
    // The mocked invoke leaves both pending: only the calls are read. One
    // after the other, as the section makes them: two dynamic imports of a
    // mocked module at once give the second undefined in vitest.
    void tauriStoredDataDeleter.prepareDelete('cached_reports');
    await waitFor(() => expect(tauri.calls.length).toBe(1));
    void tauriStoredDataDeleter.delete('cached_reports', CONFIRMATION);
    await waitFor(() => expect(tauri.calls).toEqual([
      ['stored_data_prepare_delete', { category: 'cached_reports' }],
      ['stored_data_delete', { category: 'cached_reports', confirmation: CONFIRMATION }],
    ]));
  });
});

/* ---- the Settings outline (decision 216) ----------------------------------- */

function auth(): AuthService {
  return {
    signUp: vi.fn(),
    signIn: vi.fn(),
    signInWithOAuth: vi.fn(),
    signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: (s: null) => void) => {
      cb(null);
      return () => {};
    },
  } as unknown as AuthService;
}

async function outline(): Promise<string[]> {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>
        <SettingsPage profileService={{ getOwn: vi.fn(), upsertOwn: vi.fn() } as never} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
  const nav = await screen.findByTestId('settings-nav');
  return Array.from(nav.querySelectorAll('button')).map((b) => b.textContent ?? '');
}

describe('the Settings outline', () => {
  it.each([
    ['both gates on', { projects: true, workspaces: true, storedData: true, desktop: true }, ['Projects', 'Stored data']],
    ['Stored data off', { projects: true, workspaces: true, storedData: false, desktop: true }, ['Projects']],
    ['Projects off', { projects: false, workspaces: true, storedData: true, desktop: true }, ['Stored data']],
    ['workspaces off', { projects: true, workspaces: false, storedData: true, desktop: true }, ['Stored data']],
    ['every flag off', { projects: false, workspaces: false, storedData: false, desktop: true }, []],
    ['the browser', { projects: true, workspaces: true, storedData: true, desktop: false }, []],
  ])('%s', async (_, flags, items) => {
    Object.assign(env, flags);
    const got = await outline();
    expect(got).toEqual(['Profile & ORCID', 'Sync & Privacy', 'Offline Data', 'Subscription', 'Appearance', 'Local AI', ...items, 'About']);
    // Each item it shows scrolls to a section that is there.
    for (const id of items.map((i) => (i === 'Projects' ? 'set-projects' : 'set-stored-data'))) {
      expect(document.getElementById(id)).not.toBeNull();
    }
  });
});
