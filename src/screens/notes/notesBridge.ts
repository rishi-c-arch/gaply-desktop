// Gaply — Note Creator bridge (Set 3). Typed adapters over the Set 2 local
// notes store (the 7 thin Rust commands). Fully LOCAL/offline sqlite — no
// network, no LLM, no proxy, no entitlement (Note Creator is free). A path/data
// IPC seam only; the stateful mock mirrors the Rust CRUD so the Sets 4/5 UI
// tests run with no backend.
import { isTauri } from '../../utils/isTauri';

export type NoteType = 'paper' | 'project';
export type SyncStatus = 'local_only' | 'pending' | 'synced';

/** The optional, free-form per-paper template fields (stored in fields_json).
 *  Every field is optional — the template guides, it never forces. */
export interface PaperNoteFields {
  citation?: string;
  research_question?: string;
  methodology?: string;
  key_findings?: string;
  notable_quotes?: Array<{ text: string; page?: number | string }>;
  limitations?: string;
  my_evaluation?: string;
}

/** A stored note (snake_case, wire-exact to the Rust `Note`). Note that
 *  `fields_json` is a STRING here (as the store persists it); use
 *  {@link parseFields} to read it. */
export interface Note {
  id: string;
  note_type: NoteType;
  paper_id: string | null;
  paper_title: string;
  title: string;
  fields_json: string;
  body: string;
  tags: string[];
  sync_status: SyncStatus;
  created_at: number;
  updated_at: number;
}

/** A note to create/update. `fields` is the template OBJECT (the bridge sends
 *  it as the command's `fields_json`; the store persists it as a string). */
export interface NoteDraft {
  id: string;
  note_type: NoteType;
  paper_id?: string | null;
  paper_title?: string;
  title?: string;
  fields?: PaperNoteFields;
  body?: string;
  tags?: string[];
}

export interface NotesBridge {
  create(draft: NoteDraft): Promise<Note>;
  update(draft: NoteDraft): Promise<Note>;
  get(id: string): Promise<Note | null>;
  list(noteType?: NoteType, paperId?: string): Promise<Note[]>;
  search(query: string, tag?: string): Promise<Note[]>;
  setTags(id: string, tags: string[]): Promise<Note>;
  remove(id: string): Promise<void>;
}

/** Read a note's template fields back from `fields_json` (safe: bad JSON → {}). */
export function parseFields(note: Pick<Note, 'fields_json'>): PaperNoteFields {
  try {
    const v = JSON.parse(note.fields_json || '{}');
    return v && typeof v === 'object' ? (v as PaperNoteFields) : {};
  } catch {
    return {};
  }
}

/** NoteDraft → the Rust `NoteWrite` shape (the `note` arg of note_create/update). */
function draftToWire(draft: NoteDraft): Record<string, unknown> {
  return {
    id: draft.id,
    note_type: draft.note_type,
    paper_id: draft.paper_id ?? null,
    paper_title: draft.paper_title ?? '',
    title: draft.title ?? '',
    fields_json: draft.fields ?? null, // null → the store's {}
    body: draft.body ?? '',
    tags: draft.tags ?? [],
  };
}

/** Production bridge: the fully-local Tauri commands. Desktop-app only. */
export class TauriNotesBridge implements NotesBridge {
  private async invoke<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
    if (!isTauri) throw new Error('Note Creator runs in the Gaply desktop app.');
    const { invoke } = await import('@tauri-apps/api/core');
    return (await invoke(cmd, args)) as T;
  }

  create(draft: NoteDraft) {
    return this.invoke<Note>('note_create', { note: draftToWire(draft) });
  }
  update(draft: NoteDraft) {
    return this.invoke<Note>('note_update', { note: draftToWire(draft) });
  }
  async get(id: string) {
    return (await this.invoke<Note | null>('note_get', { id })) ?? null;
  }
  list(noteType?: NoteType, paperId?: string) {
    return this.invoke<Note[]>('note_list', { noteType: noteType ?? null, paperId: paperId ?? null });
  }
  search(query: string, tag?: string) {
    return this.invoke<Note[]>('note_search', { query, tag: tag ?? null });
  }
  setTags(id: string, tags: string[]) {
    return this.invoke<Note>('note_set_tags', { id, tags });
  }
  async remove(id: string) {
    await this.invoke<void>('note_delete', { id });
  }
}

/** In-memory double mirroring the Rust semantics: create/update by id, list
 *  filtered by note_type + paper_id, search over title/body/fields_json/
 *  paper_title/tags (+ tag filter), created_at preserved, every mutation resets
 *  sync_status, update() must exist. Used by the Sets 4/5 UI tests. */
export function makeMockNotesBridge(seed: Note[] = []): NotesBridge & { rows: Map<string, Note> } {
  const rows = new Map<string, Note>(seed.map((n) => [n.id, n]));

  const write = (draft: NoteDraft): Note => {
    const prev = rows.get(draft.id);
    const row: Note = {
      id: draft.id,
      note_type: draft.note_type,
      paper_id: draft.paper_id ?? null,
      paper_title: draft.paper_title ?? '',
      title: draft.title ?? '',
      fields_json: JSON.stringify(draft.fields ?? {}), // empty → "{}"
      body: draft.body ?? '',
      tags: draft.tags ?? [],
      sync_status: 'local_only',
      created_at: prev?.created_at ?? 1,
      updated_at: (prev?.updated_at ?? 0) + 1,
    };
    rows.set(draft.id, row);
    return row;
  };

  return {
    rows,
    async create(draft) {
      return write(draft);
    },
    async update(draft) {
      if (!rows.has(draft.id)) throw new Error(`note not found: ${draft.id}`);
      return write(draft);
    },
    async get(id) {
      return rows.get(id) ?? null;
    },
    async list(noteType, paperId) {
      return Array.from(rows.values())
        .filter((r) => (!noteType || r.note_type === noteType) && (!paperId || r.paper_id === paperId))
        .sort((a, b) => b.updated_at - a.updated_at);
    },
    async search(query, tag) {
      const q = query.trim().toLowerCase();
      return Array.from(rows.values())
        .filter((r) => {
          const hit =
            q === '' ||
            r.title.toLowerCase().includes(q) ||
            r.body.toLowerCase().includes(q) ||
            r.fields_json.toLowerCase().includes(q) ||
            r.paper_title.toLowerCase().includes(q) ||
            r.tags.some((t) => t.toLowerCase().includes(q));
          const tagHit = !tag || r.tags.includes(tag);
          return hit && tagHit;
        })
        .sort((a, b) => b.updated_at - a.updated_at);
    },
    async setTags(id, tags) {
      const r = rows.get(id);
      if (!r) throw new Error(`note not found: ${id}`);
      const next: Note = { ...r, tags, sync_status: 'local_only', updated_at: r.updated_at + 1 };
      rows.set(id, next);
      return next;
    },
    async remove(id) {
      rows.delete(id);
    },
  };
}
