// Set 4 — the Journal Verification bridge. Round-trips the combined result via
// the mock: registry facts + labeled self-reported summary, the not-found
// variant, and the multiple-match disambiguation variant. Wire-exact types.
import { describe, expect, it } from 'vitest';
import { makeMockJournalVerifyBridge } from './journalVerifyBridge';
import { JournalVerificationResult } from './journalVerifyTypes';
import { OFFLINE_FEATURES } from '../subscription/tiers';
import { PREMIUM_ONLY, gateFeature } from '../subscription/tiers';

const COMBINED: JournalVerificationResult = {
  input: 'https://journal.example.org/about',
  input_kind: 'link',
  disambiguation: [],
  not_found: false,
  registry: {
    query: 'Journal of Sleep Research (1365-2869)',
    name: 'Journal of Sleep Research',
    issn: '1365-2869',
    doaj_registered: true,
    openalex_in_doaj: true,
    pubmed_indexed: true,
    works_by_year: [{ year: 2025, works: 180 }],
    recent_activity: true,
    scope: ['Sleep medicine'],
    warning: null,
    reasons: [],
    verified_sources: ['openalex', 'doaj', 'pubmed'],
    unverified: [],
    sources_not_checked: ['Scopus (no free API — not checked; absence here is not a signal)', 'Web of Science (no free API — not checked; absence here is not a signal)'],
  },
  site_summary: {
    status: 'summarized',
    source_url: 'https://journal.example.org/about',
    details: [
      { field: 'apc', value: '$1200', source: 'https://journal.example.org/about', dropped_unverifiable: false },
      { field: 'review_timeline', value: 'within 3 weeks', source: 'https://journal.example.org/about', dropped_unverifiable: false },
      { field: 'guidelines_link', value: null, source: 'https://journal.example.org/about', dropped_unverifiable: false },
      { field: 'contact', value: 'editor@journal.example.org', source: 'https://journal.example.org/about', dropped_unverifiable: false },
    ],
    label: "These are the journal's OWN claims from its website — self-reported, not independently verified.",
    notice: null,
    page_truncated: false,
  },
  notes: [],
};

const MULTI: JournalVerificationResult = {
  input: 'advances in science',
  input_kind: 'name',
  disambiguation: [
    { name: 'Advances in Science', issn: '1111-1111' },
    { name: 'Advances in Science and Technology', issn: '2222-2222' },
  ],
  not_found: false,
  registry: null,
  site_summary: null,
  notes: [],
};

describe('Journal Verification — entitlement (paid)', () => {
  it('journal_verification is PREMIUM_ONLY, not an offline feature', () => {
    expect(PREMIUM_ONLY.has('journal_verification')).toBe(true);
    expect(OFFLINE_FEATURES.has('journal_verification')).toBe(false);
    const free = gateFeature('free', 'journal_verification');
    expect(free.allowed).toBe(false);
    expect(free.reason).toBe('premium_only');
    expect(free.upsell).toBe(true);
    expect(gateFeature('premium', 'journal_verification').allowed).toBe(true);
  });
});

describe('JournalVerifyBridge (mock) — combined result round-trip', () => {
  it('verify(link) → registry facts + labeled self-reported summary', async () => {
    const b = makeMockJournalVerifyBridge({ byQuery: { 'https://journal.example.org/about': COMBINED } });
    const r = await b.verify('https://journal.example.org/about');
    // grounded registry facts
    expect(r.registry?.doaj_registered).toBe(true);
    expect(r.registry?.pubmed_indexed).toBe(true);
    expect(r.registry?.sources_not_checked.some((s) => s.includes('Scopus'))).toBe(true);
    // labeled self-reported summary — distinct from the facts
    expect(r.site_summary?.status).toBe('summarized');
    expect(r.site_summary?.label).toMatch(/self-reported/);
    expect(r.site_summary?.details.find((d) => d.field === 'apc')?.value).toBe('$1200');
    // no verdict field exists on the result at all
    expect((r as any).verdict).toBeUndefined();
  });

  it('multiple name matches → disambiguation (not silently picked)', async () => {
    const b = makeMockJournalVerifyBridge({ byQuery: { 'advances in science': MULTI } });
    const r = await b.verify('advances in science');
    expect(r.disambiguation.length).toBe(2);
    expect(r.registry).toBeNull();
    // picking a match → verifyByIssn path
    const picked = makeMockJournalVerifyBridge({ byIssn: { '1111-1111': COMBINED } });
    const r2 = await picked.verifyByIssn('1111-1111');
    expect(r2.registry?.issn).toBe('1365-2869');
    expect(picked.calls).toEqual([['issn', '1111-1111']]);
  });

  it('an unseeded query → honest not-found (never a fabricated result)', async () => {
    const b = makeMockJournalVerifyBridge({});
    const r = await b.verify('a predatory-looking journal xyz');
    expect(r.not_found).toBe(true);
    expect(r.registry).toBeNull();
    expect(r.notes.some((n) => /couldn't resolve/i.test(n))).toBe(true);
  });
});
