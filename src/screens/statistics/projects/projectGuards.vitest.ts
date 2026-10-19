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
    expect(names).toHaveLength(11);
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
    for (const f of ['SaveToProject.tsx', 'ProjectsPanel.tsx']) {
      expect(fs.readFileSync(path.join(HERE, f), 'utf8'), f).toMatch(/projectsShown\(props\.desktop\)/);
    }
    const settings = fs.readFileSync(path.join(SRC, 'screens/settings/SettingsPage.tsx'), 'utf8');
    expect(settings).toMatch(/projectsShown\(\) && \(/);
    expect(settings).toContain('SETTINGS_SENTENCE');
  });
});
