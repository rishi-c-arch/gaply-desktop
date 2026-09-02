// Gaply — Phase 9b screens: status/install, citation panel, thesis audit.
import React from 'react';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { AiStatusPanel, AiUnavailable } from './AiStatusPanel';
import { errorCode, errorText } from './aiBridge';
import { describeRun } from './CitationAiPanel';
import { CitationAiPanel } from './CitationAiPanel';
import {
  observedSecondsPerItem,
  projectDuration,
  SECONDS_PER_ITEM,
  ThesisAuditScreen,
} from './ThesisAuditScreen';
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

  it('projects from the run\u2019s OWN rate once it has one, and says which rate it used', () => {
    // The seed figure claimed to be "measured on this machine's CPU". It was
    // measured on a machine with room to spare; the same model on the same
    // laptop under memory pressure has run at a fifth of it. A stale number
    // wearing the word "measured" reads as a promise.
    expect(observedSecondsPerItem(0, 0)).toBeNull();
    expect(observedSecondsPerItem(1, 65_000)).toBeNull(); // one item is a sample, not a rate
    expect(observedSecondsPerItem(4, 1_200_000)).toBe(300);

    // …and the projection follows the measurement, not the constant.
    expect(projectDuration(10, 300)).toBe('50 minutes');
    expect(projectDuration(10, SECONDS_PER_ITEM)).toBe('11 minutes');
  });

  it('explains a slow run with measured numbers, not adjectives', () => {
    // A wall-clock figure alone cannot separate "this model is slow" from "this
    // machine had nothing left", and on 8 GB with a 2.4 GB model those look
    // identical — which is how a decode at a fifth of the bake-off rate turned
    // into a hunt for a mis-selected model.
    expect(
      describeRun({
        decodeTokensPerSec: 1.53,
        freeMemoryBytes: 0.42 * 1024 ** 3,
        totalMemoryBytes: 8 * 1024 ** 3,
      }),
    ).toBe('1.5 tok/s decode, 0.4 GB free of 8.0 GB at the end.');
    // Absent figures are omitted, never invented or zero-filled.
    expect(describeRun({ decodeTokensPerSec: 6.7 })).toBe('6.7 tok/s decode.');
    expect(describeRun({})).toBeNull();
    expect(describeRun({ decodeTokensPerSec: 0 })).toBeNull();
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
    expect(screen.getByTestId('ai-progress').textContent).not.toMatch(/undefined/);
  });

  it('never prints "undefined tokens" when the backend omits maxTokens', async () => {
    // Regression: the enum renamed its VARIANTS to camelCase but not its
    // struct-variant FIELDS, so `max_tokens` went out while the panel read
    // `maxTokens`. A missing field does not throw in JS — it renders, and it
    // rendered "552 of up to undefined tokens" for eight minutes.
    let emit: (ev: any) => void = () => {};
    const bridge = {
      ...base,
      citationSupport: (_c: string, _d: number, _s: string | undefined, onEvent: any) => {
        emit = onEvent;
        return new Promise(() => {});
      },
    };
    render(<CitationAiPanel sentence="A claim." documentId={1} aiInstalled bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('ai-check-support'));
    await waitFor(() => expect(screen.getByTestId('ai-running')).toBeTruthy());
    await act(async () => emit({ kind: 'decoding', tokens: 552 }));
    const text = screen.getByTestId('ai-progress').textContent ?? '';
    expect(text).not.toMatch(/undefined/);
    expect(text).toMatch(/552 tokens/);
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

    // The full dump is now BEHIND a disclosure: the report leads, and the
    // hundred sentences it was computed from do not bury it.
    await waitFor(() => expect(screen.getByTestId('audit-dump-toggle')).toBeTruthy());
    expect(screen.getByTestId('audit-dump-toggle').textContent).toMatch(/Show every sentence/);
    expect(screen.queryByTestId('audit-results')).toBeNull();
    fireEvent.click(screen.getByTestId('audit-dump-toggle'));

    expect(screen.getByTestId('audit-results')).toBeTruthy();
    expect(screen.getByTestId('audit-group-citation_need')).toBeTruthy();
    expect(screen.getByTestId('audit-group-unverifiable')).toBeTruthy();
    // page:null never becomes a fake number
    expect(screen.getByTestId('audit-item-1').textContent).toContain('page unknown');
    // unverifiable items carry their (deferred) affordances, clearly marked
    expect(screen.getByTestId('audit-link-1').textContent).toContain('coming soon');
  });

  it('leads with counts by category, not with a hundred sentences', async () => {
    // The first real audit rendered 100 paragraphs above the report. Every
    // number below was already being computed by `thesis_health`; the screen
    // simply showed none of them.
    const health = {
      jobId: 7,
      totalItems: 100,
      completedItems: 96,
      countsPerCategory: {
        citation_need: { done: 79, failed: 4 },
        unverifiable: { done: 17 },
      },
      verdictBreakdown: { 'need:needs_citation': 52, 'need:no_citation_needed': 27 },
      unverifiableReasons: { 'numeric citation style: no numbered bibliography was parsed': 15 },
      skippedReasons: { 'a table row or a figure caption': 5 },
      flagged: [{ sentence: 'An uncited empirical claim.', result: { output: { needs_citation: true } } }],
    };
    let emit: ((e: JobProgressEvent) => void) | undefined;
    const bridge = bridgeWith({
      startThesisAudit: async (_p: string, cb: any) => {
        emit = cb;
        return plan;
      },
      jobStatus: async () => ({ health }),
      jobResults: async () => ({ items: [] }),
    });
    render(<ThesisAuditScreen aiInstalled pickManuscript={async () => '/t.pdf'} bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('audit-pick'));
    await waitFor(() => expect(emit).toBeTruthy());
    emit!({ jobId: 7, completed: 100, total: 100, currentCategory: 'citation_need', latestItemSummary: '' });

    await waitFor(() => expect(screen.getByTestId('audit-health')).toBeTruthy());
    // Counts by category, with the failures named rather than folded in.
    expect(screen.getByTestId('audit-count-citation_need').textContent).toMatch(/83 citation need/);
    expect(screen.getByTestId('audit-count-citation_need').textContent).toMatch(/4 could not be judged/);
    expect(screen.getByTestId('audit-count-unverifiable').textContent).toMatch(/17 unverifiable/);
    // What the model actually SAID, which is the number that matters and is
    // not the same as the number queued.
    expect(screen.getByTestId('audit-verdicts').textContent).toMatch(/52 needs citation/);
    expect(screen.getByTestId('audit-verdicts').textContent).toMatch(/27 no citation needed/);
    // Why the unverifiable ones could not be checked.
    expect(screen.getByTestId('audit-unverifiable-reasons').textContent).toMatch(
      /15 numeric citation style/,
    );
    // And what was never judged at all, kept out of every tally.
    expect(screen.getByTestId('audit-skipped').textContent).toMatch(/5 a table row/);
  });

  it('an unverifiable item offers BOTH ways to supply the source', async () => {
    // The report was a dead end: it named what it could not check and stopped.
    // Every one of these items is blocked on the same thing — the cited work is
    // not in the library — and both remedies belong beside the item.
    let emit: ((e: JobProgressEvent) => void) | undefined;
    const bridge = bridgeWith({
      startThesisAudit: async (_p: string, cb: any) => { emit = cb; return plan; },
      jobStatus: async () => ({ health: { completedItems: 1, flagged: [] } }),
      jobResults: async () => ({
        items: [
          {
            seq: 4,
            kind: 'unverifiable',
            page: 1,
            sentence: 'Lexicons help [5].',
            status: 'done',
            reason: '[5] S. Mohammad — not in your library',
            libraryId: 'lib-5',
          },
        ],
      }),
    });
    render(<ThesisAuditScreen aiInstalled pickManuscript={async () => '/t.pdf'} bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('audit-pick'));
    await waitFor(() => expect(emit).toBeTruthy());
    emit!({ jobId: 7, completed: 1, total: 1, currentCategory: 'unverifiable', latestItemSummary: '' });

    await waitFor(() => expect(screen.getByTestId('audit-blocked-4')).toBeTruthy());
    expect(screen.getByTestId('audit-blocked-4').textContent).toMatch(/not in your library/);
    expect(screen.getByTestId('audit-fetch-4')).toBeTruthy();
    expect(screen.getByTestId('audit-attach-4')).toBeTruthy();
    // And the header offers the batch over the distinct sources.
    expect(screen.getByTestId('audit-fetch-all').textContent).toMatch(/these 1 sources/);
  });

  it('offers a re-check ONLY for what actually became checkable', async () => {
    // A fetch that succeeded and an item that can now be checked are different
    // facts: an abstract-only fetch "works" and still leaves nothing to check
    // against. The offer follows `checkable`, not success.
    let emit: ((e: JobProgressEvent) => void) | undefined;
    const recheckItems = vi.fn(
      async (_jobId: number, _ids: string[], _onEvent?: unknown) => ({
        requeued: 1,
        items: [{ seq: 4, documentId: 8 }],
      }),
    );
    const bridge = bridgeWith({
      startThesisAudit: async (_p: string, cb: any) => { emit = cb; return plan; },
      jobStatus: async () => ({ health: { completedItems: 1, flagged: [] } }),
      jobResults: async () => ({
        items: [
          { seq: 4, kind: 'unverifiable', page: 1, sentence: 'A [5].', status: 'done',
            reason: 'not in your library', libraryId: 'lib-5' },
          { seq: 6, kind: 'unverifiable', page: 1, sentence: 'B [6].', status: 'done',
            reason: 'not in your library', libraryId: 'lib-6' },
        ],
      }),
      fetchOpenAccess: async () => [
        { citationId: 'lib-5', title: 'A', outcome: 'fetched', checkable: true, chunksIndexed: 20, chunksEmbedded: 20 },
        // Fetched an abstract: real, useful, and NOT checkable-for-support.
        { citationId: 'lib-6', title: 'B', outcome: 'abstractOnly', checkable: false },
      ],
      recheckItems,
    });
    render(<ThesisAuditScreen aiInstalled pickManuscript={async () => '/t.pdf'} bridge={bridge as any} />);
    fireEvent.click(screen.getByTestId('audit-pick'));
    await waitFor(() => expect(emit).toBeTruthy());
    emit!({ jobId: 7, completed: 2, total: 2, currentCategory: 'unverifiable', latestItemSummary: '' });

    await waitFor(() => expect(screen.getByTestId('audit-fetch-all')).toBeTruthy());
    // No offer before anything has been fetched.
    expect(screen.queryByTestId('audit-recheck')).toBeNull();
    fireEvent.click(screen.getByTestId('audit-fetch-all'));

    await waitFor(() => expect(screen.getByTestId('audit-recheck')).toBeTruthy());
    // ONE item became checkable, not two.
    expect(screen.getByTestId('audit-recheck').textContent).toMatch(/the 1 item that became checkable/);
    fireEvent.click(screen.getByTestId('audit-recheck'));
    await waitFor(() => expect(recheckItems).toHaveBeenCalled());
    // Only the citation that gained a readable source is re-queued.
    expect(recheckItems.mock.calls[0][1]).toEqual(['lib-5']);
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
