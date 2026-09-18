// F10 — PublishReady tests. Mocked bridge/subscription, no network.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import PublishReadyPage from './PublishReadyPage';
import { makePublishReadyMock } from './publishReadyBridge';
import { buildProxyPayload } from './buildPayload';
import { synthesizeReviewerLetter, suggestAlternatives } from './synthesize';
import { PublishReadyReport, Finding } from '../report/reportTypes';
import { JOURNALS } from '../journal/journalData';
import * as fpBridge from './journal/journalFingerprintBridge';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const RAW_SENTINEL = 'RAW_MANUSCRIPT_EXCERPT_DO_NOT_LEAK';

function auth(session: any = { user: { id: 'u1', email: 'a@b.c' } }): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

// A report whose plagiarism finding DETAIL quotes the manuscript (the excerpt
// that must never reach the proxy), plus a refuted citation + a maths flag.
const REPORT: PublishReadyReport = {
  verdict: 'concern',
  combined_confidence: 1,
  findings: [
    { severity: 'critical', tier: 'mathematically_certain', certainty_label: 'mathematically certain', agent: 'validation_maths', title: 'rule failed: missing effect size', detail: 'p-value without effect size.', confidence: 1, provenance: ['rule:MissingEffectSize (MAJOR)', 'agent:validation_maths (deterministic)'], section: 'Results' },
    { severity: 'major', tier: 'reconsidered_after_peer_review', certainty_label: 'reconsidered after peer review', agent: 'verification', title: 'citation c1 REFUTED by evidence', detail: 'DOI resolves to a different work.', confidence: 0.9, provenance: ['evidence:ev-c1-0', 'agent:verification (harness-gated, via proxy)'] },
    { severity: 'minor', tier: 'ai_assessed_moderate', certainty_label: 'AI-assessed, moderate confidence', agent: 'plagiarism', title: 'high word overlap — 88% word overlap', detail: `"${RAW_SENTINEL}" matches a 2019 paper.`, confidence: 0.88, provenance: ['similarity:0.880', 'match_type:high word overlap', 'source:corpus'] },
  ],
  checklist: [
    { requirement: 'word limit (3000 words)', passed: false, detail: '4100 words', guideline_source: 'https://x' },
    { requirement: 'required section: Methods', passed: true, detail: 'found', guideline_source: null },
  ],
  debate: { rounds_run: 2, converged: true, overridden_by_constraint: true, rejected_agents: [], revised_agents: ['verification'] },
  disclaimer: 'Certainty tiers … never definitive proof.',
};

function renderPR(props: any = {}, session: any = { user: { id: 'u1', email: 'a@b.c' } }) {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <PublishReadyPage {...props} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

const journalStep = () => {
  fireEvent.change(screen.getByTestId('pr-journal-input'), { target: { value: 'lancet' } });
};

/* ------------------------------ premium gate ---------------------------- */

describe('premium gate', () => {
  it('free user sees the blurred teaser + upgrade CTA', async () => {
    renderPR({ forceTier: 'free', bridge: makePublishReadyMock(REPORT) });
    expect(await screen.findByTestId('pr-teaser')).toBeTruthy();
    expect(screen.getByTestId('pr-unlock').textContent).toMatch(/Unlock the verdict/i);
    // no entry form for free users
    expect(screen.queryByTestId('pr-run')).toBeNull();
  });

  it('premium user reaches the full entry → report flow', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-file'), { target: { files: [new File(['x'], `${RAW_SENTINEL}.pdf`, { type: 'application/pdf' })] } });
    // acceptFile is async (FileReader page-count estimate) and pr-run stays
    // disabled until it lands — wait for the accepted-file name to render,
    // otherwise the run click is a silent no-op under parallel-suite load.
    await screen.findByText(`${RAW_SENTINEL}.pdf`);
    journalStep();
    fireEvent.click(await screen.findByTestId('pr-journal-The Lancet'));
    fireEvent.click(screen.getByTestId('pr-run'));

    // reviewer letter tab renders the flagship output
    await screen.findByTestId('publishready');
    await waitFor(() => expect(screen.getByTestId('tab-Reviewer Letter')).toBeTruthy());
    fireEvent.click(screen.getByTestId('tab-Reviewer Letter'));
    expect(await screen.findByTestId('reviewer-letter-panel')).toBeTruthy();
    // major revision (1 critical + 1 refuted)
    // The LABEL is sentence case; the CAPS are presentation. textContent returns
    // the source text, so asserting /MAJOR REVISION/ here would be asserting a
    // CSS effect through the wrong channel — and would fail the moment the label
    // became usable mid-sentence, which the PDF requires.
    const verdict = screen.getByTestId('pr-verdict');
    expect(verdict.textContent).toMatch(/Major revision/);
    expect(verdict.style.textTransform).toBe('uppercase');
    // The report renders WITHOUT a publication-probability gauge (§4.20).
    expect(screen.queryByTestId('pr-probability')).toBeNull();
  });
});

/* --------------------- privacy: no manuscript text ---------------------- */

describe('privacy: only structured findings leave the device', () => {
  it('buildProxyPayload contains ZERO raw manuscript text', () => {
    const payload = buildProxyPayload(REPORT, { name: 'The Lancet', quartile: 'Q1' });
    const serialized = JSON.stringify(payload);
    // the plagiarism EXCERPT (in finding.detail) must NOT be present
    expect(serialized).not.toContain(RAW_SENTINEL);
    // detail free-text must not be included at all
    expect(serialized).not.toContain('p-value without effect size');
    // but structured findings ARE present
    expect(serialized).toContain('rule failed: missing effect size');
    expect(serialized).toContain('citation c1 REFUTED');
    expect(payload.findings[0].evidence).toContain('rule:MissingEffectSize (MAJOR)');
  });

  it('the running bridge sends only the structured payload (no excerpt)', async () => {
    const bridge = makePublishReadyMock(REPORT);
    const res = await bridge.run({ manuscriptPath: `/private/${RAW_SENTINEL}.pdf`, journal: { name: 'The Lancet', quartile: 'Q1' } });
    expect(JSON.stringify(res.proxyPayload)).not.toContain(RAW_SENTINEL);
    expect(bridge.lastPayload).toBeTruthy();
  });
});

/* --------------------- reviewer letter synthesis ------------------------ */

describe('reviewer letter synthesis (from a mocked debate result)', () => {
  it('renders a recommendation badge + probability from the report', () => {
    const letter = synthesizeReviewerLetter(REPORT, { name: 'The Lancet', quartile: 'Q1' });
    expect(letter.recommendation).toBe('major_revision'); // 1 critical + 1 refuted
    expect(letter.publicationProbability).toBeGreaterThanOrEqual(0);
    expect(letter.publicationProbability).toBeLessThanOrEqual(100);
    // The mock must match production: no novelty/fit score exists, and the mock
    // synthesizes no prose for either — an ungrounded slot stays empty.
    expect(letter.novelty.assessment).toBe('');
    expect(letter.journalFit.note).toBe('');
    expect((letter.novelty as Record<string, unknown>).score).toBeUndefined();
    expect((letter.journalFit as Record<string, unknown>).fitScore).toBeUndefined();
  });

  it('a clean report yields ACCEPT with high probability', () => {
    const clean: PublishReadyReport = { ...REPORT, verdict: 'pass', findings: [{ severity: 'info', tier: 'ai_assessed_moderate', certainty_label: 'AI-assessed, moderate confidence', agent: 'extraction', title: 'Extraction: pass', detail: 'ok', confidence: 0.9, provenance: ['swarm:round-table'] } as Finding], checklist: [{ requirement: 'x', passed: true, detail: 'ok', guideline_source: null }] };
    const letter = synthesizeReviewerLetter(clean, { name: 'Some Q3 Journal', quartile: 'Q3' });
    expect(letter.recommendation).toBe('accept');
    expect(letter.publicationProbability).toBeGreaterThan(70);
  });
});

/* ------------------------- journal-fit alternatives --------------------- */

describe('journal fit', () => {
  it('suggests same-or-higher-quartile alternatives only', () => {
    const alts = suggestAlternatives({ name: 'Nonexistent Q2 Journal', quartile: 'Q2' });
    expect(alts.length).toBeGreaterThan(0);
    const rank: Record<string, number> = { Q1: 1, Q2: 2, Q3: 3, Q4: 4 };
    // every alternative must be Q1 or Q2 (rank <= 2) — never lower than target
    for (const a of alts) expect(rank[a.quartile]).toBeLessThanOrEqual(2);
  });

  it('excludes the target journal itself', () => {
    const alts = suggestAlternatives({ name: 'The Lancet', quartile: 'Q1' });
    expect(alts.some((a) => a.name === 'The Lancet')).toBe(false);
    for (const a of alts) expect(a.quartile).toBe('Q1'); // Q1 target → only Q1 alts
  });
});

/* ----------------------- H4 · journal guidelines ------------------------ */

// A report whose checklist has ONLY structural items (no guideline_source) — the
// honest "no guidelines provided" state.
const REPORT_NO_GUIDELINES: PublishReadyReport = {
  ...REPORT,
  checklist: [
    { requirement: 'required section: Abstract', passed: true, detail: 'found', guideline_source: null },
    { requirement: 'required section: Methods', passed: true, detail: 'found', guideline_source: null },
  ],
};

// Drive the entry form to a ready-to-run state (file + journal picked).
async function reachRunnable() {
  await screen.findByTestId('pr-entry');
  fireEvent.change(screen.getByTestId('pr-file'), { target: { files: [new File(['x'], 'p.pdf', { type: 'application/pdf' })] } });
  await screen.findByText('p.pdf');
  fireEvent.change(screen.getByTestId('pr-journal-input'), { target: { value: 'lancet' } });
  fireEvent.click(await screen.findByTestId('pr-journal-The Lancet'));
}

describe('H4 · target-journal guidelines', () => {
  it('shows the guidelines input and does NOT prefill it from the picked journal', async () => {
    // The directory has no author-guidelines URL for ANY journal — it carries
    // websites. Prefilling put a homepage in the field, which ingests
    // successfully and produces a checklist indistinguishable from the blank
    // case: silent failure. Empty fails visibly. ARCHITECTURE_TRACE §18.6.2.
    const j = JOURNALS.find((x) => x.website)!;
    expect(j).toBeTruthy();
    expect(JOURNALS.every((x) => !('guidelinesUrl' in x))).toBe(true);
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    expect((screen.getByTestId('pr-guidelines-input') as HTMLInputElement).value).toBe('');
    fireEvent.change(screen.getByTestId('pr-journal-input'), { target: { value: j.name } });
    fireEvent.click(await screen.findByTestId(`pr-journal-${j.name}`));
    // Journal picked, and the field is STILL empty — the user supplies the URL.
    expect(await screen.findByTestId('pr-journal-picked')).toBeTruthy();
    expect((screen.getByTestId('pr-guidelines-input') as HTMLInputElement).value).toBe('');
    // ...and it remains user-editable.
    fireEvent.change(screen.getByTestId('pr-guidelines-input'), { target: { value: 'https://x.example/authors' } });
    expect((screen.getByTestId('pr-guidelines-input') as HTMLInputElement).value).toBe('https://x.example/authors');
  });

  it('ingests the guidelines URL BEFORE running the review (local, best-effort)', async () => {
    const bridge = makePublishReadyMock(REPORT);
    renderPR({ forceTier: 'premium', bridge });
    await reachRunnable();
    fireEvent.change(screen.getByTestId('pr-guidelines-input'), { target: { value: 'https://journal.example/authors' } });
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');
    expect(bridge.callOrder).toEqual(['ingest', 'run']); // ingest FIRST
    expect(bridge.ingestCalls).toEqual([{ guidelinesUrl: 'https://journal.example/authors' }]);
  });

  it('skips ingestion when the guidelines URL is blank (structural checks only)', async () => {
    const bridge = makePublishReadyMock(REPORT);
    renderPR({ forceTier: 'premium', bridge });
    await reachRunnable();
    fireEvent.change(screen.getByTestId('pr-guidelines-input'), { target: { value: '' } }); // clear any prefill
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');
    expect(bridge.callOrder).toEqual(['run']); // no ingest call
    expect(bridge.ingestCalls).toEqual([]);
  });

  it('checklist shows an honest "no guidelines" note when none were ingested — NOT a failure', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT_NO_GUIDELINES) });
    await reachRunnable();
    fireEvent.change(screen.getByTestId('pr-guidelines-input'), { target: { value: '' } });
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');
    fireEvent.click(await screen.findByTestId('tab-Checklist'));
    const note = await screen.findByTestId('checklist-no-guidelines');
    expect(note.textContent).toMatch(/only the structural checks/i);
    // and NO fabricated "guidelines available" FAIL row (Set 1's honest degradation)
    expect(screen.queryByText(/guidelines available/i)).toBeNull();
    // No URL was supplied, so the wrong-page message must NOT appear.
    expect(screen.queryByTestId('checklist-guidelines-empty')).toBeNull();
  });

  it('a supplied URL that yields no requirements says THAT, not "none were provided"', async () => {
    // The journal-homepage case: ingestion succeeds and reports a plausible
    // chunk count while contributing zero requirements. Saying "no guidelines
    // were provided" would be false — the user did provide one.
    // ARCHITECTURE_TRACE §18.6.2.
    const url = 'https://www.bmj.com';
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT_NO_GUIDELINES) });
    await reachRunnable();
    fireEvent.change(screen.getByTestId('pr-guidelines-input'), { target: { value: url } });
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');
    fireEvent.click(await screen.findByTestId('tab-Checklist'));

    const note = await screen.findByTestId('checklist-guidelines-empty');
    // The claim is scoped to OUR detector coverage, not to the page's content —
    // a genuine guidelines page requiring no structured abstract and using
    // author-date citations also yields zero hits, and must not be told to go
    // find a different page.
    expect(note.textContent).toMatch(/requirements that Gaply currently detects/i);
    // Naming the URL is an externally observable property — the report
    // identifies which page it analysed — not an implementation choice, so
    // asserting it is legitimate under §4.17. That is the same distinction the
    // old prefill test got wrong: it asserted HOW the field was populated
    // (the defect) instead of WHAT the user ends up able to act on.
    expect(note.textContent).toContain(url);
    // Observation and interpretation stay separate, with BOTH explanations named
    // and neither asserted as fact.
    expect(note.textContent).toMatch(/this may mean/i);
    expect(note.textContent).toMatch(/journal homepage/i);
    expect(note.textContent).toMatch(/Gaply does not yet detect/i);
    // The two empty states are mutually exclusive — the "none provided" claim
    // is false here and must not also render.
    expect(screen.queryByTestId('checklist-no-guidelines')).toBeNull();
  });

  it('checklist hides the note when guideline-derived items exist', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) }); // REPORT has a guideline_source item
    await reachRunnable();
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');
    fireEvent.click(await screen.findByTestId('tab-Checklist'));
    await screen.findByTestId('checklist');
    expect(screen.queryByTestId('checklist-no-guidelines')).toBeNull();
  });
});

/* ------------------- the profiled journals in the picker ------------------ */

/** **§11 D183 — the difference between a crawled journal and any other must be
 *  VISIBLE, and each journal must appear ONCE.**
 *
 *  Picking a profiled journal passes the crawler's key, and the backend builds
 *  the checklist from that journal's own extracted requirements — measured at
 *  nine rows for Nature Medicine against four structural rows for anything else.
 *  If the two look identical in the picker, the product hides the thing that
 *  makes the choice matter.
 *
 *  Measured before this shipped: 4 of the 10 profiled journals ALSO appear in
 *  the bundled Scopus directory under the same name, so appending rather than
 *  merging would show one journal twice, once useful and once not. */
describe('§11 D183 · profiled journals in the picker', () => {
  const profiled = [
    {
      key: 'nature-medicine',
      name: 'Nature Medicine',
      entry: 'https://www.nature.com/nm',
      ingested: true,
      origin: 'bundled',
      requirement_count: 39,
      conflict_count: 0,
      convention_count: 0,
      expectation_count: 0,
      standard_count: 11,
      by_pattern: 39,
      by_model: 0,
      version: 1,
      fetched_at: 1,
    },
  ];

  beforeEach(() => {
    vi.spyOn(fpBridge.tauriJournalFingerprintBridge, 'profiles').mockResolvedValue(profiled as any);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('shows what a crawled journal buys, and says plainly when a journal was not crawled', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-journal-input'), {
      target: { value: 'nature medicine' },
    });
    const row = await screen.findByTestId('pr-journal-profiled-Nature Medicine');
    expect(row.textContent).toContain('39');
    expect(row.textContent!.toLowerCase()).toContain('guidelines');

    // And a journal the crawler has never seen says so rather than looking the
    // same. `Lancet Oncology` is in the Scopus directory and not profiled.
    fireEvent.change(screen.getByTestId('pr-journal-input'), {
      target: { value: 'lancet oncology' },
    });
    const plain = await screen.findByTestId('pr-journal-unprofiled-Lancet Oncology');
    expect(plain.textContent!.toLowerCase()).toContain('structural checks only');
  });

  it('says whether a profile was bundled or fetched here, and when', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-journal-input'), {
      target: { value: 'nature medicine' },
    });
    const row = await screen.findByTestId('pr-journal-profiled-Nature Medicine');
    // **§11 D186.** "shipped in the app" and "this machine fetched it from the
    // journal" answer different questions for a researcher weighing a limit
    // that may be months old, and the date is the half they act on.
    expect(row.textContent).toContain('bundled with this release');
    expect(row.textContent).toMatch(/\d{1,4}[/.-]\d{1,2}[/.-]\d{1,4}/);

    // A locally crawled profile must NOT say "bundled".
    vi.spyOn(fpBridge.tauriJournalFingerprintBridge, 'profiles').mockResolvedValue([
      { ...profiled[0], origin: 'crawled' },
    ] as any);
    cleanup();
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-journal-input'), {
      target: { value: 'nature medicine' },
    });
    const crawled = await screen.findByTestId('pr-journal-profiled-Nature Medicine');
    expect(crawled.textContent).toContain('fetched on this device');
    expect(crawled.textContent).not.toContain('bundled');
  });

  it('shows a journal in BOTH sets exactly once, and it is the profiled one', async () => {
    renderPR({ forceTier: 'premium', bridge: makePublishReadyMock(REPORT) });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-journal-input'), {
      target: { value: 'nature medicine' },
    });
    await screen.findByTestId('pr-journal-profiled-Nature Medicine');
    // Nature Medicine is in the Scopus directory too. One entry, not two.
    expect(screen.getAllByTestId(/^pr-journal-Nature Medicine$/)).toHaveLength(1);
    expect(screen.queryByTestId('pr-journal-unprofiled-Nature Medicine')).toBeNull();
  });

  it('passes the crawler key to the run for a profiled journal', async () => {
    const bridge = makePublishReadyMock(REPORT);
    const runSpy = vi.spyOn(bridge, 'run');
    renderPR({ forceTier: 'premium', bridge });
    await screen.findByTestId('pr-entry');
    fireEvent.change(screen.getByTestId('pr-file'), {
      target: { files: [new File(['x'], 'p.pdf', { type: 'application/pdf' })] },
    });
    await screen.findByText('p.pdf');
    fireEvent.change(screen.getByTestId('pr-journal-input'), {
      target: { value: 'nature medicine' },
    });
    fireEvent.click(await screen.findByTestId('pr-journal-Nature Medicine'));
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');

    // **The key is what makes the checklist the journal's own.** Without it the
    // backend reads no `journal_requirements` and the researcher gets the four
    // structural rows, which is indistinguishable from not having picked a
    // crawled journal at all.
    expect(runSpy).toHaveBeenCalled();
    expect(runSpy.mock.calls[0][0].journal.key).toBe('nature-medicine');
  });

  it('sends NO key for a journal the crawler has never seen', async () => {
    const bridge = makePublishReadyMock(REPORT);
    const runSpy = vi.spyOn(bridge, 'run');
    renderPR({ forceTier: 'premium', bridge });
    await reachRunnable();
    fireEvent.click(screen.getByTestId('pr-run'));
    await screen.findByTestId('publishready');
    expect(runSpy.mock.calls[0][0].journal.key).toBeUndefined();
  });
});
