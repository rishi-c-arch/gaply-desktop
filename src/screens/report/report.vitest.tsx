// F6 — report viewer tests. Golden fixture + pure helpers + PDF export.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import ReportViewerPage from './ReportViewerPage';
import { SAMPLE_REPORT } from './sampleReport';
import { sortFindings, tierStatus, Finding, PublishReadyReport } from './reportTypes';
import { reportPdfBlob } from './exportPdf';

vi.mock('../../design-system/GaplyGlobe', () => ({
  GaplyGlobe: ({ scale }: { scale: string }) => <div data-testid={`globe-stub-${scale}`} />,
}));

afterEach(cleanup);

const offlineAuth: AuthService = {
  signUp: vi.fn(),
  signIn: vi.fn(),
  signInWithOAuth: vi.fn(),
  signOut: vi.fn(),
  getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
  onAuthStateChange: (cb: any) => {
    cb(null);
    return () => {};
  },
} as any;

function renderReport(report?: PublishReadyReport, isPaid = false) {
  return render(
    <MemoryRouter>
      <GaplySessionProvider authService={offlineAuth}>
        <ReportViewerPage report={report} isPaid={isPaid} />
      </GaplySessionProvider>
    </MemoryRouter>
  );
}

/* ------------------------------ golden render ------------------------------ */

describe('golden render against a sample compile_report() output', () => {
  it('renders the three-panel viewer with tabs, outline, manuscript, inspector', async () => {
    renderReport();
    await screen.findByTestId('report-viewer');
    // top tabs (free = 6, no Reviewer Letter)
    for (const t of ['Overview', 'Statistics', 'Citations', 'AI Risk', 'Plagiarism', 'Checklist']) {
      expect(screen.getByTestId(`tab-${t}`)).toBeTruthy();
    }
    expect(screen.queryByTestId('tab-Reviewer Letter')).toBeNull();
    // outline tree + manuscript + inspector
    expect(screen.getByTestId('outline')).toBeTruthy();
    expect(screen.getByTestId('manuscript')).toBeTruthy();
    expect(screen.getByTestId('inspector')).toBeTruthy();
    // mandatory disclaimer, verbatim-ish
    expect(screen.getByTestId('report-disclaimer').textContent).toMatch(/never definitive proof/i);
  });

  it('paid PublishReady adds the Reviewer Letter tab', async () => {
    renderReport(SAMPLE_REPORT, true);
    await screen.findByTestId('report-viewer');
    expect(screen.getByTestId('tab-Reviewer Letter')).toBeTruthy();
  });
});

/* -------------------- hard constraints sort first ------------------------- */

describe('priority ordering', () => {
  it('green hard-constraint findings sort first regardless of AI-assessed confidence', () => {
    // An AI-assessed finding at MAX confidence must NOT outrank a critical
    // mathematically-certain finding at lower confidence.
    const ai: Finding = {
      severity: 'major',
      tier: 'ai_assessed_moderate',
      certainty_label: 'AI-assessed, moderate confidence',
      agent: 'ai_detection',
      title: 'AI concern (confident)',
      detail: '',
      confidence: 0.99,
      provenance: ['swarm:round-table'],
    };
    const hard: Finding = {
      severity: 'critical',
      tier: 'mathematically_certain',
      certainty_label: 'mathematically certain',
      agent: 'validation_maths',
      title: 'rule failed',
      detail: '',
      confidence: 0.5,
      provenance: ['rule:MissingEffectSize'],
    };
    const sorted = sortFindings([ai, hard]);
    expect(sorted[0]).toBe(hard);
    expect(sorted[0].tier).toBe('mathematically_certain');
    expect(sorted[0].severity).toBe('critical');
  });

  it('the viewer lists the critical green finding first for the sample report', async () => {
    renderReport();
    await screen.findByTestId('finding-0');
    const first = screen.getByTestId('finding-0');
    expect(first.getAttribute('data-tier')).toBe('mathematically_certain');
  });
});

/* ----------------- provenance + tier label on every finding --------------- */

describe('provenance + certainty tier on every finding', () => {
  it('every finding carries provenance and its human tier label', () => {
    for (const f of SAMPLE_REPORT.findings) {
      expect(f.provenance.length).toBeGreaterThan(0);
      // label must match its tier
      const expected = {
        mathematically_certain: 'mathematically certain',
        ai_assessed_moderate: 'AI-assessed, moderate confidence',
        reconsidered_after_peer_review: 'reconsidered after peer review',
      }[f.tier];
      expect(f.certainty_label).toBe(expected);
    }
  });

  it('clicking a finding shows its provenance + tier in the inspector', async () => {
    renderReport();
    const first = await screen.findByTestId('finding-0');
    fireEvent.click(first);
    const inspector = screen.getByTestId('inspector');
    expect(within(inspector).getByTestId('inspector-tier').textContent).toBe('mathematically certain');
    const prov = within(inspector).getByTestId('inspector-provenance');
    expect(prov.textContent).toMatch(/rule:MissingEffectSize/);
  });

  it('a reconsidered verdict shows before → after', async () => {
    renderReport();
    await screen.findByTestId('report-viewer');
    // find the reconsidered finding and select it
    const idx = sortFindings(SAMPLE_REPORT.findings).findIndex((f) => f.reconsidered);
    fireEvent.click(screen.getByTestId(`finding-${idx}`));
    const recon = screen.getByTestId('inspector-reconsidered');
    expect(recon.textContent).toMatch(/SUPPORTED/);
    expect(recon.textContent).toMatch(/UNKNOWN/);
  });
});

/* ------------------------------- tier colors ------------------------------ */

describe('tier → status color mapping', () => {
  it('maps the three tiers to green/amber/red', () => {
    expect(tierStatus('mathematically_certain')).toBe('certain');
    expect(tierStatus('ai_assessed_moderate')).toBe('assessed');
    expect(tierStatus('reconsidered_after_peer_review')).toBe('flagged');
  });
});

/* --------------------------------- export --------------------------------- */

describe('local PDF export', () => {
  it('produces a non-empty application/pdf Blob', async () => {
    const blob = await reportPdfBlob(SAMPLE_REPORT);
    expect(blob).toBeInstanceOf(Blob);
    expect(blob.type).toBe('application/pdf');
    expect(blob.size).toBeGreaterThan(500);
  });

  it('export button triggers a local download (no network)', async () => {
    const fetchSpy = vi.fn();
    const origFetch = global.fetch;
    (global as any).fetch = fetchSpy;
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    // jsdom lacks createObjectURL
    (URL as any).createObjectURL = vi.fn(() => 'blob:mock');
    (URL as any).revokeObjectURL = vi.fn();

    renderReport();
    fireEvent.click(await screen.findByTestId('export-pdf'));

    await waitFor(() => expect(clickSpy).toHaveBeenCalled()); // a file download was triggered
    expect(fetchSpy).not.toHaveBeenCalled(); // generated locally, no upload

    clickSpy.mockRestore();
    global.fetch = origFetch;
  });
});
