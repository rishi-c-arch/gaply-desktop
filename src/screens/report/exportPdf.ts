// Gaply — local, publication-ready PDF export of a PublishReadyReport. Runs
// entirely on-device — nothing is uploaded. Uses the dependency-free miniPdf
// writer (jsPDF was dropped: its module init mutates a prototype, which the
// app's Object.freeze(Object.prototype) hardening — a real prototype-pollution
// defense — rejects). Findings are emitted in priority order with certainty
// tier + provenance, plus the mandatory disclaimer.
import { AGENT_LABEL, PublishReadyReport, sortFindings } from './reportTypes';
import { disclosuresFor, PdfLine, renderTextPdf } from './miniPdf';

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
    lines.push({ text: `${c.passed ? '[x]' : '[ ]'} ${c.requirement} — ${c.detail}`, size: 9, gray: 0.3 });
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

/** Trigger a local download of the PDF (browser/Tauri webview). */
export async function downloadReportPdf(report: PublishReadyReport, filename = 'gaply-report.pdf'): Promise<void> {
  const blob = await reportPdfBlob(report);
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
