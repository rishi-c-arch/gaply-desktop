// M6 file-path fix (Set 1) — the DIALOG CONTRACT test. This is the regression
// guard for the bug the mock-File tests missed: in the Tauri desktop app, the
// three single-file checks must acquire an ABSOLUTE path from the dialog plugin
// and pass THAT to the bridge — never a bare filename.
//
// isTauri is a module-level const, so we force the desktop branch by mocking the
// util (file-scoped — the existing checks.vitest.tsx stays on isTauri=false and
// keeps exercising the <input> fallback). The dialog plugin is mocked to return
// an absolute path; we then assert the bridge receives exactly that path.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

// Force the Tauri (desktop) branch for this whole file.
vi.mock('../../utils/isTauri', () => ({ isTauri: true, default: true }));

// The native picker returns an ABSOLUTE path — the exact contract that broke
// (a real Desktop path with spaces & parens, like the smoke-test file).
const ABS = '/Users/rishi/Desktop/FINAL CH 4 rewritten (1).docx';
const BASENAME = 'FINAL CH 4 rewritten (1).docx';
const openMock = vi.fn(async () => ABS);
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }));

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

import AiCheckPage from './AiCheckPage';
import PlagiarismCheckPage from './PlagiarismCheckPage';
import StatsCheckPage from './StatsCheckPage';
import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import { makeMockCheckBridge } from './checkBridge';
import { AICHECK_FIXTURE } from './aicheckFixture';
import { PLAG_EXACT_FIXTURE } from './plagiarismExactFixture';
import type { StatsValidityReport } from './agentTypes';

afterEach(() => {
  cleanup();
  openMock.mockClear();
});

const STATS_OK: StatsValidityReport = { passed: true, checks: [], flags: [] };

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: any) => { cb(null); return () => {}; },
  } as any;
}

function renderScreen(node: React.ReactElement) {
  return render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>{node}</GaplySessionProvider>
    </MemoryRouter>
  );
}

/** Pick a file via the (mocked) native dialog and confirm the basename shows. */
async function pickViaDialog() {
  fireEvent.click(screen.getByTestId('pick-file'));
  await waitFor(() => expect(screen.getByTestId('selected-name').textContent).toBe(BASENAME));
  expect(openMock).toHaveBeenCalledTimes(1);
}

describe('M6 file-path fix — the Tauri dialog supplies an ABSOLUTE path to the bridge', () => {
  it('AI Check: pick → dialog → run_aicheck gets the absolute path (not a bare name)', async () => {
    const calls: Array<[string, string]> = [];
    const bridge = makeMockCheckBridge({ aicheck: AICHECK_FIXTURE, onCall: (c, p) => calls.push([c, p]) });
    renderScreen(<AiCheckPage bridge={bridge} />);

    await pickViaDialog();
    fireEvent.click(screen.getByTestId('run-check'));

    await waitFor(() => expect(calls.some(([c, p]) => c === 'run_aicheck' && p === ABS)).toBe(true));
    // and NEVER the bare filename that caused "check failed"
    expect(calls.some(([, p]) => p === BASENAME)).toBe(false);
  });

  it('Plagiarism Check: pick → dialog → check_plagiarism_exact gets the absolute path', async () => {
    const calls: Array<[string, string]> = [];
    const bridge = makeMockCheckBridge({ exact: PLAG_EXACT_FIXTURE, onCall: (c, p) => calls.push([c, p]) });
    renderScreen(<PlagiarismCheckPage bridge={bridge} />);

    await pickViaDialog();
    fireEvent.click(screen.getByTestId('run-check'));

    await waitFor(() => expect(calls.some(([c, p]) => c === 'check_plagiarism_exact' && p === ABS)).toBe(true));
    expect(calls.some(([, p]) => p === BASENAME)).toBe(false);
  });

  it('Statistical Validation (via CheckScreen): pick → dialog → validate_manuscript gets the absolute path', async () => {
    const calls: Array<[string, string]> = [];
    const bridge = makeMockCheckBridge({ validation: STATS_OK, onCall: (c, p) => calls.push([c, p]) });
    renderScreen(<StatsCheckPage bridge={bridge} />);

    await pickViaDialog();
    fireEvent.click(screen.getByTestId('run-check'));

    await waitFor(() => expect(calls.some(([c, p]) => c === 'validate_manuscript' && p === ABS)).toBe(true));
    expect(calls.some(([, p]) => p === BASENAME)).toBe(false);
  });

  it('the dialog is asked for the right extensions (single-file, no directory)', async () => {
    renderScreen(<AiCheckPage bridge={makeMockCheckBridge({ aicheck: AICHECK_FIXTURE })} />);
    await pickViaDialog();
    expect(openMock).toHaveBeenCalledWith(
      expect.objectContaining({ multiple: false, directory: false }),
    );
  });
});
