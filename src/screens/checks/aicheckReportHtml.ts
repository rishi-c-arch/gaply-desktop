// Gaply — the AI Check report as ONE self-contained HTML document. This is the
// SECOND renderer of the same report (the first is AiCheckReport.tsx); it exists
// because print is dead in the Tauri/wry webview (window.print(), webview.print()
// AND ⌘P are all no-ops on macOS). The export flow writes this string to a temp
// file and opens it in the user's DEFAULT BROWSER, where ⌘P → "Save as PDF"
// produces the paginated document.
//
// TWO-RENDERER DISCIPLINE: every honesty string is sourced from
// ./aicheckReportModel — the SAME module AiCheckReport.tsx uses — never re-typed
// here. If you find a hardcoded honesty string below, that is the drift bug.
// Pinned by aicheckReportHtml.vitest.ts.
//
// Self-contained by construction: inline <style> only, hardcoded resolved rgba()
// (NOT color-mix, which is modern-only), print-color-adjust:exact so the
// highlight/table colors survive the PDF, a system font stack (no embedded
// fonts), and page-break-inside:avoid on findings/evidence/table blocks. NO
// React, NO fetch, NO external asset (/csl, /assets, <script>, <link>) — nothing
// but the string itself is needed to render it anywhere.
import { AiCheckAnalysis, AiCheckPassage, AiCheckResult, SignalEvidence } from './agentTypes';
import {
  EVIDENCE_GATE_NOTE,
  EVIDENCE_GROUPS,
  FOOTER_TAGLINE,
  LEGEND,
  PROPORTION_EXPLAINER,
  evidenceRows,
  resolveDisclaimer,
  segmentSections,
  splitSentence,
  statusLabel,
  tierLabel,
  tierOf,
} from './aicheckReportModel';

export interface ReportMeta {
  /** The manuscript's file name — the only datum not carried in AiCheckResult. */
  documentName: string;
}

/** HTML-escape for ELEMENT CONTENT (which is all this emitter produces — no
 *  dynamic value is ever placed in an attribute). Escaping `& < >` is sufficient
 *  and safe: `'` and `"` are literal in text content, so they pass through — which
 *  is what keeps the shared honesty strings (e.g. "isn't" in EVIDENCE_GATE_NOTE)
 *  byte-verbatim for the drift pins while still neutralizing markup injection. */
function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/** level → print color class (presentational, not an honesty string). */
function levelClass(e: SignalEvidence): string {
  if (e.status === 'measured' && e.level) return e.level; // high | moderate | low
  return 'muted';
}

function metaRows(result: AiCheckResult, meta: ReportMeta): Array<[string, string]> {
  const a = result.analysis;
  return [
    ['Document', esc(meta.documentName)],
    [
      'Analyzed',
      `${a.passages.length} flagged passages · ${a.total_chars.toLocaleString('en-US')} characters`,
    ],
    ['Models', `fast: ${esc(a.fast_model)} · deep: ${esc(a.deep_model ?? 'not available; all flags heuristic-only')}`],
    ['Budget', `deep-verified ${a.deep_verified} of ${a.candidates_found} candidate passages`],
  ];
}

function evidenceTableHtml(a: AiCheckAnalysis): string {
  const rows = evidenceRows(a);
  if (rows.length === 0) return '';
  const provisional = a.norms_provisional;
  const groups = EVIDENCE_GROUPS.map((g) => {
    const groupRows = rows.filter((r) => r.bias_tier === g.tier);
    if (groupRows.length === 0) return '';
    const head = `<div class="ev-band"><span class="ev-band__label">${esc(g.label)}</span>${
      g.caveat ? `<span class="ev-band__caveat"> (${esc(g.caveat)})</span>` : ''
    }</div>`;
    const body = groupRows
      .map((r) => {
        const sup =
          (r.signal === 'Language-model perplexity' || r.signal === 'Deep-verifier perplexity') && provisional
            ? '<sup>¹</sup>'
            : '';
        return `<div class="ev-row"><span class="ev-lvl ${levelClass(r)}">${esc(
          statusLabel(r),
        )}</span><span class="ev-txt"><span class="ev-sig">${esc(r.signal)}</span>: ${esc(
          r.detail,
        )}${sup}</span></div>`;
      })
      .join('');
    return `<div class="ev-group">${head}${body}</div>`;
  }).join('');
  const foot = provisional
    ? '<p class="ev-foot">¹ preliminary: norms not yet held-out-evaluated</p>'
    : '';
  return `<section class="block"><h2 class="sec">Evidence summary</h2><p class="gate">${esc(
    EVIDENCE_GATE_NOTE,
  )}</p>${groups}${foot}</section>`;
}

function passageEvidenceHtml(p: AiCheckPassage): string {
  const sentences = p.sentences
    .map((s) => `<tr><td>${esc(s.text)}</td><td class="mono">${s.perplexity.toFixed(1)}</td></tr>`)
    .join('');
  const gate =
    p.gate_flags.length > 0
      ? `<ul class="gate-flags">${p.gate_flags.map((g) => `<li>${esc(g)}</li>`).join('')}</ul>`
      : '';
  return (
    `<div class="pmeta"><span class="badge badge--${tierOf(p)}">${esc(tierLabel(p))}</span>` +
    `<span class="badge badge--neutral">strength: ${esc(p.strength)}</span>` +
    `<span class="mono dim">mean perplexity ${p.mean_perplexity.toFixed(1)} · burstiness ${p.burstiness.toFixed(
      1,
    )}</span></div>` +
    `<p class="note">${esc(p.depth_note)}</p>` +
    `<p class="note">${esc(p.uncertainty)}</p>` +
    `<p class="note">${esc(p.category_note)}</p>` +
    gate +
    `<table class="sent"><thead><tr><th>sentence</th><th>perplexity</th></tr></thead><tbody>${sentences}</tbody></table>`
  );
}

function findingsHtml(a: AiCheckAnalysis): string {
  const passages = a.passages;
  if (passages.length === 0) return '';
  const total = passages.length;
  const deep = passages.filter((p) => p.depth === 'deep_verified').length;
  const heuristic = total - deep;
  const items = passages
    .map(
      (p, i) =>
        `<div class="finding ev-group"><p class="finding__num">Passage ${i + 1}</p>` +
        `<blockquote class="finding__text">${esc(p.text)}</blockquote>${passageEvidenceHtml(p)}</div>`,
    )
    .join('');
  return `<section class="block"><h2 class="sec">Per-passage findings</h2><p class="split">${esc(
    splitSentence(deep, total, heuristic),
  )}</p>${items}</section>`;
}

function manuscriptHtml(result: AiCheckResult): string {
  const { rendered, unplaced } = segmentSections(result);
  const sections = rendered
    .map(({ section, segments }) => {
      const heading = section.heading ? `<h3 class="ms-head">${esc(section.heading)}</h3>` : '';
      const body = segments
        .map((seg) =>
          seg.kind === 'plain'
            ? esc(seg.text)
            : `<span class="hl ${tierOf(seg.passage)}">${esc(seg.passage.text)}</span>`,
        )
        .join('');
      return `<section class="ms-sec">${heading}<p class="ms-body">${body}</p></section>`;
    })
    .join('');
  const unplacedHtml =
    unplaced.length > 0
      ? `<div class="unplaced"><h3 class="ms-head">Flagged passages not shown above</h3>${unplaced
          .map((p) => `<p><span class="hl ${tierOf(p)}">${esc(p.text)}</span></p>`)
          .join('')}</div>`
      : '';
  const legend =
    `<p class="legend">${esc(LEGEND.plain)} <span class="hl assessed">amber</span>: ${esc(
      LEGEND.assessedTail,
    )} <span class="hl flagged">red</span>: ${esc(LEGEND.flaggedTail)} ${esc(LEGEND.levels)}</p>`;
  return `<section class="block"><h2 class="sec">Highlighted manuscript</h2>${legend}<div class="ms">${sections}</div>${unplacedHtml}</section>`;
}

function languageHtml(a: AiCheckAnalysis): string {
  const lang = a.language;
  if (lang.calibration_reliable) return `<p class="lang">${esc(lang.note)}</p>`;
  return `<section class="callout callout--warn"><span class="badge badge--flagged">non-English text: low confidence</span><p class="note">${esc(
    lang.note,
  )}</p></section>`;
}

const STYLE = `
  :root{--ink:#1b1b1f;--ink2:#45454d;--ink3:#86868b;--line:#e4e4e9;
    --flagged:#cf3323;--assessed:#b4530b;--certain:#12805c;}
  *{box-sizing:border-box}
  html,body{margin:0;background:#fff;color:var(--ink);
    font:15px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",Helvetica,Arial,sans-serif;}
  .page{max-width:760px;margin:0 auto;padding:40px 32px;}
  .hl,.ev-lvl,.badge{-webkit-print-color-adjust:exact;print-color-adjust:exact;}
  header.rep{border-bottom:2px solid var(--ink);padding-bottom:14px;margin-bottom:14px;}
  header.rep h1{font-size:26px;margin:0 0 10px;letter-spacing:-.01em;}
  .meta{display:grid;grid-template-columns:1fr;gap:3px;font-size:13px;color:var(--ink2);}
  .meta b{color:var(--ink);font-weight:600;}
  .callout{border:1px solid var(--line);border-left:3px solid var(--assessed);
    border-radius:6px;padding:12px 14px;margin:14px 0;background:#fbfafc;}
  .callout--warn{border-left-color:var(--flagged);}
  .disclaimer{font-size:12.5px;color:var(--assessed);border-left:3px solid var(--assessed);
    padding:10px 12px;line-height:1.5;margin:14px 0;background:#fbf7f2;border-radius:4px;}
  .lang{font-size:12.5px;color:var(--ink3);margin:12px 0;}
  .block{margin:22px 0;}
  h2.sec{font-size:12px;text-transform:uppercase;letter-spacing:.06em;color:var(--ink3);
    margin:0 0 8px;border-bottom:1px solid var(--line);padding-bottom:4px;}
  .prop{display:flex;align-items:baseline;gap:16px;flex-wrap:wrap;margin:6px 0;}
  .prop .num{font-size:40px;font-weight:700;}
  .prop .cap{font-size:13px;color:var(--ink3);max-width:520px;}
  .coverage{font-size:12.5px;color:var(--ink3);margin:8px 0 0;}
  .gate{font-size:12.5px;color:var(--ink2);margin:0 0 10px;}
  .ev-group{margin:0 0 12px;}
  .ev-band__label{font-size:11px;text-transform:uppercase;letter-spacing:.05em;color:var(--ink3);font-weight:600;}
  .ev-band__caveat{font-size:11px;color:var(--ink3);}
  .ev-row{display:flex;gap:10px;font-size:13px;padding:3px 0;border-top:1px solid var(--line);}
  .ev-lvl{font-weight:600;white-space:nowrap;min-width:82px;}
  .ev-lvl.high{color:var(--flagged);} .ev-lvl.moderate{color:var(--assessed);}
  .ev-lvl.low{color:var(--certain);} .ev-lvl.muted{color:var(--ink3);}
  .ev-sig{font-weight:600;color:var(--ink);}
  .ev-foot{font-size:11px;color:var(--ink3);margin:6px 0 0;}
  .split{font-size:12.5px;color:var(--ink2);margin:0 0 10px;}
  .finding{border:1px solid var(--line);border-radius:8px;padding:14px 16px;margin:10px 0;}
  .finding__num{font-weight:600;font-size:12px;color:var(--ink3);margin:0 0 4px;}
  .finding__text{margin:0 0 8px;font-style:italic;color:var(--ink);}
  .pmeta{display:flex;gap:8px;flex-wrap:wrap;align-items:center;margin:0 0 6px;}
  .badge{font-size:11px;font-weight:600;padding:2px 8px;border-radius:999px;border:1px solid var(--line);}
  .badge--flagged{color:#fff;background:var(--flagged);border-color:var(--flagged);}
  .badge--assessed{color:#fff;background:var(--assessed);border-color:var(--assessed);}
  .badge--neutral{color:var(--ink2);}
  .note{font-size:12.5px;color:var(--ink2);margin:4px 0;line-height:1.5;}
  .gate-flags{font-size:12px;color:var(--ink3);margin:4px 0;padding-left:18px;}
  .mono{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;}
  .dim{font-size:12px;color:var(--ink3);}
  table.sent{width:100%;border-collapse:collapse;font-size:12px;margin-top:8px;}
  table.sent th{text-align:left;color:var(--ink3);font-weight:500;padding:2px 8px 2px 0;}
  table.sent td{padding:3px 8px 3px 0;border-top:1px solid var(--line);vertical-align:top;}
  .legend{font-size:12.5px;color:var(--ink3);margin:0 0 10px;}
  .ms{line-height:1.75;color:var(--ink2);}
  .ms-head{font-size:11px;text-transform:uppercase;letter-spacing:.05em;color:var(--ink3);margin:16px 0 4px;}
  .ms-body{margin:0 0 8px;}
  .unplaced{margin-top:14px;}
  .hl{border-radius:3px;padding:1px 3px;color:var(--ink);}
  .hl.flagged{background:rgba(207,51,35,.26);box-shadow:inset 0 -2px 0 var(--flagged);}
  .hl.assessed{background:rgba(180,83,11,.26);box-shadow:inset 0 -2px 0 var(--assessed);}
  footer.rep{margin-top:28px;border-top:1px solid var(--line);padding-top:10px;font-size:11px;color:var(--ink3);}
  @page{margin:16mm 14mm;}
  @media print{
    .page{max-width:none;padding:0;}
    .finding,.ev-group,table.sent,.callout,.disclaimer{page-break-inside:avoid;break-inside:avoid;}
    h2.sec,.ms-head{page-break-after:avoid;break-after:avoid;}
  }
`;

/** Build the AI Check report as one self-contained HTML document (see file
 *  header). Pure: same input → same output, no I/O, no globals. */
export function buildAiCheckReportHtml(result: AiCheckResult, meta: ReportMeta): string {
  const a = result.analysis;
  const header = `<header class="rep"><h1>AI-writing signal report</h1><div class="meta">${metaRows(
    result,
    meta,
  )
    .map(([k, v]) => `<div><b>${esc(k)}</b>&nbsp; ${v}</div>`)
    .join('')}</div></header>`;

  const disclaimer = `<p class="disclaimer">${esc(resolveDisclaimer(a))}</p>`;

  const summary =
    `<section class="block"><h2 class="sec">Summary</h2><div class="prop">` +
    `<span class="num">${(a.ai_signal_proportion * 100).toFixed(1)}%</span>` +
    `<span class="cap">${esc(PROPORTION_EXPLAINER)}</span></div>` +
    `<p class="coverage">${esc(a.coverage_note)}</p></section>`;

  const paraphrase =
    `<section class="callout"><span class="badge badge--neutral">AI-generated vs AI-paraphrased</span>` +
    `<p class="note">${esc(a.classification_note)}</p>` +
    `<p class="note">${esc(a.paraphrase_caution)}</p></section>`;

  const footer = `<footer class="rep">${esc(meta.documentName)} · ${esc(FOOTER_TAGLINE)}</footer>`;

  const body =
    header +
    disclaimer +
    languageHtml(a) +
    summary +
    evidenceTableHtml(a) +
    findingsHtml(a) +
    paraphrase +
    manuscriptHtml(result) +
    footer;

  return (
    `<!doctype html><html lang="en"><head><meta charset="utf-8">` +
    `<meta name="viewport" content="width=device-width, initial-scale=1">` +
    `<title>AI-writing signal report — ${esc(meta.documentName)}</title>` +
    `<style>${STYLE}</style></head><body><div class="page">${body}</div></body></html>`
  );
}
