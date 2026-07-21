// Gaply — Research Paper Writer, Set A: the manuscript data model + scaffolds.
//
// STORAGE (Option A, ZERO migration): a manuscript is a note with
// note_type='manuscript'. Its metadata (authors, scaffold id, chosen CSL style)
// and its section BODIES live in fields_json; note.title holds the manuscript
// title. Section HEADINGS + guidance are NOT stored — they come from the static,
// versioned scaffold (below), keyed by section key, so a scaffold can evolve
// without rewriting stored notes and stored bodies always re-map by key.
//
// HONESTY: scaffolds are Gaply's OWN generic structures, never publisher
// template clones. Set A ships ONE: "Generic IMRaD structure".
import { Note, NoteDraft, parseFields } from './notesBridge';

export interface ScaffoldSection {
  key: string;       // stable id — stored bodies map to this, survives heading edits
  heading: string;   // the section H1 in the editor + docx
  guidance: string;  // honest "what editors expect" note (never auto-filled)
}

export interface Scaffold {
  id: string;
  label: string;             // honest UI label
  cslDefaultStyle: string;   // suggested style id (a real bundled CSL id)
  sections: ScaffoldSection[];
}

/** Generic IMRaD structure — Gaply's own scaffold, informed by what most
 *  journals expect. NOT an official template of any publisher. Title + authors
 *  are document metadata (the title page), so they are not body sections. */
export const IMRAD_SCAFFOLD: Scaffold = {
  id: 'imrad-generic',
  label: 'Generic IMRaD structure',
  cslDefaultStyle: 'ieee',
  sections: [
    { key: 'abstract', heading: 'Abstract', guidance: 'Typically ≤250 words: the problem, your method, the key result, and why it matters — in one paragraph.' },
    { key: 'keywords', heading: 'Keywords', guidance: '3–6 comma-separated terms that index your paper.' },
    { key: 'introduction', heading: 'Introduction', guidance: 'Motivate the problem, state the gap, and your contribution. End by outlining the paper.' },
    { key: 'methods', heading: 'Methods', guidance: 'Enough detail to reproduce: data, materials, procedures, and analysis.' },
    { key: 'results', heading: 'Results', guidance: 'What you found — without interpretation. Figures and tables belong here.' },
    { key: 'discussion', heading: 'Discussion', guidance: 'Interpret the results against the gap; state limitations and implications.' },
    { key: 'conclusion', heading: 'Conclusion', guidance: 'Restate the contribution and future work. Keep it short.' },
    { key: 'references', heading: 'References', guidance: 'Set B will insert citations from your library in your chosen style. For now, list references manually if you need them.' },
  ],
};

export const SCAFFOLDS: Scaffold[] = [IMRAD_SCAFFOLD];

export const scaffoldById = (id: string): Scaffold =>
  SCAFFOLDS.find((s) => s.id === id) ?? IMRAD_SCAFFOLD;

/** A live manuscript: scaffold-derived headings/guidance + the user's per-section
 *  markdown bodies + document metadata. `sections` is always in scaffold order. */
export interface ManuscriptSection extends ScaffoldSection {
  body: string; // canonical markdown (the RichBody value)
}

export interface Manuscript {
  id: string;
  title: string;
  authors: string;       // free text, e.g. "Ada Lovelace, Alan Turing"
  scaffoldId: string;
  cslStyleId: string;    // chosen style id (real CSL integration is Set B)
  sections: ManuscriptSection[];
}

/** The JSON persisted in fields_json — bodies + metadata only (headings/guidance
 *  are re-derived from the scaffold, never duplicated in storage). */
interface StoredManuscript {
  authors?: string;
  scaffoldId?: string;
  cslStyleId?: string;
  sections?: Array<{ key: string; body: string }>;
}

/** A fresh manuscript from a scaffold — every section empty. */
export const newManuscript = (id: string, scaffold: Scaffold = IMRAD_SCAFFOLD): Manuscript => ({
  id,
  title: '',
  authors: '',
  scaffoldId: scaffold.id,
  cslStyleId: scaffold.cslDefaultStyle,
  sections: scaffold.sections.map((s) => ({ ...s, body: '' })),
});

/** Rebuild a live Manuscript from a stored note. Section order/headings/guidance
 *  come from the scaffold; stored bodies are merged in BY KEY (robust to scaffold
 *  changes). Unknown/absent → empty. */
export const manuscriptFromNote = (note: Note): Manuscript => {
  const stored = parseFields(note) as StoredManuscript;
  const scaffold = scaffoldById(stored.scaffoldId ?? IMRAD_SCAFFOLD.id);
  const bodyByKey = new Map((stored.sections ?? []).map((s) => [s.key, s.body]));
  return {
    id: note.id,
    title: note.title,
    authors: stored.authors ?? '',
    scaffoldId: scaffold.id,
    cslStyleId: stored.cslStyleId ?? scaffold.cslDefaultStyle,
    sections: scaffold.sections.map((s) => ({ ...s, body: bodyByKey.get(s.key) ?? '' })),
  };
};

/** Serialize a manuscript to a NoteDraft. fields_json = metadata + section bodies;
 *  body = a plain-text concatenation so note_search finds manuscript content. */
export const manuscriptToDraft = (m: Manuscript): NoteDraft => ({
  id: m.id,
  note_type: 'manuscript',
  paper_id: null,
  paper_title: '',
  title: m.title.trim(),
  fields: {
    authors: m.authors.trim(),
    scaffoldId: m.scaffoldId,
    cslStyleId: m.cslStyleId,
    sections: m.sections.map((s) => ({ key: s.key, body: s.body })),
  },
  body: m.sections.map((s) => s.body).filter(Boolean).join('\n\n'),
});

/** Word count of a markdown body (whitespace-split, markers included — a live
 *  writing aid, not a submission metric). */
export const wordCount = (md: string): number => {
  const t = md.trim();
  return t ? t.split(/\s+/).length : 0;
};
