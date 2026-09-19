// Gaply — local, publication-ready PDF export of a PublishReadyReport. Runs
// entirely on-device — nothing is uploaded. Uses the dependency-free miniPdf
// writer (jsPDF was dropped: its module init mutates a prototype, which the
// app's Object.freeze(Object.prototype) hardening — a real prototype-pollution
// defense — rejects). Findings are emitted in priority order with certainty
// tier + provenance, plus the mandatory disclaimer.
import { AGENT_LABEL, ChecklistItem, PublishReadyReport, sortFindings } from './reportTypes';
import { disclosuresFor, PdfLine, renderTextPdf } from './miniPdf';
import { saveBinaryFile } from '../../utils/saveBinaryFile';


/** **The lines ONE checklist row becomes. §11 D190 — MIRROR of Rust.**
 *
 *  `gaply-core/src/report_compose.rs::checklist_lines` is the same function in
 *  the other exporter. They cannot share code — different languages, different
 *  renderers — so they share a SHAPE, pinned by `generated/checklist_line.json`:
 *  Rust asserts its version matches the artifact, `checklist_line.vitest.ts`
 *  asserts this one does, and neither can drop a field without a test failing.
 *
 *  The prefix used to be `[x]`/`[ ]` here and `[met]`/`[not met]` there — a
 *  divergence nobody chose, which is exactly how the two artefacts a researcher
 *  gets from the two buttons drift apart. Unified deliberately.
 *
 *  The span LEADS because it is the evidence: this exporter used to emit the
 *  verdict with none of the journal's own words to check it against. `also_from`
 *  follows because it is the corroboration — the backend refuses to choose
 *  between sources, and an export showing one of them un-refuses on its behalf. */
export function checklistLines(c: ChecklistItem): string[] {
  // Three states; `passed` is a bool and compliance is not.
  const status = c.unevaluable ? 'not decided' : c.passed ? 'met' : 'not met';
  const out = [`[${status}] ${c.requirement}: ${c.detail}`];
  if (c.source_span) {
    // Whole, never clipped: a truncated span is not a span.
    out.push(`the journal's words: \u201c${c.source_span}\u201d`);
  }
  const also = c.also_from ?? [];
  if (also.length > 0) {
    out.push(`also stated on ${also.length} other page(s); Gaply does not choose between them:`);
    for (const s of also) {
      const at = s.article_type ? `[${s.article_type}] ` : '';
      out.push(`${at}\u201c${s.source_span}\u201d`);
    }
  }
  return out;
}

function reportLines(report: PublishReadyReport, title: string): PdfLine[] {
  const lines: PdfLine[] = [];
  lines.push({ text: title, size: 20 });
  lines.push({
    text: `Overall verdict: ${report.verdict.toUpperCase()}  ·  combined confidence ${(report.combined_confidence * 100).toFixed(0)}%`,
    size: 11,
    gray: 0.4,
  });
  lines.push({ text: ' ', size: 6 });

  lines.push({ text: 'Findings (priority order)', size: 14 });
  for (const f of sortFindings(report.findings)) {
    lines.push({ text: `• [${f.severity.toUpperCase()}] ${f.title}`, size: 11 });
    lines.push({
      text: `   ${AGENT_LABEL[f.agent]} — ${f.certainty_label} (confidence ${(f.confidence * 100).toFixed(0)}%)`,
      size: 9,
      gray: 0.45,
    });
    if (f.reconsidered) {
      lines.push({ text: `   reconsidered: ${f.reconsidered.from} -> ${f.reconsidered.to}`, size: 9, gray: 0.5 });
    }
    lines.push({ text: `   ${f.detail}`, size: 9, gray: 0.3 });
    lines.push({ text: `   provenance: ${f.provenance.join('; ')}`, size: 8, gray: 0.55 });
  }

  lines.push({ text: ' ', size: 6 });
  lines.push({ text: 'PublishReady checklist', size: 14 });
  for (const c of report.checklist) {
    const rendered = checklistLines(c);
    lines.push({ text: rendered[0], size: 9, gray: 0.3 });
    for (const extra of rendered.slice(1)) {
      lines.push({ text: `   ${extra}`, size: 8, gray: 0.5 });
    }
  }

  lines.push({ text: ' ', size: 8 });
  lines.push({ text: report.disclaimer, size: 8, gray: 0.5 });

  // §4.23: this renderer folds accented letters to their base form and marks
  // what it cannot represent. It said neither. The disclosures are emitted only
  // when the transformation actually occurred, computed from the strings that
  // went in — the composer owns the wording, the renderer owns whether it
  // applies.
  for (const note of disclosuresFor(lines.map((l) => l.text))) {
    lines.push({ text: note, size: 8, gray: 0.5 });
  }
  return lines;
}

/** Return the report as a PDF Blob (application/pdf). */
export async function reportPdfBlob(report: PublishReadyReport, title = 'Gaply Integrity Report'): Promise<Blob> {
  return renderTextPdf(reportLines(report, title));
}

/**
 * Save the report as a PDF (§11 D91).
 *
 * This used to click an `<a download>` unconditionally — the browser pattern,
 * inside the desktop webview, which is exactly what `saveTextFile` was written
 * to stop doing for text. The bytes were always a valid PDF; the file that
 * arrived was not one. `saveBinaryFile` takes the native dialog on the desktop
 * and keeps the anchor for real browsers.
 *
 * Returns the saved path on the desktop, or null when the user cancelled or the
 * browser handled it.
 */
export async function downloadReportPdf(
  report: PublishReadyReport,
  filename = 'gaply-report.pdf',
): Promise<string | null> {
  const blob = await reportPdfBlob(report);
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return saveBinaryFile(filename, bytes, 'application/pdf');
}
