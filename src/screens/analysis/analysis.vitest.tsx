// F5 — upload validation + live analysis theater. No real Tauri, no network.
import React from 'react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { validateFile, MAX_PAGES } from './validateFile';
import { makeMockBridge, AnalysisBridge, AnalysisEvent } from './bridge';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import AnalysisTheaterPage from './AnalysisTheaterPage';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const SECTIONS = ['Abstract', 'Introduction', 'Methods', 'Results', 'Discussion'];

function fakeAuth(session: any = null): AuthService {
  return {
    signUp: vi.fn(),
    signIn: vi.fn(),
    signInWithOAuth: vi.fn(),
    signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session, offline: session === null }),
    onAuthStateChange: (cb: (s: any) => void) => {
      cb(session);
      return () => {};
    },
  } as any;
}

function renderTheater(bridge: AnalysisBridge, props: any = {}) {
  return render(
    <MemoryRouter initialEntries={['/app/upload']}>
      <GaplySessionProvider authService={fakeAuth()}>
        <Routes>
          <Route
            path="/app/upload"
            element={<AnalysisTheaterPage bridge={bridge} {...props} />}
          />
        </Routes>
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

/* ------------------------- upload validation ------------------------- */

describe('upload validation (specific, never a bare spinner)', () => {
  it('rejects an over-page-limit PDF with a specific page error', () => {
    const res = validateFile({ name: 'thesis.pdf', sizeBytes: 5_000_000, pageCount: 620 });
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toBe(`File is 620 pages — max ${MAX_PAGES}.`);
  });

  it('rejects a genuinely unsupported file type by name', () => {
    // .txt IS supported now (the Rust core reads it); use a real unsupported type.
    const res = validateFile({ name: 'slides.pptx', sizeBytes: 1000 });
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error).toMatch(/is a \.pptx file/);
  });

  it('accepts a .txt manuscript (the core reads pdf/docx/txt/md)', () => {
    expect(validateFile({ name: 'notes.txt', sizeBytes: 1000 })).toEqual({ ok: true });
  });

  it('rejects an empty file', () => {
    const res = validateFile({ name: 'empty.pdf', sizeBytes: 0 });
    expect(res.ok).toBe(false);
  });

  it('accepts a valid PDF within limits (and a DOCX without a page count)', () => {
    expect(validateFile({ name: 'ok.pdf', sizeBytes: 2_000_000, pageCount: 40 })).toEqual({ ok: true });
    expect(validateFile({ name: 'ok.docx', sizeBytes: 500_000 })).toEqual({ ok: true });
  });
});

/* ---------------------- theater: live progress ---------------------- */

describe('analysis theater (mocked Tauri command stream)', () => {
  it('renders live per-stage/per-section progress and completes', async () => {
    const bridge = makeMockBridge({ sections: SECTIONS, tier: 'free', stepMs: 0 });
    renderTheater(bridge, { initialFile: { name: 'paper.pdf', path: 'paper.pdf' }, autoStart: true });

    // all six lanes present
    for (const s of ['extraction', 'validation', 'ai', 'plagiarism', 'rag', 'verification']) {
      expect(await screen.findByTestId(`lane-${s}`)).toBeTruthy();
    }
    // extraction summary reflects the streamed section count
    await waitFor(() =>
      expect(screen.getByTestId('summary-extraction').textContent).toMatch(/Parsed 5 sections/)
    );
    // AI lane shows the MANDATORY uncertainty disclaimer
    await waitFor(() =>
      expect(screen.getByTestId('ai-disclaimer').textContent).toMatch(/never proof/i)
    );
    await screen.findByText('complete');
    // section browser populated from the extraction stream
    expect(screen.getByTestId('section-browser').textContent).toMatch(/Sections \(5\)/);
  });

  it('per-section navigation works mid-scan (browse a completed section)', async () => {
    // slow the stream so stages are still running when we click
    const bridge = makeMockBridge({ sections: SECTIONS, tier: 'free', stepMs: 5 });
    renderTheater(bridge, { initialFile: { name: 'paper.pdf', path: 'paper.pdf' }, autoStart: true });

    // wait until at least two sections have streamed in
    const first = await screen.findByTestId('section-0');
    await screen.findByTestId('section-1');
    // click the second completed section while later stages still run
    fireEvent.click(screen.getByTestId('section-1'));
    await waitFor(() =>
      expect(screen.getByTestId('current-section').textContent).toContain(SECTIONS[1])
    );
    // it was navigable before the whole run finished
    expect(first).toBeTruthy();
  });

  it('free users see the verification (cloud) lane LOCKED/teased', async () => {
    const bridge = makeMockBridge({ sections: SECTIONS, tier: 'free', stepMs: 0 });
    renderTheater(bridge, { initialFile: { name: 'p.pdf', path: 'p.pdf' }, autoStart: true });
    expect(await screen.findByTestId('verification-locked')).toBeTruthy();
  });

  it('premium runs the verification lane as a live ReConcile debate', async () => {
    const bridge = makeMockBridge({
      sections: SECTIONS,
      tier: 'premium',
      stepMs: 0,
      debate: [
        { speaker: 'Verification', text: 'DOI matches.', verdict: 'SUPPORTED' },
        { speaker: 'RevisingAgent', text: 'Peer finding grounded; downgrading.', verdict: 'UNKNOWN' },
      ],
    });
    // premium is passed via the bridge script (page tier prop is F6+); assert
    // the debate renders its turns.
    renderTheater(bridge, {
      initialFile: { name: 'p.pdf', path: 'p.pdf' },
      autoStart: true,
    });
    await waitFor(() => expect(screen.getByTestId('debate')).toBeTruthy());
    expect(screen.getByTestId('debate').textContent).toMatch(/DOI matches/);
  });

  it('abort stops the run and marks it aborted', async () => {
    const bridge = makeMockBridge({ sections: SECTIONS, tier: 'free', stepMs: 20 });
    renderTheater(bridge, { initialFile: { name: 'p.pdf', path: 'p.pdf' }, autoStart: true });
    const abortBtn = await screen.findByTestId('abort');
    fireEvent.click(abortBtn);
    await screen.findByText('aborted');
  });
});

/* --------------- zero network carries manuscript text --------------- */

describe('locality: no manuscript bytes leave the machine', () => {
  it('a local scan makes NO network calls (fetch/XHR) at all', async () => {
    const fetchSpy = vi.fn();
    const origFetch = global.fetch;
    (global as any).fetch = fetchSpy;
    const xhrOpen = vi.spyOn(XMLHttpRequest.prototype, 'open');

    // A bridge that records what it is handed — proving only a PATH string
    // (never file bytes) is passed across the seam.
    const handed: any[] = [];
    const recordingBridge: AnalysisBridge = {
      async run(input, emit) {
        handed.push(input);
        makeMockBridge({ sections: SECTIONS, tier: 'free', stepMs: 0 }).run(input, emit);
      },
    };

    renderTheater(recordingBridge, {
      initialFile: { name: 'secret-thesis.pdf', path: '/Users/me/secret-thesis.pdf' },
      autoStart: true,
    });
    await screen.findByText('complete');

    expect(fetchSpy).not.toHaveBeenCalled();
    expect(xhrOpen).not.toHaveBeenCalled();
    // the bridge only ever received a path + title, no bytes/Blob/ArrayBuffer
    expect(handed[0]).toMatchObject({ path: '/Users/me/secret-thesis.pdf' });
    const serialized = JSON.stringify(handed[0]);
    expect(serialized).not.toMatch(/Blob|ArrayBuffer|base64/i);

    global.fetch = origFetch;
    xhrOpen.mockRestore();
  });
});
