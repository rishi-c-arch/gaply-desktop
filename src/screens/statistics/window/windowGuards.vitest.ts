// What the Statistics window's files may not reach (Research OS, Phase 8P,
// decision 454; ADR-012 Part 10), on projectGuards.vitest.ts's pattern: a source
// scan with comments stripped, over the window's, the session's and the switch
// hook's files. It constrains what is TYPED, not what is rendered: a name built
// at runtime, such as a property looked up by a computed string, is not seen.
// Each pattern has a mutation in the step's record.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const HERE = __dirname;
const isTest = (f: string) => /\.vitest\.(ts|tsx)$/.test(f);

/** Every production file in this folder, read from the folder, so a file added
 *  tomorrow is scanned by a test nobody updates; and the hook beside them. */
const FILES = [
  ...fs
    .readdirSync(HERE)
    .filter((f) => /\.(ts|tsx)$/.test(f) && !isTest(f))
    .map((f) => path.join(HERE, f)),
  path.join(HERE, '../../workspace/useWorkspaceSwitch.tsx'),
];

const code = (src: string) =>
  src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((l) => l.replace(/(^|\s)\/\/.*$/, '$1'))
    .join('\n');

const FORBIDDEN: Array<[string, RegExp]> = [
  ['localStorage', /\blocalStorage\b/],
  ['sessionStorage', /\bsessionStorage\b/],
  ['indexedDB', /\bindexedDB\b/i],
  ['console', /\bconsole\s*\./],
  ['fetch', /\bfetch\s*\(/],
  ['a Supabase client', /supabase/i],
  ['an AI bridge', /\baiBridge\b|\baiReady\b|\/ai\/|statsL1Bridge|stats_assist|stats_ai\b/],
  ['markup from a string', /dangerouslySetInnerHTML/],
  ['innerHTML', /\binnerHTML\b/],
  ['outerHTML', /\bouterHTML\b/],
  ['insertAdjacentHTML', /\binsertAdjacentHTML\b/],
];

describe("the Statistics window's files", () => {
  it('are found: the scan is not inert', () => {
    const names = FILES.map((f) => path.basename(f));
    for (const n of ['StatisticsWindow.tsx', 'statisticsSession.ts', 'useWindowFile.ts', 'VariableView.tsx', 'HelpDialog.tsx', 'windowMenus.ts', 'windowWording.ts', 'useWorkspaceSwitch.tsx', 'DataGrid.tsx', 'gridCells.ts', 'gridRange.ts', 'gridTiming.ts', 'TimingLine.tsx']) {
      expect(names, n).toContain(n);
    }
    for (const f of FILES) expect(code(fs.readFileSync(f, 'utf8')).length, f).toBeGreaterThan(100);
  });

  it.each(FORBIDDEN)('name no %s', (_name, pattern) => {
    const offenders = FILES.filter((f) => pattern.test(code(fs.readFileSync(f, 'utf8')))).map((f) => path.basename(f));
    expect(offenders).toEqual([]);
  });

  it('send no project_* command of their own: the project bridge does', () => {
    const offenders = FILES.filter((f) => /['"`]project_[a-z_]+['"`]/.test(code(fs.readFileSync(f, 'utf8')))).map((f) => path.basename(f));
    expect(offenders).toEqual([]);
  });

  it('the scan sees a name in code, and not in a comment', () => {
    expect(FORBIDDEN[0][1].test(code('localStorage.setItem("a", "b");'))).toBe(true);
    expect(FORBIDDEN[0][1].test(code('// never localStorage'))).toBe(false);
    expect(FORBIDDEN[0][1].test(code('/* never localStorage */ x'))).toBe(false);
    expect(FORBIDDEN[3][1].test(code('console.log(1);'))).toBe(true);
    expect(FORBIDDEN[4][1].test(code("fetch('x');"))).toBe(true);
  });
});
