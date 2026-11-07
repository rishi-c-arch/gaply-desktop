// What scale scores' screen files may not reach (Research OS, Phase 8T-1b,
// decision 499): the Statistics window's rule (Phase 8P, decision 454; ADR-012
// Part 10), over prepTypes.ts, prepBridge.ts and prepWording.ts, which sit
// beside the methods' files and outside the window's folder, so
// windowGuards.vitest.ts does not scan them. A source scan with comments
// stripped: it constrains what is TYPED, not what is rendered.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const FILES = ['prepTypes.ts', 'prepBridge.ts', 'prepWording.ts'].map((f) => path.join(__dirname, f));

// Copied from window/windowGuards.vitest.ts:34-39, as AlphaFlow.vitest.tsx:38
// copies VERDICT: importing a vitest file would run its tests again.
const code = (src: string) =>
  src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((l) => l.replace(/(^|\s)\/\/.*$/, '$1'))
    .join('\n');

// Copied from window/windowGuards.vitest.ts:41-53.
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

describe("scale scores' screen files", () => {
  it('are found: the scan is not inert', () => {
    expect(FILES).toHaveLength(3);
    for (const f of FILES) expect(code(fs.readFileSync(f, 'utf8')).length, f).toBeGreaterThan(500);
  });

  it.each(FORBIDDEN)('name no %s', (_name, pattern) => {
    const offenders = FILES.filter((f) => pattern.test(code(fs.readFileSync(f, 'utf8')))).map((f) => path.basename(f));
    expect(offenders).toEqual([]);
  });
});
