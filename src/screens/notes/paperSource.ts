// Gaply — Note Creator paper source (Set 4). Supplies the paper PICKER: the
// researcher's citation_library references (the soft anchor → citation_library.id)
// plus a flag for whether the paper also lives in the plagiarism "my papers"
// library (which holds full_text) so the editor can OPTIONALLY offer side-by-side.
// Fully local; no network. A note can also attach to a FREE-TYPED paper (no
// library row) — the soft anchor supports paper_id = '' with a free paper_title.
import { TauriLocalLibrary, StoredReference } from '../citations/localLibrary';
import { TauriCheckBridge, CheckBridge } from '../checks/checkBridge';

export interface PaperOption {
  /** citation_library.id — the soft anchor stored on the note (paper_id). */
  id: string;
  title: string;
  authors?: string;
  year?: number | null;
  /** True when the paper is also in the plagiarism "my papers" library, i.e.
   *  its full_text exists and side-by-side reading is (optionally) available. */
  hasFullText?: boolean;
}

export interface PaperSource {
  listPapers(): Promise<PaperOption[]>;
}

/** Production source: citation_library references, tagged with full-text
 *  availability by matching titles against the plagiarism "my papers" library. */
export class TauriPaperSource implements PaperSource {
  constructor(
    private citations = new TauriLocalLibrary(),
    private papers: CheckBridge = new TauriCheckBridge()
  ) {}

  async listPapers(): Promise<PaperOption[]> {
    const refs: StoredReference[] = await this.citations.list().catch(() => []);
    const lib = await this.papers.libraryList().catch(() => []);
    // The badge must promise EXACTLY when the backend read would return text (M2
    // Set 2C's chain): id match first, else title fallback — both exactly-one-
    // guarded, so a collision NEVER promises (backend returns None there).
    //   · idCount === 1  → the id path yields text          → promise
    //   · idCount ≠ 1 (0 or 2+) → backend falls to title    → promise iff
    //       titleCount === 1  (Set-1's normalized single match)
    // Count, don't just test membership — mirrors the guarded reads exactly.
    const norm = (s: string) => s.trim().toLowerCase();
    const titleCounts = new Map<string, number>();
    const idCounts = new Map<string, number>();
    for (const p of lib) {
      titleCounts.set(norm(p.title), (titleCounts.get(norm(p.title)) ?? 0) + 1);
      if (p.citation_id) idCounts.set(p.citation_id, (idCounts.get(p.citation_id) ?? 0) + 1);
    }
    return refs.map((r) => {
      const idCount = idCounts.get(r.id) ?? 0;
      const titleCount = titleCounts.get(norm(r.title)) ?? 0;
      return {
        id: r.id,
        title: r.title,
        authors: r.authors,
        year: r.year,
        hasFullText: idCount === 1 ? true : titleCount === 1,
      };
    });
  }
}

/** Test/dev double. */
export function makeMockPaperSource(seed: PaperOption[] = []): PaperSource & { papers: PaperOption[] } {
  return {
    papers: seed,
    async listPapers() {
      return [...seed];
    },
  };
}
