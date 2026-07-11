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
  novelty: { score: 60, assessment: '' }, // backend doesn't produce assessment yet
  journalFit: { journal: 'Nature', quartile: 'Q1', fitScore: 55, note: '' }, // no note yet
  alternatives: [], // none yet
  body: 'The manuscript is promising but needs revision before it can be accepted.',
  available: true,
  issues: [{ findingRef: 'f1', severity: 'major', rationale: 'citation c1 refuted; address it' }],
  warnings: ['potential_hallucination: issue cites finding "f9" not provided; dropped'],
};

const unavailable: ReviewerLetter = {
  recommendation: 'unknown',
  publicationProbability: 0,
  novelty: { score: 0, assessment: '' },
  journalFit: { journal: 'Nature', quartile: 'Q1', fitScore: 0, note: '' },
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

  it('renders the honest unavailable-offline state (no faked letter)', () => {
    renderPanel(unavailable);
    expect(screen.getByTestId('pr-verdict').textContent).toMatch(/REVIEWER UNAVAILABLE/);
    expect(screen.getByTestId('pr-unavailable').textContent).toMatch(/unavailable offline/);
    // it does NOT fabricate a recommendation gauge / issues
    expect(screen.queryByTestId('pr-probability')).toBeNull();
    expect(screen.queryByTestId('pr-issues')).toBeNull();
  });

  // Set 4e: the three grounded fields, POPULATED.
  it('renders the three grounded fields when the backend produced them', () => {
    const populated: ReviewerLetter = {
      ...available,
      novelty: { score: 60, assessment: 'incremental over prior work' },
      journalFit: { journal: 'Nature', quartile: 'Q1', fitScore: 55, note: 'misses the word-limit requirement' },
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
  });

  it('leaves slots EMPTY when the backend omitted them (never faked)', () => {
    const outcome = {
      report: baseReport,
      reviewer: {
        recommendation: 'minor_revision',
        publication_probability: 70,
        novelty_score: 50,
        journal_fit_score: 60,
        body: 'body',
        issues: [],
        warnings: [],
        available: true,
        // novelty_assessment / journal_fit_note / alternatives ABSENT
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
