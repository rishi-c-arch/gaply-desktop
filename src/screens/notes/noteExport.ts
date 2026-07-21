// Gaply — Note Creator export (polish). PURE Markdown emitter: a Note (+ its
// parsed template fields) → clean, portable Markdown. Deterministic string
// building only — NO model, NO network, NO invention. Skip-empties is the same
// honesty rule the citation exporters hold: a field the note lacks is simply
// absent, never a blank stub, never padded.
import { Note, NoteType, PaperNoteFields, parseFields } from './notesBridge';

/** Export honesty for pasted images (Set 3): a single portable `.md` can't
 *  carry binaries, and a bare gaply-image:// ref means nothing outside Gaply.
 *  Rewrite each image ref to a clearly-labelled placeholder that keeps the ref
 *  as a breadcrumb — never a broken <img>, never a silent drop. */
export function honestImagePlaceholders(md: string): string {
  return md.replace(/!\[[^\]]*\]\((gaply-image:\/\/[A-Za-z0-9]+\.[A-Za-z0-9]+)\)/g,
    (_m, ref) => `![image stored in Gaply](${ref})`);
}

/** The minimal shape the emitter needs — a stored Note satisfies it, and so does
 *  a live editor draft (so the per-note Export button can emit unsaved edits). */
export interface ExportableNote {
  note_type: NoteType;
  title: string;
  paper_title: string;
  body: string;
  tags: string[];
}

/** The paper-note template fields, in the EDITOR'S display order, with the
 *  heading each renders under. The structure is the feature's value — one
 *  heading per field, never a flattened blob. */
const PAPER_FIELD_ORDER: Array<{ key: keyof PaperNoteFields; heading: string }> = [
  { key: 'citation', heading: 'Citation' },
  { key: 'research_question', heading: 'Research question' },
  { key: 'methodology', heading: 'Methodology' },
  { key: 'key_findings', heading: 'Key findings' },
  { key: 'notable_quotes', heading: 'Notable quotes' },
  { key: 'limitations', heading: 'Limitations' },
  { key: 'my_evaluation', heading: 'Evaluation' },
];

/** Render ONE note as Markdown. Project → title + body. Paper → structured,
 *  a heading per FILLED field. Empties are skipped entirely (never a stub). */
export function noteToMarkdown(note: ExportableNote, fields: PaperNoteFields): string {
  const lines: string[] = [];
  const noteTitle = note.title.trim();
  const paperTitle = note.paper_title.trim();
  const hasTitle = noteTitle.length > 0;

  // Header: the note title if present, else fall back to the paper title.
  lines.push(`# ${hasTitle ? noteTitle : paperTitle || 'Untitled note'}`);

  if (note.note_type === 'paper') {
    // Paper reference — only when it adds something (a distinct note title exists;
    // otherwise the H1 already IS the paper title, so we don't repeat it).
    if (paperTitle && hasTitle) lines.push('', `**Paper:** ${paperTitle}`);

    for (const { key, heading } of PAPER_FIELD_ORDER) {
      if (key === 'notable_quotes') {
        const quotes = (fields.notable_quotes ?? []).filter((q) => q.text && q.text.trim());
        if (quotes.length === 0) continue; // skip-empty
        lines.push('', `## ${heading}`);
        for (const q of quotes) {
          const p = q.page != null && String(q.page).trim() ? ` (p. ${String(q.page).trim()})` : '';
          lines.push(`> ${q.text.trim()}${p}`);
        }
      } else {
        const value = (fields[key] as string | undefined)?.trim();
        if (!value) continue; // skip-empty — no blank stub
        lines.push('', `## ${heading}`, value);
      }
    }
  } else {
    // Project note: the body under the title (image refs → honest placeholders).
    const body = honestImagePlaceholders(note.body.trim());
    if (body) lines.push('', body);
  }

  const tags = note.tags.map((t) => t.trim()).filter(Boolean);
  if (tags.length > 0) lines.push('', `_Tags: ${tags.join(', ')}_`);

  return lines.join('\n');
}

/** Bulk: every note in the (already-filtered) list, joined by a horizontal rule.
 *  The caller passes the current filtered `list`, so this honors the filter. */
export function notesToMarkdown(notes: Note[]): string {
  return notes.map((n) => noteToMarkdown(n, parseFields(n))).join('\n\n---\n\n');
}

/** Slug an arbitrary string to safe filename chars (letters/numbers/hyphens). */
const slugify = (s: string, max = 60): string =>
  s.trim().toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-+|-+$/g, '').slice(0, max);

/** A safe, readable `.md` filename from a title (slugged), with a fallback. */
export function exportFileName(title: string, fallback = 'note'): string {
  return `${slugify(title) || fallback}.md`;
}

/** The bulk-export filename, encoding the FULL active filter (type + tag +
 *  search) so a filtered export is self-describing. Every part is slugged, so a
 *  search query with illegal chars (e.g. "a/b: c") can never break the name. */
export function bulkExportFileName(typeFilter: string, tagFilter: string | null, query: string): string {
  const parts: string[] = [];
  if (typeFilter && typeFilter !== 'all') parts.push(slugify(typeFilter, 20));
  if (tagFilter) { const t = slugify(tagFilter, 30); if (t) parts.push(t); }
  const q = slugify(query, 30);
  if (q) parts.push(q);
  return `notes${parts.length ? `-${parts.join('-')}` : ''}.md`;
}
