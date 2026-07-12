// Set 7 — Research Gap Finder UI tests. Real-component render, mocked bridge
// + auth (no network, no backend). The critical assertions: the code-enforced
// grounded-vs-suggestions separation is VISIBLE, journal facts show
// verified-vs-couldn't-verify, honest offline states, entitlement gating.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import GapFinderPage from './GapFinderPage';
import { makeGapFinderMock, EMPTY_CONSTRAINTS } from './gapfinderBridge';

afterEach(cleanup);

const SESSION = { user: { id: 'u1', email: 'a@b.c' }, access_token: 'jwt-gf' };

function auth(session: any = SESSION): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: false }),
    onAuthStateChange: (cb: any) => { cb(session); return () => {}; },
  } as any;
}

function renderPage(props: any = {}, session: any = SESSION) {
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth(session)}>
        <GapFinderPage session="gf-test" forceEntitlement="entitled" {...props} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

/** Drive the flow to the gaps stage: add a link, build corpus, find gaps. */
async function driveToGaps(bridge = makeGapFinderMock()) {
  renderPage({ bridge });
  fireEvent.change(screen.getByTestId('gf-link-input'), { target: { value: 'https://papers.example/one.pdf' } });
  fireEvent.click(screen.getByTestId('gf-link-add'));
  fireEvent.click(screen.getByTestId('gf-build-corpus'));
  await screen.findByTestId('gf-corpus');
  fireEvent.click(screen.getByTestId('gf-find-gaps'));
  await screen.findByTestId('gf-grounded');
  return bridge;
}

/* ------------------------------ entitlement ------------------------------ */

describe('entitlement gating (UX-only; server is the real gate)', () => {
  it('signed out → sign-in prompt, no flow', async () => {
    renderPage({ forceEntitlement: 'signed_out' }, null);
    expect(await screen.findByTestId('gf-signin')).toBeTruthy();
    expect(screen.queryByTestId('gapfinder')).toBeNull();
  });

  it('not entitled → locked teaser with upgrade link-out and NO prices', async () => {
    renderPage({ forceEntitlement: 'not_entitled' });
    const teaser = await screen.findByTestId('gf-teaser');
    expect(screen.getByTestId('gf-unlock').textContent).toMatch(/Unlock the Gap Finder/);
    expect(teaser.textContent).not.toMatch(/₹|\$|€|INR|USD|\d+\s*\/(mo|yr)/);
  });

  it('offline_unverified → honest "can’t verify plan", not an upsell', async () => {
    renderPage({ forceEntitlement: 'offline_unverified' });
    expect(await screen.findByTestId('gf-offline')).toBeTruthy();
    expect(screen.queryByTestId('gf-unlock')).toBeNull();
  });

  it('entitled → the staged flow renders', async () => {
    renderPage();
    expect(await screen.findByTestId('gapfinder')).toBeTruthy();
    expect(screen.getByTestId('gf-files')).toBeTruthy();
  });
});

/* ------------------------- corpus + the separation ----------------------- */

describe('staged flow wiring', () => {
  it('corpus: links wire to buildCorpus and digests render with stable ids', async () => {
    const bridge = await driveToGaps();
    const corpusCall = bridge.calls.find((c) => c.method === 'buildCorpus');
    expect((corpusCall!.input as any).links).toEqual(['https://papers.example/one.pdf']);
    expect((corpusCall!.input as any).session).toBe('gf-test');
    expect(screen.getByTestId('gf-paper-p1').textContent).toMatch(/p1: Paper 1/);
  });

  it('SEPARATION VISIBLE: grounded gaps and broader suggestions are distinct, honestly labeled sections', async () => {
    await driveToGaps();
    const grounded = screen.getByTestId('gf-grounded');
    const suggestions = screen.getByTestId('gf-suggestions');
    expect(grounded).not.toBe(suggestions);
    expect(grounded.textContent).toMatch(/Grounded in your papers/);
    expect(grounded.textContent).toMatch(/dose-response curve/);
    expect(grounded.textContent).toMatch(/p1/); // paper refs visible
    expect(suggestions.textContent).toMatch(/NOT grounded in your papers/);
    expect(suggestions.textContent).toMatch(/verify them independently/i);
    expect(suggestions.textContent).toMatch(/adjacent modality/);
    // the grounded section never contains the suggestion and vice versa
    expect(grounded.textContent).not.toMatch(/adjacent modality/);
    expect(suggestions.textContent).not.toMatch(/dose-response curve/);
  });

  it('the JWT rides every cloud command', async () => {
    const bridge = await driveToGaps();
    const gapsCall = bridge.calls.find((c) => c.method === 'findGaps');
    expect((gapsCall!.input as any).userToken).toBe('jwt-gf');
  });
});

/* --------------------------------- Q&A ----------------------------------- */

describe('achievability Q&A (constraints stay local)', () => {
  it('a turn accumulates constraints, shows the follow-up and achievable gaps', async () => {
    const bridge = await driveToGaps();
    fireEvent.change(screen.getByTestId('gf-qa-input'), { target: { value: 'we have a small internal grant' } });
    fireEvent.click(screen.getByTestId('gf-qa-send'));
    await screen.findByTestId('gf-achievable');
    // follow-up rendered as an assistant message
    expect(screen.getAllByTestId('gf-qa-assistant').some((el) => /wet-lab access/.test(el.textContent ?? ''))).toBe(true);
    // constraints chip updated from the turn
    expect(screen.getByTestId('gf-constraints').textContent).toMatch(/funding: small internal grant/);
    // the turn was sent with the local snapshot (starts empty), not a transcript
    const qaCall = bridge.calls.find((c) => c.method === 'qaTurn');
    expect((qaCall!.input as any).constraints).toEqual(EMPTY_CONSTRAINTS);
    expect((qaCall!.input as any).latestAnswer).toBe('we have a small internal grant');
  });

  it('an unavailable Q&A turn is honest and keeps the flow intact', async () => {
    const bridge = makeGapFinderMock({
      qaTurn: async () => ({
        constraints: EMPTY_CONSTRAINTS,
        follow_up_question: '',
        achievable_gaps: [],
        suggestions: [],
        warnings: ['Answering questions needs Gaply’s cloud connection…'],
        kind: 'unavailable' as const,
        available: false,
      }),
    });
    await driveToGaps(bridge);
    fireEvent.change(screen.getByTestId('gf-qa-input'), { target: { value: 'hello' } });
    fireEvent.click(screen.getByTestId('gf-qa-send'));
    await waitFor(() =>
      expect(screen.getAllByTestId('gf-qa-assistant').some((el) => /cloud connection/.test(el.textContent ?? ''))).toBe(true)
    );
    expect(screen.queryByTestId('gf-achievable')).toBeNull();
  });
});

/* ------------------------------ draft stage ------------------------------ */

describe('structured draft', () => {
  async function driveToDraft(bridge = makeGapFinderMock()) {
    await driveToGaps(bridge);
    fireEvent.change(screen.getByTestId('gf-qa-input'), { target: { value: 'funded, EEG suite, 12 months' } });
    fireEvent.click(screen.getByTestId('gf-qa-send'));
    await screen.findByTestId('gf-achievable');
    fireEvent.click(screen.getByTestId('gf-draft-run'));
    return bridge;
  }

  it('renders objectives and steps as STRUCTURED list items, never prose blocks', async () => {
    await driveToDraft();
    await screen.findByTestId('gf-draft');
    const objectives = screen.getAllByTestId('gf-objective');
    const steps = screen.getAllByTestId('gf-step');
    expect(objectives.length).toBe(1);
    expect(steps.length).toBe(3);
    expect(objectives[0].tagName).toBe('LI');
    expect(steps[0].tagName).toBe('LI');
    expect(steps[0].textContent).toMatch(/Recruit 40 adults/);
    // grounding visible on the draft card
    expect(screen.getByTestId('gf-draft-g1').textContent).toMatch(/grounded in p1/);
  });

  it('an offline draft is honest and preserves the stage', async () => {
    const bridge = makeGapFinderMock({
      draft: async () => ({ drafts: [], warnings: [], kind: 'unavailable' as const, available: false }),
    });
    await driveToDraft(bridge);
    expect(await screen.findByTestId('gf-draft-offline')).toBeTruthy();
    expect(screen.getByTestId('gf-achievable')).toBeTruthy();
  });
});

/* ---------------------------- journal + fit ------------------------------ */

describe('verified journal card', () => {
  async function driveToJournal(bridge = makeGapFinderMock()) {
    await driveToGaps(bridge);
    fireEvent.change(screen.getByTestId('gf-qa-input'), { target: { value: 'funded' } });
    fireEvent.click(screen.getByTestId('gf-qa-send'));
    await screen.findByTestId('gf-achievable');
    fireEvent.change(screen.getByTestId('gf-journal-issn'), { target: { value: '1365-2869' } });
    fireEvent.click(screen.getByTestId('gf-journal-verify'));
    await screen.findByTestId('gf-journal-card');
    return bridge;
  }

  it('verified facts render as verified; a healthy journal gets the green light, no warning', async () => {
    await driveToJournal();
    expect(screen.getByTestId('gf-journal-doaj').textContent).toMatch(/yes \(verified\)/);
    expect(screen.getByTestId('gf-journal-activity').textContent).toMatch(/2026: 180/);
    expect(screen.getByTestId('gf-journal-ok')).toBeTruthy();
    expect(screen.queryByTestId('gf-journal-warning')).toBeNull();
    expect(screen.getByTestId('gf-journal-card').textContent).toMatch(/never from the AI/);
  });

  it('unverified facts say "couldn’t verify" and the predatory caution keeps its not-a-verdict framing', async () => {
    const bridge = makeGapFinderMock({
      verifyJournal: async (input) => ({
        query: input.issn,
        name: null,
        issn: input.issn,
        doaj_registered: null,
        openalex_in_doaj: null,
        works_by_year: [],
        recent_activity: null,
        scope: [],
        warning:
          'The registry data shows: DOAJ registration could not be verified; publishing activity could not be verified. This is what the data shows — not a verdict; verify this journal independently before submitting.',
        reasons: ['DOAJ registration could not be verified', 'publishing activity could not be verified'],
        verified_sources: [],
        unverified: ['DOAJ registration (registry unreachable)', 'publishing activity / scope (OpenAlex unreachable or no record)'],
      }),
    });
    await driveToJournal(bridge);
    expect(screen.getByTestId('gf-journal-doaj').textContent).toMatch(/couldn’t verify/);
    expect(screen.getByTestId('gf-journal-scope').textContent).toMatch(/couldn’t verify/);
    expect(screen.getByTestId('gf-journal-unverified').textContent).toMatch(/Couldn’t verify:/);
    const warning = screen.getByTestId('gf-journal-warning');
    expect(warning.textContent).toMatch(/not a verdict/);
    expect(warning.textContent).toMatch(/verify this journal independently/);
  });

  it('fit verdict renders as the AI’s reasoning over the verified card; offline fit is honest', async () => {
    const bridge = await driveToJournal();
    fireEvent.click(screen.getByTestId('gf-fit-run'));
    await screen.findByTestId('gf-fit');
    expect(screen.getByTestId('gf-fit').textContent).toMatch(/good fit/);
    expect(screen.getByTestId('gf-fit').textContent).toMatch(/the facts above are the registry’s/);
    const fitCall = bridge.calls.find((c) => c.method === 'fit');
    expect((fitCall!.input as any).userToken).toBe('jwt-gf');
  });
});

/* ------------------------------ honest offline --------------------------- */

describe('honest offline for the cloud stage while local stages work', () => {
  it('gaps unavailable → needs-connection notice; the (local) corpus stays rendered', async () => {
    const bridge = makeGapFinderMock({
      findGaps: async () => ({
        findings: { grounded_gaps: [], suggestions: [], warnings: [], available: false },
        proxy_payload: {},
      }),
    });
    renderPage({ bridge });
    fireEvent.change(screen.getByTestId('gf-link-input'), { target: { value: 'https://x.example/p.pdf' } });
    fireEvent.click(screen.getByTestId('gf-link-add'));
    fireEvent.click(screen.getByTestId('gf-build-corpus'));
    await screen.findByTestId('gf-corpus'); // local stage worked
    fireEvent.click(screen.getByTestId('gf-find-gaps'));
    expect(await screen.findByTestId('gf-gaps-offline')).toBeTruthy();
    expect(screen.getByTestId('gf-gaps-offline').textContent).toMatch(/cloud connection/);
    expect(screen.getByTestId('gf-corpus')).toBeTruthy(); // still there
    expect(screen.queryByTestId('gf-grounded')).toBeNull(); // nothing faked
  });
});
