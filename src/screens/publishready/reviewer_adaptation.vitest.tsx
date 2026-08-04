// Set 4d — ReviewerLetterPanel adaptation to the current backend output.
// Verifies the two states the browser premium-gate blocks in dev: an adapted
// "available" letter (empty assessment/note, [] alternatives, gated issues) and
// the honest "unavailable offline" state. Real React render (no faked fields).
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import ReviewerLetterPanel from './ReviewerLetterPanel';
import { adaptOutcome } from './publishReadyBridge';
import { ReviewerLetter } from './publishReadyTypes';

afterEach(cleanup);

const available: ReviewerLetter = {
  recommendation: 'major_revision',
  publicationProbability: 61,
  novelty: { assessment: '' }, // ungrounded -> gate emptied it
  journalFit: { journal: 'Nature', quartile: 'Q1', note: '' }, // ungrounded -> gate emptied it
  alternatives: [], // none yet
  body: 'The manuscript is promising but needs revision before it can be accepted.',
  available: true,
  issues: [{ findingRef: 'f1', severity: 'major', rationale: 'citation c1 refuted; address it' }],
  warnings: ['potential_hallucination: issue cites finding "f9" not provided; dropped'],
};

const unavailable: ReviewerLetter = {
  recommendation: 'unknown',
  publicationProbability: 0,
  novelty: { assessment: '' },
  journalFit: { journal: 'Nature', quartile: 'Q1', note: '' },
  alternatives: [],
  body: 'deep reasoning requires cloud analysis — unavailable offline',
  available: false,
  issues: [],
  warnings: ['reviewer unavailable: cloud proxy not reachable'],
};

const renderPanel = (letter: ReviewerLetter) =>
  render(
    <MemoryRouter>
      <ReviewerLetterPanel letter={letter} />
    </MemoryRouter>
  );

describe('ReviewerLetterPanel — backend adaptation (Option B)', () => {
  it('renders the available letter with gated issues, omitting fields the backend lacks', () => {
    renderPanel(available);
    expect(screen.getByTestId('pr-verdict').textContent).toMatch(/MAJOR REVISION/);
    expect(screen.getByTestId('pr-probability').textContent).toMatch(/61%/);
    expect(screen.getByTestId('pr-body')).toBeTruthy();
    // gated issues render honestly (finding_ref grounded)
    expect(screen.getByTestId('pr-issues')).toBeTruthy();
    expect(screen.getByTestId('pr-issues').textContent).toMatch(/f1/);
    // gate warnings surfaced (dropped hallucination)
    expect(screen.getByTestId('pr-warnings').textContent).toMatch(/potential_hallucination/);
    // empty assessment -> em dash, never "undefined"
    expect(screen.getByTestId('pr-novelty').textContent).toBe('—');
    // no alternatives produced -> the card is cleanly omitted, not "undefined"
    expect(screen.queryByTestId('pr-alternatives')).toBeNull();
  });

  // §4.14 at the UI: "did not affect the recommendation" is NOT "unimportant".
  // A user who sees a finding in the report but not in the verdict must be told
  // why, and the reason must not be buried in the gate-notes line.
  it('renders excluded findings separately from gate notes, with reasons', () => {
    renderPanel({
      ...available,
      warnings: [
        'potential_hallucination: dropped an ungrounded issue',
        "excluded from the recommendation: f1 — a statistical signal about authorship, not a publishability defect",
        "excluded from the recommendation: f4 — describes Gaply's own execution, not the manuscript",
      ],
    });
    const ex = screen.getByTestId('pr-excluded').textContent ?? '';
    expect(ex).toMatch(/f1/);
    expect(ex).toMatch(/not a publishability defect/);
    expect(ex).toMatch(/f4/);
    expect(ex).toMatch(/Gaply's own execution/);
    // and NOT collapsed into the gate-notes line
    const notes = screen.getByTestId('pr-warnings').textContent ?? '';
    expect(notes).toMatch(/potential_hallucination/);
    expect(notes).not.toMatch(/excluded from the recommendation/);
  });

  // The PARTIAL case's caveat, at the last boundary. Accept plus a caveat is
  // only honest if the caveat is actually shown — visible but not decisive.
  it('renders lanes that examined nothing, separately from exclusions', () => {
    renderPanel({
      ...available,
      warnings: [
        'not examined: Citation verification — no references were parsed from the manuscript',
        "excluded from the recommendation: f4 — describes Gaply's own execution, not the manuscript",
      ],
    });
    const ne = screen.getByTestId('pr-not-examined').textContent ?? '';
    expect(ne).toMatch(/Citation verification/);
    expect(ne).toMatch(/no references were parsed/);
    // The two caveats are DIFFERENT statements and must not be merged.
    expect(ne).not.toMatch(/excluded from the recommendation/);
    expect(screen.getByTestId('pr-excluded').textContent).toMatch(/f4/);
    // and the recommendation is still shown — the caveat qualifies, not withholds
    expect(screen.getByTestId('pr-probability')).toBeTruthy();
  });

  it('renders the honest unavailable-offline state (no faked letter)', () => {
    renderPanel(unavailable);
    expect(screen.getByTestId('pr-verdict').textContent).toMatch(/REVIEWER UNAVAILABLE/);
    expect(screen.getByTestId('pr-unavailable').textContent).toMatch(/unavailable offline/);
    // it does NOT fabricate a recommendation gauge / issues
    expect(screen.queryByTestId('pr-probability')).toBeNull();
    expect(screen.queryByTestId('pr-issues')).toBeNull();
    // and it is NOT the withheld state — offline means the CLOUD reviewer could
    // not run while the local verdict stands.
    expect(screen.queryByTestId('pr-withheld')).toBeNull();
  });

  // BOUNDARY 4 of the withheld invariant (§26 PR-2): the UI must observe that
  // the verdict was withheld, and must not repeat the offline copy, which claims
  // "the verdict is available in the tabs above" — false when none was produced.
  it('renders WITHHELD distinctly from offline, with its reason', () => {
    const withheld: ReviewerLetter = {
      ...unavailable,
      body:
        'No recommendation was produced for this run: the analysis evidence could not be ' +
        'interpreted, so there was nothing to base one on. This is a fault in Gaply, not a ' +
        'finding about your manuscript. The findings and checklist above are unaffected.',
      warnings: ['verdict withheld: evidence_uninterpretable'],
    };
    renderPanel(withheld);
    expect(screen.getByTestId('pr-verdict').textContent).toMatch(/RECOMMENDATION WITHHELD/);
    const body = screen.getByTestId('pr-withheld').textContent ?? '';
    expect(body).toMatch(/No recommendation was produced/);
    expect(body).toMatch(/fault in Gaply, not a finding about your manuscript/);
    // No fabricated gauge, and NOT the offline wording.
    expect(screen.queryByTestId('pr-probability')).toBeNull();
    expect(screen.queryByTestId('pr-unavailable')).toBeNull();
    expect(document.body.textContent).not.toMatch(/findings, checklist, and verdict — is available/);
  });

  // Set 4e: the three grounded fields, POPULATED.
  it('renders the three grounded fields when the backend produced them', () => {
    const populated: ReviewerLetter = {
      ...available,
      novelty: { assessment: 'incremental over prior work' },
      journalFit: { journal: 'Nature', quartile: 'Q1', note: 'misses the word-limit requirement' },
      alternatives: [{ name: 'PLOS ONE', quartile: 'Q1', reason: 'broader scope fits the analysis' }],
    };
    renderPanel(populated);
    // novelty assessment + fit note show their grounded text
    expect(screen.getByTestId('pr-novelty').textContent).toMatch(/incremental over prior work/);
    expect(screen.getByText(/misses the word-limit requirement/)).toBeTruthy();
    // alternatives section renders journal/quartile/reason, advisory tone
    const alts = screen.getByTestId('pr-alternatives');
    expect(alts.textContent).toMatch(/PLOS ONE/);
    expect(screen.getByTestId('pr-alt-reason').textContent).toMatch(/broader scope/);
    expect(screen.getByText(/advisory, not a recommendation to submit/)).toBeTruthy();
  });
});

describe('adaptOutcome — backend fields -> frontend letter (Set 4e)', () => {
  const baseReport = {
    verdict: 'concern',
    combined_confidence: 1,
    findings: [],
    checklist: [],
    debate: { rounds_run: 1, converged: true, overridden_by_constraint: false, rejected_agents: [], revised_agents: [] },
    disclaimer: 'x',
  };

  it('maps the three grounded fields through', () => {
    const outcome = {
      report: baseReport,
      reviewer: {
        recommendation: 'major_revision',
        publication_probability: 61,
        // Legacy keys a stale backend/model might still send. They were removed
        // as ungroundable; the adapter must simply not read them.
        novelty_score: 60,
        novelty_assessment: 'incremental over prior work',
        journal_fit_score: 55,
        journal_fit_note: 'misses the word-limit requirement',
        body: 'body',
        issues: [],
        alternatives: [{ journal: 'PLOS ONE', quartile: 'Q1', reason: 'broader scope', evidence_ref: 'chk1' }],
        warnings: [],
        available: true,
      },
      proxy_payload: { summary: { findings: [], checklist: [] } },
    };
    const res = adaptOutcome(outcome as unknown as Parameters<typeof adaptOutcome>[0], {
      name: 'Nature',
      quartile: 'Q1',
    });
    expect(res.reviewerLetter.novelty.assessment).toBe('incremental over prior work');
    expect(res.reviewerLetter.journalFit.note).toBe('misses the word-limit requirement');
    expect(res.reviewerLetter.alternatives).toEqual([
      { name: 'PLOS ONE', quartile: 'Q1', reason: 'broader scope' },
    ]);
    // The removed scores must not reappear on the letter, even when the backend
    // payload still carries them.
    expect((res.reviewerLetter.novelty as Record<string, unknown>).score).toBeUndefined();
    expect((res.reviewerLetter.journalFit as Record<string, unknown>).fitScore).toBeUndefined();
  });

  it('leaves slots EMPTY when the backend omitted them (never faked)', () => {
    const outcome = {
      report: baseReport,
      reviewer: {
        recommendation: 'minor_revision',
        publication_probability: 70,
        body: 'body',
        issues: [],
        warnings: [],
        available: true,
        // novelty_assessment / journal_fit_note / alternatives ABSENT,
        // and no novelty_score / journal_fit_score at all (removed backend-side)
      },
      proxy_payload: { summary: { findings: [], checklist: [] } },
    };
    const res = adaptOutcome(outcome as unknown as Parameters<typeof adaptOutcome>[0], {
      name: 'Nature',
      quartile: 'Q1',
    });
    expect(res.reviewerLetter.novelty.assessment).toBe('');
    expect(res.reviewerLetter.journalFit.note).toBe('');
    expect(res.reviewerLetter.alternatives).toEqual([]);
  });
});
