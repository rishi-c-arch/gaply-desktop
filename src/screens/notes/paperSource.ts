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
    // M2 Set 1: the badge only promises when there's EXACTLY ONE normalized
    // (trim + lowercase) match — mirroring the backend's exactly-one read. It must
    // NOT light up on a title collision (the backend returns None for those, so a
    // promise would be false) nor rely on case-sensitivity. Count, don't just
    // test membership.
    const norm = (s: string) => s.trim().toLowerCase();
    const counts = new Map<string, number>();
    for (const p of lib) counts.set(norm(p.title), (counts.get(norm(p.title)) ?? 0) + 1);
    return refs.map((r) => ({
      id: r.id,
      title: r.title,
      authors: r.authors,
      year: r.year,
      hasFullText: counts.get(norm(r.title)) === 1,
    }));
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
