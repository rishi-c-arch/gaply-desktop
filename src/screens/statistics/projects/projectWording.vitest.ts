// The project screens' words (Research OS, Phase 5B-3a; decisions 133 and
// 134). Every project.* code Rust can send (projectCodes.json, written by
// src-tauri/tests/project_commands.rs from ProjectCode::ALL) has one plain
// sentence, the lock codes included, and nothing else does; decision 133's
// five sentences are here word for word; no sentence passes a verdict on the
// user's data; a refusal never shows its raw message, a JSON parse error's
// message included.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import type { StoredRunView } from './projectTypes';

import {
  DELETE_SENTENCE,
  ALL_PROJECTS_LIMIT,
  ALL_PROJECTS_LIMIT_BYTES,
  FALLBACK_SENTENCE,
  mb,
  NAME_WORDS,
  PROJECT_TEXT,
  PROJECT_WORDS,
  projectRefusal,
  projectSentence,
  PROJECT_LIMIT,
  PROJECT_LIMIT_BYTES,
  PROVENANCE_EXPORT_SENTENCE,
  REPORT_EXPORT_SENTENCE,
  RERUN_SENTENCE,
  RERUN_WORDS,
  rerunLines,
  SAVE_SENTENCE,
  savedOn,
  SAVED_TEXT,
  SETTINGS_SENTENCE,
  sizeSentence,
  totalsSentence,
} from './projectWording';

const CODES = JSON.parse(
  fs.readFileSync(path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/projectCodes.json'), 'utf8')
) as string[];

const err = (v: unknown) => ({ code: 'validation', message: JSON.stringify(v) });

/** Words that would pass a verdict on the data, or celebrate. */
const VERDICT = /\b(significant|correct|incorrect|accurate|inaccurate|reliable|unreliable|trustworthy|wrong|invalid|suspicious|fraud)\b|★|✓/i;

describe('every project code has one plain sentence', () => {
  it('for each code in projectCodes.json, and for no other', () => {
    expect(CODES).toHaveLength(55);
    expect(Object.keys(PROJECT_WORDS).sort()).toEqual([...CODES].sort());
    for (const code of CODES) {
      const s = projectSentence(err({ code, needed: 2097152, limit: 1048576 }));
      expect(s, code).not.toBe(FALLBACK_SENTENCE);
      expect(s, code).toMatch(/^[A-Z].*\.$/);
      expect(s, code).not.toContain('undefined');
      expect(s, code).not.toMatch(/project\.[a-z_]/);
      expect(s, code).not.toMatch(VERDICT);
    }
  });

  // 5C-1a (decisions 185 to 190): each rerun refusal in the prompt's words,
  // and none passes a verdict on either result.
  it('words the six rerun codes plainly and passes no verdict', () => {
    const want: Record<string, string> = {
      'project.rerun_unsupported':
        'This result was made by a method, or with settings, that this version of Gaply cannot run again, so nothing was run.',
      'project.rerun_not_succeeded': 'Only a result that finished can be run again.',
      'project.rerun_busy': 'Another run is going. Wait for it to finish, or cancel it, then run this again.',
      'project.rerun_refused': "This result's settings were not accepted this time, so nothing was run.",
      'project.rerun_in_progress': 'A run is still going in this project. Wait for it to finish, or cancel it, first.',
      'project.rerun_data_unreadable':
        'The copy of the data file kept in this project could not be read this time, so nothing was run.',
    };
    expect(CODES.filter((c) => c.startsWith('project.rerun_')).sort()).toEqual(Object.keys(want).sort());
    for (const [code, sentence] of Object.entries(want)) {
      const s = projectSentence(err({ code }));
      expect(s, code).toBe(sentence);
      expect(s, code).not.toMatch(/reproduced|verified|correct|failed to reproduce/i);
    }
  });

  it('words the three lock codes for what the user can do', () => {
    expect(projectSentence(err({ code: 'project.open_elsewhere' }))).toBe(
      'Another copy of Gaply has this project open, so it was not deleted. Close it there first.'
    );
    expect(projectSentence(err({ code: 'project.being_deleted' }))).toBe(
      'Another copy of Gaply is deleting this project, so it was not opened.'
    );
    expect(projectSentence(err({ code: 'project.lock_failed' }))).toMatch(/another copy of the app/);
  });

  // Decisions 163 and 174: the limits are fixed words, never formatted from
  // the refusal's limit, which is in binary bytes.
  it('gives a cap its limit as fixed words in decimal units', () => {
    expect(projectSentence(err({ code: 'project.project_cap', needed: 104857601, limit: 104857600 }))).toBe(
      'Saving this would take the project past its limit of 100 MB, so nothing was saved.'
    );
    expect(projectSentence(err({ code: 'project.all_projects_cap', needed: 1, limit: 10737418240 }))).toBe(
      'Saving this would take all projects together past their limit of 10 GB, so nothing was saved.'
    );
    // The same words whatever limit the refusal carries.
    expect(projectSentence(err({ code: 'project.project_cap', needed: 9, limit: 7 }))).toBe(
      'Saving this would take the project past its limit of 100 MB, so nothing was saved.'
    );
  });

  it("words a workspace refusal from the workspace table, not its own", () => {
    expect(projectSentence(err({ code: 'workspace.inactive', params: { expected: 'statistics' }, severity: 'error' }))).toBe(
      'This needs the Statistics workspace, which is not open.'
    );
  });
});

describe('a refusal never shows its raw message', () => {
  const SENTINEL = 'RAW_SENTINEL_k4 /Users/someone/secret.csv <b>x</b>';
  it.each([
    ['a plain Error', new Error(SENTINEL)],
    ['a string', SENTINEL],
    ['a message that is not JSON', { code: 'validation', message: SENTINEL }],
    ['JSON with an unknown code', err({ code: 'project.not_a_code', detail: SENTINEL })],
    ['JSON with a code that is not a string', err({ code: 7, detail: SENTINEL })],
    ['a JSON array', err([SENTINEL])],
    ['nothing at all', undefined],
  ])('%s gives the fallback sentence', (_name, e) => {
    const s = projectSentence(e);
    expect(s).toBe(FALLBACK_SENTENCE);
    expect(s).not.toContain('SENTINEL');
  });

  it('a JSON parse error, whose own message quotes the input, is never shown', () => {
    let parseMessage = '';
    try {
      JSON.parse(`{"code": "${SENTINEL}`);
    } catch (e) {
      parseMessage = (e as Error).message;
    }
    expect(projectRefusal({ message: `{"code": "${SENTINEL}` })).toBeNull();
    expect(projectSentence({ message: `{"code": "${SENTINEL}` })).toBe(FALLBACK_SENTENCE);
    expect(projectSentence(new Error(parseMessage))).toBe(FALLBACK_SENTENCE);
  });

  it('keeps only the code and the two numbers of a refusal', () => {
    expect(projectRefusal(err({ code: 'project.project_cap', needed: 2, limit: 1, extra: SENTINEL }))).toEqual({
      code: 'project.project_cap',
      needed: 2,
      limit: 1,
    });
  });
});

describe("decision 133's sentences, word for word", () => {
  it('save, report export, provenance export, delete and settings', () => {
    expect(SAVE_SENTENCE).toBe(
      "Saving keeps a copy of the data file, exactly as it is, in this app's folder on this computer. Gaply uploads none of it. You can delete the project at any time."
    );
    expect(REPORT_EXPORT_SENTENCE).toBe(
      "This report holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. The project's name is its title."
    );
    expect(PROVENANCE_EXPORT_SENTENCE).toBe(
      "This file holds fingerprints (SHA-256) of the data file, the analysis settings and the results, and none of the data itself. It does not hold the project's name."
    );
    expect(DELETE_SENTENCE).toBe(
      "Deleting removes this project and everything in it from this app's folder. It cannot remove copies in Time Machine or other backups, in file-system snapshots, or files you exported. The disk may keep traces until the space is reused."
    );
    expect(SETTINGS_SENTENCE).toBe(
      "Projects are kept in this app's folder on this computer. Other user accounts on this computer cannot open them, though administrators, the apps you run and your backups can. Gaply does not encrypt them: they rely on your computer's disk encryption (FileVault on a Mac, BitLocker on Windows, LUKS on Linux). Turn it on in your computer's settings if it is off."
    );
  });
});

describe("the screens' other words", () => {
  it('pass no verdict, the storage line included', () => {
    const all: string[] = [
      ...Object.values(NAME_WORDS),
      ...(Object.values(PROJECT_TEXT).filter((v) => typeof v === 'string') as string[]),
      PROJECT_TEXT.results(1),
      PROJECT_TEXT.results(3),
      FALLBACK_SENTENCE,
    ];
    for (const s of all) expect(s, s).not.toMatch(VERDICT);
    expect(totalsSentence(45056)).toBe('Your projects use 0.05 MB of their 10 GB limit. A project can hold up to 100 MB.');
  });

  // Decisions 163 and 174: decimal MB, as the Finder shows them, to two
  // decimals; 0 bytes is "0 MB", and anything above 0 and below 0.01 MB is
  // "under 0.01 MB".
  it('shows sizes in decimal MB', () => {
    expect(mb(0)).toBe('0 MB');
    expect(mb(1)).toBe('under 0.01 MB');
    expect(mb(9999)).toBe('under 0.01 MB');
    expect(mb(10000)).toBe('0.01 MB');
    expect(mb(70000)).toBe('0.07 MB');
    expect(mb(1048576)).toBe('1.05 MB');
    expect(mb(1234567890)).toBe('1,234.57 MB');
    expect(sizeSentence(0)).toBe('Size: 0 MB.');
    expect(sizeSentence(4096)).toBe('Size: under 0.01 MB.');
    expect(sizeSentence(70000)).toBe('Size: 0.07 MB.');
    expect(SAVED_TEXT.dataFile('csv', 500)).toBe('Data file: CSV file, under 0.01 MB.');
    expect(SAVED_TEXT.dataFile('xlsx', 52000)).toBe('Data file: XLSX file, 0.05 MB.');
    expect(totalsSentence(0)).toBe('Your projects use 0 MB of their 10 GB limit. A project can hold up to 100 MB.');
  });

  it('names limits no larger than the caps in code', () => {
    const sizes = JSON.parse(
      fs.readFileSync(path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/projectSizes.json'), 'utf8')
    ) as { projectCapBytes: number; allProjectsCapBytes: number };
    expect([PROJECT_LIMIT, ALL_PROJECTS_LIMIT]).toEqual(['100 MB', '10 GB']);
    expect([PROJECT_LIMIT_BYTES, ALL_PROJECTS_LIMIT_BYTES]).toEqual([100000000, 10000000000]);
    expect(PROJECT_LIMIT_BYTES).toBeLessThanOrEqual(sizes.projectCapBytes);
    expect(ALL_PROJECTS_LIMIT_BYTES).toBeLessThanOrEqual(sizes.allProjectsCapBytes);
  });
});

/* ---- reruns (Phase 5C-1b; decisions 191, 198 and 199) ---------------------- */

/** The words a sentence about a rerun must never hold (the step's rules): a
 *  rerun's result is compared byte for byte, never judged. */
const RERUN_VERDICT = /\b(reproduced|verified|correct|confirmed|failed to reproduce|matches the paper)\b/i;

/** Every string literal in a source, comments removed first: single and
 *  double quotes, and template literals with their placeholders kept. */
function literals(src: string): string[] {
  const code = src
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .map((l) => l.replace(/(^|\s)\/\/.*$/, '$1'))
    .join('\n');
  return code.match(/'(?:[^'\\\n]|\\.)*'|"(?:[^"\\\n]|\\.)*"|`(?:[^`\\]|\\.)*`/g) ?? [];
}

describe('the rerun words', () => {
  const run = (over: Partial<StoredRunView>): StoredRunView => ({
    storedRunId: 4,
    capabilityId: 'stats.describe',
    capabilityVersion: 1,
    state: 'succeeded',
    endCode: null,
    startedAt: 1,
    endedAt: 2,
    savedAt: 1792396400,
    agentVersion: '0.1.0',
    rerunOf: 1,
    comparison: 'identical',
    engineVersionDiffers: false,
    ...over,
  });
  const repeated = run({ storedRunId: 1, savedAt: 1792396100, rerunOf: null, comparison: null, engineVersionDiffers: null });

  it("are decisions 191's and 198's, word for word", () => {
    expect(RERUN_SENTENCE).toBe(
      'Run this again? Gaply runs the same method, with the same settings, on the copy of the data file kept in this project. When it finishes, the new run is saved in this project, beside the saved one, which does not change.'
    );
    expect(PROJECT_TEXT.runAgain).toBe('Run again');
    expect(PROJECT_TEXT.runningAgain).toBe('Running again…');
    expect(RERUN_WORDS.repeatsOn(1792396100)).toBe(`Repeats the result saved on ${savedOn(1792396100)}.`);
    expect(RERUN_WORDS.repeatsEarlier).toBe('Repeats an earlier result.');
    expect(RERUN_WORDS.identical).toBe('Identical, byte for byte, to that result.');
    expect(RERUN_WORDS.different).toBe(
      'Not identical, byte for byte, to that result. Both are kept; Gaply does not judge which is right.'
    );
    expect(RERUN_WORDS.notCompared).toBe('Gaply could not compare it with that result.');
    expect(RERUN_WORDS.engineDiffers).toBe("It ran on a different version of Gaply's engine.");
    expect(RERUN_WORDS.saved).toBe('The new run is saved in this project.');
    expect(RERUN_WORDS.notSaved).toBe('The new run was not saved.');
  });

  it('say which result a rerun repeats, then how it compares, and the engine line when it differs', () => {
    const first = RERUN_WORDS.repeatsOn(repeated.savedAt);
    expect(rerunLines(run({}), repeated)).toEqual([first, RERUN_WORDS.identical]);
    expect(rerunLines(run({ comparison: 'different', engineVersionDiffers: true }), repeated)).toEqual([
      first,
      RERUN_WORDS.different,
      RERUN_WORDS.engineDiffers,
    ]);
    expect(rerunLines(run({ comparison: 'not_compared', engineVersionDiffers: null }), repeated)).toEqual([first, RERUN_WORDS.notCompared]);
    expect(rerunLines(run({}), null)).toEqual([RERUN_WORDS.repeatsEarlier, RERUN_WORDS.identical]);
    for (const state of ['failed', 'cancelled', 'timed_out'] as const) {
      expect(rerunLines(run({ state, comparison: null, engineVersionDiffers: null }), repeated), state).toEqual([first]);
    }
    expect(rerunLines(repeated, null)).toEqual([]);
  });

  it('no sentence in projectWording.ts holds a verdict word', () => {
    const src = fs.readFileSync(path.join(__dirname, 'projectWording.ts'), 'utf8');
    const found = literals(src);
    expect(found.length).toBeGreaterThan(100);
    expect(found.some((l) => l.includes('Identical, byte for byte'))).toBe(true);
    expect(found.filter((l) => RERUN_VERDICT.test(l))).toEqual([]);
  });

  it('the scan sees a verdict word in a literal, and not in a comment', () => {
    expect(literals("const a = 'It was reproduced.'; // verified")).toEqual(["'It was reproduced.'"]);
    expect(literals('const b = "This result is correct."; /* confirmed */').filter((l) => RERUN_VERDICT.test(l))).toHaveLength(1);
    expect(literals('const c = `Run ${n} matches the paper.`;').filter((l) => RERUN_VERDICT.test(l))).toHaveLength(1);
    expect(literals("const d = \"Gaply's run failed to reproduce it.\";").filter((l) => RERUN_VERDICT.test(l))).toHaveLength(1);
    expect(literals("// It was verified.\n/* correct */ const e = 'plain';").filter((l) => RERUN_VERDICT.test(l))).toEqual([]);
    expect(RERUN_VERDICT.test('a new confirmation')).toBe(false);
  });
});
