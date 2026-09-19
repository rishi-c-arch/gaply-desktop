// THE VITEST HALF OF THE CHECKLIST-LINE MIRROR. §11 D190.
//
// Two exporters render a checklist row: gaply-core's report_compose (the "Open
// full report" PDF) and exportPdf.ts (the "Export summary PDF"). They cannot
// share code, and before this they carried DIFFERENT FIELDS — the Rust one
// emitted `[met] requirement: detail`, the TS one `[x] requirement — detail`,
// and neither carried the journal's own sentence or the other pages it is
// stated on. A researcher got a different artefact from each button.
//
//   Rust asserts the artifact matches report_compose::checklist_lines
//   THIS asserts exportPdf.ts's checklistLines matches the same artifact
//
// Neither side can drop a field without one of the two failing. Same instrument
// as vocabulary.vitest.ts, applied to a rendering rather than a table.
import { describe, expect, it } from 'vitest';
import mirror from '../../generated/checklist_line.json';
import { checklistLines } from './exportPdf';
import type { ChecklistItem } from './reportTypes';

/** Rebuilt from the artifact's own INPUT, not from a copy of it — otherwise
 *  this would pin the TypeScript against a row TypeScript invented. */
function itemFromMirror(): ChecklistItem {
  const i = mirror.input;
  return {
    requirement: i.requirement,
    passed: i.passed,
    detail: i.detail,
    guideline_source: null,
    source_span: i.source_span,
    article_type: null,
    checked_field: null,
    unevaluable: i.unevaluable,
    also_from: i.also_from.map((s) => ({
      guideline_source: '',
      source_span: s.source_span,
      article_type: s.article_type,
    })),
  };
}

describe('checklist-line mirror — the two exporters carry the same fields', () => {
  it('the artifact is present and shaped as expected', () => {
    expect(Array.isArray(mirror.lines)).toBe(true);
    // POSITIVE COUNT: an emptied artifact would make every comparison below
    // trivially true, which is the vacuous shape this repo keeps catching.
    expect(mirror.lines.length).toBeGreaterThan(3);
    expect(mirror.input).toBeTruthy();
  });

  it('exportPdf renders exactly what Rust renders', () => {
    expect(checklistLines(itemFromMirror())).toEqual(mirror.lines);
  });

  it('the span leads and the corroboration follows', () => {
    const lines = checklistLines(itemFromMirror());
    expect(lines[1]).toContain("the journal's words");
    // THE SENTENCE BOTH EXPORTERS USED TO DROP.
    expect(lines.join('\n')).toContain('all original research manuscripts');
    expect(lines.join('\n')).toContain('[Matters Arising]');
  });

  // `unevaluable` is LATENT — no row reaches either exporter with it set today
  // (see the Rust test's comment). This is a unit assertion on the TS function,
  // not a claim that the export path handles the third state end to end.
  it('an undecidable row does not print as a failure', () => {
    const lines = checklistLines({ ...itemFromMirror(), unevaluable: true, passed: false });
    expect(lines[0].startsWith('[not decided]')).toBe(true);
    expect(lines[0].startsWith('[not met]')).toBe(false);
  });
});
