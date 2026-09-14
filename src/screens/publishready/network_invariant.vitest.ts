/**
 * **THE INVARIANT: BACKEND EXTRACTS, FRONTEND DISPLAYS.** (Prompt 5 item 8.)
 *
 * The journal layer's whole claim is that no model and no renderer invents a
 * journal fact. The backend crawls, classifies, extracts and stores; the
 * frontend asks for a fingerprint and renders it. A `fetch()` from a screen to
 * `api.crossref.org` or `api.openalex.org` would put a second, unguarded
 * ingestion path beside the audited one — outside the injection guards,
 * outside the rate limiter, outside provenance, and with no span to show.
 *
 * # Why this scans EVERY renderer path, not `src/screens/publishready/**`
 *
 * Item 8 asks for "no fetch() to a non-app origin under
 * src/screens/publishready/**". Scoped that narrowly the test is **green on an
 * empty directory** and stays green while real violations sit one directory
 * away — which is not hypothetical: `src/features/citation-generator/
 * citationFetchers.ts` calls CrossRef, OpenLibrary and PubMed today.
 *
 * So the scan is the whole tree, and the cost of widening it was measured
 * rather than assumed. Across 251 source files the renderer makes exactly
 * **THREE** non-app-origin network calls, all three in that one file.
 * Everything else that looked like a violation is same-origin on inspection:
 * `/csl/styles/*.csl` and `/csl/manifest.json` (bundled CSL assets),
 * `/manuscripts/scaffolds.json`, `/api/expert/*` (the app's own backend), and
 * `${window.location.origin}${path}` for the ethics deck. A whole-tree rule
 * therefore needs a ONE-FILE allowlist, which is small enough to be read and
 * argued with — so there is no reason to prefer the narrow scope.
 *
 * # What this test CANNOT catch
 *
 * - **It does not fix the three calls it allows.** `ALLOWED` is a record of
 *   what is already wrong, not a judgement that it is fine. Citation lookup
 *   predates the journal layer and moving it behind the backend is its own
 *   change with its own risk.
 * - **Indirection defeats it.** A URL assembled from parts
 *   (`'https://' + host`), read from config at runtime, or reached through a
 *   wrapper in another module will not be seen. This is a grep with balanced
 *   parens, not a call graph.
 * - **It says nothing about what the backend fetches.** That is
 *   `gaply-core`'s no-HTTP-client property and the crawl's own budget.
 * - **It cannot see a fetch that a dependency makes on the renderer's behalf.**
 *
 * The rule it DOES enforce, exactly: a literal non-app-origin URL passed to a
 * network primitive in renderer code. That is the shape all three known
 * violations have, and the shape a new one is overwhelmingly likely to have.
 */
import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const SRC = path.resolve(__dirname, '../../');

/** Network primitives a renderer can reach an origin with. */
const CALLS = ['fetch', 'axios.get', 'axios.post', 'axios', 'XMLHttpRequest', 'WebSocket', 'EventSource', 'navigator.sendBeacon'];

/**
 * **THE ALLOWLIST IS THE DEFECT REGISTER, and it is deliberately not empty.**
 *
 * Each entry names a file and the hosts it reaches, so that "already wrong" is
 * distinguishable from "never looked at" — the same distinction the journal
 * crawl config now carries for its unmeasured author-services hosts.
 *
 * **No journal or publishready path may ever appear here.** The test asserts
 * that separately, so weakening the invariant for the screens it exists to
 * protect fails rather than merely being noticed in review.
 */
const ALLOWED: Record<string, string[]> = {
  'features/citation-generator/citationFetchers.ts': [
    'api.crossref.org',
    'openlibrary.org',
    'eutils.ncbi.nlm.nih.gov',
  ],
};

/** Paths the invariant protects. Nothing here is ever allowlisted. */
const PROTECTED = ['screens/publishready/', 'screens/journal/', 'screens/journalverify/'];

function sourceFiles(dir: string, out: string[] = []): string[] {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) sourceFiles(p, out);
    else if (/\.(ts|tsx)$/.test(e.name) && !/\.(test|vitest)\./.test(e.name)) out.push(p);
  }
  return out;
}

/** Remove comments so a documentation URL is not a violation. */
function stripComments(s: string): string {
  return s
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((l) => l.replace(/(^|[^:])\/\/.*$/, '$1'))
    .join('\n');
}

/** The text of the call's arguments, by balanced parens from `open`. */
function argsAt(code: string, open: number): string {
  let depth = 0;
  for (let i = open; i < code.length; i++) {
    if (code[i] === '(') depth++;
    else if (code[i] === ')') {
      depth--;
      if (depth === 0) return code.slice(open + 1, i);
    }
  }
  return code.slice(open + 1, open + 400);
}

/** Absolute non-app origins named in `text`. */
function externalHosts(text: string): string[] {
  const out: string[] = [];
  // An `exec` loop rather than `matchAll`: this project targets es5, where
  // iterating an IterableIterator needs `downlevelIteration` and the CRA
  // production build fails without it (TS2802). Vitest's esbuild transform is
  // happy with either, so the suite is green and the shipped build is not.
  const re = /https?:\/\/([^\s'"`)/$}]+)/g;
  let m: RegExpExecArray | null = re.exec(text);
  while (m !== null) {
    const host = m[1];
    if (!(host === 'localhost' || host.startsWith('127.') || host.endsWith('gaply.in'))) {
      out.push(host);
    }
    m = re.exec(text);
  }
  return out;
}

type Violation = { file: string; host: string; snippet: string };

function scan(): Violation[] {
  const violations: Violation[] = [];
  for (const abs of sourceFiles(SRC)) {
    const rel = path.relative(SRC, abs).split(path.sep).join('/');
    const code = stripComments(fs.readFileSync(abs, 'utf8'));
    for (const call of CALLS) {
      let from = 0;
      for (;;) {
        const at = code.indexOf(`${call}(`, from);
        if (at === -1) break;
        from = at + call.length;
        // A word boundary before the call name, so `prefetch(` is not `fetch(`.
        if (at > 0 && /[A-Za-z0-9_$.]/.test(code[at - 1]) && !call.includes('.')) continue;
        let args = argsAt(code, at + call.length);
        // `fetch(url)` — resolve one level of indirection to the nearest
        // preceding assignment, which is the form all three real cases take.
        const bare = args.trim().match(/^([A-Za-z_$][\w$]*)\s*(?:,|$)/);
        if (bare) {
          const decl = new RegExp(`(?:const|let|var)\\s+${bare[1]}\\s*=([^;]*);`);
          const before = code.slice(Math.max(0, at - 1200), at).match(new RegExp(decl.source + '(?![\\s\\S]*' + decl.source + ')'));
          if (before) args += ' ' + before[1];
        }
        for (const host of externalHosts(args)) {
          violations.push({ file: rel, host, snippet: args.trim().slice(0, 90) });
        }
      }
    }
  }
  return violations;
}

describe('backend extracts, frontend displays', () => {
  it('makes no network call to a non-app origin outside the recorded allowlist', () => {
    const unexpected = scan().filter((v) => !(ALLOWED[v.file] ?? []).includes(v.host));
    expect(
      unexpected.map((v) => `${v.file} -> ${v.host}   [${v.snippet}]`),
      'a renderer reached a non-app origin. The backend ingests; the frontend displays.',
    ).toEqual([]);
  });

  it('never allowlists a journal or publishready path', () => {
    const smuggled = Object.keys(ALLOWED).filter((f) => PROTECTED.some((p) => f.startsWith(p)));
    expect(smuggled, 'the invariant was weakened for the screens it exists to protect').toEqual([]);
  });

  it('still sees the three known violations, so the scanner is not vacuously green', () => {
    // A scanner that finds nothing passes whether or not it works. These three
    // calls are real and are the known-good row of this batch.
    const found = scan().filter((v) => v.file.startsWith('features/citation-generator/'));
    expect(found.map((v) => v.host).sort()).toEqual([
      'api.crossref.org',
      'eutils.ncbi.nlm.nih.gov',
      'openlibrary.org',
    ]);
  });
});
