// PARTIAL instrument for ONTOLOGY §4.20 (representation correctness).
//
// ══════════════════════════════════════════════════════════════════════════
// PASSING THIS DOES NOT IMPLY REPRESENTATION CORRECTNESS.
//
// This verifies the SANITIZER'S TRANSFORMATION — what `toAscii` returns for a
// given input. It does NOT verify what the artifact CONTAINS. The sanitizer can
// be correct while the renderer is wrong, and the reverse: an assertion on
// `Finding.title` passes while the PDF says "arm".
//
// The FULL instrument compares the artifact's rendered BYTES against known
// Unicode input, and it waits for the Rust renderer — asserting miniPdf's bytes
// today would instrument the thing being replaced. Its home is the first PDF
// PR, not a tracked item (ARCHITECTURE_TRACE §31.15).
//
// This ships anyway, because the Rust renderer may be weeks out and the fix
// should not sit unguarded in the interval.
// ══════════════════════════════════════════════════════════════════════════
import { describe, expect, it } from 'vitest';
import {
  disclosuresFor,
  NOTE_MARKED,
  NOTE_SIMPLIFIED,
  toAscii,
  UNREPRESENTABLE,
} from './miniPdf';

describe('toAscii — never silently deletes', () => {
  // THE INVARIANT. Whatever the transformation policy, no character may vanish
  // without a trace. This is the assertion that must survive any future change
  // to the tiers below.
  it('emits something for every input character, never nothing', () => {
    for (const input of ['Müller', 'Kumar Śarmā', 'हिन्दी', '日本語', 'naïve', 'Ünïcödé']) {
      const out = toAscii(input);
      expect(out.length).toBeGreaterThan(0);
      expect(out).not.toBe('');
    }
    // The specific regression: the old implementation returned '' here.
    expect(toAscii('हिन्दी')).not.toBe('');
  });

  it('TIER 1 — Latin diacritics transliterate to their base letters', () => {
    expect(toAscii('Müller')).toBe('Muller');
    expect(toAscii('naïve')).toBe('naive');
    expect(toAscii('Kumar Śarmā')).toBe('Kumar Sarma');
    // Recognisable, meaning unchanged, and the reader recovers the original —
    // which is why transliteration is right here and marking is not.
    expect(toAscii('Müller')).not.toContain(UNREPRESENTABLE);
  });

  it('TIER 2 — non-Latin is MARKED, never transliterated and never dropped', () => {
    const devanagari = toAscii('हिन्दी');
    expect(devanagari).toMatch(new RegExp(`^${UNREPRESENTABLE}+$`));
    // Not transliterated: no ASCII form of Devanagari preserves meaning, so a
    // plausible-looking romanisation would be the §4.20 error in a new place.
    expect(devanagari).not.toMatch(/[a-z]/i);
    expect(toAscii('日本語')).toMatch(new RegExp(`^${UNREPRESENTABLE}+$`));
  });

  it('a marked omission is distinguishable from a literal question mark', () => {
    // toAscii keeps the distinct mark; escapePdf folds it to '?' only at the
    // byte layer. So a manuscript that really contains "?" is not confused
    // with a character we could not render.
    expect(toAscii('why?')).toBe('why?');
    expect(toAscii('why?')).not.toContain(UNREPRESENTABLE);
  });

  it('the existing punctuation folding is preserved', () => {
    expect(toAscii('“quoted”')).toBe('"quoted"');
    expect(toAscii('em—dash')).toBe('em-dash');
    expect(toAscii('a…b')).toBe('a...b');
  });

  it('mixed scripts keep the Latin part readable and mark only the rest', () => {
    const out = toAscii('Śarmā हिन्दी 2024');
    expect(out).toContain('Sarma');
    expect(out).toContain('2024');
    expect(out).toContain(UNREPRESENTABLE);
  });
});


describe('fold disclosure (§4.23) — the renderer says what it did', () => {
  it('an accented name triggers the SIMPLIFIED note and nothing else', () => {
    const notes = disclosuresFor(['Reported by Śarmā and colleagues']);
    expect(notes).toContain(NOTE_SIMPLIFIED);
    expect(notes).not.toContain(NOTE_MARKED);
  });

  it('an unrepresentable script triggers the MARKED note', () => {
    const notes = disclosuresFor(['A title in हिन्दी']);
    expect(notes).toContain(NOTE_MARKED);
  });

  it('plain ASCII triggers NEITHER — a disclosure that always fires says nothing', () => {
    expect(disclosuresFor(['A perfectly ordinary ASCII title', 'p = 0.03'])).toEqual([]);
  });

  it('a disclosure never exhibits an instance of the change it describes', () => {
    // ONTOLOGY §4.23: the first draft exhibited "Śarmā", which this renderer
    // folds, so the sentence explaining an alteration was itself altered into
    // nonsense. The property is NOT "byte-identical after toAscii" — an em dash
    // normalising to a hyphen is punctuation, not the LETTER fold the note is
    // about, and the Rust renderer does the same. The property is that no note
    // contains an accented letter or an unrepresentable character.
    for (const n of [NOTE_SIMPLIFIED, NOTE_MARKED]) {
      expect(toAscii(n)).not.toContain(UNREPRESENTABLE);
      expect(n.normalize('NFD').replace(/[\u0300-\u036f]/g, '')).toBe(n);
    }
  });

  it('the wording is byte-identical to the Rust composer constants', () => {
    // Ported verbatim; a second wording is a second thing to drift.
    expect(NOTE_SIMPLIFIED.startsWith('Some letters in this report were simplified')).toBe(true);
    expect(NOTE_SIMPLIFIED.endsWith('The spelling in your manuscript is unchanged.')).toBe(true);
    expect(NOTE_MARKED.endsWith('was skipped or removed.')).toBe(true);
  });
});
