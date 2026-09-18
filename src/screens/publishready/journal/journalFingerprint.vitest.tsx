// Gaply — the journal display, tested against the two shapes the DATA forces
// and the promises items 9-12 make.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import React from 'react';
import { JournalFingerprintView } from './JournalFingerprintView';
import { ChecklistDisplay, isStructural } from './ChecklistDisplay';
import { IngestionSummary, JournalPicker } from './JournalPicker';
import { articleTypeText, JournalFingerprint, JournalProfileRow } from './fingerprintTypes';
import { ChecklistItem } from '../../report/reportTypes';

afterEach(cleanup);

const span = (s: string) => s;

function fp(over: Partial<JournalFingerprint> = {}): JournalFingerprint {
  return {
    journal_key: 'nature-medicine',
    provenance: {
      journal_key: 'nature-medicine',
      version: 3,
      content_hash: 'abc123',
      fetched_at: 1_757_800_000,
      refetch_after: 1_765_000_000,
      origin: 'crawled',
      source_count: 12,
      quarantined_at: null,
      quarantine_reason: null,
    },
    requirements: [],
    conflicts: [],
    conventions: [],
    expectations: [],
    standards: [],
    ...over,
  };
}

describe('a CONFLICTED fact shows both values and chooses neither', () => {
  const conflicted = fp({
    conflicts: [
      {
        kind: 'abstract_limit',
        article_type: { kind: 'stated', name: 'Article' },
        values: [
          {
            kind: 'abstract_limit',
            value: '250',
            article_type: { kind: 'stated', name: 'Article' },
            status: 'conflicted',
            source_url: 'https://j.test/author-guidelines',
            source_heading: 'Abstract',
            source_span: span('Abstract — up to 250 words, unreferenced.'),
          },
          {
            kind: 'abstract_limit',
            value: '300',
            article_type: { kind: 'stated', name: 'Article' },
            status: 'conflicted',
            source_url: 'https://j.test/article-types',
            source_heading: 'Article',
            source_span: span('Abstract — up to 300 words.'),
          },
        ],
      },
    ],
  });

  it('renders both values', () => {
    render(<JournalFingerprintView fp={conflicted} />);
    const sides = screen.getAllByTestId('conflict-side');
    expect(sides).toHaveLength(2);
    expect(sides.map((s) => within(s).getByText(/^\d+$/).textContent).sort()).toEqual([
      '250',
      '300',
    ]);
  });

  it('gives each side its own source sentence, so the reader can check which page said which', () => {
    render(<JournalFingerprintView fp={conflicted} />);
    expect(screen.getByText('Abstract — up to 250 words, unreferenced.')).toBeTruthy();
    expect(screen.getByText('Abstract — up to 300 words.')).toBeTruthy();
    expect(screen.getByText('https://j.test/author-guidelines')).toBeTruthy();
    expect(screen.getByText('https://j.test/article-types')).toBeTruthy();
  });

  it('never presents one side as chosen, preferred or current', () => {
    const { container } = render(<JournalFingerprintView fp={conflicted} />);
    const text = container.textContent ?? '';
    for (const word of ['preferred', 'chosen', 'correct', 'most recent', 'we use', 'overrides']) {
      expect(text.toLowerCase(), `the display picked a winner: "${word}"`).not.toContain(word);
    }
    // Both sides carry the SAME class, so no styling implies a winner either.
    const sides = screen.getAllByTestId('conflict-side');
    expect(sides[0].className).toEqual(sides[1].className);
  });
});

describe('a requirement bound to no article type says NOT STATED', () => {
  it('renders NOT STATED, never a claim about scope', () => {
    render(
      <JournalFingerprintView
        fp={fp({
          requirements: [
            {
              kind: 'word_limit',
              value: '4000',
              article_type: { kind: 'not_stated' },
              status: 'verified',
              source_url: 'https://j.test/content',
              source_heading: 'Analysis',
              source_span: span('Format Main text — up to 4,000 words.'),
            },
          ],
        })}
      />,
    );
    expect(screen.getByTestId('article-type').textContent).toBe('NOT STATED');
  });

  it('has no code path that turns an absent article type into "all"', () => {
    expect(articleTypeText({ kind: 'not_stated' })).toBe('NOT STATED');
    expect(articleTypeText({ kind: 'stated', name: 'Correspondence' })).toBe('Correspondence');
    // The union has no third member and no string to fall through, so the
    // usual `articleType || 'All'` bug is unwritable against this type.
    const rendered = [{ kind: 'not_stated' } as const, { kind: 'stated', name: 'X' } as const].map(
      articleTypeText,
    );
    expect(rendered.some((t) => /^(all|any|every)/i.test(t))).toBe(false);
  });
});

describe('item 10 — three kinds, three visibly distinct sections', () => {
  const full = fp({
    requirements: [
      {
        kind: 'word_limit',
        value: '4000',
        article_type: { kind: 'stated', name: 'Article' },
        status: 'verified',
        source_url: 'https://j.test/content',
        source_heading: 'Article',
        source_span: span('Main text — up to 4,000 words.'),
      },
    ],
    conventions: [
      {
        metric: 'length',
        median: 9,
        iqr_low: 8,
        iqr_high: 12,
        n: 41,
        detail: '9 PAGES (the source supplies pages, not words)',
        status: 'inferred',
      },
      {
        metric: 'figure_count',
        median: null,
        iqr_low: null,
        iqr_high: null,
        n: 0,
        detail: 'Needs open-access full text parsed; not derivable from the source.',
        status: 'unavailable',
      },
    ],
    expectations: [
      {
        claim: 'Reviewers are asked whether the statistical analysis suits the design.',
        frequency_k: null,
        frequency_n: null,
        status: 'verified',
        source_url: 'https://j.test/for-reviewers',
        source_span: span('Reviewers should consider whether the statistical analysis is appropriate.'),
      },
    ],
  });

  it('renders the three sections separately', () => {
    render(<JournalFingerprintView fp={full} />);
    expect(screen.getByTestId('section-requirements')).toBeTruthy();
    expect(screen.getByTestId('section-conventions')).toBeTruthy();
    expect(screen.getByTestId('section-expectations')).toBeTruthy();
  });

  it('gives the three sections different visual treatment, not just different labels', () => {
    render(<JournalFingerprintView fp={full} />);
    const classes = ['requirements', 'conventions', 'expectations'].map(
      (s) => screen.getByTestId(`section-${s}`).className,
    );
    expect(new Set(classes).size, 'the three sections share a class').toBe(3);
  });

  it('every line carries its status', () => {
    render(<JournalFingerprintView fp={full} />);
    expect(screen.getAllByTestId('status-verified').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByTestId('status-inferred')).toBeTruthy();
    expect(screen.getByTestId('status-unavailable')).toBeTruthy();
  });

  it('renders a convention with its n AND its unit, never a bare median', () => {
    render(<JournalFingerprintView fp={full} />);
    expect(screen.getByText(/n = 41/)).toBeTruthy();
    expect(screen.getByText(/9 PAGES/)).toBeTruthy();
    expect(screen.getAllByText(/not a rule/i).length).toBeGreaterThan(0);
  });

  it('links a line to its source document and heading, not the journal homepage', () => {
    render(<JournalFingerprintView fp={full} />);
    const link = screen.getByText('https://j.test/content') as HTMLAnchorElement;
    expect(link.getAttribute('href')).toBe('https://j.test/content');
    expect(screen.getByText(/· Article/)).toBeTruthy();
  });

  it('renders the source sentence whole', () => {
    const long =
      'Preparing your submission for the fast track To facilitate timely editorial and peer ' +
      'review processes, all fast track submissions must include the following: Complete ' +
      'manuscript files, including disclosure of competing interests, funding statement, a data ' +
      'availability statement and in the case of studies involved custom code, a code ' +
      'availability statement.';
    render(
      <JournalFingerprintView
        fp={fp({
          requirements: [
            {
              kind: 'data_policy',
              value: 'data availability statement required',
              article_type: { kind: 'not_stated' },
              status: 'verified',
              source_url: 'https://j.test/fasttrack',
              source_heading: 'Fast track',
              source_span: long,
            },
          ],
        })}
      />,
    );
    // A clipped span cannot be checked against the page, so it is not a span.
    expect(screen.getByText(long)).toBeTruthy();
  });
});

describe('item 11 — provenance on the run', () => {
  it('states which fingerprint version and when it was fetched', () => {
    render(<JournalFingerprintView fp={fp()} />);
    const p = screen.getByTestId('provenance').textContent ?? '';
    expect(p).toContain('v3');
    expect(p).toContain('2025-09-13');
    expect(p).toContain('12 sources');
  });

  it('says so when the fingerprint is quarantined', () => {
    const q = fp();
    q.provenance!.quarantined_at = 1_757_900_000;
    q.provenance!.quarantine_reason = 'injection markers in fetched text';
    render(<JournalFingerprintView fp={q} />);
    expect(screen.getByTestId('quarantine').textContent).toContain('injection markers');
  });

  it('renders an uncrawled journal as a state, not an empty fingerprint that looks fetched', () => {
    render(<JournalFingerprintView fp={fp({ provenance: null })} />);
    expect(screen.getByTestId('fingerprint-empty').textContent).toMatch(/structural only/i);
  });
});

function row(over: Partial<JournalProfileRow> = {}): JournalProfileRow {
  return {
    key: 'nature-medicine',
    name: 'Nature Medicine',
    entry: 'https://www.nature.com/nm/for-authors',
    ingested: true,
    origin: 'crawled',
    requirement_count: 29,
    conflict_count: 1,
    convention_count: 5,
    expectation_count: 0,
    standard_count: 15,
    by_pattern: 29,
    by_model: 0,
    version: 3,
    fetched_at: 1_757_800_000,
    quarantine_reason: null,
    ...over,
  };
}

describe('item 9 — the picker shows what ingestion found BEFORE the run', () => {
  it('states the count and the pattern/model split', () => {
    render(<IngestionSummary row={row()} />);
    expect(screen.getByTestId('ingestion-summary').textContent).toContain('29 requirement');
    expect(screen.getByTestId('split').textContent).toBe('29 by pattern, 0 by model');
  });

  it('shows the model count even at zero, so the day it is not zero is visible', () => {
    render(<IngestionSummary row={row({ by_model: 0 })} />);
    expect(screen.getByTestId('split').textContent).toContain('0 by model');
  });

  it('says so, and offers a labelled structural-only run, when ingestion found nothing', () => {
    let proceeded = false;
    render(
      <JournalPicker
        profiles={[row({ ingested: false, requirement_count: 0 })]}
        selectedKey="nature-medicine"
        onSelect={() => {}}
        pastedUrl=""
        onPastedUrlChange={() => {}}
        pastedResult={null}
        onProceedStructuralOnly={() => {
          proceeded = true;
        }}
      />,
    );
    expect(screen.getByTestId('ingestion-empty').textContent).toMatch(/structural only/i);
    const btn = screen.getByText('Proceed structural only');
    btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(proceeded).toBe(true);
  });

  it('marks a not-ingested journal in the list without hiding it', () => {
    render(
      <JournalPicker
        profiles={[row(), row({ key: 'lancet', name: 'The Lancet', ingested: false })]}
        selectedKey={null}
        onSelect={() => {}}
        pastedUrl=""
        onPastedUrlChange={() => {}}
        pastedResult={null}
      />,
    );
    expect(screen.getByTestId('badge-nature-medicine').textContent).toBe('29 req');
    expect(screen.getByTestId('badge-lancet').textContent).toBe('not ingested');
  });
});

describe('item 12 — the checklist display', () => {
  const journalItem: ChecklistItem = {
    requirement: 'data availability statement',
    passed: false,
    detail: 'no data availability heading was found in the manuscript',
    guideline_source: 'https://j.test/fasttrack',
    source_span: 'all fast track submissions must include ... a data availability statement',
    article_type: null,
    checked_field: 'extraction.sections[heading]',
  };
  const structuralItem: ChecklistItem = {
    requirement: 'manuscript has a Methods section',
    passed: true,
    detail: 'a Methods heading was found',
    guideline_source: null,
    source_span: null,
    article_type: null,
    checked_field: 'extraction.sections[heading]',
  };

  it('shows the requirement, the source sentence and the field checked', () => {
    render(<ChecklistDisplay items={[journalItem]} />);
    expect(screen.getByText('data availability statement')).toBeTruthy();
    expect(screen.getByText(journalItem.source_span!)).toBeTruthy();
    expect(screen.getByTestId('checked-field').textContent).toContain(
      'extraction.sections[heading]',
    );
  });

  it('labels a structural item and does not dress it as a journal requirement', () => {
    render(<ChecklistDisplay items={[structuralItem]} />);
    expect(screen.getByTestId('structural-label')).toBeTruthy();
    expect(screen.getByTestId('no-journal-source')).toBeTruthy();
    expect(isStructural(structuralItem)).toBe(true);
    expect(isStructural(journalItem)).toBe(false);
  });

  it('says NOT STATED for an item whose requirement names no article type', () => {
    render(<ChecklistDisplay items={[journalItem]} />);
    expect(screen.getByTestId('checklist-article-type').textContent).toBe('NOT STATED');
  });

  /**
   * The teardown's "6 findings, 3 of which were agents saying they had nothing
   * to say" is what this refuses. A met item is met — not an issue, not a
   * finding, and not counted with the failures.
   */
  it('counts met items apart from unmet ones and never calls a pass a finding', () => {
    const { container } = render(
      <ChecklistDisplay items={[journalItem, structuralItem, { ...structuralItem, requirement: 'x' }]} />,
    );
    const counts = screen.getByTestId('checklist-counts').textContent ?? '';
    expect(counts).toContain('1 unmet');
    expect(counts).toContain('2 met');
    expect((container.textContent ?? '').toLowerCase()).not.toContain('finding');
    expect(screen.getAllByTestId('checklist-fail')).toHaveLength(1);
  });
});
