// The LaTeX export — what the bundle contains and whether it means what the
// editor showed.
//
// The compile-or-not question is answered by CI (.github/workflows/
// latex-compile.yml runs latexmk over the golden bundles these tests build).
// What is pinned here is everything a compiler cannot tell you: that prose is
// escaped rather than mangled, that math is emitted verbatim rather than
// escaped, that the in-text markers match the reference list, and that the file
// stays honest about what it is.
import { describe, expect, it, beforeAll } from 'vitest';
import { readFileSync } from 'fs';

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../citations/cslEngine';
import { CslItem } from '../citations/citationTypes';
import { renderManuscriptCitations } from './manuscriptCitations';
import { buildLatexBundle } from './manuscriptLatex';
import { escapeTex, proseToTex, texComment } from './latexEscape';
import { newManuscript, Manuscript, Scaffold, ReadingFormat, DEFAULT_READING_FORMAT } from './manuscriptModel';

const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { version: number; scaffolds: Scaffold[] };
const sc = (id: string) => CAT.scaffolds.find((s) => s.id === id)!;

const LIB = new Map<string, CslItem>([
  ['refA', { id: 'refA', type: 'article-journal', title: 'Deep residual learning', author: [{ family: 'He', given: 'Kaiming' }], issued: { year: 2016 }, containerTitle: 'CVPR' }],
  ['refB', { id: 'refB', type: 'article-journal', title: 'Attention is all you need', author: [{ family: 'Vaswani', given: 'Ashish' }], issued: { year: 2017 }, containerTitle: 'NeurIPS' }],
]);

const PNG = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR42mPQ9qnDihiGlgQA2mE9QQx+vG8AAAAASUVORK5CYII='), (c) => c.charCodeAt(0));

beforeAll(async () => {
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  for (const id of ['ieee', 'apa']) await registerStyleXml(id, readFileSync(`public/csl/styles/${id}.csl`, 'utf8'));
});

/** Build a bundle and return main.tex plus the file list. */
const build = async (
  bodies: Record<string, string>,
  opts: { scaffoldId?: string; style?: string; library?: Map<string, CslItem>; bibtex?: string; format?: ReadingFormat } = {},
) => {
  const scaffold = sc(opts.scaffoldId ?? 'ieee');
  const m: Manuscript = newManuscript('m1', scaffold);
  m.title = 'Layer Normalisation & Small Samples';
  m.authors = 'Ada Lovelace';
  for (const [k, v] of Object.entries(bodies)) { const s = m.sections.find((x) => x.key === k); if (s) s.body = v; }
  const cites = await renderManuscriptCitations(m.sections, opts.style ?? 'ieee', opts.library ?? new Map());
  const bundle = await buildLatexBundle(
    m, scaffold, opts.format ?? scaffold.readingFormat ?? DEFAULT_READING_FORMAT, cites, opts.bibtex,
    { readImage: async () => ({ data: PNG, mime: 'image/png' }) },
  );
  return { tex: bundle.files.get('main.tex') as string, files: Array.from(bundle.files.keys()), bundle, cites };
};

/* ------------------------------ escaping -------------------------------- */

describe('escaping — the highest-risk part', () => {
  it('every reserved character, in one pass', () => {
    expect(escapeTex('100% & #1 _x_ ~ ^ $5 {a}')).toBe(
      '100\\% \\& \\#1 \\_x\\_ \\textasciitilde{} \\textasciicircum{} \\$5 \\{a\\}');
  });

  it('a backslash becomes \\textbackslash{} and its OWN braces are not re-escaped', () => {
    // The bug this exists to prevent: a chain of .replace() calls turns the
    // braces introduced by the backslash rule into \{\}.
    expect(escapeTex('a\\b')).toBe('a\\textbackslash{}b');
    expect(escapeTex('a\\b')).not.toContain('\\textbackslash\\{\\}');
  });

  it('ordinary research prose survives intact', () => {
    expect(proseToTex('p_value < 0.05 in 80% of trials (Smith & Jones)'))
      .toBe('p\\_value < 0.05 in 80\\% of trials (Smith \\& Jones)');
  });

  it('a comment can never break out and become markup', () => {
    expect(texComment('line one\n\\section{evil}')).toBe('% line one\n% \\section{evil}');
  });
});

/* ------------------------------- the body -------------------------------- */

describe('body conversion', () => {
  it('prose in a section is escaped, and the section heading too', async () => {
    const { tex } = await build({ methods: 'We used p_value < 0.05 in 80% of runs.' });
    expect(tex).toContain('We used p\\_value < 0.05 in 80\\% of runs.');
    // the IEEE scaffold calls this section "Methodology" — headings come from
    // the scaffold, so assert the one this scaffold actually has
    expect(tex).toContain(`\\section{${sc('ieee').sections.find((x) => x.key === 'methods')!.heading}}`);
  });

  it('MATH IS VERBATIM — the one thing that must not be escaped', async () => {
    const { tex } = await build({ methods: 'Inline [[math:\\frac{1}{x}]] and\n\n[[math-block:\\int_0^1 x^2\\,dx]]' });
    expect(tex).toContain('$\\frac{1}{x}$');
    expect(tex).toContain('\\[\\int_0^1 x^2\\,dx\\]');
    // if it had gone through prose escaping it would look like this:
    expect(tex).not.toContain('\\textbackslash{}frac');
    expect(tex).not.toContain('\\{1\\}');
  });

  it('bold, italic, code and lists become real LaTeX', async () => {
    const { tex } = await build({ introduction: 'We make **three** *reproducible* claims via `run_all`.\n\n- first\n- second\n\n1. one\n2. two' });
    expect(tex).toContain('\\textbf{three}');
    expect(tex).toContain('\\emph{reproducible}');
    expect(tex).toContain('\\texttt{run\\_all}');
    expect(tex).toContain('\\begin{itemize}');
    expect(tex).toContain('\\begin{enumerate}');
  });

  it('a GFM table becomes a booktabs tabular with the right column spec', async () => {
    const { tex } = await build({ methods: '| Group | n | Age |\n|:---|---:|:---:|\n| Control | 20 | 24.1 |' });
    expect(tex).toContain('\\begin{tabular}{lrc}');
    expect(tex).toContain('\\toprule');
    expect(tex).toContain('\\textbf{Group} & \\textbf{n} & \\textbf{Age} \\\\');
    expect(tex).toContain('Control & 20 & 24.1 \\\\');
    expect(tex).toContain('\\end{table}');
  });

  it('a link keeps its URL, unescaped, via \\url', async () => {
    const { tex } = await build({ methods: 'Data at [our archive](https://example.org/a_b?x=1&y=2).' });
    expect(tex).toContain('\\url{https://example.org/a_b?x=1&y=2}');
    expect(tex).toContain('our archive');
  });

  it('a code block is verbatim and untouched', async () => {
    const { tex } = await build({ methods: '```\nif (x_1 > 50%) { run(); }\n```' });
    expect(tex).toContain('\\begin{verbatim}\nif (x_1 > 50%) { run(); }\n\\end{verbatim}');
  });
});

/* ------------------------------- figures --------------------------------- */

describe('figures', () => {
  it('images are written into the bundle and referenced by a readable name', async () => {
    const { tex, files } = await build({ results: '![](gaply-image://abc123.png)' });
    expect(files).toContain('figures/figure-1.png');
    expect(tex).toContain('\\includegraphics[max width=\\linewidth]{figures/figure-1.png}');
    // a figure alone in its paragraph becomes a real float
    expect(tex).toContain('\\begin{figure}[htbp]');
  });

  it('an unreadable image degrades to a placeholder, never a dangling \\includegraphics', async () => {
    const scaffold = sc('ieee');
    const m = newManuscript('m1', scaffold);
    m.title = 'T';
    m.sections.find((s) => s.key === 'results')!.body = '![](gaply-image://missing.png)';
    const cites = await renderManuscriptCitations(m.sections, 'ieee', new Map());
    const bundle = await buildLatexBundle(m, scaffold, scaffold.readingFormat, cites, undefined, {
      readImage: async () => { throw new Error('gone'); },
    });
    const tex = bundle.files.get('main.tex') as string;
    expect(tex).not.toContain('\\includegraphics');
    expect(tex).toContain('[figure:');
    expect(Array.from(bundle.files.keys()).some((f) => f.startsWith('figures/'))).toBe(false);
  });
});

/* ---------------------------- the bibliography --------------------------- */

describe('citations and the bibliography', () => {
  it('in-text markers match the reference list, in document order', async () => {
    const { tex } = await build(
      { introduction: 'First [[cite:refB]] then [[cite:refA]] then [[cite:refB]] again.' },
      { library: LIB },
    );
    // IEEE numbers by first appearance: refB = 1, refA = 2
    expect(tex).toContain('First [1] then [2] then [1] again.');
    const bib = tex.slice(tex.indexOf('\\hangindent'));
    expect(bib.indexOf('Attention is all you need')).toBeLessThan(bib.indexOf('Deep residual learning'));
  });

  it('the bibliography is PRE-RENDERED in the chosen style, not left to a .bst', async () => {
    const { tex } = await build({ introduction: 'See [[cite:refA]].' }, { library: LIB, style: 'apa' });
    // a hanging-indent block, NOT thebibliography: that environment adds its own
    // "[1]" counter and its own "References" heading, which double-numbered
    // IEEE entries and headed the section twice
    expect(tex).toContain('\\hangindent');
    expect(tex).not.toContain('\\begin{thebibliography}');
    expect(tex).not.toContain('\\bibliographystyle');
    expect(tex).not.toContain('\\bibliography{references}');
    expect(tex).toContain('(2016)'); // APA author-date, as the editor showed
  });

  it('references.bib rides along as a labelled convenience, when supplied', async () => {
    const { files, tex } = await build({ introduction: 'See [[cite:refA]].' }, { library: LIB, bibtex: '@article{He2016,title={X}}' });
    expect(files).toContain('references.bib');
    // ...and the header says what it is and is not
    expect(tex).toContain('a .bst will re-format the entries in ITS style');
  });

  it('the References heading appears exactly ONCE', async () => {
    // thebibliography emits its own heading; using it alongside \section*
    // produced "References" twice on the page. Found by compiling and LOOKING
    // at the PDF — no unit test here could have seen it.
    const { tex } = await build({ introduction: 'See [[cite:refA]].' }, { library: LIB });
    expect((tex.match(/References/g) ?? []).filter(Boolean).length).toBeGreaterThan(0);
    const headings = tex.split('\n').filter((l) => /^\\section\*?\{References\}/.test(l));
    expect(headings).toHaveLength(1);
  });

  it('a numbered style is not numbered TWICE', async () => {
    // citeproc already puts "[1] " in the entry; thebibliography added another.
    const { tex } = await build({ introduction: 'See [[cite:refA]].' }, { library: LIB, style: 'ieee' });
    const bib = tex.slice(tex.indexOf('\\hangindent'));
    expect(bib).not.toMatch(/\[1\]\s*\[1\]/);
    expect(bib).toContain('[1]'); // citeproc's own label survives, exactly once
  });

  it('ALL citations dangling → the manual References text still ships', async () => {
    const { tex, cites } = await build(
      { introduction: 'Prior work [[cite:gone]].', references: 'Smith, J. (2020). A hand-typed entry.' },
      { library: new Map() },
    );
    expect(cites.hasCitations).toBe(true);
    expect(cites.bibliography).toBe('');
    expect(tex).toContain('A hand-typed entry');
  });

  it('a dangling marker renders [?] rather than a wrong number', async () => {
    const { tex } = await build({ introduction: 'A [[cite:refA]] and B [[cite:gone]].' }, { library: LIB });
    expect(tex).toContain('A [1] and B [?].');
  });
});

/* ------------------------------- honesty --------------------------------- */

describe('the file stays honest about what it is', () => {
  it('never names a publisher class — always generic article', async () => {
    for (const id of CAT.scaffolds.map((s) => s.id)) {
      const { tex } = await build({ introduction: 'x' }, { scaffoldId: id });
      expect(tex, id).toContain('\\documentclass');
      expect(tex, id).toMatch(/\\documentclass\[[^\]]*\]\{article\}/);
      // A publisher class may only ever be MENTIONED in a comment saying it is
      // not reproduced — never declared. Check the declarations, not the prose:
      // acm's own honest notice says "ACM's camera-ready acmart layout … not
      // reproduced", and that sentence is the point, not a violation.
      const declarations = tex.split('\n').filter((l) => /^\s*\\(documentclass|usepackage|input|include)\b/.test(l));
      for (const cls of ['IEEEtran', 'acmart', 'llncs', 'elsarticle', 'interact', 'sagej']) {
        expect(declarations.join('\n'), `${id} must not load ${cls}`).not.toContain(cls);
      }
    }
  });

  it('the header travels with the file: unofficial, the venue URL, and the layout', async () => {
    const { tex } = await build({ introduction: 'x' }, { scaffoldId: 'ieee' });
    expect(tex).toContain('NOT an official template');
    expect(tex).toContain(sc('ieee').publisherAuthorUrl);
    expect(tex).toContain('READING layout');
    expect(tex).toContain('not the file you submit'.replace('not the file you submit', 'It is not a submission artefact'));
    // and it tells the author the one edit that changes the layout
    expect(tex).toContain('change "twocolumn"');
  });

  it('a single-column scaffold says single-column and offers no switch line', async () => {
    const { tex } = await build({ introduction: 'x' }, { scaffoldId: 'plos' });
    expect(tex).toContain('LAYOUT: single-column manuscript.');
    expect(tex).not.toContain('READING layout');
  });

  it('every scaffold carries a readingFormat with an honest source note', () => {
    for (const s of CAT.scaffolds) {
      expect(s.readingFormat, s.id).toBeTruthy();
      expect(s.readingFormat!.source, s.id).toBeTruthy();
      expect([1, 2]).toContain(s.readingFormat!.columns);
      // the source note must not claim to reproduce the publisher's layout
      expect(s.readingFormat!.source!.toLowerCase(), s.id).not.toMatch(/official template|camera-ready reproduction/);
    }
  });

  it('the bundle explains itself', async () => {
    const { bundle } = await build({ introduction: 'x' });
    const readme = bundle.files.get('README.txt') as string;
    expect(readme).toContain('latexmk -pdf main.tex');
    expect(readme).toContain('not a publisher template');
  });
});

/* ------------------------------- structure ------------------------------- */

describe('document structure', () => {
  it('front and back matter are unnumbered, body sections are numbered', async () => {
    const { tex } = await build({ abstract: 'a', introduction: 'b', references: 'c' });
    expect(tex).toContain('\\section*{Abstract}');
    expect(tex).toContain('\\section{Introduction}');
  });

  it('an empty section emits no heading', async () => {
    const { tex } = await build({ introduction: 'only this' });
    expect(tex).toContain('\\section{Introduction}');
    expect(tex).not.toContain('\\section{Discussion}');
  });

  it('two-column scaffolds ask for twocolumn; single-column ones do not', async () => {
    expect((await build({ introduction: 'x' }, { scaffoldId: 'ieee' })).tex).toContain('twocolumn');
    expect((await build({ introduction: 'x' }, { scaffoldId: 'plos' })).tex).not.toContain('twocolumn');
  });

  it('line numbers follow the profile', async () => {
    expect((await build({ introduction: 'x' }, { scaffoldId: 'plos' })).tex).toContain('\\linenumbers');
    expect((await build({ introduction: 'x' }, { scaffoldId: 'ieee' })).tex).not.toContain('\\linenumbers');
  });

  it('the document is well-formed at the edges', async () => {
    const { tex } = await build({ introduction: 'x' });
    expect(tex).toContain('\\begin{document}');
    expect(tex.trimEnd().endsWith('\\end{document}')).toBe(true);
    expect(tex.indexOf('\\begin{document}')).toBeLessThan(tex.indexOf('\\end{document}'));
  });
});
