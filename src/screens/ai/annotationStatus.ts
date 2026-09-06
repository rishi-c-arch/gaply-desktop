// Gaply — what colour a judged sentence gets, and what it MEANS (§11 D92).
// Separated from the view so the mapping is testable and stated once.

export type AnnotationStatus = 'checked' | 'weak' | 'blocked' | 'advisory';

export interface StatusStyle {
  /** The word. Colour is never the only signal, so this appears in the legend
   *  AND in every detail entry. */
  label: string;
  fill: string;
  /** The SECOND cue: solid for evidence-backed, dashed for a suggestion.
   *  Survives greyscale and colour-blindness, which a hue does not. */
  edge: 'solid' | 'dashed';
  /** §11 D89 carried into the annotated view: a 43%-precision suggestion is
   *  visually subordinate to a finding backed by a quoted passage. */
  weight: 'full' | 'light';
  /** Plain English, for the detail entry. What it means. */
  meaning: string;
  /** Plain English. What to do about it. */
  action: string;
}

export const STATUS_STYLE: Record<AnnotationStatus, StatusStyle> = {
  checked: {
    label: 'verified with evidence',
    fill: '#2e7d32',
    edge: 'solid',
    weight: 'full',
    meaning:
      'Gaply read the source this sentence cites and found a passage that supports it.',
    action: 'Read the quoted passage in the detail below and confirm you agree with it.',
  },
  weak: {
    label: 'weak or contradicted',
    fill: '#c62828',
    edge: 'solid',
    weight: 'full',
    meaning:
      'Gaply read the cited source, and the passage it found does not fully support this sentence — or contradicts it.',
    action:
      'Check the quoted passage. If it does not say what the sentence claims, soften the claim or cite a different source.',
  },
  blocked: {
    label: 'cited but not checkable',
    fill: '#616161',
    edge: 'solid',
    weight: 'full',
    meaning:
      'This sentence cites a source, but Gaply has no readable copy of it, so nothing was verified either way. This is a gap in your library, not a problem with your writing.',
    action:
      'Fetch an open-access copy or attach the PDF in the Citation Manager, then re-check just these items.',
  },
  advisory: {
    label: 'suggestion — not checked',
    fill: '#ffb300',
    edge: 'dashed',
    weight: 'light',
    meaning:
      'This sentence carries no citation and a language model thought it might need one. Nothing was checked against any source, and it is right slightly under half the time.',
    action: 'Skim it. If it states something a reader would want to look up, add a citation.',
  },
};

/**
 * The verdict a sentence was given -> the status it is drawn in.
 *
 * `null` means NO HIGHLIGHT. A sentence judged "no citation needed" is not a
 * finding, and colouring it would turn 65 non-events into a page of marks
 * (§11 D78, the same reason the report's advisory section filters).
 */
export function statusOf(kind: string, verdict?: string | null): AnnotationStatus | null {
  if (kind === 'unverifiable') return 'blocked';
  if (kind === 'citation_need') return verdict === 'needs_citation' ? 'advisory' : null;
  if (kind === 'citation_support') {
    switch (verdict) {
      case 'strong':
        return 'checked';
      case 'partial':
      case 'weak':
      case 'contradicts':
      case 'insufficient_evidence':
        return 'weak';
      // Judged but with no recorded verdict — not evidence of anything.
      default:
        return null;
    }
  }
  return null;
}

/** The four, in the order the legend and the detail list present them. */
export const STATUS_ORDER: AnnotationStatus[] = ['checked', 'weak', 'blocked', 'advisory'];
