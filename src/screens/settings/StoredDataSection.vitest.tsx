// The Stored data section (Research OS, Phase 5D-1a; decisions 206 and 207):
// hidden with REACT_APP_FEATURE_STORED_DATA off and in the browser, with no
// command called; nine lines of name, sentence, count and size from the
// summary, sizes through mb(); one sentence when the summary fails or does
// not have its shape; nothing from the answer but the two counts, and no
// delete control. Then its place in Settings, after Projects and before
// About, and the names it shares with Rust.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { StoredDataSection } from './StoredDataSection';
import { STORED_DATA_COMMANDS, StoredDataBridge } from './storedDataBridge';
import { STORED_CATEGORIES, STORED_DATA_TEXT } from './storedDataWording';
import SettingsPage from './SettingsPage';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import { IMAGE_REF_PREFIX, IMAGE_SUBDIR } from '../notes/noteImages';

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

/** Tauri's invoke, for the whole Settings page: every call recorded; the
 *  summary answered, everything else left pending. */
const tauri = vi.hoisted(() => ({ calls: [] as Array<[string, unknown]>, summary: undefined as unknown }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => {
    tauri.calls.push([cmd, args]);
    return cmd === 'stored_data_summary' ? Promise.resolve(tauri.summary) : new Promise(() => {});
  },
}));

const SENTINELS = ['SENTINEL_TITLE_5d1a', '/Users/SENTINEL_PATH_5d1a', 'https://SENTINEL_URL_5d1a', 'SENTINEL_NAME_5d1a'];

/** One count per category, out of order, each carrying fields a view must
 *  never show. */
function summary(over: Record<string, [number, number]> = {}) {
  const counts: Record<string, [number, number]> = {
    manuscript_analyses: [3, 2_500_000],
    gap_finder_papers: [1, 1_536],
    thesis_audits: [0, 0],
    cached_reports: [1_204, 37_000_000],
    expired_cache: [2, 9_999],
    comparison_log: [1, 10_000],
    open_access_papers: [12, 48_123_456],
    temporary_exports: [2, 1_234_567],
    orphan_note_images: [1, 60],
    ...over,
  };
  return {
    categories: Object.entries(counts)
      .reverse()
      .map(([category, [items, bytes]]) => ({
        category,
        items,
        bytes,
        title: SENTINELS[0],
        path: SENTINELS[1],
        url: SENTINELS[2],
      })),
    name: SENTINELS[3],
  };
}

function bridgeOf(answer: () => Promise<unknown>): StoredDataBridge & { calls: number } {
  const b = {
    calls: 0,
    summary() {
      b.calls += 1;
      return answer();
    },
  };
  return b;
}

beforeEach(() => {
  env.storedData = true;
  env.projects = false;
  env.workspaces = false;
  env.desktop = true;
  tauri.calls = [];
  tauri.summary = summary();
  localStorage.clear();
});
afterEach(cleanup);

describe('the section, hidden', () => {
  it('with the flag off: nothing drawn, no command called', async () => {
    env.storedData = false;
    const b = bridgeOf(() => Promise.resolve(summary()));
    const { container } = render(<StoredDataSection bridge={b} desktop />);
    await Promise.resolve();
    expect(container.innerHTML).toBe('');
    expect(b.calls).toBe(0);
  });

  it('in the browser: nothing drawn, no command called', async () => {
    const b = bridgeOf(() => Promise.resolve(summary()));
    const { container } = render(<StoredDataSection bridge={b} desktop={false} />);
    await Promise.resolve();
    expect(container.innerHTML).toBe('');
    expect(b.calls).toBe(0);
  });
});

describe('the section, shown', () => {
  it('says it is reading, then shows nine lines in order, with counts and sizes in decimal MB', async () => {
    const b = bridgeOf(() => Promise.resolve(summary()));
    render(<StoredDataSection bridge={b} desktop />);
    expect(screen.getByTestId('stored-data').textContent).toContain(STORED_DATA_TEXT.reading);
    const lines = await screen.findAllByTestId(/^stored-line-/);
    expect(b.calls).toBe(1);
    expect(lines.map((l) => l.getAttribute('data-testid'))).toEqual(STORED_CATEGORIES.map((c) => `stored-line-${c.id}`));
    STORED_CATEGORIES.forEach((c, i) => {
      expect(lines[i].textContent).toContain(c.name);
      expect(lines[i].textContent).toContain(c.sentence);
    });
    const size = (id: string) => screen.getByTestId(`stored-count-${id}`).textContent;
    expect(size('manuscript_analyses')).toBe('3 analyses. Size: 2.50 MB.');
    expect(size('gap_finder_papers')).toBe('1 paper. Size: under 0.01 MB.');
    expect(size('thesis_audits')).toBe('0 audits. Size: 0 MB.');
    expect(size('cached_reports')).toBe('1,204 reports. Size: 37.00 MB.');
    expect(size('expired_cache')).toBe('2 rows. Size: under 0.01 MB.');
    expect(size('comparison_log')).toBe('1 file. Size: 0.01 MB.');
    expect(size('open_access_papers')).toBe('12 files. Size: 48.12 MB.');
    expect(size('temporary_exports')).toBe('2 files. Size: 1.23 MB.');
    expect(size('orphan_note_images')).toBe('1 image. Size: under 0.01 MB.');
    // mb(), never settingsStore's formatBytes (binary KB and MB).
    expect(screen.getByTestId('stored-data').textContent).not.toMatch(/\bKB\b|\d B\b/);
  });

  it('shows nothing from the answer but the two counts, and offers no delete', async () => {
    render(<StoredDataSection bridge={bridgeOf(() => Promise.resolve(summary()))} desktop />);
    await screen.findAllByTestId(/^stored-line-/);
    const html = screen.getByTestId('stored-data').innerHTML;
    for (const s of SENTINELS) expect(html).not.toContain(s);
    expect(screen.getByTestId('stored-data').querySelectorAll('button, a, input, [role="button"]').length).toBe(0);
    expect(screen.getByTestId('stored-data').textContent).not.toMatch(/delete/i);
  });

  it('when the summary fails: one sentence, and nothing else', async () => {
    const b = bridgeOf(() => Promise.reject(new Error(`database error at ${SENTINELS[1]}`)));
    render(<StoredDataSection bridge={b} desktop />);
    const failed = await screen.findByTestId('stored-data-failed');
    expect(failed.textContent).toBe(STORED_DATA_TEXT.failed);
    expect(screen.queryAllByTestId(/^stored-line-/)).toEqual([]);
    expect(screen.getByTestId('stored-data').textContent).not.toContain('SENTINEL');
  });

  it.each([
    ['no answer', undefined],
    ['no categories', {}],
    ['eight categories', { categories: summary().categories.slice(1) }],
    ['a category twice', { categories: [...summary().categories.slice(1), summary().categories[1]] }],
    ['an unknown category', { categories: summary().categories.map((c, i) => (i === 0 ? { ...c, category: 'manuscripts' } : c)) }],
    ['a negative count', summary({ thesis_audits: [-1, 0] })],
    ['a fraction', summary({ thesis_audits: [1, 0.5] })],
    ['a count as text', { categories: summary().categories.map((c, i) => (i === 0 ? { ...c, items: '3' } : c)) }],
  ])('an answer with %s reads as a failure', async (_, answer) => {
    render(<StoredDataSection bridge={bridgeOf(() => Promise.resolve(answer))} desktop />);
    expect((await screen.findByTestId('stored-data-failed')).textContent).toBe(STORED_DATA_TEXT.failed);
    expect(screen.queryAllByTestId(/^stored-line-/)).toEqual([]);
  });
});

/* ---- in Settings ------------------------------------------------------------ */

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

function renderSettings() {
  return render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>
        <SettingsPage profileService={{ getOwn: vi.fn(), upsertOwn: vi.fn() } as never} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

const sectionIds = () => Array.from(document.querySelectorAll('section')).map((s) => s.id);
const summaryCalls = () => tauri.calls.filter(([c]) => c === STORED_DATA_COMMANDS.summary);

describe('in Settings', () => {
  it('after the Projects section and before About, calling the summary once with nothing', async () => {
    env.projects = true;
    env.workspaces = true;
    renderSettings();
    await screen.findAllByTestId(/^stored-line-/);
    const ids = sectionIds();
    expect(ids.indexOf('set-stored-data')).toBe(ids.indexOf('set-projects') + 1);
    expect(ids.indexOf('set-about')).toBe(ids.indexOf('set-stored-data') + 1);
    expect(summaryCalls()).toEqual([[STORED_DATA_COMMANDS.summary, undefined]]);
  });

  it('there whatever the Projects flags say', async () => {
    renderSettings();
    await screen.findAllByTestId(/^stored-line-/);
    const ids = sectionIds();
    expect(ids).not.toContain('set-projects');
    expect(ids.indexOf('set-stored-data')).toBe(ids.indexOf('set-ai') + 1);
    expect(ids.indexOf('set-about')).toBe(ids.indexOf('set-stored-data') + 1);
  });

  it.each([
    ['the flag off', () => (env.storedData = false)],
    ['the browser', () => (env.desktop = false)],
  ])('absent with %s, and the summary never called', async (_, set) => {
    set();
    renderSettings();
    await waitFor(() => expect(screen.getByTestId('settings')).toBeTruthy());
    await Promise.resolve();
    expect(sectionIds()).not.toContain('set-stored-data');
    expect(screen.queryByTestId('stored-data')).toBeNull();
    expect(summaryCalls()).toEqual([]);
  });
});

/* ---- the names it shares ------------------------------------------------------ */

const SRC = path.join(__dirname, '../..');
const CORE = path.join(SRC, '../src-tauri/gaply-core/src/stored_data.rs');

describe('the names the section shares with Rust and with the Note Creator', () => {
  it("the nine ids are Rust's StoredCategory variants, in Rust's order", () => {
    const rust = fs.readFileSync(CORE, 'utf8');
    const body = rust.slice(rust.indexOf('pub enum StoredCategory {'), rust.indexOf('impl StoredCategory'));
    const variants: string[] = [];
    const re = /^\s{4}([A-Z][A-Za-z]+),$/gm;
    let m: RegExpExecArray | null;
    while ((m = re.exec(body)) !== null) variants.push(m[1].replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase());
    expect(variants).toEqual(STORED_CATEGORIES.map((c) => c.id));
  });

  it("the note images' folder and reference are the Note Creator's", () => {
    const rust = fs.readFileSync(CORE, 'utf8');
    expect(rust).toContain(`pub const NOTE_IMAGES_DIR: &str = "${IMAGE_SUBDIR}";`);
    expect(rust).toContain(`pub const NOTE_IMAGE_REF_PREFIX: &str = "${IMAGE_REF_PREFIX}";`);
  });

  it('category 9 is named "Unused note images" (decision 208)', () => {
    expect(STORED_CATEGORIES.find((c) => c.id === 'orphan_note_images')?.name).toBe('Unused note images');
    expect(STORED_CATEGORIES.map((c) => c.name).join(' ')).not.toMatch(/orphan/i);
  });

  it('sentence 7 names exactly the features whose screens fetch into oa_papers/ (decision 208)', () => {
    // Every screen that starts the open-access fetch, by the feature a user
    // knows it as. A new caller fails here until it is classified.
    const FEATURE_OF: Record<string, string> = {
      'screens/citations/CitationManagerPage.tsx': 'Citation Manager',
      'screens/ai/DocumentRow.tsx': 'Citation Manager',
      'screens/ai/ThesisAuditScreen.tsx': 'thesis citation audit',
    };
    const callers: string[] = [];
    const walk = (dir: string) => {
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, e.name);
        if (e.isDirectory()) walk(p);
        else if (/\.(ts|tsx)$/.test(e.name) && !/\.vitest\.tsx?$/.test(e.name)) {
          if (/\.fetchOpenAccess\(/.test(fs.readFileSync(p, 'utf8'))) callers.push(path.relative(SRC, p).split(path.sep).join('/'));
        }
      }
    };
    walk(SRC);
    callers.sort();
    expect(callers).toEqual(Object.keys(FEATURE_OF).sort());
    // The bridge method is the open-access fetch command.
    const bridge = fs.readFileSync(path.join(SRC, 'screens/ai/aiBridge.ts'), 'utf8');
    expect(bridge.slice(bridge.indexOf('async fetchOpenAccess('))).toMatch(/^[\s\S]{0,400}'citation_fetch_oa'/);
    // The sentence's "Made by" names those features and no other.
    const sentence = STORED_CATEGORIES.find((c) => c.id === 'open_access_papers')!.sentence;
    const madeBy = sentence.slice(sentence.indexOf('Made by '));
    const features = Array.from(new Set(Object.values(FEATURE_OF))).sort();
    expect(madeBy).toBe(`Made by the ${features.join(' and the ')}.`);
  });

  it('only the bridge names the command, and only Settings draws the section', () => {
    const offenders: string[] = [];
    const drawers: string[] = [];
    const walk = (dir: string) => {
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, e.name);
        if (e.isDirectory()) walk(p);
        else if (/\.(ts|tsx)$/.test(e.name) && !/\.vitest\.tsx?$/.test(e.name)) {
          const text = fs.readFileSync(p, 'utf8');
          const rel = path.relative(SRC, p).split(path.sep).join('/');
          if (text.includes('stored_data_') && rel !== 'screens/settings/storedDataBridge.ts') offenders.push(rel);
          if (/<StoredDataSection[\s/>]/.test(text)) drawers.push(rel);
        }
      }
    };
    walk(SRC);
    expect(offenders).toEqual([]);
    expect(drawers).toEqual(['screens/settings/SettingsPage.tsx']);
    const settings = fs.readFileSync(path.join(SRC, 'screens/settings/SettingsPage.tsx'), 'utf8');
    expect(settings).toMatch(/storedDataShown\(\) && \(/);
  });
});
