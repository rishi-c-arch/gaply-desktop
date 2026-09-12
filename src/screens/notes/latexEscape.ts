// Gaply — LaTeX escaping. One function, because there is only ever one right
// answer and two of them would drift.
//
// THIS IS THE HIGHEST-RISK CODE IN THE .tex EXPORTER, for the same reason the
// SQL `LIKE` wildcard defect was: the characters are not exotic. `_` and `%` and
// `&` appear in ordinary research prose — `p_value`, "80% of trials", "Smith &
// Jones" — and getting one wrong either fails the build with an error pointing
// at the wrong line, or compiles into something that silently means something
// else. `50%` unescaped comments out the rest of the line.
//
// WHAT IS DELIBERATELY *NOT* ESCAPED: math. A formula's TeX is the author's own
// LaTeX and is emitted verbatim; escaping it would destroy it. The walk keeps
// math in its own branch and never routes it through here.

/** The ten characters TeX reserves, and what each becomes in text mode.
 *  `\textbackslash{}` must be produced by the backslash rule BEFORE the brace
 *  rules would see the braces it introduces — so this is one pass, not ten. */
const TEXT_ESCAPES: Record<string, string> = {
  '\\': '\\textbackslash{}',
  '{': '\\{',
  '}': '\\}',
  $: '\\$',
  '&': '\\&',
  '#': '\\#',
  _: '\\_',
  '%': '\\%',
  '~': '\\textasciitilde{}',
  '^': '\\textasciicircum{}',
};

const TEXT_RE = /[\\{}$&#_%~^]/g;

/**
 * Escape a run of PROSE for LaTeX text mode.
 *
 * Single pass, so a replacement's own output is never re-escaped: `\` becomes
 * `\textbackslash{}` and its braces are not then turned into `\{\}`. That bug
 * is the reason this is a table lookup over one regex rather than a chain of
 * `.replace()` calls, which is the shape everybody writes first.
 */
export function escapeTex(text: string): string {
  return text.replace(TEXT_RE, (c) => TEXT_ESCAPES[c]);
}

/** Straight quotes → TeX's directional quotes, applied AFTER escaping so the
 *  backtick/apostrophe pairs it emits are not themselves escaped. Typographic
 *  only: it never changes which characters the reader sees. */
export function texQuotes(escaped: string): string {
  return escaped
    .replace(/"([^"]*)"/g, '``$1\'\'')
    .replace(/(^|[\s([])'/g, '$1`');
}

/** Prose → LaTeX, the normal path: escape, then prettify quotes. */
export const proseToTex = (text: string): string => texQuotes(escapeTex(text));

/** A LaTeX comment line — every newline re-prefixed so a multi-line note can
 *  never break out of the comment and become executable markup. */
export const texComment = (text: string): string =>
  text.split('\n').map((l) => `% ${l}`.trimEnd()).join('\n');

/** Sanitize a string for use as a BibTeX/LaTeX key or a filename inside the
 *  bundle: ASCII letters, digits, hyphen. Never empty. */
export const texSafeName = (s: string, fallback = 'item'): string =>
  s.normalize('NFKD').replace(/[^\w-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 60) || fallback;
