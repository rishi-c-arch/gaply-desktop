// Gaply — Phase 9b screens: status/install, citation panel, thesis audit.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AiStatusPanel, AiUnavailable } from './AiStatusPanel';
import { errorCode, errorText } from './aiBridge';
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

describe('rejected invokes render as text', () => {
  // Tauri rejects with the command's serialized Err. GaplyError is
  // `{ code, message }` — a plain object — so the old
  // `e instanceof Error ? e.message : String(e)` printed "[object Object]" in
  // red for every real backend failure. This is the property, not the example.
  const SHAPES: Array<[string, unknown]> = [
    ['a GaplyError from the wire', { code: 'validation', message: "the model's output failed validation twice: <output>: not valid JSON" }],
    ['a TaskError surfaced as validation', { code: 'validation', message: 'generation failed: forward at 3: shape mismatch' }],
    ['a cancellation', { code: 'cancelled', message: 'cancelled' }],
    ['a real Error', new Error('boom')],
    ['a bare string', 'something went wrong'],
    ['an object with no message at all', { code: 'internal', detail: 42 }],
    ['an empty object', {}],
    ['null', null],
    ['undefined', undefined],
  ];

  it.each(SHAPES)('%s never renders as [object Object]', (_label, shape) => {
    const text = errorText(shape);
    expect(text).not.toBe('[object Object]');
    expect(text).not.toContain('[object Object]');
    expect(text.trim().length).toBeGreaterThan(0);
  });

  it('prefers the message the backend actually sent', () => {
    expect(errorText({ code: 'validation', message: 'failed validation twice' })).toBe(
      'failed validation twice',
    );
  });

  it('reads the code so a cancellation is not mistaken for a fault', () => {
    expect(errorCode({ code: 'cancelled', message: 'cancelled' })).toBe('cancelled');
    expect(errorCode(new Error('boom'))).toBeNull();
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

  it('renders a structured TaskError as its message, never [object Object]', async () => {
    // The live failure: the 1.5B ran out of room mid-JSON, run_task failed
    // validation twice, and the command rejected with GaplyError's wire shape.
    const wire = {
      code: 'validation',
      message:
        "the model's output failed validation twice: <output>: not valid JSON: EOF while parsing a list at line 54 column 5",
    };
    const bridge = { ...base, citationSupport: async () => Promise.reject(wire) };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-error')).toBeTruthy());
    const shown = screen.getByTestId('ai-error').textContent ?? '';
    expect(shown).not.toContain('[object Object]');
    expect(shown).toContain('failed validation twice');
  });

  it('citation_need renders a structured error as text too', async () => {
    const bridge = { ...base, citationNeed: async () => Promise.reject({ code: 'internal', message: 'model lock poisoned' }) };
    render(<CitationAiPanel sentence="A claim." aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-need'));
    await waitFor(() => expect(screen.getByTestId('ai-error')).toBeTruthy());
    expect(screen.getByTestId('ai-error').textContent).toBe('model lock poisoned');
  });

  it('a cancelled invoke is read from its code, not its message text', async () => {
    // Cancel from anywhere else (another surface, a job runner) still arrives
    // here as a rejection; the panel must not paint it red.
    const bridge = { ...base, citationSupport: async () => Promise.reject({ code: 'cancelled', message: 'cancelled' }) };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-cancelled')).toBeTruthy());
    expect(screen.queryByTestId('ai-error')).toBeNull();
  });

  it('a validationFailed OUTCOME carries the reason, so "why" is not lost', async () => {
    const bridge = {
      ...base,
      citationSupport: async () => ({
        outcome: 'validationFailed',
        reason: "the model's output failed validation twice: <output>: not valid JSON",
        persisted: false,
      }),
    };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-validation-failed')).toBeTruthy());
    const t = screen.getByTestId('ai-validation-failed').textContent ?? '';
    expect(t).toContain('failed verification');
    expect(t).toContain('not valid JSON');
    expect(t).not.toContain('[object Object]');
  });

  it('takes the claim as INPUT and never prefills it with the reference title', () => {
    // Regression: the Citation Manager passed selected.csl.title as the claim,
    // so the request read "does this document support 'Chapter 1 — Literature
    // Review'?" with the same string as claim AND cited source. A title has
    // none of the elements the prompt asks the model to decompose, and the
    // model answered with prose. The eval harness runs this same code path
    // against real sentences and passes.
    render(
      <CitationAiPanel
        sentence=""
        documentId={1}
        citedSource="Chapter 1 — Literature Review"
        aiInstalled
        bridge={base as any}
      />,
    );
    const field = screen.getByTestId('ai-claim') as HTMLTextAreaElement;
    expect(field.value).toBe('');
    // Nothing runs without one — a check on an empty claim can only waste
    // minutes of CPU to reject itself.
    expect((screen.getByTestId('ai-check-support') as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId('ai-check-need') as HTMLButtonElement).disabled).toBe(true);

    fireEvent.change(field, { target: { value: '  Organic management increased richness by 31 percent.  ' } });
    expect((screen.getByTestId('ai-check-support') as HTMLButtonElement).disabled).toBe(false);
  });

  it('sends the typed claim, trimmed — not the citation title', async () => {
    const citationSupport = vi.fn(async (..._args: unknown[]) => ({ outcome: 'noEvidence', reason: 'none' }));
    render(
      <CitationAiPanel
        sentence=""
        documentId={7}
        citedSource="Chapter 1 — Literature Review"
        aiInstalled
        bridge={{ ...base, citationSupport } as any}
      />,
    );
    fireEvent.change(screen.getByTestId('ai-claim'), { target: { value: '  A checkable claim.  ' } });
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(citationSupport).toHaveBeenCalled());
    expect(citationSupport.mock.calls[0][0]).toBe('A checkable claim.');
  });

  it('stamps WHICH weights answered, on results and on failures alike', async () => {
    // "Which model was this?" cost a whole diagnosis once. The registry records
    // what was CONFIGURED; only the backend knows what it opened.
    const withFile = (extra: Record<string, unknown>) => ({
      ...base,
      citationSupport: async () => ({ loadedModelFile: 'qwen2.5-3b-instruct-q4_k_m.gguf', ...extra }),
    });

    const ok = {
      outcome: 'ok',
      output: { verdict: 'weak', confidence: 0.4, explanation: 'x', supporting_chunks: [] },
    };
    const { unmount } = render(
      <CitationAiPanel sentence="" documentId={1} aiInstalled bridge={withFile(ok) as any} />,
    );
    fireEvent.change(screen.getByTestId('ai-claim'), { target: { value: 'A claim.' } });
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-judged-by')).toBeTruthy());
    expect(screen.getByTestId('ai-judged-by').textContent).toContain('qwen2.5-3b-instruct-q4_k_m.gguf');
    unmount();

    const failed = { outcome: 'validationFailed', reason: 'no JSON object or array found in the reply' };
    render(<CitationAiPanel sentence="" documentId={1} aiInstalled bridge={withFile(failed) as any} />);
    fireEvent.change(screen.getByTestId('ai-claim'), { target: { value: 'A claim.' } });
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-validation-failed')).toBeTruthy());
    expect(screen.getByTestId('ai-judged-by').textContent).toContain('qwen2.5-3b-instruct-q4_k_m.gguf');
  });

  it('carries the examined passages through, so an unsupported verdict has something to show', async () => {
    // End to end through the panel: the command sends examinedPassages, and
    // toFinding must map them into the same verifiable row shape as cited
    // evidence — same document, same source label, same page linking.
    const bridge = {
      ...base,
      citationSupport: async () => ({
        outcome: 'ok',
        chunksSent: 9,
        examinedPassages: [{ chunkId: 'c3', page: 8, text: 'Soil pH was measured monthly.' }],
        output: {
          verdict: 'insufficient_evidence',
          confidence: 0.2,
          explanation: 'The retrieved passages do not address the claim.',
          supporting_chunks: [],
        },
      }),
    };
    render(
      <CitationAiPanel
        sentence=""
        documentId={7}
        citedSource="Chapter 1"
        aiInstalled
        bridge={bridge as any}
      />,
    );
    fireEvent.change(screen.getByTestId('ai-claim'), { target: { value: 'An unsupported claim.' } });
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('evidence-searched')).toBeTruthy());
    expect(screen.getByTestId('evidence-searched').textContent).toContain('Checked 9 retrieved passages');
    expect(screen.getByTestId('evidence-examined-quote-0').textContent).toContain('Soil pH');
    expect(screen.getByTestId('evidence-examined-open-0').textContent).toContain('Chapter 1 — p.8');
    expect(screen.queryByTestId('evidence-ungrounded-notice')).toBeNull();
  });

  it('says WHY support is unavailable when no document is linked, instead of a dead button', () => {
    // Regression: the reason lived in a `title` tooltip on a greyed-out button,
    // so on Wakefield/Naidu (no citation_documents row) the check looked broken.
    render(<CitationAiPanel sentence="A claim." documentId={null} aiInstalled bridge={base as any} />);
    expect(screen.queryByTestId('ai-check-support')).toBeNull();
    const why = screen.getByTestId('ai-support-unavailable').textContent ?? '';
    expect(why).toMatch(/isn’t linked to an indexed document/);
    // "Needs a citation?" judges a sentence, not a source — it stays available.
    expect(screen.getByTestId('ai-check-need')).toBeTruthy();
  });

  it('reports the live stage and elapsed time, never a predicted duration', async () => {
    // Regression: the panel showed one static "this takes about a minute" for
    // the whole run, so a working multi-minute check and a wedged one looked
    // identical. The command streams these stages; the bridge used to drop them.
    let emit: (ev: any) => void = () => {};
    const bridge = {
      ...base,
      citationSupport: (_c: string, _d: number, _s: string | undefined, onEvent: any) => {
        emit = onEvent;
        return new Promise(() => {}); // never settles: we are inspecting mid-run
      },
    };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-running')).toBeTruthy());
    expect(screen.getByTestId('ai-running').textContent).toMatch(/0:00 elapsed/);
    expect(screen.getByTestId('ai-running').textContent).not.toMatch(/about a minute/);

    await act(async () => emit({ kind: 'retrieved', chunksSent: 6, chunksDropped: 5 }));
    expect(screen.getByTestId('ai-progress').textContent).toMatch(/Judging 6 passages \(5 dropped/);

    // The queue is the difference between "slow" and "waiting" — say which.
    await act(async () => emit({ kind: 'generating', queuedBehind: 2 }));
    expect(screen.getByTestId('ai-progress').textContent).toMatch(/2 other checks ahead/);

    await act(async () => emit({ kind: 'generating', queuedBehind: 0 }));
    expect(screen.getByTestId('ai-progress').textContent).not.toMatch(/ahead/);

    await act(async () => emit({ kind: 'decoding', tokens: 64, maxTokens: 1024 }));
    expect(screen.getByTestId('ai-progress').textContent).toMatch(/64 of up to 1024 tokens/);
  });

  it('reports a user cancellation as a decision, not a failure — and says why it is not instant', async () => {
    // Regression: Cancel called through to the backend and changed nothing on
    // screen, so a press that WAS working looked like a dead button. It cannot
    // be instant — cancellation is checked between model steps and the first
    // step reads the whole prompt — so the panel says that rather than nothing.
    let reject: (e: Error) => void = () => {};
    const bridge = {
      ...base,
      cancelGeneration: vi.fn(async () => {}),
      citationSupport: () => new Promise((_res, rej) => { reject = rej; }),
    };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-cancel')).toBeTruthy());

    fireEvent.click(screen.getByTestId('ai-cancel'));
    expect(bridge.cancelGeneration).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId('ai-cancel').textContent).toContain('Stopping');
    expect((screen.getByTestId('ai-cancel') as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByTestId('ai-progress').textContent).toMatch(/has to finish first/);

    // The backend then rejects the cancelled run. That is the user's own
    // request coming back — it must not be styled as an error.
    await act(async () => reject(new Error('cancelled')));
    await waitFor(() => expect(screen.getByTestId('ai-cancelled')).toBeTruthy());
    expect(screen.queryByTestId('ai-error')).toBeNull();
    expect(screen.getByTestId('ai-cancelled').textContent).toMatch(/Nothing was saved/);
  });

  it('cancels the run it started when the panel goes away mid-check', async () => {
    // The engine runs ONE generation at a time, so an orphaned run holds the
    // model. Selecting another citation remounts this panel (it is keyed by
    // citation id), which must not leave that run holding the permit.
    const cancelGeneration = vi.fn(async () => {});
    const bridge = {
      ...base,
      cancelGeneration,
      citationSupport: () => new Promise(() => {}),
    };
    const { unmount } = render(
      <CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />,
    );
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-running')).toBeTruthy());
    unmount();
    expect(cancelGeneration).toHaveBeenCalledTimes(1);
  });

  it('does not cancel anything when the panel goes away with no run in flight', () => {
    const cancelGeneration = vi.fn(async () => {});
    const { unmount } = render(
      <CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={{ ...base, cancelGeneration } as any} />,
    );
    unmount();
    expect(cancelGeneration).not.toHaveBeenCalled();
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
