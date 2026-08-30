// Gaply — Phase 9b screens: status/install, citation panel, thesis audit.
import React from 'react';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AiStatusPanel, AiUnavailable } from './AiStatusPanel';
import { CitationAiPanel } from './CitationAiPanel';
import { ThesisAuditScreen, projectDuration, SECONDS_PER_ITEM } from './ThesisAuditScreen';
import type { JobProgressEvent } from './aiBridge';

vi.mock('react-pdf', () => ({
  pdfjs: { GlobalWorkerOptions: { workerSrc: '' }, version: '4.8.69' },
  Document: ({ children }: any) => <div>{children}</div>,
  Page: ({ pageNumber }: any) => <div data-testid={`mock-page-${pageNumber}`} />,
}));

afterEach(cleanup);

// The wire shape the Rust side actually sends: `#[serde(tag = "state")]` on
// EngineState / GenState. These fixtures said `kind`, which is why nothing
// caught the panel reading a field the backend never emits.
const READY = {
  embedding: { state: 'ready' },
  generative: { state: 'notLoaded' },
  generativeModelId: 'qwen2.5-1.5b-instruct-q4km',
  inFlight: 0,
  generativeRam: { totalBytes: 1024 ** 3 * 2 },
  activeDevice: 'cpu' as const,
  deviceFallbackReason: 'MTLResidencySetDescriptor is absent',
};

describe('AI status + install', () => {
  it('offers ONE install affordance when a model is missing, and no broken buttons', async () => {
    const bridge = {
      modelStatus: async () => ({ ...READY, embedding: { state: 'notInstalled' } }),
      installEmbedding: async () => {},
      installGenerative: async () => {},
      cancelInstall: async () => {},
    };
    render(<AiStatusPanel bridge={bridge as any} />);
    await waitFor(() => expect(screen.getByTestId('ai-not-installed')).toBeTruthy());
    expect(screen.getByTestId('ai-install').textContent).toContain('Install Gaply AI');
    // the deterministic app is described as complete without AI
    expect(screen.getByTestId('ai-not-installed').textContent).toContain('works without');
    expect(screen.queryByTestId('ai-device')).toBeNull();
  });

  it('shows the device as a FACT and the RAM figure from the backend', async () => {
    const bridge = {
      modelStatus: async () => READY,
      installEmbedding: async () => {},
      installGenerative: async () => {},
      cancelInstall: async () => {},
    };
    render(<AiStatusPanel bridge={bridge as any} />);
    await waitFor(() => expect(screen.getByTestId('ai-device')).toBeTruthy());
    // "CPU", not "CPU (Metal unavailable)" — below macOS 15 cpu is correct
    expect(screen.getByTestId('ai-device').textContent).toBe('CPU');
    expect(screen.getByTestId('ai-device').textContent).not.toMatch(/unavailable|fallback|warning/i);
    // 2 GiB, formatted from the backend's byte count — never invented
    expect(screen.getByTestId('ai-ram').textContent).toBe('2.0 GB');
  });

  it('reports a resumed download rather than restarting silently', async () => {
    const bridge = {
      modelStatus: async () => ({ ...READY, embedding: { state: 'notInstalled' } }),
      installEmbedding: async (cb: any) => {
        cb({ kind: 'downloading', file: 'model.safetensors', fromBytes: 500, totalBytes: 1000 });
        await new Promise((r) => setTimeout(r, 0));
      },
      installGenerative: async () => {},
      cancelInstall: async () => {},
    };
    render(<AiStatusPanel bridge={bridge as any} />);
    await waitFor(() => expect(screen.getByTestId('ai-install')).toBeTruthy());
    fireEvent.click(screen.getByTestId('ai-install'));
    await waitFor(() => expect(screen.getByText(/Resuming/)).toBeTruthy());
  });

  it('an INSTALLED model is recognised through the backend\u2019s own `state` tag', async () => {
    // Regression: isReady() read `.kind`, which is always undefined on the wire,
    // so a fully installed + registered pair still showed "Install Gaply AI".
    // Every generative lifecycle state below means the files are on disk.
    for (const gen of ['notLoaded', 'loading', 'ready', 'idle', 'unloading']) {
      const bridge = {
        modelStatus: async () => ({ ...READY, generative: { state: gen } }),
        installEmbedding: async () => {},
        installGenerative: async () => {},
        cancelInstall: async () => {},
      };
      render(<AiStatusPanel bridge={bridge as any} />);
      await waitFor(() => expect(screen.getByTestId('ai-model-id')).toBeTruthy());
      expect(screen.queryByTestId('ai-install')).toBeNull();
      cleanup();
    }
    // ...and the two that genuinely mean "nothing usable on disk" still offer it.
    for (const bad of ['notInstalled', 'corrupt']) {
      const bridge = {
        modelStatus: async () => ({ ...READY, generative: { state: bad } }),
        installEmbedding: async () => {},
        installGenerative: async () => {},
        cancelInstall: async () => {},
      };
      render(<AiStatusPanel bridge={bridge as any} />);
      await waitFor(() => expect(screen.getByTestId('ai-install')).toBeTruthy());
      cleanup();
    }
  });

  it('keeps showing progress across BOTH installers, not just the first', async () => {
    // Regression: the embedding installer's `done` cleared `installing`, so the
    // 1.1 GB generative download that follows ran with the install button back
    // on screen and no progress at all.
    let releaseGenerative: () => void = () => {};
    const generativeStarted = new Promise<void>((resolve) => {
      releaseGenerative = resolve;
    });
    const bridge = {
      modelStatus: async () => ({ ...READY, embedding: { state: 'notInstalled' } }),
      installEmbedding: async (cb: any) => {
        cb({ kind: 'alreadyPresent', file: 'model.safetensors' });
        cb({ kind: 'done', modelId: 'bge-small-en-v1.5', dir: '/m' });
      },
      installGenerative: async (_id: string, cb: any) => {
        cb({ kind: 'progress', file: 'qwen.gguf', bytes: 300, totalBytes: 1000 });
        releaseGenerative();
        await new Promise((r) => setTimeout(r, 50));
      },
      cancelInstall: async () => {},
    };
    render(<AiStatusPanel bridge={bridge as any} />);
    await waitFor(() => expect(screen.getByTestId('ai-install')).toBeTruthy());
    fireEvent.click(screen.getByTestId('ai-install'));
    await generativeStarted;
    await waitFor(() => expect(screen.getByText(/qwen\.gguf — 30%/)).toBeTruthy());
    expect(screen.queryByTestId('ai-install')).toBeNull();
    expect(screen.getByTestId('ai-install-progress')).toBeTruthy();
  });

  it('the NotInstalled affordance explains rather than disabling silently', () => {
    render(<AiUnavailable feature="Citation checking" onOpenSettings={() => {}} />);
    expect(screen.getByTestId('ai-unavailable-text').textContent).toContain('isn’t installed');
    expect(screen.getByTestId('ai-unavailable-install')).toBeTruthy();
  });
});

describe('Citation AI panel', () => {
  const base = {
    citationNeed: async () => ({ output: { needs_citation: true, severity: 'high', reason: 'empirical claim' } }),
    citationSupport: async () => ({}),
    cancelGeneration: async () => {},
    documentSource: async () => ({ documentId: 1, path: '/x.pdf', exists: true, extension: 'pdf' }),
  };

  it('renders NotInstalled instead of dead buttons', () => {
    render(<CitationAiPanel sentence="A claim." aiInstalled={false} bridge={base as any} />);
    expect(screen.getByTestId('ai-unavailable')).toBeTruthy();
    expect(screen.queryByTestId('ai-check-support')).toBeNull();
  });

  it('renders a rejected result honestly, not as a generic error', async () => {
    const bridge = { ...base, citationSupport: async () => ({ outcome: 'validationFailed' }) };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-validation-failed')).toBeTruthy());
    expect(screen.getByTestId('ai-validation-failed').textContent).toContain('failed verification');
    expect(screen.getByTestId('ai-validation-failed').textContent).toContain('discarded');
  });

  it('renders an accepted support result through the evidence unit', async () => {
    const bridge = {
      ...base,
      citationSupport: async () => ({
        outcome: 'ok',
        output: {
          verdict: 'weak',
          confidence: 0.4,
          explanation: 'Reports richness but not the magnitude.',
          supporting_chunks: [{ chunk_id: 'c7', page: 5, quote: 'Richness rose 31 percent.' }],
        },
      }),
    };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('evidence-card')).toBeTruthy());
    expect(screen.getByTestId('evidence-quote-0').textContent).toContain('31 percent');
    expect(screen.getByTestId('evidence-open-0').textContent).toContain('p.5');
  });

  it('citation_need renders as text, NOT through the evidence unit (D8: no evidence block)', async () => {
    render(<CitationAiPanel sentence="A claim." aiInstalled bridge={base as any} />);
    fireEvent.click(screen.getByTestId('ai-check-need'));
    await waitFor(() => expect(screen.getByTestId('ai-need-result')).toBeTruthy());
    expect(screen.getByTestId('ai-need-result').textContent).toContain('Needs a citation');
    // it must NOT render the ungrounded refusal — citation_need has no evidence
    // by design, and showing "cannot be displayed" would be wrong
    expect(screen.queryByTestId('evidence-card')).toBeNull();
  });
});

describe('Thesis audit', () => {
  const plan = {
    jobId: 7,
    documentTypesSupported: ['pdf'],
    totalSentences: 31,
    cited: 20,
    uncited: 11,
    skipped: 3,
    markersFound: 20,
    queuedCitationNeed: 8,
    queuedCitationSupport: 2,
    queuedUnverifiable: 18,
  };

  function bridgeWith(over: Record<string, any> = {}) {
    return {
      startThesisAudit: async () => plan,
      jobStatus: async () => ({ health: { completedItems: 2, flagged: [] } }),
      pauseJob: async () => true,
      resumeJob: async () => ({}),
      cancelJob: async () => true,
      jobResults: async () => ({ items: [] }),
      ...over,
    };
  }

  it('projects duration from item count, excluding instant unverifiable items', () => {
    expect(projectDuration(10, SECONDS_PER_ITEM)).toBe('11 minutes');
    expect(projectDuration(238, SECONDS_PER_ITEM)).toBe('4.3 hours');
  });

  /** THE gate: no model work may begin without an explicit confirmation. */
  it('never starts the model without explicit confirmation', async () => {
    const resumeJob = vi.fn(async () => ({}));
    render(
      <ThesisAuditScreen
        aiInstalled
        pickManuscript={async () => '/thesis.pdf'}
        bridge={bridgeWith({ resumeJob }) as any}
      />,
    );
    fireEvent.click(screen.getByTestId('audit-pick'));
    await waitFor(() => expect(screen.getByTestId('audit-plan')).toBeTruthy());

    // the pre-pass is shown and NOTHING has run
    expect(screen.getByTestId('audit-projection').textContent).toContain('10 need the model');
    expect(screen.getByTestId('audit-projection').textContent).toContain('11 minutes');
    expect(screen.getByTestId('audit-plan-unverifiable').textContent).toContain('18');
    expect(resumeJob).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('audit-confirm-start'));
    await waitFor(() => expect(resumeJob).toHaveBeenCalledTimes(1));
  });

  it('fills the results list from streamed progress events', async () => {
    let emit: ((e: JobProgressEvent) => void) | undefined;
    const bridge = bridgeWith({
      startThesisAudit: async (_p: string, cb: any) => {
        emit = cb;
        return plan;
      },
      jobResults: async () => ({
        items: [
          { seq: 0, kind: 'citation_need', page: 1, sentence: 'An uncited claim.', status: 'done' },
          { seq: 1, kind: 'unverifiable', page: null, sentence: 'A cited claim.', status: 'done' },
        ],
      }),
    });
    render(<ThesisAuditScreen aiInstalled pickManuscript={async () => '/t.pdf'} bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('audit-pick'));
    await waitFor(() => expect(emit).toBeTruthy());

    emit!({ jobId: 7, completed: 2, total: 28, currentCategory: 'citation_need', latestItemSummary: 'needs_citation=true' });

    await waitFor(() => expect(screen.getByTestId('audit-results')).toBeTruthy());
    expect(screen.getByTestId('audit-group-citation_need')).toBeTruthy();
    expect(screen.getByTestId('audit-group-unverifiable')).toBeTruthy();
    // page:null never becomes a fake number
    expect(screen.getByTestId('audit-item-1').textContent).toContain('page unknown');
    // unverifiable items carry their (deferred) affordances, clearly marked
    expect(screen.getByTestId('audit-link-1').textContent).toContain('coming soon');
  });

  it('offers a resume when a job was interrupted by a restart', async () => {
    const resumeJob = vi.fn(async () => ({}));
    render(
      <ThesisAuditScreen
        aiInstalled
        resumableJob={{ jobId: 7, completed: 42, total: 238 }}
        bridge={bridgeWith({ resumeJob }) as any}
      />,
    );
    expect(screen.getByTestId('audit-resume-offer').textContent).toContain('42 of 238');
    expect(screen.getByTestId('audit-resume-offer').textContent).toContain('kept');
    fireEvent.click(screen.getByTestId('audit-resume'));
    await waitFor(() => expect(resumeJob).toHaveBeenCalledWith(7, expect.anything()));
  });

  it('renders NotInstalled instead of an audit nobody can run', () => {
    render(<ThesisAuditScreen aiInstalled={false} bridge={bridgeWith() as any} />);
    expect(screen.getByTestId('ai-unavailable')).toBeTruthy();
    expect(screen.queryByTestId('audit-pick')).toBeNull();
  });
});
