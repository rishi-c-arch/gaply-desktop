// Gaply — a project's name, checked on the screen before any call (Research
// OS, Phase 5B-3a; decision 121). The rule is Rust's
// (gaply_core::research_project::ProjectTitle::from_typed): trimmed, then 1
// to 100 characters, with no control character and no invisible formatting
// character. Rust stays the boundary: it checks again on every Save.
//
// Mirrored exactly, not with JavaScript's built-ins, which differ:
// * trimming strips Rust's char::is_whitespace (Unicode White_Space), which
//   includes U+0085 and leaves U+FEFF, where String.prototype.trim does the
//   opposite;
// * a character is a Unicode code point, as Rust's chars().count(), not a
//   UTF-16 unit;
// * a control character is Rust's char::is_control (category Cc).
// Both sides read the same test vectors
// (src-tauri/gaply-core/tests/fixtures/research_project/name_vectors.json).

export const NAME_MAX_CHARS = 100;

/** Why a typed name is refused. */
export type NameRefusal = 'empty' | 'too_long' | 'hidden_character';

export type NameCheck = { ok: true; name: string } | { ok: false; reason: NameRefusal };

/** Rust's char::is_whitespace: Unicode White_Space. */
function isWhiteSpace(c: number): boolean {
  return (
    (c >= 0x09 && c <= 0x0d) ||
    c === 0x20 ||
    c === 0x85 ||
    c === 0xa0 ||
    c === 0x1680 ||
    (c >= 0x2000 && c <= 0x200a) ||
    c === 0x2028 ||
    c === 0x2029 ||
    c === 0x202f ||
    c === 0x205f ||
    c === 0x3000
  );
}

/** Rust's char::is_control: category Cc. */
function isControl(c: number): boolean {
  return c <= 0x1f || (c >= 0x7f && c <= 0x9f);
}

/** research_project::is_invisible_format, code point for code point. */
function isInvisibleFormat(c: number): boolean {
  const ranges: Array<[number, number]> = [
    [0x00ad, 0x00ad],
    [0x034f, 0x034f],
    [0x0600, 0x0605],
    [0x061c, 0x061c],
    [0x06dd, 0x06dd],
    [0x070f, 0x070f],
    [0x0890, 0x0891],
    [0x08e2, 0x08e2],
    [0x115f, 0x1160],
    [0x180e, 0x180e],
    [0x200b, 0x200f],
    [0x2028, 0x202e],
    [0x2060, 0x2064],
    [0x2066, 0x206f],
    [0x3164, 0x3164],
    [0xfeff, 0xfeff],
    [0xffa0, 0xffa0],
    [0xfff9, 0xfffb],
    [0x110bd, 0x110bd],
    [0x110cd, 0x110cd],
    [0x13430, 0x1343f],
    [0x1bca0, 0x1bca3],
    [0x1d173, 0x1d17a],
    [0xe0001, 0xe0001],
    [0xe0020, 0xe007f],
  ];
  return ranges.some(([lo, hi]) => c >= lo && c <= hi);
}

/** The code points of `s`; a lone surrogate is -1, which no rule admits:
 *  it cannot be sent as UTF-8. */
function codePoints(s: string): number[] {
  const out: number[] = [];
  for (let i = 0; i < s.length; i++) {
    const hi = s.charCodeAt(i);
    if (hi >= 0xd800 && hi <= 0xdbff && i + 1 < s.length) {
      const lo = s.charCodeAt(i + 1);
      if (lo >= 0xdc00 && lo <= 0xdfff) {
        out.push((hi - 0xd800) * 0x400 + (lo - 0xdc00) + 0x10000);
        i++;
        continue;
      }
    }
    out.push(hi >= 0xd800 && hi <= 0xdfff ? -1 : hi);
  }
  return out;
}

/** The name as Rust would store it, or why it is refused. */
export function checkName(typed: string): NameCheck {
  const cps = codePoints(typed);
  let start = 0;
  let end = cps.length;
  while (start < end && isWhiteSpace(cps[start])) start++;
  while (end > start && isWhiteSpace(cps[end - 1])) end--;
  const kept = cps.slice(start, end);
  if (kept.length === 0) return { ok: false, reason: 'empty' };
  if (kept.length > NAME_MAX_CHARS) return { ok: false, reason: 'too_long' };
  if (kept.some((c) => c < 0 || isControl(c) || isInvisibleFormat(c))) return { ok: false, reason: 'hidden_character' };
  // The trimmed text, cut at the same code points.
  let from = 0;
  for (let i = 0; i < start; i++) from += cps[i] > 0xffff ? 2 : 1;
  let length = 0;
  for (const c of kept) length += c > 0xffff ? 2 : 1;
  return { ok: true, name: typed.substring(from, from + length) };
}
