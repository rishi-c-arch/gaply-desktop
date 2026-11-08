// One codeSentence (Research OS, Phase 8U-2, decision 539): projectWording.ts
// holds it, exported beside projectSentence; the project page imports it and
// defines none of its own. And what it says: a code's sentence, or the fallback
// for none or a code this table does not know.
import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import * as wording from './projectWording';
import type { ProjectCode } from './projectTypes';

const code = (src: string) => src.replace(/\/\*[\s\S]*?\*\//g, '').replace(/(^|\s)\/\/.*$/gm, '$1');
const page = code(fs.readFileSync(path.join(__dirname, 'ProjectPage.tsx'), 'utf8'));
const words = code(fs.readFileSync(path.join(__dirname, 'projectWording.ts'), 'utf8'));

describe('one codeSentence (decision 539)', () => {
  it('projectWording.ts exports it', () => {
    expect(words).toMatch(/export const codeSentence\b|export function codeSentence\b/);
    expect(typeof (wording as Record<string, unknown>).codeSentence).toBe('function');
  });

  it('the page defines none and imports it from projectWording', () => {
    expect(page).not.toMatch(/(const|function|let)\s+codeSentence\b/);
    expect(page).toMatch(/import \{[^}]*\bcodeSentence\b[^}]*\} from '\.\/projectWording';/);
  });

  it("gives a code's sentence, and the fallback for none or an unknown code", () => {
    const codeSentence = (wording as unknown as { codeSentence: (c: ProjectCode | null) => string }).codeSentence;
    expect(codeSentence('project.not_open')).toBe(wording.PROJECT_WORDS['project.not_open']({ code: 'project.not_open' }));
    expect(codeSentence(null)).toBe(wording.FALLBACK_SENTENCE);
    expect(codeSentence('project.no_such' as ProjectCode)).toBe(wording.FALLBACK_SENTENCE);
  });
});
