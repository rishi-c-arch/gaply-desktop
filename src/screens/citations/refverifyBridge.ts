// Gaply — refverify bridge for the Citation Manager. Runs the existing
// gaply_core refverify pipeline (CrossRef / OpenAlex / Retraction Watch /
// Unpaywall / Semantic Scholar) to confirm a DOI exists, flag retractions, and
// enrich metadata. Real path calls the `verify_reference` Tauri command
// (commands.rs:398, registered lib.rs:95); tests use makeMockRefVerify.
import { Citation, CslItem } from './citationTypes';

/** Minimal mirror of gaply_core::refverify::ReferenceVerification. Free-text
 *  fields arrive as UntrustedText: `{ safe_text, ... }` — we read safe_text. */
export interface UntrustedTextJson {
  safe_text: string;
  suspicious: boolean;
}
export interface ReferenceVerification {
  reference_raw: string;
  exists: { found: boolean; source: string; doi: string | null; title: UntrustedTextJson | null; is_retracted_hint: boolean | null } | null;
  retraction: { retracted: boolean; reasons: UntrustedTextJson[]; notice_url: string | null } | null;
  enrichment: { citation_count: number | null; abstract_text: UntrustedTextJson | null; venue: UntrustedTextJson | null } | null;
  open_access: { is_oa: boolean; best_url: string | null } | null;
  provenance: Array<{ source: string; url: string }>;
  warnings: string[];
}

export interface RefVerifyBridge {
  verify(reference: { raw: string; doi?: string; title?: string; year?: number }): Promise<ReferenceVerification>;
}

export class TauriRefVerifyBridge implements RefVerifyBridge {
  async verify(reference: { raw: string; doi?: string; title?: string; year?: number }): Promise<ReferenceVerification> {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<ReferenceVerification>('verify_reference', { reference });
  }
}

/** Test/dev double. */
export function makeMockRefVerify(result: ReferenceVerification | ((doi?: string) => ReferenceVerification)): RefVerifyBridge {
  return {
    async verify(ref) {
      return typeof result === 'function' ? result(ref.doi) : result;
    },
  };
}

/** Merge refverify output into a Citation (metadata only). */
export function applyVerification(base: Citation, v: ReferenceVerification): Citation {
  const csl: CslItem = { ...base.csl };
  if (v.exists?.doi) csl.DOI = v.exists.doi;
  if (v.exists?.title?.safe_text) csl.title = csl.title || v.exists.title.safe_text;
  if (v.enrichment?.venue?.safe_text) csl.containerTitle = csl.containerTitle || v.enrichment.venue.safe_text;
  return {
    ...base,
    csl,
    doi: v.exists?.doi ?? base.doi,
    retracted: Boolean(v.retraction?.retracted || v.exists?.is_retracted_hint),
    noticeUrl: v.retraction?.notice_url ?? base.noticeUrl,
    provenance: v.provenance.map((p) => `${p.source}:${p.url}`),
  };
}
