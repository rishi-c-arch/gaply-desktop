// Gaply — the AI Check HTML export emitter. Pure-function tests + the
// SHARED-STRING PINS that stop the two renderers (AiCheckReport.tsx and this
// emitter) from drifting: every honesty string in the exported HTML must be the
// SAME constant the on-screen report uses, sourced by reference here — never a
// re-typed copy. A string hardcoded in the emitter fails these pins.
import { describe, expect, it } from 'vitest';

import { buildAiCheckReportHtml } from './aicheckReportHtml';
import {
  EVIDENCE_GATE_NOTE,
  FOOTER_TAGLINE,
  LEGEND,
  PROPORTION_EXPLAINER,
  resolveDisclaimer,
  splitSentence,
} from './aicheckReportModel';
import { AICHECK_FIXTURE, AICHECK_FIXTURE_SPANISH } from './aicheckFixture';

const DOC = 'my-thesis-ch3.pdf';
const build = (r = AICHECK_FIXTURE) => buildAiCheckReportHtml(r, { documentName: DOC });

describe('aicheckReportHtml — shared-string pins (no second-renderer drift)', () => {
  it('the disclaimer is the wire value verbatim (resolveDisclaimer), not re-typed', () => {
    const html = build();
    // sourced from the SAME resolver the screen uses
    expect(html).toContain(resolveDisclaimer(AICHECK_FIXTURE.analysis));
    expect(html).toContain(AICHECK_FIXTURE.analysis.disclaimer);
  });

  it('the proportion explainer comes from the shared constant', () => {
    expect(build()).toContain(PROPORTION_EXPLAINER);
  });

  it('the evidence gate note comes from the shared constant', () => {
    expect(build()).toContain(EVIDENCE_GATE_NOTE);
  });

  it('the per-passage split sentence comes from the shared function', () => {
    // fixture: 2 passages, 1 deep-verified, 1 heuristic-only
    expect(build()).toContain(splitSentence(1, 2, 1));
  });

  it('the footer is "{document} · signal, not proof", tagline from the shared constant', () => {
    const html = build();
    expect(html).toContain(`${DOC} · ${FOOTER_TAGLINE}`);
    expect(FOOTER_TAGLINE).toBe('signal, not proof');
  });

  it('the legend prose comes from the shared LEGEND constant', () => {
    const html = build();
    expect(html).toContain(LEGEND.plain);
    expect(html).toContain(LEGEND.assessedTail);
    expect(html).toContain(LEGEND.flaggedTail);
    expect(html).toContain(LEGEND.levels);
  });

  it('the wire cautions (coverage, classification, paraphrase, language) render verbatim', () => {
    const html = build();
    const a = AICHECK_FIXTURE.analysis;
    expect(html).toContain(a.coverage_note);
    expect(html).toContain(a.classification_note);
    expect(html).toContain(a.paraphrase_caution);
    expect(html).toContain(a.language.note);
  });
});

describe('aicheckReportHtml — self-contained + pure', () => {
  it('is one HTML document with no external asset, script, or fetch', () => {
    const html = build();
    expect(html.startsWith('<!doctype html>')).toBe(true);
    expect(html).toContain('</html>');
    expect(html).not.toContain('<script');
    expect(html).not.toContain('<link ');
    expect(html).not.toContain('http://');
    expect(html).not.toContain('https://');
    expect(html).not.toContain('/csl');
    expect(html).not.toContain('/assets');
    // no external resource attributes (self-contained; only inline <style>)
    expect(html).not.toMatch(/\ssrc=/);
    expect(html).not.toMatch(/\shref=/);
  });

  it('is deterministic — same input yields byte-identical output', () => {
    expect(build()).toBe(build());
  });

  it('carries the header meta (Document · Analyzed · Models · Budget)', () => {
    const html = build();
    expect(html).toContain('AI-writing signal report');
    expect(html).toContain('<b>Document</b>');
    expect(html).toContain('<b>Analyzed</b>');
    expect(html).toContain('<b>Models</b>');
    expect(html).toContain('<b>Budget</b>');
    expect(html).toContain(DOC);
    // the % renders, formatted like the screen
    expect(html).toContain('42.0%');
    // NEVER the word "probability" (the honest-% invariant)
    expect(html.toLowerCase()).not.toContain('probability');
  });
});

describe('aicheckReportHtml — highlighting + evidence colors survive', () => {
  it('the manuscript keeps the two-tier highlight spans with resolved rgba (not color-mix)', () => {
    const html = build();
    // both flagged passages appear inside a highlight span
    expect(html).toContain('<span class="hl flagged">');
    expect(html).toContain('<span class="hl assessed">');
    expect(html).toContain(AICHECK_FIXTURE.analysis.passages[0].text); // deep → flagged
    expect(html).toContain(AICHECK_FIXTURE.analysis.passages[1].text); // heuristic → assessed
    // hardcoded resolved colors + the print flag that stops browsers stripping them
    expect(html).toContain('rgba(207,51,35,.26)'); // --flagged #cf3323
    expect(html).toContain('rgba(180,83,11,.26)'); // --assessed #b4530b
    expect(html).toContain('print-color-adjust:exact');
    expect(html).not.toContain('color-mix');
  });

  it('the evidence table renders every row detail and its status label', () => {
    const html = build();
    for (const r of AICHECK_FIXTURE.analysis.document_score.evidence) {
      expect(html).toContain(r.detail);
    }
    expect(html).toContain('Unavailable'); // the citation-density row is status:unavailable
  });

  it('keeps a passage whole across page breaks (page-break-inside: avoid on findings)', () => {
    expect(build()).toContain('page-break-inside:avoid');
  });
});

describe('aicheckReportHtml — untrusted text is escaped (no HTML injection)', () => {
  it('escapes markup in the manuscript body and passage text', () => {
    const evil = {
      ...AICHECK_FIXTURE,
      sections: [
        {
          kind: 'introduction',
          heading: 'Introduction',
          text: 'A safe sentence. <script>alert(1)</script> and <b>bold</b> tags.',
        },
      ],
    };
    const html = buildAiCheckReportHtml(evil, { documentName: '<img src=x>.pdf' });
    // the injected script is escaped as text, never live markup
    expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;');
    expect(html).not.toContain('<script>alert(1)</script>');
    // the doc name is escaped too (title + footer)
    expect(html).toContain('&lt;img src=x&gt;.pdf');
  });
});

describe('aicheckReportHtml — language downgrade path', () => {
  it('non-English renders the low-confidence banner (not the plain calibration note)', () => {
    const html = buildAiCheckReportHtml(AICHECK_FIXTURE_SPANISH, { documentName: DOC });
    expect(html).toContain('non-English text: low confidence');
    expect(html).toContain(AICHECK_FIXTURE_SPANISH.analysis.language.note);
  });
});
