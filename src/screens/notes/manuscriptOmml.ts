// Gaply — LaTeX → OMML, for the journal-styled .docx export.
//
// A RESTRICTED PARSER WITH AN EXPLICIT FALLBACK, not a TeX engine. It covers
// the subset the editor accepts (see the math commit's list) and reports, per
// formula, whether it produced real OMML or fell back. Anything it cannot map
// degrades to the LaTeX source in a math font — the same stopgap the plain
// .docx export uses — because dropping an author's mathematics silently is the
// one outcome that is never acceptable.
//
// WHY A PARSER AT ALL. docx exposes builders (MathFraction, MathRadical,
// MathSum, …) but they are an API, not a reader: something has to turn
// `\frac{1}{x}` into calls. And the builders cover only part of the subset —
// there is no matrix, no cases, no equation array — so those are emitted as raw
// OMML through ImportedXmlComponent, which was proved to round-trip at the XML
// level before any of this was written.
//
// WHAT "SUPPORTED" MEANS HERE is deliberately narrower than what KaTeX renders.
// KaTeX is the editor's contract; this is the .docx renderer's. The gap is the
// fallback list, and it is recorded rather than discovered — see FALLBACK_NOTES
// and the tests that pin each entry.

/** Greek, operators and relations that map to a single Unicode point. Word's
 *  math font renders these directly, so no markup is needed for them. */
const SYMBOLS: Record<string, string> = {
  alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ε', varepsilon: 'ε',
  zeta: 'ζ', eta: 'η', theta: 'θ', vartheta: 'ϑ', iota: 'ι', kappa: 'κ',
  lambda: 'λ', mu: 'μ', nu: 'ν', xi: 'ξ', pi: 'π', rho: 'ρ', sigma: 'σ',
  tau: 'τ', upsilon: 'υ', phi: 'φ', varphi: 'φ', chi: 'χ', psi: 'ψ', omega: 'ω',
  Gamma: 'Γ', Delta: 'Δ', Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ', Pi: 'Π',
  Sigma: 'Σ', Upsilon: 'Υ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω',
  times: '×', cdot: '⋅', div: '÷', pm: '±', mp: '∓',
  leq: '≤', le: '≤', geq: '≥', ge: '≥', neq: '≠', ne: '≠',
  approx: '≈', equiv: '≡', sim: '∼', simeq: '≃', propto: '∝',
  infty: '∞', partial: '∂', nabla: '∇', forall: '∀', exists: '∃',
  in: '∈', notin: '∉', subset: '⊂', subseteq: '⊆', cup: '∪', cap: '∩',
  rightarrow: '→', to: '→', leftarrow: '←', Rightarrow: '⇒', leftrightarrow: '↔',
  ldots: '…', cdots: '⋯', dots: '…', top: '⊤', bot: '⊥', angle: '∠',
  prime: '′', circ: '∘', star: '⋆', bullet: '∙', oplus: '⊕', otimes: '⊗',
  perp: '⊥', parallel: '∥', ll: '≪', gg: '≫', mid: '∣',
};

/** Spacing commands — real in TeX, invisible in OMML. Consumed, not rendered. */
const SPACERS = new Set([',', ';', ':', '!', ' ', 'quad', 'qquad', 'thinspace', 'medspace', 'thickspace']);

/** Accents docx can build. `\hat{x}` → x with a combining hat. */
const ACCENTS: Record<string, string> = {
  hat: '̂', widehat: '̂', bar: '̄', overline: '̄',
  vec: '⃗', tilde: '̃', widetilde: '̃', dot: '̇', ddot: '̈',
};

/** Named operators rendered upright, e.g. \sin, \log, \max. */
const FUNCTIONS = new Set([
  'sin', 'cos', 'tan', 'sec', 'csc', 'cot', 'arcsin', 'arccos', 'arctan',
  'sinh', 'cosh', 'tanh', 'log', 'ln', 'exp', 'lim', 'max', 'min', 'sup', 'inf',
  'det', 'dim', 'ker', 'deg', 'gcd', 'arg', 'Pr',
]);

/** The big operators with optional limits. */
const NARY: Record<string, string> = {
  sum: '∑', prod: '∏', coprod: '∐', int: '∫', iint: '∬', iiint: '∭', oint: '∮',
  bigcup: '⋃', bigcap: '⋂', bigoplus: '⨁', bigotimes: '⨂',
};

/** Matrix-like environments and the delimiters they carry. */
const MATRIX_ENVS: Record<string, [string, string]> = {
  matrix: ['', ''], pmatrix: ['(', ')'], bmatrix: ['[', ']'],
  Bmatrix: ['{', '}'], vmatrix: ['|', '|'], Vmatrix: ['‖', '‖'],
  cases: ['{', ''], aligned: ['', ''], gathered: ['', ''], array: ['', ''], split: ['', ''],
};

/* --------------------------------- AST ---------------------------------- */

type Node =
  | { t: 'run'; text: string }
  | { t: 'fn'; name: string }
  | { t: 'frac'; num: Node[]; den: Node[] }
  | { t: 'rad'; deg: Node[] | null; body: Node[] }
  | { t: 'sup'; base: Node[]; sup: Node[] }
  | { t: 'sub'; base: Node[]; sub: Node[] }
  | { t: 'subsup'; base: Node[]; sub: Node[]; sup: Node[] }
  | { t: 'nary'; op: string; sub: Node[] | null; sup: Node[] | null; body: Node[] }
  | { t: 'delim'; open: string; close: string; body: Node[] }
  | { t: 'accent'; ch: string; body: Node[] }
  | { t: 'matrix'; open: string; close: string; rows: Node[][][] };

class Unsupported extends Error {}

/* -------------------------------- lexer --------------------------------- */

interface Tok { kind: 'cmd' | 'char' | '{' | '}' | '^' | '_' | '&' | '\\\\'; v: string }

function lex(src: string): Tok[] {
  const out: Tok[] = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === '\\') {
      if (src.startsWith('\\\\', i)) { out.push({ kind: '\\\\', v: '\\\\' }); i += 2; continue; }
      const m = /^\\([A-Za-z]+|.)/.exec(src.slice(i));
      if (!m) throw new Unsupported('stray backslash');
      out.push({ kind: 'cmd', v: m[1] });
      i += m[0].length;
      continue;
    }
    if (c === '{' || c === '}' || c === '^' || c === '_' || c === '&') { out.push({ kind: c as Tok['kind'], v: c }); i += 1; continue; }
    if (/\s/.test(c)) { i += 1; continue; } // TeX whitespace is not content
    out.push({ kind: 'char', v: c });
    i += 1;
  }
  return out;
}

/* -------------------------------- parser -------------------------------- */

class Parser {
  private i = 0;
  constructor(private toks: Tok[]) {}

  atEnd(): boolean { return this.i >= this.toks.length; }
  peek(): Tok | undefined { return this.toks[this.i]; }
  next(): Tok { const t = this.toks[this.i]; if (!t) throw new Unsupported('unexpected end'); this.i += 1; return t; }

  /** A group: `{…}`, or a single atom when unbraced (TeX's `x^2` rule). */
  group(): Node[] {
    const t = this.peek();
    if (!t) throw new Unsupported('expected a group');
    if (t.kind === '{') {
      this.next();
      const body: Node[] = [];
      while (this.peek() && this.peek()!.kind !== '}') body.push(...this.atom());
      if (!this.peek()) throw new Unsupported('unclosed {');
      this.next();
      return body;
    }
    // TeX's rule: an unbraced script argument is ONE atom and takes no scripts
    // of its own. Calling atom() here made `x_i^2` parse as `x_{i^2}`, because
    // the `i` greedily claimed the `^2` before `x` could.
    return this.primary();
  }

  /** Parse until end / `}` — the body of an environment or a whole formula. */
  seq(stop: (t: Tok) => boolean = () => false): Node[] {
    const out: Node[] = [];
    while (!this.atEnd() && !stop(this.peek()!)) out.push(...this.atom());
    return out;
  }

  private applyScripts(base: Node[]): Node[] {
    let sub: Node[] | null = null;
    let sup: Node[] | null = null;
    for (;;) {
      const t = this.peek();
      if (t?.kind === '^') { this.next(); sup = this.group(); continue; }
      if (t?.kind === '_') { this.next(); sub = this.group(); continue; }
      break;
    }
    if (sub && sup) return [{ t: 'subsup', base, sub, sup }];
    if (sup) return [{ t: 'sup', base, sup }];
    if (sub) return [{ t: 'sub', base, sub }];
    return base;
  }

  /** One item plus any scripts attached to it. */
  atom(): Node[] {
    return this.applyScripts(this.primary());
  }

  /** One item, WITHOUT scripts — the unit a `^`/`_` argument may be. */
  primary(): Node[] {
    const t = this.next();

    if (t.kind === 'char') return [{ t: 'run', text: t.v }];
    if (t.kind === '{') { this.i -= 1; return this.group(); }
    if (t.kind === '&' || t.kind === '\\\\') throw new Unsupported('alignment outside a matrix');
    if (t.kind === '^' || t.kind === '_') throw new Unsupported('script with no base');
    if (t.kind === '}') throw new Unsupported('unmatched }');

    // commands
    const c = t.v;
    if (SPACERS.has(c)) return [];
    if (c === 'frac' || c === 'dfrac' || c === 'tfrac') {
      return [{ t: 'frac', num: this.group(), den: this.group() }];
    }
    if (c === 'sqrt') {
      let deg: Node[] | null = null;
      if (this.peek()?.kind === 'char' && this.peek()!.v === '[') {
        this.next();
        const inner: Tok[] = [];
        while (this.peek() && !(this.peek()!.kind === 'char' && this.peek()!.v === ']')) inner.push(this.next());
        if (!this.peek()) throw new Unsupported('unclosed [');
        this.next();
        deg = new Parser(inner).seq();
      }
      return [{ t: 'rad', deg, body: this.group() }];
    }
    if (c in ACCENTS) return [{ t: 'accent', ch: ACCENTS[c], body: this.group() }];
    if (c in NARY) {
      let sub: Node[] | null = null;
      let sup: Node[] | null = null;
      for (;;) {
        const n = this.peek();
        if (n?.kind === '_') { this.next(); sub = this.group(); continue; }
        if (n?.kind === '^') { this.next(); sup = this.group(); continue; }
        break;
      }
      // The operand runs to the end of the current level. TeX's real rule is
      // subtler; this is the honest approximation and it is what the tests pin.
      const body = this.seq((n) => n.kind === '&' || n.kind === '\\\\');
      return [{ t: 'nary', op: NARY[c], sub, sup, body }];
    }
    if (FUNCTIONS.has(c)) return [{ t: 'fn', name: c }];
    if (c === 'text' || c === 'mathrm' || c === 'mathit' || c === 'mathbf' || c === 'operatorname') {
      const g = this.group();
      return [{ t: 'run', text: flatten(g) }];
    }
    if (c === 'left' || c === 'right') {
      // `\left(… \right)` — take the delimiter, parse to the matching \right.
      if (c === 'right') throw new Unsupported('\\right without \\left');
      const open = this.next().v;
      const body = this.seq((n) => n.kind === 'cmd' && n.v === 'right');
      if (this.atEnd()) throw new Unsupported('\\left without \\right');
      this.next();                       // consume \right
      const close = this.next().v;
      return [{ t: 'delim', open: open === '.' ? '' : open, close: close === '.' ? '' : close, body }];
    }
    if (c === 'begin') {
      const env = flatten(this.group());
      if (!(env in MATRIX_ENVS)) throw new Unsupported(`environment ${env}`);
      if (env === 'array') this.group();  // the column spec, which OMML has no use for
      const [open, close] = MATRIX_ENVS[env];
      const rows: Node[][][] = [[]];
      let cell: Node[] = [];
      for (;;) {
        const n = this.peek();
        if (!n) throw new Unsupported(`unclosed ${env}`);
        if (n.kind === 'cmd' && n.v === 'end') { this.next(); this.group(); break; }
        if (n.kind === '&') { this.next(); rows[rows.length - 1].push(cell); cell = []; continue; }
        if (n.kind === '\\\\') { this.next(); rows[rows.length - 1].push(cell); cell = []; rows.push([]); continue; }
        cell.push(...this.atom());
      }
      rows[rows.length - 1].push(cell);
      // `\\` at the very end leaves a trailing empty row; drop it.
      while (rows.length > 1 && rows[rows.length - 1].every((r) => r.length === 0)) rows.pop();
      return [{ t: 'matrix', open, close, rows }];
    }
    if (c in SYMBOLS) return [{ t: 'run', text: SYMBOLS[c] }];
    if (c === '{' || c === '}' || c === '%' || c === '$' || c === '&' || c === '#' || c === '_') {
      return [{ t: 'run', text: c }];
    }
    throw new Unsupported(`\\${c}`);
  }
}

const flatten = (ns: Node[]): string =>
  ns.map((n) => (n.t === 'run' ? n.text : n.t === 'fn' ? n.name : '')).join('');

/* ------------------------------ OMML output ------------------------------ */

const M = 'xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"';
const esc = (s: string): string =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

const run = (text: string, upright = false): string =>
  `<m:r>${upright ? '<m:rPr><m:sty m:val="p"/></m:rPr>' : ''}<m:t xml:space="preserve">${esc(text)}</m:t></m:r>`;

function emit(ns: Node[]): string {
  return ns.map(emitOne).join('');
}

function emitOne(n: Node): string {
  switch (n.t) {
    case 'run': return run(n.text);
    // An UPRIGHT RUN, not <m:func>. OMML's m:func expects the operand inside its
    // <m:e>, and this parser does not bind one (TeX does not delimit it either:
    // `\sin(x)` and `\sin x` are both legal). Emitting m:func with an empty
    // <m:e/> makes Word draw an empty argument placeholder box next to the name.
    // An upright run renders "sin(x)" exactly as written, and scripts still
    // attach correctly — `\log_2 n` becomes sSub(log, 2) followed by n.
    case 'fn': return run(n.name, true);
    case 'frac':
      return `<m:f><m:num>${emit(n.num)}</m:num><m:den>${emit(n.den)}</m:den></m:f>`;
    case 'rad':
      return n.deg
        ? `<m:rad><m:radPr><m:degHide m:val="0"/></m:radPr><m:deg>${emit(n.deg)}</m:deg><m:e>${emit(n.body)}</m:e></m:rad>`
        : `<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>${emit(n.body)}</m:e></m:rad>`;
    case 'sup': return `<m:sSup><m:e>${emit(n.base)}</m:e><m:sup>${emit(n.sup)}</m:sup></m:sSup>`;
    case 'sub': return `<m:sSub><m:e>${emit(n.base)}</m:e><m:sub>${emit(n.sub)}</m:sub></m:sSub>`;
    case 'subsup':
      return `<m:sSubSup><m:e>${emit(n.base)}</m:e><m:sub>${emit(n.sub)}</m:sub><m:sup>${emit(n.sup)}</m:sup></m:sSubSup>`;
    case 'accent':
      return `<m:acc><m:accPr><m:chr m:val="${esc(n.ch)}"/></m:accPr><m:e>${emit(n.body)}</m:e></m:acc>`;
    case 'delim':
      return `<m:d><m:dPr><m:begChr m:val="${esc(n.open)}"/><m:endChr m:val="${esc(n.close)}"/></m:dPr><m:e>${emit(n.body)}</m:e></m:d>`;
    case 'nary': {
      const pr = `<m:naryPr><m:chr m:val="${esc(n.op)}"/><m:limLoc m:val="undOvr"/>`
        + `<m:subHide m:val="${n.sub ? 0 : 1}"/><m:supHide m:val="${n.sup ? 0 : 1}"/></m:naryPr>`;
      return `<m:nary>${pr}<m:sub>${n.sub ? emit(n.sub) : ''}</m:sub>`
        + `<m:sup>${n.sup ? emit(n.sup) : ''}</m:sup><m:e>${emit(n.body)}</m:e></m:nary>`;
    }
    case 'matrix': {
      const cols = Math.max(...n.rows.map((r) => r.length));
      const mcPr = `<m:mcs>${`<m:mc><m:mcPr><m:count m:val="${cols}"/><m:mcJc m:val="center"/></m:mcPr></m:mc>`}</m:mcs>`;
      const body = `<m:m><m:mPr>${mcPr}</m:mPr>`
        + n.rows.map((r) => `<m:mr>${r.map((c) => `<m:e>${emit(c)}</m:e>`).join('')}</m:mr>`).join('')
        + '</m:m>';
      if (!n.open && !n.close) return body;
      return `<m:d><m:dPr><m:begChr m:val="${esc(n.open)}"/><m:endChr m:val="${esc(n.close)}"/></m:dPr><m:e>${body}</m:e></m:d>`;
    }
    default: return '';
  }
}

export interface OmmlResult {
  /** A complete `<m:oMath>` element, or null when the formula fell back. */
  omml: string | null;
  /** Why it fell back — the construct, for the recorded fallback list. */
  reason?: string;
}

/**
 * Convert one formula's TeX to an `<m:oMath>` element.
 *
 * Returns `{ omml: null, reason }` rather than throwing: a caller must always
 * be able to fall back to showing the LaTeX source, and a formula that cannot
 * be typeset is a rendering limit, never a reason to lose the mathematics.
 */
export function texToOmml(tex: string): OmmlResult {
  try {
    const p = new Parser(lex(tex));
    const nodes = p.seq();
    if (!p.atEnd()) return { omml: null, reason: 'trailing input' };
    const inner = emit(nodes);
    if (!inner) return { omml: null, reason: 'empty' };
    return { omml: `<m:oMath ${M}>${inner}</m:oMath>` };
  } catch (e) {
    return { omml: null, reason: e instanceof Unsupported ? e.message : 'parse error' };
  }
}

/** Constructs inside the editor's accepted subset that this renderer does NOT
 *  map, and which therefore reach the .docx as their LaTeX source in a math
 *  font. Recorded here so the limit is stated, and pinned by tests. */
export const FALLBACK_NOTES: ReadonlyArray<{ construct: string; why: string }> = [
  { construct: '\\begin{align} / \\begin{gather} / \\tag', why: 'display-only environments carrying equation numbering; OMML has no equation-number model, so a numbered display would lose its number silently' },
  { construct: '\\newcommand and other macro definitions', why: 'expansion needs a TeX engine; KaTeX has one, this renderer does not' },
  { construct: '\\ce (mhchem), \\ref, \\label', why: 'outside the editor subset already — listed so the two lists are read together' },
  { construct: 'anything not in the symbol, function, accent or operator tables', why: 'an unknown control sequence is refused rather than guessed at' },
];
