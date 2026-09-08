// Gaply — the reference-manager import corpus (§11 D113).
//
// `importCitations.vitest.ts` has 13 tests and every fixture in it is a
// hand-written one-line string: `@article{a2020, title={Alpha Study}, ...}`.
// That is §11 D98's pattern — a check validated only against input we wrote,
// and we wrote it to be easy. It passed while the splitter was cutting real
// entries in half and inventing citations out of the offcuts.
//
// This file runs the REAL importer over files built to be hard. See
// `fixtures/README.md` for provenance: they are reconstructions of what Zotero,
// Mendeley and EndNote emit, not exports from a real install.
//
// THE RULE, and the reason the corpus exists: an entry that cannot be parsed
// must be REPORTED, never silently dropped — and the importer must never
// produce a citation the user did not have.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { join } from 'path';
import { planImport, splitBibtex, ImportPlan } from './importCitations';
import { Citation } from './citationTypes';

const read = (f: string) => readFileSync(join(__dirname, 'fixtures', f), 'utf8');
const titles = (cs: Citation[]) => cs.map((c) => c.csl.title);
const byTitle = (p: ImportPlan, t: string) => p.added.find((c) => c.csl.title === t);
const authors = (c?: Citation) =>
  (c?.csl.author ?? []).map((a) => [a.family, a.given].filter(Boolean).join(', '));

describe('import corpus — real-shaped .bib and .ris', () => {
  it('Zotero: every entry imports, with accents, multiline fields and macros resolved', async () => {
    const plan = await planImport(read('zotero.bib'), 'bibtex', []);
    expect(plan.failed).toEqual([]);
    expect(plan.added).toHaveLength(5);

    // LaTeX accent escapes become real characters.
    expect(authors(byTitle(plan, 'SMOTE for Learning from Imbalanced Data: Progress and Challenges'))[0])
      .toBe('Fernández, Alberto');
    expect(authors(byTitle(plan, 'An Introduction to Kernel-Based Learning Algorithms'))[0])
      .toBe('Müller, Klaus-Robert');

    // §11 D113 defect 3. `journal = jml` is an @string macro. The splitter
    // filtered @string out before parsing, so it could never resolve and the
    // journal vanished with no error — an entry that imports looking complete
    // and is missing the field every bibliography style prints.
    const glove = byTitle(plan, 'GloVe: Global Vectors for Word Representation');
    expect(glove?.csl.containerTitle).toBe('Journal of Machine Learning Research');
    const goem = byTitle(plan, 'GoEmotions: A Dataset of Fine-Grained Emotions');
    expect(goem?.csl.containerTitle).toBe(
      'Proceedings of the Association for Computational Linguistics',
    );

    // Case-protecting braces are stripped, not printed.
    expect(glove?.csl.title).not.toContain('{');
  });

  it('Mendeley: a UTF-8 BOM does not break the first entry', async () => {
    const text = read('mendeley-bom.bib');
    expect(text.charCodeAt(0)).toBe(0xfeff); // the hazard is really present
    const plan = await planImport(text, 'bibtex', []);
    expect(plan.failed).toEqual([]);
    expect(titles(plan.added)).toContain('SMOTE: Synthetic Minority Over-sampling Technique');
  });

  it('EndNote: BOM + CRLF + two-space tags + PY///', async () => {
    const text = read('endnote.ris');
    expect(text.includes('\r\n')).toBe(true);
    const plan = await planImport(text, 'ris', []);
    expect(plan.failed).toEqual([]);
    expect(plan.added).toHaveLength(2);
    expect(byTitle(plan, 'GloVe: Global Vectors for Word Representation')?.csl.issued?.year)
      .toBe(2014);
  });

  /* --------------------------------------------------------------------- *
   *  §11 D113 defect 1 — the worst thing this importer can do.
   * --------------------------------------------------------------------- */

  it('NEVER invents a citation from an @ inside a field value', async () => {
    // The abstract contains `@article{foo, title={bar}}`. The old splitter cut
    // the entry there: the real half failed to parse, and the trailing half
    // parsed CLEAN and was added to the library as a paper titled "bar".
    // A fabricated citation is worse than a failed import — the researcher
    // never sees it arrive.
    const plan = await planImport(read('edge.bib'), 'bibtex', []);
    expect(titles(plan.added)).not.toContain('bar');

    // And the real entry survives rather than being lost to the split.
    const real = byTitle(plan, 'A Paper Whose Abstract Quotes BibTeX');
    expect(real).toBeTruthy();
    expect(real?.doi).toBe('10.1/atsplit');
  });

  it('NEVER splits on an email followed by a brace group', async () => {
    // `note = {Corresponding author: nora@lab {group site}}` — `@lab {` matched
    // the old pattern because it allowed whitespace before the brace. Both
    // halves failed, so this entry was simply lost.
    const text = read('at-hazard.bib');
    expect(splitBibtex(text)).toHaveLength(2); // two entries, not three fragments
    const plan = await planImport(text, 'bibtex', []);
    expect(plan.failed).toEqual([]);
    expect(titles(plan.added).sort()).toEqual(['A Perfectly Normal Paper', 'Another Normal Paper']);
  });

  /* --------------------------------------------------------------------- *
   *  §11 D113 defects 2 and 4.
   * --------------------------------------------------------------------- */

  it('inherits crossref fields, so a valid entry is not marked malformed', async () => {
    // The child defines no year; its parent does. Without inheritance the entry
    // imports with year undefined, and `computeStatus` then calls a
    // perfectly well-formed reference "malformed metadata".
    const plan = await planImport(read('edge.bib'), 'bibtex', []);
    const child = byTitle(plan, 'A Chapter That Inherits Everything');
    expect(child?.csl.issued?.year).toBe(1999);
    expect(child?.csl.containerTitle).toBe('The Parent Book');
  });

  it('does not turn "and others" into an author called others', async () => {
    const plan = await planImport(read('edge.bib'), 'bibtex', []);
    const nested = byTitle(plan, 'GloVe: Global Vectors for Word Representation');
    expect(authors(nested)).toEqual(['Nested, Nora']);
    expect(authors(nested).join(' ')).not.toMatch(/others/i);
  });

  /* --------------------------------------------------------------------- *
   *  Shapes that already worked — pinned so a fix cannot quietly break them.
   * --------------------------------------------------------------------- */

  it('line endings change nothing', async () => {
    const lf = await planImport(read('edge.bib'), 'bibtex', []);
    const crlf = await planImport(read('edge-crlf.bib'), 'bibtex', []);
    expect(titles(crlf.added).sort()).toEqual(titles(lf.added).sort());
    expect(crlf.failed.length).toBe(lf.failed.length);
  });

  it('a last entry with no trailing newline still imports', async () => {
    const plan = await planImport(read('no-newline.bib'), 'bibtex', []);
    expect(plan.failed).toEqual([]);
    expect(titles(plan.added)).toEqual(['Last Entry No Newline']);
  });

  it('re-importing the same file adds nothing new', async () => {
    const text = read('zotero.bib');
    const first = await planImport(text, 'bibtex', []);
    const second = await planImport(text, 'bibtex', first.added);
    expect(second.added).toEqual([]);
    // The DOI-less entries land in `review` rather than `added`; the page
    // applies those under keep-both, which is why the VIEW duplicated them
    // (§11 D113 defect 5 — fixed in the page, not here).
    expect(second.skipped.length + second.review.length).toBe(first.added.length);
  });
});
