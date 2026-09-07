// Gaply — what colour a judged sentence gets, and what it MEANS (§11 D92).
// Separated from the view so the mapping is testable and stated once.

export type AnnotationStatus = 'evidence' | 'blocked' | 'advisory';

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
  // §11 D108. Was TWO statuses — green 'verified with evidence' for a `strong`
  // verdict and red 'weak or contradicted' for everything else. The verdict
  // measured as a constant `weak` (14 of 14 valid outputs across two runs), so
  // in practice every citation_support sentence was drawn RED, labelled
  // "weak or contradicted", and the green legend entry was unreachable — a
  // legend describing a distinction the page could not make.
  //
  // One status now, and it is NEUTRAL on purpose. Blue, not green and not red:
  // this marks where the evidence is, and asserts nothing about whether the
  // sentence is well supported. That judgement is the reader's, and the
  // passages are printed so they can make it.
  evidence: {
    label: 'source passages found',
    fill: '#1565c0',
    edge: 'solid',
    weight: 'full',
    meaning:
      'Gaply read the source this sentence cites and found the passages it rests on. It does NOT grade how well they support the sentence — its grader returned the same grade for every case on a labelled set, so that grade is not shown.',
    action:
      'Read the quoted passages in the detail below and decide for yourself whether they say what the sentence claims.',
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
export function statusOf(
  kind: string,
  verdict?: string | null,
  /** How many source passages were recorded for this item (§11 D108). */
  passageCount = 0,
): AnnotationStatus | null {
  if (kind === 'unverifiable') return 'blocked';
  if (kind === 'citation_need') return verdict === 'needs_citation' ? 'advisory' : null;
  if (kind === 'citation_support') {
    // §11 D108. The VERDICT no longer decides anything here — it is a constant.
    // What decides is whether there are passages to show, because that is what
    // the highlight now claims. A mark saying "source passages found" over a
    // sentence with none would be the same lie in a new colour, and there is no
    // honest label for "read, nothing recorded" that does not collide with
    // `blocked` (whose action is to go and fetch the source).
    return passageCount > 0 ? 'evidence' : null;
  }
  return null;
}

/** The three, in the order the legend and the detail list present them. */
export const STATUS_ORDER: AnnotationStatus[] = ['evidence', 'blocked', 'advisory'];
