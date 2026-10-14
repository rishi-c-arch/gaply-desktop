// Level 1, "Check a paper against its data" (Phase 2, Step 7): the screens
// over a fake bridge that plays Rust's part: it keeps each preview's body and
// "sends" exactly the stored body for a handle, as the session does.
import React from 'react';
import { MemoryRouter } from 'react-router-dom';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { GaplySessionProvider } from '../session/SessionProvider';
import type { AuthService } from '../../services/supabase';
import StatsVerifierPage from './StatsVerifierPage';
import L1Flow from './l1/L1Flow';
import { makeMockStatsVerifierBridge } from './statsVerifierBridge';
import type { StatsL1Bridge } from './statsL1Bridge';
import type {
  AiSettings,
  BindingEdits,
  HelperTaskInput,
  L1ConfirmView,
  L1HelperResultView,
  L1OpenView,
  L1PreviewView,
  L1ProposalView,
  OptInCategory,
  ReproductionView,
} from './statsL1Types';

const flags = vi.hoisted(() => ({ statsL1: true } as Record<string, boolean>));
vi.mock('../../config/featureFlags', () => ({ isFeatureEnabled: (n: string) => flags[n] === true }));
vi.mock('../../design-system/GaplyGlobe', () => ({ GaplyGlobe: () => <div /> }));

afterEach(() => {
  cleanup();
  flags.statsL1 = true;
});

const XSS = '<img src=x onerror=alert(1)>';
const MD = '[click here](https://evil.example/x)';

/* ------------------------------------------------------------------------ *
 *  Fixtures
 * ------------------------------------------------------------------------ */

const SENTENCE_0 = 'Anxiety was lower after treatment, t(38) = 2.10, p = .042.';
const SENTENCE_1 = 'Age correlated with anxiety, r(38) = .31, p = .05.';

function openView(): L1OpenView {
  return {
    run: 7,
    claims: [
      {
        claimId: 0, revision: 'rev0', sectionIndex: 2, section: 'results', paragraph: 1, sentenceIndex: 0, sentence: SENTENCE_0,
        family: 't', statistic: '2.10', df: ['38'], n: null, p: '= .042', level0: { code: 'consistent' },
      },
      {
        claimId: 1, revision: 'rev1', sectionIndex: 2, section: 'results', paragraph: 1, sentenceIndex: 1, sentence: SENTENCE_1,
        family: 'r', statistic: '.31', df: ['38'], n: null, p: '= .05', level0: { code: 'concern', reason: 'one_tailed_unstated' },
      },
    ],
    notLocated: 0,
    schema: {
      rows: 40,
      columns: [
        { alias: 'C1', header: 'anxiety_post', columnType: 'numeric', hasMissing: true, levelBucket: null, levels: [], issues: [] },
        {
          alias: 'C2', header: 'condition', columnType: 'categorical', hasMissing: false, levelBucket: 'two',
          levels: [{ code: '1', label: 'control', count: 20 }, { code: '2', label: 'treatment', count: 20 }], issues: [],
        },
        { alias: 'C3', header: 'age', columnType: 'integer', hasMissing: false, levelBucket: null, levels: [], issues: [] },
      ],
      issues: [],
    },
    proposals: [
      {
        proposal: { run: 7, id: 1 }, claimId: 0, method: 'independent_t',
        roles: [{ role: 'outcome', aliases: ['C1'] }, { role: 'group', aliases: ['C2'] }], levels: [],
        variant: 'not_stated', variantSetBy: { code: 'not_stated' }, source: 'local', reason: 'header_matches_sentence',
      },
      {
        proposal: { run: 7, id: 2 }, claimId: 1, method: 'pearson',
        roles: [{ role: 'x', aliases: ['C3'] }, { role: 'y', aliases: ['C1'] }], levels: [],
        variant: 'none', variantSetBy: { code: 'not_stated' }, source: 'local', reason: 'first_eligible_columns',
      },
    ],
    catalogue: [
      { method: 'independent_t', slots: [{ role: 'outcome', kind: 'numeric', levels: null, min: null }, { role: 'group', kind: 'group', levels: { code: 'exactly', levels: 2 }, min: null }], variants: ['student', 'welch'], needsTarget: false },
      { method: 'pearson', slots: [{ role: 'x', kind: 'numeric', levels: null, min: null }, { role: 'y', kind: 'numeric', levels: null, min: null }], variants: [], needsTarget: false },
    ],
  };
}

function reproduction(tokens: string[]): ReproductionView {
  const claims: ReproductionView['claims'] = [];
  if (tokens.some((t) => t.endsWith(':0'))) {
    claims.push({
      claimId: 0, sectionIndex: 2, paragraph: 1, sentence: 0, method: 'independent_t',
      variant: { code: 'not_stated' }, variantSetBy: { code: 'not_stated' },
      verdict: { kind: 'reproduced', notes: [{ code: 'variant_matched', variant: 'student_pooled' }] },
      notes: [],
      candidates: [{ variant: { code: 'student_pooled' }, numbers: [
        { kind: { code: 'statistic' }, printed: '2.10', relation: '=', computed: 2.1034, acceptedLo: 2.095, acceptedHi: 2.105, status: { code: 'matched' } },
        { kind: { code: 'p' }, printed: '.042', relation: '=', computed: 0.04221, acceptedLo: 0.0415, acceptedHi: 0.0425, status: { code: 'matched' } },
      ] }],
      nUsed: 36, rowsRead: 40, exclusions: [{ reason: { code: 'missing_in_bound_column' }, rows: 4 }],
      provenance: { methodId: 'independent_t', reference: null, methodVersion: '1', engineVersion: 'stats_l1 engine 1', inputsDigest: 'ab'.repeat(32), bindingSource: { code: 'local', reason: 'header_matches_sentence' }, variantSetBy: { code: 'not_stated' } },
    });
  }
  if (tokens.some((t) => t.endsWith(':1'))) {
    claims.push({
      claimId: 1, sectionIndex: 2, paragraph: 1, sentence: 1, method: 'pearson', variant: { code: 'none' }, variantSetBy: { code: 'not_stated' },
      verdict: { kind: 'couldNotCheck', reason: { code: 'refused', refusal: 'zero_variance' } },
      notes: [], candidates: [], nUsed: null, rowsRead: null, exclusions: [], provenance: null,
    });
  }
  return {
    counts: { reproduced: claims.filter((c) => c.verdict.kind === 'reproduced').length, notReproduced: 0, couldNotCheck: claims.filter((c) => c.verdict.kind === 'couldNotCheck').length },
    data: { code: 'read' }, unmatchedPairs: 0, claims,
  };
}

/* ------------------------------------------------------------------------ *
 *  The fake bridge
 * ------------------------------------------------------------------------ */

interface Fake extends StatsL1Bridge {
  calls: Array<[string, unknown]>;
  /** The bodies "sent", in order, with their task. */
  sent: Array<{ task: string; body: string }>;
  names(): string[];
}

function fakeBridge(opts: { settings?: Partial<AiSettings>; replies?: Partial<Record<HelperTaskInput['task'], L1HelperResultView>>; extraProposals?: L1ProposalView[] } = {}): Fake {
  const calls: Array<[string, unknown]> = [];
  const sent: Array<{ task: string; body: string }> = [];
  const previews = new Map<number, L1PreviewView>();
  const v = openView();
  let proposals = [...v.proposals];
  const claimOf = new Map<number, number>(proposals.map((p) => [p.proposal.id, p.claimId]));
  let next = 100;
  let settings: AiSettings = { aiHelp: false, webContext: false, model: 'gpt-5.5', ...opts.settings };
  const rec = (n: string, a: unknown = {}) => calls.push([n, a]);
  const fake: Fake = {
    calls,
    sent,
    names: () => calls.map(([n]) => n),
    open: async (manuscript, data) => { rec('open', { manuscript, data }); return v; },
    editClaim: async (run, claimId, claim) => {
      rec('editClaim', { run, claimId, claim });
      proposals = proposals.filter((p) => p.claimId !== claimId);
      return { claimId, revision: `rev${claimId}-edited` };
    },
    addClaim: async (run, claim) => { rec('addClaim', { run, claim }); return { claimId: 9, revision: 'rev9' }; },
    proposals: async (run) => { rec('proposals', { run }); return [...proposals, ...(opts.extraProposals ?? [])]; },
    helperPreview: async (run, input: HelperTaskInput, categories: OptInCategory[]) => {
      rec('helperPreview', { run, input, categories });
      const id = next++;
      const body = JSON.stringify({ model: settings.model, store: false, input: JSON.stringify({ ...input, optIns: categories, query: input.task === 'web_context' ? (input.editedQuery ?? 'welch t test assumptions') : undefined }) });
      const p: L1PreviewView = { preview: { run, id }, task: input.task, body, sha256: 'f'.repeat(64), categories: [`Base fields for ${input.task}`, ...categories.map((c) => `Ticked ${c}`)] };
      previews.set(id, p);
      return p;
    },
    helperSend: async (run, preview) => {
      rec('helperSend', { run, preview });
      const p = previews.get(preview.id)!;
      sent.push({ task: p.task, body: p.body });
      const r = opts.replies?.[p.task as HelperTaskInput['task']];
      if (!r) throw new Error('{"code":"no_key"}');
      return r;
    },
    helperRevoke: async (run, task) => { rec('helperRevoke', { run, task }); return true; },
    helperCancel: async (run) => { rec('helperCancel', { run }); },
    draftProposal: async (run, claimId, from, edits: BindingEdits) => {
      rec('draftProposal', { run, claimId, from, edits });
      const id = next++;
      claimOf.set(id, claimId);
      return {
        proposal: { run, id }, claimId, method: edits.methodId,
        roles: edits.roles.map((r) => ({ role: r.role, aliases: r.aliases.map((a) => `C${a}`) })), levels: edits.levels,
        variant: edits.variantTicked ?? 'not_stated', variantSetBy: { code: edits.variantTicked ? 'user_ticked' : 'not_stated' },
        source: 'manual', reason: null,
      };
    },
    confirmBinding: async (run, proposal) => {
      rec('confirmBinding', { run, proposal });
      const claimId = claimOf.get(proposal.id) ?? -1;
      const out: L1ConfirmView = { token: `tok${proposal.id}:${claimId}`, claimId, rowsRead: 40, rowsFilteredOut: 0, rowsExcludedMissing: 4, nUsed: 36, suspectMissingCodes: [] };
      return out;
    },
    compute: async (run, tokens) => { rec('compute', { run, tokens }); return reproduction(tokens); },
    close: async (run) => { rec('close', { run }); return true; },
    usage: async (run) => {
      rec('usage', { run });
      const by = new Map<string, number>();
      for (const s of sent) by.set(s.task, (by.get(s.task) ?? 0) + 1);
      return { tasks: Array.from(by.entries()).map(([task, requests]) => ({ task, requests, inputTokens: 300 * requests, outputTokens: 40 * requests })) };
    },
    settingGet: async () => { rec('settingGet'); return settings; },
    settingSet: async (s) => { rec('settingSet', s); settings = s; return s; },
    keyStatus: async () => { rec('keyStatus'); return { saved: true, provider: 'openai' }; },
    keySave: async () => { rec('keySave', { key: '<redacted>' }); return { saved: true, provider: 'openai' }; },
    keyTest: async () => { rec('keyTest'); return { result: 'accepted' }; },
    keyRemove: async () => { rec('keyRemove'); return { saved: false, provider: 'openai' }; },
    openSourceLink: async (url) => { rec('openSourceLink', { url }); },
  };
  return fake;
}

/* ------------------------------------------------------------------------ *
 *  Driving the flow
 * ------------------------------------------------------------------------ */

async function openBoth(b: Fake) {
  const r = render(<L1Flow bridge={b} />);
  await waitFor(() => expect(b.names()).toContain('settingGet'));
  fireEvent.change(screen.getByTestId('l1-manuscript-file'), { target: { files: [new File(['x'], 'paper.docx')] } });
  fireEvent.change(screen.getByTestId('l1-data-file'), { target: { files: [new File(['x'], 'trial.csv')] } });
  fireEvent.click(screen.getByTestId('l1-open'));
  await screen.findByTestId('l1-claims');
  return r;
}

async function toBind(b: Fake) {
  const r = await openBoth(b);
  fireEvent.click(screen.getByTestId('l1-to-bind'));
  await screen.findByTestId('l1-bind-0');
  return r;
}

async function confirmAll() {
  fireEvent.click(screen.getByTestId('l1-to-confirm'));
  await screen.findByTestId('l1-confirm-0');
  fireEvent.click(screen.getByTestId('l1-confirm-btn-0'));
  await screen.findByTestId('l1-confirmed-0');
  fireEvent.click(screen.getByTestId('l1-confirm-btn-1'));
  await screen.findByTestId('l1-confirmed-1');
}

async function toResults(b: Fake) {
  const r = await toBind(b);
  await confirmAll();
  fireEvent.click(screen.getByTestId('l1-run'));
  await screen.findByTestId('l1-results');
  return r;
}

const preText = (panel: HTMLElement) => within(panel).getByTestId('l1-ai-request').textContent;

/* ------------------------------------------------------------------------ *
 *  Tests
 * ------------------------------------------------------------------------ */

describe('Level 1: the add screen', () => {
  it('shows both file names, the claim count and the column count, and makes no helper call on open', async () => {
    const b = fakeBridge({ settings: { aiHelp: true } });
    await openBoth(b);
    const summary = screen.getByTestId('l1-files-summary').textContent ?? '';
    expect(summary).toContain('paper.docx');
    expect(summary).toContain('trial.csv');
    expect(summary).toMatch(/2 claims read, 3 columns/);
    expect(b.names().filter((n) => n.startsWith('helper'))).toEqual([]);
    expect(b.calls.find(([n]) => n === 'open')?.[1]).toEqual({ manuscript: 'paper.docx', data: 'trial.csv' });
  });

  it('review shows the sentence, the parsed values and the Level 0 verdict', async () => {
    const b = fakeBridge();
    await openBoth(b);
    const row = screen.getByTestId('l1-claim-0');
    expect(row.textContent).toContain(SENTENCE_0);
    expect(row.textContent).toContain('t(38) = 2.10, p = .042');
    expect(screen.getByTestId('l1-claim-0-level0').textContent).toBe('Level 0: consistent');
    expect(screen.getByTestId('l1-claim-1-level0').textContent).toMatch(/concern\. These numbers agree only if the test was one-tailed/);
  });
});

describe('Level 1: AI help off', () => {
  it('shows no AI button anywhere, and the manual path completes', async () => {
    const b = fakeBridge({ settings: { aiHelp: false } });
    await toBind(b);
    expect(screen.queryByText('Suggest method with AI')).toBeNull();
    expect(screen.queryByText('Suggest columns with AI')).toBeNull();
    // A by-hand change: the variant is ticked, so the binding is drafted by Rust.
    fireEvent.change(screen.getByTestId('l1-bind-0-variant-pick'), { target: { value: 'welch' } });
    fireEvent.click(screen.getByTestId('l1-bind-0-variant-tick'));
    await confirmAll();
    expect(screen.getByTestId('l1-summary-0').textContent).toMatch(
      /^Welch's t-test\. Outcome: C1 anxiety_post\. Groups: C2 condition, levels control and treatment\. 4 rows excluded for missing values\./
    );
    fireEvent.click(screen.getByTestId('l1-run'));
    await screen.findByTestId('l1-results');
    expect(screen.queryByText('Explain this result')).toBeNull();
    expect(screen.queryByText('Method context (web)')).toBeNull();
    expect(b.names().filter((n) => n.startsWith('helper'))).toEqual([]);
    expect(b.calls.find(([n]) => n === 'draftProposal')?.[1]).toMatchObject({ claimId: 0, edits: { variantTicked: 'welch' } });
  });
});

describe('Level 1: confirmation', () => {
  it('Run stays disabled until every included claim is confirmed or skipped', async () => {
    const b = fakeBridge();
    await toBind(b);
    fireEvent.click(screen.getByTestId('l1-to-confirm'));
    await screen.findByTestId('l1-confirm-0');
    const runBtn = screen.getByTestId('l1-run') as HTMLButtonElement;
    expect(runBtn.disabled).toBe(true);
    fireEvent.click(screen.getByTestId('l1-confirm-btn-0'));
    await screen.findByTestId('l1-confirmed-0');
    expect(runBtn.disabled).toBe(true);
    // Skipping the other claim is the other way to unblock.
    fireEvent.click(screen.getByTestId('l1-back-bind'));
    fireEvent.click(await screen.findByTestId('l1-back-claims'));
    fireEvent.click(await screen.findByTestId('l1-claim-1-skip'));
    fireEvent.click(screen.getByTestId('l1-to-bind'));
    fireEvent.click(await screen.findByTestId('l1-to-confirm'));
    await screen.findByTestId('l1-confirmed-0');
    expect((screen.getByTestId('l1-run') as HTMLButtonElement).disabled).toBe(false);
    expect(b.names()).not.toContain('compute');
  });

  it('an edit to a claim invalidates its confirmation', async () => {
    const b = fakeBridge();
    await toBind(b);
    await confirmAll();
    expect((screen.getByTestId('l1-run') as HTMLButtonElement).disabled).toBe(false);
    fireEvent.click(screen.getByTestId('l1-back-bind'));
    fireEvent.click(await screen.findByTestId('l1-back-claims'));
    fireEvent.click(await screen.findByTestId('l1-claim-0-edit'));
    fireEvent.change(screen.getByTestId('l1-edit-0-stat'), { target: { value: '2.30' } });
    fireEvent.click(screen.getByTestId('l1-edit-0-save'));
    expect(await screen.findByTestId('l1-claim-0-reconfirm')).toBeTruthy();
    expect(screen.getByTestId('l1-claim-0-level0').textContent).toMatch(/not run on an edited/);
    fireEvent.click(screen.getByTestId('l1-to-bind'));
    expect((await screen.findByTestId('l1-bind-0-source')).textContent).toMatch(/needs confirming again/);
    fireEvent.click(screen.getByTestId('l1-to-confirm'));
    await screen.findByTestId('l1-confirm-btn-0');
    expect(screen.queryByTestId('l1-confirmed-0')).toBeNull();
    expect(screen.getByTestId('l1-confirmed-1')).toBeTruthy();
    expect((screen.getByTestId('l1-run') as HTMLButtonElement).disabled).toBe(true);
    // The edited claim goes back through a fresh draft, from nothing.
    const draft = b.calls.filter(([n]) => n === 'draftProposal').pop()?.[1] as { claimId: number; from: unknown };
    expect(draft).toMatchObject({ claimId: 0, from: null });
  });

  it('every proposal is marked as a suggestion with its source', async () => {
    const b = fakeBridge();
    await toBind(b);
    expect(screen.getByTestId('l1-bind-0-source').textContent).toMatch(/Suggestion, not confirmed.*Source: Local/);
    fireEvent.change(screen.getByTestId('l1-bind-0-outcome'), { target: { value: '3' } });
    expect(screen.getByTestId('l1-bind-0-source').textContent).toMatch(/Source: Local, changed by you/);
  });
});

describe('Level 1: the AI panels', () => {
  const methodReply: L1HelperResultView = {
    kind: 'claimMethod', proposal: { run: 7, id: 50 }, method: 'independent_t', variantHint: 'welch', modelReason: 'The sentence compares two groups.', reliedOn: ['statistic'],
  };
  const aiProposal: L1ProposalView = {
    proposal: { run: 7, id: 50 }, claimId: 0, method: 'independent_t', roles: [], levels: [],
    variant: 'not_stated', variantSetBy: { code: 'not_stated' }, source: 'ai_suggestion', reason: null,
  };
  const columnsReply: L1HelperResultView = {
    kind: 'bindingSuggestion', proposal: { run: 7, id: 51 }, roles: [{ role: 'outcome', alias: 'C1', levelsToSelect: null }], modelReason: 'C1 is numeric.', reliedOn: [],
  };

  it('the preview text is exactly what is sent, and Send is the only thing that sends', async () => {
    const b = fakeBridge({ settings: { aiHelp: true }, replies: { claim_method: methodReply }, extraProposals: [aiProposal] });
    await toBind(b);
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-method'));
    const panel = await screen.findByTestId('l1-ai-panel-claim_method');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    expect(b.names().filter((n) => n === 'helperSend')).toEqual([]);
    const shown = preText(panel);
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    await waitFor(() => expect(b.sent.length).toBe(1));
    expect(b.sent[0]).toEqual({ task: 'claim_method', body: shown });
    // The panel names what is sent and what is not, who gets it and what they keep.
    expect(within(panel).getByTestId('l1-ai-sent').textContent).toContain('Base fields for claim_method');
    expect(within(panel).getByTestId('l1-ai-not-sent').textContent).toBe("Not sent: your data, column names, file names, level names, your manuscript's text.");
    expect(within(panel).getByTestId('l1-ai-recipient').textContent).toMatch(/OpenAI/);
    expect(within(panel).getByTestId('l1-ai-retention').textContent).toMatch(/store to false.*does not mean nothing is kept.*30 days/);
    expect(await within(panel).findByTestId('l1-ai-withdraw-text')).toHaveProperty(
      'textContent',
      'Each Send is your consent for this task and these categories for this check. Withdrawing ends that consent for the rest of this check and stops any call or retry in flight.'
    );
    // Withdraw revokes the grant and cancels anything in flight.
    fireEvent.click(within(panel).getByTestId('l1-ai-withdraw'));
    await waitFor(() => expect(b.names()).toContain('helperCancel'));
    expect(b.calls.find(([n]) => n === 'helperRevoke')?.[1]).toEqual({ run: 7, task: 'claim_method' });
  });

  it('Cancel sends nothing', async () => {
    const b = fakeBridge({ settings: { aiHelp: true }, replies: { binding_suggestion: columnsReply } });
    await toBind(b);
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-columns'));
    const panel = await screen.findByTestId('l1-ai-panel-binding_suggestion');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-cancel'));
    expect(screen.queryByTestId('l1-ai-panel-binding_suggestion')).toBeNull();
    await act(async () => undefined);
    expect(b.sent).toEqual([]);
    expect(b.names().filter((n) => n === 'helperSend' || n === 'helperRevoke')).toEqual([]);
    // Focus returns to the button that opened the panel.
    expect(document.activeElement).toBe(screen.getByTestId('l1-bind-0-ai-columns'));
  });

  it('a tick rebuilds the preview with the opt-in, and the two suggestion consents are separate', async () => {
    const b = fakeBridge({ settings: { aiHelp: true }, replies: { claim_method: methodReply, binding_suggestion: columnsReply }, extraProposals: [aiProposal] });
    await toBind(b);
    // Method: consented by its own Send.
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-method'));
    let panel = await screen.findByTestId('l1-ai-panel-claim_method');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    await waitFor(() => expect(b.sent.length).toBe(1));
    fireEvent.click(await within(panel).findByTestId('l1-ai-close'));

    // Columns: a new panel, which does not inherit the method's consent.
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-columns'));
    panel = await screen.findByTestId('l1-ai-panel-binding_suggestion');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    expect(within(panel).getByTestId('l1-ai-send')).toBeTruthy();
    const before = preText(panel);
    expect(before).not.toContain('column_headers');
    const box = within(panel).getByTestId('l1-optin-column_headers') as HTMLInputElement;
    expect(box.checked).toBe(false);
    fireEvent.click(box);
    await waitFor(() => expect(preText(panel)).toContain('column_headers'));
    expect(b.calls.filter(([n]) => n === 'helperPreview').pop()?.[1]).toMatchObject({ categories: ['column_headers'] });
    expect(within(panel).getByTestId('l1-ai-not-sent').textContent).not.toContain('column names');
    expect(b.sent.length).toBe(1);
    const shown = preText(panel);
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    await waitFor(() => expect(b.sent.length).toBe(2));
    expect(b.sent.map((s) => s.task)).toEqual(['claim_method', 'binding_suggestion']);
    expect(b.sent[1].body).toBe(shown);
    // Two Sends, two previews: one consent each.
    const handles = b.calls.filter(([n]) => n === 'helperSend').map(([, a]) => (a as { preview: { id: number } }).preview.id);
    expect(new Set(handles).size).toBe(2);
  });

  it('an AI-suggested variant shows as not stated until the user ticks the box', async () => {
    const b = fakeBridge({ settings: { aiHelp: true }, replies: { claim_method: methodReply }, extraProposals: [aiProposal] });
    await toBind(b);
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-method'));
    const panel = await screen.findByTestId('l1-ai-panel-claim_method');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    await waitFor(() => expect(screen.getByTestId('l1-bind-0-source').textContent).toMatch(/Source: AI/));
    expect(screen.getByTestId('l1-bind-0-variant-status').textContent).toBe('not stated');
    expect(screen.getByTestId('l1-bind-0-variant').textContent).toMatch(/AI suggests Welch/);
    fireEvent.click(screen.getByTestId('l1-bind-0-variant-tick'));
    expect(screen.getByTestId('l1-bind-0-variant-status').textContent).toMatch(/you ticked that the paper states this/);
  });

  it('the usage line counts every request that left the device', async () => {
    const b = fakeBridge({ settings: { aiHelp: true }, replies: { claim_method: methodReply }, extraProposals: [aiProposal] });
    await toBind(b);
    expect(screen.getByTestId('l1-usage').textContent).toBe('No request has left this device in this check.');
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-method'));
    const panel = await screen.findByTestId('l1-ai-panel-claim_method');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    await waitFor(() => expect(screen.getByTestId('l1-usage').textContent).toMatch(/Suggest method with AI: 1 request, 300 tokens in and 40 out/));
  });
});

describe('Level 1: model and web text is plain text', () => {
  const hostile = `${XSS} ${MD}`;
  it('every model and web field renders literally, creating no element and no link', async () => {
    const b = fakeBridge({
      settings: { aiHelp: true, webContext: true },
      replies: {
        claim_method: { kind: 'claimMethod', proposal: { run: 7, id: 50 }, method: 'independent_t', variantHint: null, modelReason: hostile, reliedOn: [] },
        result_explanation: { kind: 'explanation', modelText: hostile },
        web_context: {
          kind: 'context',
          items: [{ sentence: hostile, sources: [{ url: 'https://www.bmj.com/a', title: hostile, domain: 'bmj.com' }], retrievedAt: 1792000000, querySent: 'q' }],
          uncitedSentences: 0, droppedCitations: [], emptyMessage: null,
        },
      },
      extraProposals: [{ proposal: { run: 7, id: 50 }, claimId: 0, method: 'independent_t', roles: [], levels: [], variant: 'not_stated', variantSetBy: { code: 'not_stated' }, source: 'ai_suggestion', reason: null }],
    });
    const { container } = await toBind(b);
    fireEvent.click(screen.getByTestId('l1-bind-0-ai-method'));
    let panel = await screen.findByTestId('l1-ai-panel-claim_method');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    expect((await screen.findByTestId('l1-bind-0-ai-reason')).textContent).toBe(`AI's reason, as written: ${hostile}`);
    fireEvent.click(within(panel).getByTestId('l1-ai-close'));

    // Bind the AI proposal's claim by hand and run.
    fireEvent.change(screen.getByTestId('l1-bind-0-outcome'), { target: { value: '1' } });
    fireEvent.change(screen.getByTestId('l1-bind-0-group'), { target: { value: '2' } });
    await confirmAll();
    fireEvent.click(screen.getByTestId('l1-run'));
    await screen.findByTestId('l1-results');

    fireEvent.click(screen.getByTestId('l1-result-0-explain'));
    panel = await screen.findByTestId('l1-ai-panel-result_explanation');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    expect((await screen.findByTestId('l1-result-0-explanation')).textContent).toContain(hostile);

    fireEvent.click(screen.getByTestId('l1-result-0-context-open'));
    panel = await screen.findByTestId('l1-ai-panel-web_context');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    const ctx = await screen.findByTestId('l1-result-0-context');
    expect(within(ctx).getByTestId('l1-result-0-context-item').textContent).toContain(hostile);
    expect(within(ctx).getByTestId('l1-result-0-source').textContent).toBe(`${hostile} (bmj.com)`);

    // Nothing parsed: no image, no link, anywhere in the flow.
    expect(container.querySelector('img')).toBeNull();
    expect(container.querySelectorAll('a').length).toBe(0);
    expect(document.querySelectorAll('a').length).toBe(0);
  });
});

describe('Level 1: results', () => {
  it('each printed number is beside the computed one; Could-not-check is neutral with its reason', async () => {
    const b = fakeBridge();
    await toResults(b);
    expect(screen.getByTestId('l1-results-counts').textContent).toMatch(/^1 reproduced, 0 not reproduced, 1 could not be checked\./);
    const card = screen.getByTestId('l1-result-0');
    expect(within(card).getByTestId('l1-result-0-verdict').textContent).toBe('Reproduced');
    const rows = within(card).getAllByRole('row');
    expect(rows[1].textContent).toMatch(/the test statistic.*2\.10.*2\.1034.*Matches\./);
    expect(within(card).getByTestId('l1-result-0-n').textContent).toMatch(/n used: 36 of 40 rows read\. 4 rows left out: a missing value in a chosen column\./);
    expect(within(card).getByTestId('l1-result-0-notes').textContent).toMatch(/Matched as Student's t-test \(pooled variance\)\./);
    expect(within(card).getByTestId('l1-result-0-provenance').textContent).toMatch(/Engine: stats_l1 engine 1/);
    const cnc = screen.getByTestId('l1-result-1');
    expect(within(cnc).getByTestId('l1-result-1-verdict').className).toContain('gds-badge--neutral');
    expect(within(cnc).getByTestId('l1-result-1-reason').textContent).toBe('A chosen column does not vary, so the statistic is undefined.');
    // No score, no percentage, no gauge.
    const text = screen.getByTestId('l1-results').textContent ?? '';
    expect(text).not.toMatch(/%|score|percent/i);
    expect(screen.queryByRole('meter')).toBeNull();
  });

  it('Not reproduced never says the paper is wrong, and lists the usual causes', async () => {
    const { L1ResultCard } = await import('./l1/L1ResultCard');
    const b = fakeBridge();
    render(
      <L1ResultCard
        bridge={b}
        run={7}
        claim={openView().claims[0]}
        result={{ ...reproduction(['t:0']).claims[0], verdict: { kind: 'notReproduced', conclusionChanges: true } }}
        ctx={{ column: (a) => `C${a}` }}
        aiHelp={false}
        webContext={false}
        onUsage={() => undefined}
      />
    );
    const t = screen.getByTestId('l1-result-0-not-reproduced').textContent ?? '';
    expect(t).toMatch(/data under this binding gives a different value/);
    expect(t).toMatch(/different subset of rows, a different version of the data, a different method, or a typo/);
    expect((screen.getByTestId('l1-result-0').textContent ?? '').toLowerCase()).not.toMatch(/wrong|incorrect|error/);
    expect(screen.getByTestId('l1-result-0-verdict').className).toContain('gds-badge--assessed');
  });

  it('the explanation sits beside the result and cannot replace the engine numbers', async () => {
    const b = fakeBridge({ settings: { aiHelp: true }, replies: { result_explanation: { kind: 'explanation', modelText: 'It was reproduced with t = 99.99 and p = .5.' } } });
    await toResults(b);
    const before = screen.getAllByTestId('l1-result-0-computed').map((e) => e.textContent);
    fireEvent.click(screen.getByTestId('l1-result-0-explain'));
    const panel = await screen.findByTestId('l1-ai-panel-result_explanation');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    const ex = await screen.findByTestId('l1-result-0-explanation');
    expect(ex.textContent).toMatch(/^AI explanation/);
    expect(screen.getAllByTestId('l1-result-0-computed').map((e) => e.textContent)).toEqual(before);
    expect(before.join(' ')).not.toContain('99.99');
    const table = within(screen.getByTestId('l1-result-0')).getAllByTestId('l1-result-0-numbers')[0];
    expect(table.textContent).not.toContain('99.99');
    expect(screen.getByTestId('l1-result-0').querySelectorAll('a').length).toBe(0);
  });

  it('web sources show their retrieval time and open only through the bridge; an empty result says so', async () => {
    const ctxReply: L1HelperResultView = {
      kind: 'context',
      items: [{ sentence: "Welch's test does not assume equal variances.", sources: [{ url: 'https://www.bmj.com/content/1', title: 'Statistics notes', domain: 'bmj.com' }], retrievedAt: 1792000000, querySent: 'welch t test assumptions' }],
      uncitedSentences: 1, droppedCitations: [], emptyMessage: null,
    };
    const b = fakeBridge({ settings: { aiHelp: true, webContext: true }, replies: { web_context: ctxReply } });
    await toResults(b);
    fireEvent.click(screen.getByTestId('l1-result-0-context-open'));
    const panel = await screen.findByTestId('l1-ai-panel-web_context');
    // The query field shows the query the request carries, and an edit
    // rebuilds the preview with it.
    await waitFor(() => expect((screen.getByTestId('l1-result-0-query') as HTMLInputElement).value).toBe('welch t test assumptions'));
    expect((within(panel).getByTestId('l1-optin-paper_doi_or_title') as HTMLInputElement).checked).toBe(false);
    fireEvent.change(screen.getByTestId('l1-result-0-query'), { target: { value: 'welch test unequal variances' } });
    fireEvent.click(screen.getByTestId('l1-result-0-query-apply'));
    await waitFor(() => expect(preText(panel)).toContain('welch test unequal variances'));
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    const ctx = await screen.findByTestId('l1-result-0-context');
    expect(ctx.textContent).toContain('Method context from the web');
    expect(within(ctx).getByTestId('l1-result-0-context-item').textContent).toContain('Retrieved 2026-10-14 17:46 UTC');
    const src = within(ctx).getByTestId('l1-result-0-source');
    expect(src.tagName).toBe('BUTTON');
    fireEvent.click(src);
    await waitFor(() => expect(b.calls.find(([n]) => n === 'openSourceLink')?.[1]).toEqual({ url: 'https://www.bmj.com/content/1' }));
    expect(ctx.querySelectorAll('a').length).toBe(0);
    // It sits apart from the engine's evidence: not inside the numbers table.
    expect(within(screen.getByTestId('l1-result-0')).getAllByTestId('l1-result-0-numbers')[0].contains(ctx)).toBe(false);
  });

  it('an empty context result shows its message', async () => {
    const b = fakeBridge({
      settings: { aiHelp: true, webContext: true },
      replies: { web_context: { kind: 'context', items: [], uncitedSentences: 0, droppedCitations: [], emptyMessage: 'No cited sources found' } },
    });
    await toResults(b);
    fireEvent.click(screen.getByTestId('l1-result-0-context-open'));
    const panel = await screen.findByTestId('l1-ai-panel-web_context');
    await waitFor(() => expect(within(panel).queryByTestId('l1-ai-request')).toBeTruthy());
    fireEvent.click(within(panel).getByTestId('l1-ai-send'));
    expect((await screen.findByTestId('l1-result-0-context-empty')).textContent).toBe('No cited sources found');
  });
});

describe('Level 1: settings and the run closing', () => {
  it('saves, tests and removes the key, showing only its status', async () => {
    const b = fakeBridge();
    render(<L1Flow bridge={b} />);
    fireEvent.click(screen.getByTestId('l1-settings-toggle'));
    expect((await screen.findByTestId('l1-key-status')).textContent).toBe('Key saved');
    fireEvent.change(screen.getByTestId('l1-key-input'), { target: { value: 'sk-test-SECRET' } });
    fireEvent.click(screen.getByTestId('l1-key-save'));
    await waitFor(() => expect((screen.getByTestId('l1-key-input') as HTMLInputElement).value).toBe(''));
    expect(document.body.textContent).not.toContain('sk-test-SECRET');
    fireEvent.click(screen.getByTestId('l1-key-test'));
    expect((await screen.findByTestId('l1-settings-message')).textContent).toBe('OpenAI accepted the key.');
    fireEvent.click(screen.getByTestId('l1-key-remove'));
    await waitFor(() => expect(screen.getByTestId('l1-key-status').textContent).toBe('No key'));
    fireEvent.click(screen.getByTestId('l1-ai-help-switch'));
    await waitFor(() => expect(b.calls.find(([n]) => n === 'settingSet')?.[1]).toMatchObject({ aiHelp: true }));
  });

  it('closing the check closes the Rust run and clears the headers and labels from the screen', async () => {
    const b = fakeBridge();
    await toBind(b);
    expect(document.body.textContent).toContain('anxiety_post');
    fireEvent.click(screen.getByTestId('l1-close'));
    await waitFor(() => expect(b.calls.find(([n]) => n === 'close')?.[1]).toEqual({ run: 7 }));
    expect(document.body.textContent).not.toContain('anxiety_post');
    expect(document.body.textContent).not.toContain('treatment');
    expect(screen.getByTestId('l1-manuscript-name').textContent).toBe('No manuscript chosen');
  });

  it('nothing is written to browser storage', async () => {
    window.localStorage.clear();
    window.sessionStorage.clear();
    const b = fakeBridge();
    await toResults(b);
    expect(window.localStorage.length).toBe(0);
    expect(window.sessionStorage.length).toBe(0);
  });
});

/* ------------------------------------------------------------------------ *
 *  The page and the flag
 * ------------------------------------------------------------------------ */

function auth(): AuthService {
  return {
    signUp: vi.fn(), signIn: vi.fn(), signInWithOAuth: vi.fn(), signOut: vi.fn(),
    getSession: vi.fn().mockResolvedValue({ session: null, offline: true }),
    onAuthStateChange: (cb: (s: null) => void) => { cb(null); return () => undefined; },
  } as unknown as AuthService;
}

const renderPage = (props: Record<string, unknown>) =>
  render(
    <MemoryRouter>
      <GaplySessionProvider authService={auth()}>
        <StatsVerifierPage bridge={makeMockStatsVerifierBridge({})} {...props} />
      </GaplySessionProvider>
    </MemoryRouter>
  );

describe('Level 1 on the Stats Verifier page', () => {
  it('the flag off hides the flow', async () => {
    flags.statsL1 = false;
    renderPage({ forceTier: 'premium', l1Bridge: fakeBridge() });
    expect(await screen.findByTestId('sv-entry')).toBeTruthy();
    expect(screen.queryByTestId('sv-l1-start')).toBeNull();
    expect(screen.queryByText('Check a paper against its data')).toBeNull();
  });

  it('the flag on shows the flow, behind the page gate', async () => {
    renderPage({ forceTier: 'premium', l1Bridge: fakeBridge() });
    fireEvent.click(await screen.findByTestId('sv-l1-start'));
    expect(await screen.findByTestId('l1-flow')).toBeTruthy();
  });

  it('the page gate still applies with the flag on', () => {
    renderPage({ forceTier: 'free', l1Bridge: fakeBridge() });
    expect(screen.getByTestId('sv-teaser')).toBeTruthy();
    expect(screen.queryByTestId('sv-l1-start')).toBeNull();
  });
});
