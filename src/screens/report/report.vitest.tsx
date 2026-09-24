// F6 — report viewer tests. Golden fixture + pure helpers + PDF export.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import ReportViewerPage, { MANUSCRIPT_NOT_IN_REPORT, reportDisclaimerFallback } from './ReportViewerPage';
import vocab from '../../generated/vocabulary.json';
import { SAMPLE_MANUSCRIPT_SECTIONS, SAMPLE_REPORT } from './sampleReport';
import { sortFindings, tierStatus, Finding, PublishReadyReport, ReportTab } from './reportTypes';
import { reportPdfBlob } from './exportPdf';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const offlineAuth: AuthService = {
  signUp: vi.fn(),
  signIn: vi.fn(),
  signInWithOAuth: vi.fn(),
  signOut: vi.fn(),
  getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
  onAuthStateChange: (cb: any) => {
    cb(null);
    return () => {};
  },
} as any;

function renderReport(report?: PublishReadyReport, isPaid = false, extra: any = {}) {
  return render(
    <MemoryRouter>
      <GaplySessionProvider authService={offlineAuth}>
        <ReportViewerPage report={report} isPaid={isPaid} {...extra} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

/* ------------------------------ golden render ------------------------------ */

describe('golden render against a sample compile_report() output', () => {
  it('renders the three-panel viewer with tabs, outline, manuscript, inspector', async () => {
    // Sections passed EXPLICITLY: since §11 D223 there is no sample default, and
    // the outline renders only when a body was given.
    renderReport(undefined, false, { manuscriptSections: SAMPLE_MANUSCRIPT_SECTIONS });
    await screen.findByTestId('report-viewer');
    // top tabs (free = 6, no Reviewer Letter)
    for (const t of ['Overview', 'Statistics', 'Citations', 'AI Risk', 'Plagiarism', 'Checklist']) {
      expect(screen.getByTestId(`tab-${t}`)).toBeTruthy();
    }
    expect(screen.queryByTestId('tab-Reviewer Letter')).toBeNull();
    // outline tree + manuscript + inspector
    expect(screen.getByTestId('outline')).toBeTruthy();
    expect(screen.getByTestId('manuscript')).toBeTruthy();
    expect(screen.getByTestId('inspector')).toBeTruthy();
    // mandatory disclaimer, verbatim-ish
    expect(screen.getByTestId('report-disclaimer').textContent).toMatch(/never definitive proof/i);
  });

  it('the mandatory disclaimer is NEVER empty — a hardcoded fallback backstops a wire regression (L4)', async () => {
    // wire regressed the required disclaimer to empty → the fallback renders, NOT an empty <p>
    renderReport({ ...SAMPLE_REPORT, disclaimer: '' });
    await screen.findByTestId('report-viewer');
    const guarded = screen.getByTestId('report-disclaimer').textContent!.trim();
    expect(guarded.length).toBeGreaterThan(0);
    expect(guarded).toMatch(/never definitive proof/i);
    cleanup();
    // NORMAL PATH unchanged: a present wire value renders verbatim, fallback dormant
    renderReport(SAMPLE_REPORT);
    await screen.findByTestId('report-viewer');
    expect(screen.getByTestId('report-disclaimer').textContent).toBe(SAMPLE_REPORT.disclaimer);
  });

  it('paid PublishReady adds the Reviewer Letter tab', async () => {
    renderReport(SAMPLE_REPORT, true);
    await screen.findByTestId('report-viewer');
    expect(screen.getByTestId('tab-Reviewer Letter')).toBeTruthy();
  });
});

/* -------------------- hard constraints sort first ------------------------- */

describe('priority ordering', () => {
  it('green hard-constraint findings sort first regardless of AI-assessed confidence', () => {
    // An AI-assessed finding at MAX confidence must NOT outrank a critical
    // mathematically-certain finding at lower confidence.
    const ai: Finding = {
      severity: 'major',
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'ai_detection',
      title: 'AI concern (confident)',
      detail: '',
      confidence: 0.99,
      provenance: ['swarm:round-table'],
    };
    const hard: Finding = {
      severity: 'critical',
      tier: 'mathematically_certain',
      certainty_label: 'mathematically certain',
      agent: 'validation_maths',
      title: 'rule failed',
      detail: '',
      confidence: 0.5,
      provenance: ['rule:MissingEffectSize'],
    };
    const sorted = sortFindings([ai, hard]);
    expect(sorted[0]).toBe(hard);
    expect(sorted[0].tier).toBe('mathematically_certain');
    expect(sorted[0].severity).toBe('critical');
  });

  it('the viewer lists the critical green finding first for the sample report', async () => {
    renderReport();
    await screen.findByTestId('finding-0');
    const first = screen.getByTestId('finding-0');
    expect(first.getAttribute('data-tier')).toBe('mathematically_certain');
  });
});

/* ----------------- provenance + tier label on every finding --------------- */

describe('provenance + certainty tier on every finding', () => {
  it('every finding carries provenance and its human tier label', () => {
    for (const f of SAMPLE_REPORT.findings) {
      expect(f.provenance.length).toBeGreaterThan(0);
      // label must match its tier
      const expected = {
        mathematically_certain: 'mathematically certain',
        ai_assessed_moderate: 'AI-assessed, moderate confidence',
        reconsidered_after_peer_review: 'reconsidered after peer review',
      }[f.tier];
      expect(f.certainty_label).toBe(expected);
    }
  });

  it('clicking a finding shows its provenance + tier in the inspector', async () => {
    renderReport();
    const first = await screen.findByTestId('finding-0');
    fireEvent.click(first);
    const inspector = screen.getByTestId('inspector');
    expect(within(inspector).getByTestId('inspector-tier').textContent).toBe('mathematically certain');
    const prov = within(inspector).getByTestId('inspector-provenance');
    expect(prov.textContent).toMatch(/rule:MissingEffectSize/);
  });

  it('a reconsidered verdict shows before → after', async () => {
    renderReport();
    await screen.findByTestId('report-viewer');
    // find the reconsidered finding and select it
    const idx = sortFindings(SAMPLE_REPORT.findings).findIndex((f) => f.reconsidered);
    fireEvent.click(screen.getByTestId(`finding-${idx}`));
    const recon = screen.getByTestId('inspector-reconsidered');
    expect(recon.textContent).toMatch(/SUPPORTED/);
    expect(recon.textContent).toMatch(/UNKNOWN/);
  });
});

/* ------------------------------- tier colors ------------------------------ */

describe('tier → status color mapping', () => {
  it('maps the three tiers to green/amber/red', () => {
    expect(tierStatus('mathematically_certain')).toBe('certain');
    expect(tierStatus('ai_assessed_moderate')).toBe('assessed');
    expect(tierStatus('reconsidered_after_peer_review')).toBe('flagged');
  });
});

/* --------------------------------- export --------------------------------- */

describe('local PDF export', () => {
  it('produces a non-empty application/pdf Blob', async () => {
    const blob = await reportPdfBlob(SAMPLE_REPORT);
    expect(blob).toBeInstanceOf(Blob);
    expect(blob.type).toBe('application/pdf');
    expect(blob.size).toBeGreaterThan(500);
  });

  it('export button triggers a local download (no network)', async () => {
    const fetchSpy = vi.fn();
    const origFetch = global.fetch;
    (global as any).fetch = fetchSpy;
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    // jsdom lacks createObjectURL
    (URL as any).createObjectURL = vi.fn(() => 'blob:mock');
    (URL as any).revokeObjectURL = vi.fn();

    renderReport();
    fireEvent.click(await screen.findByTestId('export-pdf'));

    await waitFor(() => expect(clickSpy).toHaveBeenCalled()); // a file download was triggered
    expect(fetchSpy).not.toHaveBeenCalled(); // generated locally, no upload

    clickSpy.mockRestore();
    global.fetch = origFetch;
  });
});

/* ------------- D188: every source shown, and the third state -------------- */

describe('checklist rows carry every page the journal states them on (§11 D188)', () => {
  // The backend refuses to pick a best source because three selection rules
  // were measured and all three fail. That refusal is only worth anything if
  // the screen shows what it carried.
  const withSources = (extra: any): PublishReadyReport =>
    ({
      ...SAMPLE_REPORT,
      checklist: [
        {
          requirement: 'data availability statement',
          passed: false,
          detail: 'none of 3 phrasings was found anywhere in the manuscript',
          guideline_source: 'https://www.nature.com/nm/aims/fasttrack',
          source_span: 'all fast track submissions must include the following',
          article_type: null,
          checked_field: 'manuscript full text',
          ...extra,
        },
      ],
    }) as any;

  it('shows every carried source, with its own article type', () => {
    renderReport(
      withSources({
        also_from: [
          {
            guideline_source: 'https://www.nature.com/nm/editorial-policies/clinicalresearch',
            source_span: 'must be included with all original research manuscripts',
            article_type: null,
          },
          {
            guideline_source: 'https://www.nature.com/nm/submission-guidelines/matters-arising',
            source_span: 'A statement is required.',
            article_type: 'Matters Arising',
          },
        ],
      }),
      true
    );
    fireEvent.click(screen.getByTestId('tab-Checklist'));
    const box = screen.getByTestId('check-sources-0');
    expect(box.textContent).toMatch(/states this on 3 pages/);
    // THE SENTENCE THAT USED TO BE DISCARDED.
    expect(box.textContent).toMatch(/all original research manuscripts/);
    // …and the narrow one is not hidden: showing BOTH is the point, because a
    // reader can tell which scope covers them and the code cannot.
    expect(box.textContent).toMatch(/fast track/);
    // The per-source article type survives — one page scopes it, another does not.
    expect(box.textContent).toMatch(/\[Matters Arising\]/);
  });

  // **THE NEGATIVE IS PAIRED WITH ITS POSITIVE, and the deletion test is why.**
  // Asserting only "no box for one source" passed while the corroboration
  // feature was deleted outright — it cannot tell "no box because there is one
  // source" from "no box ever". Rendering both cases in one test makes that
  // impossible: remove the feature and the second half fails.
  it('the box appears for several sources and not for one', () => {
    const { unmount } = renderReport(withSources({}), true);
    fireEvent.click(screen.getByTestId('tab-Checklist'));
    expect(screen.queryByTestId('check-sources-0')).toBeNull();
    unmount();

    renderReport(
      withSources({
        also_from: [
          {
            guideline_source: 'https://example.test/other',
            source_span: 'stated again elsewhere',
            article_type: null,
          },
        ],
      }),
      true
    );
    fireEvent.click(screen.getByTestId('tab-Checklist'));
    expect(screen.getByTestId('check-sources-0')).toBeTruthy();
  });

  it('an UNDECIDABLE row is not drawn as a failure', () => {
    renderReport(withSources({ unevaluable: true, passed: false }), true);
    fireEvent.click(screen.getByTestId('tab-Checklist'));
    const mark = screen.getByTestId('check-mark-0');
    // Neither a tick nor a cross: `passed: false` + `unevaluable: true` used to
    // render '✗', telling a researcher they failed a check nobody could run.
    expect(mark.textContent).not.toBe('✗');
    expect(mark.textContent).not.toBe('✓');
    expect(screen.getByTestId('check-undecided-0').textContent).toMatch(/not a finding about your manuscript/i);
  });
});

/* --------- the empty-checklist sentence, where no note was kept ----------- */

/** **§11 D192 — the fallback, and why it is still here.**
 *
 *  The live path renders the sentence the backend composed; a report reopened
 *  from storage kept no note, so this text is what a reader meets there. What it
 *  no longer says is *"That page was fetched successfully"* — that was asserted
 *  for a 404, a rate-limited host and a bot interstitial alike, by the layer
 *  furthest from the evidence and with nothing available to it that could tell
 *  them apart. The backend now carries that distinction (`reached`); this
 *  surface simply stops claiming it. */
describe('§11 D192 · the checklist sentence for a URL that yielded nothing', () => {
  const emptyChecklistReport = {
    ...(SAMPLE_REPORT as any),
    checklist: ((SAMPLE_REPORT as any).checklist ?? []).filter((c: any) => !c.guideline_source),
  };

  it('renders the backend sentence verbatim when there is one', async () => {
    const note = 'That page was reached, and no author guidance was found on it: X.';
    renderReport(emptyChecklistReport as any, true, {
      guidelinesUrl: 'https://www.bmj.com',
      guidelinesNote: note,
    });
    fireEvent.click(await screen.findByTestId('tab-Checklist'));
    expect((await screen.findByTestId('checklist-guidelines-empty')).textContent).toBe(note);
  });

  it('falls back without claiming the page was fetched successfully', async () => {
    renderReport(emptyChecklistReport as any, true, { guidelinesUrl: 'https://www.bmj.com' });
    fireEvent.click(await screen.findByTestId('tab-Checklist'));
    const el = await screen.findByTestId('checklist-guidelines-empty');
    expect(el.textContent).toContain('https://www.bmj.com');
    expect(el.textContent).toMatch(/requirements that Gaply currently detects/i);
    // The claim this surface cannot make. A stored report does not record
    // whether the fetch landed, so the sentence must not decide that it did.
    expect(el.textContent).not.toMatch(/fetched successfully/i);
  });
});

// **§11 D221.** The fallback and the sample were hardcoded copies of the
// pre-D220 disclaimer. Both now read Rust's `disclaimer_for` through the mirror;
// its wording is pinned in Rust (`the_disclaimer_says_what_deterministic_findings_state`).
describe('report disclaimer copies read Rust (§11 D221)', () => {
  it('the fallback picks the form Rust would produce, by whether anything revised', () => {
    const none = { ...SAMPLE_REPORT, debate: { ...SAMPLE_REPORT.debate, revised_agents: [] } };
    expect(reportDisclaimerFallback(none)).toBe(vocab.report_disclaimer.no_revision);
    expect(SAMPLE_REPORT.debate.revised_agents.length).toBeGreaterThan(0); // precondition
    expect(reportDisclaimerFallback(SAMPLE_REPORT)).toBe(vocab.report_disclaimer.with_revision);
  });

  it('an empty wire disclaimer renders the fallback', async () => {
    const none = { ...SAMPLE_REPORT, disclaimer: '', debate: { ...SAMPLE_REPORT.debate, revised_agents: [] } };
    renderReport(none);
    await screen.findByTestId('report-viewer');
    expect(screen.getByTestId('report-disclaimer').textContent).toBe(vocab.report_disclaimer.no_revision);
  });

  it('the sample report carries what Rust produces for a run with a revision', () => {
    expect(SAMPLE_REPORT.debate.revised_agents.length).toBeGreaterThan(0); // precondition
    expect(SAMPLE_REPORT.disclaimer).toBe(vocab.report_disclaimer.with_revision);
  });
});

/* --------------------- §11 D222/D223: no other document ---------------------- */

// Shaped like run 32's cached report (9 findings: 2 validation, 1 verification,
// 3 ai_detection, extraction + rag, NO plagiarism), with synthetic titles — the
// real one quotes the user's manuscript and is not committed. No title names a
// manuscript section, so a section name on screen can only come from the viewer.
function run32Shaped(): PublishReadyReport {
  const f = (agent: any, severity: any, tier: any, title: string): Finding => ({
    agent, severity, tier, title, detail: `${title} detail`, confidence: 0.5,
    certainty_label: tier === 'mathematically_certain' ? 'not detected by an automated check' : 'AI-assessed, moderate confidence',
    provenance: ['rule:x'],
  } as Finding);
  return {
    ...SAMPLE_REPORT,
    checklist: [],
    findings: [
      f('validation_maths', 'major', 'mathematically_certain', 'stat rule one'),
      f('validation_maths', 'major', 'mathematically_certain', 'stat rule two'),
      f('ai_detection', 'minor', 'ai_assessed_moderate', 'ai signal one'),
      f('ai_detection', 'minor', 'ai_assessed_moderate', 'ai signal two'),
      f('extraction', 'minor', 'ai_assessed_moderate', 'extraction note'),
      f('verification', 'minor', 'ai_assessed_moderate', 'citations could not be checked'),
      f('extraction', 'info', 'ai_assessed_moderate', 'extraction pass'),
      f('ai_detection', 'info', 'ai_assessed_moderate', 'ai concern'),
      f('rag', 'info', 'ai_assessed_moderate', 'rag pass'),
    ],
    debate: { ...SAMPLE_REPORT.debate, revised_agents: [] },
  };
}
const PR_TABS = ['Overview', 'Statistics', 'Citations', 'AI Risk', 'Plagiarism', 'Checklist', 'Reviewer Letter'] as ReportTab[];

describe('the viewer shows no other document (§11 D223)', () => {
  // ACCEPTANCE: with no manuscriptSections — every production caller — no sample
  // sentence and no sample section name appears on ANY tab, and there is no
  // outline. The card says why it is empty.
  it('with no body passed, no sample sentence or section name appears on any tab', async () => {
    renderReport(run32Shaped(), false, { tabs: PR_TABS });
    await screen.findByTestId('report-viewer');
    expect(screen.getByTestId('manuscript-not-in-report').textContent).toBe(MANUSCRIPT_NOT_IN_REPORT);
    for (const t of PR_TABS) {
      fireEvent.click(screen.getByTestId(`tab-${t}`));
      const body = document.body.textContent ?? '';
      for (const s of SAMPLE_MANUSCRIPT_SECTIONS) {
        expect(body, `${t}: sample sentence`).not.toContain(s.text);
        expect(body, `${t}: sample section name`).not.toContain(s.section);
      }
      expect(screen.queryByTestId('outline'), `${t}: outline`).toBeNull();
    }
  });

  // NEGATIVE CONTROL: the feature is not deleted — a body passed explicitly
  // still renders in the card and the outline, and the empty-state line does not.
  it('an explicitly passed body still renders', async () => {
    const sections = [{ section: 'Methods', text: 'A caller-supplied sentence.' }];
    renderReport(run32Shaped(), false, { manuscriptSections: sections });
    await screen.findByTestId('report-viewer');
    expect(screen.getByTestId('manuscript').textContent).toContain('A caller-supplied sentence.');
    expect(screen.getByTestId('outline-Methods')).toBeTruthy();
    expect(screen.queryByTestId('manuscript-not-in-report')).toBeNull();
  });
});
