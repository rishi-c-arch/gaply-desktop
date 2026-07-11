// Set 4d — ReviewerLetterPanel adaptation to the current backend output.
// Verifies the two states the browser premium-gate blocks in dev: an adapted
// "available" letter (empty assessment/note, [] alternatives, gated issues) and
// the honest "unavailable offline" state. Real React render (no faked fields).
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import ReviewerLetterPanel from './ReviewerLetterPanel';
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
});
