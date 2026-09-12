// Emit the golden LaTeX bundles that CI compiles with latexmk.
//
// WHY THIS EXISTS. No PDF engine ships with Gaply — that was a deliberate scope
// decision — so nothing in the app can tell us a generated .tex actually builds.
// Every other export here is verified by reading the artefact back: the .docx
// tests grep document.xml, and one was opened in Word. LaTeX has no equivalent
// unless we add one, and "a format we can never compile" is the definition of
// untestable. The app bundles no TeX engine; CI can.
//
// This is deliberately NOT a *.vitest.ts file, so it never runs in the ordinary
// suite. CI points its own vitest config at it (scripts/latex-goldens/
// vitest.config.ts), then hands the output to a TeX Live container.
import { describe, it, beforeAll } from 'vitest';
import { mkdirSync, writeFileSync, readFileSync } from 'fs';
import { dirname, join } from 'path';

import { ensureCslEngine, registerStyleXml, registerLocaleXml } from '../../src/screens/citations/cslEngine';
import { CslItem } from '../../src/screens/citations/citationTypes';
import { renderManuscriptCitations } from '../../src/screens/notes/manuscriptCitations';
import { buildLatexBundle } from '../../src/screens/notes/manuscriptLatex';
import { newManuscript, Manuscript, Scaffold, DEFAULT_LATEX_FORMAT } from '../../src/screens/notes/manuscriptModel';

const OUT = process.env.LATEX_GOLDEN_DIR || '/tmp/latex-goldens';
const CAT = JSON.parse(readFileSync('public/manuscripts/scaffolds.json', 'utf8')) as { scaffolds: Scaffold[] };

// An 8x8 solid PNG with CORRECT chunk CRCs, so \includegraphics has real bytes.
// NOT the 1x1 fixture that was here first: its IDAT CRC was wrong. pdflatex
// rejects that outright ("libpng: internal error") while Word accepts it and
// renders nothing — which is why the embedded image in the first Word hand-test
// looked blank. A corrupt fixture proves nothing about the exporter.
const PNG = Uint8Array.from(
  atob('iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR42mPQ9qnDihiGlgQA2mE9QQx+vG8AAAAASUVORK5CYII='),
  (c) => c.charCodeAt(0),
);

const LIB = new Map<string, CslItem>([
  ['refA', { id: 'refA', type: 'article-journal', title: 'Deep residual learning for image recognition', author: [{ family: 'He', given: 'Kaiming' }, { family: 'Zhang', given: 'Xiangyu' }], issued: { year: 2016 }, containerTitle: 'CVPR', page: '770-778' }],
  ['refB', { id: 'refB', type: 'article-journal', title: 'Attention is all you need', author: [{ family: 'Vaswani', given: 'Ashish' }], issued: { year: 2017 }, containerTitle: 'NeurIPS' }],
]);

/** A body that exercises every construct the walk can emit. If a scaffold
 *  compiles with THIS in it, the emitter is exercised end to end. */
const FULL: Record<string, string> = {
  abstract: 'We revisit normalisation when n is small. Accuracy rose in 80% of runs; p_value < 0.05 throughout.',
  keywords: 'normalisation\nsmall samples\nreproducibility',
  introduction:
    'Residual networks [[cite:refA]] changed the field, and attention [[cite:refB]] changed it again. '
    + 'Our data are at [the project archive](https://example.org/archive/2026?a=1&b=2) in full.\n\n'
    + 'We make **three** contributions, all *reproducible*, and none requiring ~ or ^ or #1 or {braces} or 100% of anything.',
  methods:
    '## Participants\n\nForty volunteers (Smith \\& Jones, 2020).\n\n'
    + '| Group | n | Age |\n|:---|---:|:---:|\n| Control | 20 | 24.1 |\n| Treated | 20 | 23.8 |\n\n'
    + 'Inline `code_sample` stays monospace.\n\n'
    + '> A blockquote, indented and ruled.\n\n'
    + '```\nif (x_1 > 50%) { run_all(); }\n```\n\n'
    + '![](gaply-image://abc123.png)',
  results:
    'Accuracy rose by 2.1 points, replicating [[cite:refA]]. The estimator is '
    + '[[math:\\hat{\\beta} = (X^{\\top}X)^{-1}X^{\\top}y]] and the loss integrates as\n\n'
    + '[[math-block:\\int_0^1 x^2\\,dx = \\frac{1}{3}]]\n\n'
    + 'with the block form\n\n'
    + '[[math-block:\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}]]\n\n'
    + 'and a piecewise definition [[math:\\begin{cases} x & x \\ge 0 \\\\ -x & x < 0 \\end{cases}]].',
  discussion: 'The effect is small but consistent across every subgroup we measured.',
  conclusion: 'Normalisation matters most when data are scarce.',
};

const write = async (name: string, m: Manuscript, scaffold: Scaffold, library: Map<string, CslItem>, style: string) => {
  const cites = await renderManuscriptCitations(m.sections, style, library);
  const bundle = await buildLatexBundle(
    m, scaffold, scaffold.latexFormat ?? DEFAULT_LATEX_FORMAT, cites, undefined,
    { readImage: async () => ({ data: PNG, mime: 'image/png' }) },
  );
  bundle.files.forEach((contents, path) => {
    const full = join(OUT, name, path);
    mkdirSync(dirname(full), { recursive: true });
    writeFileSync(full, contents as never);
  });
};

const manuscript = (scaffold: Scaffold, bodies: Record<string, string>): Manuscript => {
  const m = newManuscript('golden', scaffold);
  m.title = 'Layer Normalisation & Small-Sample Regimes: 100% of the Story';
  m.authors = 'Ada Lovelace, Alan Turing';
  m.affiliations = '¹Analytical Engine Lab · ²Bletchley Park';
  m.correspondingAuthor = 'ada@example.edu';
  for (const [k, v] of Object.entries(bodies)) { const s = m.sections.find((x) => x.key === k); if (s) s.body = v; }
  return m;
};

beforeAll(async () => {
  mkdirSync(OUT, { recursive: true });
  await ensureCslEngine();
  await registerLocaleXml('en-US', readFileSync('public/csl/locales/locales-en-US.xml', 'utf8'));
  for (const id of ['ieee', 'apa']) await registerStyleXml(id, readFileSync(`public/csl/styles/${id}.csl`, 'utf8'));
});

describe('emit golden LaTeX bundles', () => {
  it('one per scaffold, fully loaded', async () => {
    for (const scaffold of CAT.scaffolds) {
      await write(scaffold.id, manuscript(scaffold, FULL), scaffold, LIB, 'ieee');
    }
  });

  it('edge cases that have broken other exporters', async () => {
    const generic = CAT.scaffolds.find((s) => s.id === 'imrad-generic')!;

    // every citation dangling: the bibliography is empty and the author's
    // hand-written References text is the only reference text that exists
    await write('edge-all-dangling',
      manuscript(generic, { introduction: 'Prior work [[cite:gone]] established it.', references: 'Smith, J. (2020). A hand-typed entry. Journal of Things, 4(2), 10–22.' }),
      generic, new Map(), 'apa');

    // nothing written at all — a title page and no sections
    await write('edge-empty', manuscript(generic, {}), generic, new Map(), 'apa');

    // prose that is nothing but the characters TeX reserves
    await write('edge-reserved',
      manuscript(generic, { introduction: 'All of them: & % $ # _ { } ~ ^ \\ and again 100% & #1 _x_ {y} ~z ^w' }),
      generic, new Map(), 'apa');

    // author-date rather than numbered, to exercise the other marker shape
    await write('edge-author-date',
      manuscript(generic, { introduction: 'As shown [[cite:refA]] and later [[cite:refB]].' }),
      generic, LIB, 'apa');
  });
});
