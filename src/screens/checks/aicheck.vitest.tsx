// Set 5 — the AI Check two-way report. Presentational tests over the wire
// fixture: 2-color highlighting, the honest %, the evidence inspector, the
// paraphrase lane's honest unavailability, and the un-strippable cautions.
import React from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import AiCheckReport, { segmentSections } from './AiCheckReport';
import { AICHECK_FIXTURE, AICHECK_FIXTURE_SPANISH } from './aicheckFixture';

afterEach(cleanup);

describe('AiCheckReport — two-way in-document highlighting', () => {
  it('highlights AI-associated passages in two tiers and leaves human text plain', () => {
    render(<AiCheckReport result={AICHECK_FIXTURE} />);
    const deep = screen.getByTestId('aicheck-highlight-0');
    const heur = screen.getByTestId('aicheck-highlight-1');
    expect(deep.getAttribute('data-tier')).toBe('flagged');
    expect(heur.getAttribute('data-tier')).toBe('assessed');
    // human text renders OUTSIDE any highlight
    const manuscript = screen.getByTestId('aicheck-manuscript');
    expect(manuscript.textContent).toContain('Her grandmother’s schematics');
    expect(deep.textContent).not.toContain('grandmother');
    // the legend explains the two-way scheme without verdict language
    expect(screen.getByTestId('aicheck-legend').textContent).toMatch(/never\s+verdicts/i);
  });

  it('anchors passages by text, not offsets (Rust byte offsets ≠ JS indices)', () => {
    const { rendered, unplaced } = segmentSections(AICHECK_FIXTURE);
    expect(unplaced).toHaveLength(0);
    const segs = rendered[0].segments;
    // plain / passage / plain / passage / plain
    expect(segs.map((s) => s.kind)).toEqual(['plain', 'passage', 'plain', 'passage', 'plain']);
    // reassembling the segments reproduces the section text exactly
    const reassembled = segs
      .map((s) => (s.kind === 'plain' ? s.text : s.passage.text))
      .join('');
    expect(reassembled).toBe(AICHECK_FIXTURE.sections[0].text);
  });

  it('exactly two highlight colors, tier-driven — no third "paraphrased" color exists', () => {
    const { container } = render(<AiCheckReport result={AICHECK_FIXTURE} />);
    // every colored element carries a TIER (deep-verified/heuristic-only),
    // never a category — the palette is structurally two-way
    const tiers = Array.from(container.querySelectorAll('[data-tier]')).map((el) =>
      el.getAttribute('data-tier')
    );
    expect(tiers.length).toBeGreaterThan(0);
    expect(new Set(tiers)).toEqual(new Set(['flagged', 'assessed']));
    // "paraphrased" appears ONLY inside the honest unavailable lane, never
    // as a passage label or color
    const lane = screen.getByTestId('paraphrase-unavailable');
    const outsideLane = container.textContent!.replace(lane.textContent!, '');
    expect(outsideLane.toLowerCase()).not.toContain('paraphras');
  });

  it('an all-clear document still renders every caution (nothing is flag-gated)', () => {
    const clear = {
      ...AICHECK_FIXTURE,
      analysis: {
        ...AICHECK_FIXTURE.analysis,
        passages: [],
        flagged_chars: 0,
        ai_signal_proportion: 0,
        candidates_found: 0,
        deep_verified: 0,
      },
    };
    render(<AiCheckReport result={clear} />);
    expect(screen.getByTestId('ai-proportion').textContent).toBe('0.0%');
    expect(screen.getByTestId('ai-disclaimer').textContent).toBe(clear.analysis.disclaimer);
    expect(screen.getByTestId('coverage-note')).toBeTruthy();
    expect(screen.getByTestId('language-note')).toBeTruthy();
    expect(screen.getByTestId('paraphrase-unavailable')).toBeTruthy();
  });

  it('a passage that cannot be anchored still surfaces (never silently dropped)', () => {
    const broken = {
      ...AICHECK_FIXTURE,
      analysis: {
        ...AICHECK_FIXTURE.analysis,
        passages: [
          { ...AICHECK_FIXTURE.analysis.passages[0], text: 'THIS TEXT IS IN NO SECTION.' },
        ],
      },
    };
    render(<AiCheckReport result={broken} />);
    expect(screen.getByTestId('aicheck-unplaced-0').textContent).toContain('IN NO SECTION');
  });
});

describe('AiCheckReport — the honest %', () => {
  it('shows the proportion with proportion framing and NO probability anywhere', () => {
    const { container } = render(<AiCheckReport result={AICHECK_FIXTURE} />);
    expect(screen.getByTestId('ai-proportion').textContent).toBe('42.0%');
    // the copy right next to the number denies the probability reading
    expect(container.textContent).toMatch(/NOT the chance that this document was AI-written/);
    // and the word "probability" never appears in the whole report
    expect(container.textContent!.toLowerCase()).not.toContain('probability');
    // the coverage note is always present, verbatim
    expect(screen.getByTestId('coverage-note').textContent).toContain('deep-verified 1');
  });
});

describe('AiCheckReport — evidence inspector', () => {
  it('clicking a highlight opens verbatim per-passage cautions and sentence evidence', () => {
    render(<AiCheckReport result={AICHECK_FIXTURE} />);
    expect(screen.queryByTestId('passage-inspector')).toBeNull();
    fireEvent.click(screen.getByTestId('aicheck-highlight-0'));
    const inspector = screen.getByTestId('passage-inspector');
    expect(within(inspector).getByTestId('inspector-depth-note').textContent).toBe(
      AICHECK_FIXTURE.analysis.passages[0].depth_note
    );
    expect(within(inspector).getByTestId('inspector-uncertainty').textContent).toBe(
      AICHECK_FIXTURE.analysis.passages[0].uncertainty
    );
    expect(within(inspector).getByTestId('inspector-category-note').textContent).toBe(
      AICHECK_FIXTURE.analysis.passages[0].category_note
    );
    // per-sentence evidence rides along
    expect(within(inspector).getByTestId('inspector-sentences').textContent).toContain('4.2');
    // tier label is a signal level, not a verdict
    expect(within(inspector).getByText('deep-verified signal')).toBeTruthy();
  });
});

describe('AiCheckReport — honest paraphrase lane + cautions', () => {
  it('the paraphrase lane reports UNAVAILABLE and never a third category', () => {
    render(<AiCheckReport result={AICHECK_FIXTURE} />);
    const lane = screen.getByTestId('paraphrase-unavailable');
    expect(within(lane).getByTestId('classification-note').textContent).toContain('UNAVAILABLE');
    expect(within(lane).getByTestId('classification-note').textContent).toContain(
      'nothing was guessed'
    );
    expect(within(lane).getByTestId('paraphrase-caution').textContent).toContain(
      'EVEN LESS reliable'
    );
  });

  it('the un-strippable caution leads the report, verbatim from the core', () => {
    render(<AiCheckReport result={AICHECK_FIXTURE} />);
    expect(screen.getByTestId('ai-disclaimer').textContent).toBe(
      AICHECK_FIXTURE.analysis.disclaimer
    );
    expect(screen.getByTestId('ai-caution').textContent).toMatch(/signal, not proof/i);
  });
});

describe('AiCheckReport — language honesty (Set 5)', () => {
  it('English shows the calibration note; non-English shows the downgrade banner', () => {
    const first = render(<AiCheckReport result={AICHECK_FIXTURE} />);
    expect(screen.getByTestId('language-note').textContent).toContain('English');
    expect(screen.queryByTestId('language-downgrade')).toBeNull();
    first.unmount();

    render(<AiCheckReport result={AICHECK_FIXTURE_SPANISH} />);
    const banner = screen.getByTestId('language-downgrade');
    expect(banner.textContent).toContain('LOW-CONFIDENCE');
    expect(screen.queryByTestId('language-note')).toBeNull();
  });
});
