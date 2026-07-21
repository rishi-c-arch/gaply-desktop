// Gaply — Research Paper Writer: the manuscript data model + scaffold types.
//
// STORAGE (Option A, ZERO migration): a manuscript is a note with
// note_type='manuscript'. Its metadata (authors, scaffold id, chosen CSL style)
// and its section BODIES live in fields_json; note.title holds the manuscript
// title. Section HEADINGS + guidance are NOT stored — they come from the
// scaffold (Set C: a versioned static asset public/manuscripts/scaffolds.json),
// keyed by section key, so a scaffold can evolve without rewriting stored notes
// and stored bodies always re-map by key.
//
// HONESTY (Set C): scaffolds are Gaply's OWN generic structures informed by
// publishers' PUBLIC author guidance — never official template clones. Every
// scaffold is labelled "…-style (unofficial)" and links the publisher's real
// author page. Section keys are SHARED across scaffolds so switching venue
// preserves matching content.
import { Note, NoteDraft, parseFields } from './notesBridge';

export interface ScaffoldSection {
  key: string;       // stable id — stored bodies map to this, shared across scaffolds
  heading: string;   // the section H1 in the editor + docx (may differ per venue)
  guidance: string;  // honest "what editors expect" note (never auto-filled)
  typicalWords?: string; // honest hint, e.g. "150–250"
  required?: boolean;    // venues differ on what's mandatory
}

/** Per-venue MANUSCRIPT (not camera-ready) formatting profile applied to the
 *  .docx export. Grounded in each venue's public submission guidance where it
 *  specifies one; otherwise a sensible academic default (see `source`). */
export interface DocxFormat {
  font: string;
  fontSizePt: number;
  marginInch: number;
  pageSize: 'letter' | 'a4';
  lineSpacing: 'single' | '1.5' | 'double';
  lineNumbers: boolean;
  sectionNumbering: 'none' | 'decimal' | 'roman-upper';
  titleBlock: { affiliations: boolean; correspondingAuthor: boolean };
  source?: string; // honest note: grounded in the venue's guideline vs Gaply default
}

/** The academic review-manuscript default (double / TNR 12 / 1-inch / line
 *  numbers), used as a fallback when a scaffold omits its own profile. */
export const DEFAULT_DOCX_FORMAT: DocxFormat = {
  font: 'Times New Roman', fontSizePt: 12, marginInch: 1, pageSize: 'letter',
  lineSpacing: 'double', lineNumbers: true, sectionNumbering: 'none',
  titleBlock: { affiliations: true, correspondingAuthor: true },
};

export interface Scaffold {
  id: string;
  label: string;             // honest UI label, e.g. "IEEE-style (unofficial)"
  unofficial: boolean;       // ALWAYS true — Gaply's own, never an official template
  cslDefaultStyle: string;   // suggested style id (a real bundled CSL id)
  publisherAuthorUrl: string;// link to the venue's real author page
  noticeText: string;        // honest per-scaffold disclaimer
  docxFormat?: DocxFormat;   // per-venue manuscript formatting (Set C+)
  sections: ScaffoldSection[];
}

/** Bundled fallback = the Generic IMRaD scaffold (mirrors the first asset entry),
 *  used offline / if the scaffolds.json fetch fails so the editor never blocks. */
export const IMRAD_SCAFFOLD: Scaffold = {
  id: 'imrad-generic',
  label: 'Generic IMRaD (journal)',
  unofficial: true,
  cslDefaultStyle: 'apa',
  publisherAuthorUrl: 'https://www.icmje.org/recommendations/',
  noticeText: "A general IMRaD structure — Gaply's own starting scaffold, not tied to any publisher. Follow your target journal's guidelines.",
  docxFormat: { ...DEFAULT_DOCX_FORMAT },
  sections: [
    { key: 'abstract', heading: 'Abstract', guidance: 'One paragraph: problem, method, key result, significance. Usually no citations.', typicalWords: '150–250', required: true },
    { key: 'keywords', heading: 'Keywords', guidance: '3–6 comma-separated indexing terms.', typicalWords: '3–6 terms', required: false },
    { key: 'introduction', heading: 'Introduction', guidance: 'Motivate the problem, state the gap and your contribution.', typicalWords: '500–800', required: true },
    { key: 'methods', heading: 'Methods', guidance: 'Enough detail to reproduce: data, materials, procedures, analysis.', typicalWords: '600–1200', required: true },
    { key: 'results', heading: 'Results', guidance: 'What you found, without interpretation. Figures and tables here.', typicalWords: '600–1200', required: true },
    { key: 'discussion', heading: 'Discussion', guidance: 'Interpret results against the gap; limitations; implications.', typicalWords: '600–1000', required: true },
    { key: 'conclusion', heading: 'Conclusion', guidance: 'Restate the contribution and future work. Keep it short.', typicalWords: '100–250', required: false },
    { key: 'references', heading: 'References', guidance: 'Gaply will insert your library citations in the chosen style (Set B). For now, list manually if needed.', required: true },
  ],
};

/** A live manuscript section. The scaffold SEEDS these, but the user can rename,
 *  add, delete, and reorder them — so `sections` is the AUTHORITATIVE structure
 *  (order + headings + bodies), persisted in fields_json. guidance/typicalWords/
 *  required are hints carried from the scaffold by key (absent for custom
 *  sections). */
export interface ManuscriptSection {
  key: string;       // stable id — bodies map to this across scaffold switches
  heading: string;   // user-editable display heading
  body: string;      // canonical markdown (the RichBody value)
  guidance?: string;
  typicalWords?: string;
  required?: boolean;
}

export interface Manuscript {
  id: string;
  title: string;
  authors: string;              // free text, e.g. "Ada Lovelace, Alan Turing"
  affiliations: string;         // e.g. "¹Dept …, ²…"  (title block)
  correspondingAuthor: string;  // e.g. "ada@…"        (title block)
  scaffoldId: string;
  cslStyleId: string;           // chosen style id (real CSL integration is Set B)
  sections: ManuscriptSection[];
}

interface StoredManuscript {
  authors?: string;
  affiliations?: string;
  correspondingAuthor?: string;
  scaffoldId?: string;
  cslStyleId?: string;
  // heading is stored so renames/custom sections persist; older manuscripts
  // stored only {key, body} — heading then falls back to the scaffold by key.
  sections?: Array<{ key: string; heading?: string; body: string }>;
}

const titleize = (key: string): string =>
  key.replace(/[-_]+/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());

/** A fresh manuscript from a scaffold — every section empty. */
export const newManuscript = (id: string, scaffold: Scaffold = IMRAD_SCAFFOLD): Manuscript => ({
  id,
  title: '',
  authors: '',
  affiliations: '',
  correspondingAuthor: '',
  scaffoldId: scaffold.id,
  cslStyleId: scaffold.cslDefaultStyle,
  sections: scaffold.sections.map((s) => ({ ...s, body: '' })),
});

/** Read the stored scaffold id (for the caller to resolve the right scaffold). */
export const storedScaffoldId = (note: Note): string =>
  (parseFields(note) as StoredManuscript).scaffoldId ?? IMRAD_SCAFFOLD.id;

/** Rebuild a live Manuscript from a stored note using the RESOLVED scaffold
 *  (caller looks it up from the loaded set). Section order/headings/guidance come
 *  from the scaffold; stored bodies merge in BY KEY (robust to scaffold changes).
 *  Bodies for keys the scaffold no longer has are dropped from the live view —
 *  callers pass the manuscript's OWN scaffold so nothing is lost silently. */
export const manuscriptFromNote = (note: Note, scaffold: Scaffold = IMRAD_SCAFFOLD): Manuscript => {
  const stored = parseFields(note) as StoredManuscript;
  const scByKey = new Map(scaffold.sections.map((s) => [s.key, s]));
  const st = stored.sections ?? [];
  // Stored sections are AUTHORITATIVE (order + headings), enriched with the
  // scaffold's guidance by key. A brand-new/legacy note with none → seed from
  // the scaffold. Legacy stored {key, body} → heading falls back to the scaffold.
  const sections: ManuscriptSection[] = st.length
    ? st.map((ss) => {
        const sc = scByKey.get(ss.key);
        return {
          key: ss.key,
          heading: ss.heading ?? sc?.heading ?? titleize(ss.key),
          body: ss.body ?? '',
          guidance: sc?.guidance,
          typicalWords: sc?.typicalWords,
          required: sc?.required,
        };
      })
    : scaffold.sections.map((s) => ({ ...s, body: '' }));
  return {
    id: note.id,
    title: note.title,
    authors: stored.authors ?? '',
    affiliations: stored.affiliations ?? '',
    correspondingAuthor: stored.correspondingAuthor ?? '',
    scaffoldId: scaffold.id,
    cslStyleId: stored.cslStyleId ?? scaffold.cslDefaultStyle,
    sections,
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
    affiliations: m.affiliations.trim(),
    correspondingAuthor: m.correspondingAuthor.trim(),
    scaffoldId: m.scaffoldId,
    cslStyleId: m.cslStyleId,
    // Store heading too so renames / custom sections / reorder persist.
    sections: m.sections.map((s) => ({ key: s.key, heading: s.heading, body: s.body })),
  },
  body: m.sections.map((s) => s.body).filter(Boolean).join('\n\n'),
});

/** Result of switching a manuscript to a new scaffold: the remapped manuscript
 *  (bodies carried over BY KEY, CSL default updated to the new venue) plus the
 *  sections whose content would be DROPPED (had text, key absent in the new
 *  scaffold). The caller MUST warn on a non-empty `dropped` — never silent. */
export interface ScaffoldSwitch {
  manuscript: Manuscript;
  dropped: Array<{ heading: string; body: string }>;
}

export const switchScaffold = (m: Manuscript, next: Scaffold): ScaffoldSwitch => {
  const bodyByKey = new Map(m.sections.map((s) => [s.key, s.body]));
  const nextKeys = new Set(next.sections.map((s) => s.key));
  const dropped = m.sections
    .filter((s) => s.body.trim() && !nextKeys.has(s.key))
    .map((s) => ({ heading: s.heading, body: s.body }));
  return {
    manuscript: {
      ...m,
      scaffoldId: next.id,
      cslStyleId: next.cslDefaultStyle, // venue switch → the venue's suggested style
      sections: next.sections.map((s) => ({ ...s, body: bodyByKey.get(s.key) ?? '' })),
    },
    dropped,
  };
};

/* ---- Custom section editing (the scaffold is a starting point, not a cage) ---- */

const slugKey = (heading: string): string =>
  heading.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'section';

/** A unique section key from a base, disambiguated against existing keys. */
export const uniqueSectionKey = (base: string, existing: Iterable<string>): string => {
  const taken = new Set(existing);
  const root = slugKey(base);
  if (!taken.has(root)) return root;
  let i = 2;
  while (taken.has(`${root}-${i}`)) i += 1;
  return `${root}-${i}`;
};

/** Rename a section's heading (key stays stable, so content still maps). */
export const renameSection = (m: Manuscript, key: string, heading: string): Manuscript =>
  ({ ...m, sections: m.sections.map((s) => (s.key === key ? { ...s, heading } : s)) });

/** Insert a new empty custom section at `atIndex`. Its generated key is at that
 *  index in the returned manuscript (the caller can select/rename it). */
export const addSection = (m: Manuscript, atIndex: number, heading = 'New Section'): Manuscript => {
  const key = uniqueSectionKey(heading, m.sections.map((s) => s.key));
  const at = Math.max(0, Math.min(atIndex, m.sections.length));
  const sections = [...m.sections];
  sections.splice(at, 0, { key, heading, body: '' });
  return { ...m, sections };
};

/** Remove a section by key (the caller confirms if its body is non-empty). */
export const deleteSection = (m: Manuscript, key: string): Manuscript =>
  ({ ...m, sections: m.sections.filter((s) => s.key !== key) });

/** Move a section up (-1) or down (+1); a no-op at the ends. */
export const moveSection = (m: Manuscript, key: string, dir: -1 | 1): Manuscript => {
  const i = m.sections.findIndex((s) => s.key === key);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= m.sections.length) return m;
  const sections = [...m.sections];
  [sections[i], sections[j]] = [sections[j], sections[i]];
  return { ...m, sections };
};

/** Word count of a markdown body (whitespace-split, markers included). */
export const wordCount = (md: string): number => {
  const t = md.trim();
  return t ? t.split(/\s+/).length : 0;
};
