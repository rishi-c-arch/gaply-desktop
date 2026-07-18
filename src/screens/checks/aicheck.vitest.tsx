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
    // "paraphrased" appears ONLY inside honest "unavailable" cautions — the
    // dedicated lane AND each passage's category-note (which SAYS the paraphrase
    // distinction is unavailable, the opposite of surfacing a paraphrased
    // verdict) — never as a determined passage label or a highlight color.
    const excluded = [
      screen.getByTestId('paraphrase-unavailable'),
      ...Array.from(container.querySelectorAll('[data-testid$="category-note"]')),
    ].map((el) => (el as HTMLElement).textContent ?? '');
    let outside = container.textContent ?? '';
    for (const t of excluded) outside = outside.replace(t, '');
    expect(outside.toLowerCase()).not.toContain('paraphras');
  });

  it('Per-passage findings lists every flagged passage, numbered, with tier + evidence + cautions', () => {
    render(<AiCheckReport result={AICHECK_FIXTURE} />);
    const section = screen.getByTestId('per-passage-findings');
    // the honest split sentence (deep-verified vs heuristic-only, templated)
    expect(screen.getByTestId('findings-split').textContent).toMatch(
      /1 of 2 flagged passages were deep-verified by the on-device model; the remaining 1 carry heuristic-only flags and are preliminary\./,
    );
    // one numbered finding per passage in the fixture (2)
    const findings = within(section).getAllByText(/^Passage \d+$/);
    expect(findings.map((n) => n.textContent)).toEqual(['Passage 1', 'Passage 2']);
    // passage 1 is deep-verified, passage 2 heuristic-only — verbatim tier cautions
    expect(screen.getByTestId('finding-0-depth-note').textContent).toMatch(/Deep-verified: the full on-device model/);
    expect(screen.getByTestId('finding-1-depth-note').textContent).toMatch(/Heuristic-only: flagged by the fast pre-pass/);
    // evidence (sentence perplexity table) rendered per finding, not click-gated
    expect(screen.getByTestId('finding-0-sentences')).toBeTruthy();
    expect(screen.getByTestId('finding-1-sentences')).toBeTruthy();
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

  it('the AI disclaimer is NEVER empty — a hardcoded fallback backstops a wire regression (L4)', () => {
    // wire regressed the required disclaimer to empty → the fallback renders, NOT an empty <p>
    const empty = { ...AICHECK_FIXTURE, analysis: { ...AICHECK_FIXTURE.analysis, disclaimer: '' } };
    const { rerender } = render(<AiCheckReport result={empty} />);
    const guarded = screen.getByTestId('ai-disclaimer').textContent!.trim();
    expect(guarded.length).toBeGreaterThan(0);
    expect(guarded).toMatch(/NOT proof of AI authorship/);
    // whitespace-only counts as empty too
    const blank = { ...AICHECK_FIXTURE, analysis: { ...AICHECK_FIXTURE.analysis, disclaimer: '   ' } };
    rerender(<AiCheckReport result={blank} />);
    expect(screen.getByTestId('ai-disclaimer').textContent!.trim().length).toBeGreaterThan(0);
    // NORMAL PATH unchanged: a present wire value renders verbatim, fallback dormant
    rerender(<AiCheckReport result={AICHECK_FIXTURE} />);
    expect(screen.getByTestId('ai-disclaimer').textContent).toBe(AICHECK_FIXTURE.analysis.disclaimer);
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

describe('AiCheckReport — highlight scrolls to its finding', () => {
  it('evidence lives in the findings (no click-gated inspector); a highlight click emphasizes the matching finding', () => {
    render(<AiCheckReport result={AICHECK_FIXTURE} />);
    // the click-one-at-a-time inspector is gone — evidence is ALWAYS present in
    // the numbered findings (verbatim cautions + per-sentence table).
    expect(screen.queryByTestId('passage-inspector')).toBeNull();
    const f0 = screen.getByTestId('finding-0');
    expect(within(f0).getByTestId('finding-0-depth-note').textContent).toBe(
      AICHECK_FIXTURE.analysis.passages[0].depth_note
    );
    expect(within(f0).getByTestId('finding-0-category-note').textContent).toBe(
      AICHECK_FIXTURE.analysis.passages[0].category_note
    );
    expect(within(f0).getByTestId('finding-0-sentences').textContent).toContain('4.2');
    expect(within(f0).getByText('deep-verified signal')).toBeTruthy();
    // a manuscript highlight click emphasizes (aria-current) its finding — the
    // scroll target is unambiguous (id finding-0).
    expect(f0.getAttribute('aria-current')).toBe('false');
    fireEvent.click(screen.getByTestId('aicheck-highlight-0'));
    expect(screen.getByTestId('finding-0').getAttribute('aria-current')).toBe('true');
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
