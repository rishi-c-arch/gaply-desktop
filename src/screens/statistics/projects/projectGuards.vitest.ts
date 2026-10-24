// Source guards for the project screens (Research OS, Phase 5B-3a). Each list
// is read from the source, not typed:
//
// 1. Only projectBridge.ts names a project command. The names come from its
//    PROJECT_COMMANDS; every other file under src/, tests included, is
//    scanned for each name and for any quoted 'project_…' string. So no
//    screen outside the project screens, and no project screen but through
//    the bridge, can call one.
// 2. No project screen uses raw HTML (dangerouslySetInnerHTML, innerHTML,
//    outerHTML, insertAdjacentHTML, document.write, DOMParser), or writes to
//    the console, browser storage, document.title or the router: a
//    project's name may reach none of them.
// 3. Every component that shows a project screen asks projectsShown(), and so
//    does Settings for its sentence.
// 4. Phase 5B-3b: the page around the project page (ProjectPageScreen.tsx,
//    outside this folder because it navigates) uses no raw HTML, writes to
//    no console, storage, title or history, and navigates only to fixed
//    paths; and in the new code of 5B-3b no `throw` builds its message from
//    anything but a string literal, so no error there can carry data.
//
// Comments are removed first (a block comment, and a `//` at a line's start
// or after white space), so documentation may name a command; code may not.
//
// This constrains what was TYPED. A name built at run time from pieces, or
// a call through an alias of invoke, is not seen; the runtime tests in
// projectScreens.vitest.tsx spy on invoke for that. Each scan asserts a
// positive count first, so an inert scan fails rather than passes.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import { PROJECT_COMMANDS } from './projectBridge';

const SRC = path.join(__dirname, '../../..');
const HERE = __dirname;

function walk(dir: string, out: string[] = []): string[] {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (/\.(ts|tsx|js|jsx)$/.test(e.name)) out.push(p);
  }
  return out;
}

const rel = (p: string) => path.relative(SRC, p).split(path.sep).join('/');

/** The source without its comments. */
const code = (src: string) =>
  src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((l) => l.replace(/(^|\s)\/\/.*$/, '$1'))
    .join('\n');
const isTest = (p: string) => /\.vitest\.(ts|tsx)$/.test(p) || /\.test\.(ts|tsx|js)$/.test(p);
const BRIDGE = path.join(HERE, 'projectBridge.ts');

describe('only the project bridge names a project command', () => {
  it('no other file, inside or outside the project screens', () => {
    const names = Object.values(PROJECT_COMMANDS);
    // Twelve since Phase 5B-3b: project_current (decision 148); thirteen
    // since 5C-1a: project_rerun (decision 185).
    expect(names).toHaveLength(13);
    const files = walk(SRC);
    expect(files.length).toBeGreaterThan(200);
    const bridge = fs.readFileSync(BRIDGE, 'utf8');
    for (const n of names) expect(bridge, n).toContain(`'${n}'`);
    const offenders: string[] = [];
    for (const f of files) {
      if (f === BRIDGE) continue;
      const src = code(fs.readFileSync(f, 'utf8'));
      // This file's own tests may name the commands they check, through the
      // bridge's list or the fixtures; the guard file itself only reads them.
      const testHere = isTest(f) && path.dirname(f) === HERE;
      for (const n of names) {
        // As a whole identifier: 'gaply_project_report.md' is a file name.
        if (!testHere && new RegExp(`(^|[^a-z_])${n}([^a-z_]|$)`).test(src)) offenders.push(`${rel(f)}: ${n}`);
      }
      const quoted = src.match(/['"`]project_[a-z_]+['"`]/g) ?? [];
      if (!testHere && quoted.length > 0) offenders.push(`${rel(f)}: ${quoted.join(', ')}`);
    }
    expect(offenders).toEqual([]);
  });

  it('the scan sees a name in code, and not in a comment', () => {
    const quoted = code("invoke('project_delete', {}); // 'project_list'").match(/['"`]project_[a-z_]+['"`]/g);
    expect(quoted).toEqual(["'project_delete'"]);
    expect(code('/* project_open */ x')).not.toContain('project_open');
    expect(code("const u = 'https://x'; // project_open")).toContain('https://x');
    expect(/(^|[^a-z_])project_report([^a-z_]|$)/.test("'gaply_project_report.md'")).toBe(false);
    expect(/(^|[^a-z_])project_report([^a-z_]|$)/.test("b.invoke(project_report)")).toBe(true);
  });
});

describe('the project screens', () => {
  const screens = fs
    .readdirSync(HERE)
    .filter((f) => /\.(ts|tsx)$/.test(f) && !isTest(f))
    .map((f) => path.join(HERE, f));

  it('use no raw HTML and send a name nowhere it must not go', () => {
    expect(screens.length).toBeGreaterThanOrEqual(8);
    const forbidden = [
      'dangerouslySetInnerHTML',
      'innerHTML',
      'outerHTML',
      'insertAdjacentHTML',
      'document.write',
      'DOMParser',
      'console.',
      'localStorage',
      'sessionStorage',
      'indexedDB',
      'document.title',
      'useNavigate',
      'history.',
      'fetch(',
      'XMLHttpRequest',
      'sendBeacon',
    ];
    const offenders: string[] = [];
    for (const f of screens) {
      const typed = code(fs.readFileSync(f, 'utf8'));
      for (const w of forbidden) if (typed.includes(w)) offenders.push(`${path.basename(f)}: ${w}`);
    }
    expect(offenders).toEqual([]);
  });

  it('each screen asks projectsShown(), and so does Settings', () => {
    for (const f of ['SaveToProject.tsx', 'ProjectsPanel.tsx', 'ProjectPage.tsx']) {
      expect(fs.readFileSync(path.join(HERE, f), 'utf8'), f).toMatch(/projectsShown\(props\.desktop\)/);
    }
    const settings = fs.readFileSync(path.join(SRC, 'screens/settings/SettingsPage.tsx'), 'utf8');
    expect(settings).toMatch(/projectsShown\(\) && \(/);
    expect(settings).toContain('SETTINGS_SENTENCE');
  });
});

/* ---- Phase 5B-3b ------------------------------------------------------------ */

const PAGE_SCREEN = path.join(SRC, 'screens/statistics/ProjectPageScreen.tsx');

/** The files of 5B-3b's new code: the page, the saved result view and its
 *  check, the export's checks, the counter, the route and the page around it. */
const NEW_CODE = [
  path.join(HERE, 'ProjectPage.tsx'),
  path.join(HERE, 'SavedResult.tsx'),
  path.join(HERE, 'storedShape.ts'),
  path.join(HERE, 'projectExport.ts'),
  path.join(HERE, 'useGeneration.ts'),
  path.join(HERE, 'projectRoute.tsx'),
  PAGE_SCREEN,
];

/** Every `throw` in `src` whose operand is anything but `new Error('…')`
 *  with one plain string literal: a variable, a template with `${`, a
 *  concatenation, a call, or a rethrown error all can carry data. */
function throwsBuiltFromData(src: string): string[] {
  const out: string[] = [];
  const re = /\bthrow\b([^;]*);?/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(src)) !== null) {
    const operand = m[1].trim();
    if (!/^new Error\((['"])[^'"`$\\]*\1\)$/.test(operand)) out.push(`throw ${operand}`);
  }
  return out;
}

describe("Phase 5B-3b's new code", () => {
  it('the page around the project page uses no raw HTML, sends nothing where a name must not go, and navigates only to fixed paths', () => {
    const typed = code(fs.readFileSync(PAGE_SCREEN, 'utf8'));
    for (const w of ['dangerouslySetInnerHTML', 'innerHTML', 'console.', 'localStorage', 'sessionStorage', 'indexedDB', 'document.title', 'history.', 'fetch(']) {
      expect(typed, w).not.toContain(w);
    }
    const calls = typed.match(/navigate\(([^)]*)\)/g) ?? [];
    expect(calls.length).toBeGreaterThan(0);
    for (const c of calls) expect(c, c).toMatch(/^navigate\((STATISTICS_HOME_PATH|'\/app')\)$/);
    // No router state: navigate takes one argument here, a fixed path.
    expect(typed).not.toMatch(/navigate\([^)]*,/);
  });

  it('no throw builds its message from data', () => {
    const files = NEW_CODE.filter((f) => fs.existsSync(f));
    expect(files).toHaveLength(NEW_CODE.length);
    const offenders = files.flatMap((f) => throwsBuiltFromData(code(fs.readFileSync(f, 'utf8'))).map((t) => `${path.basename(f)}: ${t}`));
    expect(offenders).toEqual([]);
  });

  it('the throw scan sees a throw built from data, and passes a fixed one', () => {
    expect(throwsBuiltFromData("throw new Error('fixed words');")).toEqual([]);
    expect(throwsBuiltFromData('throw new Error(`x ${v}`);')).toHaveLength(1);
    expect(throwsBuiltFromData("throw new Error('a' + v);")).toHaveLength(1);
    expect(throwsBuiltFromData('throw e;')).toHaveLength(1);
    expect(throwsBuiltFromData('throw new Error(String(v));')).toHaveLength(1);
    expect(throwsBuiltFromData("if (x) throw { code: v };")).toHaveLength(1);
  });
});
