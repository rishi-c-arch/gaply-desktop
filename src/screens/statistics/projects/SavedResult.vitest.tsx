// The saved result view (Research OS, Phase 5B-3b; decision 150).
//
// * The shape check accepts every stored result Rust wrote for this step
//   (src-tauri/tests/fixtures/projects/stored/, pinned by
//   stored_results_have_the_layout_of_their_fixtures in
//   src-tauri/tests/project_commands.rs), and rejects each one mutated: every
//   field removed in turn, every number made null in turn, wrong types, an
//   unknown code, text that is not JSON.
// * A saved summary and a saved t-test render through the live screens' own
//   components, and show the same text as the live screens show for the same
//   result, with the time zone fixed.
// * A failed run shows its code's sentence. An unknown capability, an unknown
//   schema version, a wrong shape, a damaged artifact, and a saved summary
//   with the summary's flag off each show their own sentence and nothing of
//   the stored JSON. The error boundary's fallback is one fixed sentence.
// * No Save, re-run, edit or AI-explanation control; the data file only as
//   its kind, format and size. Under StrictMode, only reads.
import fs from 'node:fs';
import path from 'node:path';
import React from 'react';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import DescribeFlow from '../DescribeFlow';
import TTestFlow from '../TTestFlow';
import { makeMockDescribeBridge } from '../describeBridge';
import { makeMockTTestBridge } from '../ttestBridge';
import { datasetView } from '../fixtures/describeFixtures';
import { ttestSentence } from '../ttestWording';
import type { TTestDiagnostic } from '../ttestTypes';
import { makeMockProjectBridge } from './projectBridge';
import type { StoredRunView } from './projectTypes';
import { PROJECT_WORDS, SAVED_TEXT, savedResultLine } from './projectWording';
import { SavedResult, SavedResultBoundary, SavedResultBody } from './SavedResult';
import { checkStored, KNOWN_SCHEMA_VERSION } from './storedShape';
import { useGeneration } from './useGeneration';

const flags = vi.hoisted(() => ({ statsDescribe: true }));
vi.mock('../../../config/featureFlags', () => ({
  isFeatureEnabled: (name: string) => name === 'projects' || name === 'workspaces' || (name === 'statsDescribe' && flags.statsDescribe),
}));
vi.mock('../../../utils/isTauri', () => ({ isTauri: true, default: true }));

const TZ = process.env.TZ;
beforeAll(() => {
  process.env.TZ = 'UTC';
});
afterAll(() => {
  process.env.TZ = TZ;
});
beforeEach(() => {
  flags.statsDescribe = true;
});
afterEach(cleanup);

const STORED = path.join(__dirname, '../../../../src-tauri/tests/fixtures/projects/stored');
type Stored = Record<string, unknown> & { bundleJson: string | null; failureJson: string | null; specJson: string; manifestJson: string | null };
const stored = (name: string): Stored => JSON.parse(fs.readFileSync(path.join(STORED, name), 'utf8')) as Stored;
const SNAPSHOTS = fs.readdirSync(STORED).filter((f) => f.endsWith('.json')).sort();
const ID = '0123456789abcdef0123456789abcdef';

const RUN = (capabilityId: string, state: 'succeeded' | 'failed', storedRunId = 1): StoredRunView => ({
  storedRunId,
  capabilityId,
  capabilityVersion: 1,
  state,
  endCode: null,
  startedAt: 1792396000,
  endedAt: 1792396001,
  savedAt: 1792396100,
  agentVersion: '0.1.0',
});

/** Every path to a leaf (or a key) in a value: `["a", 0, "b"]`. */
function paths(v: unknown, at: Array<string | number> = [], out: Array<Array<string | number>> = []) {
  if (Array.isArray(v)) v.forEach((x, i) => paths(x, [...at, i], out));
  else if (v && typeof v === 'object') Object.entries(v as Record<string, unknown>).forEach(([k, x]) => {
    out.push([...at, k]);
    paths(x, [...at, k], out);
  });
  return out;
}

function getAt(v: unknown, p: Array<string | number>): unknown {
  return p.reduce<unknown>((o, k) => (o as Record<string | number, unknown>)[k], v);
}

function setAt(v: unknown, p: Array<string | number>, value: unknown | typeof DELETE): void {
  const parent = getAt(v, p.slice(0, -1)) as Record<string | number, unknown>;
  const k = p[p.length - 1];
  if (value === DELETE) {
    if (Array.isArray(parent)) parent.splice(k as number, 1);
    else delete parent[k];
  } else parent[k] = value;
}
const DELETE = Symbol('delete');

/** A snapshot with its stored text `field` changed by `change`. */
function withStored(name: string, field: 'bundleJson' | 'failureJson' | 'specJson', change: (v: unknown) => void): Stored {
  const s = stored(name);
  const v = JSON.parse(s[field] as string);
  change(v);
  return { ...s, [field]: JSON.stringify(v) };
}

/* ---- the shape check ----------------------------------------------------------- */

describe('the shape check', () => {
  it('has the three snapshots Rust wrote, and accepts every one', () => {
    expect(SNAPSHOTS).toEqual(['summary.json', 'ttest.json', 'ttest_failed.json']);
    expect(checkStored(stored('summary.json')).kind).toBe('summary');
    expect(checkStored(stored('ttest.json')).kind).toBe('ttest');
    const failed = checkStored(stored('ttest_failed.json'));
    expect(failed.kind).toBe('failed');
    expect(KNOWN_SCHEMA_VERSION).toBe(1);
  });

  it.each([
    ['summary.json', 'bundleJson'],
    ['ttest.json', 'bundleJson'],
    ['ttest_failed.json', 'failureJson'],
  ] as const)('%s: rejects it with any one field of its %s removed', (name, field) => {
    const all = paths(JSON.parse(stored(name)[field] as string)).filter((p) => typeof p[p.length - 1] === 'string');
    expect(all.length).toBeGreaterThan(5);
    const accepted: string[] = [];
    for (const p of all) {
      const s = withStored(name, field, (v) => setAt(v, p, DELETE));
      if (checkStored(s).kind !== 'badShape' && checkStored(s).kind !== 'unknownSchema') accepted.push(p.join('.'));
    }
    expect(accepted).toEqual([]);
  });

  it.each([
    ['summary.json', 'bundleJson'],
    ['ttest.json', 'bundleJson'],
    ['ttest_failed.json', 'failureJson'],
  ] as const)('%s: rejects it with any one number in its %s made null, or made a string', (name, field) => {
    const v0 = JSON.parse(stored(name)[field] as string);
    const numbers = paths(v0).filter((p) => typeof getAt(v0, p) === 'number');
    if (name !== 'ttest_failed.json') expect(numbers.length).toBeGreaterThan(10);
    for (const replacement of [null, '1']) {
      const accepted: string[] = [];
      for (const p of numbers) {
        const s = withStored(name, field, (v) => setAt(v, p, replacement));
        if (checkStored(s).kind !== 'badShape' && checkStored(s).kind !== 'unknownSchema') accepted.push(p.join('.'));
      }
      expect(accepted, String(replacement)).toEqual([]);
    }
  });

  it('rejects wrong types, unknown words and codes, extra groups, and text that is not JSON', () => {
    const bad = [
      withStored('ttest.json', 'bundleJson', (v) => ((v as { primaryOutputs: { variant: string } }).primaryOutputs.variant = 'evil')),
      withStored('ttest.json', 'bundleJson', (v) => ((v as { primaryOutputs: { groups: unknown[] } }).primaryOutputs.groups.push({ level: 2, label: 'x', n: 1, mean: 1, sd: 1 }))),
      withStored('ttest.json', 'bundleJson', (v) => ((v as { primaryOutputs: { limitations: string[] } }).primaryOutputs.limitations = ['HTSENTINEL_code'])),
      withStored('summary.json', 'bundleJson', (v) => ((v as { primaryOutputs: { columns: unknown } }).primaryOutputs.columns = 'HTSENTINEL')),
      withStored('summary.json', 'bundleJson', (v) => ((v as { manifest: { inputs: unknown } }).manifest.inputs = ['not a hash'])),
      withStored('ttest_failed.json', 'failureJson', (v) => ((v as { failure: { code: string } }).failure.code = 'HTSENTINEL_code')),
      { ...stored('summary.json'), bundleJson: '{"schemaVersion":1, HTSENTINEL' },
      { ...stored('summary.json'), bundleJson: null },
      { ...stored('summary.json'), state: 'cancelled' },
      { ...stored('summary.json'), dataFile: { kind: 'data_file', format: 'HTSENTINEL', bytes: 1, sha256: 'a'.repeat(64) } },
      { ...stored('summary.json'), storedRunId: null },
      withStored('summary.json', 'specJson', (v) => ((v as { capabilityId: string }).capabilityId = 'stats.analyze.independent_t')),
      null,
      'HTSENTINEL',
      [],
    ];
    for (const b of bad) expect(checkStored(b).kind).toBe('badShape');
  });

  it('names an unknown capability and an unknown schema version as such', () => {
    expect(checkStored({ ...stored('summary.json'), capabilityId: 'stats.analyze.anova' }).kind).toBe('unknownCapability');
    expect(checkStored(withStored('summary.json', 'bundleJson', (v) => ((v as { schemaVersion: number }).schemaVersion = 2))).kind).toBe('unknownSchema');
    expect(checkStored(withStored('ttest.json', 'specJson', (v) => ((v as { schemaVersion: number }).schemaVersion = 2))).kind).toBe('unknownSchema');
    expect(checkStored(withStored('ttest_failed.json', 'failureJson', (v) => ((v as { schemaVersion: number }).schemaVersion = 3))).kind).toBe('unknownSchema');
  });

  it('never throws, whatever it is given', () => {
    const hostile = { get capabilityId(): string { throw new Error('HTSENTINEL getter'); } };
    expect(checkStored(hostile).kind).toBe('badShape');
  });
});

/* ---- the view ------------------------------------------------------------------ */

function saved(view: unknown, run: StoredRunView, strict = false) {
  const bridge = makeMockProjectBridge({ project_run_result: view });
  const Host: React.FC = () => {
    const gen = useGeneration();
    return <SavedResult projectId={ID} run={run} bridge={bridge} gen={gen} />;
  };
  const ui = <Host />;
  render(strict ? <React.StrictMode>{ui}</React.StrictMode> : ui);
  return bridge;
}

/** What the live screens show for the same result. The live view is made
 *  from the stored bundle as Rust's stats_*_commands::result makes it. */
async function liveSummaryText(bundle: Record<string, unknown>): Promise<string> {
  const live = {
    runId: bundle.runId,
    status: bundle.status,
    columns: (bundle.primaryOutputs as { columns: unknown }).columns,
    diagnostics: bundle.diagnostics,
    manifest: bundle.manifest,
  };
  const bridge = makeMockDescribeBridge({
    stats_desc_open: datasetView,
    stats_desc_close: true,
    stats_desc_validate: { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 6, columns: 2, cells: 12 } },
    stats_desc_run: { runId: 1 },
    stats_desc_status: { runId: 1, state: 'succeeded', terminal: true, columnsDone: 2, columnsTotal: 2, elapsedMs: 0, cancelRequested: false, diagnostics: [] },
    stats_desc_result: live,
  });
  render(<DescribeFlow bridge={bridge} pickPath={() => Promise.resolve('/x/data.csv')} pollMs={5} />);
  fireEvent.click(screen.getByTestId('desc-choose'));
  fireEvent.click(await screen.findByTestId('desc-to-settings'));
  fireEvent.click(await screen.findByTestId('desc-check'));
  fireEvent.click(await screen.findByTestId('desc-run'));
  fireEvent.click(await screen.findByTestId('desc-see-results'));
  await screen.findByTestId('desc-manifest');
  // The result: from the notes to the manifest, as DescribeResult draws it.
  const card = screen.getByTestId('desc-manifest').closest('.gds-card') as HTMLElement;
  const text = Array.from(card.children)
    .filter((c) => !['desc-stage-heading', 'desc-project-save'].includes(c.getAttribute('data-testid') ?? '') && c.tagName !== 'DIV')
    .map((c) => c.textContent)
    .join('');
  cleanup();
  return text;
}

async function liveTTestText(bundle: Record<string, unknown>): Promise<string> {
  const r = bundle.primaryOutputs as { groups: Array<{ level: number; label: string; n: number }> };
  const bridge = makeMockTTestBridge({
    stats_desc_open: datasetView,
    stats_desc_close: true,
    stats_ttest_group_levels: ({ slot, column }: Record<string, unknown>) => ({
      slot,
      column,
      levels: r.groups.map((g) => ({ index: g.level, label: g.label, count: g.n })),
    }),
    stats_ttest_validate: { status: 'valid', errors: [], warnings: [], missingInformation: [], estimatedResources: { rows: 6, columns: 2, cells: 12 } },
    stats_ttest_run: { runId: 2 },
    stats_ttest_status: { runId: 2, state: 'succeeded', terminal: true, elapsedMs: 0, cancelRequested: false, diagnostics: [] },
    stats_ttest_result: { runId: bundle.runId, status: bundle.status, result: bundle.primaryOutputs, diagnostics: bundle.diagnostics, manifest: bundle.manifest },
  });
  render(<TTestFlow bridge={bridge} pickPath={async () => '/x/data.csv'} pollMs={5} />);
  fireEvent.click(screen.getByTestId('tt-choose'));
  fireEvent.click(await screen.findByTestId('tt-to-settings'));
  fireEvent.change(await screen.findByTestId('tt-outcome'), { target: { value: 'C2' } });
  fireEvent.change(screen.getByTestId('tt-group'), { target: { value: 'C1' } });
  await screen.findByTestId('tt-levels');
  fireEvent.click(screen.getByTestId('tt-variant-welch'));
  fireEvent.click(screen.getByTestId('tt-check'));
  fireEvent.click(await screen.findByTestId('tt-run'));
  fireEvent.click(await screen.findByTestId('tt-see-results'));
  const text = (await screen.findByTestId('tt-result-card')).textContent ?? '';
  cleanup();
  return text;
}

describe('a saved summary and a saved t-test', () => {
  it('a summary renders through DescribeResult, with the same text as the live results screen', async () => {
    const s = stored('summary.json');
    const live = await liveSummaryText(JSON.parse(s.bundleJson as string));
    expect(live.length).toBeGreaterThan(100);
    saved(s, RUN('stats.describe', 'succeeded'));
    const manifest = await screen.findByTestId('desc-manifest');
    const section = screen.getByTestId('saved-result');
    expect(screen.getByTestId('saved-result-line').textContent).toBe(savedResultLine(1792396100));
    expect(screen.getByTestId('saved-result-line').textContent).toBe('Saved result, saved 19 Oct 2026, 07:48.');
    // DescribeResult's own nodes, in order, after the data-file line.
    const drawn = Array.from(manifest.parentElement?.children ?? [])
      .filter((c) => !['saved-result-line', 'saved-result-kind', 'saved-data-file'].includes(c.getAttribute('data-testid') ?? ''))
      .map((c) => c.textContent)
      .join('');
    expect(drawn).toBe(live);
    expect(within(section).queryAllByRole('button')).toHaveLength(0);
  });

  it('a t-test renders through ResultCard, with the same text as the live results screen', async () => {
    const s = stored('ttest.json');
    const live = await liveTTestText(JSON.parse(s.bundleJson as string));
    expect(live.length).toBeGreaterThan(100);
    saved(s, RUN('stats.analyze.independent_t', 'succeeded', 2));
    expect((await screen.findByTestId('tt-result-card')).textContent).toBe(live);
    expect(within(screen.getByTestId('saved-result')).queryAllByRole('button')).toHaveLength(0);
  });

  // Finding b (Research OS, 5B-4b): the saved view draws the same tables, so
  // its cells carry the same right padding.
  it("pads every header and data cell of a saved result's tables on the right", async () => {
    const padded = (table: HTMLElement) => {
      const cells = Array.from(table.querySelectorAll<HTMLElement>('th, td'));
      expect(cells.length).toBeGreaterThan(0);
      expect(cells.filter((c) => c.style.paddingRight !== '12px').map((c) => `${c.tagName} ${c.textContent}`)).toEqual([]);
    };
    saved(stored('summary.json'), RUN('stats.describe', 'succeeded'));
    padded(await screen.findByTestId('desc-numeric-table'));
    const levels = screen.getAllByTestId('desc-levels-table');
    expect(levels.length).toBeGreaterThan(0);
    levels.forEach(padded);
    cleanup();
    saved(stored('ttest.json'), RUN('stats.analyze.independent_t', 'succeeded', 2));
    padded(await screen.findByTestId('tt-groups'));
  });

  it('has no Save, re-run, edit or AI-explanation control, and shows the data file only as its kind, format and size', async () => {
    for (const [name, run] of [
      ['summary.json', RUN('stats.describe', 'succeeded')],
      ['ttest.json', RUN('stats.analyze.independent_t', 'succeeded', 2)],
      ['ttest_failed.json', RUN('stats.analyze.independent_t', 'failed', 3)],
    ] as const) {
      const s = stored(name);
      saved(s, run);
      const facts = await screen.findByTestId('saved-data-file');
      const df = s.dataFile as { format: string; bytes: number; sha256: string };
      expect(facts.textContent).toBe(SAVED_TEXT.dataFile(df.format, df.bytes));
      const section = screen.getByTestId('saved-result');
      // The view's own words name the data file by kind, format and size. Its
      // fingerprint appears only where the live result component prints it,
      // the manifest's "Data file (SHA-256)" row, as on the live screen.
      const own = Array.from(section.children).filter((c) => !c.closest('[data-testid="desc-manifest"], [data-testid="tt-result-card"]'));
      expect(own.map((c) => c.textContent).join('')).not.toContain(df.sha256.slice(0, 12));
      const shown = section.textContent ?? '';
      const at = shown.indexOf(df.sha256.slice(0, 12));
      if (at >= 0) {
        const row = screen.getByTestId(run.capabilityId === 'stats.describe' ? 'desc-manifest-input' : 'tt-manifest-input');
        expect(row.textContent).toBe(`${df.sha256.slice(0, 12)}…`);
        expect(shown.indexOf(df.sha256.slice(0, 12), at + 1)).toBe(-1);
      }
      expect(within(section).queryAllByRole('button')).toHaveLength(0);
      expect(section.querySelectorAll('input, textarea, select, a[href]')).toHaveLength(0);
      for (const word of ['Save', 'Run again', 'Re-run', 'Edit', 'Explain', 'AI']) {
        expect(section.textContent, word).not.toMatch(new RegExp(`\\b${word}\\b`));
      }
      expect(screen.queryByTestId('tt-project-save')).toBeNull();
      expect(screen.queryByTestId('desc-project-save')).toBeNull();
      cleanup();
    }
  });

  it("a failed run shows its code's sentence", async () => {
    const s = stored('ttest_failed.json');
    saved(s, RUN('stats.analyze.independent_t', 'failed', 3));
    const failure = JSON.parse(s.failureJson as string).failure as TTestDiagnostic;
    expect((await screen.findByTestId('saved-result-failed')).textContent).toBe(`${SAVED_TEXT.failed} ${ttestSentence(failure)}`);
  });
});

describe('a saved result that is not shown', () => {
  const SENTINEL = 'HTSENTINEL_stored';
  const cases: Array<[string, unknown, string]> = [
    ['an unknown capability', { ...stored('summary.json'), capabilityId: 'stats.analyze.anova', specJson: `{"x":"${SENTINEL}"}` }, 'saved-result-unknown-capability'],
    [
      'an unknown schema version',
      withStored('summary.json', 'bundleJson', (v) => {
        (v as { schemaVersion: number }).schemaVersion = 2;
        (v as { note: string }).note = SENTINEL;
      }),
      'saved-result-unknown-schema',
    ],
    [
      'a wrong shape',
      withStored('summary.json', 'bundleJson', (v) => {
        const c = (v as { primaryOutputs: { columns: Array<{ numeric?: { header: string; mean: unknown } }> } }).primaryOutputs.columns[0];
        if (c.numeric) {
          c.numeric.header = SENTINEL;
          c.numeric.mean = null;
        }
      }),
      'saved-result-bad-shape',
    ],
    ['text that is not JSON', { ...stored('summary.json'), bundleJson: `{"header":"${SENTINEL}", oops` }, 'saved-result-bad-shape'],
  ];

  it.each(cases)('%s: its sentence, and nothing of the stored JSON', async (_name, view, testId) => {
    const errors = vi.spyOn(console, 'error');
    saved(view, RUN((view as { capabilityId: string }).capabilityId, 'succeeded'));
    const shown = await screen.findByTestId(testId);
    const sentences: Record<string, string> = {
      'saved-result-unknown-capability': SAVED_TEXT.unknownCapability,
      'saved-result-unknown-schema': SAVED_TEXT.unknownSchema,
      'saved-result-bad-shape': SAVED_TEXT.badShape,
    };
    expect(shown.textContent).toBe(sentences[testId]);
    expect(document.body.innerHTML).not.toContain(SENTINEL);
    expect(screen.queryByTestId('desc-manifest')).toBeNull();
    expect(screen.queryByTestId('saved-data-file')).toBeNull();
    expect(errors.mock.calls.flat().map(String).join('\n')).not.toContain(SENTINEL);
    errors.mockRestore();
  });

  it("a damaged artifact: its code's sentence", async () => {
    const bridge = makeMockProjectBridge({
      project_run_result: () => {
        throw { code: 'validation', message: JSON.stringify({ code: 'project.artifact_wrong_hash' }) };
      },
    });
    const Host: React.FC = () => {
      const gen = useGeneration();
      return <SavedResult projectId={ID} run={RUN('stats.describe', 'succeeded')} bridge={bridge} gen={gen} />;
    };
    render(<Host />);
    expect((await screen.findByTestId('saved-result-refused')).textContent).toBe(
      PROJECT_WORDS['project.artifact_wrong_hash']({ code: 'project.artifact_wrong_hash' })
    );
    expect(screen.queryByTestId('desc-manifest')).toBeNull();
  });

  it("a saved summary with the summary's flag off: a sentence, and the stored result is not even read", async () => {
    flags.statsDescribe = false;
    const bridge = saved(stored('summary.json'), RUN('stats.describe', 'succeeded'));
    expect((await screen.findByTestId('saved-result-hidden')).textContent).toBe(SAVED_TEXT.summaryHidden);
    expect(bridge.calls).toEqual([]);
    expect(screen.queryByTestId('desc-manifest')).toBeNull();
    cleanup();
    // A saved t-test is still shown (decision 150 names the summary).
    saved(stored('ttest.json'), RUN('stats.analyze.independent_t', 'succeeded', 2));
    expect(await screen.findByTestId('tt-result-card')).toBeTruthy();
  });

  it('the error boundary shows one fixed sentence if rendering throws, and nothing of the result', async () => {
    const errors = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    // A result that skipped the check: rendering it throws.
    const unchecked = { kind: 'ttest', result: { result: { groups: [], label: 'HTSENTINEL_b' } }, dataFile: { format: 'csv', bytes: 1 } };
    render(
      <SavedResultBoundary>
        <SavedResultBody checked={unchecked as never} />
      </SavedResultBoundary>
    );
    expect(screen.getByTestId('saved-result-fallback').textContent).toBe(SAVED_TEXT.cannotShow);
    expect(document.body.innerHTML).not.toContain('HTSENTINEL');
    expect(errors.mock.calls.flat().map(String).join('\n')).not.toContain('HTSENTINEL');
    errors.mockRestore();
  });

  it('under StrictMode, only project_run_result is called', async () => {
    const bridge = saved(stored('ttest.json'), RUN('stats.analyze.independent_t', 'succeeded', 2), true);
    await screen.findByTestId('tt-result-card');
    await act(async () => undefined);
    expect(bridge.calls.every(([c]) => c === 'project_run_result')).toBe(true);
    expect(bridge.calls.length).toBeGreaterThan(0);
  });
});
