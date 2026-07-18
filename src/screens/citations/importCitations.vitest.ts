// Citation Manager import pipeline (Set 2b-i) — pure. No React, no network.
// citation-js is real here (the bibtex/ris plugins parse the fixtures).
import { describe, expect, it } from 'vitest';
import { planImport, detectFormat, splitBibtex, splitRis, normalizeToCitation } from './importCitations';
import { Citation } from './citationTypes';

const existing = (over: Partial<Citation> & { title?: string; year?: number; doi?: string }): Citation => ({
  id: over.id ?? 'lib-1',
  csl: {
    id: 's',
    type: 'article-journal',
    title: over.title ?? 'Existing',
    author: [{ family: 'Prior' }], // populated → a same-DOI import that only re-supplies authors adds nothing
    ...(over.year != null ? { issued: { year: over.year } } : {}),
    ...(over.doi ? { DOI: over.doi } : {}),
  },
  doi: over.doi ?? null,
  retracted: false,
  source: 'manual',
});

const BIB_GOOD_A = `@article{a2020, title={Alpha Study}, author={Doe, Jane}, year={2020}, doi={10.1/alpha}}`;
const BIB_GOOD_B = `@article{b2021, title={Beta Study}, author={Roe, Ann}, year={2021}, doi={10.1/beta}}`;
const BIB_BAD = `@article{broken, title={Unterminated, author={`; // malformed

describe('detectFormat', () => {
  it('maps extensions to formats', () => {
    expect(detectFormat('lib.bib')).toBe('bibtex');
    expect(detectFormat('X.BibTeX')).toBe('bibtex');
    expect(detectFormat('refs.ris')).toBe('ris');
    expect(detectFormat('data.json')).toBe('csl-json');
    expect(detectFormat('paper.pdf')).toBeNull();
  });
});

describe('splitters', () => {
  it('bibtex splits per entry and drops @comment/@string/@preamble', () => {
    const text = `@string{x = "y"}\n${BIB_GOOD_A}\n@comment{ignore}\n${BIB_GOOD_B}`;
    const parts = splitBibtex(text);
    expect(parts.length).toBe(2);
    expect(parts[0]).toContain('Alpha');
    expect(parts[1]).toContain('Beta');
  });
  it('ris splits per TY…ER record', () => {
    const ris = 'TY  - JOUR\nTI  - One\nER  - \nTY  - JOUR\nTI  - Two\nER  - \n';
    expect(splitRis(ris).length).toBe(2);
  });
});

describe('normalizeToCitation', () => {
  it('maps CSL-JSON date-parts + container-title, marks source imported', () => {
    const c = normalizeToCitation(
      { title: 'X', DOI: '10.1/x', issued: { 'date-parts': [[2019]] }, 'container-title': 'J. Test', author: [{ family: 'Ng', given: 'A' }] },
      0,
    );
    expect(c.source).toBe('imported');
    expect(c.csl.issued?.year).toBe(2019);
    expect(c.csl.containerTitle).toBe('J. Test');
    expect(c.doi).toBe('10.1/x');
  });
});

describe('planImport — per-entry defensive parse (pin b)', () => {
  it('one malformed entry never kills the batch: good/bad/good → 2 added, 1 failed', async () => {
    const text = `${BIB_GOOD_A}\n${BIB_BAD}\n${BIB_GOOD_B}`;
    const plan = await planImport(text, 'bibtex', []);
    expect(plan.added.length).toBe(2);
    expect(plan.failed.length).toBe(1);
    expect(plan.added.map((c) => c.csl.title).sort()).toEqual(['Alpha Study', 'Beta Study']);
    // the failure is inspectable — the raw + a reason are kept
    expect(plan.failed[0].raw.length).toBeGreaterThan(0);
    expect(plan.failed[0].error.length).toBeGreaterThan(0);
  });
});

describe('planImport — dedupe', () => {
  it('DOI already in the library → Tier-1 skipped, not added', async () => {
    const lib = [existing({ id: 'have', doi: '10.1/alpha', title: 'Alpha (already here)', year: 2020 })];
    const plan = await planImport(BIB_GOOD_A, 'bibtex', lib);
    expect(plan.added.length).toBe(0);
    expect(plan.skipped.length).toBe(1);
    expect(plan.skipped[0].match.id).toBe('have');
  });

  it('batch-internal (pin c): the SAME entry twice in one file → 1 added, 1 skipped', async () => {
    const text = `${BIB_GOOD_A}\n${BIB_GOOD_A}`;
    const plan = await planImport(text, 'bibtex', []);
    expect(plan.added.length).toBe(1);
    expect(plan.skipped.length).toBe(1); // the accumulator caught the second copy
  });

  it('same title different year both survive (conservative Tier-2)', async () => {
    const lib = [existing({ id: 'y19', title: 'Alpha Study', year: 2019 })];
    const plan = await planImport(BIB_GOOD_A, 'bibtex', lib); // Alpha Study 2020
    expect(plan.added.length).toBe(1);
    expect(plan.skipped.length).toBe(0);
    expect(plan.review.length).toBe(0);
  });
});

describe('planImport — enrichment routing (2b-iv)', () => {
  // a rich .bib entry (journal/volume/issue/pages) for the SAME DOI as a thin lib entry
  const RICH = `@article{r, title={Rich Paper}, author={Doe, Jane}, journal={J. Rest}, volume={12}, number={3}, pages={1--9}, year={2020}, doi={10.1/rich}}`;

  it('pin b: richer incoming on a THIN existing → enrich (NOT skipped), with the right missingFields', async () => {
    const lib = [existing({ id: 'thin', doi: '10.1/rich', title: 'Rich Paper', year: 2020 })]; // no journal/vol/issue/page
    const plan = await planImport(RICH, 'bibtex', lib);
    expect(plan.added.length).toBe(0);
    expect(plan.skipped.length).toBe(0);
    expect(plan.enrich.length).toBe(1);
    expect(plan.enrich[0].match.id).toBe('thin');
    expect(plan.enrich[0].missingFields).toEqual(expect.arrayContaining(['containerTitle', 'volume', 'issue', 'page']));
  });

  it('pin b: thinner/equal incoming on a RICH existing → skipped (true no-op dup, NOT offered)', async () => {
    const rich: Citation = {
      id: 'rich',
      csl: { id: 's', type: 'article-journal', title: 'Rich Paper', author: [{ family: 'Doe', given: 'Jane' }], issued: { year: 2020 }, DOI: '10.1/rich', containerTitle: 'J. Rest', volume: '12', issue: '3', page: '1-9' },
      doi: '10.1/rich',
      retracted: false,
      source: 'manual',
    };
    const thinBib = `@article{t, title={Rich Paper}, author={Doe, Jane}, year={2020}, doi={10.1/rich}}`;
    const plan = await planImport(thinBib, 'bibtex', [rich]);
    expect(plan.enrich.length).toBe(0);
    expect(plan.skipped.length).toBe(1);
    expect(plan.skipped[0].match.id).toBe('rich');
  });
});

describe('planImport — csl-json + cap', () => {
  it('parses a CSL-JSON array', async () => {
    const json = JSON.stringify([
      { title: 'J1', DOI: '10.1/j1', issued: { 'date-parts': [[2020]] } },
      { title: 'J2', DOI: '10.1/j2', issued: { 'date-parts': [[2021]] } },
    ]);
    const plan = await planImport(json, 'csl-json', []);
    expect(plan.added.length).toBe(2);
  });
  it('invalid JSON fails as one honest FailedEntry (the noted caveat)', async () => {
    const plan = await planImport('{ not valid', 'csl-json', []);
    expect(plan.added.length).toBe(0);
    expect(plan.failed.length).toBe(1);
    expect(plan.failed[0].error).toMatch(/JSON/i);
  });
  it('honors the cap and reports it (never silently truncates)', async () => {
    const items = Array.from({ length: 10 }, (_, i) => ({ title: `T${i}`, DOI: `10.1/c${i}`, issued: { 'date-parts': [[2000 + i]] } }));
    const plan = await planImport(JSON.stringify(items), 'csl-json', [], { cap: 4 });
    expect(plan.added.length).toBe(4);
    expect(plan.total).toBe(10);
    expect(plan.capped).toBe(true);
  });
});
